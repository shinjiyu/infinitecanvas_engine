//! 折线描边几何
//!
//! 沿采样点生成带宽度的三角形带（展开为三角形列表），并处理拐角。
//! 斜接（miter）过长时退化为斜角（bevel），避免尖刺。
//! 实线 + 平头 + 斜接走原来的三角形带。虚线、圆头/方头、圆角/斜角另生成
//! 线段四边形、端点帽和外侧拐角，矩形和圆会把重合的首尾当成闭合环。

use super::{ElementStyle, StrokeCap, StrokeJoin};
use infinitecanvas_math::{Color, Rect, Transform, Vec2, Vertex};
use std::f32::consts::PI;

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

/// 一次描边的线宽、颜色和线型。
///
/// `dash` 为空（或全是 0）时是实线。奇数长度的图案按 SVG 规则重复一遍再画。
pub(crate) struct StrokePaint<'a> {
    /// 线宽。小于等于 0 时不生成顶点。
    pub width: f32,
    /// 顶点颜色，调用方已经乘过不透明度。
    pub color: Color,
    /// 虚线图案。空切片是实线。
    pub dash: &'a [f32],
    /// 开口端和每一段虚线两端的线帽。
    pub cap: StrokeCap,
    /// 拐角。闭合环的接缝也用它。
    pub join: StrokeJoin,
}

/// 按线型生成描边三角形。
///
/// 实线、平头、斜接（闭合环的线帽本来就看不见）仍走 [`stroke_triangles`]，
/// 顶点和原来的实线描边一致。其它组合按虚线段、线帽和线结重新铺三角形。
/// 首尾重合且至少有 3 个不同顶点时当成闭合环：实线不画线帽，虚线的每一段仍有线帽。
pub(crate) fn stroke_styled(points: &[Vec2], paint: StrokePaint<'_>) -> Vec<Vertex> {
    if paint.width <= 0.0 || !paint.width.is_finite() {
        return Vec::new();
    }
    let pattern = normalize_dash(paint.dash);
    let looped = is_closed_loop(points);
    if pattern.is_empty()
        && paint.join == StrokeJoin::Miter
        && (paint.cap == StrokeCap::Butt || looped)
    {
        return stroke_triangles(points, paint.width, paint.color);
    }
    let mut vertices = Vec::new();
    if pattern.is_empty() {
        append_stroke(
            &mut vertices,
            points,
            paint.width,
            paint.color,
            paint.cap,
            paint.join,
        );
    } else {
        for piece in dash_pieces(points, &pattern) {
            stroke_prepared(
                &mut vertices,
                &piece.points,
                piece.closed,
                paint.width,
                paint.color,
                paint.cap,
                paint.join,
            );
        }
    }
    vertices
}

/// 中心线线段列表。实线返回整段折线；虚线只保留图案里的“开”区间。
///
/// 矩形和圆的 1px 描边走这里，避免粗描边的间隙被一条实心细线填上。
pub(crate) fn stroke_centerline(points: &[Vec2], color: Color, dash: &[f32]) -> Vec<Vertex> {
    let pattern = normalize_dash(dash);
    let polylines = if pattern.is_empty() {
        vec![centerline_points(points)]
    } else {
        dash_pieces(points, &pattern)
            .into_iter()
            .map(|piece| {
                let mut pts = piece.points;
                if piece.closed {
                    if let Some(first) = pts.first().copied() {
                        pts.push(first);
                    }
                }
                pts
            })
            .collect()
    };
    let mut vertices = Vec::new();
    for pts in polylines {
        for window in pts.windows(2) {
            if (window[1] - window[0]).length_squared() <= 1e-12 {
                continue;
            }
            vertices.push(Vertex::new(window[0].x, window[0].y, color));
            vertices.push(Vertex::new(window[1].x, window[1].y, color));
        }
    }
    vertices
}

/// 图案是否应当画成实线。
pub(crate) fn dash_is_solid(dash: &[f32]) -> bool {
    normalize_dash(dash).is_empty()
}

struct StrokePiece {
    points: Vec<Vec2>,
    closed: bool,
}

