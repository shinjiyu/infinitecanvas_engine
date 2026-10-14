//! 选中框上的缩放 / 旋转手柄。
//!
//! 单选时选中框跟元素自身的旋转对齐；多选时用轴对齐包围盒，整组一起变。
//! 拖动过程中每次都从手势开始时的变换重算，避免累积误差。
//!
//! ```text
//!            [旋转]
//!              |
//!    nw --- n --- ne
//!    |             |
//!    w     +       e
//!    |             |
//!    sw --- s --- se
//! ```
//!
//! # 示例
//! ```
//! use infinitecanvas_engine::transform_handles::{HandleKind, ResizeHandle, SelectionFrame};
//! use infinitecanvas_engine::{Element, Rectangle};
//! use infinitecanvas_math::Vec2;
//!
//! let rect = Rectangle::new(1, Vec2::new(0.0, 0.0), Vec2::new(80.0, 40.0));
//! let frame = SelectionFrame::from_element(&rect);
//! let metrics_gap = 28.0;
//! let hit = frame.hit_test(Vec2::new(40.0, 0.0), 6.0, metrics_gap);
//! assert_eq!(hit, Some(HandleKind::Resize(ResizeHandle::East)));
//! ```

use crate::element::{Element, ElementId};
use infinitecanvas_math::{Transform, Vec2};

/// 旋转吸附的步进（15°）。
pub const ROTATE_SNAP_RADIANS: f32 = 15.0 * std::f32::consts::PI / 180.0;

/// 八个缩放手柄。坐标系 Y 向下，北是更小的 Y。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeHandle {
    /// 左上
    NorthWest,
    /// 上
    North,
    /// 右上
    NorthEast,
    /// 右
    East,
    /// 右下
    SouthEast,
    /// 下
    South,
    /// 左下
    SouthWest,
    /// 左
    West,
}

impl ResizeHandle {
    /// 手柄相对中心的方向：x/y 为 -1、0 或 1。
    fn axis_signs(self) -> (i8, i8) {
        match self {
            Self::NorthWest => (-1, -1),
            Self::North => (0, -1),
            Self::NorthEast => (1, -1),
            Self::East => (1, 0),
            Self::SouthEast => (1, 1),
            Self::South => (0, 1),
            Self::SouthWest => (-1, 1),
            Self::West => (-1, 0),
        }
    }

    /// 手柄所在边的朝向，用于选择光标。0 指向东，顺时针为正（Y 向下）。
    fn axis_angle(self) -> f32 {
        match self {
            Self::East => 0.0,
            Self::SouthEast => std::f32::consts::FRAC_PI_4,
            Self::South => std::f32::consts::FRAC_PI_2,
            Self::SouthWest => 3.0 * std::f32::consts::FRAC_PI_4,
            Self::West => std::f32::consts::PI,
            Self::NorthWest => 5.0 * std::f32::consts::FRAC_PI_4,
            Self::North => 3.0 * std::f32::consts::FRAC_PI_2,
            Self::NorthEast => 7.0 * std::f32::consts::FRAC_PI_4,
        }
    }
}

/// 手柄种类：缩放或旋转。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleKind {
    /// 拖动该手柄缩放
    Resize(ResizeHandle),
    /// 绕选中框中心旋转
    Rotate,
}

/// 一个手柄的世界坐标。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandlePoint {
    /// 手柄种类
    pub kind: HandleKind,
    /// 世界坐标
    pub position: Vec2,
}

/// 可旋转的选中框。`half_size` 在框自身的局部坐标里，不受旋转影响。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionFrame {
    /// 框中心（世界坐标）
    pub center: Vec2,
    /// 半宽、半高（局部坐标）
    pub half_size: Vec2,
    /// 框的旋转（弧度）
    pub rotation: f32,
}

