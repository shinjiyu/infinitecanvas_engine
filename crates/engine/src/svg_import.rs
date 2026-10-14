//! 把 SVG 子集导入场景。
//!
//! ```text
//! SVG 文本
//!   │ roxmltree + svgtypes
//!   ▼
//! rect / circle / ellipse / line / polyline / polygon / path / text
//! image（data URL）     g / svg：变换压进子节点，不建组
//!   │ 锚点 = viewBox 原点（没有 viewBox 时是用户坐标原点）
//!   ▼
//! ImportSvgCommand ── 一条历史，撤销删掉整批
//! ```
//!
//! 不解析滤镜、动画、CSS 类、`<use>`、渐变、裁剪和 `preserveAspectRatio`。
//! 百分比、`em` 长度的图元会跳过。路径和折线没有填充图元，填充会改成沿轮廓的描边。

use crate::asset::{AssetTable, ImageAsset, MAX_IMAGE_EDGE};
use crate::command::{Command, CommandHistory};
use crate::element::{StrokeCap, StrokeJoin, TextAlign};
use crate::scene::{AnyElement, Scene};
use crate::{Circle, ElementId, ElementStyle, Image, Line, Path, Rectangle, Text};
use base64::Engine;
use infinitecanvas_math::{Color, Transform, Vec2};
use std::collections::HashSet;
use std::str::FromStr;

const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

/// SVG 导入失败。场景保持原样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgImportError {
    message: String,
}

impl SvgImportError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// 给人看的原因。
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for SvgImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SvgImportError {}

/// 解析 SVG 并写入场景，不进撤销栈。
///
/// `anchor` 是 viewBox 原点（没有 viewBox 时是用户坐标原点）要落到的世界坐标。
/// 成功时新元素 `selected` 为 true，并排在当前最大 `z_index` 之上。
///
/// # 错误
/// 空文本、非法 XML、根节点不是 `svg`，或没有任何可识别图元。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{apply_svg_import, Scene};
/// use infinitecanvas_math::Vec2;
///
/// let mut scene = Scene::new();
/// let ids = apply_svg_import(
///     &mut scene,
///     r#"<svg><rect x="0" y="0" width="10" height="4"/></svg>"#,
///     Vec2::new(8.0, 6.0),
/// )
/// .unwrap();
/// assert_eq!(ids.len(), 1);
/// assert_eq!(scene.count(), 1);
/// ```
pub fn apply_svg_import(
    scene: &mut Scene,
    svg: &str,
    anchor: Vec2,
) -> Result<Vec<ElementId>, SvgImportError> {
    let plan = plan_svg(svg, anchor)?;
    if plan.is_empty() {
        return Err(SvgImportError::new("SVG contains no supported shapes"));
    }
    apply_plan(scene, plan)
}

/// 解析 SVG、写入场景，并压入一条撤销记录。
///
/// 撤销会删掉这次导入的全部元素。重做按原来的 ID 放回。
/// 解析失败时场景和历史都不变。
///
/// # 参数
/// - `scene`: 目标场景
/// - `history`: 撤销栈
/// - `svg`: SVG 文档
/// - `anchor`: viewBox 原点的世界坐标
///
/// # 返回
/// 新元素 ID，文档顺序。
///
/// # 错误
/// 与 [`apply_svg_import`] 相同。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{import_svg, CommandHistory, Scene};
/// use infinitecanvas_math::Vec2;
///
/// let mut scene = Scene::new();
/// let mut history = CommandHistory::new();
/// let svg = r#"<svg><rect width="4" height="4"/><path d="M0 0 L8 0" stroke="black"/></svg>"#;
/// import_svg(&mut scene, &mut history, svg, Vec2::ZERO).unwrap();
/// assert_eq!(scene.count(), 2);
/// assert!(history.undo(&mut scene));
/// assert!(scene.is_empty());
/// ```
pub fn import_svg(
    scene: &mut Scene,
    history: &mut CommandHistory,
    svg: &str,
    anchor: Vec2,
) -> Result<Vec<ElementId>, SvgImportError> {
    let ids = apply_svg_import(scene, svg, anchor)?;
    history.push_executed(Box::new(ImportSvgCommand::applied(ids.clone(), scene)));
    Ok(ids)
}

/// 一次 SVG 导入。撤销删除整批元素。
///
/// 用 [`import_svg`] 创建并入栈。已经写入场景时用 [`ImportSvgCommand::applied`]，
/// 再 [`CommandHistory::push_executed`]。
#[derive(Debug)]
pub struct ImportSvgCommand {
    elements: Vec<AnyElement>,
    roots: Vec<ElementId>,
    parked: Vec<ImageAsset>,
}

impl ImportSvgCommand {
    /// 元素已经在场景里。`roots` 是 [`apply_svg_import`] 的返回值。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{apply_svg_import, CommandHistory, ImportSvgCommand, Scene};
    /// use infinitecanvas_math::Vec2;
    ///
    /// let mut scene = Scene::new();
    /// let ids = apply_svg_import(&mut scene, "<svg><rect width=\"2\" height=\"2\"/></svg>", Vec2::ZERO).unwrap();
    /// let mut history = CommandHistory::new();
    /// history.push_executed(Box::new(ImportSvgCommand::applied(ids, &scene)));
    /// assert!(history.undo(&mut scene));
    /// assert!(scene.is_empty());
    /// ```
    pub fn applied(roots: Vec<ElementId>, scene: &Scene) -> Self {
        let elements = roots
            .iter()
            .filter_map(|id| scene.get(*id).cloned())
            .collect();
        Self {
            elements,
            roots,
            parked: Vec::new(),
        }
    }
}

impl Command for ImportSvgCommand {
    fn execute(&mut self, scene: &mut Scene) {
        scene.restore_assets(std::mem::take(&mut self.parked));
        for element in &self.elements {
            scene.add(element.clone());
        }
    }

    fn undo(&mut self, scene: &mut Scene) {
        for id in self.roots.iter().rev() {
            scene.remove(*id);
        }
        self.parked = take_parked(scene, &self.elements);
    }

    fn name(&self) -> &'static str {
        "ImportSvg"
    }
}

fn take_parked(scene: &mut Scene, elements: &[AnyElement]) -> Vec<ImageAsset> {
    let keep: HashSet<String> = elements
        .iter()
        .filter_map(|element| match element {
            AnyElement::Image(image) if !image.asset_id.is_empty() => Some(image.asset_id.clone()),
            _ => None,
        })
        .collect();
    scene
        .take_detached_assets()
        .into_iter()
        .filter(|asset| keep.contains(&asset.id))
        .collect()
}

