//! 标尺与手动参考线。
//!
//! 顶边标尺的刻度是世界 x，从这里向下拖出水平参考线（固定世界 y）。
//! 左边标尺的刻度是世界 y，从这里向右拖出垂直参考线（固定世界 x）。
//! 参考线存在场景里，和元素一起进出 JSON。拖拽吸附把它们和元素边缘放在同一次比较里。
//!
//! ```text
//! 顶标尺 ──拖出──▶ 水平参考线 y = constant
//! 左标尺 ──拖出──▶ 垂直参考线 x = constant
//!                      │
//!                      ▼
//!              snap_drag_with_guides（与元素边缘/中心比距离）
//! ```
//!
//! # 示例
//! ```
//! use infinitecanvas_engine::guides::GuideStore;
//! use infinitecanvas_engine::snap::GuideAxis;
//!
//! let mut guides = GuideStore::default();
//! let guide = guides.add(GuideAxis::Vertical, 40.0).unwrap();
//! assert_eq!(guides.hit_test(infinitecanvas_math::Vec2::new(42.0, 0.0), 3.0), Some(guide.id));
//! ```

use crate::math::{Rect, Vec2};
use crate::snap::{GuideAxis, GuideLines};
use crate::transform_handles::ResizeHandle;
use crate::viewport::Viewport;
use serde::{Deserialize, Serialize};

/// 标尺厚度（屏幕像素）。顶边和左边各占这一条。
pub const RULER_THICKNESS_PX: f32 = 24.0;

/// 点选参考线的屏幕像素半径。和缩放无关，世界距离要再除以 zoom。
pub const GUIDE_HIT_PX: f32 = 5.0;

/// 标尺主刻度希望占据的屏幕像素。实际步长会收成 1/2/5×10^n。
const RULER_TARGET_PX: f32 = 80.0;

/// 边和世界轴夹角小于该值（弧度）时，才把这条边吸到参考线上。
const AXIS_ALIGN_EPSILON: f32 = 0.02;

/// 一条手动参考线。
///
/// `position` 是世界坐标：水平线用 y，垂直线用 x。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ManualGuide {
    /// 场景内稳定 id。和元素 id 各算各的。
    pub id: u64,
    /// 水平线或垂直线。
    pub axis: GuideAxis,
    /// 世界坐标。
    pub position: f32,
}

/// 参考线集合的可撤销快照。
#[derive(Debug, Clone, PartialEq)]
pub struct GuideSnapshot {
    guides: Vec<ManualGuide>,
    next_id: u64,
}

/// 场景里的手动参考线。
///
/// id 从 1 递增。删除不回收 id，避免撤销后和新线撞号。
#[derive(Debug, Clone, PartialEq)]
pub struct GuideStore {
    guides: Vec<ManualGuide>,
    next_id: u64,
}

impl Default for GuideStore {
    fn default() -> Self {
        Self {
            guides: Vec::new(),
            next_id: 1,
        }
    }
}

impl GuideStore {
    /// 从 JSON 记录恢复。重复 id 或非有限坐标返回错误。
    ///
    /// 结果按 id 升序，`next_id` 至少是 `max(id) + 1`。
    pub fn from_records(records: Vec<ManualGuide>) -> Result<Self, String> {
        let mut guides = Vec::with_capacity(records.len());
        let mut seen = std::collections::HashSet::with_capacity(records.len());
        let mut max_id = 0u64;
        for guide in records {
            if !guide.position.is_finite() {
                return Err(format!("guide {} position is not finite", guide.id));
            }
            if !seen.insert(guide.id) {
                return Err(format!("duplicate guide id {}", guide.id));
            }
            max_id = max_id.max(guide.id);
            guides.push(guide);
        }
        guides.sort_by_key(|guide| guide.id);
        Ok(Self {
            guides,
            next_id: max_id.saturating_add(1).max(1),
        })
    }

    /// 写出 JSON 用的记录，按 id 升序。
    pub fn records(&self) -> Vec<ManualGuide> {
        let mut guides = self.guides.clone();
        guides.sort_by_key(|guide| guide.id);
        guides
    }

    /// 当前快照。撤销命令拿它做 before / after。
    pub fn snapshot(&self) -> GuideSnapshot {
        GuideSnapshot {
            guides: self.guides.clone(),
            next_id: self.next_id,
        }
    }

