//! 多选对齐与分布。
//!
//! 按选中元素的世界包围盒平移。选中组时用组的世界包围盒，只移动组根。
//! 拖拽吸附不经过这里。

use crate::command::{nudge_targets, Command, CommandHistory};
use crate::element::ElementId;
use crate::math::{Rect, Vec2};
use crate::Scene;

/// 小于该距离的平移视为没有移动，不记入撤销栈。
const MOVE_EPSILON: f32 = 1e-4;

/// 水平或垂直对齐方式。
///
/// 参考框是全部目标世界包围盒的并集。左/右/上/下贴齐并集的对应边，
/// 水平居中和垂直居中贴齐并集的中心。只改平移。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignMode {
    /// 左边缘对齐到并集左边缘。
    Left,
    /// 水平中心对齐到并集水平中心。
    Center,
    /// 右边缘对齐到并集右边缘。
    Right,
    /// 上边缘对齐到并集上边缘。世界 Y 向下，上是较小的 Y。
    Top,
    /// 垂直中心对齐到并集垂直中心。
    Middle,
    /// 下边缘对齐到并集下边缘。
    Bottom,
}

impl AlignMode {
    /// 解析对齐方式。
    ///
    /// 接受 `left` / `center` / `right` / `top` / `middle` / `bottom`。
    /// `center` 是水平居中；`middle` 是垂直居中。大小写不敏感，两端空白忽略。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::AlignMode;
    ///
    /// assert_eq!(AlignMode::parse(" Center "), Some(AlignMode::Center));
    /// assert_eq!(AlignMode::parse("middle"), Some(AlignMode::Middle));
    /// assert!(AlignMode::parse("justify").is_none());
    /// ```
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "left" => Some(Self::Left),
            "center" | "hcenter" | "horizontal-center" => Some(Self::Center),
            "right" => Some(Self::Right),
            "top" => Some(Self::Top),
            "middle" | "vcenter" | "vertical-center" => Some(Self::Middle),
            "bottom" => Some(Self::Bottom),
            _ => None,
        }
    }

    /// 稳定的短名字，与 [`AlignMode::parse`] 对应。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::Top => "top",
            Self::Middle => "middle",
            Self::Bottom => "bottom",
        }
    }

    fn delta(self, bounds: Rect, union: Rect) -> Vec2 {
        match self {
            Self::Left => Vec2::new(union.left() - bounds.left(), 0.0),
            Self::Center => Vec2::new(union.center().x - bounds.center().x, 0.0),
            Self::Right => Vec2::new(union.right() - bounds.right(), 0.0),
            Self::Top => Vec2::new(0.0, union.top() - bounds.top()),
            Self::Middle => Vec2::new(0.0, union.center().y - bounds.center().y),
            Self::Bottom => Vec2::new(0.0, union.bottom() - bounds.bottom()),
        }
    }
}

/// 均匀分布的轴向。
///
/// 按包围盒沿该轴排序，两端元素不动，中间元素平移到相邻间隙相等。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributeAxis {
    /// 水平间隙。按左边缘排序。
    Horizontal,
    /// 垂直间隙。按上边缘排序。世界 Y 向下。
    Vertical,
}

impl DistributeAxis {
    /// 解析分布轴。
    ///
    /// 接受 `horizontal` / `h` / `x` 和 `vertical` / `v` / `y`。
    /// 大小写不敏感，两端空白忽略。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::DistributeAxis;
    ///
    /// assert_eq!(
    ///     DistributeAxis::parse("Horizontal"),
    ///     Some(DistributeAxis::Horizontal)
    /// );
    /// assert!(DistributeAxis::parse("both").is_none());
    /// ```
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "horizontal" | "h" | "x" => Some(Self::Horizontal),
            "vertical" | "v" | "y" => Some(Self::Vertical),
            _ => None,
        }
    }

    /// 稳定的短名字，与 [`DistributeAxis::parse`] 对应。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// 把当前选择按包围盒对齐，并记入一条撤销记录。