#[derive(Clone, Copy, Debug)]
struct Affine {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
}

impl Affine {
    fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    fn translate(tx: f32, ty: f32) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: tx,
            f: ty,
        }
    }

    fn scale(sx: f32, sy: f32) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            e: 0.0,
            f: 0.0,
        }
    }

    fn rotate_deg(angle: f32) -> Self {
        let (sin, cos) = angle.to_radians().sin_cos();
        Self {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            e: 0.0,
            f: 0.0,
        }
    }

    fn skew_x(angle: f32) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: angle.to_radians().tan(),
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    fn skew_y(angle: f32) -> Self {
        Self {
            a: 1.0,
            b: angle.to_radians().tan(),
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    /// `self` 是父变换，`child` 后作用在点上。
    fn mul(self, child: Self) -> Self {
        Self {
            a: self.a * child.a + self.c * child.b,
            b: self.b * child.a + self.d * child.b,
            c: self.a * child.c + self.c * child.d,
            d: self.b * child.c + self.d * child.d,
            e: self.a * child.e + self.c * child.f + self.e,
            f: self.b * child.e + self.d * child.f + self.f,
        }
    }

    fn apply(self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    fn apply_point(self, point: Vec2) -> Vec2 {
        let (x, y) = self.apply(point.x, point.y);
        Vec2::new(x, y)
    }

    /// 线性部分两列长度的几何平均，用来把描边从用户单位换到世界单位。
    fn scale_extent(self) -> f32 {
        let sx = (self.a * self.a + self.b * self.b).sqrt();
        let sy = (self.c * self.c + self.d * self.d).sqrt();
        let mean = (sx * sy).sqrt();
        if mean.is_finite() && mean > 1.0e-6 {
            mean
        } else {
            1.0
        }
    }
}

/// 分解成缩放、旋转、平移。有错切时返回 `None`。
fn decompose(m: Affine) -> Option<Transform> {
    let sx = (m.a * m.a + m.b * m.b).sqrt();
    if !sx.is_finite() || sx < 1.0e-5 {
        return None;
    }
    let sin = m.b / sx;
    let cos = m.a / sx;
    let det = m.a * m.d - m.b * m.c;
    let sy = det / sx;
    if !sy.is_finite() {
        return None;
    }
    let pred_c = -sin * sy;
    let pred_d = cos * sy;
    let tol = 1.0e-3 * sx.max(sy.abs()).max(1.0);
    if (m.c - pred_c).abs() > tol || (m.d - pred_d).abs() > tol {
        return None;
    }
    let rotation = sin.atan2(cos);
    if !rotation.is_finite() || !m.e.is_finite() || !m.f.is_finite() {
        return None;
    }
    Some(Transform {
        translation: Vec2::new(m.e, m.f),
        rotation,
        scale: Vec2::new(sx, sy),
    })
}

/// 局部原点放在 `(origin_x, origin_y)` 的用户空间点上。
fn transform_about(m: Affine, origin_x: f32, origin_y: f32) -> Option<Transform> {
    let mut transform = decompose(m)?;
    let (tx, ty) = m.apply(origin_x, origin_y);
    if !tx.is_finite() || !ty.is_finite() {
        return None;
    }
    transform.translation = Vec2::new(tx, ty);
    Some(transform)
}

#[derive(Clone, Debug)]
struct PaintStyle {
    fill: Option<Color>,
    stroke: Option<Color>,
    stroke_width: f32,
    opacity: f32,
    fill_opacity: f32,
    stroke_opacity: f32,
    cap: StrokeCap,
    join: StrokeJoin,
    dash: Vec<f32>,
    font_size: f32,
    text_anchor: TextAlign,
    font_family: Option<String>,
    visibility_hidden: bool,
}

impl PaintStyle {
    fn initial() -> Self {
        Self {
            fill: Some(Color::BLACK),
            stroke: None,
            stroke_width: 1.0,
            opacity: 1.0,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            cap: StrokeCap::Butt,
            join: StrokeJoin::Miter,
            dash: Vec::new(),
            font_size: 16.0,
            text_anchor: TextAlign::Left,
            font_family: None,
            visibility_hidden: false,
        }
    }

    fn to_element_style(&self) -> ElementStyle {
        let mut style = ElementStyle {
            fill: self.fill.map(|color| scale_alpha(color, self.fill_opacity)),
            stroke: self
                .stroke
                .map(|color| scale_alpha(color, self.stroke_opacity)),
            stroke_width: self.stroke_width,
            opacity: self.opacity.clamp(0.0, 1.0),
            stroke_dash: self.dash.clone(),
            stroke_cap: self.cap,
            stroke_join: self.join,
        };
        if style.stroke_width <= 0.0 {
            style.stroke = None;
        }
        style
    }
}

fn scale_alpha(mut color: Color, factor: f32) -> Color {
    color.a = (color.a * factor).clamp(0.0, 1.0);
    color
}

enum Planned {
    Shape(AnyElement),
    Image {
        bytes: Vec<u8>,
        mime: String,
        size: Option<Vec2>,
        transform: Transform,
        alpha: f32,
    },
}

fn plan_svg(svg: &str, anchor: Vec2) -> Result<Vec<Planned>, SvgImportError> {
    let text = svg.strip_prefix('\u{feff}').unwrap_or(svg);
    if text.trim().is_empty() {
        return Err(SvgImportError::new("SVG document is empty"));
    }
    if !anchor.x.is_finite() || !anchor.y.is_finite() {
        return Err(SvgImportError::new("import anchor must be finite"));
    }
    let doc = roxmltree::Document::parse(text)
        .map_err(|err| SvgImportError::new(format!("invalid SVG XML: {err}")))?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg" {
        return Err(SvgImportError::new("SVG root element is missing"));
    }
    if display_none(&root) {
        return Err(SvgImportError::new("SVG contains no supported shapes"));
    }
    let style = overlay_style(&PaintStyle::initial(), &root);
    let ctm = Affine::translate(anchor.x, anchor.y)
        .mul(svg_viewport(&root))
        .mul(parse_transform(root.attribute("transform").unwrap_or("")));
    let mut planned = Vec::new();
    for child in root.children().filter(|node| node.is_element()) {
        walk(child, ctm, &style, xml_preserve(&root), &mut planned);
    }
    if planned.is_empty() {
        return Err(SvgImportError::new("SVG contains no supported shapes"));
    }
    Ok(planned)
}

fn walk(
    node: roxmltree::Node<'_, '_>,
    parent_ctm: Affine,
    parent_style: &PaintStyle,
    parent_preserve: bool,
    out: &mut Vec<Planned>,
) {
    let name = node.tag_name().name();
    if matches!(
        name,
        "defs"
            | "symbol"
            | "clipPath"
            | "mask"
            | "pattern"
            | "marker"
            | "filter"
            | "style"
            | "script"
            | "metadata"
            | "title"
            | "desc"
            | "linearGradient"
            | "radialGradient"
            | "use"
    ) {
        return;
    }
    if display_none(&node) {
        return;
    }
    let style = overlay_style(parent_style, &node);
    let preserve = parent_preserve || xml_preserve(&node);
    let mut ctm = parent_ctm.mul(parse_transform(node.attribute("transform").unwrap_or("")));
    if name == "svg" {
        ctm = ctm.mul(svg_viewport(&node));
    }
    let hidden = style.visibility_hidden;
    match name {
        "g" | "a" | "svg" | "switch" => {
            for child in node.children().filter(|child| child.is_element()) {
                walk(child, ctm, &style, preserve, out);
            }
        }
        "rect" if !hidden => push_rect(&node, ctm, &style, out),
        "circle" if !hidden => push_circle(&node, ctm, &style, out),
        "ellipse" if !hidden => push_ellipse(&node, ctm, &style, out),
        "line" if !hidden => push_line(&node, ctm, &style, out),
        "polyline" if !hidden => push_poly(&node, ctm, &style, false, out),
        "polygon" if !hidden => push_poly(&node, ctm, &style, true, out),
        "path" if !hidden => push_path(&node, ctm, &style, out),
        "text" if !hidden => push_text(&node, ctm, &style, preserve, out),
        "image" if !hidden => push_image(&node, ctm, &style, out),
        _ => {
            for child in node.children().filter(|child| child.is_element()) {
                walk(child, ctm, &style, preserve, out);
            }
        }
    }
}

fn push_rect(
    node: &roxmltree::Node<'_, '_>,
    ctm: Affine,
    style: &PaintStyle,
    out: &mut Vec<Planned>,
) {
    let Some(width) = required_length(node, "width") else {
        return;
    };
    let Some(height) = required_length(node, "height") else {
        return;
    };
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let Some(x) = coord(node, "x") else {
        return;
    };
    let Some(y) = coord(node, "y") else {
        return;
    };
    let center_x = x + width * 0.5;
    let center_y = y + height * 0.5;
    let paint = style.to_element_style();
    let radius = corner_radius(node, width, height);
    if let Some(transform) = transform_about(ctm, center_x, center_y) {
        let mut rect = Rectangle::new(0, Vec2::ZERO, Vec2::new(width, height))
            .with_style(paint)
            .with_corner_radius(radius);
        rect.props.transform = transform;
        out.push(Planned::Shape(AnyElement::Rectangle(rect)));
        return;
    }
    let corners = [
        Vec2::new(x, y),
        Vec2::new(x + width, y),
        Vec2::new(x + width, y + height),
        Vec2::new(x, y + height),
    ];
    push_outline(&corners, true, ctm, style, out);
}

fn corner_radius(node: &roxmltree::Node<'_, '_>, width: f32, height: f32) -> f32 {
    let rx = optional_length(node, "rx");
    let ry = optional_length(node, "ry");
    let radius = match (rx, ry) {
        (Some(rx), Some(ry)) => rx.min(ry),
        (Some(rx), None) => rx,
        (None, Some(ry)) => ry,
        (None, None) => 0.0,
    };
    Rectangle::clamp_radius(Vec2::new(width, height), radius)
}

fn push_circle(
    node: &roxmltree::Node<'_, '_>,
    ctm: Affine,
    style: &PaintStyle,
    out: &mut Vec<Planned>,
) {
    let Some(radius) = required_length(node, "r") else {
        return;
    };
    if radius <= 0.0 {
        return;
    }
    let Some(cx) = coord(node, "cx") else {
        return;
    };
    let Some(cy) = coord(node, "cy") else {
        return;
    };
    if let Some(transform) = transform_about(ctm, cx, cy) {
        let mut circle = Circle::new(0, Vec2::ZERO, radius).with_style(style.to_element_style());
        circle.props.transform = transform;
        out.push(Planned::Shape(AnyElement::Circle(circle)));
        return;
    }
    push_sampled_ellipse(ctm, cx, cy, radius, radius, style, out);
}

fn push_ellipse(
    node: &roxmltree::Node<'_, '_>,
    ctm: Affine,
    style: &PaintStyle,
    out: &mut Vec<Planned>,
) {
    let Some(rx) = required_length(node, "rx") else {
        return;
    };
    let Some(ry) = required_length(node, "ry") else {
        return;
    };
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let Some(cx) = coord(node, "cx") else {
        return;
    };
    let Some(cy) = coord(node, "cy") else {
        return;
    };
    // 半径放进缩放，圆元素才能画出长短轴。描边会跟着这条非均匀缩放走。
    let stretched = Affine {
        a: ctm.a * rx,
        b: ctm.b * rx,
        c: ctm.c * ry,
        d: ctm.d * ry,
        e: 0.0,
        f: 0.0,
    };
    if let Some(mut transform) = decompose(stretched) {
        let (tx, ty) = ctm.apply(cx, cy);
        if tx.is_finite() && ty.is_finite() {
            transform.translation = Vec2::new(tx, ty);
            let mut circle = Circle::new(0, Vec2::ZERO, 1.0).with_style(style.to_element_style());
            circle.props.transform = transform;
            out.push(Planned::Shape(AnyElement::Circle(circle)));
            return;
        }
    }
    push_sampled_ellipse(ctm, cx, cy, rx, ry, style, out);
}

fn push_sampled_ellipse(
    ctm: Affine,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    style: &PaintStyle,
    out: &mut Vec<Planned>,
) {
    let mut points = Vec::with_capacity(33);
    for step in 0..=32 {
        let angle = step as f32 / 32.0 * std::f32::consts::TAU;
        points.push(Vec2::new(cx + rx * angle.cos(), cy + ry * angle.sin()));
    }
    push_outline(&points, true, ctm, style, out);
}

fn push_line(
    node: &roxmltree::Node<'_, '_>,
    ctm: Affine,
    style: &PaintStyle,
    out: &mut Vec<Planned>,
) {
    let Some(x1) = coord(node, "x1") else {
        return;
    };
    let Some(y1) = coord(node, "y1") else {
        return;
    };
    let Some(x2) = coord(node, "x2") else {
        return;
    };
    let Some(y2) = coord(node, "y2") else {
        return;
    };
    let Some(mut paint) = stroke_paint(style, ctm.scale_extent()) else {
        return;
    };
    paint.fill = None;
    let start = ctm.apply_point(Vec2::new(x1, y1));
    let end = ctm.apply_point(Vec2::new(x2, y2));
    if !start.x.is_finite() || !start.y.is_finite() || !end.x.is_finite() || !end.y.is_finite() {
        return;
    }
    out.push(Planned::Shape(AnyElement::Line(Line::from_world(
        0, start, end, paint, false,
    ))));
}

fn push_poly(
    node: &roxmltree::Node<'_, '_>,
    ctm: Affine,
    style: &PaintStyle,
    close: bool,
    out: &mut Vec<Planned>,
) {
    let Some(raw) = node.attribute("points") else {
        return;
    };
    let mut points: Vec<Vec2> = svgtypes::PointsParser::from(raw)
        .map(|(x, y)| Vec2::new(x as f32, y as f32))
        .filter(|point| point.x.is_finite() && point.y.is_finite())
        .collect();
    if points.len() < 2 {
        return;
    }
    if close {
        if let Some(first) = points.first().copied() {
            if points
                .last()
                .is_some_and(|last| (*last - first).length() > 1.0e-3)
            {
                points.push(first);
            }
        }
    }
    push_outline(&points, false, ctm, style, out);
}

fn push_path(
    node: &roxmltree::Node<'_, '_>,
    ctm: Affine,
    style: &PaintStyle,
    out: &mut Vec<Planned>,
) {
    let Some(data) = node.attribute("d") else {
        return;
    };
    let Some(subpaths) = flatten_path(data) else {
        return;
    };
    for subpath in subpaths {
        push_outline(&subpath, false, ctm, style, out);
    }
}

fn push_outline(
    local_points: &[Vec2],
    already_closed: bool,
    ctm: Affine,
    style: &PaintStyle,
    out: &mut Vec<Planned>,
) {
    let mut world: Vec<Vec2> = local_points
        .iter()
        .copied()
        .map(|point| ctm.apply_point(point))
        .filter(|point| point.x.is_finite() && point.y.is_finite())
        .collect();
    if already_closed {
        if let Some(first) = world.first().copied() {
            if world
                .last()
                .is_some_and(|last| (*last - first).length() > 1.0e-3)
            {
                world.push(first);
            }
        }
    }
    if world.len() < 2 {
        return;
    }
    let Some(paint) = stroke_paint(style, ctm.scale_extent()) else {
        return;
    };
    out.push(Planned::Shape(AnyElement::Path(Path::from_world_points(
        0, &world, paint,
    ))));
}

/// 路径只存折线。没有描边时用填充色描一圈，避免实心多边形消失。
fn stroke_paint(style: &PaintStyle, scale: f32) -> Option<ElementStyle> {
    let mut paint = style.to_element_style();
    if paint.stroke.is_none() {
        paint.stroke = paint.fill;
    }
    paint.fill = None;
    paint.stroke?;
    if paint.stroke_width <= 0.0 {
        paint.stroke_width = 1.0;
    }
    paint.stroke_width *= scale;
    if !paint.stroke_width.is_finite() || paint.stroke_width <= 0.0 {
        return None;
    }
    Some(paint)
}

fn push_text(
    node: &roxmltree::Node<'_, '_>,
    ctm: Affine,
    style: &PaintStyle,
    preserve: bool,
    out: &mut Vec<Planned>,
) {
    let content = collect_text(node, preserve);
    if content.trim().is_empty() {
        return;
    }
    let Some(x) = coord(node, "x") else {
        return;
    };
    let Some(y) = coord(node, "y") else {
        return;
    };
    let Some(color) = style
        .fill
        .map(|color| scale_alpha(color, style.fill_opacity))
        .or_else(|| {
            style
                .stroke
                .map(|color| scale_alpha(color, style.stroke_opacity))
        })
    else {
        return;
    };
    let transform = transform_about(ctm, x, y).unwrap_or_else(|| {
        let (tx, ty) = ctm.apply(x, y);
        Transform::from_translation(Vec2::new(tx, ty))
    });
    let mut text = Text::new(0, Vec2::ZERO, content)
        .with_font_size(style.font_size.max(1.0))
        .with_color(color)
        .with_align(style.text_anchor);
    if let Some(family) = &style.font_family {
        if !family.trim().is_empty() {
            text = text.with_font_family(family.clone());
        }
    }
    text.opacity = style.opacity.clamp(0.0, 1.0);
    if let Some(stroke) = style.stroke {
        if style.stroke_width > 0.0 {
            text.stroke = Some(scale_alpha(stroke, style.stroke_opacity));
            text.stroke_width = style.stroke_width;
        }
    }
    text.props.transform = transform;
    out.push(Planned::Shape(AnyElement::Text(text)));
}

fn collect_text(node: &roxmltree::Node<'_, '_>, preserve: bool) -> String {
    let mut lines = Vec::new();
    let mut buf = String::new();
    for child in node.children() {
        if child.is_text() {
            buf.push_str(child.text().unwrap_or(""));
        } else if child.is_element() && child.tag_name().name() == "tspan" {
            if !buf.is_empty() {
                lines.push(std::mem::take(&mut buf));
            }
            lines.push(collect_text(&child, preserve));
        }
    }
    if !buf.is_empty() {
        lines.push(buf);
    }
    let lines: Vec<String> = lines
        .into_iter()
        .map(|line| normalize_text(&line, preserve))
        .filter(|line| preserve || !line.is_empty())
        .collect();
    lines.join("\n")
}

fn normalize_text(text: &str, preserve: bool) -> String {
    if preserve {
        return text.to_string();
    }
    let mut out = String::new();
    let mut pending_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
            }
            pending_space = false;
            out.push(ch);
        }
    }
    out
}

