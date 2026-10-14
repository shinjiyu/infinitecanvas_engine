//! 正多边形与星形。
//!
//! 两种外形共用一个元素：[`PolygonKind`] 区分正多边形和星形，
//! `sides` 是边数或角数，`inner_radius` 是星形内点相对外接圆的比例。
//! 顶点在局部空间绕原点生成，再经 [`ElementProps::transform`] 变到世界空间。
//!
//! ```text
//! sides / inner_radius
//!        │
//!        ▼
//! 局部顶点 ──► 填充扇形 + 描边
//!        │
//!        ├─ 命中：局部多边形
//!        └─ 包围盒：顶点的轴对齐盒
//! ```

use super::stroke::{
    color_with_opacity, dash_is_solid, distance_to_polyline, stroke_centerline, stroke_styled,
    transform_vertices, StrokePaint,
};
use super::{Element, ElementId, ElementProps, ElementStyle};
use infinitecanvas_math::{Color, Rect, Vec2, Vertex};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// 边数或角数下限。三角形是最少的闭合多边形。
pub const MIN_SIDES: u32 = 3;
/// 边数或角数上限，避免一次拖动生成过多三角形。
pub const MAX_SIDES: u32 = 64;
/// 正多边形缺省边数（六边形）。
pub const DEFAULT_POLYGON_SIDES: u32 = 6;
/// 星形缺省角数。
pub const DEFAULT_STAR_POINTS: u32 = 5;
/// 星形内径相对外接圆半径的缺省比例。
pub const DEFAULT_INNER_RADIUS: f32 = 0.5;
/// 内径比下限。再小就会缩成一条线。
pub const MIN_INNER_RADIUS: f32 = 0.05;
/// 内径比上限。再大就和正多边形重合。
pub const MAX_INNER_RADIUS: f32 = 0.95;

/// 正多边形还是星形。
///
/// JSON 用小写名字。缺省为 [`PolygonKind::Polygon`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PolygonKind {
    /// 正多边形。顶点都在外接圆上。
    #[default]
    Polygon,
    /// 星形。外点和内点交替，内点半径是外接圆乘 [`Polygon::inner_radius`]。
    Star,
}

impl PolygonKind {
    /// `"polygon"` 或 `"star"`。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Polygon => "polygon",
            Self::Star => "star",
        }
    }
}

fn default_sides() -> u32 {
    DEFAULT_POLYGON_SIDES
}

fn default_inner_radius() -> f32 {
    DEFAULT_INNER_RADIUS
}

/// 正多边形或星形。
///
/// # 属性
/// - 中心在 [`ElementProps`] 的平移上，局部原点就是中心
/// - `radius`：外接圆半径（局部单位）
/// - `sides`：正多边形的边数，或星形的角数，钳在 3–64
/// - `inner_radius`：星形内点半径 / 外接圆半径，钳在 0.05–0.95。正多边形不使用它
/// - 填充和描边走 [`ElementStyle`]
///
/// 第一个顶点在局部负 Y（画面上方），所以六边形是尖顶，五角星也有一角朝上。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{Polygon, PolygonKind};
/// use infinitecanvas_math::Vec2;
///
/// let hex = Polygon::hexagon(1, Vec2::ZERO, 20.0);
/// assert_eq!(hex.kind(), PolygonKind::Polygon);
/// assert_eq!(hex.sides(), 6);
///
/// let star = Polygon::star(2, Vec2::new(8.0, 8.0), 20.0);
/// assert_eq!(star.kind(), PolygonKind::Star);
/// assert_eq!(star.sides(), 5);
/// assert_eq!(star.inner_radius(), 0.5);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Polygon {
    /// 元素属性。
    pub props: ElementProps,
    /// 外接圆半径。非有限或负数在绘制时取绝对值，非有限值当成 0。
    pub radius: f32,
    /// 边数或角数。JSON 缺省为 6。也可以用 `points` 作为别名。
    #[serde(default = "default_sides", alias = "points")]
    pub sides: u32,
    /// 星形内径比。JSON 缺省为 0.5。正多边形保留这个值但不参与外形。
    #[serde(default = "default_inner_radius")]
    pub inner_radius: f32,
    /// 正多边形或星形。JSON 缺省为正多边形。
    #[serde(default)]
    pub kind: PolygonKind,
    /// 样式。JSON 中可省略，缺省为 [`ElementStyle::new`]。
    #[serde(default)]
    pub style: ElementStyle,
}