///
/// 少于两个可移动目标、方式无法产生位移、或选择为空时不入栈。
/// 可移动目标与方向键相同：在场景中、自身和祖先都可见、未被锁定，
/// 且选中子树只保留根。选中组时移动组根。
///
/// # 参数
/// - `history`: 撤销栈
/// - `scene`: 要修改的场景
/// - `selected`: 当前选中的元素 id
/// - `mode`: 对齐方式
///
/// # 返回
/// 是否写入了撤销栈
///
/// # 示例
/// ```
/// use infinitecanvas_engine::math::Vec2;
/// use infinitecanvas_engine::{
///     apply_align, AlignMode, CommandHistory, Element, Rectangle, Scene,
/// };
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
/// scene.add_rect(Rectangle::new(
///     2,
///     Vec2::new(40.0, 20.0),
///     Vec2::new(10.0, 10.0),
/// ));
/// let mut history = CommandHistory::new();
/// assert!(apply_align(
///     &mut history,
///     &mut scene,
///     &[1, 2],
///     AlignMode::Left,
/// ));
/// let left = scene.get(2).unwrap().as_element().world_bounds().left();
/// assert!((left + 5.0).abs() < 1e-3);
/// assert!(history.undo(&mut scene));
/// assert!(!apply_align(
///     &mut history,
///     &mut scene,
///     &[1],
///     AlignMode::Right,
/// ));
/// ```
pub fn apply_align(
    history: &mut CommandHistory,
    scene: &mut Scene,
    selected: &[ElementId],
    mode: AlignMode,
) -> bool {
    let Some(command) = AlignElementsCommand::from_selection(scene, selected, mode) else {
        return false;
    };
    history.execute(Box::new(command), scene);
    true
}

/// 把当前选择沿一个轴均匀分布，并记入一条撤销记录。
///
/// 少于三个可移动目标时不入栈：两端保持不动，没有中间元素可排。
/// 目标过滤与 [`apply_align`] 相同。
///
/// # 参数
/// - `history`: 撤销栈
/// - `scene`: 要修改的场景
/// - `selected`: 当前选中的元素 id
/// - `axis`: 分布轴
///
/// # 返回
/// 是否写入了撤销栈
///
/// # 示例
/// ```
/// use infinitecanvas_engine::math::Vec2;
/// use infinitecanvas_engine::{apply_distribute, DistributeAxis, CommandHistory, Scene};
///
/// let mut scene = Scene::new();
/// let mut history = CommandHistory::new();
/// assert!(!apply_distribute(
///     &mut history,
///     &mut scene,
///     &[],
///     DistributeAxis::Vertical,
/// ));
/// ```
pub fn apply_distribute(
    history: &mut CommandHistory,
    scene: &mut Scene,
    selected: &[ElementId],
    axis: DistributeAxis,
) -> bool {
    let Some(command) = DistributeElementsCommand::from_selection(scene, selected, axis) else {
        return false;
    };
    history.execute(Box::new(command), scene);
    true
}

/// 一次对齐。每个目标有自己的平移量，撤销时按相反方向移回。
#[derive(Debug)]
pub struct AlignElementsCommand {
    deltas: Vec<(ElementId, Vec2)>,
}

impl AlignElementsCommand {
    /// 按当前选择构造一条尚未执行的对齐命令。
    ///
    /// 可移动目标少于 2，或所有包围盒已经贴齐时返回 `None`。
    ///
    /// # 参数
    /// - `scene`: 当前场景
    /// - `selected`: 当前选中的元素 id
    /// - `mode`: 对齐方式
    pub fn from_selection(scene: &Scene, selected: &[ElementId], mode: AlignMode) -> Option<Self> {
        let items = layout_items(scene, selected);
        if items.len() < 2 {
            return None;
        }
        let union = union_bounds(&items)?;
        let deltas = movement_deltas(
            items
                .into_iter()
                .map(|(id, bounds)| (id, mode.delta(bounds, union))),
        );
        (!deltas.is_empty()).then_some(Self { deltas })
    }
}

impl Command for AlignElementsCommand {
    fn execute(&mut self, scene: &mut Scene) {
        apply_deltas(scene, &self.deltas, false);
    }

    fn undo(&mut self, scene: &mut Scene) {
        apply_deltas(scene, &self.deltas, true);
    }

    fn name(&self) -> &'static str {
        "AlignElements"
    }
}

/// 一次分布。两端元素不在位移列表里。
#[derive(Debug)]
pub struct DistributeElementsCommand {
    deltas: Vec<(ElementId, Vec2)>,
}

impl DistributeElementsCommand {
    /// 按当前选择构造一条尚未执行的分布命令。
    ///
    /// 可移动目标少于 3，或间隙已经相等时返回 `None`。
    ///
    /// # 参数
    /// - `scene`: 当前场景
    /// - `selected`: 当前选中的元素 id
    /// - `axis`: 分布轴
    pub fn from_selection(
        scene: &Scene,
        selected: &[ElementId],
        axis: DistributeAxis,
    ) -> Option<Self> {
        let items = layout_items(scene, selected);
        if items.len() < 3 {
            return None;
        }
        let deltas = distribute_deltas(&items, axis);
        (!deltas.is_empty()).then_some(Self { deltas })
    }
}