impl SelectionFrame {
    /// 用单个元素的局部包围盒和变换建立选中框。
    ///
    /// 框的旋转等于元素的 [`Element::rotation`]，因此手柄贴在旋转后的边上，
    /// 而不是贴在轴对齐包围盒上。
    pub fn from_element(element: &dyn Element) -> Self {
        let local = element.local_bounds();
        let transform = element.props().transform;
        let center = transform.transform_point(local.center());
        let half = Vec2::new(
            (local.width.abs() * transform.scale.x.abs() * 0.5).max(0.5),
            (local.height.abs() * transform.scale.y.abs() * 0.5).max(0.5),
        );
        Self {
            center,
            half_size: half,
            rotation: transform.rotation,
        }
    }

    /// 从一个或多个元素建立选中框。
    ///
    /// 只有一个元素时框跟它一起转；多个元素时取世界包围盒的并集，旋转为 0。
    /// 没有元素时返回 `None`。
    pub fn from_elements(elements: &[&dyn Element]) -> Option<Self> {
        match elements.len() {
            0 => None,
            1 => Some(Self::from_element(elements[0])),
            _ => Some(Self::union_bounds(elements)),
        }
    }

    /// 四边都向外扩 `padding` 世界单位。点选手柄和绘制共用这一圈留白。
    pub fn inflated(self, padding: f32) -> Self {
        Self {
            half_size: self.half_size + Vec2::splat(padding.max(0.0)),
            ..self
        }
    }

    /// 框局部坐标转到世界坐标。
    pub fn to_world(&self, local: Vec2) -> Vec2 {
        self.center + rotate(local, self.rotation)
    }

    /// 世界坐标转到框局部坐标。
    pub fn to_local(&self, world: Vec2) -> Vec2 {
        rotate_inv(world - self.center, self.rotation)
    }

    /// 四个角，顺序为左上、右上、右下、左下（局部坐标，Y 向下）。
    pub fn corners(&self) -> [Vec2; 4] {
        let half = self.half_size;
        [
            self.to_world(Vec2::new(-half.x, -half.y)),
            self.to_world(Vec2::new(half.x, -half.y)),
            self.to_world(Vec2::new(half.x, half.y)),
            self.to_world(Vec2::new(-half.x, half.y)),
        ]
    }

    /// 8 个缩放手柄加 1 个旋转手柄。
    ///
    /// `rotate_gap` 是旋转手柄离开顶边中点的距离（世界单位）。
    pub fn handles(&self, rotate_gap: f32) -> [HandlePoint; 9] {
        let half = self.half_size;
        let locals = [
            (
                HandleKind::Resize(ResizeHandle::NorthWest),
                Vec2::new(-half.x, -half.y),
            ),
            (
                HandleKind::Resize(ResizeHandle::North),
                Vec2::new(0.0, -half.y),
            ),
            (
                HandleKind::Resize(ResizeHandle::NorthEast),
                Vec2::new(half.x, -half.y),
            ),
            (
                HandleKind::Resize(ResizeHandle::East),
                Vec2::new(half.x, 0.0),
            ),
            (
                HandleKind::Resize(ResizeHandle::SouthEast),
                Vec2::new(half.x, half.y),
            ),
            (
                HandleKind::Resize(ResizeHandle::South),
                Vec2::new(0.0, half.y),
            ),
            (
                HandleKind::Resize(ResizeHandle::SouthWest),
                Vec2::new(-half.x, half.y),
            ),
            (
                HandleKind::Resize(ResizeHandle::West),
                Vec2::new(-half.x, 0.0),
            ),
            (
                HandleKind::Rotate,
                Vec2::new(0.0, -half.y - rotate_gap.max(0.0)),
            ),
        ];
        locals.map(|(kind, local)| HandlePoint {
            kind,
            position: self.to_world(local),
        })
    }