fn push_image(
    node: &roxmltree::Node<'_, '_>,
    ctm: Affine,
    style: &PaintStyle,
    out: &mut Vec<Planned>,
) {
    let Some(href) = image_href(node) else {
        return;
    };
    let Some((mime, bytes)) = decode_data_image(href) else {
        return;
    };
    if AssetTable::new()
        .insert_encoded(&bytes, &mime, MAX_IMAGE_EDGE)
        .is_err()
    {
        return;
    }
    let Some(x) = coord(node, "x") else {
        return;
    };
    let Some(y) = coord(node, "y") else {
        return;
    };
    let Some(transform) = transform_about(ctm, x, y) else {
        return;
    };
    let size = match (
        optional_length(node, "width"),
        optional_length(node, "height"),
    ) {
        (Some(width), Some(height)) if width > 0.0 && height > 0.0 => {
            Some(Vec2::new(width, height))
        }
        _ => None,
    };
    out.push(Planned::Image {
        bytes,
        mime,
        size,
        transform,
        alpha: style.opacity.clamp(0.0, 1.0),
    });
}

fn image_href<'a>(node: &roxmltree::Node<'a, 'a>) -> Option<&'a str> {
    node.attribute("href")
        .or_else(|| node.attribute((XLINK_NS, "href")))
}

