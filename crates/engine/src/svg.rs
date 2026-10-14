//! 场景 SVG 导出
//!
//! ```text
//! Scene
//!   │ elements_in_paint_order   ← 与 PNG 同一套遍历
//!   ▼
//! rect | circle | line+arrow | polyline | text
//! image → <image href=data URL>（无字节的纹理仍是注释）
//! group → <g id> 包住子孙，不再套一层组变换
//!         隐藏组：子孙不导出，也不计入 viewBox
//!   │
//!   ▼
//! SVG 字符串（viewBox = 导出元素的世界包围盒）
//! ```
//!
//! 变换写成 SVG `matrix(a b c d e f)`，列与 [`Transform::to_matrix`] 一致：
//! 先缩放，再绕局部原点旋转，最后平移。Y 轴向下，和画布相同。
//!
//! 文本写成 `<text>` / `<tspan>`。锚点与 [`TextAlign`] 相同：左上、上边中点或右上角。

use crate::element::{
    Circle, ElementId, ElementStyle, Image, Line, Path, Rectangle, StrokeCap, StrokeJoin, Text,
    TextAlign,
};
use crate::persist::elements_in_paint_order;
use crate::scene::{AnyElement, Scene};
use crate::snap::GuideAxis;
use infinitecanvas_math::{Color, Rect, Transform, Vec2};
use std::collections::{HashMap, HashSet};

/// viewBox 在元素包围盒外再留出的世界单位，避免描边贴边被裁掉。
const VIEW_PADDING: f32 = 2.0;

/// 把场景写成 SVG 文档。
///
/// 元素按与 PNG 导出相同的绘制顺序输出（`z_index` 升序，再按 id）。
/// 自己或祖先不可见的元素跳过（[`Scene::is_effectively_visible`]），
/// 隐藏组的子孙不进入文档，也不计入 viewBox。
/// `selected_only` 为 true 时导出自身 `selected` 的叶子，以及选中组下的全部可见子孙叶子。
/// viewBox 取这些叶子的包围盒；没有叶子时用 1×1，避免 `0 0 0 0`。
///
/// 矩形、圆、直线（含箭头）、自由路径、文本，以及带资源字节的图片写成对应的 SVG 图元，
/// 矩阵是叶子自己的世界变换。组不画几何。子孙在绘制顺序里连成一段时，组写成不带
/// `transform` 的 `<g id="eN">`。没有资源字节的图片跳过，并在 SVG 里留下注释，不计入 viewBox。
///
/// # 参数
/// - `scene`: 要导出的场景
/// - `selected_only`: 为 true 时只导出选中叶子，以及选中组的可见子孙叶子
///
/// # 返回
/// 完整 SVG 文档。没有可导出元素时仍返回合法 SVG，并带说明注释。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::math::{Color, Vec2};
/// use infinitecanvas_engine::{export_svg, ElementStyle, Rectangle, Scene};
///
/// let mut scene = Scene::new();
/// let rect = Rectangle::new(1, Vec2::new(10.0, 20.0), Vec2::new(4.0, 6.0))
///     .with_style(ElementStyle::fill_only(Color::RED));
/// scene.add_rect(rect);
/// let svg = export_svg(&scene, false);
/// assert!(svg.contains("<rect"));
/// assert!(svg.contains("fill=\"#ff0000\""));
/// ```
pub fn export_svg(scene: &Scene, selected_only: bool) -> String {
    export_svg_with_guides(scene, selected_only, false)
}

/// 同 [`export_svg`]。`include_guides` 为 true 时在 viewBox 内画出参考线。
///
/// 默认不画。参考线不改变 viewBox。标尺不进 SVG。
///
/// # 参数
/// - `scene`: 要导出的场景
/// - `selected_only`: 为 true 时只导出选中叶子
/// - `include_guides`: 为 true 时追加参考线
pub fn export_svg_with_guides(scene: &Scene, selected_only: bool, include_guides: bool) -> String {
    let mut bounds: Option<Rect> = None;
    let mut items: Vec<(Vec<ElementId>, String)> = Vec::new();

    for element in elements_in_paint_order(scene) {
        if !export_this_leaf(scene, element, selected_only) {
            continue;
        }
        let Some(leaf) = leaf_fragment(scene, element) else {
            continue;
        };
        // 没有像素的图片只留注释，不把占位包围盒算进 viewBox。
        if leaf.counts_toward_view_box {
            if let Some(element_bounds) = export_bounds(element) {
                bounds = Some(match bounds {
                    Some(existing) => existing.union(&element_bounds),
                    None => element_bounds,
                });
            }
        }
        items.push((ancestor_group_chain(scene, element), leaf.fragment));
    }

    let mut body = String::new();
    write_with_group_wrappers(&mut body, &items);

    // 空文档给 1×1，部分查看器拒绝 0×0 的 viewBox。
    let view = match bounds {
        Some(bounds) => bounds.inflate(VIEW_PADDING),
        None => Rect::new(0.0, 0.0, 1.0, 1.0),
    };

    let mut svg = String::new();
    svg.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\">\n",
        fmt_num(view.width),
        fmt_num(view.height),
        fmt_num(view.x),
        fmt_num(view.y),
        fmt_num(view.width),
        fmt_num(view.height),
    ));
    if items.is_empty() {
        if selected_only {
            svg.push_str("  <!-- nothing selected -->\n");
        } else {
            svg.push_str("  <!-- empty scene -->\n");
        }
    }
    svg.push_str(&body);
    if include_guides {
        write_guides(&mut svg, scene, view);
    }
    svg.push_str("</svg>\n");
    svg
}

