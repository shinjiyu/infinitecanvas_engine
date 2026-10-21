//! 属性面板数值化变换（X / Y / W / H / 旋转）。
//!
//! X/Y 是元素本地原点的世界坐标（`transform.translation`），与变换手柄一致：
//! 矩形 / 圆 / 多边形 / 星形 / 图片 / 画板的局部包围盒以原点为中心，因此
//! translation 等于选中框中心（旋转手柄的枢轴）。直线的原点是起点；文本原点
//! 随对齐方式变化。
//!
//! W/H 是世界尺寸 `|local_bounds| * |scale|`，与选中框半宽半高的两倍相同。
//! 写入时只改 `scale`（保留符号），与缩放手柄同一路径。直线没有可编辑宽高，
//! `size_supported` 为 false。
//!
//! 旋转角与 `transform.rotation` / 旋转手柄共用同一字段；面板用角度，引擎存弧度。
//!
//! ```text
//! 属性面板数字框
//!     │ get_selection_transform / set_selection_transform
//!     ▼
//! selection_transform / apply_selection_transform
//!     │
//!     └─ TransformElementsCommand ──► Scene::apply_world_transforms / mark_dirty
//! ```

use crate::command::{nudge_targets, CommandHistory, TransformElementsCommand};
use crate::element::{Element, ElementId};
use crate::math::{Transform, Vec2};
use crate::scene::Scene;
use crate::style::StyleField;

const VALUE_EPS: f32 = 1e-4;
const LOCAL_SIZE_EPS: f32 = 1e-4;

/// 一次数值变换补丁。`None` 表示该分量不动。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TransformNumericPatch {
    /// 本地原点世界 X。
    pub x: Option<f32>,
    /// 本地原点世界 Y。
    pub y: Option<f32>,
    /// 世界宽度（正有限）。直线忽略。
    pub width: Option<f32>,
    /// 世界高度（正有限）。直线忽略。
    pub height: Option<f32>,
    /// 旋转角（度）。与旋转手柄同一字段。
    pub rotation_deg: Option<f32>,
}

impl TransformNumericPatch {
    /// 没有任何分量。
    pub fn is_empty(self) -> bool {
        self.x.is_none()
            && self.y.is_none()
            && self.width.is_none()
            && self.height.is_none()
            && self.rotation_deg.is_none()
    }

    /// 校验已写出的分量。非有限或非正宽高返回 `false`。
    pub fn is_valid(self) -> bool {
        for value in [self.x, self.y, self.rotation_deg].into_iter().flatten() {
            if !value.is_finite() {
                return false;
            }
        }
        for value in [self.width, self.height].into_iter().flatten() {
            if !value.is_finite() || value <= 0.0 {
                return false;
            }
        }
        true
    }
}

/// 当前选择的公共变换数值。
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionTransform {
    /// 至少有一个可编辑目标（未锁定的选中根）。
    pub supported: bool,
    /// 选择里有可改宽高的目标（非直线）。
    pub size_supported: bool,
    /// 本地原点 X。
    pub x: StyleField<f32>,
    /// 本地原点 Y。
    pub y: StyleField<f32>,
    /// 世界宽度。没有可改尺寸的目标时为 [`StyleField::Absent`]。
    pub width: StyleField<f32>,
    /// 世界高度。
    pub height: StyleField<f32>,
    /// 旋转角（度）。
    pub rotation_deg: StyleField<f32>,
}

impl SelectionTransform {
    /// 空选或全部锁定。
    pub fn unsupported() -> Self {
        Self {
            supported: false,
            size_supported: false,
            x: StyleField::Absent,
            y: StyleField::Absent,
            width: StyleField::Absent,
            height: StyleField::Absent,
            rotation_deg: StyleField::Absent,
        }
    }
}

/// 读取当前选择的变换数值。目标与方向键微调相同（选中子树根，跳过锁定）。
pub fn selection_transform(scene: &Scene, selected: &[ElementId]) -> SelectionTransform {
    let ids = nudge_targets(scene, selected);
    if ids.is_empty() {
        return SelectionTransform::unsupported();
    }

    let mut xs = Vec::with_capacity(ids.len());
    let mut ys = Vec::with_capacity(ids.len());
    let mut rotations = Vec::with_capacity(ids.len());
    let mut widths = Vec::new();
    let mut heights = Vec::new();

    for id in &ids {
        let Some(element) = scene.get(*id) else {
            continue;
        };
        let element = element.as_element();
        let transform = element.props().transform;
        xs.push(transform.translation.x);
        ys.push(transform.translation.y);
        rotations.push(transform.rotation.to_degrees());
        if element_has_size(element) {
            let size = world_size(element);
            widths.push(size.x);
            heights.push(size.y);
        }
    }

    if xs.is_empty() {
        return SelectionTransform::unsupported();
    }

    SelectionTransform {
        supported: true,
        size_supported: !widths.is_empty(),
        x: uniform_field(&xs),
        y: uniform_field(&ys),
        width: if widths.is_empty() {
            StyleField::Absent
        } else {
            uniform_field(&widths)
        },
        height: if heights.is_empty() {
            StyleField::Absent
        } else {
            uniform_field(&heights)
        },
        rotation_deg: uniform_field(&rotations),
    }
}

