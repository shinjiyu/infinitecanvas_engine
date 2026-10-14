//! 折线描边几何
//!
//! 沿采样点生成带宽度的三角形带（展开为三角形列表），并处理拐角。
//! 斜接（miter）过长时退化为斜角（bevel），避免尖刺。

use super::ElementStyle;
use infinitecanvas_math::{Color, Rect, Transform, Vec2, Vertex};

/// 斜接长度超过半线宽的这个倍数时，拐角改为斜角。
pub(crate) const MITER_LIMIT: f32 = 4.0;

/// 点选细线时，在半线宽之外额外放宽的距离。
pub(crate) const HIT_SLOP: f32 = 3.0;

/// 点到线段的最短距离。
///
/// 投影落在线段外时，距离取到较近端点。
pub(crate) fn point_to_segment_distance(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= f32::EPSILON {
        return (point - a).length();
    }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let proj = a + ab * t;
    (point - proj).length()
}

/// 点到折线的最短距离。空折线返回无穷大。
pub(crate) fn distance_to_polyline(point: Vec2, points: &[Vec2]) -> f32 {
    if points.is_empty() {
        return f32::INFINITY;
    }
    if points.len() == 1 {
        return (point - points[0]).length();
    }
    let mut best = f32::INFINITY;
    for window in points.windows(2) {
        best = best.min(point_to_segment_distance(point, window[0], window[1]));
    }
    best
}

/// 去掉连续重合点，避免零长度段产生 NaN 法线。
pub(crate) fn dedup_points(points: &[Vec2]) -> Vec<Vec2> {
    const EPS_SQ: f32 = 1e-8;
    let mut out = Vec::with_capacity(points.len());
    for point in points {
        if out
            .last()
            .map(|prev: &Vec2| (*point - *prev).length_squared() > EPS_SQ)
            .unwrap_or(true)
        {
            out.push(*point);
        }
    }
    out
}

/// 单位方向。零长度返回 `None`。
fn safe_dir(from: Vec2, to: Vec2) -> Option<Vec2> {
    let delta = to - from;
    if delta.length_squared() <= 1e-12 {
        None
    } else {
        Some(delta.normalize())
    }
}

/// 朝折线左侧的单位法线（逆时针旋转 90°）。
fn left_normal(dir: Vec2) -> Vec2 {
    Vec2::new(-dir.y, dir.x)
}

struct StripVert {
    left: Vec2,
    right: Vec2,
}

/// 沿折线生成描边三角形。
///
/// # 参数
/// - `points`: 折线顶点（至少 2 个不重合的点才有输出）
/// - `width`: 线宽，小于等于 0 时返回空
/// - `color`: 顶点颜色
///
/// # 返回
/// 三角形列表（每 3 个顶点一个三角形）。拐角在斜接限制内用斜接，否则用斜角。
pub(crate) fn stroke_triangles(points: &[Vec2], width: f32, color: Color) -> Vec<Vertex> {
    if width <= 0.0 {
        return Vec::new();
    }
    let points = dedup_points(points);
    if points.len() < 2 {
        return Vec::new();
    }
    let half = width * 0.5;
    let sides = build_strip(&points, half);
    strip_to_vertices(&sides, color)
}

fn build_strip(points: &[Vec2], half: f32) -> Vec<StripVert> {
    let mut sides = Vec::new();
    let last = points.len() - 1;
    for index in 0..points.len() {
        if index == 0 {
            if let Some(dir) = safe_dir(points[0], points[1]) {
                push_cap(&mut sides, points[0], dir, half);
            }
        } else if index == last {
            if let Some(dir) = safe_dir(points[index - 1], points[index]) {
                push_cap(&mut sides, points[index], dir, half);
            }
        } else if let (Some(dir_in), Some(dir_out)) = (
            safe_dir(points[index - 1], points[index]),
            safe_dir(points[index], points[index + 1]),
        ) {
            push_join(&mut sides, points[index], dir_in, dir_out, half);
        }
    }
    sides
}

fn push_cap(sides: &mut Vec<StripVert>, point: Vec2, dir: Vec2, half: f32) {
    let normal = left_normal(dir) * half;
    sides.push(StripVert {
        left: point + normal,
        right: point - normal,
    });
}