fn write_guides(svg: &mut String, scene: &Scene, view: Rect) {
    for guide in scene.guides().iter() {
        match guide.axis {
            GuideAxis::Vertical => {
                svg.push_str(&format!(
                    "  <line x1=\"{x}\" y1=\"{y0}\" x2=\"{x}\" y2=\"{y1}\" stroke=\"#3db7ff\" stroke-width=\"1\"/>\n",
                    x = fmt_num(guide.position),
                    y0 = fmt_num(view.y),
                    y1 = fmt_num(view.bottom()),
                ));
            }
            GuideAxis::Horizontal => {
                svg.push_str(&format!(
                    "  <line x1=\"{x0}\" y1=\"{y}\" x2=\"{x1}\" y2=\"{y}\" stroke=\"#3db7ff\" stroke-width=\"1\"/>\n",
                    x0 = fmt_num(view.x),
                    y = fmt_num(guide.position),
                    x1 = fmt_num(view.right()),
                ));
            }
        }
    }
}

/// 叶子才输出。组没有几何。可见性走 [`Scene::is_effectively_visible`]，
/// 这样隐藏组会藏住子孙。`selected_only` 时，选中组带上全部可见子孙叶子。
fn export_this_leaf(scene: &Scene, element: &AnyElement, selected_only: bool) -> bool {
    if matches!(element, AnyElement::Group(_)) || !scene.is_effectively_visible(element.id()) {
        return false;
    }
    if !selected_only {
        return true;
    }
    element.as_element().props().selected || selected_via_ancestor(scene, element)
}

fn selected_via_ancestor(scene: &Scene, element: &AnyElement) -> bool {
    let mut current = element.as_element().props().parent;
    while let Some(id) = current {
        let Some(ancestor) = scene.get(id) else {
            return false;
        };
        if ancestor.as_element().props().selected {
            return true;
        }
        current = ancestor.as_element().props().parent;
    }
    false
}

/// 从根到直接父组。叶子的世界矩阵不在这里，组节点只用来包一层 `<g>`。
fn ancestor_group_chain(scene: &Scene, element: &AnyElement) -> Vec<ElementId> {
    let mut chain = Vec::new();
    let mut current = element.as_element().props().parent;
    while let Some(id) = current {
        let Some(ancestor) = scene.get(id) else {
            break;
        };
        if matches!(ancestor, AnyElement::Group(_)) {
            chain.push(id);
        }
        current = ancestor.as_element().props().parent;
    }
    chain.reverse();
    chain
}

/// 一条叶子的 SVG 片段。`counts_toward_view_box` 为 false 时是说明注释，不占包围盒。
struct ExportedLeaf {
    fragment: String,
    counts_toward_view_box: bool,
}

fn leaf_fragment(scene: &Scene, element: &AnyElement) -> Option<ExportedLeaf> {
    match element {
        AnyElement::Image(image) => Some(export_image(scene, image)),
        other => write_element(other).map(|fragment| ExportedLeaf {
            fragment,
            counts_toward_view_box: true,
        }),
    }
}

/// 有编码字节时嵌 `<image>`；否则留注释。可见性和选区由调用方按叶子规则处理。
fn export_image(scene: &Scene, image: &Image) -> ExportedLeaf {
    if let Some(fragment) = embedded_image(scene, image) {
        return ExportedLeaf {
            fragment,
            counts_toward_view_box: true,
        };
    }
    ExportedLeaf {
        fragment: format!(
            "  <!-- image {} skipped: texture pixels are not stored in the scene -->\n",
            image.props.id
        ),
        counts_toward_view_box: false,
    }
}

/// 把资源 data URL 写成 `<image>`。局部原点在左上角，宽高是显示尺寸。
///
/// UV 裁剪用嵌套 `<svg viewBox>` 取出像素区域，再拉伸到显示尺寸，和纹理四边形一致。
fn embedded_image(scene: &Scene, image: &Image) -> Option<String> {
    if image.asset_id.is_empty() {
        return None;
    }
    let asset = scene.image_asset(&image.asset_id)?;
    if asset.encoded.is_empty() || asset.width == 0 || asset.height == 0 {
        return None;
    }
    let href = xml_escape(&asset.to_data_url());
    let width = image.size.x.abs();
    let height = image.size.y.abs();
    let body = if let Some(crop) = pixel_crop(image, asset.width, asset.height) {
        format!(
            "    <svg x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\" preserveAspectRatio=\"none\">\n      <image href=\"{}\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\"/>\n    </svg>\n",
            fmt_num(width),
            fmt_num(height),
            fmt_num(crop.x),
            fmt_num(crop.y),
            fmt_num(crop.width),
            fmt_num(crop.height),
            href,
            asset.width,
            asset.height,
        )
    } else {
        format!(
            "    <image href=\"{}\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\"/>\n",
            href,
            fmt_num(width),
            fmt_num(height),
        )
    };
    Some(wrap_group(
        image.props.id,
        &image.props.transform,
        image.alpha,
        &body,
    ))
}

/// UV 矩形换成像素 viewBox。整张图或非正尺寸返回 `None`，调用方直接画满显示矩形。
fn pixel_crop(image: &Image, width: u32, height: u32) -> Option<Rect> {
    let rect = image.uv_rect?;
    if ![rect.x, rect.y, rect.width, rect.height]
        .into_iter()
        .all(f32::is_finite)
    {
        return None;
    }
    let full = rect.x.abs() <= 1.0e-4
        && rect.y.abs() <= 1.0e-4
        && (rect.width - 1.0).abs() <= 1.0e-4
        && (rect.height - 1.0).abs() <= 1.0e-4;
    if full || rect.width <= 0.0 || rect.height <= 0.0 {
        return None;
    }
    Some(Rect::new(
        rect.x * width as f32,
        rect.y * height as f32,
        rect.width * width as f32,
        rect.height * height as f32,
    ))
}

/// 子孙在绘制顺序里连成一段的组才包 `<g>`。中间夹了别的叶子时不包，以免打乱 z 顺序或写出重复 id。
fn contiguous_group_ids(items: &[(Vec<ElementId>, String)]) -> HashSet<ElementId> {
    let mut span: HashMap<ElementId, (usize, usize, usize)> = HashMap::new();
    for (index, (chain, _)) in items.iter().enumerate() {
        for id in chain {
            let entry = span.entry(*id).or_insert((index, index, 0));
            entry.1 = index;
            entry.2 += 1;
        }
    }
    span.into_iter()
        .filter(|(_, (first, last, count))| last - first + 1 == *count)
        .map(|(id, _)| id)
        .collect()
}