struct Seg {
    a: Vec2,
    b: Vec2,
    len: f32,
    start: f32,
}

fn normalize_dash(dash: &[f32]) -> Vec<f32> {
    let mut values: Vec<f32> = dash
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .map(|value| value.max(0.0))
        .collect();
    if values.iter().all(|value| *value <= 1e-6) {
        return Vec::new();
    }
    if values.len() % 2 == 1 {
        let copy = values.clone();
        values.extend(copy);
    }
    values
}

fn is_closed_loop(points: &[Vec2]) -> bool {
    let points = dedup_points(points);
    points.len() >= 4
        && points
            .first()
            .zip(points.last())
            .is_some_and(|(start, end)| (*start - *end).length_squared() <= 1e-8)
}

/// 去掉闭合环重复的终点。返回的点列首尾不再重合。
fn open_or_loop(points: &[Vec2]) -> (Vec<Vec2>, bool) {
    let mut pts = dedup_points(points);
    let closed = pts.len() >= 4
        && pts
            .first()
            .zip(pts.last())
            .is_some_and(|(start, end)| (*start - *end).length_squared() <= 1e-8);
    if closed {
        pts.pop();
    }
    (pts, closed)
}

fn centerline_points(points: &[Vec2]) -> Vec<Vec2> {
    let (mut pts, closed) = open_or_loop(points);
    if closed {
        if let Some(first) = pts.first().copied() {
            pts.push(first);
        }
    }
    pts
}

fn append_stroke(
    vertices: &mut Vec<Vertex>,
    points: &[Vec2],
    width: f32,
    color: Color,
    cap: StrokeCap,
    join: StrokeJoin,
) {
    let (pts, closed) = open_or_loop(points);
    stroke_prepared(vertices, &pts, closed, width, color, cap, join);
}

fn stroke_prepared(
    vertices: &mut Vec<Vertex>,
    points: &[Vec2],
    closed: bool,
    width: f32,
    color: Color,
    cap: StrokeCap,
    join: StrokeJoin,
) {
    if width <= 0.0 || points.len() < 2 || (closed && points.len() < 3) {
        return;
    }
    let half = width * 0.5;
    let mut pts = points.to_vec();
    if !closed && cap == StrokeCap::Square {
        extend_square(&mut pts, half);
    }
    let count = pts.len();
    if closed {
        for index in 0..count {
            push_segment(vertices, pts[index], pts[(index + 1) % count], half, color);
        }
        for index in 0..count {
            let prev = pts[(index + count - 1) % count];
            let curr = pts[index];
            let next = pts[(index + 1) % count];
            if let (Some(dir_in), Some(dir_out)) = (safe_dir(prev, curr), safe_dir(curr, next)) {
                push_join_wedge(vertices, curr, dir_in, dir_out, half, join, color);
            }
        }
        return;
    }
    for window in pts.windows(2) {
        push_segment(vertices, window[0], window[1], half, color);
    }
    for index in 1..count.saturating_sub(1) {
        if let (Some(dir_in), Some(dir_out)) = (
            safe_dir(pts[index - 1], pts[index]),
            safe_dir(pts[index], pts[index + 1]),
        ) {
            push_join_wedge(vertices, pts[index], dir_in, dir_out, half, join, color);
        }
    }
    if cap == StrokeCap::Round {
        if let Some(dir) = safe_dir(pts[0], pts[1]) {
            push_round_cap(vertices, pts[0], -dir, half, color);
        }
        if let Some(dir) = safe_dir(pts[count - 2], pts[count - 1]) {
            push_round_cap(vertices, pts[count - 1], dir, half, color);
        }
    }
}

fn extend_square(points: &mut [Vec2], half: f32) {
    if points.len() < 2 || half <= 0.0 {
        return;
    }
    if let Some(dir) = safe_dir(points[0], points[1]) {
        let len = (points[1] - points[0]).length();
        points[0] -= dir * half.min(len * 0.5);
    }
    let last = points.len() - 1;
    if let Some(dir) = safe_dir(points[last - 1], points[last]) {
        let len = (points[last] - points[last - 1]).length();
        points[last] += dir * half.min(len * 0.5);
    }
}

