//! 背景网格
//!
//! 线落在世界坐标的整数倍上。平移和缩放只改变哪些线进入视口，
//! 不移动已有线的世界位置，所以网格和画布内容一起走。
//!
//! 基础间距是 20 个世界单位，和 M1.8 在 1× 下的 20px 一致，
//! 也和标尺在 1× 时的次刻度（主刻度 100 的五等分）重合。

use infinitecanvas_math::{Rect, Vec2, Vertex};

/// 1× 下的网格间距（世界单位）。
pub const GRID_SPACING: f32 = 20.0;

/// 暗色画布上的网格线。
pub const GRID_COLOR: [f32; 4] = [0.30, 0.33, 0.38, 1.0];

/// 原点轴，比普通网格线亮一点。
pub const GRID_AXIS_COLOR: [f32; 4] = [0.45, 0.58, 0.78, 1.0];

/// 缩小时相邻线至少占这么多屏幕像素，更密就按 5 倍加大间距。
const MIN_SCREEN_SPACING: f32 = 8.0;

/// 单轴最多生成这么多条线，避免异常视口。
const MAX_LINES_PER_AXIS: usize = 2048;

/// 一条世界空间线段。
///
/// 垂直线的 `start.x` 与 `end.x` 相同，水平线的 `y` 相同。
/// `on_axis` 为真表示这条线落在 x = 0 或 y = 0。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridLine {
    /// 起点（世界坐标）
    pub start: Vec2,
    /// 终点（世界坐标）
    pub end: Vec2,
    /// 是否是原点轴
    pub on_axis: bool,
}

/// 按缩放选择实际间距。
///
/// 1× 及更近时返回 [`GRID_SPACING`]。相邻线不足 8 个屏幕像素时，
/// 间距按 5 倍加大，新间距仍是 20 的整数倍，原来的线不会漂到别的世界坐标。
/// 非有限或非正的缩放按 1× 处理。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::visible_grid_spacing;
///
/// assert_eq!(visible_grid_spacing(1.0), 20.0);
/// assert_eq!(visible_grid_spacing(0.2), 100.0);
/// ```
pub fn visible_grid_spacing(zoom: f32) -> f32 {
    if !zoom.is_finite() || zoom <= 0.0 {
        return GRID_SPACING;
    }
    let mut step = GRID_SPACING;
    for _ in 0..8 {
        if step * zoom >= MIN_SCREEN_SPACING {
            break;
        }
        step *= 5.0;
    }
    step
}

/// 坐标是否落在间距的整数倍上。
///
/// 非有限坐标，或非正、非有限的间距，返回 false。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::on_grid_line;
///
/// assert!(on_grid_line(40.0, 20.0));
/// assert!(on_grid_line(-20.0, 20.0));
/// assert!(!on_grid_line(10.0, 20.0));
/// ```
pub fn on_grid_line(coord: f32, spacing: f32) -> bool {
    if !coord.is_finite() || !spacing.is_finite() || spacing <= 0.0 {
        return false;
    }
    let nearest = (coord / spacing).round() * spacing;
    (coord - nearest).abs() <= spacing * 1.0e-4
}

/// 覆盖 `bounds` 的网格线。端点拉到矩形边上，线的位置是间距的整数倍。
///
/// 间距非正、非有限，或矩形本身非有限时返回空。单轴超过 2048 条时把间距按 5 倍加大，
/// 加粗后的线仍是传入间距的整数倍。
///
/// # 参数
/// - `bounds`: 世界空间可见范围
/// - `spacing`: 世界单位间距。通常来自 [`visible_grid_spacing`]
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{grid_lines, GRID_SPACING};
/// use infinitecanvas_math::Rect;
///
/// let lines = grid_lines(Rect::new(0.0, 0.0, 40.0, 20.0), GRID_SPACING);
/// assert!(!lines.is_empty());
/// ```
pub fn grid_lines(bounds: Rect, spacing: f32) -> Vec<GridLine> {
    if !spacing.is_finite() || spacing <= 0.0 || !rect_is_finite(bounds) {
        return Vec::new();
    }
    let vertical = line_coords(bounds.left(), bounds.right(), spacing);
    let horizontal = line_coords(bounds.top(), bounds.bottom(), spacing);
    let mut lines = Vec::with_capacity(vertical.len() + horizontal.len());
    for x in vertical {
        lines.push(GridLine {
            start: Vec2::new(x, bounds.top()),
            end: Vec2::new(x, bounds.bottom()),
            on_axis: on_grid_line(x, spacing) && x.abs() <= spacing * 1.0e-4,
        });
    }
    for y in horizontal {
        lines.push(GridLine {
            start: Vec2::new(bounds.left(), y),
            end: Vec2::new(bounds.right(), y),
            on_axis: on_grid_line(y, spacing) && y.abs() <= spacing * 1.0e-4,
        });
    }
    lines
}