impl Polygon {
    /// 创建指定种类和边数的形状。半径、边数和内径都会先钳制。
    ///
    /// # 参数
    /// - `id`: 元素 ID
    /// - `center`: 中心（世界坐标，写入平移）
    /// - `radius`: 外接圆半径
    /// - `kind`: 正多边形或星形
    /// - `sides`: 边数或角数
    ///
    /// # 返回
    /// 已钳制边数、内径为 0.5 的元素。
    pub fn new(id: ElementId, center: Vec2, radius: f32, kind: PolygonKind, sides: u32) -> Self {
        let mut props = ElementProps::new(id);
        props.transform.translation = center;
        let mut polygon = Self {
            props,
            radius: 0.0,
            sides,
            inner_radius: DEFAULT_INNER_RADIUS,
            kind,
            style: ElementStyle::default(),
        };
        polygon.set_radius(radius);
        polygon.set_sides(sides);
        polygon
    }

    /// 正六边形。
    pub fn hexagon(id: ElementId, center: Vec2, radius: f32) -> Self {
        Self::new(
            id,
            center,
            radius,
            PolygonKind::Polygon,
            DEFAULT_POLYGON_SIDES,
        )
    }

    /// 五角星，内径比 0.5。
    pub fn star(id: ElementId, center: Vec2, radius: f32) -> Self {
        Self::new(id, center, radius, PolygonKind::Star, DEFAULT_STAR_POINTS)
    }

    /// 指定边数的正多边形。边数会被钳到 3–64。
    pub fn regular(id: ElementId, center: Vec2, radius: f32, sides: u32) -> Self {
        Self::new(id, center, radius, PolygonKind::Polygon, sides)
    }

    /// 设置样式。
    pub fn with_style(mut self, style: ElementStyle) -> Self {
        self.style = style;
        self
    }

    /// 设置内径比后返回自身。只影响星形的外形。
    pub fn with_inner_radius(mut self, ratio: f32) -> Self {
        self.set_inner_radius(ratio);
        self
    }

    /// 正多边形或星形。
    pub fn kind(&self) -> PolygonKind {
        self.kind
    }

    /// 改种类。不改边数和内径。调用方还要 [`crate::Scene::mark_dirty`]。
    pub fn set_kind(&mut self, kind: PolygonKind) {
        self.kind = kind;
    }

    /// 当前边数或角数（已钳制）。
    pub fn sides(&self) -> u32 {
        Self::clamp_sides(self.sides)
    }

    /// 写入边数或角数，并钳到 3–64。
    ///
    /// 调用方还要 [`crate::Scene::mark_dirty`]。样式命令会自己标脏。
    pub fn set_sides(&mut self, sides: u32) {
        self.sides = Self::clamp_sides(sides);
    }

    /// 把边数钳到 [`MIN_SIDES`]–[`MAX_SIDES`]。
    pub fn clamp_sides(sides: u32) -> u32 {
        sides.clamp(MIN_SIDES, MAX_SIDES)
    }

    /// 把有限的浮点边数四舍五入再钳制。非有限值返回 `None`。
    pub fn sides_from_f32(value: f32) -> Option<u32> {
        if !value.is_finite() {
            return None;
        }
        let rounded = value.round();
        if rounded < 0.0 || rounded > u32::MAX as f32 {
            return None;
        }
        Some(Self::clamp_sides(rounded as u32))
    }

