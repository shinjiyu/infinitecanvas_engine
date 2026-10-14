//! 投影阴影。
//!
//! 每个叶子一份 [`DropShadow`]：局部偏移、模糊半径、颜色（alpha 是浓度）。
//! 颜色 alpha 为 0，或者偏移和模糊都是 0 时不绘制。JSON 缺省等于关闭，
//! 旧文档没有这些字段也能读入。和缺省值完全一样时不写回文件。
//!
//! 绘制是偏移后的轮廓，再加上沿外法线展开的羽化环，不是离屏高斯模糊。
//! 矩形和圆用形状外轮廓（圆角矩形跟半径）。直线和路径用笔画胶囊，箭头另加三角形。
//! 文本和图片用局部包围盒。命中测试和包围盒都不含阴影。
//!
//! 羽化环写在该元素自己的填充三角形之前，所以阴影在本体后面，并跟着 z 顺序。
//! 文本和图片的纹理仍走纹理通道；它们的阴影落在填充通道里、位于该元素的绘制位置。

use infinitecanvas_math::{Color, Transform, Vec2, Vertex};
use serde::{Deserialize, Serialize};

/// 羽化环的圈数。圈数固定，模糊变大只把环推远，不会按像素加顶点。
const FEATHER_RINGS: usize = 4;

/// 胶囊端点的圆弧段数。
const CAP_SEGMENTS: usize = 4;

fn default_shadow_color() -> Color {
    Color::new(0.0, 0.0, 0.0, 0.0)
}

/// 一个元素的投影。
///
/// 偏移在元素局部空间，旋转和缩放会把它一起带走。
/// `blur` 为 0 时是硬边的偏移剪影。颜色 alpha 为 0 时关闭。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::DropShadow;
/// use infinitecanvas_math::{Color, Vec2};
///
/// let off = DropShadow::default();
/// assert!(off.is_off());
/// let on = DropShadow::new(Vec2::new(8.0, 8.0), 12.0, Color::new(0.0, 0.0, 0.0, 0.4));
/// assert!(!on.is_off());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DropShadow {
    /// 局部空间偏移。缺省 `(0, 0)`。
    #[serde(default)]
    pub offset: Vec2,
    /// 羽化半径，和元素尺寸同一单位。缺省 0。负数在 [`Self::sanitized`] 里收成 0。
    #[serde(default)]
    pub blur: f32,
    /// 阴影颜色。alpha 是浓度，缺省 0（关闭）。
    #[serde(default = "default_shadow_color")]
    pub color: Color,
}

impl Default for DropShadow {
    fn default() -> Self {
        Self {
            offset: Vec2::ZERO,
            blur: 0.0,
            color: default_shadow_color(),
        }
    }
}

impl DropShadow {
    /// 收成有限值后的阴影。
    ///
    /// # 参数
    /// - `offset`: 局部偏移
    /// - `blur`: 羽化半径，负数变成 0
    /// - `color`: 颜色，通道钳到 0 到 1
    pub fn new(offset: Vec2, blur: f32, color: Color) -> Self {
        Self {
            offset,
            blur,
            color,
        }
        .sanitized()
    }

    /// 非有限值收成 0，模糊和颜色通道钳到合法范围。
    pub fn sanitized(self) -> Self {
        Self {
            offset: Vec2::new(finite_or_zero(self.offset.x), finite_or_zero(self.offset.y)),
            blur: if self.blur.is_finite() {
                self.blur.max(0.0)
            } else {
                0.0
            },
            color: Color::new(
                finite_unit(self.color.r),
                finite_unit(self.color.g),
                finite_unit(self.color.b),
                finite_unit(self.color.a),
            ),
        }
    }

    /// 不需要绘制。
    ///
    /// 颜色 alpha 接近 0，或者偏移和模糊都接近 0（贴在本体后面，没有可见光晕）。
    pub fn is_off(&self) -> bool {
        let shadow = self.sanitized();
        if shadow.color.a <= 1.0e-4 {
            return true;
        }
        shadow.blur <= 1.0e-4 && shadow.offset.x.abs() <= 1.0e-4 && shadow.offset.y.abs() <= 1.0e-4
    }