    /// 用快照整表替换。
    pub fn restore(&mut self, snapshot: &GuideSnapshot) {
        self.guides = snapshot.guides.clone();
        self.next_id = snapshot.next_id;
    }

    /// 新增一条参考线。坐标不是有限数时返回 `None`。
    pub fn add(&mut self, axis: GuideAxis, position: f32) -> Option<ManualGuide> {
        if !position.is_finite() {
            return None;
        }
        let guide = ManualGuide {
            id: self.next_id,
            axis,
            position,
        };
        self.next_id = self.next_id.saturating_add(1);
        self.guides.push(guide);
        Some(guide)
    }

    /// 删除一条参考线。
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.guides.len();
        self.guides.retain(|guide| guide.id != id);
        self.guides.len() != before
    }

    /// 改位置。id 不存在或坐标非法时返回 `false`。
    pub fn move_to(&mut self, id: u64, position: f32) -> bool {
        if !position.is_finite() {
            return false;
        }
        let Some(guide) = self.guides.iter_mut().find(|guide| guide.id == id) else {
            return false;
        };
        guide.position = position;
        true
    }

    /// 按 id 查找。
    pub fn get(&self, id: u64) -> Option<&ManualGuide> {
        self.guides.iter().find(|guide| guide.id == id)
    }

    /// 按 id 升序遍历。存储顺序本身就是递增的。
    pub fn iter(&self) -> impl Iterator<Item = &ManualGuide> {
        self.guides.iter()
    }

    /// 参考线数量。
    pub fn len(&self) -> usize {
        self.guides.len()
    }

    /// 没有任何参考线。
    pub fn is_empty(&self) -> bool {
        self.guides.is_empty()
    }

    /// 离 `world` 最近、且距离不超过 `threshold`（世界单位）的参考线。
    ///
    /// 水平线比 y，垂直线比 x。距离相同取更小的 id。
    pub fn hit_test(&self, world: Vec2, threshold: f32) -> Option<u64> {
        if !world.x.is_finite() || !world.y.is_finite() || !threshold.is_finite() || threshold < 0.0
        {
            return None;
        }
        let mut best: Option<(u64, f32)> = None;
        for guide in &self.guides {
            let delta = match guide.axis {
                GuideAxis::Horizontal => (world.y - guide.position).abs(),
                GuideAxis::Vertical => (world.x - guide.position).abs(),
            };
            if !delta.is_finite() || delta > threshold {
                continue;
            }
            let replace = match best {
                Some((id, current)) => {
                    delta < current || ((delta - current).abs() <= 1e-4 && guide.id < id)
                }
                None => true,
            };
            if replace {
                best = Some((guide.id, delta));
            }
        }
        best.map(|(id, _)| id)
    }

    /// 给吸附用的两轴坐标。空集合不会改变原来的元素对元素吸附。
    pub fn lines(&self) -> GuideLines {
        let mut lines = GuideLines::default();
        for guide in &self.guides {
            match guide.axis {
                GuideAxis::Horizontal => lines.horizontal.push(guide.position),
                GuideAxis::Vertical => lines.vertical.push(guide.position),
            }
        }
        lines
    }
}

/// 指针落在标尺的哪一段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulerBand {
    /// 左上角，两条标尺重叠，不从这里拖线。
    Corner,
    /// 顶边。拖出水平参考线。
    Top,
    /// 左边。拖出垂直参考线。
    Left,
}

/// 屏幕点落在哪条标尺上。
///
/// 厚度外、或坐标非法时返回 `None`。原点在画布左上角，Y 向下。
///
/// # 参数
/// - `screen`: 画布像素
/// - `thickness`: 标尺厚度（像素）
pub fn ruler_band(screen: Vec2, thickness: f32) -> Option<RulerBand> {
    if !screen.x.is_finite() || !screen.y.is_finite() || !thickness.is_finite() || thickness <= 0.0
    {
        return None;
    }
    let on_left = screen.x >= 0.0 && screen.x < thickness;
    let on_top = screen.y >= 0.0 && screen.y < thickness;
    match (on_left, on_top) {
        (true, true) => Some(RulerBand::Corner),
        (false, true) => Some(RulerBand::Top),
        (true, false) => Some(RulerBand::Left),
        (false, false) => None,
    }
}

/// 标尺朝向。决定刻度沿哪条世界轴。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulerOrientation {
    /// 顶边标尺，刻度是世界 x。
    Horizontal,
    /// 左边标尺，刻度是世界 y。
    Vertical,
}

