//! 图片裁剪。
//!
//! 裁剪是归一化窗口，盖在整张显示矩形上。拖动手柄或属性面板只改这个窗口：
//! 世界变换、显示尺寸和资源像素都留着，撤销能回到整张图。
//!
//! ```text
//! 选中图片 ──► CropPatch / 手柄
//!                 │
//!                 ▼
//!         SetImageCropCommand ──► Image::crop
//!                 │
//!                 └─ Scene::mark_dirty ──► 顶点 / 命中 / 导出
//! ```

use crate::element::{ElementId, Image};
use crate::scene::{AnyElement, Scene};
use crate::style::StyleField;
use crate::transform_handles::ResizeHandle;
use crate::{math::Rect, math::Vec2, Command};
use std::collections::HashSet;

/// 一次裁剪修改。`None` 表示这个分量不动。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CropPatch {
    /// 窗口左边缘，0–1。
    pub x: Option<f32>,
    /// 窗口上边缘，0–1。
    pub y: Option<f32>,
    /// 窗口宽度，0–1。
    pub width: Option<f32>,
    /// 窗口高度，0–1。
    pub height: Option<f32>,
}

impl CropPatch {
    /// 四个分量都没写。
    pub fn is_empty(self) -> bool {
        self.x.is_none() && self.y.is_none() && self.width.is_none() && self.height.is_none()
    }

    /// 把写了的分量盖到现有窗口上。非有限数字忽略。
    pub fn apply(self, crop: Rect) -> Rect {
        Rect::new(
            finite(self.x).unwrap_or(crop.x),
            finite(self.y).unwrap_or(crop.y),
            finite(self.width).unwrap_or(crop.width),
            finite(self.height).unwrap_or(crop.height),
        )
    }
}

fn finite(value: Option<f32>) -> Option<f32> {
    value.filter(|number| number.is_finite())
}

/// 当前选择里图片裁剪的公共值。
#[derive(Debug, Clone, PartialEq)]
pub struct ImageCropSelection {
    /// 选择里至少有一张未锁定的图片。
    pub supported: bool,
    /// 左边缘。
    pub x: StyleField<f32>,
    /// 上边缘。
    pub y: StyleField<f32>,
    /// 宽度。
    pub width: StyleField<f32>,
    /// 高度。
    pub height: StyleField<f32>,
}

impl ImageCropSelection {
    /// 没有可裁剪的图片。
    pub fn unsupported() -> Self {
        Self {
            supported: false,
            x: StyleField::Absent,
            y: StyleField::Absent,
            width: StyleField::Absent,
            height: StyleField::Absent,
        }
    }
}

/// 选择里的图片，组展开到子孙。锁定的（含被锁定的祖先）去掉。
///
/// 空选返回空。结果按首次出现的顺序，同一张图只出现一次。
pub fn crop_target_ids(scene: &Scene, ids: &[ElementId]) -> Vec<ElementId> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for id in ids {
        for node in scene.subtree_ids(*id) {
            if !seen.insert(node) || scene.is_effectively_locked(node) {
                continue;
            }
            if matches!(scene.get(node), Some(AnyElement::Image(_))) {
                out.push(node);
            }
        }
    }
    out
}

/// 读取选择上的裁剪。没有未锁定图片时 [`ImageCropSelection::supported`] 为 false。
pub fn image_crop_selection(scene: &Scene, ids: &[ElementId]) -> ImageCropSelection {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut widths = Vec::new();
    let mut heights = Vec::new();
    for id in crop_target_ids(scene, ids) {
        let Some(AnyElement::Image(image)) = scene.get(id) else {
            continue;
        };
        let crop = image.normalized_crop();
        xs.push(crop.x);
        ys.push(crop.y);
        widths.push(crop.width);
        heights.push(crop.height);
    }
    if xs.is_empty() {
        return ImageCropSelection::unsupported();
    }
    ImageCropSelection {
        supported: true,
        x: uniform_field(&xs),
        y: uniform_field(&ys),
        width: uniform_field(&widths),
        height: uniform_field(&heights),
    }
}