    /// 点是否落在某个手柄上。旋转手柄优先于缩放手柄。
    ///
    /// # 参数
    /// - `world`: 世界坐标
    /// - `hit_radius`: 点选半径（世界单位）
    /// - `rotate_gap`: 与 [`Self::handles`] 相同的旋转手柄间距
    pub fn hit_test(&self, world: Vec2, hit_radius: f32, rotate_gap: f32) -> Option<HandleKind> {
        let handles = self.handles(rotate_gap);
        if let Some(rotate) = handles
            .iter()
            .find(|handle| matches!(handle.kind, HandleKind::Rotate))
        {
            if (rotate.position - world).length() <= hit_radius {
                return Some(HandleKind::Rotate);
            }
        }

        let mut best: Option<(HandleKind, f32)> = None;
        for handle in &handles {
            if matches!(handle.kind, HandleKind::Rotate) {
                continue;
            }
            let distance = (handle.position - world).length();
            if distance <= hit_radius && best.map(|(_, current)| distance < current).unwrap_or(true)
            {
                best = Some((handle.kind, distance));
            }
        }
        best.map(|(kind, _)| kind)
    }

    fn union_bounds(elements: &[&dyn Element]) -> Self {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for element in elements {
            let bounds = element.world_bounds();
            min_x = min_x.min(bounds.x);
            min_y = min_y.min(bounds.y);
            max_x = max_x.max(bounds.x + bounds.width);
            max_y = max_y.max(bounds.y + bounds.height);
        }
        let center = Vec2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
        let half = Vec2::new(
            ((max_x - min_x) * 0.5).max(0.5),
            ((max_y - min_y) * 0.5).max(0.5),
        );
        Self {
            center,
            half_size: half,
            rotation: 0.0,
        }
    }
}

/// 屏幕像素尺寸换算到当前缩放下的世界单位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandleMetrics {
    /// 手柄点选半径
    pub hit_radius: f32,
    /// 旋转手柄离开顶边的距离
    pub rotate_gap: f32,
    /// 选中框留白
    pub pad: f32,
    /// 缩放后半边的最小值，避免缩成 0 导致逆变换除零
    pub min_half: f32,
    /// 绘制手柄方块的半边
    pub draw_half: f32,
}

impl HandleMetrics {
    /// 按视口缩放把固定的屏幕像素换算成世界单位。
    ///
    /// 手柄在屏幕上大约 8px，放大后仍然点得中。
    pub fn from_zoom(zoom: f32) -> Self {
        let zoom = zoom.max(0.05);
        Self {
            hit_radius: 8.0 / zoom,
            rotate_gap: 28.0 / zoom,
            pad: 3.0 / zoom,
            min_half: 4.0 / zoom,
            draw_half: 4.0 / zoom,
        }
    }
}

/// 缩放或旋转光标。缩放光标会跟着选中框的旋转走。
pub fn cursor_for(kind: HandleKind, frame_rotation: f32) -> &'static str {
    match kind {
        HandleKind::Rotate => "grab",
        HandleKind::Resize(handle) => {
            let angle = (frame_rotation + handle.axis_angle()).rem_euclid(std::f32::consts::PI);
            let sector = (angle / std::f32::consts::FRAC_PI_4).round() as i32 % 4;
            match sector {
                0 => "ew-resize",
                1 => "nwse-resize",
                2 => "ns-resize",
                _ => "nesw-resize",
            }
        }
    }
}

/// 手势预览：新的选中框，以及每个元素应得的变换。
#[derive(Debug, Clone, PartialEq)]
pub struct GesturePreview {
    /// 预览用的选中框
    pub frame: SelectionFrame,
    /// 与手势开始时相同顺序的变换
    pub transforms: Vec<(ElementId, Transform)>,
}

/// 一次手柄拖动。预览始终从开始时的变换计算。
#[derive(Debug, Clone)]
pub struct HandleGesture {
    kind: HandleKind,
    origin: SelectionFrame,
    originals: Vec<(ElementId, Transform)>,
    start_pointer_angle: f32,
}

impl HandleGesture {
    /// 开始一次手柄拖动。
    ///
    /// # 参数
    /// - `frame`: 拖动开始时的选中框（含留白）
    /// - `kind`: 被抓住的手柄
    /// - `pointer`: 指针的世界坐标
    /// - `originals`: 参与变换的元素及其当前变换
    pub fn start(
        frame: SelectionFrame,
        kind: HandleKind,
        pointer: Vec2,
        originals: Vec<(ElementId, Transform)>,
    ) -> Self {
        let start_pointer_angle = pointer_angle(frame.center, pointer).unwrap_or(0.0);
        Self {
            kind,
            origin: frame,
            originals,
            start_pointer_angle,
        }
    }