fn decode_data_image(href: &str) -> Option<(String, Vec<u8>)> {
    let rest = href.trim().strip_prefix("data:")?;
    let (meta, data) = rest.split_once(',')?;
    let mut mime = meta
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime == "image/jpg" {
        mime = "image/jpeg".to_string();
    }
    if mime != "image/png" && mime != "image/jpeg" {
        return None;
    }
    if !meta.to_ascii_lowercase().contains("base64") {
        return None;
    }
    let compact: String = data.chars().filter(|ch| !ch.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(compact)
        .ok()?;
    Some((mime, bytes))
}

fn apply_plan(scene: &mut Scene, plan: Vec<Planned>) -> Result<Vec<ElementId>, SvgImportError> {
    let mut z = scene
        .iter()
        .map(|element| element.as_element().props().z_index)
        .max()
        .unwrap_or(-1)
        .saturating_add(1);
    let mut ids = Vec::new();
    for item in plan {
        match place_one(scene, item, z) {
            Ok(id) => {
                ids.push(id);
                z = z.saturating_add(1);
            }
            Err(err) => {
                for id in ids.iter().rev() {
                    scene.remove(*id);
                }
                let _ = scene.take_detached_assets();
                return Err(err);
            }
        }
    }
    if ids.is_empty() {
        return Err(SvgImportError::new("SVG contains no supported shapes"));
    }
    Ok(ids)
}

fn place_one(scene: &mut Scene, item: Planned, z: i32) -> Result<ElementId, SvgImportError> {
    let mut element = match item {
        Planned::Shape(element) => element,
        Planned::Image {
            bytes,
            mime,
            size,
            transform,
            alpha,
        } => {
            let asset_id = scene
                .insert_image_bytes(&bytes, &mime)
                .map_err(|err| SvgImportError::new(err.to_string()))?;
            let (width, height) = scene
                .image_asset(&asset_id)
                .map(|asset| (asset.width, asset.height))
                .ok_or_else(|| SvgImportError::new("image asset disappeared during import"))?;
            let mut image = Image::from_asset(0, Vec2::ZERO, &asset_id, width, height);
            if let Some(size) = size {
                image.size = size;
            }
            image.alpha = alpha;
            image.props.transform = transform;
            AnyElement::Image(image)
        }
    };
    let id = alloc_id(scene);
    let props = element.as_element_mut().props_mut();
    props.id = id;
    props.z_index = z;
    props.selected = true;
    props.dirty = true;
    scene.add(element);
    Ok(id)
}

fn alloc_id(scene: &mut Scene) -> ElementId {
    loop {
        let id = scene.next_id();
        if !scene.contains(id) {
            return id;
        }
    }
}

fn svg_viewport(node: &roxmltree::Node<'_, '_>) -> Affine {
    let x = optional_length(node, "x").unwrap_or(0.0);
    let y = optional_length(node, "y").unwrap_or(0.0);
    let placed = Affine::translate(x, y);
    let Some(raw) = node
        .attribute("viewBox")
        .or_else(|| node.attribute("viewbox"))
    else {
        return placed;
    };
    let Ok(view_box) = svgtypes::ViewBox::from_str(raw) else {
        return placed;
    };
    let (Some(width), Some(height)) = (
        optional_length(node, "width"),
        optional_length(node, "height"),
    ) else {
        return placed.mul(Affine::translate(
            -(view_box.x as f32),
            -(view_box.y as f32),
        ));
    };
    if width <= 0.0 || height <= 0.0 || view_box.w <= 0.0 || view_box.h <= 0.0 {
        return placed.mul(Affine::translate(
            -(view_box.x as f32),
            -(view_box.y as f32),
        ));
    }
    let sx = width / view_box.w as f32;
    let sy = height / view_box.h as f32;
    placed.mul(Affine::scale(sx, sy)).mul(Affine::translate(
        -(view_box.x as f32),
        -(view_box.y as f32),
    ))
}

fn parse_transform(value: &str) -> Affine {
    if value.trim().is_empty() {
        return Affine::identity();
    }
    let lower = value.to_ascii_lowercase();
    let mut out = Affine::identity();
    for token in svgtypes::TransformListParser::from(lower.as_str()) {
        let Ok(token) = token else {
            break;
        };
        let part = match token {
            svgtypes::TransformListToken::Matrix { a, b, c, d, e, f } => Affine {
                a: a as f32,
                b: b as f32,
                c: c as f32,
                d: d as f32,
                e: e as f32,
                f: f as f32,
            },
            svgtypes::TransformListToken::Translate { tx, ty } => {
                Affine::translate(tx as f32, ty as f32)
            }
            svgtypes::TransformListToken::Scale { sx, sy } => Affine::scale(sx as f32, sy as f32),
            svgtypes::TransformListToken::Rotate { angle } => Affine::rotate_deg(angle as f32),
            svgtypes::TransformListToken::SkewX { angle } => Affine::skew_x(angle as f32),
            svgtypes::TransformListToken::SkewY { angle } => Affine::skew_y(angle as f32),
        };
        out = out.mul(part);
    }
    out
}

const PRESENTATION: &[&str] = &[
    "fill",
    "stroke",
    "stroke-width",
    "opacity",
    "fill-opacity",
    "stroke-opacity",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-dasharray",
    "font-size",
    "text-anchor",
    "font-family",
    "visibility",
];

fn overlay_style(parent: &PaintStyle, node: &roxmltree::Node<'_, '_>) -> PaintStyle {
    let mut style = parent.clone();
    let mut declared: Vec<(String, String)> = Vec::new();
    for name in PRESENTATION {
        if let Some(value) = node.attribute(*name) {
            declared.push(((*name).to_string(), value.to_string()));
        }
    }
    if let Some(inline) = node.attribute("style") {
        declared.extend(parse_style_decls(inline));
    }
    for (name, value) in &declared {
        apply_declared(&mut style, parent, name, value);
    }
    style
}

fn parse_style_decls(style: &str) -> Vec<(String, String)> {
    style
        .split(';')
        .filter_map(|part| {
            let (name, value) = part.split_once(':')?;
            let name = name.trim().to_ascii_lowercase();
            if name.is_empty() {
                return None;
            }
            Some((name, value.trim().to_string()))
        })
        .collect()
}

fn apply_declared(style: &mut PaintStyle, parent: &PaintStyle, name: &str, value: &str) {
    match name {
        "fill" => {
            if let Some(paint) = parse_paint(value) {
                style.fill = paint;
            }
        }
        "stroke" => {
            if let Some(paint) = parse_paint(value) {
                style.stroke = paint;
            }
        }
        "stroke-width" => {
            if let Some(width) = user_length(value) {
                style.stroke_width = width.max(0.0);
            }
        }
        "opacity" => {
            if let Some(opacity) = parse_opacity(value) {
                style.opacity = parent.opacity * opacity;
            }
        }
        "fill-opacity" => {
            if let Some(opacity) = parse_opacity(value) {
                style.fill_opacity = opacity;
            }
        }
        "stroke-opacity" => {
            if let Some(opacity) = parse_opacity(value) {
                style.stroke_opacity = opacity;
            }
        }
        "stroke-linecap" => {
            if let Some(cap) = StrokeCap::parse(value) {
                style.cap = cap;
            }
        }
        "stroke-linejoin" => {
            if let Some(join) = StrokeJoin::parse(value) {
                style.join = join;
            }
        }
        "stroke-dasharray" => {
            if let Some(dash) = parse_dash(value) {
                style.dash = dash;
            }
        }
        "font-size" => {
            if let Some(size) = user_length(value) {
                style.font_size = size.max(1.0);
            }
        }
        "text-anchor" => {
            if let Some(align) = TextAlign::parse(value) {
                style.text_anchor = align;
            }
        }
        "font-family" => {
            let family = value.trim().trim_matches(|ch| ch == '"' || ch == '\'');
            if !family.is_empty() {
                style.font_family = Some(family.to_string());
            }
        }
        "visibility" => {
            let lower = value.trim().to_ascii_lowercase();
            if lower == "hidden" || lower == "collapse" {
                style.visibility_hidden = true;
            } else if lower == "visible" {
                style.visibility_hidden = false;
            }
        }
        _ => {}
    }
}

/// `None` 表示 `none` 或无法映射的渐变。`Some(None)` 不该出现；继承用 `None` 返回值表示不改。
fn parse_paint(value: &str) -> Option<Option<Color>> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("inherit") {
        return None;
    }
    if trimmed.eq_ignore_ascii_case("none") {
        return Some(None);
    }
    if trimmed.eq_ignore_ascii_case("transparent") {
        return Some(Some(Color::new(0.0, 0.0, 0.0, 0.0)));
    }
    if trimmed.eq_ignore_ascii_case("currentcolor") {
        return Some(Some(Color::BLACK));
    }
    let lower = trimmed.to_ascii_lowercase();
    let paint = svgtypes::Paint::from_str(trimmed)
        .or_else(|_| svgtypes::Paint::from_str(lower.as_str()))
        .ok()?;
    Some(match paint {
        svgtypes::Paint::None => None,
        svgtypes::Paint::Inherit => return None,
        svgtypes::Paint::CurrentColor => Some(Color::BLACK),
        svgtypes::Paint::Color(color) => Some(svg_color(color)),
        svgtypes::Paint::FuncIRI(_, Some(svgtypes::PaintFallback::Color(color))) => {
            Some(svg_color(color))
        }
        svgtypes::Paint::FuncIRI(_, Some(svgtypes::PaintFallback::None)) => None,
        svgtypes::Paint::FuncIRI(_, _)
        | svgtypes::Paint::ContextFill
        | svgtypes::Paint::ContextStroke => None,
    })
}