fn uniform_field(values: &[f32]) -> StyleField<f32> {
    let first = values[0];
    if values.iter().all(|value| (*value - first).abs() <= 1.0e-4) {
        StyleField::Uniform(first)
    } else {
        StyleField::Mixed
    }
}

/// 把裁剪写到图片上并 [`Scene::mark_dirty`]。不是图片时返回 false。
pub fn apply_image_crop(scene: &mut Scene, id: ElementId, crop: Rect) -> bool {
    {
        let Some(AnyElement::Image(image)) = scene.get_mut(id) else {
            return false;
        };
        let next = Image::clamp_crop(image.size, crop);
        if crops_equal(image.crop, next) {
            return false;
        }
        image.crop = next;
    }
    scene.mark_dirty(id);
    true
}

fn crops_equal(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() <= 1.0e-5
        && (a.y - b.y).abs() <= 1.0e-5
        && (a.width - b.width).abs() <= 1.0e-5
        && (a.height - b.height).abs() <= 1.0e-5
}

/// 拖动手柄时，固定对边，让裁剪边跟上指针。
///
/// 结果是归一化窗口。`start` 是按下时的窗口。世界变换和 `size` 只用来把指针
/// 变到局部坐标，函数本身不改它们。负缩放时手柄仍贴在视觉上的那一条边。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::crop::resize_image_crop;
/// use infinitecanvas_engine::transform_handles::ResizeHandle;
/// use infinitecanvas_engine::Image;
/// use infinitecanvas_math::{Rect, Vec2};
///
/// let image = Image::new(1, Vec2::ZERO, Vec2::new(100.0, 40.0), "photo".into());
/// let crop = resize_image_crop(
///     &image,
///     image.normalized_crop(),
///     ResizeHandle::West,
///     Vec2::new(25.0, 20.0),
/// );
/// assert_eq!(crop, Rect::new(0.25, 0.0, 0.75, 1.0));
/// ```
pub fn resize_image_crop(image: &Image, start: Rect, handle: ResizeHandle, world: Vec2) -> Rect {
    let transform = &image.props.transform;
    if !transform.is_invertible() {
        return Image::clamp_crop(image.size, start);
    }
    let local = transform.inverse_transform_point(world);
    if !local.x.is_finite() || !local.y.is_finite() {
        return Image::clamp_crop(image.size, start);
    }
    let span_x = image.size.x.abs();
    let span_y = image.size.y.abs();
    if span_x < 1.0e-4 || span_y < 1.0e-4 {
        return Rect::new(0.0, 0.0, 1.0, 1.0);
    }
    let start = Image::clamp_crop(image.size, start);
    let mut left = start.x * span_x;
    let mut top = start.y * span_y;
    let mut right = left + start.width * span_x;
    let mut bottom = top + start.height * span_y;
    let min_w = 1.0_f32.min(span_x);
    let min_h = 1.0_f32.min(span_y);
    let (hx, hy) = handle.axis_signs();
    let sx = hx * if transform.scale.x < 0.0 { -1 } else { 1 };
    let sy = hy * if transform.scale.y < 0.0 { -1 } else { 1 };
    let pointer_x = if image.size.x >= 0.0 {
        local.x
    } else {
        -local.x
    };
    let pointer_y = if image.size.y >= 0.0 {
        local.y
    } else {
        -local.y
    };
    if sx < 0 {
        left = pointer_x.clamp(0.0, (right - min_w).max(0.0));
    } else if sx > 0 {
        right = pointer_x.clamp((left + min_w).min(span_x), span_x);
    }
    if sy < 0 {
        top = pointer_y.clamp(0.0, (bottom - min_h).max(0.0));
    } else if sy > 0 {
        bottom = pointer_y.clamp((top + min_h).min(span_y), span_y);
    }
    Image::clamp_crop(
        image.size,
        Rect::new(
            left / span_x,
            top / span_y,
            (right - left) / span_x,
            (bottom - top) / span_y,
        ),
    )
}