    /// 被抓住的手柄。
    pub fn kind(&self) -> HandleKind {
        self.kind
    }

    /// 拖动开始时的变换，撤销命令用它作为 before。
    pub fn originals(&self) -> &[(ElementId, Transform)] {
        &self.originals
    }

    /// 根据当前指针计算预览。
    ///
    /// - 缩放：对边（或对边中点）钉住。`shift` 时两轴使用同一个比例。
    /// - 旋转：绕选中框中心。`shift` 时把框的绝对角度吸附到 15°。
    /// - `min_half`: 半边下限，防止元素缩没。
    pub fn preview(&self, pointer: Vec2, shift: bool, min_half: f32) -> GesturePreview {
        match self.kind {
            HandleKind::Rotate => self.preview_rotate(pointer, shift),
            HandleKind::Resize(handle) => self.preview_resize(handle, pointer, shift, min_half),
        }
    }

    fn preview_rotate(&self, pointer: Vec2, shift: bool) -> GesturePreview {
        let current =
            pointer_angle(self.origin.center, pointer).unwrap_or(self.start_pointer_angle);
        let mut delta = wrap_pi(current - self.start_pointer_angle);
        if shift {
            let target = snap_angle(self.origin.rotation + delta, ROTATE_SNAP_RADIANS);
            delta = target - self.origin.rotation;
        }
        let frame = SelectionFrame {
            center: self.origin.center,
            half_size: self.origin.half_size,
            rotation: self.origin.rotation + delta,
        };
        let transforms = self
            .originals
            .iter()
            .map(|(id, transform)| (*id, rotated_around(*transform, self.origin.center, delta)))
            .collect();
        GesturePreview { frame, transforms }
    }

    fn preview_resize(
        &self,
        handle: ResizeHandle,
        pointer: Vec2,
        shift: bool,
        min_half: f32,
    ) -> GesturePreview {
        let (sign_x, sign_y) = handle.axis_signs();
        let half = self.origin.half_size;
        let anchor_local = Vec2::new(
            if sign_x != 0 {
                -(sign_x as f32) * half.x
            } else {
                0.0
            },
            if sign_y != 0 {
                -(sign_y as f32) * half.y
            } else {
                0.0
            },
        );
        let pointer_local = self.origin.to_local(pointer);
        let mut scale_x = 1.0;
        let mut scale_y = 1.0;
        if sign_x != 0 {
            let moving = (sign_x as f32) * half.x;
            let anchor = -moving;
            let denom = moving - anchor;
            if denom.abs() > 1e-6 {
                scale_x = (pointer_local.x - anchor) / denom;
            }
        }
        if sign_y != 0 {
            let moving = (sign_y as f32) * half.y;
            let anchor = -moving;
            let denom = moving - anchor;
            if denom.abs() > 1e-6 {
                scale_y = (pointer_local.y - anchor) / denom;
            }
        }

        if shift {
            let uniform = if sign_x != 0 && sign_y != 0 {
                let handle_local = Vec2::new((sign_x as f32) * half.x, (sign_y as f32) * half.y);
                let origin_span = handle_local - anchor_local;
                let current_span = pointer_local - anchor_local;
                let denom = origin_span.length_squared();
                if denom > 1e-8 {
                    current_span.dot(origin_span) / denom
                } else {
                    1.0
                }
            } else if sign_x != 0 {
                scale_x
            } else {
                scale_y
            };
            let min_scale = min_axis_scale(half.x, min_half).max(min_axis_scale(half.y, min_half));
            let uniform = uniform.max(min_scale);
            scale_x = uniform;
            scale_y = uniform;
        } else {
            if sign_x != 0 {
                scale_x = scale_x.max(min_axis_scale(half.x, min_half));
            }
            if sign_y != 0 {
                scale_y = scale_y.max(min_axis_scale(half.y, min_half));
            }
        }

        let scale = Vec2::new(scale_x, scale_y);
        let anchor_world = self.origin.to_world(anchor_local);
        let center_from_anchor = -anchor_local;
        let new_center = anchor_world
            + rotate(
                Vec2::new(
                    center_from_anchor.x * scale_x,
                    center_from_anchor.y * scale_y,
                ),
                self.origin.rotation,
            );
        let frame = SelectionFrame {
            center: new_center,
            half_size: Vec2::new((half.x * scale_x).abs(), (half.y * scale_y).abs()),
            rotation: self.origin.rotation,
        };
        let transforms = self
            .originals
            .iter()
            .map(|(id, transform)| {
                (
                    *id,
                    resized_transform(*transform, anchor_world, self.origin.rotation, scale),
                )
            })
            .collect();
        GesturePreview { frame, transforms }
    }
}