/// 把补丁写到当前选择，并记入一条 [`TransformElementsCommand`]。
///
/// 空选、全部锁定、非法输入、或没有任何实际变化时返回 `false`，不入栈。
/// 多选时每个根写入同一绝对值（例如大家都把 X 设成 100）。直线忽略 W/H。
/// 成功时由 [`Scene::apply_world_transforms`] 标脏。
pub fn apply_selection_transform(
    history: &mut CommandHistory,
    scene: &mut Scene,
    selected: &[ElementId],
    patch: TransformNumericPatch,
) -> bool {
    if patch.is_empty() || !patch.is_valid() {
        return false;
    }
    let Some(command) = TransformElementsCommand::from_numeric_patch(scene, selected, patch) else {
        return false;
    };
    history.execute(Box::new(command), scene);
    true
}

impl TransformElementsCommand {
    /// 按数值补丁构造尚未执行的变换命令。没有可写目标时返回 `None`。
    pub fn from_numeric_patch(
        scene: &Scene,
        selected: &[ElementId],
        patch: TransformNumericPatch,
    ) -> Option<Self> {
        if patch.is_empty() || !patch.is_valid() {
            return None;
        }
        let ids = nudge_targets(scene, selected);
        let mut before = Vec::new();
        let mut after = Vec::new();
        for id in ids {
            let Some(element) = scene.get(id) else {
                continue;
            };
            let element = element.as_element();
            let current = element.props().transform;
            let Some(next) = patched_transform(element, patch) else {
                continue;
            };
            if transforms_close(current, next) {
                continue;
            }
            before.push((id, current));
            after.push((id, next));
        }
        if before.is_empty() {
            return None;
        }
        Some(Self::new(before, after))
    }
}

fn patched_transform(element: &dyn Element, patch: TransformNumericPatch) -> Option<Transform> {
    let mut transform = element.props().transform;
    let local = element.local_bounds();
    let has_size = element_has_size(element);

    if let Some(x) = patch.x {
        transform.translation.x = x;
    }
    if let Some(y) = patch.y {
        transform.translation.y = y;
    }
    if has_size {
        if let Some(width) = patch.width {
            let local_w = local.width.abs();
            if local_w > LOCAL_SIZE_EPS {
                transform.scale.x = signed_scale(transform.scale.x, width / local_w);
            }
        }
        if let Some(height) = patch.height {
            let local_h = local.height.abs();
            if local_h > LOCAL_SIZE_EPS {
                transform.scale.y = signed_scale(transform.scale.y, height / local_h);
            }
        }
    }
    if let Some(degrees) = patch.rotation_deg {
        transform.rotation = degrees.to_radians();
    }
    Some(transform)
}

fn element_has_size(element: &dyn Element) -> bool {
    element.type_name() != "Line"
}

fn world_size(element: &dyn Element) -> Vec2 {
    let local = element.local_bounds();
    let transform = element.props().transform;
    Vec2::new(
        local.width.abs() * transform.scale.x.abs(),
        local.height.abs() * transform.scale.y.abs(),
    )
}

fn signed_scale(current: f32, magnitude: f32) -> f32 {
    let sign = if current < 0.0 { -1.0 } else { 1.0 };
    sign * magnitude.max(LOCAL_SIZE_EPS)
}

fn transforms_close(left: Transform, right: Transform) -> bool {
    (left.translation.x - right.translation.x).abs() <= VALUE_EPS
        && (left.translation.y - right.translation.y).abs() <= VALUE_EPS
        && (left.rotation - right.rotation).abs() <= VALUE_EPS
        && (left.scale.x - right.scale.x).abs() <= VALUE_EPS
        && (left.scale.y - right.scale.y).abs() <= VALUE_EPS
}