/// 拐角：斜接长度在限制内时输出一个斜接点，否则输出两个斜角点。
fn push_join(sides: &mut Vec<StripVert>, curr: Vec2, dir_in: Vec2, dir_out: Vec2, half: f32) {
    let n0 = left_normal(dir_in);
    let n1 = left_normal(dir_out);
    let sum = n0 + n1;
    let sum_len = sum.length();
    if sum_len < 1e-5 {
        push_bevel(sides, curr, n0, n1, half);
        return;
    }
    let miter = sum / sum_len;
    let denom = miter.dot(n0);
    if denom.abs() < 1.0 / MITER_LIMIT {
        push_bevel(sides, curr, n0, n1, half);
        return;
    }
    let offset = miter * (half / denom);
    sides.push(StripVert {
        left: curr + offset,
        right: curr - offset,
    });
}

fn push_bevel(sides: &mut Vec<StripVert>, curr: Vec2, n0: Vec2, n1: Vec2, half: f32) {
    sides.push(StripVert {
        left: curr + n0 * half,
        right: curr - n0 * half,
    });
    sides.push(StripVert {
        left: curr + n1 * half,
        right: curr - n1 * half,
    });
}

fn strip_to_vertices(sides: &[StripVert], color: Color) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(sides.len().saturating_sub(1) * 6);
    for window in sides.windows(2) {
        let a = &window[0];
        let b = &window[1];
        push_tri(&mut vertices, a.left, a.right, b.left, color);
        push_tri(&mut vertices, b.left, a.right, b.right, color);
    }
    vertices
}

fn push_tri(vertices: &mut Vec<Vertex>, a: Vec2, b: Vec2, c: Vec2, color: Color) {
    vertices.push(Vertex::new(a.x, a.y, color));
    vertices.push(Vertex::new(b.x, b.y, color));
    vertices.push(Vertex::new(c.x, c.y, color));
}

/// 箭头尖端到箭尾的长度，随线宽放大。
pub(crate) fn arrow_length_for(stroke_width: f32) -> f32 {
    (stroke_width * 4.0).max(8.0)
}

/// 箭头底边宽度。
pub(crate) fn arrow_width_for(stroke_width: f32) -> f32 {
    (stroke_width * 3.0).max(6.0)
}

/// 箭头三角形：尖端、左翼、右翼。
///
/// `dir` 指向尖端方向。长度或宽度无效时返回 `None`。
pub(crate) fn arrow_points(tip: Vec2, dir: Vec2, length: f32, width: f32) -> Option<[Vec2; 3]> {
    if length <= 0.0 || width <= 0.0 || dir.length_squared() <= 1e-12 {
        return None;
    }
    let dir = dir.normalize();
    let base = tip - dir * length;
    let wing = left_normal(dir) * (width * 0.5);
    Some([tip, base + wing, base - wing])
}

/// 点是否在三角形内（含边界）。
pub(crate) fn point_in_triangle(point: Vec2, tri: [Vec2; 3]) -> bool {
    let (a, b, c) = (tri[0], tri[1], tri[2]);
    let v0 = c - a;
    let v1 = b - a;
    let v2 = point - a;
    let dot00 = v0.dot(v0);
    let dot01 = v0.dot(v1);
    let dot02 = v0.dot(v2);
    let dot11 = v1.dot(v1);
    let dot12 = v1.dot(v2);
    let denom = dot00 * dot11 - dot01 * dot01;
    if denom.abs() < 1e-8 {
        return false;
    }
    let inv = 1.0 / denom;
    let u = (dot11 * dot02 - dot01 * dot12) * inv;
    let v = (dot00 * dot12 - dot01 * dot02) * inv;
    u >= -1e-4 && v >= -1e-4 && (u + v) <= 1.0 + 1e-4
}

/// 将折线终点沿最后一段回退，给箭头留出箭杆。
pub(crate) fn shorten_end(points: &mut [Vec2], amount: f32) {
    if points.len() < 2 || amount <= 0.0 {
        return;
    }
    let last = points.len() - 1;
    let end = points[last];
    let prev = points[last - 1];
    let delta = end - prev;
    let len = delta.length();
    if len <= 1e-6 {
        return;
    }
    let back = amount.min(len * 0.95);
    points[last] = end - (delta / len) * back;
}

