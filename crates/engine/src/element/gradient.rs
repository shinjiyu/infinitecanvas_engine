//! 线性 / 径向渐变填充。
//!
//! 纯色仍是缺省。渐变写在 [`super::ElementStyle`] 上，经 `SetStyle` 一次撤销。
//!
//! ```text
//! 局部点 ── gradient_offset ── 色标插值 ── 顶点颜色
//!                │
//!                ├─ 线性：沿角度投影到包围盒，按色标切成条带（条带内颜色是仿射的）
//!                └─ 径向：从中心发出射线，在每个色标半径上放一圈顶点
//! ```
//!
//! WebGPU 仍用顶点颜色。线性条带内的插值与色标一致。径向沿每条射线一致，
//! 射线之间是分段线性近似。描边不读这些字段，保持纯色。
//! 不透明度在写入顶点或 PNG 像素时乘进 alpha。

use super::stroke::{color_with_opacity, dedup_points};
use super::ElementStyle;
use infinitecanvas_math::{Color, Rect, Transform, Vec2, Vertex};
use serde::{Deserialize, Serialize};

/// 一个渐变里最多保留的色标数。再多的会被丢掉。
pub const MAX_GRADIENT_STOPS: usize = 16;

/// 径向填充的射线数。沿每条射线颜色按色标精确，射线之间是近似。
const RADIAL_RAYS: usize = 32;

/// 填充种类。
///
/// JSON 用小写名字。缺省 [`FillKind::Solid`]，所以没有 `fill_kind` 的旧文件仍是纯色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FillKind {
    /// 使用 [`ElementStyle::fill`]。缺省值。
    #[default]
    Solid,
    /// 沿 [`ElementStyle::fill_angle`] 跨越局部包围盒。
    Linear,
    /// 以 [`ElementStyle::fill_center`] 为圆心，[`ElementStyle::fill_radius`] 为半径。
    Radial,
}

impl FillKind {
    /// 属性面板和 WASM 用的名字。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Linear => "linear",
            Self::Radial => "radial",
        }
    }

    /// 解析 `solid` / `linear` / `radial`。无法识别时返回 `None`。
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "solid" => Some(Self::Solid),
            "linear" => Some(Self::Linear),
            "radial" => Some(Self::Radial),
            _ => None,
        }
    }
}

/// 渐变上的一个色标。
///
/// `offset` 为 0 到 1。颜色通道与 [`Color`] 相同，范围 0 到 1。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    /// 在渐变轴上的位置。0 是起点，1 是终点。
    #[serde(default)]
    pub offset: f32,
    /// 该位置的颜色。JSON 里缺省的通道与 [`Color`] 相同。
    #[serde(default)]
    pub color: Color,
}