fn svg_color(color: svgtypes::Color) -> Color {
    Color::new(
        color.red as f32 / 255.0,
        color.green as f32 / 255.0,
        color.blue as f32 / 255.0,
        color.alpha as f32 / 255.0,
    )
}

fn parse_opacity(value: &str) -> Option<f32> {
    let value = value.trim();
    let number = if let Some(percent) = value.strip_suffix('%') {
        percent.trim().parse::<f32>().ok()? / 100.0
    } else {
        value.parse::<f32>().ok()?
    };
    if number.is_finite() {
        Some(number.clamp(0.0, 1.0))
    } else {
        None
    }
}

fn parse_dash(value: &str) -> Option<Vec<f32>> {
    let lower = value.trim().to_ascii_lowercase();
    if lower.is_empty() || lower == "none" {
        return Some(Vec::new());
    }
    let mut dash = Vec::new();
    for part in lower.split(|ch: char| ch == ',' || ch.is_whitespace()) {
        if part.is_empty() {
            continue;
        }
        let length = user_length(part)?;
        if length < 0.0 {
            return None;
        }
        dash.push(length);
    }
    if dash.is_empty() {
        None
    } else {
        Some(dash)
    }
}

fn display_none(node: &roxmltree::Node<'_, '_>) -> bool {
    let inline = node
        .attribute("style")
        .and_then(|style| {
            parse_style_decls(style)
                .into_iter()
                .rev()
                .find(|(name, _)| name == "display")
                .map(|(_, value)| value)
        })
        .unwrap_or_default();
    let value = if inline.is_empty() {
        node.attribute("display").unwrap_or("")
    } else {
        inline.as_str()
    };
    value.trim().eq_ignore_ascii_case("none")
}