    /// 当前内径比（已钳制）。
    pub fn inner_radius(&self) -> f32 {
        Self::clamp_inner_radius(self.inner_radius)
    }

    /// 写入星形内径比，并钳到 0.05–0.95。非有限值写成缺省 0.5。
    pub fn set_inner_radius(&mut self, ratio: f32) {
        self.inner_radius = Self::clamp_inner_radius(ratio);
    }

    /// 把内径比钳到 [`MIN_INNER_RADIUS`]–[`MAX_INNER_RADIUS`]。非有限值变成缺省 0.5。
    pub fn clamp_inner_radius(ratio: f32) -> f32 {
        if !ratio.is_finite() {
            return DEFAULT_INNER_RADIUS;
        }
        ratio.clamp(MIN_INNER_RADIUS, MAX_INNER_RADIUS)
    }

    /// 外接圆半径。非有限值当成 0，负数取绝对值。
    pub fn radius(&self) -> f32 {
        if !self.radius.is_finite() {
            0.0
        } else {
            self.radius.abs()
        }
    }

    /// 写入外接圆半径。
    pub fn set_radius(&mut self, radius: f32) {
        self.radius = if radius.is_finite() {
            radius.abs()
        } else {
            0.0
        };
    }

    /// 按 JSON 读入的原始值再钳一次。导入路径会调用。
    pub fn normalize(&mut self) {
        self.set_radius(self.radius);
        self.set_sides(self.sides);
        self.set_inner_radius(self.inner_radius);
    }

    /// 局部空间的顶点，不闭合（首尾不重复）。
    ///
    /// 正多边形有 `sides` 个点。星形有 `sides * 2` 个点，外点和内点交替。
    pub fn local_points(&self) -> Vec<Vec2> {
        let count = self.sides() as usize;
        let radius = self.radius();
        if self.kind == PolygonKind::Star {
            let inner = radius * self.inner_radius();
            let mut points = Vec::with_capacity(count * 2);
            for index in 0..count {
                let outer = vertex_angle(index, count);
                points.push(Vec2::new(outer.cos() * radius, outer.sin() * radius));
                let inner_angle = outer + PI / count as f32;
                points.push(Vec2::new(
                    inner_angle.cos() * inner,
                    inner_angle.sin() * inner,
                ));
            }
            points
        } else {
            (0..count)
                .map(|index| {
                    let angle = vertex_angle(index, count);
                    Vec2::new(angle.cos() * radius, angle.sin() * radius)
                })
                .collect()
        }
    }

    /// 闭合轮廓。首尾是同一个点，供描边使用。
    pub fn closed_outline(&self) -> Vec<Vec2> {
        let mut points = self.local_points();
        if points.len() >= 3 {
            if let Some(first) = points.first().copied() {
                points.push(first);
            }
        }
        points
    }

    /// 生成填充顶点（中心扇形）。没有填充色时返回空。
    pub fn generate_fill_vertices(&self) -> Vec<Vertex> {
        let Some(fill) = self.style.fill else {
            return Vec::new();
        };
        let points = self.local_points();
        if points.len() < 3 {
            return Vec::new();
        }
        let color = color_with_opacity(fill, self.style.opacity);
        let transform = &self.props.transform;
        let center = transform.transform_point(Vec2::ZERO);
        let mut vertices = Vec::with_capacity(points.len() * 3);
        for index in 0..points.len() {
            let a = transform.transform_point(points[index]);
            let b = transform.transform_point(points[(index + 1) % points.len()]);
            vertices.push(Vertex::new(center.x, center.y, color));
            vertices.push(Vertex::new(a.x, a.y, color));
            vertices.push(Vertex::new(b.x, b.y, color));
        }
        vertices
    }