impl GradientStop {
    /// 丢掉非有限值，把 offset 钳到 0 到 1，按 offset 排序，并合并靠得过近的色标。
    ///
    /// 同一位置保留后者。超过 [`MAX_GRADIENT_STOPS`] 的尾部被丢掉。
    pub fn normalize(stops: &[Self]) -> Vec<Self> {
        let mut out: Vec<Self> = stops
            .iter()
            .filter(|stop| stop.offset.is_finite() && color_is_finite(stop.color))
            .map(|stop| Self {
                offset: stop.offset.clamp(0.0, 1.0),
                color: stop.color,
            })
            .collect();
        out.sort_by(|left, right| {
            left.offset
                .partial_cmp(&right.offset)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut merged: Vec<Self> = Vec::with_capacity(out.len());
        for stop in out {
            if let Some(last) = merged.last_mut() {
                if (last.offset - stop.offset).abs() < 1.0e-4 {
                    *last = stop;
                    continue;
                }
            }
            merged.push(stop);
        }
        if merged.len() > MAX_GRADIENT_STOPS {
            merged.truncate(MAX_GRADIENT_STOPS);
        }
        merged
    }
}

impl ElementStyle {
    /// 已经整理过的色标。纯色填充不看这个结果。
    pub fn normalized_stops(&self) -> Vec<GradientStop> {
        GradientStop::normalize(&self.fill_stops)
    }

    /// 线性或径向，且至少有一个有效色标。
    ///
    /// 种类是渐变但色标为空时返回 false，绘制退回 [`ElementStyle::fill`]。
    pub fn uses_gradient(&self) -> bool {
        matches!(self.fill_kind, FillKind::Linear | FillKind::Radial)
            && !self.normalized_stops().is_empty()
    }

    /// 有填充可画。渐变看色标，纯色看 [`ElementStyle::fill`]。
    pub fn has_paintable_fill(&self) -> bool {
        if self.uses_gradient() {
            true
        } else {
            self.fill.is_some()
        }
    }

    /// 局部点的填充色，还没乘元素不透明度。
    ///
    /// 没有可绘制的填充时返回 `None`。渐变在包围盒上取值：线性把包围盒投影到角度上，
    /// 径向用到 [`Self::fill_center`] 的距离除以半径。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::math::{Color, Rect, Vec2};
    /// use infinitecanvas_engine::{ElementStyle, FillKind, GradientStop};
    ///
    /// let mut style = ElementStyle::fill_only(Color::BLACK);
    /// style.fill_kind = FillKind::Linear;
    /// style.fill_stops = vec![
    ///     GradientStop { offset: 0.0, color: Color::RED },
    ///     GradientStop { offset: 1.0, color: Color::BLUE },
    /// ];
    /// let bounds = Rect::from_center_size(Vec2::ZERO, Vec2::new(20.0, 10.0));
    /// assert_eq!(style.sample_fill(Vec2::new(-10.0, 0.0), bounds), Some(Color::RED));
    /// assert_eq!(style.sample_fill(Vec2::new(10.0, 0.0), bounds), Some(Color::BLUE));
    /// ```
    pub fn sample_fill(&self, local: Vec2, bounds: Rect) -> Option<Color> {
        match self.fill_kind {
            FillKind::Solid => self.fill,
            FillKind::Linear | FillKind::Radial => {
                let stops = self.normalized_stops();
                if stops.is_empty() {
                    self.fill
                } else {
                    Some(sample_stops(&stops, self.gradient_offset(local, bounds)))
                }
            }
        }
    }

    /// 局部点在渐变轴上的位置，范围 0 到 1。纯色返回 0。
    pub fn gradient_offset(&self, local: Vec2, bounds: Rect) -> f32 {
        match self.fill_kind {
            FillKind::Solid => 0.0,
            FillKind::Linear => linear_offset(local, bounds, self.fill_angle),
            FillKind::Radial => {
                let radius = radial_radius(self.fill_radius, bounds);
                if radius <= 1.0e-4 {
                    0.0
                } else {
                    ((local - self.fill_center).length() / radius).clamp(0.0, 1.0)
                }
            }
        }
    }

    /// 径向渐变实际使用的半径。0 或非有限值用包围盒对角线的一半。
    pub fn radial_radius(&self, bounds: Rect) -> f32 {
        radial_radius(self.fill_radius, bounds)
    }
}

/// 把凸轮廓铺成渐变填充，再变到世界坐标。
///
/// `outline` 可以首尾重复。少于 3 个点时返回空。描边不经过这里。
pub(crate) fn paint_gradient_fill(
    outline: &[Vec2],
    style: &ElementStyle,
    bounds: Rect,
    transform: &Transform,
) -> Vec<Vertex> {
    let stops = style.normalized_stops();
    let ring = open_ring(outline);
    if ring.len() < 3 || stops.is_empty() {
        return Vec::new();
    }
    let local = match style.fill_kind {
        FillKind::Radial => radial_triangles(
            &ring,
            style.fill_center,
            style.radial_radius(bounds),
            &stops,
        ),
        FillKind::Linear | FillKind::Solid => {
            linear_triangles(&ring, bounds, style.fill_angle, &stops)
        }
    };
    let mut vertices = Vec::with_capacity(local.len());
    for triangle in local {
        for point in triangle {
            let color = color_with_opacity(
                sample_stops(&stops, style.gradient_offset(point, bounds)),
                style.opacity,
            );
            let world = transform.transform_point(point);
            vertices.push(Vertex::new(world.x, world.y, color));
        }
    }
    vertices
}

fn color_is_finite(color: Color) -> bool {
    color.r.is_finite() && color.g.is_finite() && color.b.is_finite() && color.a.is_finite()
}

fn radial_radius(radius: f32, bounds: Rect) -> f32 {
    if radius.is_finite() && radius > 1.0e-4 {
        return radius;
    }
    let diagonal = bounds.width.hypot(bounds.height) * 0.5;
    if diagonal.is_finite() && diagonal > 1.0e-4 {
        diagonal
    } else {
        1.0
    }
}

fn linear_offset(local: Vec2, bounds: Rect, angle_deg: f32) -> f32 {
    let (dir, min_p, span) = linear_frame(bounds, angle_deg);
    ((local.dot(dir) - min_p) / span).clamp(0.0, 1.0)
}

fn linear_frame(bounds: Rect, angle_deg: f32) -> (Vec2, f32, f32) {
    let radians = if angle_deg.is_finite() {
        angle_deg.to_radians()
    } else {
        0.0
    };
    let dir = Vec2::new(radians.cos(), radians.sin());
    let corners = [
        Vec2::new(bounds.x, bounds.y),
        Vec2::new(bounds.x + bounds.width, bounds.y),
        Vec2::new(bounds.x, bounds.y + bounds.height),
        Vec2::new(bounds.x + bounds.width, bounds.y + bounds.height),
    ];
    let mut min_p = f32::INFINITY;
    let mut max_p = f32::NEG_INFINITY;
    for corner in corners {
        let projected = corner.dot(dir);
        min_p = min_p.min(projected);
        max_p = max_p.max(projected);
    }
    if !min_p.is_finite() || !max_p.is_finite() {
        return (Vec2::new(1.0, 0.0), 0.0, 1.0);
    }
    let span = (max_p - min_p).max(1.0e-6);
    (dir, min_p, span)
}

fn sample_stops(stops: &[GradientStop], t: f32) -> Color {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let Some(first) = stops.first() else {
        return Color::BLACK;
    };
    if t <= first.offset {
        return first.color;
    }
    let Some(last) = stops.last() else {
        return first.color;
    };
    if t >= last.offset {
        return last.color;
    }
    for pair in stops.windows(2) {
        let left = pair[0];
        let right = pair[1];
        if t >= left.offset && t <= right.offset {
            let span = (right.offset - left.offset).max(1.0e-6);
            let amount = (t - left.offset) / span;
            return lerp_color(left.color, right.color, amount);
        }
    }
    last.color
}

fn lerp_color(left: Color, right: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color::new(
        left.r + (right.r - left.r) * amount,
        left.g + (right.g - left.g) * amount,
        left.b + (right.b - left.b) * amount,
        left.a + (right.a - left.a) * amount,
    )
}

fn open_ring(points: &[Vec2]) -> Vec<Vec2> {
    let mut ring = dedup_points(points);
    if ring.len() >= 2 {
        let first = ring[0];
        if let Some(last) = ring.last().copied() {
            if (first - last).length_squared() <= 1.0e-8 {
                ring.pop();
            }
        }
    }
    ring
}

fn linear_triangles(
    ring: &[Vec2],
    bounds: Rect,
    angle_deg: f32,
    stops: &[GradientStop],
) -> Vec<[Vec2; 3]> {
    let (dir, min_p, span) = linear_frame(bounds, angle_deg);
    let mut edges = vec![0.0];
    for stop in stops {
        let offset = stop.offset.clamp(0.0, 1.0);
        if offset - *edges.last().unwrap_or(&0.0) > 1.0e-4 {
            edges.push(offset);
        }
    }
    if 1.0 - *edges.last().unwrap_or(&0.0) > 1.0e-4 {
        edges.push(1.0);
    }
    let mut triangles = Vec::new();
    for pair in edges.windows(2) {
        let low = min_p + pair[0] * span;
        let high = min_p + pair[1] * span;
        let clipped = clip_halfplane(ring, |point| point.dot(dir) - low);
        let clipped = clip_halfplane(&clipped, |point| high - point.dot(dir));
        push_fan(&mut triangles, &clipped);
    }
    triangles
}

fn clip_halfplane(poly: &[Vec2], inside: impl Fn(Vec2) -> f32) -> Vec<Vec2> {
    if poly.len() < 3 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(poly.len() + 1);
    let count = poly.len();
    for index in 0..count {
        let current = poly[index];
        let previous = poly[(index + count - 1) % count];
        let current_in = inside(current) >= -1.0e-4;
        let previous_in = inside(previous) >= -1.0e-4;
        if current_in {
            if !previous_in {
                if let Some(hit) = intersect_zero(previous, current, &inside) {
                    out.push(hit);
                }
            }
            out.push(current);
        } else if previous_in {
            if let Some(hit) = intersect_zero(previous, current, &inside) {
                out.push(hit);
            }
        }
    }
    dedup_points(&out)
}

fn intersect_zero(start: Vec2, end: Vec2, inside: &impl Fn(Vec2) -> f32) -> Option<Vec2> {
    let start_side = inside(start);
    let end_side = inside(end);
    let denom = start_side - end_side;
    if denom.abs() < 1.0e-8 {
        return None;
    }
    let t = (start_side / denom).clamp(0.0, 1.0);
    Some(start + (end - start) * t)
}

fn push_fan(triangles: &mut Vec<[Vec2; 3]>, poly: &[Vec2]) {
    if poly.len() < 3 {
        return;
    }
    for index in 1..poly.len() - 1 {
        let triangle = [poly[0], poly[index], poly[index + 1]];
        if triangle_area(triangle) > 1.0e-6 {
            triangles.push(triangle);
        }
    }
}

fn triangle_area(triangle: [Vec2; 3]) -> f32 {
    (triangle[1] - triangle[0])
        .perp_dot(triangle[2] - triangle[0])
        .abs()
}

fn radial_triangles(
    ring: &[Vec2],
    center: Vec2,
    radius: f32,
    stops: &[GradientStop],
) -> Vec<[Vec2; 3]> {
    let origin = if point_in_convex(center, ring) {
        center
    } else {
        centroid(ring)
    };
    let mut radii = Vec::new();
    for stop in stops {
        let distance = stop.offset.clamp(0.0, 1.0) * radius;
        if distance > 1.0e-3
            && radii
                .last()
                .map(|previous: &f32| distance - *previous > 1.0e-3)
                .unwrap_or(true)
        {
            radii.push(distance);
        }
    }
    let mut triangles = Vec::new();
    let step = std::f32::consts::TAU / RADIAL_RAYS as f32;
    for index in 0..RADIAL_RAYS {
        let dir_a = Vec2::new((index as f32 * step).cos(), (index as f32 * step).sin());
        let dir_b = Vec2::new(
            ((index + 1) as f32 * step).cos(),
            ((index + 1) as f32 * step).sin(),
        );
        let Some(hit_a) = ray_exit(origin, dir_a, ring) else {
            continue;
        };
        let Some(hit_b) = ray_exit(origin, dir_b, ring) else {
            continue;
        };
        let chain_a = ray_chain(origin, dir_a, hit_a, &radii);
        let chain_b = ray_chain(origin, dir_b, hit_b, &radii);
        stitch_rays(&mut triangles, &chain_a, &chain_b);
    }
    triangles
}

fn centroid(ring: &[Vec2]) -> Vec2 {
    let sum = ring
        .iter()
        .copied()
        .fold(Vec2::ZERO, |acc, point| acc + point);
    sum / ring.len() as f32
}

fn point_in_convex(point: Vec2, ring: &[Vec2]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut sign = 0.0;
    let count = ring.len();
    for index in 0..count {
        let start = ring[index];
        let end = ring[(index + 1) % count];
        let cross = (end - start).perp_dot(point - start);
        if cross.abs() <= 1.0e-4 {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if sign * cross < 0.0 {
            return false;
        }
    }
    true
}

fn ray_exit(origin: Vec2, dir: Vec2, ring: &[Vec2]) -> Option<f32> {
    let mut best: Option<f32> = None;
    let count = ring.len();
    for index in 0..count {
        let start = ring[index];
        let end = ring[(index + 1) % count];
        if let Some(distance) = ray_segment(origin, dir, start, end) {
            best = Some(
                best.map(|current| current.max(distance))
                    .unwrap_or(distance),
            );
        }
    }
    best.filter(|distance| *distance > 1.0e-4)
}

fn ray_segment(origin: Vec2, dir: Vec2, start: Vec2, end: Vec2) -> Option<f32> {
    let edge = end - start;
    let denom = dir.perp_dot(edge);
    if denom.abs() < 1.0e-8 {
        return None;
    }
    let delta = start - origin;
    let t = delta.perp_dot(edge) / denom;
    let u = delta.perp_dot(dir) / denom;
    if t >= 0.0 && (0.0..=1.0).contains(&u) {
        Some(t)
    } else {
        None
    }
}

fn ray_chain(origin: Vec2, dir: Vec2, hit: f32, radii: &[f32]) -> Vec<Vec2> {
    let mut points = vec![origin];
    for radius in radii {
        let distance = radius.min(hit);
        if distance <= 1.0e-4 {
            continue;
        }
        let point = origin + dir * distance;
        if (point - *points.last().unwrap()).length_squared() > 1.0e-8 {
            points.push(point);
        }
    }
    let boundary = origin + dir * hit;
    if (boundary - *points.last().unwrap()).length_squared() > 1.0e-8 {
        points.push(boundary);
    }
    points
}

fn stitch_rays(triangles: &mut Vec<[Vec2; 3]>, left: &[Vec2], right: &[Vec2]) {
    if left.is_empty() || right.is_empty() {
        return;
    }
    let origin = left[0];
    let mut i = 0;
    let mut j = 0;
    while i + 1 < left.len() || j + 1 < right.len() {
        let advance_left = if i + 1 >= left.len() {
            false
        } else if j + 1 >= right.len() {
            true
        } else {
            (left[i + 1] - origin).length_squared() <= (right[j + 1] - origin).length_squared()
        };
        let triangle = if advance_left {
            let triangle = [left[i], left[i + 1], right[j]];
            i += 1;
            triangle
        } else {
            let triangle = [left[i], right[j + 1], right[j]];
            j += 1;
            triangle
        };
        if triangle_area(triangle) > 1.0e-6 {
            triangles.push(triangle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red_blue_linear() -> ElementStyle {
        let mut style = ElementStyle::fill_only(Color::WHITE);
        style.fill_kind = FillKind::Linear;
        style.fill_angle = 0.0;
        style.fill_stops = vec![
            GradientStop {
                offset: 0.0,
                color: Color::RED,
            },
            GradientStop {
                offset: 1.0,
                color: Color::BLUE,
            },
        ];
        style
    }

    fn bounds() -> Rect {
        Rect::from_center_size(Vec2::ZERO, Vec2::new(20.0, 10.0))
    }

    fn rect_ring() -> Vec<Vec2> {
        vec![
            Vec2::new(-10.0, -5.0),
            Vec2::new(10.0, -5.0),
            Vec2::new(10.0, 5.0),
            Vec2::new(-10.0, 5.0),
        ]
    }

    fn nearest(vertices: &[Vertex], x: f32) -> &Vertex {
        vertices
            .iter()
            .min_by(|left, right| {
                (left.position[0] - x)
                    .abs()
                    .partial_cmp(&(right.position[0] - x).abs())
                    .unwrap()
            })
            .unwrap()
    }

    #[test]
    fn normalize_drops_bad_stops_and_sorts() {
        let stops = GradientStop::normalize(&[
            GradientStop {
                offset: 1.5,
                color: Color::BLUE,
            },
            GradientStop {
                offset: f32::NAN,
                color: Color::RED,
            },
            GradientStop {
                offset: 0.0,
                color: Color::RED,
            },
            GradientStop {
                offset: 0.0,
                color: Color::GREEN,
            },
        ]);
        assert_eq!(stops.len(), 2);
        assert_eq!(stops[0].offset, 0.0);
        assert_eq!(stops[0].color, Color::GREEN);
        assert_eq!(stops[1].offset, 1.0);
        assert_eq!(stops[1].color, Color::BLUE);
    }

    #[test]
    fn linear_vertices_match_the_sampled_color_and_split_on_stops() {
        let mut style = red_blue_linear();
        style.fill_stops.insert(
            1,
            GradientStop {
                offset: 0.5,
                color: Color::GREEN,
            },
        );
        let vertices = paint_gradient_fill(&rect_ring(), &style, bounds(), &Transform::identity());
        assert!(vertices.len() >= 12);
        assert!(vertices.len().is_multiple_of(3));
        for vertex in &vertices {
            let local = Vec2::new(vertex.position[0], vertex.position[1]);
            let expected = style.sample_fill(local, bounds()).unwrap();
            assert!((vertex.color[0] - expected.r).abs() < 1.0e-4);
            assert!((vertex.color[1] - expected.g).abs() < 1.0e-4);
            assert!((vertex.color[2] - expected.b).abs() < 1.0e-4);
        }
        let mid = nearest(&vertices, 0.0);
        assert!(
            mid.color[1] > 0.9,
            "midline should be green, got {:?}",
            mid.color
        );
        assert!(nearest(&vertices, -10.0).color[0] > 0.9);
        assert!(nearest(&vertices, 10.0).color[2] > 0.9);

        for triangle in vertices.chunks(3) {
            let ts: Vec<f32> = triangle
                .iter()
                .map(|vertex| {
                    style.gradient_offset(
                        Vec2::new(vertex.position[0], vertex.position[1]),
                        bounds(),
                    )
                })
                .collect();
            let min_t = ts.iter().copied().fold(f32::INFINITY, f32::min);
            let max_t = ts.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            assert!(
                max_t - min_t <= 0.5 + 1.0e-3,
                "triangle spans more than one stop band: {ts:?}"
            );
        }
    }

    #[test]
    fn radial_center_and_rim_follow_stops_and_opacity() {
        let mut style = ElementStyle::fill_only(Color::WHITE);
        style.fill_kind = FillKind::Radial;
        style.fill_radius = 10.0;
        style.opacity = 0.5;
        style.fill_stops = vec![
            GradientStop {
                offset: 0.0,
                color: Color::RED,
            },
            GradientStop {
                offset: 1.0,
                color: Color::BLUE,
            },
        ];
        let square = Rect::from_center_size(Vec2::ZERO, Vec2::new(20.0, 20.0));
        let ring = [
            Vec2::new(-10.0, -10.0),
            Vec2::new(10.0, -10.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(-10.0, 10.0),
        ];
        let vertices = paint_gradient_fill(&ring, &style, square, &Transform::identity());
        let center = vertices
            .iter()
            .min_by(|left, right| {
                let left_d = Vec2::new(left.position[0], left.position[1]).length_squared();
                let right_d = Vec2::new(right.position[0], right.position[1]).length_squared();
                left_d.partial_cmp(&right_d).unwrap()
            })
            .unwrap();
        assert!(center.position[0].abs() < 1.0e-3 && center.position[1].abs() < 1.0e-3);
        assert!((center.color[0] - 1.0).abs() < 1.0e-4);
        assert!((center.color[3] - 0.5).abs() < 1.0e-4);
        let rim = vertices
            .iter()
            .max_by(|left, right| {
                let left_d = Vec2::new(left.position[0], left.position[1]).length_squared();
                let right_d = Vec2::new(right.position[0], right.position[1]).length_squared();
                left_d.partial_cmp(&right_d).unwrap()
            })
            .unwrap();
        let distance = Vec2::new(rim.position[0], rim.position[1]).length();
        assert!(distance > 9.0, "rim distance {distance}");
        assert!(
            rim.color[2] > 0.9,
            "rim should be blue, got {:?}",
            rim.color
        );
        assert!((rim.color[3] - 0.5).abs() < 1.0e-4);
    }

    #[test]
    fn empty_stops_fall_back_to_solid_fill() {
        let mut style = ElementStyle::fill_only(Color::RED);
        style.fill_kind = FillKind::Linear;
        assert!(!style.uses_gradient());
        assert_eq!(style.sample_fill(Vec2::ZERO, bounds()), Some(Color::RED));
        assert!(
            paint_gradient_fill(&rect_ring(), &style, bounds(), &Transform::identity()).is_empty()
        );
    }
}