fn xml_preserve(node: &roxmltree::Node<'_, '_>) -> bool {
    node.attribute((XML_NS, "space"))
        .or_else(|| node.attribute("xml:space"))
        .is_some_and(|value| value.trim() == "preserve")
}

/// 属性缺失时返回 `None`。写了但单位无法换算时也是 `None`。
fn optional_length(node: &roxmltree::Node<'_, '_>, name: &str) -> Option<f32> {
    user_length(node.attribute(name)?)
}

/// 坐标缺省为 0。写了百分比或 `em` 时返回 `None`，调用方跳过该图元。
fn coord(node: &roxmltree::Node<'_, '_>, name: &str) -> Option<f32> {
    match node.attribute(name) {
        None => Some(0.0),
        Some(value) => user_length(value),
    }
}

fn required_length(node: &roxmltree::Node<'_, '_>, name: &str) -> Option<f32> {
    let raw = node.attribute(name)?;
    user_length(raw)
}

fn user_length(value: &str) -> Option<f32> {
    let length = svgtypes::Length::from_str(value.trim()).ok()?;
    let px = match length.unit {
        svgtypes::LengthUnit::None | svgtypes::LengthUnit::Px => length.number,
        svgtypes::LengthUnit::In => length.number * 96.0,
        svgtypes::LengthUnit::Cm => length.number * 96.0 / 2.54,
        svgtypes::LengthUnit::Mm => length.number * 96.0 / 25.4,
        svgtypes::LengthUnit::Pt => length.number * 96.0 / 72.0,
        svgtypes::LengthUnit::Pc => length.number * 16.0,
        svgtypes::LengthUnit::Em | svgtypes::LengthUnit::Ex | svgtypes::LengthUnit::Percent => {
            return None
        }
    };
    let px = px as f32;
    if px.is_finite() {
        Some(px)
    } else {
        None
    }
}