/// 点集的轴对齐包围盒，四边外扩 `padding`。
pub(crate) fn bounds_containing(points: &[Vec2], padding: f32) -> Rect {
    if points.is_empty() {
        return Rect::new(0.0, 0.0, 0.0, 0.0);
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for point in points {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    let pad = padding.max(0.0);
    Rect::new(
        min_x - pad,
        min_y - pad,
        (max_x - min_x) + pad * 2.0,
        (max_y - min_y) + pad * 2.0,
    )
}

/// 把局部顶点变到世界坐标。
pub(crate) fn transform_vertices(vertices: Vec<Vertex>, transform: &Transform) -> Vec<Vertex> {
    vertices
        .into_iter()
        .map(|mut vertex| {
            let point =
                transform.transform_point(Vec2::new(vertex.position[0], vertex.position[1]));
            vertex.position = [point.x, point.y];
            vertex
        })
        .collect()
}

/// 描边颜色：优先描边色，否则填充色，并乘上不透明度。
pub(crate) fn paint_color(style: &ElementStyle) -> Color {
    let mut color = style.stroke.or(style.fill).unwrap_or(Color::BLACK);
    color.a = (color.a * style.opacity.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    color
}

/// 有效线宽。非正数回落到 1。
pub(crate) fn effective_width(stroke_width: f32) -> f32 {
    if stroke_width > 0.0 {
        stroke_width
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(vertex: &Vertex) -> Vec2 {
        Vec2::new(vertex.position[0], vertex.position[1])
    }

    #[test]
    fn point_to_segment_distance_clamps_to_endpoints() {
        let a = Vec2::ZERO;
        let b = Vec2::new(10.0, 0.0);
        assert!((point_to_segment_distance(Vec2::new(5.0, 3.0), a, b) - 3.0).abs() < 1e-4);
        assert!((point_to_segment_distance(Vec2::new(-2.0, 0.0), a, b) - 2.0).abs() < 1e-4);
        assert!((point_to_segment_distance(Vec2::new(14.0, 0.0), a, b) - 4.0).abs() < 1e-4);
    }

    #[test]
    fn straight_segment_is_a_quad() {
        let points = [Vec2::ZERO, Vec2::new(100.0, 0.0)];
        let vertices = stroke_triangles(&points, 10.0, Color::RED);
        assert_eq!(vertices.len(), 6);
        assert!(vertices
            .iter()
            .all(|vertex| vertex.color == [1.0, 0.0, 0.0, 1.0]));
        assert!(vertices.iter().all(|vertex| {
            let p = pos(vertex);
            (0.0..=100.0).contains(&p.x) && (p.y.abs() - 5.0).abs() < 1e-3
        }));
    }

    #[test]
    fn duplicate_points_are_ignored() {
        let points = [Vec2::ZERO, Vec2::ZERO, Vec2::new(40.0, 0.0)];
        let vertices = stroke_triangles(&points, 4.0, Color::BLACK);
        assert_eq!(vertices.len(), 6);
    }

    #[test]
    fn right_angle_uses_miter_join() {
        let points = [Vec2::ZERO, Vec2::new(100.0, 0.0), Vec2::new(100.0, 100.0)];
        let vertices = stroke_triangles(&points, 20.0, Color::BLACK);
        // 起点、斜接拐角、终点 → 两个四边形
        assert_eq!(vertices.len(), 12);
        let outer = Vec2::new(90.0, 10.0);
        assert!(
            vertices
                .iter()
                .any(|vertex| (pos(vertex) - outer).length() < 0.05),
            "missing miter corner"
        );
        for vertex in &vertices {
            let distance = distance_to_polyline(pos(vertex), &points);
            assert!(
                distance <= 10.0 * MITER_LIMIT + 0.05,
                "vertex drifted {distance}"
            );
        }
    }

    #[test]
    fn sharp_corner_falls_back_to_bevel() {
        let points = [Vec2::ZERO, Vec2::new(100.0, 0.0), Vec2::new(0.0, 1.0)];
        let vertices = stroke_triangles(&points, 10.0, Color::BLACK);
        // 斜角在拐角处多插入一对点，三角形多于直角斜接
        assert!(vertices.len() > 12, "got {}", vertices.len());
        for vertex in &vertices {
            let distance = distance_to_polyline(pos(vertex), &points);
            assert!(distance <= 5.0 * MITER_LIMIT + 0.05);
        }
    }

    #[test]
    fn arrow_contains_tip_and_rejects_points_past_it() {
        let tri = arrow_points(Vec2::new(100.0, 0.0), Vec2::new(1.0, 0.0), 16.0, 12.0).unwrap();
        assert!(point_in_triangle(Vec2::new(100.0, 0.0), tri));
        assert!(point_in_triangle(Vec2::new(84.0, 5.5), tri));
        assert!(!point_in_triangle(Vec2::new(110.0, 0.0), tri));
    }

    #[test]
    fn shorten_end_keeps_a_stub() {
        let mut points = [Vec2::ZERO, Vec2::new(10.0, 0.0)];
        shorten_end(&mut points, 100.0);
        assert!(points[1].x > 0.0 && points[1].x < 10.0);
    }
}