fn write_with_group_wrappers(body: &mut String, items: &[(Vec<ElementId>, String)]) {
    let wrap = contiguous_group_ids(items);
    let mut open: Vec<ElementId> = Vec::new();
    for (chain, fragment) in items {
        let desired: Vec<ElementId> = chain
            .iter()
            .copied()
            .filter(|id| wrap.contains(id))
            .collect();
        let mut prefix = 0;
        while prefix < open.len() && prefix < desired.len() && open[prefix] == desired[prefix] {
            prefix += 1;
        }
        while open.len() > prefix {
            open.pop();
            push_indent(body, open.len() + 1);
            body.push_str("</g>\n");
        }
        while open.len() < desired.len() {
            let id = desired[open.len()];
            push_indent(body, open.len() + 1);
            body.push_str(&format!("<g id=\"e{id}\">\n"));
            open.push(id);
        }
        body.push_str(&indent_block(fragment, open.len()));
    }
    while !open.is_empty() {
        open.pop();
        push_indent(body, open.len() + 1);
        body.push_str("</g>\n");
    }
}

fn push_indent(body: &mut String, level: usize) {
    for _ in 0..level {
        body.push_str("  ");
    }
}

fn indent_block(fragment: &str, extra_levels: usize) -> String {
    if extra_levels == 0 {
        return fragment.to_string();
    }
    let pad = "  ".repeat(extra_levels);
    let mut out = String::new();
    for line in fragment.split_inclusive('\n') {
        if line.trim().is_empty() {
            out.push_str(line);
        } else {
            out.push_str(&pad);
            out.push_str(line);
        }
    }
    out
}

fn write_element(element: &AnyElement) -> Option<String> {
    match element {
        AnyElement::Rectangle(rect) => Some(write_rectangle(rect)),
        AnyElement::Circle(circle) => Some(write_circle(circle)),
        AnyElement::Line(line) => Some(write_line(line)),
        AnyElement::Path(path) => write_path(path),
        AnyElement::Text(text) => write_text(text),
        AnyElement::Image(_) | AnyElement::Group(_) => None,
    }
}

fn write_rectangle(rect: &Rectangle) -> String {
    let width = rect.size.x.abs();
    let height = rect.size.y.abs();
    let mut shape = format!(
        "    <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
        fmt_num(-width * 0.5),
        fmt_num(-height * 0.5),
        fmt_num(width),
        fmt_num(height),
    );
    let radius = rect.effective_corner_radius();
    if radius > 0.0 {
        shape.push_str(&format!(
            " rx=\"{}\" ry=\"{}\"",
            fmt_num(radius),
            fmt_num(radius)
        ));
    }
    push_fill_stroke(
        &mut shape,
        rect.style.fill,
        rect.style.stroke,
        rect.style.stroke_width,
    );
    push_shape_line_style(&mut shape, &rect.style);
    shape.push_str("/>\n");
    wrap_group(
        rect.props.id,
        &rect.props.transform,
        rect.style.opacity,
        &shape,
    )
}

fn write_circle(circle: &Circle) -> String {
    let mut shape = format!(
        "    <circle cx=\"0\" cy=\"0\" r=\"{}\"",
        fmt_num(circle.radius.abs())
    );
    push_fill_stroke(
        &mut shape,
        circle.style.fill,
        circle.style.stroke,
        circle.style.stroke_width,
    );
    push_shape_line_style(&mut shape, &circle.style);
    shape.push_str("/>\n");
    wrap_group(
        circle.props.id,
        &circle.props.transform,
        circle.style.opacity,
        &shape,
    )
}

fn write_line(line: &Line) -> String {
    let (start, end, arrow) = line.local_shaft_and_arrow();
    let (color, _) = stroke_paint(&line.style);
    let mut body = String::new();
    body.push_str(&format!(
        "    <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"",
        fmt_num(start.x),
        fmt_num(start.y),
        fmt_num(end.x),
        fmt_num(end.y),
    ));
    push_stroke_only(&mut body, &line.style);
    body.push_str("/>\n");
    if let Some(tri) = arrow {
        body.push_str("    <polygon points=\"");
        body.push_str(&points_attr(&tri));
        body.push_str("\" fill=\"");
        body.push_str(&rgb_hex(color));
        body.push('"');
        push_alpha_attr(&mut body, "fill-opacity", color.a);
        body.push_str(" stroke=\"none\"/>\n");
    }
    wrap_group(
        line.props.id,
        &line.props.transform,
        line.style.opacity,
        &body,
    )
}

/// 文本锚点在局部原点。行距是字号的 1.25 倍，和 [`Text`] 的估算高度一致。
fn write_text(text: &Text) -> Option<String> {
    if text.content.is_empty() {
        return None;
    }
    let font_size = if text.font_size.is_finite() && text.font_size >= 1.0 {
        text.font_size
    } else {
        32.0
    };
    let anchor = match text.align {
        TextAlign::Left => "start",
        TextAlign::Center => "middle",
        TextAlign::Right => "end",
    };
    let mut body = format!(
        "    <text fill=\"{}\" font-size=\"{}\" font-family=\"{}\" text-anchor=\"{}\" dominant-baseline=\"text-before-edge\" xml:space=\"preserve\"",
        rgb_hex(text.color),
        fmt_num(font_size),
        xml_escape(&text.font_family),
        anchor,
    );
    push_alpha_attr(&mut body, "fill-opacity", text.color.a);
    if text.stroke_width > 0.0 {
        let stroke = text.stroke.unwrap_or(text.color);
        body.push_str(" stroke=\"");
        body.push_str(&rgb_hex(stroke));
        body.push_str("\" stroke-width=\"");
        body.push_str(&fmt_num(text.stroke_width));
        body.push('"');
        push_alpha_attr(&mut body, "stroke-opacity", stroke.a);
    }
    body.push_str(">\n");
    let line_height = font_size * 1.25;
    for (index, line) in text.content.split('\n').enumerate() {
        body.push_str(&format!(
            "      <tspan x=\"0\" y=\"{}\">{}</tspan>\n",
            fmt_num(index as f32 * line_height),
            xml_escape(line),
        ));
    }
    body.push_str("    </text>\n");
    Some(wrap_group(
        text.props.id,
        &text.props.transform,
        text.opacity,
        &body,
    ))
}