    /// 生成 1 像素描边线段。没有描边色或线宽小于等于 0 时返回空。
    pub fn generate_stroke_vertices(&self) -> Vec<Vertex> {
        if self.style.stroke.is_none() || self.style.stroke_width <= 0.0 {
            return Vec::new();
        }
        let color = color_with_opacity(self.style.stroke.unwrap(), self.style.opacity);
        let outline = self.closed_outline();
        if outline.len() < 2 {
            return Vec::new();
        }
        if !dash_is_solid(&self.style.stroke_dash) {
            return transform_vertices(
                stroke_centerline(&outline, color, &self.style.stroke_dash),
                &self.props.transform,
            );
        }
        let transform = &self.props.transform;
        let mut vertices = Vec::with_capacity(outline.len().saturating_sub(1) * 2);
        for pair in outline.windows(2) {
            let a = transform.transform_point(pair[0]);
            let b = transform.transform_point(pair[1]);
            vertices.push(Vertex::new(a.x, a.y, color));
            vertices.push(Vertex::new(b.x, b.y, color));
        }
        vertices
    }

    /// 按线宽生成闭合描边三角形。线段管线不能表达线宽，粗描边放进填充缓冲。
    pub(crate) fn stroke_ribbon_vertices(&self) -> Vec<Vertex> {
        let Some(stroke) = self.style.stroke else {
            return Vec::new();
        };
        if self.style.stroke_width <= 0.0 {
            return Vec::new();
        }
        let color = color_with_opacity(stroke, self.style.opacity);
        let outline = self.closed_outline();
        transform_vertices(
            stroke_styled(
                &outline,
                StrokePaint {
                    width: self.style.stroke_width,
                    color,
                    dash: &self.style.stroke_dash,
                    cap: self.style.stroke_cap,
                    join: self.style.stroke_join,
                },
            ),
            &self.props.transform,
        )
    }

    /// 填充和细描边顶点。
    pub fn generate_vertices(&self) -> Vec<Vertex> {
        let mut vertices = self.generate_fill_vertices();
        vertices.extend(self.generate_stroke_vertices());
        vertices
    }

    /// 投影顶点，画在填充之前。关闭时为空。命中测试不读这些顶点。
    pub(crate) fn shadow_vertices(&self) -> Vec<Vertex> {
        super::shadow::soft_shadow_vertices(
            &self.local_points(),
            self.style.shadow,
            self.style.opacity,
            &self.props.transform,
        )
    }

    /// 到轮廓的有符号距离。内部为负，供 PNG 在本体外侧羽化投影。
    pub(crate) fn silhouette_distance(&self, point: Vec2) -> f32 {
        let outline = self.closed_outline();
        let distance = distance_to_polyline(point, &outline);
        if self.contains_local(point) {
            -distance
        } else {
            distance
        }
    }

    /// 局部点是否在填充内部。边界附近用奇偶射线，测试请用明显在内或在外的点。
    pub fn contains_local(&self, point: Vec2) -> bool {
        point_in_polygon(point, &self.local_points())
    }

    /// PNG 采样。`local` 已经逆变换到局部空间。描边盖在填充上面。
    pub(crate) fn sample_local(&self, local: Vec2) -> Option<Color> {
        let outline = self.closed_outline();
        let stroke = self.style.stroke_width.max(0.0) * 0.5;
        let in_stroke = stroke > 0.0
            && self.style.stroke.is_some()
            && distance_to_polyline(local, &outline) <= stroke + 1.0e-4;
        if in_stroke {
            if let Some(color) = self.style.stroke {
                return with_opacity(color, self.style.opacity);
            }
        }
        if self.contains_local(local) {
            if let Some(color) = self.style.fill {
                return with_opacity(color, self.style.opacity);
            }
        }
        None
    }
}

impl Element for Polygon {
    fn props(&self) -> &ElementProps {
        &self.props
    }

    fn props_mut(&mut self) -> &mut ElementProps {
        &mut self.props
    }

    fn local_bounds(&self) -> Rect {
        bounds_of(&self.local_points())
    }