impl Command for DistributeElementsCommand {
    fn execute(&mut self, scene: &mut Scene) {
        apply_deltas(scene, &self.deltas, false);
    }

    fn undo(&mut self, scene: &mut Scene) {
        apply_deltas(scene, &self.deltas, true);
    }

    fn name(&self) -> &'static str {
        "DistributeElements"
    }
}

fn layout_items(scene: &Scene, selected: &[ElementId]) -> Vec<(ElementId, Rect)> {
    nudge_targets(scene, selected)
        .into_iter()
        .filter_map(|id| {
            let bounds = scene.get(id)?.as_element().world_bounds();
            finite_rect(bounds).then_some((id, bounds))
        })
        .collect()
}

fn union_bounds(items: &[(ElementId, Rect)]) -> Option<Rect> {
    let mut iter = items.iter().map(|(_, bounds)| *bounds);
    let first = iter.next()?;
    Some(iter.fold(first, |acc, bounds| acc.union(&bounds)))
}

fn distribute_deltas(items: &[(ElementId, Rect)], axis: DistributeAxis) -> Vec<(ElementId, Vec2)> {
    let mut ordered = items.to_vec();
    ordered.sort_by(|a, b| {
        let key = match axis {
            DistributeAxis::Horizontal => (a.1.left(), b.1.left()),
            DistributeAxis::Vertical => (a.1.top(), b.1.top()),
        };
        key.0.total_cmp(&key.1).then(a.0.cmp(&b.0))
    });
    let count = ordered.len();
    if count < 3 {
        return Vec::new();
    }
    let (span, total_size) = match axis {
        DistributeAxis::Horizontal => {
            let span = ordered[count - 1].1.right() - ordered[0].1.left();
            let total_size: f32 = ordered.iter().map(|(_, bounds)| bounds.width).sum();
            (span, total_size)
        }
        DistributeAxis::Vertical => {
            let span = ordered[count - 1].1.bottom() - ordered[0].1.top();
            let total_size: f32 = ordered.iter().map(|(_, bounds)| bounds.height).sum();
            (span, total_size)
        }
    };
    let gap = (span - total_size) / (count - 1) as f32;
    if !gap.is_finite() {
        return Vec::new();
    }
    let mut cursor = match axis {
        DistributeAxis::Horizontal => ordered[0].1.right(),
        DistributeAxis::Vertical => ordered[0].1.bottom(),
    } + gap;
    let mut deltas = Vec::new();
    for (id, bounds) in ordered.iter().take(count - 1).skip(1) {
        let delta = match axis {
            DistributeAxis::Horizontal => Vec2::new(cursor - bounds.left(), 0.0),
            DistributeAxis::Vertical => Vec2::new(0.0, cursor - bounds.top()),
        };
        if meaningful(delta) {
            deltas.push((*id, delta));
        }
        let size = match axis {
            DistributeAxis::Horizontal => bounds.width,
            DistributeAxis::Vertical => bounds.height,
        };
        cursor += size + gap;
    }
    deltas
}

fn movement_deltas(deltas: impl IntoIterator<Item = (ElementId, Vec2)>) -> Vec<(ElementId, Vec2)> {
    deltas
        .into_iter()
        .filter(|(_, delta)| meaningful(*delta))
        .collect()
}

fn meaningful(delta: Vec2) -> bool {
    delta.x.is_finite()
        && delta.y.is_finite()
        && (delta.x.abs() > MOVE_EPSILON || delta.y.abs() > MOVE_EPSILON)
}

fn finite_rect(rect: Rect) -> bool {
    rect.x.is_finite() && rect.y.is_finite() && rect.width.is_finite() && rect.height.is_finite()
}