/// 一个刻度。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RulerTick {
    /// 世界坐标。
    pub world: f32,
    /// 对应的屏幕像素（沿标尺方向）。
    pub screen: f32,
    /// 主刻度才写数字。
    pub major: bool,
}

/// 主刻度的世界步长。缩放越大，步长越小，屏幕上仍大约 [`RULER_TARGET_PX`] 像素一格。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::guides::ruler_major_step;
///
/// assert_eq!(ruler_major_step(1.0), 100.0);
/// assert!(ruler_major_step(4.0) < ruler_major_step(1.0));
/// ```
pub fn ruler_major_step(zoom: f32) -> f32 {
    let zoom = if zoom.is_finite() && zoom > f32::EPSILON {
        zoom
    } else {
        1.0
    };
    nice_step(RULER_TARGET_PX / zoom)
}

/// 当前视口里要画的刻度，含主刻度和 5 等分的次刻度。
///
/// 平移或缩放后世界范围变了，刻度跟着变。步长见 [`ruler_major_step`]。
pub fn ruler_ticks(viewport: &Viewport, orientation: RulerOrientation) -> Vec<RulerTick> {
    let size = viewport.canvas_size();
    let along_x = matches!(orientation, RulerOrientation::Horizontal);
    let length = if along_x { size.x } else { size.y };
    if !length.is_finite() || length <= 0.0 {
        return Vec::new();
    }
    let major = ruler_major_step(viewport.zoom());
    let minor = major / 5.0;
    if !minor.is_finite() || minor <= 0.0 {
        return Vec::new();
    }
    let start = viewport.screen_to_world(Vec2::ZERO);
    let end = viewport.screen_to_world(size);
    let (world_start, world_end) = if along_x {
        (start.x.min(end.x), start.x.max(end.x))
    } else {
        (start.y.min(end.y), start.y.max(end.y))
    };
    let mut world = (world_start / minor).floor() * minor;
    let mut ticks = Vec::new();
    let mut guard = 0;
    while world <= world_end + minor * 0.5 && guard < 2000 {
        guard += 1;
        let screen = if along_x {
            viewport.world_to_screen(Vec2::new(world, 0.0)).x
        } else {
            viewport.world_to_screen(Vec2::new(0.0, world)).y
        };
        if screen.is_finite() && screen >= -1.0 && screen <= length + 1.0 {
            ticks.push(RulerTick {
                world,
                screen,
                major: is_major_tick(world, major),
            });
        }
        world += minor;
    }
    ticks
}

/// 缩放时，把正在移动的元素边吸到最近参考线，返回应加到指针上的修正。
///
/// `edge` 是未吸附预览里这条边的世界坐标。`slope` 是指针沿该世界轴移动 1 个单位时，
/// 这条边移动多少。斜率接近 0、或没有进入阈值的参考线时返回 0。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::guides::resize_edge_correction;
///
/// let correction = resize_edge_correction(28.0, 1.0, &[30.0], 6.0);
/// assert!((correction - 2.0).abs() < 1e-4);
/// ```
pub fn resize_edge_correction(edge: f32, slope: f32, guides: &[f32], threshold: f32) -> f32 {
    if !edge.is_finite()
        || !slope.is_finite()
        || slope.abs() < 1e-4
        || !threshold.is_finite()
        || threshold < 0.0
    {
        return 0.0;
    }
    let Some(target) = nearest_value(edge, guides, threshold) else {
        return 0.0;
    };
    let correction = (target - edge) / slope;
    if correction.is_finite() {
        correction
    } else {
        0.0
    }
}

/// 手柄在世界坐标里的外向。边手柄长度是 1，角手柄长度是 √2。
///
/// 正 x 表示这条边在增大世界 x（右边缘），正 y 表示在增大世界 y（下边缘，Y 向下）。
pub fn handle_outward(handle: ResizeHandle, rotation: f32) -> Vec2 {
    let (sx, sy) = handle.axis_signs();
    rotate(Vec2::new(sx as f32, sy as f32), rotation)
}