fn write_path(path: &Path) -> Option<String> {
    if path.points().len() < 2 {
        return None;
    }
    let mut body = format!("    <polyline points=\"{}\"", points_attr(path.points()));
    push_stroke_only(&mut body, &path.style);
    body.push_str("/>\n");
    Some(wrap_group(
        path.props.id,
        &path.props.transform,
        path.style.opacity,
        &body,
    ))
}

fn wrap_group(id: u64, transform: &Transform, opacity: f32, body: &str) -> String {
    let mut group = format!("  <g id=\"e{id}\"");
    if let Some(matrix) = matrix_value(transform) {
        group.push_str(" transform=\"matrix(");
        group.push_str(&matrix);
        group.push_str(")\"");
    }
    if let Some(value) = unit_alpha(opacity) {
        group.push_str(" opacity=\"");
        group.push_str(&value);
        group.push('"');
    }
    group.push_str(">\n");
    group.push_str(body);
    group.push_str("  </g>\n");
    group
}

fn push_fill_stroke(
    shape: &mut String,
    fill: Option<Color>,
    stroke: Option<Color>,
    stroke_width: f32,
) {
    match fill {
        Some(color) => {
            shape.push_str(" fill=\"");
            shape.push_str(&rgb_hex(color));
            shape.push('"');
            push_alpha_attr(shape, "fill-opacity", color.a);
        }
        None => shape.push_str(" fill=\"none\""),
    }
    if let Some(color) = stroke {
        if stroke_width > 0.0 {
            shape.push_str(" stroke=\"");
            shape.push_str(&rgb_hex(color));
            shape.push_str("\" stroke-width=\"");
            shape.push_str(&fmt_num(stroke_width));
            shape.push('"');
            push_alpha_attr(shape, "stroke-opacity", color.a);
            return;
        }
    }
    shape.push_str(" stroke=\"none\"");
}

fn push_shape_line_style(shape: &mut String, style: &ElementStyle) {
    if style.stroke.is_none() || style.stroke_width <= 0.0 {
        return;
    }
    if style.stroke_cap != StrokeCap::Butt {
        shape.push_str(" stroke-linecap=\"");
        shape.push_str(style.stroke_cap.as_str());
        shape.push('"');
    }
    if style.stroke_join != StrokeJoin::Miter {
        shape.push_str(" stroke-linejoin=\"");
        shape.push_str(style.stroke_join.as_str());
        shape.push('"');
    }
    push_dasharray(shape, &style.stroke_dash);
}

fn push_dasharray(shape: &mut String, dash: &[f32]) {
    if dash.is_empty() {
        return;
    }
    shape.push_str(" stroke-dasharray=\"");
    for (index, value) in dash.iter().enumerate() {
        if index > 0 {
            shape.push(' ');
        }
        shape.push_str(&fmt_num(*value));
    }
    shape.push('"');
}

fn push_stroke_only(shape: &mut String, style: &ElementStyle) {
    let (color, width) = stroke_paint(style);
    shape.push_str(" fill=\"none\" stroke=\"");
    shape.push_str(&rgb_hex(color));
    shape.push_str("\" stroke-width=\"");
    shape.push_str(&fmt_num(width));
    shape.push('"');
    push_alpha_attr(shape, "stroke-opacity", color.a);
    shape.push_str(" stroke-linecap=\"");
    shape.push_str(style.stroke_cap.as_str());
    shape.push_str("\" stroke-linejoin=\"");
    shape.push_str(style.stroke_join.as_str());
    shape.push_str("\" stroke-miterlimit=\"4\"");
    push_dasharray(shape, &style.stroke_dash);
}

fn push_alpha_attr(shape: &mut String, name: &str, alpha: f32) {
    if let Some(value) = unit_alpha(alpha) {
        shape.push(' ');
        shape.push_str(name);
        shape.push_str("=\"");
        shape.push_str(&value);
        shape.push('"');
    }
}

/// 与 `element::stroke::paint_color` 的取色一致，但透明度留给分组上的 `opacity`。
/// 线宽非正时回落到 1，与 `effective_width` 一致。
fn stroke_paint(style: &ElementStyle) -> (Color, f32) {
    let color = style.stroke.or(style.fill).unwrap_or(Color::BLACK);
    let width = if style.stroke_width > 0.0 {
        style.stroke_width
    } else {
        1.0
    };
    (color, width)
}

fn export_bounds(element: &AnyElement) -> Option<Rect> {
    let base = element.as_element().world_bounds();
    if ![base.x, base.y, base.width, base.height]
        .into_iter()
        .all(f32::is_finite)
    {
        return None;
    }
    Some(base.inflate(stroke_overflow(element)))
}

/// 描边画出局部包围盒之外的部分。直线和路径的 `local_bounds` 已经含线宽。
fn stroke_overflow(element: &AnyElement) -> f32 {
    match element {
        AnyElement::Rectangle(rect) => half_stroke(
            rect.style.stroke,
            rect.style.stroke_width,
            &rect.props.transform,
        ),
        AnyElement::Circle(circle) => half_stroke(
            circle.style.stroke,
            circle.style.stroke_width,
            &circle.props.transform,
        ),
        AnyElement::Line(line) if line.arrow() => arrow_overflow(line),
        AnyElement::Line(_)
        | AnyElement::Path(_)
        | AnyElement::Image(_)
        | AnyElement::Text(_)
        | AnyElement::Group(_) => 0.0,
    }
}

fn half_stroke(stroke: Option<Color>, width: f32, transform: &Transform) -> f32 {
    if stroke.is_none() || width <= 0.0 {
        return 0.0;
    }
    width.abs() * 0.5 * scale_extent(transform)
}