fn apply_deltas(scene: &mut Scene, deltas: &[(ElementId, Vec2)], invert: bool) {
    if invert {
        for (id, delta) in deltas.iter().rev() {
            let _ = scene.translate_element(*id, -*delta);
        }
    } else {
        for (id, delta) in deltas {
            let _ = scene.translate_element(*id, *delta);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandHistory;
    use crate::{Rectangle, Text};
    use infinitecanvas_math::Vec2;

    fn translation(scene: &Scene, id: ElementId) -> Vec2 {
        scene
            .get(id)
            .unwrap()
            .as_element()
            .props()
            .transform
            .translation
    }

    fn bounds(scene: &Scene, id: ElementId) -> Rect {
        scene.get(id).unwrap().as_element().world_bounds()
    }

    fn close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-3,
            "actual {actual} expected {expected}"
        );
    }

    fn close_vec(actual: Vec2, expected: Vec2) {
        assert!(
            (actual - expected).length() < 1e-3,
            "actual {actual:?} expected {expected:?}"
        );
    }

    fn assert_same(values: &[f32]) {
        for value in &values[1..] {
            close(*value, values[0]);
        }
    }

    /// 矩形 A、B 与文本 C。包围盒并集是 x∈[-10, 110]，y∈[-20, 50]。
    fn sample_scene() -> (Scene, Vec<ElementId>) {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(20.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(50.0, 30.0),
            Vec2::new(10.0, 40.0),
        ));
        let mut text = Text::new(3, Vec2::new(80.0, -20.0), "Hi");
        text.size = Vec2::new(30.0, 8.0);
        scene.add_text(text);
        (scene, vec![1, 2, 3])
    }

    fn edge(scene: &Scene, id: ElementId, mode: AlignMode) -> f32 {
        let bounds = bounds(scene, id);
        match mode {
            AlignMode::Left => bounds.left(),
            AlignMode::Center => bounds.center().x,
            AlignMode::Right => bounds.right(),
            AlignMode::Top => bounds.top(),
            AlignMode::Middle => bounds.center().y,
            AlignMode::Bottom => bounds.bottom(),
        }
    }

    #[test]
    fn horizontal_and_vertical_align_share_edges_or_centers() {
        let expected = [
            (AlignMode::Left, [-10.0, -10.0, -10.0]),
            (AlignMode::Center, [50.0, 50.0, 50.0]),
            (AlignMode::Right, [110.0, 110.0, 110.0]),
            (AlignMode::Top, [-20.0, -20.0, -20.0]),
            (AlignMode::Middle, [15.0, 15.0, 15.0]),
            (AlignMode::Bottom, [50.0, 50.0, 50.0]),
        ];
        for (mode, edges) in expected {
            let (mut scene, ids) = sample_scene();
            let mut history = CommandHistory::new();
            let before: Vec<Vec2> = ids
                .iter()
                .copied()
                .map(|id| translation(&scene, id))
                .collect();
            assert!(apply_align(&mut history, &mut scene, &ids, mode));
            assert_eq!(history.undo_count(), 1);
            assert_eq!(history.redo_count(), 0);
            let actual = [
                edge(&scene, 1, mode),
                edge(&scene, 2, mode),
                edge(&scene, 3, mode),
            ];
            assert_same(&actual);
            close(actual[0], edges[0]);
            for (id, original) in ids.iter().copied().zip(before.iter()) {
                let moved = translation(&scene, id);
                if matches!(mode, AlignMode::Left | AlignMode::Center | AlignMode::Right) {
                    close(moved.y, original.y);
                } else {
                    close(moved.x, original.x);
                }
                let transform = scene.get(id).unwrap().as_element().props().transform;
                assert_eq!(transform.rotation, 0.0);
                assert_eq!(transform.scale, Vec2::ONE);
            }
            assert!(history.undo(&mut scene));
            for (id, original) in ids.iter().copied().zip(before.iter()) {
                close_vec(translation(&scene, id), *original);
            }
            assert!(history.redo(&mut scene));
            close(edge(&scene, 2, mode), edges[1]);
            assert!(history.undo(&mut scene));
            assert!(!history.can_undo());
        }
    }

    #[test]
    fn distribute_makes_equal_gaps_and_keeps_ends() {
        let (mut scene, ids) = sample_scene();
        let mut history = CommandHistory::new();
        let before: Vec<Vec2> = ids
            .iter()
            .copied()
            .map(|id| translation(&scene, id))
            .collect();

        assert!(apply_distribute(
            &mut history,
            &mut scene,
            &ids,
            DistributeAxis::Horizontal,
        ));
        assert_eq!(history.undo_count(), 1);
        close(bounds(&scene, 1).left(), -10.0);
        close(bounds(&scene, 2).left(), 40.0);
        close(bounds(&scene, 3).left(), 80.0);
        close(bounds(&scene, 2).left() - bounds(&scene, 1).right(), 30.0);
        close(bounds(&scene, 3).left() - bounds(&scene, 2).right(), 30.0);
        close(translation(&scene, 1).y, before[0].y);
        close(translation(&scene, 3).y, before[2].y);

        assert!(history.undo(&mut scene));
        for (id, original) in ids.iter().copied().zip(before.iter()) {
            close_vec(translation(&scene, id), *original);
        }
        assert!(history.redo(&mut scene));
        close(bounds(&scene, 2).left(), 40.0);
        assert!(history.undo(&mut scene));

        assert!(apply_distribute(
            &mut history,
            &mut scene,
            &ids,
            DistributeAxis::Vertical,
        ));
        close(bounds(&scene, 3).top(), -20.0);
        close(bounds(&scene, 1).top(), -6.0);
        close(bounds(&scene, 2).top(), 10.0);
        close(bounds(&scene, 1).top() - bounds(&scene, 3).bottom(), 6.0);
        close(bounds(&scene, 2).top() - bounds(&scene, 1).bottom(), 6.0);
        close(translation(&scene, 2).x, before[1].x);
        close(translation(&scene, 3).x, before[2].x);
        assert_eq!(history.undo_count(), 1);

        assert!(history.undo(&mut scene));
        for (id, original) in ids.iter().copied().zip(before.iter()) {
            close_vec(translation(&scene, id), *original);
        }
        assert!(history.can_redo());
    }

    #[test]
    fn single_or_empty_selection_is_noop() {
        let (mut scene, _) = sample_scene();
        let mut history = CommandHistory::new();
        let before = translation(&scene, 1);

        assert!(!apply_align(&mut history, &mut scene, &[], AlignMode::Left));
        assert!(!apply_distribute(
            &mut history,
            &mut scene,
            &[],
            DistributeAxis::Horizontal,
        ));
        assert!(AlignElementsCommand::from_selection(&scene, &[1], AlignMode::Bottom).is_none());
        assert!(!apply_align(
            &mut history,
            &mut scene,
            &[2],
            AlignMode::Center
        ));
        assert!(DistributeElementsCommand::from_selection(
            &scene,
            &[1, 2],
            DistributeAxis::Vertical,
        )
        .is_none());
        assert!(!apply_distribute(
            &mut history,
            &mut scene,
            &[1, 3],
            DistributeAxis::Horizontal,
        ));
        assert_eq!(history.undo_count(), 0);
        close_vec(translation(&scene, 1), before);
        close_vec(translation(&scene, 2), Vec2::new(50.0, 30.0));
        assert!(!history.undo(&mut scene));
    }

    #[test]
    fn group_align_moves_root_only_and_marks_dirty() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(20.0, 10.0),
        ));
        scene
            .get_mut(1)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .transform
            .rotation = 0.4;
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(40.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let group_id = scene.group(&[1, 2]).unwrap();
        scene.add_rect(Rectangle::new(
            4,
            Vec2::new(-80.0, 30.0),
            Vec2::new(10.0, 20.0),
        ));
        let before_child = translation(&scene, 1);
        let before_sibling = translation(&scene, 2);
        let before_group = translation(&scene, group_id);
        let before_outside = translation(&scene, 4);
        let rotation = scene
            .get(1)
            .unwrap()
            .as_element()
            .props()
            .transform
            .rotation;

        for id in [1, 2, 4, group_id] {
            scene.get_mut(id).unwrap().as_element_mut().clear_dirty();
        }
        let _ = scene.take_dirty_ids();

        let mut history = CommandHistory::new();
        let selected = [group_id, 1, 4];
        assert!(apply_align(
            &mut history,
            &mut scene,
            &selected,
            AlignMode::Left,
        ));
        assert_eq!(history.undo_count(), 1);
        assert_eq!(scene.parent_of(1), Some(group_id));
        assert_eq!(scene.parent_of(2), Some(group_id));
        assert_eq!(scene.children_of(group_id), vec![1, 2]);
        assert!(scene.parent_of(group_id).is_none());

        let child_delta = translation(&scene, 1) - before_child;
        let sibling_delta = translation(&scene, 2) - before_sibling;
        let group_delta = translation(&scene, group_id) - before_group;
        close_vec(child_delta, group_delta);
        close_vec(sibling_delta, group_delta);
        close(bounds(&scene, group_id).left(), bounds(&scene, 4).left());
        close(translation(&scene, 4).y, before_outside.y);
        assert_eq!(
            scene
                .get(1)
                .unwrap()
                .as_element()
                .props()
                .transform
                .rotation,
            rotation
        );
        assert!(scene.get(1).unwrap().as_element().is_dirty());
        assert!(scene.get(2).unwrap().as_element().is_dirty());
        assert!(scene.get(group_id).unwrap().as_element().is_dirty());
        assert!(scene.has_dirty_elements());

        assert!(history.undo(&mut scene));
        close_vec(translation(&scene, 1), before_child);
        close_vec(translation(&scene, 2), before_sibling);
        close_vec(translation(&scene, group_id), before_group);
        close_vec(translation(&scene, 4), before_outside);
        assert_eq!(scene.parent_of(1), Some(group_id));
        assert!(history.redo(&mut scene));
        close(bounds(&scene, group_id).left(), bounds(&scene, 4).left());
    }

    #[test]
    fn aligning_children_keeps_them_in_the_group() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(30.0, 12.0),
            Vec2::new(8.0, 8.0),
        ));
        let group_id = scene.group(&[1, 2]).unwrap();
        let mut history = CommandHistory::new();
        assert!(apply_align(
            &mut history,
            &mut scene,
            &[1, 2],
            AlignMode::Top,
        ));
        assert_eq!(scene.parent_of(1), Some(group_id));
        assert_eq!(scene.parent_of(2), Some(group_id));
        assert_eq!(scene.children_of(group_id), vec![1, 2]);
        close(bounds(&scene, 1).top(), bounds(&scene, 2).top());
        assert!(history.undo(&mut scene));
        close_vec(translation(&scene, 1), Vec2::ZERO);
        close_vec(translation(&scene, 2), Vec2::new(30.0, 12.0));
        assert_eq!(scene.children_of(group_id), vec![1, 2]);
    }

    #[test]
    fn rotated_scaled_align_uses_world_aabb_and_keeps_orientation() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(20.0, 10.0)));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(40.0, 15.0),
            Vec2::new(12.0, 8.0),
        ));
        {
            let transform = &mut scene
                .get_mut(2)
                .unwrap()
                .as_element_mut()
                .props_mut()
                .transform;
            transform.rotation = 0.6;
            transform.scale = Vec2::new(2.0, 0.5);
        }
        let min_left = bounds(&scene, 1).left().min(bounds(&scene, 2).left());
        let y = translation(&scene, 2).y;
        let mut history = CommandHistory::new();
        assert!(apply_align(
            &mut history,
            &mut scene,
            &[1, 2],
            AlignMode::Left,
        ));
        close(bounds(&scene, 1).left(), min_left);
        close(bounds(&scene, 2).left(), min_left);
        let transform = scene.get(2).unwrap().as_element().props().transform;
        close(transform.rotation, 0.6);
        assert_eq!(transform.scale, Vec2::new(2.0, 0.5));
        close(transform.translation.y, y);
        assert!(scene.get(2).unwrap().as_element().is_dirty());
    }

    #[test]
    fn hidden_or_locked_elements_are_not_separate_targets() {
        let (mut scene, ids) = sample_scene();
        let mut history = CommandHistory::new();
        scene.set_visible(2, false);
        let hidden_at = translation(&scene, 2);
        assert!(apply_align(
            &mut history,
            &mut scene,
            &ids,
            AlignMode::Right,
        ));
        close_vec(translation(&scene, 2), hidden_at);
        close(bounds(&scene, 1).right(), bounds(&scene, 3).right());
        assert!(history.undo(&mut scene));
        scene.set_visible(2, true);

        scene.set_locked(1, true);
        scene.set_locked(3, true);
        assert!(!apply_align(&mut history, &mut scene, &ids, AlignMode::Top,));
        assert_eq!(history.undo_count(), 0);
        close_vec(translation(&scene, 2), hidden_at);
    }

    #[test]
    fn mode_names_round_trip() {
        for mode in [
            AlignMode::Left,
            AlignMode::Center,
            AlignMode::Right,
            AlignMode::Top,
            AlignMode::Middle,
            AlignMode::Bottom,
        ] {
            assert_eq!(AlignMode::parse(mode.as_str()), Some(mode));
        }
        assert_eq!(DistributeAxis::parse("x"), Some(DistributeAxis::Horizontal));
        assert_eq!(DistributeAxis::parse("y"), Some(DistributeAxis::Vertical));
        assert!(AlignMode::parse("").is_none());
        assert!(DistributeAxis::parse("z").is_none());
    }
}