fn flatten_path(data: &str) -> Option<Vec<Vec<Vec2>>> {
    let mut subpaths = Vec::new();
    let mut current: Vec<Vec2> = Vec::new();
    for segment in svgtypes::SimplifyingPathParser::from(data) {
        let segment = segment.ok()?;
        match segment {
            svgtypes::SimplePathSegment::MoveTo { x, y } => {
                finish_subpath(&mut subpaths, &mut current);
                current.push(Vec2::new(x as f32, y as f32));
            }
            svgtypes::SimplePathSegment::LineTo { x, y } => {
                line_to(&mut current, x as f32, y as f32);
            }
            svgtypes::SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let start = current.last().copied().unwrap_or(Vec2::ZERO);
                flatten_cubic(
                    start,
                    Vec2::new(x1 as f32, y1 as f32),
                    Vec2::new(x2 as f32, y2 as f32),
                    Vec2::new(x as f32, y as f32),
                    &mut current,
                );
            }
            svgtypes::SimplePathSegment::Quadratic { x1, y1, x, y } => {
                let start = current.last().copied().unwrap_or(Vec2::ZERO);
                flatten_quadratic(
                    start,
                    Vec2::new(x1 as f32, y1 as f32),
                    Vec2::new(x as f32, y as f32),
                    &mut current,
                );
            }
            svgtypes::SimplePathSegment::ClosePath => {
                if let Some(first) = current.first().copied() {
                    push_distinct(&mut current, first);
                }
                finish_subpath(&mut subpaths, &mut current);
            }
        }
    }
    finish_subpath(&mut subpaths, &mut current);
    if subpaths.is_empty() {
        None
    } else {
        Some(subpaths)
    }
}

fn line_to(current: &mut Vec<Vec2>, x: f32, y: f32) {
    if current.is_empty() {
        current.push(Vec2::ZERO);
    }
    push_distinct(current, Vec2::new(x, y));
}

fn finish_subpath(subpaths: &mut Vec<Vec<Vec2>>, current: &mut Vec<Vec2>) {
    if current.len() >= 2 {
        subpaths.push(std::mem::take(current));
    } else {
        current.clear();
    }
}

fn push_distinct(points: &mut Vec<Vec2>, point: Vec2) {
    if !point.x.is_finite() || !point.y.is_finite() {
        return;
    }
    if points
        .last()
        .is_some_and(|last| (*last - point).length() <= 1.0e-4)
    {
        return;
    }
    points.push(point);
}

fn flatten_cubic(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, out: &mut Vec<Vec2>) {
    let length = (p0 - p1).length() + (p1 - p2).length() + (p2 - p3).length();
    let steps = ((length / 2.0).ceil() as usize).clamp(4, 24);
    if out.is_empty() {
        out.push(p0);
    }
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let u = 1.0 - t;
        let point =
            p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t);
        push_distinct(out, point);
    }
}