#[derive(Debug, Clone, PartialEq)]
struct CropChange {
    id: ElementId,
    before: Rect,
    after: Rect,
}

/// 一张或多张图片的裁剪。空选、锁定、没有实际变化时不产生命令。
///
/// 执行和撤销都 [`Scene::mark_dirty`]。不改世界变换，也不动资源表。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::crop::{CropPatch, SetImageCropCommand};
/// use infinitecanvas_engine::{Command, CommandHistory, Image, Scene};
/// use infinitecanvas_math::Vec2;
///
/// let mut scene = Scene::new();
/// scene.add_image(Image::new(1, Vec2::ZERO, Vec2::new(40.0, 20.0), "photo".into()));
/// let patch = CropPatch {
///     x: Some(0.25),
///     width: Some(0.5),
///     ..CropPatch::default()
/// };
/// let mut history = CommandHistory::new();
/// let command = SetImageCropCommand::from_patch(&scene, &[1], &patch).unwrap();
/// history.execute(Box::new(command), &mut scene);
/// history.undo(&mut scene);
/// match scene.get(1).unwrap() {
///     infinitecanvas_engine::AnyElement::Image(image) => assert!(image.crop_is_full()),
///     _ => panic!("image"),
/// }
/// ```
#[derive(Debug)]
pub struct SetImageCropCommand {
    changes: Vec<CropChange>,
}

impl SetImageCropCommand {
    /// 按当前场景和补丁生成命令。没有实际变化时返回 `None`。
    ///
    /// 锁定的图片（含被锁定的祖先）会跳过。选中组时补丁作用在组里的图片上。
    pub fn from_patch(scene: &Scene, ids: &[ElementId], patch: &CropPatch) -> Option<Self> {
        if patch.is_empty() {
            return None;
        }
        let mut changes = Vec::new();
        for id in crop_target_ids(scene, ids) {
            let Some(AnyElement::Image(image)) = scene.get(id) else {
                continue;
            };
            let before = image.normalized_crop();
            let after = Image::clamp_crop(image.size, patch.apply(before));
            if !crops_equal(before, after) {
                changes.push(CropChange { id, before, after });
            }
        }
        if changes.is_empty() {
            None
        } else {
            Some(Self { changes })
        }
    }

    /// 用已经发生的前后窗口组一条命令。手柄松手时入栈，不会再次执行。
    pub fn from_states(before: Vec<(ElementId, Rect)>, after: Vec<(ElementId, Rect)>) -> Self {
        let changes = before
            .into_iter()
            .zip(after)
            .filter(|((_, previous), (_, next))| !crops_equal(*previous, *next))
            .map(|((id, before), (_id, after))| CropChange { id, before, after })
            .collect();
        Self { changes }
    }
}

impl Command for SetImageCropCommand {
    fn execute(&mut self, scene: &mut Scene) {
        for change in &self.changes {
            let _ = apply_image_crop(scene, change.id, change.after);
        }
    }

    fn undo(&mut self, scene: &mut Scene) {
        for change in &self.changes {
            let _ = apply_image_crop(scene, change.id, change.before);
        }
    }

    fn name(&self) -> &'static str {
        "SetImageCrop"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec2;
    use crate::vertex_cache::VertexCache;
    use crate::{CommandHistory, Element, Image};

    #[test]
    fn empty_and_locked_selection_do_not_crop() {
        let mut scene = Scene::new();
        scene.add_image(Image::new(1, Vec2::ZERO, Vec2::new(20.0, 10.0), "a".into()));
        scene.set_locked(1, true);
        let patch = CropPatch {
            width: Some(0.5),
            ..CropPatch::default()
        };
        assert!(SetImageCropCommand::from_patch(&scene, &[], &patch).is_none());
        assert!(SetImageCropCommand::from_patch(&scene, &[1], &patch).is_none());
        assert!(!image_crop_selection(&scene, &[1]).supported);
    }