fn uniform_field(values: &[f32]) -> StyleField<f32> {
    let Some(first) = values.first().copied() else {
        return StyleField::Absent;
    };
    if values
        .iter()
        .all(|value| (*value - first).abs() <= VALUE_EPS)
    {
        StyleField::Uniform(first)
    } else {
        StyleField::Mixed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{ElementStyle, Line, Rectangle};
    use crate::math::Vec2;

    fn rect_scene() -> (Scene, CommandHistory) {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(10.0, 20.0),
            Vec2::new(40.0, 30.0),
        ));
        (scene, CommandHistory::new())
    }

    #[test]
    fn single_rect_reads_origin_size_and_rotation() {
        let (mut scene, _) = rect_scene();
        scene
            .get_mut(1)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .set_rotation(std::f32::consts::FRAC_PI_2);
        let summary = selection_transform(&scene, &[1]);
        assert!(summary.supported);
        assert!(summary.size_supported);
        assert_eq!(summary.x, StyleField::Uniform(10.0));
        assert_eq!(summary.y, StyleField::Uniform(20.0));
        assert_eq!(summary.width, StyleField::Uniform(40.0));
        assert_eq!(summary.height, StyleField::Uniform(30.0));
        assert_eq!(summary.rotation_deg, StyleField::Uniform(90.0));
    }

    #[test]
    fn set_xy_wh_rotation_undoes_and_marks_dirty() {
        let (mut scene, mut history) = rect_scene();
        assert!(apply_selection_transform(
            &mut history,
            &mut scene,
            &[1],
            TransformNumericPatch {
                x: Some(100.0),
                y: Some(50.0),
                width: Some(80.0),
                height: Some(60.0),
                rotation_deg: Some(45.0),
            },
        ));
        let element = scene.get(1).unwrap().as_element();
        let transform = element.props().transform;
        assert!((transform.translation.x - 100.0).abs() < 1e-4);
        assert!((transform.translation.y - 50.0).abs() < 1e-4);
        assert!((world_size(element).x - 80.0).abs() < 1e-3);
        assert!((world_size(element).y - 60.0).abs() < 1e-3);
        assert!((transform.rotation.to_degrees() - 45.0).abs() < 1e-3);
        assert!(element.props().is_dirty());

        assert!(history.undo(&mut scene));
        let restored = scene.get(1).unwrap().as_element().props().transform;
        assert!((restored.translation.x - 10.0).abs() < 1e-4);
        assert!((restored.translation.y - 20.0).abs() < 1e-4);
        assert!((restored.scale.x - 1.0).abs() < 1e-4);
        assert!((restored.rotation).abs() < 1e-4);
    }

    #[test]
    fn empty_locked_invalid_are_noop() {
        let (mut scene, mut history) = rect_scene();
        assert!(!apply_selection_transform(
            &mut history,
            &mut scene,
            &[],
            TransformNumericPatch {
                x: Some(1.0),
                ..Default::default()
            },
        ));
        scene.set_locked(1, true);
        assert!(!apply_selection_transform(
            &mut history,
            &mut scene,
            &[1],
            TransformNumericPatch {
                x: Some(1.0),
                ..Default::default()
            },
        ));
        scene.set_locked(1, false);
        assert!(!apply_selection_transform(
            &mut history,
            &mut scene,
            &[1],
            TransformNumericPatch {
                width: Some(-4.0),
                ..Default::default()
            },
        ));
        assert!(!apply_selection_transform(
            &mut history,
            &mut scene,
            &[1],
            TransformNumericPatch {
                x: Some(f32::NAN),
                ..Default::default()
            },
        ));
        assert!(!apply_selection_transform(
            &mut history,
            &mut scene,
            &[1],
            TransformNumericPatch {
                x: Some(10.0),
                ..Default::default()
            },
        ));
        assert!(!history.can_undo());
    }

    #[test]
    fn line_size_not_supported_and_wh_ignored() {
        let mut scene = Scene::new();
        scene.add_line(Line::from_world(
            2,
            Vec2::ZERO,
            Vec2::new(100.0, 0.0),
            ElementStyle::default(),
            false,
        ));
        let summary = selection_transform(&scene, &[2]);
        assert!(summary.supported);
        assert!(!summary.size_supported);
        assert_eq!(summary.width, StyleField::Absent);
        assert_eq!(summary.height, StyleField::Absent);

        let mut history = CommandHistory::new();
        let before = scene.get(2).unwrap().as_element().props().transform;
        assert!(!apply_selection_transform(
            &mut history,
            &mut scene,
            &[2],
            TransformNumericPatch {
                width: Some(50.0),
                height: Some(50.0),
                ..Default::default()
            },
        ));
        assert_eq!(scene.get(2).unwrap().as_element().props().transform, before);
        assert!(apply_selection_transform(
            &mut history,
            &mut scene,
            &[2],
            TransformNumericPatch {
                x: Some(5.0),
                rotation_deg: Some(10.0),
                ..Default::default()
            },
        ));
        let after = scene.get(2).unwrap().as_element().props().transform;
        assert!((after.translation.x - 5.0).abs() < 1e-4);
        assert!((after.rotation.to_degrees() - 10.0).abs() < 1e-3);
    }

    #[test]
    fn multi_select_mixed_and_batch_same_value() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(30.0, 40.0),
            Vec2::new(20.0, 10.0),
        ));
        let mixed = selection_transform(&scene, &[1, 2]);
        assert_eq!(mixed.x, StyleField::Mixed);
        assert_eq!(mixed.y, StyleField::Mixed);
        assert_eq!(mixed.width, StyleField::Mixed);
        assert_eq!(mixed.height, StyleField::Uniform(10.0));

        let mut history = CommandHistory::new();
        assert!(apply_selection_transform(
            &mut history,
            &mut scene,
            &[1, 2],
            TransformNumericPatch {
                x: Some(7.0),
                ..Default::default()
            },
        ));
        assert_eq!(
            selection_transform(&scene, &[1, 2]).x,
            StyleField::Uniform(7.0)
        );
        assert!(
            (scene
                .get(1)
                .unwrap()
                .as_element()
                .props()
                .transform
                .translation
                .y)
                .abs()
                < 1e-4
        );
        assert!(
            (scene
                .get(2)
                .unwrap()
                .as_element()
                .props()
                .transform
                .translation
                .y
                - 40.0)
                .abs()
                < 1e-4
        );
    }
}