/// 箭翼可能比 `local_bounds` 的斜接留白再宽一点。
fn arrow_overflow(line: &Line) -> f32 {
    let width = if line.style.stroke_width > 0.0 {
        line.style.stroke_width
    } else {
        1.0
    };
    let wing = (width * 1.5).max(3.0);
    let pad = width * 0.5 * 4.0;
    (wing - pad).max(0.0) * scale_extent(&line.props.transform)
}

fn scale_extent(transform: &Transform) -> f32 {
    transform.scale.x.abs().max(transform.scale.y.abs())
}

fn matrix_value(transform: &Transform) -> Option<String> {
    let cos = transform.rotation.cos();
    let sin = transform.rotation.sin();
    let a = fmt_num(cos * transform.scale.x);
    let b = fmt_num(sin * transform.scale.x);
    let c = fmt_num(-sin * transform.scale.y);
    let d = fmt_num(cos * transform.scale.y);
    let e = fmt_num(transform.translation.x);
    let f = fmt_num(transform.translation.y);
    if a == "1" && b == "0" && c == "0" && d == "1" && e == "0" && f == "0" {
        None
    } else {
        Some(format!("{a} {b} {c} {d} {e} {f}"))
    }
}

fn points_attr(points: &[Vec2]) -> String {
    points
        .iter()
        .map(|point| format!("{},{}", fmt_num(point.x), fmt_num(point.y)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn xml_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn rgb_hex(color: Color) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(color.r),
        channel(color.g),
        channel(color.b)
    )
}

fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// 把 \[0, 1\] 的系数格式化。接近 1 时返回 `None`，调用方省略该属性。
fn unit_alpha(value: f32) -> Option<String> {
    if !value.is_finite() {
        return Some("0".to_string());
    }
    let text = fmt_num(value.clamp(0.0, 1.0));
    if text == "1" {
        None
    } else {
        Some(text)
    }
}

/// 固定 4 位小数再去掉尾零，保证同一场景多次导出的文本相同。
fn fmt_num(value: f32) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    let scaled = (value * 10_000.0).round() as i64;
    if scaled == 0 {
        return "0".to_string();
    }
    let sign = if scaled < 0 { "-" } else { "" };
    let scaled = scaled.unsigned_abs();
    let whole = scaled / 10_000;
    let frac = scaled % 10_000;
    if frac == 0 {
        return format!("{sign}{whole}");
    }
    let mut frac_text = format!("{frac:04}");
    while frac_text.ends_with('0') {
        frac_text.pop();
    }
    format!("{sign}{whole}.{frac_text}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{Element, ElementStyle};
    use crate::snap::GuideAxis;
    use infinitecanvas_math::Color;
    use std::f32::consts::FRAC_PI_2;

    fn rect(id: u64, at: Vec2, size: Vec2, style: ElementStyle) -> Rectangle {
        Rectangle::new(id, at, size).with_style(style)
    }

    #[test]
    fn rectangle_snapshot_includes_fill_stroke_opacity_and_translation() {
        let mut scene = Scene::new();
        let shape = rect(
            1,
            Vec2::new(10.0, 20.0),
            Vec2::new(40.0, 30.0),
            ElementStyle {
                fill: Some(Color::new(1.0, 0.0, 0.0, 0.5)),
                stroke: Some(Color::rgb(0.0, 0.0, 1.0)),
                stroke_width: 4.0,
                opacity: 0.25,
                ..ElementStyle::new()
            },
        );
        scene.add_rect(shape);

        let svg = export_svg(&scene, false);
        assert_eq!(
            svg,
            concat!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"48\" height=\"38\" viewBox=\"-14 1 48 38\">\n",
                "  <g id=\"e1\" transform=\"matrix(1 0 0 1 10 20)\" opacity=\"0.25\">\n",
                "    <rect x=\"-20\" y=\"-15\" width=\"40\" height=\"30\" fill=\"#ff0000\" fill-opacity=\"0.5\" stroke=\"#0000ff\" stroke-width=\"4\"/>\n",
                "  </g>\n",
                "</svg>\n",
            )
        );
    }

    #[test]
    fn rounded_rectangle_writes_rx_and_ry() {
        let mut scene = Scene::new();
        scene.add_rect(
            rect(
                1,
                Vec2::ZERO,
                Vec2::new(40.0, 20.0),
                ElementStyle::fill_only(Color::BLACK),
            )
            .with_corner_radius(6.0),
        );
        let svg = export_svg(&scene, false);
        assert!(svg.contains("rx=\"6\" ry=\"6\""));
        let mut sharp = Scene::new();
        sharp.add_rect(rect(
            1,
            Vec2::ZERO,
            Vec2::new(40.0, 20.0),
            ElementStyle::fill_only(Color::BLACK),
        ));
        assert!(!export_svg(&sharp, false).contains("rx="));
    }

    #[test]
    fn circle_snapshot() {
        let mut scene = Scene::new();
        scene.add_circle(
            Circle::new(1, Vec2::ZERO, 10.0).with_style(ElementStyle::fill_only(Color::GREEN)),
        );
        let svg = export_svg(&scene, false);
        assert_eq!(
            svg,
            concat!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"-12 -12 24 24\">\n",
                "  <g id=\"e1\">\n",
                "    <circle cx=\"0\" cy=\"0\" r=\"10\" fill=\"#00ff00\" stroke=\"none\"/>\n",
                "  </g>\n",
                "</svg>\n",
            )
        );
    }

    #[test]
    fn line_and_arrow_snapshot() {
        let mut scene = Scene::new();
        scene.add_line(Line::from_world(
            1,
            Vec2::ZERO,
            Vec2::new(40.0, 0.0),
            ElementStyle::stroke_only(Color::BLACK, 4.0),
            true,
        ));
        let svg = export_svg(&scene, false);
        assert!(svg.contains(
            "<line x1=\"0\" y1=\"0\" x2=\"24\" y2=\"0\" fill=\"none\" stroke=\"#000000\" stroke-width=\"4\""
        ));
        assert!(svg
            .contains("stroke-linecap=\"butt\" stroke-linejoin=\"miter\" stroke-miterlimit=\"4\""));
        assert!(
            svg.contains("<polygon points=\"40,0 24,6 24,-6\" fill=\"#000000\" stroke=\"none\"/>")
        );
    }

    #[test]
    fn path_snapshot() {
        let mut scene = Scene::new();
        scene.add_path(Path::from_world_points(
            1,
            &[
                Vec2::new(5.0, 5.0),
                Vec2::new(15.0, 8.0),
                Vec2::new(18.0, 20.0),
            ],
            ElementStyle::stroke_only(Color::BLUE, 2.0),
        ));
        let svg = export_svg(&scene, false);
        assert!(svg.contains("transform=\"matrix(1 0 0 1 5 5)\""));
        assert!(svg.contains(
            "<polyline points=\"0,0 10,3 13,15\" fill=\"none\" stroke=\"#0000ff\" stroke-width=\"2\""
        ));
    }

    #[test]
    fn rotated_rectangle_uses_canvas_matrix() {
        let mut scene = Scene::new();
        let mut shape = rect(
            3,
            Vec2::new(10.0, 20.0),
            Vec2::new(40.0, 10.0),
            ElementStyle::fill_only(Color::RED),
        );
        shape.set_rotation(FRAC_PI_2);
        shape.props.transform.scale = Vec2::new(2.0, 0.5);
        scene.add_rect(shape);

        let svg = export_svg(&scene, false);
        // cos(π/2)=0, sin(π/2)=1, scale (2, 0.5) → matrix(0 2 -0.5 0 tx ty)
        assert!(svg.contains("transform=\"matrix(0 2 -0.5 0 10 20)\""));
        assert!(svg.contains("fill=\"#ff0000\""));

        let transform = Transform {
            translation: Vec2::new(10.0, 20.0),
            rotation: FRAC_PI_2,
            scale: Vec2::new(2.0, 0.5),
        };
        for corner in [
            Vec2::new(-20.0, -5.0),
            Vec2::new(20.0, -5.0),
            Vec2::new(20.0, 5.0),
            Vec2::new(-20.0, 5.0),
        ] {
            let world = transform.transform_point(corner);
            assert!(
                point_in_view_box(&svg, world),
                "rotated corner {world:?} outside viewBox"
            );
        }
    }

    #[test]
    fn paint_order_follows_z_index_then_id() {
        let mut scene = Scene::new();
        let mut back = rect(
            2,
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
            ElementStyle::fill_only(Color::BLACK),
        );
        back.props.z_index = 0;
        let mut front = rect(
            1,
            Vec2::new(4.0, 4.0),
            Vec2::new(10.0, 10.0),
            ElementStyle::fill_only(Color::RED),
        );
        front.props.z_index = 5;
        scene.add_rect(front);
        scene.add_rect(back);

        let svg = export_svg(&scene, false);
        let back_at = svg.find("id=\"e2\"").expect("back element");
        let front_at = svg.find("id=\"e1\"").expect("front element");
        assert!(back_at < front_at);
    }

    #[test]
    fn selected_only_keeps_selection_and_reports_empty_selection() {
        let mut scene = Scene::new();
        let mut selected = rect(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(8.0, 8.0),
            ElementStyle::fill_only(Color::RED),
        );
        selected.props.selected = true;
        let other = rect(
            2,
            Vec2::new(30.0, 30.0),
            Vec2::new(8.0, 8.0),
            ElementStyle::fill_only(Color::BLUE),
        );
        scene.add_rect(selected);
        scene.add_rect(other);

        let svg = export_svg(&scene, true);
        assert!(svg.contains("id=\"e1\""));
        assert!(!svg.contains("id=\"e2\""));
        assert!(!svg.contains("#0000ff"));

        let empty = export_svg(&Scene::new(), true);
        assert!(empty.contains("<!-- nothing selected -->"));
        assert!(empty.contains("width=\"1\" height=\"1\" viewBox=\"0 0 1 1\""));
        assert!(!empty.contains("viewBox=\"0 0 0 0\""));
    }

    #[test]
    fn hidden_elements_and_empty_scene_are_omitted() {
        let mut scene = Scene::new();
        let mut hidden = rect(
            1,
            Vec2::ZERO,
            Vec2::new(10.0, 10.0),
            ElementStyle::fill_only(Color::RED),
        );
        hidden.props.visible = false;
        scene.add_rect(hidden);
        let svg = export_svg(&scene, false);
        assert!(svg.contains("<!-- empty scene -->"));
        assert!(svg.contains("width=\"1\" height=\"1\" viewBox=\"0 0 1 1\""));
        assert!(!svg.contains("<rect"));
        assert!(!svg.contains("viewBox=\"0 0 0 0\""));

        let empty = export_svg(&Scene::new(), false);
        assert!(empty.contains("<!-- empty scene -->"));
        assert!(empty.contains("width=\"1\" height=\"1\" viewBox=\"0 0 1 1\""));
    }

    #[test]
    fn image_is_skipped_with_a_note() {
        use crate::element::Image;
        let mut scene = Scene::new();
        scene.add_image(Image::new(
            7,
            Vec2::new(1.0, 2.0),
            Vec2::new(16.0, 16.0),
            "badge".to_string(),
        ));
        let svg = export_svg(&scene, false);
        assert!(
            svg.contains("<!-- image 7 skipped: texture pixels are not stored in the scene -->")
        );
        assert!(!svg.contains("<image"));
        assert!(svg.contains("viewBox=\"0 0 1 1\""));
    }

    #[test]
    fn image_asset_is_embedded_as_data_url_with_transform() {
        use crate::element::Image;

        let mut scene = Scene::new();
        let png = crate::asset::encode_png_rgba(2, 2, &[255, 0, 0, 255].repeat(4)).unwrap();
        let asset_id = scene.insert_image_bytes(&png, "image/png").unwrap();
        let data_url = scene.image_asset(&asset_id).unwrap().to_data_url();
        let mut image = Image::from_asset(7, Vec2::new(10.0, 20.0), &asset_id, 2, 2);
        image.size = Vec2::new(8.0, 4.0);
        image.set_rotation(FRAC_PI_2);
        image.set_alpha(0.5);
        scene.add_image(image);
        scene.add_image(Image::new(
            8,
            Vec2::new(500.0, 500.0),
            Vec2::new(32.0, 32.0),
            "badge".to_string(),
        ));

        let svg = export_svg(&scene, false);
        assert!(svg.contains(&format!("href=\"{data_url}\"")));
        assert!(svg.contains("data:image/png;base64,"));
        assert!(svg.contains("<image href=\""));
        assert!(svg.contains("width=\"8\" height=\"4\" preserveAspectRatio=\"none\""));
        assert!(svg.contains("transform=\"matrix(0 1 -1 0 10 20)\""));
        assert!(svg.contains("opacity=\"0.5\""));
        assert!(svg.contains("id=\"e7\""));
        assert!(
            svg.contains("<!-- image 8 skipped: texture pixels are not stored in the scene -->")
        );
        assert!(point_in_view_box(&svg, Vec2::new(10.0, 20.0)));
        assert!(point_in_view_box(&svg, Vec2::new(6.0, 28.0)));
        assert!(!point_in_view_box(&svg, Vec2::new(500.0, 500.0)));
        assert!(!svg.contains("viewBox=\"0 0 1 1\""));
    }

    #[test]
    fn image_uv_crop_uses_nested_view_box() {
        use crate::element::Image;

        let mut scene = Scene::new();
        let png = crate::asset::encode_png_rgba(2, 2, &[0, 0, 255, 255].repeat(4)).unwrap();
        let asset_id = scene.insert_image_bytes(&png, "image/png").unwrap();
        let mut image = Image::from_asset(3, Vec2::new(1.0, 2.0), &asset_id, 2, 2);
        image.size = Vec2::new(8.0, 4.0);
        image.set_uv_rect(Some(Rect::new(0.0, 0.0, 0.5, 1.0)));
        scene.add_image(image);

        let svg = export_svg(&scene, false);
        assert!(svg.contains(
            "<svg x=\"0\" y=\"0\" width=\"8\" height=\"4\" viewBox=\"0 0 1 2\" preserveAspectRatio=\"none\">"
        ));
        assert!(svg.contains("<image href=\"data:image/png;base64,"));
        assert!(svg.contains("width=\"2\" height=\"2\" preserveAspectRatio=\"none\""));
        assert!(svg.contains("transform=\"matrix(1 0 0 1 1 2)\""));
    }

    #[test]
    fn image_in_group_follows_visibility_and_selection() {
        use crate::element::Image;

        let mut scene = Scene::new();
        let png = crate::asset::encode_png_rgba(1, 1, &[0, 255, 0, 255]).unwrap();
        let asset_id = scene.insert_image_bytes(&png, "image/png").unwrap();
        scene.add_image(Image::from_asset(1, Vec2::ZERO, &asset_id, 1, 1));
        scene.add_image(Image::new(
            3,
            Vec2::new(400.0, 400.0),
            Vec2::new(8.0, 8.0),
            "badge".to_string(),
        ));
        scene.add_rect(rect(
            2,
            Vec2::new(100.0, 100.0),
            Vec2::new(10.0, 10.0),
            ElementStyle::fill_only(Color::BLUE),
        ));
        let group_id = scene.group(&[1, 3]).unwrap();
        scene
            .get_mut(group_id)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .selected = true;

        let selected = export_svg(&scene, true);
        assert!(selected.contains("<image"));
        assert!(selected.contains("data:image/png;base64,"));
        assert!(selected.contains("id=\"e1\""));
        assert!(selected
            .contains("<!-- image 3 skipped: texture pixels are not stored in the scene -->"));
        assert!(selected.contains(&format!("<g id=\"e{group_id}\">")));
        assert!(!selected.contains(&format!("<g id=\"e{group_id}\" transform=")));
        assert!(!selected.contains("id=\"e2\""));
        assert!(point_in_view_box(&selected, Vec2::ZERO));
        assert!(!point_in_view_box(&selected, Vec2::new(400.0, 400.0)));
        assert!(!point_in_view_box(&selected, Vec2::new(100.0, 100.0)));

        scene.set_visible(group_id, false);
        let hidden = export_svg(&scene, false);
        assert!(!hidden.contains("<image"));
        assert!(!hidden.contains("image 3 skipped"));
        assert!(!hidden.contains("id=\"e1\""));
        assert!(!hidden.contains(&format!("id=\"e{group_id}\"")));
        assert!(hidden.contains("id=\"e2\""));
        assert!(!point_in_view_box(&hidden, Vec2::ZERO));
        assert!(point_in_view_box(&hidden, Vec2::new(100.0, 100.0)));
    }

    #[test]
    fn text_snapshot_keeps_anchor_lines_and_rotation() {
        use crate::element::{Text, TextAlign};

        let mut scene = Scene::new();
        let text = Text::new(1, Vec2::new(10.0, 20.0), "Hello")
            .with_font_size(16.0)
            .with_color(Color::RED)
            .with_font_family("sans-serif");
        scene.add_text(text);
        let svg = export_svg(&scene, false);
        assert!(svg.contains(concat!(
            "<text fill=\"#ff0000\" font-size=\"16\" font-family=\"sans-serif\" ",
            "text-anchor=\"start\" dominant-baseline=\"text-before-edge\" xml:space=\"preserve\">",
        )));
        assert!(svg.contains("<tspan x=\"0\" y=\"0\">Hello</tspan>"));
        assert!(svg.contains("transform=\"matrix(1 0 0 1 10 20)\""));

        let mut rotated = Scene::new();
        let mut text = Text::new(2, Vec2::new(4.0, 8.0), "A<B>&\"C\"\nnext")
            .with_font_size(20.0)
            .with_align(TextAlign::Center)
            .with_color(Color::new(0.0, 0.0, 1.0, 0.5));
        text.set_rotation(FRAC_PI_2);
        rotated.add_text(text);
        let svg = export_svg(&rotated, false);
        assert!(svg.contains("text-anchor=\"middle\""));
        assert!(svg.contains("fill=\"#0000ff\""));
        assert!(svg.contains("fill-opacity=\"0.5\""));
        assert!(svg.contains("transform=\"matrix(0 1 -1 0 4 8)\""));
        assert!(svg.contains("<tspan x=\"0\" y=\"0\">A&lt;B&gt;&amp;&quot;C&quot;</tspan>"));
        assert!(svg.contains("<tspan x=\"0\" y=\"25\">next</tspan>"));
    }

    #[test]
    fn groups_are_not_drawn_and_hidden_groups_hide_children() {
        let mut scene = Scene::new();
        scene.add_rect(rect(
            1,
            Vec2::ZERO,
            Vec2::new(10.0, 10.0),
            ElementStyle::fill_only(Color::RED),
        ));
        scene.add_rect(rect(
            2,
            Vec2::new(20.0, 0.0),
            Vec2::new(10.0, 10.0),
            ElementStyle::fill_only(Color::BLUE),
        ));
        let group_id = scene.group(&[1, 2]).unwrap();
        let svg = export_svg(&scene, false);
        assert!(svg.contains("id=\"e1\""));
        assert!(svg.contains("id=\"e2\""));
        assert!(svg.contains("<rect"));
        let group_tag = format!("<g id=\"e{group_id}\">");
        assert!(
            svg.contains(&group_tag),
            "group should wrap children without a transform"
        );
        assert!(!svg.contains(&format!("<g id=\"e{group_id}\" transform=")));

        scene.set_visible(group_id, false);
        let hidden = export_svg(&scene, false);
        assert!(hidden.contains("<!-- empty scene -->"));
        assert!(!hidden.contains("<rect"));
        assert!(hidden.contains("width=\"1\" height=\"1\" viewBox=\"0 0 1 1\""));
    }

    #[test]
    fn selected_group_exports_rect_and_text_hidden_group_ignores_view_box() {
        use crate::element::Text;

        let mut scene = Scene::new();
        scene.add_rect(rect(
            1,
            Vec2::ZERO,
            Vec2::new(10.0, 10.0),
            ElementStyle::fill_only(Color::RED),
        ));
        scene.add_text(Text::new(2, Vec2::new(40.0, 0.0), "Hi").with_font_size(16.0));
        scene.add_rect(rect(
            4,
            Vec2::new(200.0, 200.0),
            Vec2::new(10.0, 10.0),
            ElementStyle::fill_only(Color::BLUE),
        ));
        let group_id = scene.group(&[1, 2]).unwrap();
        scene
            .get_mut(group_id)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .selected = true;
        assert!(!scene.get(1).unwrap().as_element().props().selected);
        assert!(!scene.get(2).unwrap().as_element().props().selected);

        let svg = export_svg(&scene, true);
        assert!(svg.contains("<rect"));
        assert!(svg.contains("<text"));
        assert!(svg.contains(">Hi</tspan>"));
        assert!(svg.contains("id=\"e1\""));
        assert!(svg.contains("id=\"e2\""));
        assert!(!svg.contains("id=\"e4\""));
        assert!(!svg.contains("#0000ff"));
        assert!(svg.contains(&format!("<g id=\"e{group_id}\">")));
        assert!(!svg.contains(&format!("<g id=\"e{group_id}\" transform=")));
        assert!(svg.contains("transform=\"matrix(1 0 0 1 40 0)\""));
        let group_at = svg.find(&format!("id=\"e{group_id}\"")).unwrap();
        let rect_at = svg.find("id=\"e1\"").unwrap();
        let text_at = svg.find("id=\"e2\"").unwrap();
        assert!(group_at < rect_at && rect_at < text_at);
        assert!(point_in_view_box(&svg, Vec2::ZERO));
        assert!(point_in_view_box(&svg, Vec2::new(40.0, 0.0)));
        assert!(!point_in_view_box(&svg, Vec2::new(200.0, 200.0)));

        scene.set_visible(group_id, false);
        let hidden = export_svg(&scene, false);
        assert!(!hidden.contains("id=\"e1\""));
        assert!(!hidden.contains("id=\"e2\""));
        assert!(!hidden.contains("<text"));
        assert!(!hidden.contains(&format!("id=\"e{group_id}\"")));
        assert!(hidden.contains("id=\"e4\""));
        assert!(point_in_view_box(&hidden, Vec2::new(200.0, 200.0)));
        assert!(!point_in_view_box(&hidden, Vec2::ZERO));
        assert!(!point_in_view_box(&hidden, Vec2::new(40.0, 0.0)));
    }

    fn point_in_view_box(svg: &str, point: Vec2) -> bool {
        let marker = "viewBox=\"";
        let start = svg.find(marker).expect("viewBox") + marker.len();
        let end = svg[start..].find('"').expect("viewBox end") + start;
        let mut parts = svg[start..end]
            .split_whitespace()
            .map(|part| part.parse::<f32>().unwrap());
        let x = parts.next().unwrap();
        let y = parts.next().unwrap();
        let width = parts.next().unwrap();
        let height = parts.next().unwrap();
        point.x >= x - 0.01
            && point.y >= y - 0.01
            && point.x <= x + width + 0.01
            && point.y <= y + height + 0.01
    }

    #[test]
    fn svg_omits_guides_unless_the_flag_is_set() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::new(10.0, 10.0), Vec2::new(8.0, 8.0))
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        scene.add_guide(GuideAxis::Horizontal, 4.0).unwrap();
        let plain = export_svg(&scene, false);
        assert!(!plain.contains("#3db7ff"));
        assert_eq!(export_svg_with_guides(&scene, false, false), plain);
        let marked = export_svg_with_guides(&scene, false, true);
        assert!(marked.contains("#3db7ff"));
        assert!(marked.contains("y1=\"4\""));
        assert!(plain.contains("<rect"));
    }
}