    #[test]
    fn one_patch_updates_vertices_and_undo_restores_the_full_frame() {
        let mut scene = Scene::new();
        scene.add_image(Image::new(
            1,
            Vec2::new(4.0, 6.0),
            Vec2::new(80.0, 40.0),
            "photo".into(),
        ));
        let mut cache = VertexCache::new();
        let _ = cache.sync(&mut scene);
        let transform = match scene.get(1).unwrap() {
            AnyElement::Image(image) => image.props.transform,
            _ => panic!("image"),
        };
        let patch = CropPatch {
            x: Some(0.25),
            width: Some(0.5),
            ..CropPatch::default()
        };
        let epoch = scene.geometry_epoch();
        let mut history = CommandHistory::new();
        let command = SetImageCropCommand::from_patch(&scene, &[1], &patch).unwrap();
        history.execute(Box::new(command), &mut scene);
        assert!(scene.geometry_epoch() > epoch);
        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        match scene.get(1).unwrap() {
            AnyElement::Image(image) => {
                assert_eq!(image.props.transform, transform);
                assert_eq!(image.normalized_crop(), Rect::new(0.25, 0.0, 0.5, 1.0));
                assert!(!image.contains_point(Vec2::new(4.0, 6.0)));
                assert!(image.contains_point(Vec2::new(30.0, 10.0)));
                assert_eq!(cache.textured_slice(1).unwrap(), image.generate_vertices());
            }
            _ => panic!("image"),
        }
        assert!(history.undo(&mut scene));
        match scene.get(1).unwrap() {
            AnyElement::Image(image) => {
                assert!(image.crop_is_full());
                assert_eq!(image.props.transform, transform);
                assert!(image.contains_point(Vec2::new(4.0, 6.0)));
            }
            _ => panic!("image"),
        }
        assert!(history.redo(&mut scene));
        match scene.get(1).unwrap() {
            AnyElement::Image(image) => {
                assert_eq!(image.normalized_crop().x, 0.25);
            }
            _ => panic!("image"),
        }
    }

    #[test]
    fn a_group_crop_skips_locked_children_and_non_images() {
        let mut scene = Scene::new();
        scene.add_image(Image::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0), "a".into()));
        scene.add_image(Image::new(
            2,
            Vec2::new(20.0, 0.0),
            Vec2::new(10.0, 10.0),
            "b".into(),
        ));
        let group = scene.group(&[1, 2]).unwrap();
        scene.set_locked(2, true);
        let patch = CropPatch {
            y: Some(0.5),
            height: Some(0.5),
            ..CropPatch::default()
        };
        let mut command = SetImageCropCommand::from_patch(&scene, &[group], &patch).unwrap();
        command.execute(&mut scene);
        match scene.get(1).unwrap() {
            AnyElement::Image(image) => assert_eq!(image.normalized_crop().y, 0.5),
            _ => panic!("image"),
        }
        match scene.get(2).unwrap() {
            AnyElement::Image(image) => assert!(image.crop_is_full()),
            _ => panic!("image"),
        }
    }

    #[test]
    fn negative_scale_moves_the_visual_west_edge() {
        let mut image = Image::new(1, Vec2::ZERO, Vec2::new(100.0, 40.0), "photo".into());
        image.props.transform.scale = Vec2::new(-1.0, 1.0);
        let crop = resize_image_crop(
            &image,
            image.normalized_crop(),
            ResizeHandle::West,
            Vec2::new(-75.0, 20.0),
        );
        assert_eq!(crop, Rect::new(0.0, 0.0, 0.75, 1.0));
        assert_eq!(image.props.transform.scale, Vec2::new(-1.0, 1.0));
    }
}