/// 两次变换是否有可见差别。低于阈值的抖动不写入撤销栈。
pub fn transforms_differ(
    before: &[(ElementId, Transform)],
    after: &[(ElementId, Transform)],
) -> bool {
    const POSITION_EPS: f32 = 0.05;
    const ROTATION_EPS: f32 = 0.001;
    const SCALE_EPS: f32 = 0.001;
    before
        .iter()
        .zip(after.iter())
        .any(|((_, left), (_, right))| {
            (left.translation - right.translation).length() > POSITION_EPS
                || (left.rotation - right.rotation).abs() > ROTATION_EPS
                || (left.scale - right.scale).length() > SCALE_EPS
        })
}

fn min_axis_scale(half: f32, min_half: f32) -> f32 {
    if half > 1e-4 {
        (min_half / half).clamp(0.01, 1.0)
    } else {
        1.0
    }
}

fn pointer_angle(center: Vec2, pointer: Vec2) -> Option<f32> {
    let delta = pointer - center;
    if delta.length_squared() < 1e-8 {
        None
    } else {
        Some(delta.y.atan2(delta.x))
    }
}

fn wrap_pi(angle: f32) -> f32 {
    let full = std::f32::consts::TAU;
    let mut wrapped = angle % full;
    if wrapped > std::f32::consts::PI {
        wrapped -= full;
    }
    if wrapped < -std::f32::consts::PI {
        wrapped += full;
    }
    wrapped
}

fn snap_angle(angle: f32, step: f32) -> f32 {
    (angle / step).round() * step
}

fn rotate(vector: Vec2, rotation: f32) -> Vec2 {
    let (sin, cos) = rotation.sin_cos();
    Vec2::new(
        cos * vector.x - sin * vector.y,
        sin * vector.x + cos * vector.y,
    )
}

fn rotate_inv(vector: Vec2, rotation: f32) -> Vec2 {
    let (sin, cos) = rotation.sin_cos();
    Vec2::new(
        cos * vector.x + sin * vector.y,
        -sin * vector.x + cos * vector.y,
    )
}

fn basis(transform: Transform) -> (Vec2, Vec2) {
    let (sin, cos) = transform.rotation.sin_cos();
    (
        Vec2::new(cos * transform.scale.x, sin * transform.scale.x),
        Vec2::new(-sin * transform.scale.y, cos * transform.scale.y),
    )
}

fn map_point(anchor: Vec2, rotation: f32, scale: Vec2, point: Vec2) -> Vec2 {
    let local = rotate_inv(point - anchor, rotation);
    anchor + rotate(Vec2::new(local.x * scale.x, local.y * scale.y), rotation)
}

fn map_vector(rotation: f32, scale: Vec2, vector: Vec2) -> Vec2 {
    let local = rotate_inv(vector, rotation);
    rotate(Vec2::new(local.x * scale.x, local.y * scale.y), rotation)
}

