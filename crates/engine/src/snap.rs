//! 拖拽对齐与吸附。
//!
//! 对齐主体是选区的轴对齐包围盒（多选时是并集）。它的左/右/上/下边缘
//! 以及水平/垂直中心，会去贴其他可见元素包围盒上的同类锚点。
//! X 与 Y 各自选距离最近的一次修正。阈值按屏幕像素配置，调用方用
//! [`SnapSettings::world_threshold`] 除掉当前缩放，手感不随缩放变化。
//!
//! 缩放/旋转手柄不在这里处理，拖拽平移先用包围盒吸附。
//!
//! ```text
//!        vertical guide
//!              |
//!   +----------|-----+
//!   |  moving  |     |
//!   +----------|-----+
//!              |
//!         +----|----+
//!         | target  |
//!         +---------+
//! ```
//!
//! # 示例
//! ```
//! use infinitecanvas_engine::math::{Rect, Vec2};
//! use infinitecanvas_engine::snap::{snap_bounds, AlignmentGuide, GuideAxis};
//!
//! let moving = Rect::new(0.0, 0.0, 40.0, 20.0);
//! let target = Rect::new(46.0, 200.0, 20.0, 20.0);
//! let result = snap_bounds(moving, &[target], 8.0);
//! assert_eq!(result.delta, Vec2::new(6.0, 0.0));
//! assert_eq!(
//!     result.guides,
//!     vec![AlignmentGuide {
//!         axis: GuideAxis::Vertical,
//!         position: 46.0,
//!     }]
//! );
//! ```

use crate::math::{Rect, Vec2};

/// 默认吸附阈值（屏幕像素）。
pub const DEFAULT_SNAP_THRESHOLD_PX: f32 = 6.0;

/// 阈值下限（屏幕像素）。0 表示只有完全重合才吸附。
pub const MIN_SNAP_THRESHOLD_PX: f32 = 0.0;

/// 阈值上限（屏幕像素），避免一次拖拽吸到很远的元素。
pub const MAX_SNAP_THRESHOLD_PX: f32 = 64.0;

/// 两条修正相差不超过该值时视为同一次吸附。
const DELTA_MATCH_EPSILON: f32 = 1e-3;

/// 距离几乎相同时的并列区间，用来做稳定的左右/上下取舍。
const TIE_EPSILON: f32 = 1e-4;

/// 参考线方向。
///
/// 垂直线固定世界坐标 x，表示左右边缘或水平中心对齐。
/// 水平线固定世界坐标 y，表示上下边缘或垂直中心对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GuideAxis {
    /// 水平线，[`AlignmentGuide::position`] 是世界坐标 y。
    Horizontal,
    /// 垂直线，[`AlignmentGuide::position`] 是世界坐标 x。
    Vertical,
}

impl GuideAxis {
    /// 给 WASM / HUD 用的稳定名称。
    ///
    /// # 返回
    /// `"horizontal"` 或 `"vertical"`。
    pub fn name(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// 一条当前生效的对齐参考线。
///
/// HUD 只需要方向和世界坐标：垂直线画在 `position` 的 x 上，
/// 水平线画在 `position` 的 y 上。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlignmentGuide {
    /// 参考线方向。
    pub axis: GuideAxis,
    /// 参考线所在的世界坐标。
    pub position: f32,
}

/// [`snap_bounds`] 的结果。
#[derive(Debug, Clone, PartialEq)]
pub struct SnapResult {
    /// 加到移动包围盒上的修正（世界坐标）。未进入阈值时为 0。
    pub delta: Vec2,
    /// 本次修正带出的参考线，先按轴、再按坐标排序。
    pub guides: Vec<AlignmentGuide>,
}

impl SnapResult {
    /// 不吸附：零修正，没有参考线。
    pub fn none() -> Self {
        Self {
            delta: Vec2::ZERO,
            guides: Vec::new(),
        }
    }
}

/// 一次拖拽求解。`translation` 从按下时的位置算起，已经包含吸附。
#[derive(Debug, Clone, PartialEq)]
pub struct DragSnap {
    /// 应写入元素变换的总位移。
    pub translation: Vec2,
    /// 与 [`SnapResult::guides`] 相同。
    pub guides: Vec<AlignmentGuide>,
}

/// 吸附开关和屏幕像素阈值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapSettings {
    /// 关闭后拖拽位移与未接入吸附时一致。
    pub enabled: bool,
    /// 屏幕像素。世界距离是 `threshold_px / zoom`。
    pub threshold_px: f32,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_px: DEFAULT_SNAP_THRESHOLD_PX,
        }
    }
}