fn push_segment(vertices: &mut Vec<Vertex>, a: Vec2, b: Vec2, half: f32, color: Color) {
    let Some(dir) = safe_dir(a, b) else {
        return;
    };
    let normal = left_normal(dir) * half;
    let a_left = a + normal;
    let a_right = a - normal;
    let b_left = b + normal;
    let b_right = b - normal;
    push_tri(vertices, a_left, a_right, b_left, color);
    push_tri(vertices, b_left, a_right, b_right, color);
}

fn push_join_wedge(
    vertices: &mut Vec<Vertex>,
    curr: Vec2,
    dir_in: Vec2,
    dir_out: Vec2,
    half: f32,
    join: StrokeJoin,
    color: Color,
) {
    let cross = dir_in.x * dir_out.y - dir_in.y * dir_out.x;
    if cross.abs() < 1e-4 {
        return;
    }
    let n0 = left_normal(dir_in);
    let n1 = left_normal(dir_out);
    let left_turn = cross > 0.0;
    let outer_in = if left_turn { -n0 } else { n0 };
    let outer_out = if left_turn { -n1 } else { n1 };
    match join {
        StrokeJoin::Bevel => {
            push_tri(
                vertices,
                curr,
                curr + outer_in * half,
                curr + outer_out * half,
                color,
            );
        }
        StrokeJoin::Round => push_arc(vertices, curr, outer_in, outer_out, half, color),
        StrokeJoin::Miter => {
            if let Some(miter) = miter_outer(curr, n0, n1, half, left_turn) {
                push_tri(vertices, curr, curr + outer_in * half, miter, color);
                push_tri(vertices, curr, miter, curr + outer_out * half, color);
            } else {
                push_tri(
                    vertices,
                    curr,
                    curr + outer_in * half,
                    curr + outer_out * half,
                    color,
                );
            }
        }
    }
}

fn miter_outer(curr: Vec2, n0: Vec2, n1: Vec2, half: f32, left_turn: bool) -> Option<Vec2> {
    let sum = n0 + n1;
    let sum_len = sum.length();
    if sum_len < 1e-5 {
        return None;
    }
    let miter = sum / sum_len;
    let denom = miter.dot(n0);
    if denom.abs() < 1.0 / MITER_LIMIT {
        return None;
    }
    let offset = miter * (half / denom);
    Some(if left_turn {
        curr - offset
    } else {
        curr + offset
    })
}

fn push_arc(
    vertices: &mut Vec<Vertex>,
    center: Vec2,
    from_unit: Vec2,
    to_unit: Vec2,
    radius: f32,
    color: Color,
) {
    let start = from_unit.y.atan2(from_unit.x);
    let sweep = wrap_pi(to_unit.y.atan2(to_unit.x) - start);
    let steps = ((sweep.abs() / (PI / 8.0)).ceil() as usize).clamp(1, 32);
    let mut prev = center + from_unit * radius;
    for step in 1..=steps {
        let angle = start + sweep * (step as f32 / steps as f32);
        let next = center + Vec2::new(angle.cos(), angle.sin()) * radius;
        push_tri(vertices, center, prev, next, color);
        prev = next;
    }
}

fn push_round_cap(
    vertices: &mut Vec<Vertex>,
    center: Vec2,
    outward: Vec2,
    half: f32,
    color: Color,
) {
    let right = -left_normal(outward);
    let start = right.y.atan2(right.x);
    let outward_angle = outward.y.atan2(outward.x);
    let mut sweep = PI;
    if wrap_pi(outward_angle - (start + PI * 0.5)).abs() > 0.5 {
        sweep = -PI;
    }
    let steps = 8;
    let mut prev = center + right * half;
    for step in 1..=steps {
        let angle = start + sweep * (step as f32 / steps as f32);
        let next = center + Vec2::new(angle.cos(), angle.sin()) * half;
        push_tri(vertices, center, prev, next, color);
        prev = next;
    }
}