    /// 和 [`Self::default`] 相同。序列化时整段省略，避免旧文件变长。
    pub fn is_unset(&self) -> bool {
        let shadow = self.sanitized();
        shadow.offset == Vec2::ZERO
            && shadow.blur == 0.0
            && shadow.color.r == 0.0
            && shadow.color.g == 0.0
            && shadow.color.b == 0.0
            && shadow.color.a == 0.0
    }
}

/// 有符号距离上的浓度。距离小于等于 0（剪影内部）为 1，超过 `blur` 为 0。
///
/// 中间用 smoothstep，外圈比线性衰减更软。`blur` 为 0 时内部仍是 1，外部是 0。
pub(crate) fn feather_coverage(signed_distance: f32, blur: f32) -> f32 {
    if !signed_distance.is_finite() {
        return 0.0;
    }
    if signed_distance <= 0.0 {
        return 1.0;
    }
    if !blur.is_finite() || blur <= 1.0e-4 || signed_distance >= blur {
        return 0.0;
    }
    let t = 1.0 - signed_distance / blur;
    t * t * (3.0 - 2.0 * t)
}

/// `src` 盖在 `dst` 上。两者都是直通 alpha，结果也是。
pub(crate) fn composite_over(src: Color, dst: Color) -> Color {
    let sa = src.a.clamp(0.0, 1.0);
    let da = dst.a.clamp(0.0, 1.0);
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 1.0e-6 {
        return Color::TRANSPARENT;
    }
    let inv = 1.0 - sa;
    Color::new(
        (src.r.clamp(0.0, 1.0) * sa + dst.r.clamp(0.0, 1.0) * da * inv) / out_a,
        (src.g.clamp(0.0, 1.0) * sa + dst.g.clamp(0.0, 1.0) * da * inv) / out_a,
        (src.b.clamp(0.0, 1.0) * sa + dst.b.clamp(0.0, 1.0) * da * inv) / out_a,
        out_a,
    )
}

/// 闭合轮廓的软阴影。轮廓可以首尾重复。偏移在局部空间，再乘 `transform`。
///
/// 关闭时返回空。模糊为 0 时只有实心剪影。
pub(crate) fn soft_shadow_vertices(
    outline: &[Vec2],
    shadow: DropShadow,
    opacity: f32,
    transform: &Transform,
) -> Vec<Vertex> {
    let shadow = shadow.sanitized();
    if shadow.is_off() {
        return Vec::new();
    }
    let ring = open_ring(outline);
    if ring.len() < 3 {
        return Vec::new();
    }
    let moved: Vec<Vec2> = ring.iter().map(|point| *point + shadow.offset).collect();
    let solid = shade(shadow, opacity, 1.0);
    let mut vertices = fan_fill(&moved, solid, transform);
    if shadow.blur <= 1.0e-3 {
        return vertices;
    }
    let normals = outward_normals(&moved);
    for index in 0..FEATHER_RINGS {
        let inner_d = shadow.blur * (index as f32) / (FEATHER_RINGS as f32);
        let outer_d = shadow.blur * ((index + 1) as f32) / (FEATHER_RINGS as f32);
        let outer: Vec<Vec2> = moved
            .iter()
            .zip(&normals)
            .map(|(point, normal)| *point + *normal * outer_d)
            .collect();
        let inner_pts: Vec<Vec2> = if index == 0 {
            moved.clone()
        } else {
            moved
                .iter()
                .zip(&normals)
                .map(|(point, normal)| *point + *normal * inner_d)
                .collect()
        };
        vertices.extend(feather_strip(
            &inner_pts,
            &outer,
            shade(shadow, opacity, feather_coverage(inner_d, shadow.blur)),
            shade(shadow, opacity, feather_coverage(outer_d, shadow.blur)),
            transform,
        ));
    }
    vertices
}

/// 开口笔画的胶囊剪影，再交给 [`soft_shadow_vertices`]。
pub(crate) fn stroke_capsule_shadow(
    centerline: &[Vec2],
    half_width: f32,
    shadow: DropShadow,
    opacity: f32,
    transform: &Transform,
) -> Vec<Vertex> {
    let outline = capsule_outline(centerline, half_width);
    soft_shadow_vertices(&outline, shadow, opacity, transform)
}