fn decompose(column_x: Vec2, column_y: Vec2) -> (f32, Vec2) {
    let scale_x = column_x.length();
    if scale_x < 1e-6 {
        return (0.0, Vec2::new(1e-3, column_y.length().max(1e-3)));
    }
    let rotation = column_x.y.atan2(column_x.x);
    let scale_y_len = column_y.length().max(1e-4);
    let (sin, cos) = rotation.sin_cos();
    let expected_y = Vec2::new(-sin, cos) * scale_y_len;
    let scale_y = if column_y.dot(expected_y) < 0.0 {
        -scale_y_len
    } else {
        scale_y_len
    };
    (rotation, Vec2::new(scale_x.max(1e-4), scale_y))
}

fn resized_transform(
    original: Transform,
    anchor: Vec2,
    frame_rotation: f32,
    scale: Vec2,
) -> Transform {
    let (column_x, column_y) = basis(original);
    let (rotation, new_scale) = decompose(
        map_vector(frame_rotation, scale, column_x),
        map_vector(frame_rotation, scale, column_y),
    );
    Transform {
        translation: map_point(anchor, frame_rotation, scale, original.translation),
        rotation,
        scale: new_scale,
    }
}

fn rotated_around(original: Transform, pivot: Vec2, delta: f32) -> Transform {
    Transform {
        translation: pivot + rotate(original.translation - pivot, delta),
        rotation: original.rotation + delta,
        scale: original.scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandHistory, TransformElementsCommand};
    use crate::element::Rectangle;
    use crate::Scene;
    use infinitecanvas_math::Vec2;

    fn rect(id: ElementId, center: Vec2, size: Vec2) -> Rectangle {
        Rectangle::new(id, center, size)
    }

    fn approx(actual: Vec2, expected: Vec2) {
        assert!(
            (actual - expected).length() < 1e-3,
            "actual {actual:?} expected {expected:?}"
        );
    }

    #[test]
    fn single_rect_frame_uses_center_and_half_size() {
        let element = rect(1, Vec2::new(100.0, 80.0), Vec2::new(80.0, 40.0));
        let frame = SelectionFrame::from_element(&element);
        approx(frame.center, Vec2::new(100.0, 80.0));
        approx(frame.half_size, Vec2::new(40.0, 20.0));
        assert!(frame.rotation.abs() < 1e-6);
    }

    #[test]
    fn rotated_rect_frame_keeps_local_extents() {
        let mut element = rect(1, Vec2::ZERO, Vec2::new(100.0, 40.0));
        element
            .props_mut()
            .set_rotation(std::f32::consts::FRAC_PI_2);
        let frame = SelectionFrame::from_element(&element);
        approx(frame.half_size, Vec2::new(50.0, 20.0));
        assert!((frame.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        let east = frame
            .handles(28.0)
            .into_iter()
            .find(|handle| handle.kind == HandleKind::Resize(ResizeHandle::East))
            .unwrap();
        approx(east.position, Vec2::new(0.0, 50.0));
    }

    #[test]
    fn handles_are_eight_resize_plus_rotate() {
        let element = rect(1, Vec2::ZERO, Vec2::new(20.0, 10.0));
        let frame = SelectionFrame::from_element(&element);
        let handles = frame.handles(12.0);
        assert_eq!(handles.len(), 9);
        assert_eq!(handles[8].kind, HandleKind::Rotate);
        approx(handles[8].position, Vec2::new(0.0, -5.0 - 12.0));
        assert!(frame.hit_test(handles[8].position, 4.0, 12.0).unwrap() == HandleKind::Rotate);
        assert!(frame.hit_test(Vec2::ZERO, 4.0, 12.0).is_none());
    }

    #[test]
    fn resize_east_keeps_west_edge() {
        let element = rect(1, Vec2::new(100.0, 100.0), Vec2::new(80.0, 40.0));
        let frame = SelectionFrame::from_element(&element);
        let originals = vec![(1, element.props().transform)];
        let gesture = HandleGesture::start(
            frame,
            HandleKind::Resize(ResizeHandle::East),
            Vec2::new(140.0, 100.0),
            originals,
        );
        let preview = gesture.preview(Vec2::new(180.0, 100.0), false, 1.0);
        let transform = preview.transforms[0].1;
        approx(transform.translation, Vec2::new(120.0, 100.0));
        approx(transform.scale, Vec2::new(1.5, 1.0));
        assert!(transform.rotation.abs() < 1e-4);
        approx(preview.frame.center, Vec2::new(120.0, 100.0));
        approx(preview.frame.half_size, Vec2::new(60.0, 20.0));
    }

    #[test]
    fn resize_rotated_rect_keeps_anchor_and_rotation() {
        let mut element = rect(1, Vec2::ZERO, Vec2::new(100.0, 40.0));
        element
            .props_mut()
            .set_rotation(std::f32::consts::FRAC_PI_2);
        let frame = SelectionFrame::from_element(&element);
        let gesture = HandleGesture::start(
            frame,
            HandleKind::Resize(ResizeHandle::East),
            Vec2::new(0.0, 50.0),
            vec![(1, element.props().transform)],
        );
        let preview = gesture.preview(Vec2::new(0.0, 100.0), false, 1.0);
        let transform = preview.transforms[0].1;
        approx(transform.translation, Vec2::new(0.0, 25.0));
        approx(transform.scale, Vec2::new(1.5, 1.0));
        assert!((transform.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    }

    #[test]
    fn shift_resize_corner_keeps_aspect() {
        let element = rect(1, Vec2::ZERO, Vec2::new(80.0, 40.0));
        let frame = SelectionFrame::from_element(&element);
        let gesture = HandleGesture::start(
            frame,
            HandleKind::Resize(ResizeHandle::NorthEast),
            Vec2::new(40.0, -20.0),
            vec![(1, element.props().transform)],
        );
        // 东北角相对锚点的向量是 (80, -40)，拉到 2 倍。
        let preview = gesture.preview(Vec2::new(120.0, -60.0), true, 1.0);
        let transform = preview.transforms[0].1;
        approx(transform.scale, Vec2::new(2.0, 2.0));
        assert!((transform.scale.x - transform.scale.y).abs() < 1e-4);
    }

    #[test]
    fn resize_does_not_flip_through_the_anchor() {
        let element = rect(1, Vec2::ZERO, Vec2::new(80.0, 40.0));
        let frame = SelectionFrame::from_element(&element);
        let gesture = HandleGesture::start(
            frame,
            HandleKind::Resize(ResizeHandle::East),
            Vec2::new(40.0, 0.0),
            vec![(1, element.props().transform)],
        );
        let preview = gesture.preview(Vec2::new(-200.0, 0.0), false, 4.0);
        assert!(preview.transforms[0].1.scale.x > 0.0);
        assert!(preview.frame.half_size.x + 1e-3 >= 4.0);
    }

    #[test]
    fn multi_resize_scales_positions_from_the_anchor() {
        let left = rect(1, Vec2::ZERO, Vec2::new(20.0, 20.0));
        let right = rect(2, Vec2::new(40.0, 0.0), Vec2::new(20.0, 20.0));
        let elements: [&dyn Element; 2] = [&left, &right];
        let frame = SelectionFrame::from_elements(&elements).unwrap();
        approx(frame.center, Vec2::new(20.0, 0.0));
        approx(frame.half_size, Vec2::new(30.0, 10.0));
        let originals = vec![(1, left.props().transform), (2, right.props().transform)];
        let east = frame
            .handles(10.0)
            .into_iter()
            .find(|handle| handle.kind == HandleKind::Resize(ResizeHandle::East))
            .unwrap();
        let gesture = HandleGesture::start(
            frame,
            HandleKind::Resize(ResizeHandle::East),
            east.position,
            originals,
        );
        // 向东再移一个整宽，比例变成 2。
        let preview = gesture.preview(east.position + Vec2::new(60.0, 0.0), false, 1.0);
        approx(preview.transforms[0].1.translation, Vec2::new(10.0, 0.0));
        approx(preview.transforms[1].1.translation, Vec2::new(90.0, 0.0));
        approx(preview.transforms[0].1.scale, Vec2::new(2.0, 1.0));
        approx(preview.transforms[1].1.scale, Vec2::new(2.0, 1.0));
    }

    #[test]
    fn rotate_group_around_selection_center() {
        let left = rect(1, Vec2::ZERO, Vec2::new(10.0, 10.0));
        let right = rect(2, Vec2::new(100.0, 0.0), Vec2::new(10.0, 10.0));
        let elements: [&dyn Element; 2] = [&left, &right];
        let frame = SelectionFrame::from_elements(&elements).unwrap();
        let gesture = HandleGesture::start(
            frame,
            HandleKind::Rotate,
            frame.center + Vec2::new(100.0, 0.0),
            vec![(1, left.props().transform), (2, right.props().transform)],
        );
        let preview = gesture.preview(frame.center + Vec2::new(0.0, 100.0), false, 1.0);
        approx(preview.transforms[0].1.translation, Vec2::new(50.0, -50.0));
        approx(preview.transforms[1].1.translation, Vec2::new(50.0, 50.0));
        assert!((preview.transforms[0].1.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        assert!((preview.transforms[1].1.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        assert!((preview.frame.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    }

    #[test]
    fn shift_rotate_snaps_to_fifteen_degrees() {
        let element = rect(1, Vec2::ZERO, Vec2::new(30.0, 20.0));
        let frame = SelectionFrame::from_element(&element);
        let gesture = HandleGesture::start(
            frame,
            HandleKind::Rotate,
            frame.center + Vec2::new(100.0, 0.0),
            vec![(1, element.props().transform)],
        );
        let radians = 20.0 * std::f32::consts::PI / 180.0;
        let pointer = frame.center + Vec2::new(100.0 * radians.cos(), 100.0 * radians.sin());
        let preview = gesture.preview(pointer, true, 1.0);
        let expected = 15.0 * std::f32::consts::PI / 180.0;
        assert!((preview.transforms[0].1.rotation - expected).abs() < 1e-4);
        assert!((preview.frame.rotation - expected).abs() < 1e-4);
    }

    #[test]
    fn transform_command_undo_restores_resize() {
        let mut scene = Scene::new();
        let element = rect(1, Vec2::new(100.0, 100.0), Vec2::new(80.0, 40.0));
        scene.add_rect(element);
        let frame = SelectionFrame::from_element(scene.get(1).unwrap().as_element());
        let before = vec![(1, scene.get(1).unwrap().as_element().props().transform)];
        let gesture = HandleGesture::start(
            frame,
            HandleKind::Resize(ResizeHandle::East),
            Vec2::new(140.0, 100.0),
            before.clone(),
        );
        let preview = gesture.preview(Vec2::new(180.0, 100.0), false, 1.0);
        scene
            .get_mut(1)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .transform = preview.transforms[0].1;

        let mut history = CommandHistory::new();
        history.push_executed(Box::new(TransformElementsCommand::new(
            before,
            preview.transforms.clone(),
        )));
        assert!((scene.get(1).unwrap().as_element().rotation()).abs() < 1e-4);
        assert!((scene.get(1).unwrap().as_element().props().transform.scale.x - 1.5).abs() < 1e-3);

        assert!(history.undo(&mut scene));
        let restored = scene.get(1).unwrap().as_element().props().transform;
        approx(restored.translation, Vec2::new(100.0, 100.0));
        approx(restored.scale, Vec2::ONE);
        assert!(history.redo(&mut scene));
        assert!((scene.get(1).unwrap().as_element().props().transform.scale.x - 1.5).abs() < 1e-3);
    }

    #[test]
    fn cursor_follows_frame_rotation() {
        assert_eq!(
            cursor_for(HandleKind::Resize(ResizeHandle::East), 0.0),
            "ew-resize"
        );
        assert_eq!(
            cursor_for(
                HandleKind::Resize(ResizeHandle::East),
                std::f32::consts::FRAC_PI_2
            ),
            "ns-resize"
        );
        assert_eq!(cursor_for(HandleKind::Rotate, 0.0), "grab");
    }
}