/// 网格线的线段顶点，供 WebGPU 的 `LineList` 使用。
///
/// 每条线两个顶点。原点轴用 [`GRID_AXIS_COLOR`]，其余用 [`GRID_COLOR`]。
///
/// # 参数
/// - `bounds`: 世界空间可见范围
/// - `spacing`: 世界单位间距
pub fn grid_vertices(bounds: Rect, spacing: f32) -> Vec<Vertex> {
    let lines = grid_lines(bounds, spacing);
    let mut vertices = Vec::with_capacity(lines.len() * 2);
    for line in lines {
        let color = if line.on_axis {
            GRID_AXIS_COLOR
        } else {
            GRID_COLOR
        };
        vertices.push(Vertex::from_arrays([line.start.x, line.start.y], color));
        vertices.push(Vertex::from_arrays([line.end.x, line.end.y], color));
    }
    vertices
}

fn rect_is_finite(bounds: Rect) -> bool {
    bounds.x.is_finite()
        && bounds.y.is_finite()
        && bounds.width.is_finite()
        && bounds.height.is_finite()
        && bounds.width >= 0.0
        && bounds.height >= 0.0
}

/// 把间距加大到单轴不超过 [`MAX_LINES_PER_AXIS`]。结果是 `spacing` 的 5 的幂次倍。
fn capped_spacing(min: f32, max: f32, spacing: f32) -> f32 {
    let span = (max - min).abs();
    if !span.is_finite() {
        return spacing;
    }
    let mut step = spacing;
    for _ in 0..8 {
        if span / step <= MAX_LINES_PER_AXIS as f32 {
            break;
        }
        step *= 5.0;
    }
    step
}