/// 判断缩放边是否已经和世界轴对齐，调用方据此决定吸 x 还是吸 y。
///
/// 斜边（例如旋转 45°）两个分量都不够接近 0 或 1 时，仍可以用 [`handle_outward`]
/// 的主分量决定吸哪条轴。本函数给测试和光标用。
pub fn edge_is_axis_aligned(rotation: f32) -> bool {
    if !rotation.is_finite() {
        return false;
    }
    let (sin, cos) = rotation.sin_cos();
    sin.abs() <= AXIS_ALIGN_EPSILON || cos.abs() <= AXIS_ALIGN_EPSILON
}

fn rotate(vector: Vec2, rotation: f32) -> Vec2 {
    let (sin, cos) = rotation.sin_cos();
    Vec2::new(
        cos * vector.x - sin * vector.y,
        sin * vector.x + cos * vector.y,
    )
}

fn nearest_value(value: f32, candidates: &[f32], threshold: f32) -> Option<f32> {
    let mut best: Option<f32> = None;
    for &candidate in candidates {
        if !candidate.is_finite() {
            continue;
        }
        let delta = candidate - value;
        if delta.abs() > threshold {
            continue;
        }
        let replace = match best {
            Some(current) => {
                let current_delta = current - value;
                let better_distance = delta.abs() + 1e-4 < current_delta.abs();
                let tie = (delta.abs() - current_delta.abs()).abs() <= 1e-4 && candidate < current;
                better_distance || tie
            }
            None => true,
        };
        if replace {
            best = Some(candidate);
        }
    }
    best
}

fn nice_step(raw: f32) -> f32 {
    if !raw.is_finite() || raw <= f32::EPSILON {
        return 1.0;
    }
    let exp = raw.log10().floor();
    let base = 10f32.powf(exp);
    if !base.is_finite() || base <= 0.0 {
        return 1.0;
    }
    let frac = raw / base;
    let nice = if frac < 1.5 {
        1.0
    } else if frac < 3.5 {
        2.0
    } else if frac < 7.5 {
        5.0
    } else {
        10.0
    };
    let step = nice * base;
    if step.is_finite() && step > 0.0 {
        step
    } else {
        1.0
    }
}

fn is_major_tick(world: f32, major: f32) -> bool {
    if major <= 0.0 {
        return false;
    }
    let k = (world / major).round();
    (world - k * major).abs() <= major * 1e-3
}

