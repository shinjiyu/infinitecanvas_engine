//! 选区翻转与 90° 旋转。
//!
//! 绕选区中心（单个元素时即元素中心）改 `transform`：平移、旋转，以及翻转时的缩放符号。
//! 选中组只改组根，子孙由 [`Scene::set_world_transform`] 跟着走，不拆组。
//!
//! ```text
//! 选择
//!   │
//!   ▼
//! 可见、未锁定、选中子树的根
//!   │
//!   ├─ 一个目标：绕该元素世界包围盒中心
//!   └─ 多个目标：绕这些包围盒并集的中心
//!         │
//!         ▼
//! OrientSelectionCommand ── set_world_transform ── mark_dirty，一条历史
//! ```

use crate::command::{nudge_targets, Command, CommandHistory};
use crate::element::ElementId;
use crate::math::{Rect, Transform, Vec2};
use crate::Scene;

/// 翻转轴。水平是左右镜像，垂直是上下镜像。世界 Y 向下。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlipAxis {
    /// 绕通过中心的竖线镜像，否定 X。
    Horizontal,
    /// 绕通过中心的横线镜像，否定 Y。
    Vertical,
}

impl FlipAxis {
    /// 解析翻转轴。
    ///
    /// 接受 `horizontal` / `h` / `x` 和 `vertical` / `v` / `y`。
    /// 大小写不敏感，两端空白忽略。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::FlipAxis;
    ///
    /// assert_eq!(FlipAxis::parse(" Horizontal "), Some(FlipAxis::Horizontal));
    /// assert_eq!(FlipAxis::parse("y"), Some(FlipAxis::Vertical));
    /// assert!(FlipAxis::parse("both").is_none());
    /// ```
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "horizontal" | "h" | "x" => Some(Self::Horizontal),
            "vertical" | "v" | "y" => Some(Self::Vertical),
            _ => None,
        }
    }

    /// 稳定的短名字，与 [`FlipAxis::parse`] 对应。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// 绕中心旋转 90°。屏幕 Y 向下，正角度是顺时针。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuarterTurn {
    /// 顺时针 90°（+π/2）。
    Clockwise,
    /// 逆时针 90°（-π/2）。
    CounterClockwise,
}

impl QuarterTurn {
    /// 解析转向。
    ///
    /// 接受 `cw` / `clockwise` 和 `ccw` / `counterclockwise` / `counter-clockwise`。
    /// 大小写不敏感，两端空白忽略。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::QuarterTurn;
    ///
    /// assert_eq!(QuarterTurn::parse("cw"), Some(QuarterTurn::Clockwise));
    /// assert_eq!(
    ///     QuarterTurn::parse("counter-clockwise"),
    ///     Some(QuarterTurn::CounterClockwise)
    /// );
    /// assert!(QuarterTurn::parse("180").is_none());
    /// ```
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "cw" | "clockwise" => Some(Self::Clockwise),
            "ccw" | "counterclockwise" | "counter-clockwise" => Some(Self::CounterClockwise),
            _ => None,
        }
    }

    /// 稳定的短名字，与 [`QuarterTurn::parse`] 对应。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clockwise => "cw",
            Self::CounterClockwise => "ccw",
        }
    }

    /// 加到 `rotation` 上的弧度。顺时针为正。
    pub fn radians(self) -> f32 {
        match self {
            Self::Clockwise => std::f32::consts::FRAC_PI_2,
            Self::CounterClockwise => -std::f32::consts::FRAC_PI_2,
        }
    }
}