impl SnapSettings {
    /// 写入阈值。非有限值忽略，其余夹到 [`MIN_SNAP_THRESHOLD_PX`] 与 [`MAX_SNAP_THRESHOLD_PX`]。
    ///
    /// # 参数
    /// - `threshold_px`: 屏幕像素
    pub fn set_threshold_px(&mut self, threshold_px: f32) {
        if threshold_px.is_finite() {
            self.threshold_px = threshold_px.clamp(MIN_SNAP_THRESHOLD_PX, MAX_SNAP_THRESHOLD_PX);
        }
    }

    /// 把屏幕像素阈值换成当前缩放下的世界距离。
    ///
    /// 缩放越大，同样的像素对应更小的世界距离，吸附范围在屏幕上保持不变。
    /// 缩放不是正有限值时按 1 处理。
    ///
    /// # 参数
    /// - `zoom`: 视口缩放
    ///
    /// # 返回
    /// 世界坐标下的吸附半径。
    pub fn world_threshold(&self, zoom: f32) -> f32 {
        let zoom = if zoom.is_finite() && zoom > f32::EPSILON {
            zoom
        } else {
            1.0
        };
        let px = if self.threshold_px.is_finite() && self.threshold_px > 0.0 {
            self.threshold_px
        } else {
            0.0
        };
        px / zoom
    }
}

/// 若干矩形的轴对齐并集。空输入返回 `None`。
///
/// 多选拖拽用它做对齐主体，而不是逐个元素各自吸附。
///
/// # 参数
/// - `rects`: 选中元素的世界包围盒
pub fn bounds_union(rects: impl IntoIterator<Item = Rect>) -> Option<Rect> {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut any = false;
    for rect in rects {
        if !rect_is_finite(rect) {
            continue;
        }
        any = true;
        min_x = min_x.min(rect.left());
        min_y = min_y.min(rect.top());
        max_x = max_x.max(rect.right());
        max_y = max_y.max(rect.bottom());
    }
    if !any {
        return None;
    }
    Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
}