/// 轴对齐盒子的软阴影。`origin` 是局部左上角，`size` 可以是负的。
pub(crate) fn quad_shadow(
    origin: Vec2,
    size: Vec2,
    shadow: DropShadow,
    opacity: f32,
    transform: &Transform,
) -> Vec<Vertex> {
    let min = Vec2::new(
        origin.x.min(origin.x + size.x),
        origin.y.min(origin.y + size.y),
    );
    let max = Vec2::new(
        origin.x.max(origin.x + size.x),
        origin.y.max(origin.y + size.y),
    );
    let outline = [
        Vec2::new(min.x, min.y),
        Vec2::new(max.x, min.y),
        Vec2::new(max.x, max.y),
        Vec2::new(min.x, max.y),
        Vec2::new(min.x, min.y),
    ];
    soft_shadow_vertices(&outline, shadow, opacity, transform)
}

fn shade(shadow: DropShadow, opacity: f32, coverage: f32) -> Color {
    let alpha =
        (shadow.color.a * opacity.clamp(0.0, 1.0) * coverage.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    Color::new(
        shadow.color.r.clamp(0.0, 1.0),
        shadow.color.g.clamp(0.0, 1.0),
        shadow.color.b.clamp(0.0, 1.0),
        alpha,
    )
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn finite_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn open_ring(outline: &[Vec2]) -> Vec<Vec2> {
    let mut points = Vec::new();
    for point in outline {
        if !point.x.is_finite() || !point.y.is_finite() {
            continue;
        }
        if points
            .last()
            .is_some_and(|prev: &Vec2| (*prev - *point).length_squared() < 1.0e-8)
        {
            continue;
        }
        points.push(*point);
    }
    if points.len() >= 2 {
        let first = points[0];
        let last = points[points.len() - 1];
        if (first - last).length_squared() < 1.0e-8 {
            points.pop();
        }
    }
    points
}

fn fan_fill(ring: &[Vec2], color: Color, transform: &Transform) -> Vec<Vertex> {
    let center = transform.transform_point(centroid(ring));
    let mut vertices = Vec::with_capacity(ring.len() * 3);
    for index in 0..ring.len() {
        let a = transform.transform_point(ring[index]);
        let b = transform.transform_point(ring[(index + 1) % ring.len()]);
        vertices.push(Vertex::new(center.x, center.y, color));
        vertices.push(Vertex::new(a.x, a.y, color));
        vertices.push(Vertex::new(b.x, b.y, color));
    }
    vertices
}

fn feather_strip(
    inner: &[Vec2],
    outer: &[Vec2],
    inner_color: Color,
    outer_color: Color,
    transform: &Transform,
) -> Vec<Vertex> {
    let count = inner.len().min(outer.len());
    let mut vertices = Vec::with_capacity(count * 6);
    for index in 0..count {
        let next = (index + 1) % count;
        let a = transform.transform_point(inner[index]);
        let b = transform.transform_point(inner[next]);
        let c = transform.transform_point(outer[index]);
        let d = transform.transform_point(outer[next]);
        vertices.push(Vertex::new(a.x, a.y, inner_color));
        vertices.push(Vertex::new(c.x, c.y, outer_color));
        vertices.push(Vertex::new(b.x, b.y, inner_color));
        vertices.push(Vertex::new(b.x, b.y, inner_color));
        vertices.push(Vertex::new(c.x, c.y, outer_color));
        vertices.push(Vertex::new(d.x, d.y, outer_color));
    }
    vertices
}

fn centroid(points: &[Vec2]) -> Vec2 {
    let scale = points.len() as f32;
    points.iter().fold(Vec2::ZERO, |acc, point| acc + *point) / scale
}

fn signed_area(points: &[Vec2]) -> f32 {
    let mut sum = 0.0;
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        sum += a.x * b.y - b.x * a.y;
    }
    sum * 0.5
}

fn outward_normals(points: &[Vec2]) -> Vec<Vec2> {
    let count = points.len();
    let clockwise = signed_area(points) >= 0.0;
    let mut edge_normals = Vec::with_capacity(count);
    for index in 0..count {
        let edge = points[(index + 1) % count] - points[index];
        edge_normals.push(edge_outward(edge, clockwise));
    }
    let mut normals = Vec::with_capacity(count);
    for index in 0..count {
        let previous = edge_normals[(index + count - 1) % count];
        normals.push(miter_normal(previous, edge_normals[index]));
    }
    normals
}

fn edge_outward(edge: Vec2, clockwise: bool) -> Vec2 {
    let raw = if clockwise {
        Vec2::new(edge.y, -edge.x)
    } else {
        Vec2::new(-edge.y, edge.x)
    };
    unit(raw).unwrap_or(Vec2::ZERO)
}

fn miter_normal(first: Vec2, second: Vec2) -> Vec2 {
    if first.length_squared() < 1.0e-8 {
        return if second.length_squared() < 1.0e-8 {
            Vec2::new(0.0, -1.0)
        } else {
            second
        };
    }
    if second.length_squared() < 1.0e-8 {
        return first;
    }
    let sum = first + second;
    let Some(direction) = unit(sum) else {
        return first;
    };
    let denom = direction.dot(first).abs().max(0.2);
    direction / denom
}

fn capsule_outline(centerline: &[Vec2], half_width: f32) -> Vec<Vec2> {
    let points = open_ring_open(centerline);
    if points.len() < 2 || !half_width.is_finite() || half_width <= 1.0e-4 {
        return Vec::new();
    }
    let mut left = Vec::with_capacity(points.len());
    let mut right = Vec::with_capacity(points.len());
    for index in 0..points.len() {
        let normal = polyline_normal(&points, index);
        left.push(points[index] + normal * half_width);
        right.push(points[index] - normal * half_width);
    }
    let mut outline = Vec::new();
    outline.extend(left.iter().copied());
    let end_dir =
        unit(points[points.len() - 1] - points[points.len() - 2]).unwrap_or(Vec2::new(1.0, 0.0));
    append_cap(&mut outline, points[points.len() - 1], end_dir, half_width);
    outline.extend(right.iter().rev().copied());
    let start_dir = unit(points[0] - points[1]).unwrap_or(Vec2::new(-1.0, 0.0));
    append_cap(&mut outline, points[0], start_dir, half_width);
    if let Some(first) = outline.first().copied() {
        outline.push(first);
    }
    outline
}

/// 去掉相邻重复点，但保留开口折线的两端，即使它们在几何上会绕回去。
fn open_ring_open(points: &[Vec2]) -> Vec<Vec2> {
    let mut cleaned = Vec::new();
    for point in points {
        if !point.x.is_finite() || !point.y.is_finite() {
            continue;
        }
        if cleaned
            .last()
            .is_some_and(|prev: &Vec2| (*prev - *point).length_squared() < 1.0e-8)
        {
            continue;
        }
        cleaned.push(*point);
    }
    cleaned
}

fn polyline_normal(points: &[Vec2], index: usize) -> Vec2 {
    let previous = if index > 0 {
        unit(points[index] - points[index - 1])
    } else {
        None
    };
    let next = if index + 1 < points.len() {
        unit(points[index + 1] - points[index])
    } else {
        None
    };
    match (previous, next) {
        (Some(before), Some(after)) => miter_normal(left_normal(before), left_normal(after)),
        (Some(dir), None) | (None, Some(dir)) => left_normal(dir),
        (None, None) => Vec2::new(0.0, 1.0),
    }
}

fn left_normal(direction: Vec2) -> Vec2 {
    Vec2::new(-direction.y, direction.x)
}

fn append_cap(outline: &mut Vec<Vec2>, center: Vec2, direction: Vec2, radius: f32) {
    let left = left_normal(direction);
    for step in 1..CAP_SEGMENTS {
        let angle = std::f32::consts::PI * (step as f32) / (CAP_SEGMENTS as f32);
        outline.push(center + left * (angle.cos() * radius) + direction * (angle.sin() * radius));
    }
}

fn unit(vector: Vec2) -> Option<Vec2> {
    let length_squared = vector.length_squared();
    if length_squared < 1.0e-12 {
        None
    } else {
        Some(vector / length_squared.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitecanvas_math::Transform;

    fn square() -> Vec<Vec2> {
        vec![
            Vec2::new(-5.0, -5.0),
            Vec2::new(5.0, -5.0),
            Vec2::new(5.0, 5.0),
            Vec2::new(-5.0, 5.0),
            Vec2::new(-5.0, -5.0),
        ]
    }

    fn black(alpha: f32) -> Color {
        Color::new(0.0, 0.0, 0.0, alpha)
    }

    #[test]
    fn off_shadow_emits_no_vertices_and_partial_json_stays_clear() {
        let off = DropShadow::default();
        assert!(off.is_off());
        assert!(off.is_unset());
        assert!(soft_shadow_vertices(&square(), off, 1.0, &Transform::identity()).is_empty());

        let partial: DropShadow = serde_json::from_str(r#"{"blur":4}"#).unwrap();
        assert_eq!(partial.color.a, 0.0);
        assert!(partial.is_off());

        let empty: DropShadow = serde_json::from_str("{}").unwrap();
        assert!(empty.is_unset());
    }

    #[test]
    fn hard_shadow_is_the_silhouette_shifted_in_local_space() {
        let shadow = DropShadow::new(Vec2::new(12.0, 0.0), 0.0, black(0.5));
        let vertices = soft_shadow_vertices(&square(), shadow, 1.0, &Transform::identity());
        assert!(!vertices.is_empty());
        assert!(vertices.iter().all(|vertex| {
            vertex.position[0] >= 6.9
                && vertex.position[0] <= 17.1
                && vertex.position[1] >= -5.1
                && vertex.position[1] <= 5.1
                && (vertex.color[3] - 0.5).abs() < 1.0e-4
        }));
        assert!(vertices
            .iter()
            .any(|vertex| (vertex.position[0] - 12.0).abs() < 1.0e-3
                && vertex.position[1].abs() < 1.0e-3));
    }

    #[test]
    fn blur_extends_past_the_hard_silhouette_and_fades() {
        let hard = DropShadow::new(Vec2::new(12.0, 0.0), 0.0, black(0.5));
        let soft = DropShadow::new(Vec2::new(12.0, 0.0), 8.0, black(0.5));
        let hard_count = soft_shadow_vertices(&square(), hard, 1.0, &Transform::identity()).len();
        let vertices = soft_shadow_vertices(&square(), soft, 1.0, &Transform::identity());
        assert!(vertices.len() > hard_count);
        assert!(vertices.iter().any(|vertex| vertex.position[0] > 17.5));
        assert!(vertices.iter().any(|vertex| vertex.color[3] < 0.2));
        assert!(vertices
            .iter()
            .any(|vertex| (vertex.color[3] - 0.5).abs() < 1.0e-3));
    }

    #[test]
    fn element_opacity_scales_shadow_alpha_and_rotation_turns_the_offset() {
        let shadow = DropShadow::new(Vec2::new(10.0, 0.0), 0.0, black(0.5));
        let faded = soft_shadow_vertices(&square(), shadow, 0.5, &Transform::identity());
        assert!(faded
            .iter()
            .all(|vertex| (vertex.color[3] - 0.25).abs() < 1.0e-4));

        let mut transform = Transform::identity();
        transform.rotation = std::f32::consts::FRAC_PI_2;
        let turned = soft_shadow_vertices(&square(), shadow, 1.0, &transform);
        assert!(turned.iter().all(|vertex| vertex.position[0] <= 5.2));
        assert!(turned.iter().all(|vertex| vertex.position[1] >= 4.8));
        assert!(turned.iter().any(|vertex| vertex.position[0].abs() < 1.0e-2
            && (vertex.position[1] - 10.0).abs() < 1.0e-2));
    }

    #[test]
    fn feather_is_solid_inside_and_clear_past_the_blur() {
        assert_eq!(feather_coverage(-2.0, 4.0), 1.0);
        assert_eq!(feather_coverage(0.0, 0.0), 1.0);
        assert_eq!(feather_coverage(1.0, 0.0), 0.0);
        assert_eq!(feather_coverage(4.0, 4.0), 0.0);
        let mid = feather_coverage(2.0, 4.0);
        assert!(mid > 0.4 && mid < 0.6);
    }
}