fn flatten_quadratic(p0: Vec2, p1: Vec2, p2: Vec2, out: &mut Vec<Vec2>) {
    let length = (p0 - p1).length() + (p1 - p2).length();
    let steps = ((length / 2.0).ceil() as usize).clamp(4, 24);
    if out.is_empty() {
        out.push(p0);
    }
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let u = 1.0 - t;
        let point = p0 * (u * u) + p1 * (2.0 * u * t) + p2 * (t * t);
        push_distinct(out, point);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::AnyElement;
    use infinitecanvas_math::Color;

    fn close_to(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1.0e-3, "{actual} != {expected}");
    }

    #[test]
    fn imports_rect_style_at_anchor() {
        let mut scene = Scene::new();
        let ids = apply_svg_import(
            &mut scene,
            r##"<svg><rect x="10" y="20" width="30" height="40" fill="#ff0000" stroke="#00ff00" stroke-width="2" opacity="0.5" rx="4"/></svg>"##,
            Vec2::new(1.0, 2.0),
        )
        .unwrap();
        let Some(AnyElement::Rectangle(rect)) = scene.get(ids[0]) else {
            panic!("expected a rectangle");
        };
        close_to(rect.size.x, 30.0);
        close_to(rect.size.y, 40.0);
        close_to(rect.props.transform.translation.x, 1.0 + 25.0);
        close_to(rect.props.transform.translation.y, 2.0 + 40.0);
        assert_eq!(rect.style.fill, Some(Color::RED));
        assert_eq!(rect.style.stroke, Some(Color::rgb(0.0, 1.0, 0.0)));
        close_to(rect.style.stroke_width, 2.0);
        close_to(rect.style.opacity, 0.5);
        close_to(rect.corner_radius(), 4.0);
        assert!(rect.props.selected);
    }

    #[test]
    fn imports_path_and_keeps_segments() {
        let mut scene = Scene::new();
        let ids = apply_svg_import(
            &mut scene,
            r##"<svg><path d="M0 0 L10 0 L10 10" stroke="#000" stroke-width="3"/></svg>"##,
            Vec2::new(1.0, 2.0),
        )
        .unwrap();
        let Some(AnyElement::Path(path)) = scene.get(ids[0]) else {
            panic!("expected a path");
        };
        let world: Vec<Vec2> = path
            .points()
            .iter()
            .map(|point| path.props.transform.transform_point(*point))
            .collect();
        assert_eq!(world.len(), 3);
        close_to(world[0].x, 1.0);
        close_to(world[0].y, 2.0);
        close_to(world[1].x, 11.0);
        close_to(world[2].y, 12.0);
        close_to(path.style.stroke_width, 3.0);
        assert!(path.style.fill.is_none());
        assert_eq!(path.style.stroke, Some(Color::BLACK));
    }

    #[test]
    fn empty_and_invalid_xml_do_not_change_the_scene() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(7, Vec2::ZERO, Vec2::ONE));
        for svg in [
            "",
            "   ",
            "<<<",
            "<svg>",
            "<notsvg/>",
            "<svg></svg>",
            "<svg><rect width=\"0\" height=\"4\"/></svg>",
        ] {
            let before = scene.count();
            let error = apply_svg_import(&mut scene, svg, Vec2::ZERO).unwrap_err();
            assert!(!error.message().is_empty(), "{svg}");
            assert_eq!(scene.count(), before, "{svg}");
            assert!(scene.get(7).is_some(), "{svg}");
        }
    }

    #[test]
    fn undo_rolls_back_one_import() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(2.0, 2.0)));
        let mut history = CommandHistory::new();
        let svg = r##"<svg>
            <rect width="4" height="4" fill="#336699"/>
            <path d="M0 0 L6 0" stroke="black"/>
        </svg>"##;
        let ids = import_svg(&mut scene, &mut history, svg, Vec2::new(3.0, 4.0)).unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(scene.count(), 3);
        assert_eq!(history.undo_count(), 1);
        assert!(history.undo(&mut scene));
        assert_eq!(scene.count(), 1);
        assert!(scene.get(1).is_some());
        assert!(history.redo(&mut scene));
        assert_eq!(scene.count(), 3);
        assert!(scene.get(ids[0]).is_some());
        assert!(scene.get(ids[1]).is_some());
    }

    #[test]
    fn nested_translate_circle_ellipse_line_and_polygon() {
        let mut scene = Scene::new();
        let svg = r##"<svg>
            <g transform="translate(10 20)">
              <g transform="translate(5 0)">
                <circle cx="0" cy="1" r="3" fill="#0000ff"/>
              </g>
            </g>
            <ellipse cx="4" cy="6" rx="8" ry="2" fill="#00ff00"/>
            <line x1="0" y1="0" x2="10" y2="0" stroke="#111" stroke-width="4" stroke-linecap="round"/>
            <polygon points="0,0 4,0 4,2" fill="#ff00ff"/>
        </svg>"##;
        let ids = apply_svg_import(&mut scene, svg, Vec2::ZERO).unwrap();
        assert_eq!(ids.len(), 4);
        let Some(AnyElement::Circle(circle)) = scene.get(ids[0]) else {
            panic!("circle");
        };
        close_to(circle.radius, 3.0);
        close_to(circle.props.transform.translation.x, 15.0);
        close_to(circle.props.transform.translation.y, 21.0);
        assert_eq!(circle.style.fill, Some(Color::BLUE));

        let Some(AnyElement::Circle(ellipse)) = scene.get(ids[1]) else {
            panic!("ellipse");
        };
        close_to(ellipse.radius, 1.0);
        close_to(ellipse.props.transform.translation.x, 4.0);
        close_to(ellipse.props.transform.translation.y, 6.0);
        close_to(ellipse.props.transform.scale.x, 8.0);
        close_to(ellipse.props.transform.scale.y, 2.0);

        let Some(AnyElement::Line(line)) = scene.get(ids[2]) else {
            panic!("line");
        };
        close_to(line.world_end().x - line.world_start().x, 10.0);
        assert_eq!(line.style.stroke_cap, StrokeCap::Round);

        let Some(AnyElement::Path(polygon)) = scene.get(ids[3]) else {
            panic!("polygon");
        };
        assert!(polygon.points().len() >= 4);
        assert_eq!(polygon.style.stroke, Some(Color::rgb(1.0, 0.0, 1.0)));
    }

    #[test]
    fn text_and_view_box_origin_follow_the_anchor() {
        let mut scene = Scene::new();
        let svg = r##"<svg viewBox="10 20 100 80">
            <text x="10" y="20" font-size="18" text-anchor="middle" fill="#ffffff">Hello</text>
            <rect x="10" y="20" width="5" height="6"/>
        </svg>"##;
        let ids = apply_svg_import(&mut scene, svg, Vec2::new(3.0, 4.0)).unwrap();
        let Some(AnyElement::Text(text)) = scene.get(ids[0]) else {
            panic!("text");
        };
        assert_eq!(text.content, "Hello");
        assert_eq!(text.align, TextAlign::Center);
        close_to(text.font_size, 18.0);
        close_to(text.props.transform.translation.x, 3.0);
        close_to(text.props.transform.translation.y, 4.0);
        let Some(AnyElement::Rectangle(rect)) = scene.get(ids[1]) else {
            panic!("rect");
        };
        close_to(rect.props.transform.translation.x, 3.0 + 2.5);
        close_to(rect.props.transform.translation.y, 4.0 + 3.0);
    }

    #[test]
    fn exported_rectangle_imports_back() {
        use crate::export_svg;
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::new(10.0, 20.0), Vec2::new(30.0, 40.0))
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        let svg = export_svg(&scene, false);
        let mut imported = Scene::new();
        let ids = apply_svg_import(&mut imported, &svg, Vec2::ZERO).unwrap();
        let Some(AnyElement::Rectangle(rect)) = imported.get(ids[0]) else {
            panic!("round trip should stay a rectangle, svg was {svg}");
        };
        close_to(rect.size.x, 30.0);
        close_to(rect.size.y, 40.0);
        assert_eq!(rect.style.fill, Some(Color::RED));
    }
}