/// 计算移动包围盒贴齐目标所需的修正和参考线。
///
/// `moving` 是「假如完全跟着指针、还没吸附」时的包围盒。`targets` 不应包含
/// 正在被拖动的元素，否则会吸到自己身上。阈值与矩形使用同一单位（世界坐标），
/// 包含等于阈值的情况。
///
/// 同一轴上距离相等时，取更小的修正（优先向左 / 向上）。
///
/// # 参数
/// - `moving`: 未吸附的移动包围盒
/// - `targets`: 其他元素的世界包围盒
/// - `threshold`: 世界坐标阈值
///
/// # 返回
/// 修正量和当前参考线。没有命中时修正为 0、参考线为空。
pub fn snap_bounds(moving: Rect, targets: &[Rect], threshold: f32) -> SnapResult {
    if !threshold.is_finite() || threshold < 0.0 || !rect_is_finite(moving) {
        return SnapResult::none();
    }

    let mut guides = Vec::new();
    let mut delta = Vec2::ZERO;

    if let Some(axis) = snap_axis(anchors(moving, true), targets, threshold, true) {
        delta.x = axis.delta;
        for position in axis.positions {
            guides.push(AlignmentGuide {
                axis: GuideAxis::Vertical,
                position,
            });
        }
    }
    if let Some(axis) = snap_axis(anchors(moving, false), targets, threshold, false) {
        delta.y = axis.delta;
        for position in axis.positions {
            guides.push(AlignmentGuide {
                axis: GuideAxis::Horizontal,
                position,
            });
        }
    }

    guides.sort_by(|left, right| {
        left.axis.cmp(&right.axis).then_with(|| {
            left.position
                .partial_cmp(&right.position)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    SnapResult { delta, guides }
}

/// 把指针位移吸附到周围元素上。
///
/// 关闭吸附时 `translation == raw_delta`，参考线为空，拖拽结果与未接入吸附时一致。
///
/// # 参数
/// - `origin`: 按下时的选区包围盒
/// - `raw_delta`: 指针相对按下点的世界位移（不含吸附）
/// - `targets`: 其他可见元素的世界包围盒
/// - `settings`: 开关和屏幕像素阈值
/// - `zoom`: 当前视口缩放
///
/// # 返回
/// 从按下位置算起的总位移，以及这一帧的参考线。
pub fn snap_drag(
    origin: Rect,
    raw_delta: Vec2,
    targets: &[Rect],
    settings: &SnapSettings,
    zoom: f32,
) -> DragSnap {
    if !settings.enabled {
        return DragSnap {
            translation: raw_delta,
            guides: Vec::new(),
        };
    }
    let proposed = translate_rect(origin, raw_delta);
    let snapped = snap_bounds(proposed, targets, settings.world_threshold(zoom));
    DragSnap {
        translation: raw_delta + snapped.delta,
        guides: snapped.guides,
    }
}

struct AxisSnap {
    delta: f32,
    positions: Vec<f32>,
}

fn translate_rect(rect: Rect, delta: Vec2) -> Rect {
    Rect::new(rect.x + delta.x, rect.y + delta.y, rect.width, rect.height)
}

fn rect_is_finite(rect: Rect) -> bool {
    rect.x.is_finite() && rect.y.is_finite() && rect.width.is_finite() && rect.height.is_finite()
}

/// 一个轴上的三个锚点：起点、中心、终点。
fn anchors(rect: Rect, along_x: bool) -> [f32; 3] {
    if along_x {
        [rect.left(), rect.center().x, rect.right()]
    } else {
        [rect.top(), rect.center().y, rect.bottom()]
    }
}

fn snap_axis(
    moving: [f32; 3],
    targets: &[Rect],
    threshold: f32,
    along_x: bool,
) -> Option<AxisSnap> {
    let best = best_delta(moving, targets, threshold, along_x)?;
    let mut positions = Vec::new();
    for target in targets {
        if !rect_is_finite(*target) {
            continue;
        }
        for target_anchor in anchors(*target, along_x) {
            for moving_anchor in moving {
                let delta = target_anchor - moving_anchor;
                if (delta - best).abs() <= DELTA_MATCH_EPSILON {
                    push_unique(&mut positions, target_anchor);
                }
            }
        }
    }
    positions.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    Some(AxisSnap {
        delta: best,
        positions,
    })
}

fn best_delta(moving: [f32; 3], targets: &[Rect], threshold: f32, along_x: bool) -> Option<f32> {
    let mut best: Option<f32> = None;
    for target in targets {
        if !rect_is_finite(*target) {
            continue;
        }
        for target_anchor in anchors(*target, along_x) {
            for moving_anchor in moving {
                let delta = target_anchor - moving_anchor;
                if !delta.is_finite() || delta.abs() > threshold {
                    continue;
                }
                if best
                    .map(|current| is_better_delta(delta, current))
                    .unwrap_or(true)
                {
                    best = Some(delta);
                }
            }
        }
    }
    best
}

/// 更小的绝对距离优先；距离相同则取更小（更靠左 / 更靠上）的修正。
fn is_better_delta(candidate: f32, current: f32) -> bool {
    let candidate_abs = candidate.abs();
    let current_abs = current.abs();
    if (candidate_abs - current_abs).abs() <= TIE_EPSILON {
        candidate < current
    } else {
        candidate_abs < current_abs
    }
}

fn push_unique(positions: &mut Vec<f32>, value: f32) {
    if positions
        .iter()
        .any(|existing| (existing - value).abs() <= DELTA_MATCH_EPSILON)
    {
        return;
    }
    positions.push(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guide(axis: GuideAxis, position: f32) -> AlignmentGuide {
        AlignmentGuide { axis, position }
    }

    #[test]
    fn snaps_right_edge_to_target_left_within_threshold() {
        let moving = Rect::new(0.0, 0.0, 40.0, 10.0);
        let target = Rect::new(48.0, 500.0, 20.0, 10.0);
        let result = snap_bounds(moving, &[target], 10.0);

        assert_eq!(result.delta, Vec2::new(8.0, 0.0));
        assert_eq!(result.guides, vec![guide(GuideAxis::Vertical, 48.0)]);
    }

    #[test]
    fn snaps_center_and_reports_one_vertical_guide() {
        let moving = Rect::new(0.0, 0.0, 100.0, 20.0);
        let target = Rect::new(54.0, 800.0, 0.0, 10.0);
        let result = snap_bounds(moving, &[target], 6.0);

        assert_eq!(result.delta, Vec2::new(4.0, 0.0));
        assert_eq!(result.guides, vec![guide(GuideAxis::Vertical, 54.0)]);
    }

    #[test]
    fn snaps_top_edge_to_target_bottom() {
        let moving = Rect::new(0.0, 46.0, 10.0, 20.0);
        let target = Rect::new(500.0, 10.0, 10.0, 30.0);
        let result = snap_bounds(moving, &[target], 8.0);

        assert_eq!(result.delta, Vec2::new(0.0, -6.0));
        assert_eq!(result.guides, vec![guide(GuideAxis::Horizontal, 40.0)]);
    }

    #[test]
    fn snaps_both_axes_and_sorts_guides() {
        let moving = Rect::new(0.0, 0.0, 100.0, 100.0);
        let target = Rect::new(54.0, 47.0, 0.0, 0.0);
        let result = snap_bounds(moving, &[target], 6.0);

        assert_eq!(result.delta, Vec2::new(4.0, -3.0));
        assert_eq!(
            result.guides,
            vec![
                guide(GuideAxis::Horizontal, 47.0),
                guide(GuideAxis::Vertical, 54.0),
            ]
        );
    }

    #[test]
    fn outside_threshold_does_not_snap() {
        let moving = Rect::new(0.0, 0.0, 10.0, 10.0);
        let target = Rect::new(30.0, 400.0, 10.0, 10.0);
        let result = snap_bounds(moving, &[target], 8.0);

        assert_eq!(result, SnapResult::none());
    }

    #[test]
    fn already_aligned_keeps_position_and_emits_guide() {
        let moving = Rect::new(0.0, 0.0, 20.0, 10.0);
        let target = Rect::new(0.0, 400.0, 50.0, 10.0);
        let result = snap_bounds(moving, &[target], 5.0);

        assert_eq!(result.delta, Vec2::ZERO);
        assert_eq!(result.guides, vec![guide(GuideAxis::Vertical, 0.0)]);
    }

    #[test]
    fn equal_distance_prefers_the_negative_correction() {
        let moving = Rect::new(0.0, 0.0, 0.0, 10.0);
        let targets = [
            Rect::new(6.0, 1000.0, 0.0, 10.0),
            Rect::new(-6.0, 1000.0, 0.0, 10.0),
        ];
        let result = snap_bounds(moving, &targets, 8.0);

        assert_eq!(result.delta, Vec2::new(-6.0, 0.0));
        assert_eq!(result.guides, vec![guide(GuideAxis::Vertical, -6.0)]);
    }

    #[test]
    fn closest_target_wins_regardless_of_order() {
        let moving = Rect::new(0.0, 0.0, 10.0, 10.0);
        let far = Rect::new(22.0, 400.0, 0.0, 10.0);
        let near = Rect::new(14.0, 400.0, 0.0, 10.0);

        let forward = snap_bounds(moving, &[far, near], 12.0);
        let backward = snap_bounds(moving, &[near, far], 12.0);

        assert_eq!(forward.delta, Vec2::new(4.0, 0.0));
        assert_eq!(backward, forward);
        assert_eq!(forward.guides, vec![guide(GuideAxis::Vertical, 14.0)]);
    }

    #[test]
    fn one_translation_can_emit_two_guides_on_the_same_axis() {
        let moving = Rect::new(0.0, 0.0, 100.0, 10.0);
        let targets = [
            Rect::new(10.0, 500.0, 0.0, 10.0),
            Rect::new(110.0, 500.0, 0.0, 10.0),
        ];
        let result = snap_bounds(moving, &targets, 12.0);

        assert_eq!(result.delta, Vec2::new(10.0, 0.0));
        assert_eq!(
            result.guides,
            vec![
                guide(GuideAxis::Vertical, 10.0),
                guide(GuideAxis::Vertical, 110.0),
            ]
        );
    }

    #[test]
    fn coincident_targets_share_one_guide() {
        let moving = Rect::new(0.0, 0.0, 10.0, 10.0);
        let targets = [
            Rect::new(16.0, 400.0, 10.0, 10.0),
            Rect::new(16.0, 800.0, 20.0, 10.0),
        ];
        let result = snap_bounds(moving, &targets, 8.0);

        assert_eq!(result.delta, Vec2::new(6.0, 0.0));
        assert_eq!(result.guides, vec![guide(GuideAxis::Vertical, 16.0)]);
    }

    #[test]
    fn empty_or_non_finite_input_does_not_snap() {
        let moving = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert_eq!(snap_bounds(moving, &[], 8.0), SnapResult::none());
        assert_eq!(snap_bounds(moving, &[moving], f32::NAN), SnapResult::none());
        assert_eq!(
            snap_bounds(Rect::new(f32::NAN, 0.0, 1.0, 1.0), &[moving], 8.0),
            SnapResult::none()
        );
    }

    #[test]
    fn threshold_stays_constant_in_screen_pixels() {
        let origin = Rect::new(0.0, 0.0, 10.0, 10.0);
        let raw = Vec2::ZERO;
        let targets = [Rect::new(18.0, 400.0, 10.0, 10.0)];
        let settings = SnapSettings {
            enabled: true,
            threshold_px: 8.0,
        };

        let at_one = snap_drag(origin, raw, &targets, &settings, 1.0);
        let zoomed_in = snap_drag(origin, raw, &targets, &settings, 2.0);
        let zoomed_out = snap_drag(origin, raw, &targets, &settings, 0.5);

        assert_eq!(at_one.translation, Vec2::new(8.0, 0.0));
        assert_eq!(at_one.guides, vec![guide(GuideAxis::Vertical, 18.0)]);
        assert_eq!(zoomed_in.translation, raw);
        assert!(zoomed_in.guides.is_empty());
        assert_eq!(zoomed_out.translation, Vec2::new(8.0, 0.0));
        assert_eq!(settings.world_threshold(2.0), 4.0);
        assert_eq!(settings.world_threshold(0.0), 8.0);
    }

    #[test]
    fn snapping_off_keeps_the_pointer_translation() {
        let origin = Rect::new(0.0, 0.0, 40.0, 20.0);
        let raw = Vec2::new(56.0, 3.0);
        let targets = [Rect::new(100.0, 0.0, 40.0, 20.0)];
        let settings = SnapSettings {
            enabled: false,
            threshold_px: 8.0,
        };

        let result = snap_drag(origin, raw, &targets, &settings, 1.0);
        assert_eq!(result.translation, raw);
        assert!(result.guides.is_empty());
    }

    #[test]
    fn drag_within_threshold_lands_on_the_target_edge() {
        let origin = Rect::new(0.0, 0.0, 40.0, 20.0);
        let raw = Vec2::new(56.0, 3.0);
        let targets = [Rect::new(100.0, 400.0, 40.0, 20.0)];
        let settings = SnapSettings {
            enabled: true,
            threshold_px: 8.0,
        };

        let result = snap_drag(origin, raw, &targets, &settings, 1.0);
        assert_eq!(result.translation, Vec2::new(60.0, 3.0));
        assert_eq!(result.guides, vec![guide(GuideAxis::Vertical, 100.0)]);
    }

    #[test]
    fn multi_selection_snaps_the_union_not_each_member() {
        let parts = [
            Rect::new(0.0, 0.0, 40.0, 20.0),
            Rect::new(60.0, 30.0, 20.0, 10.0),
        ];
        let union = bounds_union(parts).unwrap();
        assert_eq!(union, Rect::new(0.0, 0.0, 80.0, 40.0));

        let raw = Vec2::new(6.0, 0.0);
        let targets = [Rect::new(90.0, 400.0, 10.0, 10.0)];
        let settings = SnapSettings::default();

        let grouped = snap_drag(union, raw, &targets, &settings, 1.0);
        let alone = snap_drag(parts[0], raw, &targets, &settings, 1.0);

        assert_eq!(grouped.translation, Vec2::new(10.0, 0.0));
        assert_eq!(grouped.guides, vec![guide(GuideAxis::Vertical, 90.0)]);
        assert_eq!(alone.translation, raw);
        assert!(alone.guides.is_empty());
    }

    #[test]
    fn threshold_setter_clamps_and_ignores_non_finite_values() {
        let mut settings = SnapSettings::default();
        assert!(settings.enabled);
        assert_eq!(settings.threshold_px, DEFAULT_SNAP_THRESHOLD_PX);

        settings.set_threshold_px(100.0);
        assert_eq!(settings.threshold_px, MAX_SNAP_THRESHOLD_PX);
        settings.set_threshold_px(-5.0);
        assert_eq!(settings.threshold_px, MIN_SNAP_THRESHOLD_PX);
        settings.set_threshold_px(f32::NAN);
        assert_eq!(settings.threshold_px, MIN_SNAP_THRESHOLD_PX);
    }

    #[test]
    fn bounds_union_skips_empty_input() {
        assert_eq!(bounds_union([]), None);
    }
}