/// 元素轴对齐包围盒上、手柄正在推动的那条边。
///
/// 外向 x 为主时取左或右，外向 y 为主时取上或下。两条都明显时调用方各算一次。
pub fn moving_edge(bounds: Rect, outward_component: f32, horizontal: bool) -> f32 {
    if horizontal {
        if outward_component > 0.0 {
            bounds.bottom()
        } else {
            bounds.top()
        }
    } else if outward_component > 0.0 {
        bounds.right()
    } else {
        bounds.left()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec2;

    #[test]
    fn add_move_remove_and_hit_test() {
        let mut guides = GuideStore::default();
        let vertical = guides.add(GuideAxis::Vertical, 40.0).unwrap();
        let horizontal = guides.add(GuideAxis::Horizontal, 10.0).unwrap();
        assert_eq!(vertical.id, 1);
        assert_eq!(horizontal.id, 2);
        assert!(guides.move_to(vertical.id, 44.0));
        assert_eq!(guides.get(vertical.id).unwrap().position, 44.0);
        assert_eq!(
            guides.hit_test(Vec2::new(46.0, 100.0), 3.0),
            Some(vertical.id)
        );
        assert_eq!(
            guides.hit_test(Vec2::new(0.0, 12.0), 3.0),
            Some(horizontal.id)
        );
        assert_eq!(guides.hit_test(Vec2::new(0.0, 20.0), 3.0), None);
        assert!(guides.remove(vertical.id));
        assert!(guides.get(vertical.id).is_none());
        assert!(!guides.add(GuideAxis::Horizontal, f32::NAN).is_some());
    }

    #[test]
    fn hit_test_prefers_the_closer_guide_then_smaller_id() {
        let mut guides = GuideStore::default();
        let first = guides.add(GuideAxis::Vertical, 0.0).unwrap();
        let second = guides.add(GuideAxis::Vertical, 4.0).unwrap();
        assert_eq!(guides.hit_test(Vec2::new(1.0, 0.0), 6.0), Some(first.id));
        assert_eq!(guides.hit_test(Vec2::new(2.0, 0.0), 6.0), Some(first.id));
        let mut same = GuideStore::default();
        let low = same.add(GuideAxis::Vertical, 5.0).unwrap();
        let _high = same.add(GuideAxis::Horizontal, 5.0).unwrap();
        assert_eq!(same.hit_test(Vec2::new(5.0, 5.0), 1.0), Some(low.id));
        assert_eq!(second.id, 2);
    }

    #[test]
    fn records_round_trip_rejects_duplicates() {
        let mut guides = GuideStore::default();
        guides.add(GuideAxis::Horizontal, 3.0).unwrap();
        guides.add(GuideAxis::Vertical, -2.0).unwrap();
        let restored = GuideStore::from_records(guides.records()).unwrap();
        assert_eq!(restored, guides);
        let err = GuideStore::from_records(vec![
            ManualGuide {
                id: 1,
                axis: GuideAxis::Vertical,
                position: 0.0,
            },
            ManualGuide {
                id: 1,
                axis: GuideAxis::Horizontal,
                position: 1.0,
            },
        ]);
        assert!(err.is_err());
    }

    #[test]
    fn ruler_band_splits_corner_top_and_left() {
        assert_eq!(
            ruler_band(Vec2::new(4.0, 4.0), RULER_THICKNESS_PX),
            Some(RulerBand::Corner)
        );
        assert_eq!(
            ruler_band(Vec2::new(40.0, 8.0), RULER_THICKNESS_PX),
            Some(RulerBand::Top)
        );
        assert_eq!(
            ruler_band(Vec2::new(8.0, 40.0), RULER_THICKNESS_PX),
            Some(RulerBand::Left)
        );
        assert_eq!(ruler_band(Vec2::new(40.0, 40.0), RULER_THICKNESS_PX), None);
    }

    #[test]
    fn ticks_follow_zoom_and_include_origin() {
        let viewport = Viewport::new(Vec2::new(800.0, 600.0));
        assert_eq!(ruler_major_step(viewport.zoom()), 100.0);
        let ticks = ruler_ticks(&viewport, RulerOrientation::Horizontal);
        assert!(ticks
            .iter()
            .any(|tick| tick.major && tick.world.abs() < 1e-3));
        let origin = ticks.iter().find(|tick| tick.world.abs() < 1e-3).unwrap();
        assert!((origin.screen - 400.0).abs() < 1e-2);

        let mut zoomed = viewport.clone();
        zoomed.set_zoom(4.0);
        assert_eq!(ruler_major_step(zoomed.zoom()), 20.0);
        let dense = ruler_ticks(&zoomed, RulerOrientation::Horizontal);
        assert!(dense.len() > ticks.len());
    }

    #[test]
    fn pan_shifts_tick_screen_positions() {
        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));
        viewport.move_to(Vec2::new(50.0, -20.0));
        let ticks = ruler_ticks(&viewport, RulerOrientation::Vertical);
        let origin = ticks
            .iter()
            .find(|tick| tick.major && (tick.world - 0.0).abs() < 1e-2);
        let origin = origin.expect("zero stays on a major step");
        let screen = viewport.world_to_screen(Vec2::new(0.0, 0.0)).y;
        assert!((origin.screen - screen).abs() < 1e-2);
    }

    #[test]
    fn resize_correction_uses_slope_and_threshold() {
        assert!((resize_edge_correction(28.0, 1.0, &[30.0], 6.0) - 2.0).abs() < 1e-4);
        assert_eq!(resize_edge_correction(28.0, 1.0, &[100.0], 6.0), 0.0);
        assert_eq!(resize_edge_correction(28.0, 0.0, &[30.0], 6.0), 0.0);
        assert!((resize_edge_correction(32.0, -1.0, &[30.0], 6.0) - 2.0).abs() < 1e-4);
    }

    #[test]
    fn outward_of_east_handle_follows_rotation() {
        let east = handle_outward(ResizeHandle::East, 0.0);
        assert!((east.x - 1.0).abs() < 1e-5 && east.y.abs() < 1e-5);
        let turned = handle_outward(ResizeHandle::East, std::f32::consts::FRAC_PI_2);
        assert!(turned.x.abs() < 1e-5 && (turned.y - 1.0).abs() < 1e-5);
        assert!(edge_is_axis_aligned(0.0));
        assert!(edge_is_axis_aligned(std::f32::consts::FRAC_PI_2));
        assert!(!edge_is_axis_aligned(std::f32::consts::FRAC_PI_4));
    }
}