fn line_coords(min: f32, max: f32, spacing: f32) -> Vec<f32> {
    let spacing = capped_spacing(min, max, spacing);
    if !spacing.is_finite() || spacing <= 0.0 {
        return Vec::new();
    }
    let first = (min / spacing).ceil() * spacing;
    let last = (max / spacing).floor() * spacing;
    if !first.is_finite() || !last.is_finite() || last + spacing * 1.0e-4 < first {
        return Vec::new();
    }
    let count = ((last - first) / spacing).round() as i64 + 1;
    if count <= 0 {
        return Vec::new();
    }
    let count = count.min(MAX_LINES_PER_AXIS as i64) as usize;
    (0..count)
        .map(|index| first + index as f32 * spacing)
        .filter(|value| value.is_finite() && *value <= last + spacing * 1.0e-3)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Viewport;

    #[test]
    fn spacing_stays_at_twenty_until_lines_would_be_denser_than_eight_pixels() {
        assert_eq!(visible_grid_spacing(1.0), GRID_SPACING);
        assert_eq!(visible_grid_spacing(2.0), GRID_SPACING);
        assert_eq!(visible_grid_spacing(10.0), GRID_SPACING);
        assert_eq!(visible_grid_spacing(0.4), GRID_SPACING);
        assert_eq!(visible_grid_spacing(0.2), 100.0);
        assert_eq!(visible_grid_spacing(0.1), 100.0);
        assert_eq!(visible_grid_spacing(0.04), 500.0);
        assert_eq!(visible_grid_spacing(f32::NAN), GRID_SPACING);
        assert_eq!(visible_grid_spacing(0.0), GRID_SPACING);
        assert_eq!(visible_grid_spacing(-1.0), GRID_SPACING);
    }

    #[test]
    fn coarsened_spacing_still_lands_on_the_base_grid() {
        for zoom in [0.2_f32, 0.1, 0.04] {
            let spacing = visible_grid_spacing(zoom);
            assert!(on_grid_line(spacing, GRID_SPACING));
            assert!(on_grid_line(0.0, spacing));
            assert!(on_grid_line(-spacing, spacing));
        }
        assert!(!on_grid_line(20.0, visible_grid_spacing(0.2)));
        assert!(on_grid_line(20.0, visible_grid_spacing(1.0)));
    }

    #[test]
    fn lines_cover_the_bounds_and_sit_on_multiples_of_the_spacing() {
        let bounds = Rect::new(-15.0, 5.0, 50.0, 30.0);
        let lines = grid_lines(bounds, GRID_SPACING);
        let vertical: Vec<f32> = lines
            .iter()
            .filter(|line| (line.start.x - line.end.x).abs() < 1.0e-4)
            .map(|line| line.start.x)
            .collect();
        let horizontal: Vec<f32> = lines
            .iter()
            .filter(|line| (line.start.y - line.end.y).abs() < 1.0e-4)
            .map(|line| line.start.y)
            .collect();
        // -15..=35 只盖住 0 和 20；-20 在矩形外面。5..=35 只有 y = 20。
        assert_eq!(vertical.len(), 2);
        assert!(vertical.iter().any(|x| x.abs() < 1.0e-3));
        assert!(vertical.iter().any(|x| (*x - 20.0).abs() < 1.0e-3));
        assert_eq!(horizontal.len(), 1);
        assert!((horizontal[0] - 20.0).abs() < 1.0e-3);
        assert!(vertical.iter().all(|x| on_grid_line(*x, GRID_SPACING)));
        assert!(horizontal.iter().all(|y| on_grid_line(*y, GRID_SPACING)));
        assert!(vertical
            .iter()
            .all(|x| *x >= bounds.left() - 1.0e-3 && *x <= bounds.right() + 1.0e-3));
        let axis = lines.iter().filter(|line| line.on_axis).count();
        assert_eq!(axis, 1);
    }

    #[test]
    fn pan_keeps_world_positions_and_moves_the_origin_on_screen() {
        let mut viewport = Viewport::new(Vec2::new(200.0, 100.0));
        let before = grid_lines(viewport.bounds(), GRID_SPACING);
        assert!(before
            .iter()
            .any(|line| line.on_axis && line.start.x.abs() < 1.0e-3));
        let origin_before = viewport.world_to_screen(Vec2::ZERO);

        viewport.translate(Vec2::new(10.0, 0.0));
        let after = grid_lines(viewport.bounds(), GRID_SPACING);
        let vertical: Vec<f32> = after
            .iter()
            .filter(|line| (line.start.x - line.end.x).abs() < 1.0e-4)
            .map(|line| line.start.x)
            .collect();
        assert!(vertical.iter().any(|x| x.abs() < 1.0e-3));
        assert!(vertical.iter().all(|x| on_grid_line(*x, GRID_SPACING)));
        let origin_after = viewport.world_to_screen(Vec2::ZERO);
        assert!((origin_after.x - (origin_before.x - 10.0)).abs() < 1.0e-3);
    }

    #[test]
    fn zoom_does_not_move_lines_off_the_base_grid() {
        let mut viewport = Viewport::new(Vec2::new(400.0, 300.0));
        let wide = grid_lines(viewport.bounds(), visible_grid_spacing(viewport.zoom()));
        viewport.set_zoom(2.0);
        let tight = grid_lines(viewport.bounds(), visible_grid_spacing(viewport.zoom()));
        assert!(tight.iter().any(|line| line.on_axis));
        for line in tight.iter().chain(wide.iter()) {
            let vertical = (line.start.x - line.end.x).abs() < 1.0e-4;
            let coord = if vertical { line.start.x } else { line.start.y };
            assert!(on_grid_line(coord, GRID_SPACING));
        }
        let tight_xs: Vec<f32> = tight
            .iter()
            .filter(|line| (line.start.x - line.end.x).abs() < 1.0e-4)
            .map(|line| line.start.x)
            .collect();
        let wide_xs: Vec<f32> = wide
            .iter()
            .filter(|line| (line.start.x - line.end.x).abs() < 1.0e-4)
            .map(|line| line.start.x)
            .collect();
        assert!(tight_xs
            .iter()
            .all(|x| wide_xs.iter().any(|other| (x - other).abs() < 1.0e-3)));
    }

    #[test]
    fn invalid_input_yields_no_lines_and_vertices_come_in_pairs() {
        let bounds = Rect::new(0.0, 0.0, 40.0, 40.0);
        assert!(grid_lines(bounds, 0.0).is_empty());
        assert!(grid_lines(bounds, -5.0).is_empty());
        assert!(grid_lines(bounds, f32::NAN).is_empty());
        assert!(grid_lines(Rect::new(f32::NAN, 0.0, 10.0, 10.0), GRID_SPACING).is_empty());
        assert!(!on_grid_line(f32::NAN, GRID_SPACING));
        assert!(!on_grid_line(0.0, 0.0));
        let vertices = grid_vertices(bounds, GRID_SPACING);
        assert_eq!(vertices.len() % 2, 0);
        assert!(vertices.len() >= 4);
        assert_eq!(vertices[0].color, GRID_AXIS_COLOR);
    }

    #[test]
    fn a_huge_span_does_not_emit_more_lines_than_the_cap() {
        let bounds = Rect::new(-1.0e7, -1.0e7, 2.0e7, 2.0e7);
        let lines = grid_lines(bounds, GRID_SPACING);
        let vertical = lines
            .iter()
            .filter(|line| (line.start.x - line.end.x).abs() < 1.0e-3)
            .count();
        let horizontal = lines.len() - vertical;
        assert!(vertical <= MAX_LINES_PER_AXIS);
        assert!(horizontal <= MAX_LINES_PER_AXIS);
        assert!(lines.iter().all(|line| {
            let vertical = (line.start.x - line.end.x).abs() < 1.0e-2;
            let coord = if vertical { line.start.x } else { line.start.y };
            on_grid_line(coord, GRID_SPACING)
        }));
    }
}