fn wrap_pi(mut angle: f32) -> f32 {
    while angle > PI {
        angle -= 2.0 * PI;
    }
    while angle < -PI {
        angle += 2.0 * PI;
    }
    angle
}

fn dash_pieces(points: &[Vec2], pattern: &[f32]) -> Vec<StrokePiece> {
    let (pts, closed) = open_or_loop(points);
    if pts.len() < 2 || (closed && pts.len() < 3) || pattern.is_empty() {
        return Vec::new();
    }
    let segs = build_segs(&pts, closed);
    let total = segs.last().map(|seg| seg.start + seg.len).unwrap_or(0.0);
    if total <= 1e-4 {
        return Vec::new();
    }
    let mut pieces = Vec::new();
    let mut cursor = 0.0;
    let mut index = 0;
    let mut draw = true;
    let mut guard = 0;
    while cursor < total - 1e-4 && guard < 100_000 {
        guard += 1;
        let span = pattern[index];
        let drawing = draw;
        index = (index + 1) % pattern.len();
        draw = !draw;
        if span <= 1e-6 {
            continue;
        }
        if drawing && cursor <= 1e-4 && span >= total - 1e-4 && closed {
            return vec![StrokePiece {
                points: pts,
                closed: true,
            }];
        }
        let end = (cursor + span).min(total);
        if drawing {
            let piece = slice_segs(&segs, cursor, end);
            if piece.len() >= 2 {
                pieces.push(StrokePiece {
                    points: piece,
                    closed: false,
                });
            }
        }
        cursor = end;
    }
    pieces
}

fn build_segs(points: &[Vec2], closed: bool) -> Vec<Seg> {
    let mut segs = Vec::new();
    let mut dist = 0.0;
    let count = if closed {
        points.len()
    } else {
        points.len().saturating_sub(1)
    };
    for index in 0..count {
        let a = points[index];
        let b = if closed {
            points[(index + 1) % points.len()]
        } else {
            points[index + 1]
        };
        let len = (b - a).length();
        if len > 1e-6 {
            segs.push(Seg {
                a,
                b,
                len,
                start: dist,
            });
            dist += len;
        }
    }
    segs
}

fn slice_segs(segs: &[Seg], start: f32, end: f32) -> Vec<Vec2> {
    let mut out = Vec::new();
    for seg in segs {
        let seg_end = seg.start + seg.len;
        if seg_end <= start + 1e-5 || seg.start >= end - 1e-5 {
            continue;
        }
        let from = start.max(seg.start);
        let to = end.min(seg_end);
        let p0 = point_on_seg(seg, from);
        let p1 = point_on_seg(seg, to);
        if out
            .last()
            .map(|prev: &Vec2| (*prev - p0).length_squared() > 1e-10)
            .unwrap_or(true)
        {
            out.push(p0);
        }
        if (p1 - p0).length_squared() > 1e-10 {
            out.push(p1);
        }
    }
    out
}

fn point_on_seg(seg: &Seg, dist: f32) -> Vec2 {
    let t = ((dist - seg.start) / seg.len).clamp(0.0, 1.0);
    seg.a + (seg.b - seg.a) * t
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
    color_with_opacity(
        style.stroke.or(style.fill).unwrap_or(Color::BLACK),
        style.opacity,
    )
}