/// 对当前选择做一次翻转，并记入一条撤销记录。
///
/// 空选、全部锁定或全部不可见时不入栈。选中组时只写组根，子孙保持父子关系。
/// 只改平移、旋转和缩放符号。
///
/// # 参数
/// - `history`: 撤销栈
/// - `scene`: 要修改的场景
/// - `selected`: 当前选中的元素 id
/// - `axis`: 水平或垂直
///
/// # 返回
/// 是否写入了撤销栈
///
/// # 示例
/// ```
/// use infinitecanvas_engine::math::Vec2;
/// use infinitecanvas_engine::{
///     apply_flip, CommandHistory, Element, FlipAxis, Rectangle, Scene,
/// };
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(1, Vec2::new(10.0, 4.0), Vec2::new(6.0, 4.0)));
/// let mut history = CommandHistory::new();
/// assert!(apply_flip(
///     &mut history,
///     &mut scene,
///     &[1],
///     FlipAxis::Horizontal,
/// ));
/// assert!(scene.get(1).unwrap().as_element().props().transform.scale.x < 0.0);
/// assert!(history.undo(&mut scene));
/// assert!(scene.get(1).unwrap().as_element().props().transform.scale.x > 0.0);
/// assert!(!apply_flip(&mut history, &mut scene, &[], FlipAxis::Vertical));
/// ```
pub fn apply_flip(
    history: &mut CommandHistory,
    scene: &mut Scene,
    selected: &[ElementId],
    axis: FlipAxis,
) -> bool {
    let Some(command) = OrientSelectionCommand::flip(scene, selected, axis) else {
        return false;
    };
    history.execute(Box::new(command), scene);
    true
}

/// 对当前选择顺时针或逆时针转 90°，并记入一条撤销记录。
///
/// 空选、全部锁定或全部不可见时不入栈。缩放向量保持不变，包括已经为负的分量。
///
/// # 参数
/// - `history`: 撤销栈
/// - `scene`: 要修改的场景
/// - `selected`: 当前选中的元素 id
/// - `turn`: 顺时针或逆时针
///
/// # 返回
/// 是否写入了撤销栈
///
/// # 示例
/// ```
/// use infinitecanvas_engine::math::Vec2;
/// use infinitecanvas_engine::{
///     apply_rotate_quarter, CommandHistory, Element, QuarterTurn, Rectangle, Scene,
/// };
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(8.0, 4.0)));
/// let mut history = CommandHistory::new();
/// assert!(apply_rotate_quarter(
///     &mut history,
///     &mut scene,
///     &[1],
///     QuarterTurn::Clockwise,
/// ));
/// let rotation = scene.get(1).unwrap().as_element().props().transform.rotation;
/// assert!((rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
/// assert!(history.undo(&mut scene));
/// assert!(!apply_rotate_quarter(
///     &mut history,
///     &mut scene,
///     &[],
///     QuarterTurn::CounterClockwise,
/// ));
/// ```
pub fn apply_rotate_quarter(
    history: &mut CommandHistory,
    scene: &mut Scene,
    selected: &[ElementId],
    turn: QuarterTurn,
) -> bool {
    let Some(command) = OrientSelectionCommand::quarter_turn(scene, selected, turn) else {
        return false;
    };
    history.execute(Box::new(command), scene);
    true
}

/// 一次翻转或 90° 旋转。撤销恢复每个目标翻转/旋转前的世界变换。
///
/// 列表里只有选中子树的根。组的子孙不在列表里，执行时由场景按组根的增量改写。
#[derive(Debug)]
pub struct OrientSelectionCommand {
    roots: Vec<(ElementId, Transform)>,
    next: Vec<(ElementId, Transform)>,
    label: &'static str,
}

impl OrientSelectionCommand {
    /// 按当前选择构造一条尚未执行的翻转命令。没有可变换目标时返回 `None`。
    pub fn flip(scene: &Scene, selected: &[ElementId], axis: FlipAxis) -> Option<Self> {
        Self::from_selection(scene, selected, "FlipSelection", |transform, pivot| {
            flip_transform(transform, pivot, axis)
        })
    }

    /// 按当前选择构造一条尚未执行的 90° 旋转命令。没有可变换目标时返回 `None`。
    pub fn quarter_turn(scene: &Scene, selected: &[ElementId], turn: QuarterTurn) -> Option<Self> {
        Self::from_selection(scene, selected, "RotateSelection", |transform, pivot| {
            rotate_quarter(transform, pivot, turn)
        })
    }