    fn type_name(&self) -> &'static str {
        match self.kind {
            PolygonKind::Polygon => "Polygon",
            PolygonKind::Star => "Star",
        }
    }

    fn contains_point(&self, point: Vec2) -> bool {
        let local = self.props.transform.inverse_transform_point(point);
        self.contains_local(local)
    }
}

/// 第 `index` 个外顶点的角度。0 号顶点朝向局部负 Y。
fn vertex_angle(index: usize, count: usize) -> f32 {
    let count = count.max(1) as f32;
    -PI * 0.5 + index as f32 * (2.0 * PI / count)
}

fn bounds_of(points: &[Vec2]) -> Rect {
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    for point in points {
        min.x = min.x.min(point.x);
        min.y = min.y.min(point.y);
        max.x = max.x.max(point.x);
        max.y = max.y.max(point.y);
    }
    if !min.x.is_finite() || !max.x.is_finite() {
        return Rect::from_center_size(Vec2::ZERO, Vec2::ZERO);
    }
    Rect::from_corners(min, max)
}

/// 奇偶规则的射线法。水平边不会除以零。
fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let count = polygon.len();
    if count < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = count - 1;
    for current in 0..count {
        let a = polygon[current];
        let b = polygon[previous];
        let crosses = (a.y > point.y) != (b.y > point.y);
        if crosses {
            let x = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
            if point.x < x {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

fn with_opacity(color: Color, opacity: f32) -> Option<Color> {
    let painted = color_with_opacity(color, opacity);
    if painted.a <= 0.0 {
        None
    } else {
        Some(painted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{AddElementCommand, CommandHistory, SetStyleCommand};
    use crate::style::{selection_style, StyleField, StylePatch};
    use crate::vertex_cache::VertexCache;
    use crate::{AnyElement, Scene};
    use infinitecanvas_math::Color;
    use std::f32::consts::{FRAC_PI_2, PI};

    fn approx(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1.0e-3, "{actual} != {expected}");
    }

    #[test]
    fn defaults_are_hexagon_and_five_point_star_and_sides_clamp() {
        let hex = Polygon::hexagon(1, Vec2::new(4.0, 5.0), 12.0);
        assert_eq!(hex.kind(), PolygonKind::Polygon);
        assert_eq!(hex.sides(), 6);
        assert_eq!(hex.inner_radius(), DEFAULT_INNER_RADIUS);
        assert_eq!(hex.radius(), 12.0);
        assert_eq!(hex.type_name(), "Polygon");
        assert!(hex.is_dirty());

        let star = Polygon::star(2, Vec2::ZERO, 8.0);
        assert_eq!(star.kind(), PolygonKind::Star);
        assert_eq!(star.sides(), 5);
        assert_eq!(star.type_name(), "Star");
        assert_eq!(star.local_points().len(), 10);

        let mut wide = Polygon::regular(3, Vec2::ZERO, 10.0, 100);
        assert_eq!(wide.sides(), MAX_SIDES);
        wide.set_sides(2);
        assert_eq!(wide.sides(), MIN_SIDES);
        wide.set_sides(0);
        assert_eq!(wide.sides(), MIN_SIDES);
        assert_eq!(Polygon::sides_from_f32(6.6), Some(7));
        assert_eq!(Polygon::sides_from_f32(f32::NAN), None);
        wide.set_inner_radius(2.0);
        assert_eq!(wide.inner_radius(), MAX_INNER_RADIUS);
        wide.set_inner_radius(0.0);
        assert_eq!(wide.inner_radius(), MIN_INNER_RADIUS);
        wide.set_inner_radius(f32::NAN);
        assert_eq!(wide.inner_radius(), DEFAULT_INNER_RADIUS);
        wide.set_radius(-4.0);
        assert_eq!(wide.radius(), 4.0);
        wide.set_radius(f32::NAN);
        assert_eq!(wide.radius(), 0.0);
    }

    #[test]
    fn drop_shadow_sits_beside_the_hexagon_without_growing_the_hit_box() {
        let mut hex =
            Polygon::hexagon(1, Vec2::ZERO, 10.0).with_style(ElementStyle::fill_only(Color::WHITE));
        assert!(hex.shadow_vertices().is_empty());
        hex.style.shadow =
            crate::DropShadow::new(Vec2::new(30.0, 0.0), 0.0, Color::new(0.0, 0.0, 0.0, 0.5));
        assert!(!hex.shadow_vertices().is_empty());
        assert!(hex.contains_point(Vec2::ZERO));
        assert!(!hex.contains_point(Vec2::new(30.0, 0.0)));
        assert!(hex.silhouette_distance(Vec2::ZERO) < 0.0);
        assert!(hex.silhouette_distance(Vec2::new(30.0, 0.0)) > 0.0);
    }

    #[test]
    fn hexagon_bounds_hit_fill_and_stroke() {
        let hex = Polygon::hexagon(1, Vec2::new(100.0, 50.0), 10.0)
            .with_style(ElementStyle::fill_only(Color::RED));
        let bounds = hex.local_bounds();
        approx(bounds.height, 20.0);
        approx(bounds.width, 10.0 * 3.0_f32.sqrt());
        approx(bounds.center().x, 0.0);
        approx(bounds.center().y, 0.0);
        let world = hex.world_bounds();
        approx(world.center().x, 100.0);
        approx(world.center().y, 50.0);
        approx(world.height, 20.0);

        assert!(hex.contains_point(Vec2::new(100.0, 50.0)));
        assert!(hex.contains_point(Vec2::new(105.0, 50.0)));
        assert!(!hex.contains_point(Vec2::new(109.5, 50.0)));

        let fill = hex.generate_fill_vertices();
        assert_eq!(fill.len(), 18);
        assert!(fill
            .iter()
            .all(|vertex| vertex.color == [1.0, 0.0, 0.0, 1.0]));
        assert!(fill.chunks(3).all(|tri| {
            (tri[0].position[0] - 100.0).abs() < 1.0e-3
                && (tri[0].position[1] - 50.0).abs() < 1.0e-3
        }));

        let stroked = Polygon::hexagon(2, Vec2::ZERO, 10.0)
            .with_style(ElementStyle::stroke_only(Color::BLUE, 2.0));
        let lines = stroked.generate_stroke_vertices();
        assert_eq!(lines.len(), 12);
        assert!(lines
            .iter()
            .all(|vertex| vertex.color == [0.0, 0.0, 1.0, 1.0]));
        assert!(!stroked.stroke_ribbon_vertices().is_empty());
        assert!(stroked.generate_fill_vertices().is_empty());
    }

    #[test]
    fn star_is_concave_and_keeps_the_center() {
        let star = Polygon::star(1, Vec2::ZERO, 10.0)
            .with_style(ElementStyle::fill_only(Color::RED))
            .with_inner_radius(0.5);
        assert_eq!(star.generate_fill_vertices().len(), 30);
        assert!(star.contains_point(Vec2::ZERO));
        assert!(star.contains_point(Vec2::new(0.0, -9.0)));

        let valley = -FRAC_PI_2 + PI / 5.0;
        let outside = Vec2::new(valley.cos() * 8.0, valley.sin() * 8.0);
        let inside = Vec2::new(valley.cos() * 2.0, valley.sin() * 2.0);
        assert!(!star.contains_point(outside));
        assert!(star.contains_point(inside));
        assert!(star.sample_local(Vec2::ZERO).is_some());
        assert!(star.sample_local(outside).is_none());
    }

    #[test]
    fn rotation_and_scale_use_the_existing_transform() {
        let mut star = Polygon::star(1, Vec2::new(100.0, 50.0), 10.0);
        star.set_rotation(0.4);
        star.props.transform.scale = Vec2::new(2.0, 0.5);
        let local_inside = Vec2::new(0.0, -2.0);
        let world = star.props.transform.transform_point(local_inside);
        assert!(star.contains_point(world));
        let local_outside = Vec2::new(9.0, 9.0);
        let world_out = star.props.transform.transform_point(local_outside);
        assert!(!star.contains_point(world_out));
        // 星形的轴对齐盒不以原点为中心，旋转后世界包围盒中心会离开平移点。
        // 平移点本身仍落在包围盒里。
        assert!(star.world_bounds().contains(Vec2::new(100.0, 50.0)));
        assert!(star.world_bounds().width > 10.0);
    }

    #[test]
    fn opacity_tints_vertices_and_dashed_stroke_is_shorter() {
        let mut style = ElementStyle::fill_only(Color::BLACK);
        style.opacity = 0.5;
        style.stroke = Some(Color::BLUE);
        style.stroke_width = 2.0;
        let solid = Polygon::hexagon(1, Vec2::ZERO, 10.0).with_style(style.clone());
        assert!(solid
            .generate_fill_vertices()
            .iter()
            .all(|vertex| vertex.color[3] == 0.5));
        let solid_lines = solid.generate_stroke_vertices();
        style.stroke_dash = vec![4.0, 4.0];
        let dashed = Polygon::hexagon(2, Vec2::ZERO, 10.0).with_style(style);
        let dashed_lines = dashed.generate_stroke_vertices();
        assert!(!dashed_lines.is_empty());
        assert_eq!(dashed_lines.len() % 2, 0);
        assert_ne!(dashed_lines.len(), solid_lines.len());
        assert_ne!(
            dashed.stroke_ribbon_vertices().len(),
            solid.stroke_ribbon_vertices().len()
        );
    }

    #[test]
    fn json_defaults_and_round_trip() {
        let mut star = Polygon::star(7, Vec2::new(4.0, 5.0), 30.0).with_inner_radius(0.25);
        star.set_sides(7);
        star.set_rotation(0.2);
        let json = serde_json::to_string(&star).unwrap();
        assert!(json.contains("\"kind\":\"star\""));
        assert!(json.contains("\"sides\":7"));
        assert!(json.contains("\"inner_radius\":0.25"));
        let restored: Polygon = serde_json::from_str(&json).unwrap();
        restored_eq(&restored, &star);

        let legacy = serde_json::json!({
            "props": { "id": 3 },
            "radius": 10.0
        });
        let old: Polygon = serde_json::from_value(legacy).unwrap();
        assert_eq!(old.kind(), PolygonKind::Polygon);
        assert_eq!(old.sides(), 6);
        assert_eq!(old.inner_radius(), 0.5);
        assert_eq!(old.style, ElementStyle::new());

        let alias = serde_json::json!({
            "props": { "id": 4 },
            "radius": 10.0,
            "points": 4,
            "kind": "star"
        });
        let from_points: Polygon = serde_json::from_value(alias).unwrap();
        assert_eq!(from_points.kind(), PolygonKind::Star);
        assert_eq!(from_points.sides(), 4);
    }

    fn restored_eq(restored: &Polygon, star: &Polygon) {
        assert_eq!(restored.kind(), star.kind());
        assert_eq!(restored.sides(), star.sides());
        assert_eq!(restored.inner_radius(), star.inner_radius());
        assert_eq!(restored.radius(), star.radius());
        assert_eq!(restored.props.transform.translation, Vec2::new(4.0, 5.0));
        approx(restored.rotation(), 0.2);
    }

    #[test]
    fn create_and_side_edit_are_separate_undo_steps_and_mark_dirty() {
        let mut scene = Scene::new();
        let mut history = CommandHistory::new();
        let polygon =
            Polygon::hexagon(1, Vec2::ZERO, 10.0).with_style(ElementStyle::fill_only(Color::RED));
        history.execute(
            Box::new(AddElementCommand::new(AnyElement::Polygon(polygon))),
            &mut scene,
        );
        assert!(scene.contains(1));
        assert!(scene.has_dirty_elements());

        let mut cache = VertexCache::new();
        let first = cache.sync(&mut scene);
        assert_eq!(first.regenerated, 1);
        assert_eq!(cache.fill_slice(1).unwrap().len(), 18);

        let style = selection_style(&scene, &[1]);
        assert_eq!(style.sides, StyleField::Uniform(6.0));
        assert_eq!(style.inner_radius, StyleField::Absent);
        assert!(style.has_polygon);
        assert!(!style.has_star);
        assert_eq!(style.corner_radius, StyleField::Absent);

        let patch = StylePatch {
            sides: Some(4),
            ..StylePatch::default()
        };
        let command = SetStyleCommand::from_patch(&scene, &[1], &patch).unwrap();
        history.execute(Box::new(command), &mut scene);
        match scene.get(1).unwrap() {
            AnyElement::Polygon(shape) => assert_eq!(shape.sides(), 4),
            other => panic!("expected polygon, got {other:?}"),
        }
        assert!(scene.has_dirty_elements());
        let second = cache.sync(&mut scene);
        assert!(!second.geometry_unchanged);
        assert_eq!(cache.fill_slice(1).unwrap().len(), 12);

        assert!(history.undo(&mut scene));
        match scene.get(1).unwrap() {
            AnyElement::Polygon(shape) => assert_eq!(shape.sides(), 6),
            other => panic!("expected polygon, got {other:?}"),
        }
        assert!(history.undo(&mut scene));
        assert!(!scene.contains(1));
        assert!(history.redo(&mut scene));
        assert!(scene.contains(1));

        let ignored = StylePatch {
            sides: Some(9),
            inner_radius: Some(0.2),
            ..StylePatch::default()
        };
        assert!(SetStyleCommand::from_patch(&scene, &[1], &ignored).is_some());
        let mut circle_scene = Scene::new();
        circle_scene.add_circle(crate::Circle::new(2, Vec2::ZERO, 4.0));
        assert!(SetStyleCommand::from_patch(&circle_scene, &[2], &ignored).is_none());
    }

    #[test]
    fn star_inner_radius_is_one_undo_and_polygons_ignore_it() {
        let mut scene = Scene::new();
        scene.add_polygon(
            Polygon::star(1, Vec2::ZERO, 10.0).with_style(ElementStyle::fill_only(Color::RED)),
        );
        scene.add_polygon(Polygon::hexagon(2, Vec2::new(30.0, 0.0), 10.0));
        assert!(scene.has_dirty_elements());
        let summary = selection_style(&scene, &[1]);
        assert!(summary.has_star);
        assert_eq!(summary.sides, StyleField::Uniform(5.0));
        assert_eq!(summary.inner_radius, StyleField::Uniform(0.5));

        let mut history = CommandHistory::new();
        let patch = StylePatch {
            inner_radius: Some(0.25),
            fill: Some(Some(Color::BLUE)),
            ..StylePatch::default()
        };
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[1, 2], &patch).unwrap()),
            &mut scene,
        );
        match scene.get(1).unwrap() {
            AnyElement::Polygon(shape) => {
                assert_eq!(shape.inner_radius(), 0.25);
                assert_eq!(shape.style.fill, Some(Color::BLUE));
            }
            other => panic!("expected star, got {other:?}"),
        }
        match scene.get(2).unwrap() {
            AnyElement::Polygon(shape) => {
                assert_eq!(shape.inner_radius(), 0.5);
                assert_eq!(shape.style.fill, Some(Color::BLUE));
            }
            other => panic!("expected polygon, got {other:?}"),
        }
        assert!(history.undo(&mut scene));
        match scene.get(1).unwrap() {
            AnyElement::Polygon(shape) => {
                assert_eq!(shape.inner_radius(), 0.5);
                assert_eq!(shape.style.fill, Some(Color::RED));
            }
            other => panic!("expected star, got {other:?}"),
        }
    }
}