/// 把元素不透明度乘进颜色 alpha。
pub(crate) fn color_with_opacity(mut color: Color, opacity: f32) -> Color {
    color.a = (color.a * opacity.clamp(0.0, 1.0)).clamp(0.0, 1.0);
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
    use super::super::{StrokeCap, StrokeJoin};
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

    fn covers(vertices: &[Vertex], point: Vec2) -> bool {
        vertices.chunks_exact(3).any(|tri| {
            point_in_triangle(
                point,
                [
                    Vec2::new(tri[0].position[0], tri[0].position[1]),
                    Vec2::new(tri[1].position[0], tri[1].position[1]),
                    Vec2::new(tri[2].position[0], tri[2].position[1]),
                ],
            )
        })
    }

    fn paint<'a>(width: f32, dash: &'a [f32], cap: StrokeCap, join: StrokeJoin) -> StrokePaint<'a> {
        StrokePaint {
            width,
            color: Color::BLACK,
            dash,
            cap,
            join,
        }
    }

    #[test]
    fn dash_leaves_gaps_and_keeps_the_on_segments() {
        let points = [Vec2::ZERO, Vec2::new(100.0, 0.0)];
        let vertices = stroke_styled(
            &points,
            paint(8.0, &[20.0, 20.0], StrokeCap::Butt, StrokeJoin::Miter),
        );
        assert!(covers(&vertices, Vec2::new(10.0, 0.0)));
        assert!(!covers(&vertices, Vec2::new(30.0, 0.0)));
        assert!(covers(&vertices, Vec2::new(50.0, 0.0)));
        assert!(vertices.iter().all(|vertex| vertex.color[3] == 1.0));
    }

    #[test]
    fn round_and_square_caps_extend_past_butt_end() {
        let points = [Vec2::ZERO, Vec2::new(100.0, 0.0)];
        let butt = stroke_styled(
            &points,
            paint(10.0, &[], StrokeCap::Butt, StrokeJoin::Miter),
        );
        let round = stroke_styled(
            &points,
            paint(10.0, &[], StrokeCap::Round, StrokeJoin::Miter),
        );
        let square = stroke_styled(
            &points,
            paint(10.0, &[], StrokeCap::Square, StrokeJoin::Miter),
        );
        let past = Vec2::new(-2.0, 0.0);
        let too_far = Vec2::new(-6.0, 0.0);
        assert!(!covers(&butt, past));
        assert!(covers(&round, past));
        assert!(covers(&square, past));
        assert!(!covers(&round, too_far));
        assert!(!covers(&square, too_far));
        assert!(covers(&round, Vec2::new(102.0, 0.0)));
    }

    #[test]
    fn bevel_cuts_the_miter_and_round_follows_the_outer_arc() {
        let points = [Vec2::ZERO, Vec2::new(100.0, 0.0), Vec2::new(100.0, 100.0)];
        let miter_point = Vec2::new(110.0, -10.0);
        let arc_point = Vec2::new(106.3, -6.3);
        let bevel = stroke_styled(
            &points,
            paint(20.0, &[], StrokeCap::Butt, StrokeJoin::Bevel),
        );
        let round = stroke_styled(
            &points,
            paint(20.0, &[], StrokeCap::Butt, StrokeJoin::Round),
        );
        let miter = stroke_styled(
            &points,
            paint(20.0, &[], StrokeCap::Round, StrokeJoin::Miter),
        );
        assert!(!covers(&bevel, miter_point));
        assert!(covers(&bevel, Vec2::new(103.0, -3.0)));
        assert!(covers(&round, arc_point));
        assert!(!covers(&bevel, arc_point));
        assert!(covers(&miter, miter_point));
    }

    #[test]
    fn odd_length_dash_repeats_like_svg() {
        let points = [Vec2::ZERO, Vec2::new(40.0, 0.0)];
        let vertices = stroke_styled(
            &points,
            paint(4.0, &[10.0], StrokeCap::Butt, StrokeJoin::Miter),
        );
        assert!(covers(&vertices, Vec2::new(5.0, 0.0)));
        assert!(!covers(&vertices, Vec2::new(15.0, 0.0)));
        assert!(covers(&vertices, Vec2::new(25.0, 0.0)));
    }

    #[test]
    fn closed_loop_dash_breaks_one_edge() {
        let points = [
            Vec2::new(-50.0, -10.0),
            Vec2::new(50.0, -10.0),
            Vec2::new(50.0, 10.0),
            Vec2::new(-50.0, 10.0),
            Vec2::new(-50.0, -10.0),
        ];
        let vertices = stroke_styled(
            &points,
            paint(4.0, &[20.0, 20.0], StrokeCap::Butt, StrokeJoin::Miter),
        );
        assert!(covers(&vertices, Vec2::new(-40.0, -10.0)));
        assert!(!covers(&vertices, Vec2::new(-20.0, -10.0)));
    }
}