    fn from_selection(
        scene: &Scene,
        selected: &[ElementId],
        label: &'static str,
        map: impl Fn(Transform, Vec2) -> Transform,
    ) -> Option<Self> {
        let ids = nudge_targets(scene, selected);
        let pivot = selection_pivot(scene, &ids)?;
        let mut roots = Vec::with_capacity(ids.len());
        let mut next = Vec::with_capacity(ids.len());
        for id in ids {
            let transform = scene.get(id)?.as_element().props().transform;
            let updated = map(transform, pivot);
            if transform == updated {
                continue;
            }
            roots.push((id, transform));
            next.push((id, updated));
        }
        if roots.is_empty() {
            None
        } else {
            Some(Self { roots, next, label })
        }
    }
}

impl Command for OrientSelectionCommand {
    fn execute(&mut self, scene: &mut Scene) {
        // 只提交根。组根走 set_world_transform，子孙世界变换跟着变，父子关系不动。
        scene.apply_world_transforms(&self.next);
    }

    fn undo(&mut self, scene: &mut Scene) {
        scene.apply_world_transforms(&self.roots);
    }

    fn name(&self) -> &'static str {
        self.label
    }
}

/// 绕 `pivot` 做左右或上下镜像。
///
/// 平移是原点绕轴的反射。旋转取反，使朝向跟着镜像。
/// 水平翻转否定 `scale.x`，垂直翻转否定 `scale.y`。另一个缩放分量不动，
/// 因此 `|scale.x| / |scale.y|` 不变，图片的显示宽高比保持原样。
fn flip_transform(transform: Transform, pivot: Vec2, axis: FlipAxis) -> Transform {
    let (translation, scale) = match axis {
        FlipAxis::Horizontal => (
            Vec2::new(
                pivot.x * 2.0 - transform.translation.x,
                transform.translation.y,
            ),
            Vec2::new(-transform.scale.x, transform.scale.y),
        ),
        FlipAxis::Vertical => (
            Vec2::new(
                transform.translation.x,
                pivot.y * 2.0 - transform.translation.y,
            ),
            Vec2::new(transform.scale.x, -transform.scale.y),
        ),
    };
    Transform {
        translation,
        rotation: -transform.rotation,
        scale,
    }
}

/// 绕 `pivot` 转 90°。缩放原样保留，包括符号。
fn rotate_quarter(transform: Transform, pivot: Vec2, turn: QuarterTurn) -> Transform {
    let delta = turn.radians();
    let offset = transform.translation - pivot;
    let (sin, cos) = delta.sin_cos();
    Transform {
        translation: pivot
            + Vec2::new(
                cos * offset.x - sin * offset.y,
                sin * offset.x + cos * offset.y,
            ),
        rotation: transform.rotation + delta,
        scale: transform.scale,
    }
}

/// 目标世界包围盒的中心。一个元素时是它自己的中心，多个时是并集的中心。
fn selection_pivot(scene: &Scene, ids: &[ElementId]) -> Option<Vec2> {
    let mut union: Option<Rect> = None;
    for id in ids {
        let bounds = scene.get(*id)?.as_element().world_bounds();
        union = Some(match union {
            Some(existing) => existing.union(&bounds),
            None => bounds,
        });
    }
    union.map(|bounds| bounds.center())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::Image;
    use crate::{AnyElement, Rectangle};

    fn approx(actual: Vec2, expected: Vec2) {
        assert!(
            (actual - expected).length() < 1e-3,
            "actual {actual:?} expected {expected:?}"
        );
    }

    fn transform_of(scene: &Scene, id: ElementId) -> Transform {
        scene.get(id).unwrap().as_element().props().transform
    }

    fn world_point(scene: &Scene, id: ElementId, local: Vec2) -> Vec2 {
        transform_of(scene, id).transform_point(local)
    }

    fn rect(id: ElementId, center: Vec2) -> Rectangle {
        Rectangle::new(id, center, Vec2::new(10.0, 10.0))
    }

    #[test]
    fn single_selection_flips_and_rotates_around_its_center() {
        let mut scene = Scene::new();
        let mut shape = Rectangle::new(1, Vec2::new(30.0, 40.0), Vec2::new(20.0, 10.0));
        shape.props.set_rotation(0.3);
        shape.props.transform.scale = Vec2::new(1.5, 0.8);
        scene.add_rect(shape);
        let original = transform_of(&scene, 1);
        let mut history = CommandHistory::new();
        let _ = scene.take_dirty_ids();

        assert!(apply_flip(
            &mut history,
            &mut scene,
            &[1],
            FlipAxis::Horizontal,
        ));
        assert_eq!(history.undo_count(), 1);
        let flipped = transform_of(&scene, 1);
        approx(flipped.translation, original.translation);
        assert!((flipped.rotation + original.rotation).abs() < 1e-4);
        approx(flipped.scale, Vec2::new(-1.5, 0.8));
        let right = world_point(&scene, 1, Vec2::new(10.0, 0.0));
        let before_right = original.transform_point(Vec2::new(10.0, 0.0));
        assert!((right.x - (60.0 - before_right.x)).abs() < 1e-3);
        assert!((right.y - before_right.y).abs() < 1e-3);
        assert!(scene.has_dirty_elements());

        assert!(history.undo(&mut scene));
        assert_eq!(transform_of(&scene, 1), original);
        assert!(history.redo(&mut scene));
        assert_eq!(transform_of(&scene, 1).scale, Vec2::new(-1.5, 0.8));
        history.undo(&mut scene);

        assert!(apply_flip(
            &mut history,
            &mut scene,
            &[1],
            FlipAxis::Vertical,
        ));
        let vertical = transform_of(&scene, 1);
        approx(vertical.translation, original.translation);
        assert!((vertical.rotation + 0.3).abs() < 1e-4);
        approx(vertical.scale, Vec2::new(1.5, -0.8));
        history.undo(&mut scene);

        assert!(apply_rotate_quarter(
            &mut history,
            &mut scene,
            &[1],
            QuarterTurn::Clockwise,
        ));
        let clockwise = transform_of(&scene, 1);
        approx(clockwise.translation, original.translation);
        assert!((clockwise.rotation - (0.3 + std::f32::consts::FRAC_PI_2)).abs() < 1e-4);
        approx(clockwise.scale, original.scale);
        assert!(history.undo(&mut scene));
        assert!(history.redo(&mut scene));
        assert_eq!(transform_of(&scene, 1).scale, original.scale);
        history.undo(&mut scene);

        assert!(apply_rotate_quarter(
            &mut history,
            &mut scene,
            &[1],
            QuarterTurn::CounterClockwise,
        ));
        let counter = transform_of(&scene, 1);
        approx(counter.translation, original.translation);
        assert!((counter.rotation - (0.3 - std::f32::consts::FRAC_PI_2)).abs() < 1e-4);
        approx(counter.scale, original.scale);
        assert_eq!(history.undo_count(), 1);
        assert!(history.undo(&mut scene));
        assert_eq!(transform_of(&scene, 1), original);
    }

    #[test]
    fn flip_and_quarter_turn_keep_corner_radius_on_the_rectangle() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::new(20.0, 10.0), Vec2::new(40.0, 20.0)).with_corner_radius(6.0),
        );
        let mut history = CommandHistory::new();

        assert!(apply_flip(
            &mut history,
            &mut scene,
            &[1],
            FlipAxis::Horizontal,
        ));
        match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.corner_radius(), 6.0);
                assert!(rect.generate_fill_vertices().len() > 6);
                assert!(rect.props.transform.scale.x < 0.0);
            }
            other => panic!("rect, got {other:?}"),
        }

        assert!(apply_rotate_quarter(
            &mut history,
            &mut scene,
            &[1],
            QuarterTurn::Clockwise,
        ));
        match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.corner_radius(), 6.0);
                assert!(rect.generate_fill_vertices().len() > 6);
            }
            other => panic!("rect, got {other:?}"),
        }

        assert!(history.undo(&mut scene));
        match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => assert_eq!(rect.corner_radius(), 6.0),
            other => panic!("rect, got {other:?}"),
        }
    }

    #[test]
    fn multi_selection_uses_the_union_center_and_one_history_entry() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(100.0, 0.0)));
        scene.add_rect(rect(3, Vec2::new(400.0, 40.0)));
        let mut history = CommandHistory::new();

        assert!(apply_flip(
            &mut history,
            &mut scene,
            &[1, 2],
            FlipAxis::Horizontal,
        ));
        assert_eq!(history.undo_count(), 1);
        approx(transform_of(&scene, 1).translation, Vec2::new(100.0, 0.0));
        approx(transform_of(&scene, 2).translation, Vec2::ZERO);
        assert!(transform_of(&scene, 1).scale.x < 0.0);
        assert!(transform_of(&scene, 2).scale.x < 0.0);
        approx(transform_of(&scene, 3).translation, Vec2::new(400.0, 40.0));
        assert!(history.undo(&mut scene));
        approx(transform_of(&scene, 1).translation, Vec2::ZERO);
        assert!(history.redo(&mut scene));
        approx(transform_of(&scene, 1).translation, Vec2::new(100.0, 0.0));
        history.undo(&mut scene);

        scene.add_rect(rect(4, Vec2::new(0.0, 80.0)));
        assert!(apply_flip(
            &mut history,
            &mut scene,
            &[1, 4],
            FlipAxis::Vertical,
        ));
        assert_eq!(history.undo_count(), 1);
        approx(transform_of(&scene, 1).translation, Vec2::new(0.0, 80.0));
        approx(transform_of(&scene, 4).translation, Vec2::ZERO);
        assert!(transform_of(&scene, 1).scale.y < 0.0);
        assert!(transform_of(&scene, 4).scale.y < 0.0);
        history.undo(&mut scene);

        assert!(apply_rotate_quarter(
            &mut history,
            &mut scene,
            &[1, 2],
            QuarterTurn::Clockwise,
        ));
        approx(transform_of(&scene, 1).translation, Vec2::new(50.0, -50.0));
        approx(transform_of(&scene, 2).translation, Vec2::new(50.0, 50.0));
        assert!((transform_of(&scene, 1).rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        approx(transform_of(&scene, 1).scale, Vec2::ONE);
        approx(transform_of(&scene, 3).translation, Vec2::new(400.0, 40.0));
        assert!(history.undo(&mut scene));
        approx(transform_of(&scene, 1).translation, Vec2::ZERO);
        assert!(history.redo(&mut scene));
        approx(transform_of(&scene, 2).translation, Vec2::new(50.0, 50.0));
        history.undo(&mut scene);

        assert!(apply_rotate_quarter(
            &mut history,
            &mut scene,
            &[1, 2],
            QuarterTurn::CounterClockwise,
        ));
        approx(transform_of(&scene, 1).translation, Vec2::new(50.0, 50.0));
        approx(transform_of(&scene, 2).translation, Vec2::new(50.0, -50.0));
        assert!((transform_of(&scene, 1).rotation + std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        assert_eq!(history.undo_count(), 1);
        assert!(history.undo(&mut scene));
        approx(transform_of(&scene, 2).translation, Vec2::new(100.0, 0.0));
    }

    #[test]
    fn group_selection_orients_the_root_without_ungrouping() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(40.0, 0.0)));
        let group_id = scene.group(&[1, 2]).unwrap();
        let pivot = transform_of(&scene, group_id).translation;
        approx(pivot, Vec2::new(20.0, 0.0));
        let mut history = CommandHistory::new();

        #[derive(Clone, Copy)]
        enum Action {
            Flip(FlipAxis),
            Turn(QuarterTurn),
        }
        let actions = [
            Action::Flip(FlipAxis::Horizontal),
            Action::Flip(FlipAxis::Vertical),
            Action::Turn(QuarterTurn::Clockwise),
            Action::Turn(QuarterTurn::CounterClockwise),
        ];
        for action in actions {
            let before_child = world_point(&scene, 1, Vec2::new(5.0, 1.0));
            let before_sibling = world_point(&scene, 2, Vec2::new(-5.0, -1.0));
            let before_group = transform_of(&scene, group_id);
            let _ = scene.take_dirty_ids();
            let changed = match action {
                Action::Flip(axis) => apply_flip(&mut history, &mut scene, &[group_id, 1], axis),
                Action::Turn(turn) => {
                    apply_rotate_quarter(&mut history, &mut scene, &[group_id, 1], turn)
                }
            };
            assert!(changed);
            assert_eq!(history.undo_count(), 1);
            assert_eq!(scene.children_of(group_id), vec![1, 2]);
            assert_eq!(scene.parent_of(1), Some(group_id));
            assert_eq!(scene.parent_of(2), Some(group_id));
            assert_ne!(transform_of(&scene, group_id), before_group);
            let dirty = scene.take_dirty_ids();
            assert!(dirty.contains(&group_id) && dirty.contains(&1) && dirty.contains(&2));

            assert!(history.undo(&mut scene));
            approx(world_point(&scene, 1, Vec2::new(5.0, 1.0)), before_child);
            approx(
                world_point(&scene, 2, Vec2::new(-5.0, -1.0)),
                before_sibling,
            );
            assert_eq!(scene.parent_of(1), Some(group_id));
            assert!(history.redo(&mut scene));
            assert_eq!(scene.children_of(group_id), vec![1, 2]);
            history.undo(&mut scene);
            assert!(!history.can_undo());
        }

        assert!(apply_flip(
            &mut history,
            &mut scene,
            &[group_id],
            FlipAxis::Horizontal,
        ));
        approx(world_point(&scene, 1, Vec2::ZERO), Vec2::new(40.0, 0.0));
        approx(world_point(&scene, 2, Vec2::ZERO), Vec2::ZERO);
        let mirrored = world_point(&scene, 1, Vec2::new(5.0, 1.0));
        assert!((mirrored.x - 35.0).abs() < 1e-2);
        assert!((mirrored.y - 1.0).abs() < 1e-2);
        history.undo(&mut scene);

        assert!(apply_rotate_quarter(
            &mut history,
            &mut scene,
            &[group_id],
            QuarterTurn::Clockwise,
        ));
        approx(world_point(&scene, 1, Vec2::ZERO), Vec2::new(20.0, -20.0));
        approx(world_point(&scene, 2, Vec2::ZERO), Vec2::new(20.0, 20.0));
        assert_eq!(scene.children_of(group_id), vec![1, 2]);
    }

    #[test]
    fn empty_locked_or_hidden_selection_is_noop() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::new(6.0, 7.0)));
        let original = transform_of(&scene, 1);
        let mut history = CommandHistory::new();

        assert!(!apply_flip(
            &mut history,
            &mut scene,
            &[],
            FlipAxis::Horizontal
        ));
        assert!(!apply_flip(
            &mut history,
            &mut scene,
            &[],
            FlipAxis::Vertical
        ));
        assert!(!apply_rotate_quarter(
            &mut history,
            &mut scene,
            &[],
            QuarterTurn::Clockwise,
        ));
        assert!(!apply_rotate_quarter(
            &mut history,
            &mut scene,
            &[],
            QuarterTurn::CounterClockwise,
        ));
        assert!(OrientSelectionCommand::flip(&scene, &[], FlipAxis::Horizontal).is_none());
        assert_eq!(history.undo_count(), 0);
        assert_eq!(transform_of(&scene, 1), original);

        scene.set_locked(1, true);
        assert!(!apply_flip(
            &mut history,
            &mut scene,
            &[1],
            FlipAxis::Vertical,
        ));
        assert!(!apply_rotate_quarter(
            &mut history,
            &mut scene,
            &[1],
            QuarterTurn::Clockwise,
        ));
        assert_eq!(transform_of(&scene, 1), original);
        scene.set_locked(1, false);

        scene.set_visible(1, false);
        assert!(!apply_flip(
            &mut history,
            &mut scene,
            &[1],
            FlipAxis::Horizontal,
        ));
        assert!(!apply_rotate_quarter(
            &mut history,
            &mut scene,
            &[1],
            QuarterTurn::CounterClockwise,
        ));
        assert_eq!(history.undo_count(), 0);
        assert_eq!(transform_of(&scene, 1), original);
        assert!(!history.can_undo());
    }

    #[test]
    fn image_flip_keeps_size_and_aspect_ratio() {
        let mut scene = Scene::new();
        let mut image = Image::new(
            1,
            Vec2::new(10.0, 20.0),
            Vec2::new(80.0, 40.0),
            "badge".to_string(),
        );
        image.props.transform.scale = Vec2::new(2.0, 2.0);
        scene.add_image(image);
        let mut history = CommandHistory::new();

        assert!(apply_flip(
            &mut history,
            &mut scene,
            &[1],
            FlipAxis::Horizontal,
        ));
        let image = match scene.get(1).unwrap() {
            AnyElement::Image(image) => image,
            _ => panic!("image"),
        };
        assert_eq!(image.size, Vec2::new(80.0, 40.0));
        let scale = image.props.transform.scale;
        assert!(scale.x < 0.0);
        assert!((scale.x.abs() - 2.0).abs() < 1e-4);
        assert!((scale.y - 2.0).abs() < 1e-4);
        let aspect = scale.x.abs() * image.size.x / (scale.y.abs() * image.size.y);
        assert!((aspect - 2.0).abs() < 1e-4);
        approx(
            image.props.transform.transform_point(Vec2::new(40.0, 20.0)),
            Vec2::new(90.0, 60.0),
        );
        let top_left = image.props.transform.transform_point(Vec2::ZERO);
        approx(top_left, Vec2::new(170.0, 20.0));

        assert!(history.undo(&mut scene));
        let restored = match scene.get(1).unwrap() {
            AnyElement::Image(image) => image,
            _ => panic!("image"),
        };
        assert_eq!(restored.size, Vec2::new(80.0, 40.0));
        approx(restored.props.transform.scale, Vec2::new(2.0, 2.0));
        approx(restored.props.transform.translation, Vec2::new(10.0, 20.0));

        assert!(apply_flip(
            &mut history,
            &mut scene,
            &[1],
            FlipAxis::Vertical,
        ));
        let image = match scene.get(1).unwrap() {
            AnyElement::Image(image) => image,
            _ => panic!("image"),
        };
        assert_eq!(image.size, Vec2::new(80.0, 40.0));
        assert!(image.props.transform.scale.y < 0.0);
        assert!((image.props.transform.scale.x - 2.0).abs() < 1e-4);
        assert!((image.props.transform.scale.y.abs() - 2.0).abs() < 1e-4);
        approx(
            image.props.transform.transform_point(Vec2::new(40.0, 20.0)),
            Vec2::new(90.0, 60.0),
        );
    }

    #[test]
    fn parse_accepts_short_names() {
        assert_eq!(FlipAxis::parse("x"), Some(FlipAxis::Horizontal));
        assert_eq!(FlipAxis::parse("vertical"), Some(FlipAxis::Vertical));
        assert!(FlipAxis::parse("diagonal").is_none());
        assert_eq!(FlipAxis::Horizontal.as_str(), "horizontal");
        assert_eq!(
            QuarterTurn::parse("clockwise"),
            Some(QuarterTurn::Clockwise)
        );
        assert_eq!(
            QuarterTurn::parse("CCW"),
            Some(QuarterTurn::CounterClockwise)
        );
        assert!(QuarterTurn::parse("left").is_none());
        assert_eq!(QuarterTurn::Clockwise.as_str(), "cw");
        assert!(
            (QuarterTurn::CounterClockwise.radians() + std::f32::consts::FRAC_PI_2).abs() < 1e-6
        );
    }
}
