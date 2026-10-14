//! 图片裁剪的 WASM 接口。
//!
//! `get_image_crop` 汇总当前选择。面板拖动调用 `preview_image_crop`，
//! 松手调用 `commit_image_crop`，连续拖动只进一条 `SetImageCrop`。
//! 裁剪模式下手柄改的是同一窗口，不改世界变换。

use infinitecanvas_engine::{
    apply_image_crop, image_crop_selection,
    math::Rect,
    resize_image_crop,
    transform_handles::{ResizeHandle, SelectionFrame},
    AnyElement, CropPatch, Element, ElementId, SetImageCropCommand, StyleField,
};
use js_sys::{Object, Reflect};
use wasm_bindgen::prelude::*;

use crate::renderer::CropChrome;
use crate::InfiniteCanvas;

/// 滑块按下时的裁剪基线。预览每次都从这里重放。
pub(crate) struct CropPreview {
    before: Vec<(ElementId, Rect)>,
}

/// 一次裁剪手柄拖动。松手前场景里已经是新窗口。
pub(crate) struct CropDrag {
    pub pointer_id: i32,
    image_id: ElementId,
    pub handle: ResizeHandle,
    before: Rect,
    pub rotation: f32,
}

#[wasm_bindgen]
impl InfiniteCanvas {
    /// 读取当前选择里图片的裁剪窗口。
    ///
    /// 没有未锁定的图片时 `supported` 为 false。多选某一边不一致时对应的 `*Mixed` 为 true，
    /// 数值为 null。`x` / `y` / `width` / `height` 都是 0–1，整张图是 `0,0,1,1`。
    /// 组会展开到里面的图片。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const crop = canvas.get_image_crop();
    /// if (crop.supported && !crop.widthMixed) widthInput.value = crop.width;
    /// ```
    #[wasm_bindgen]
    pub fn get_image_crop(&self) -> JsValue {
        let selection = image_crop_selection(&self.scene, &self.selection.selected_ids());
        let obj = Object::new();
        js_set(&obj, "supported", JsValue::from(selection.supported));
        write_crop_number(&obj, "x", &selection.x);
        write_crop_number(&obj, "y", &selection.y);
        write_crop_number(&obj, "width", &selection.width);
        write_crop_number(&obj, "height", &selection.height);
        obj.into()
    }

    /// 把裁剪补丁写进当前选择并记一条撤销记录。
    ///
    /// 对象里出现的 `x` / `y` / `width` / `height` 才会修改。空选、锁定或没有变化时
    /// 返回 false，也不入栈。不改世界变换。
    #[wasm_bindgen]
    pub fn set_image_crop(&mut self, patch: JsValue) -> bool {
        self.commit_crop_preview();
        let Some(patch) = parse_crop_patch(&patch) else {
            return false;
        };
        let ids = self.selection.selected_ids();
        let Some(command) = SetImageCropCommand::from_patch(&self.scene, &ids, &patch) else {
            return false;
        };
        self.command_history
            .execute(Box::new(command), &mut self.scene);
        true
    }

    /// 预览裁剪。第一次调用记下基线，之后每次都从基线重放。
    ///
    /// 不进入撤销栈。松手后调用 [`Self::commit_image_crop`]。
    #[wasm_bindgen]
    pub fn preview_image_crop(&mut self, patch: JsValue) -> bool {
        let Some(patch) = parse_crop_patch(&patch) else {
            return false;
        };
        if self.crop_preview.is_none() {
            let before = self.crop_baselines();
            if before.is_empty() {
                return false;
            }
            self.crop_preview = Some(CropPreview { before });
        }
        let before = self
            .crop_preview
            .as_ref()
            .map(|preview| preview.before.clone())
            .unwrap_or_default();
        let mut changed = false;
        for (id, crop) in before {
            let _ = apply_image_crop(&mut self.scene, id, crop);
            let next = patch.apply(crop);
            if apply_image_crop(&mut self.scene, id, next) {
                changed = true;
            }
        }
        changed
    }

    /// 把进行中的裁剪预览收成一条撤销记录。
    ///
    /// 场景已经是预览后的值，所以只入栈、不再执行。没有变化时返回 false。
    #[wasm_bindgen]
    pub fn commit_image_crop(&mut self) -> bool {
        self.commit_crop_preview()
    }

    /// 打开或关闭裁剪手柄。
    ///
    /// 只在选中单张未锁定的可见图片时，缩放手柄改裁剪而不是世界变换。
    /// 没有新的快捷键。关闭时先提交未结束的手柄拖动。
    #[wasm_bindgen]
    pub fn set_crop_mode(&mut self, enabled: bool) {
        if !enabled {
            self.commit_crop_gesture();
        }
        self.crop_mode = enabled;
    }

    /// 裁剪手柄是否打开。
    #[wasm_bindgen]
    pub fn crop_mode(&self) -> bool {
        self.crop_mode
    }
}

impl InfiniteCanvas {
    pub(crate) fn commit_crop_preview(&mut self) -> bool {
        let Some(preview) = self.crop_preview.take() else {
            return false;
        };
        let mut before = Vec::new();
        let mut after = Vec::new();
        for (id, crop) in preview.before {
            let Some(current) = image_crop(&self.scene, id) else {
                continue;
            };
            if !crops_close(crop, current) {
                before.push((id, crop));
                after.push((id, current));
            }
        }
        if before.is_empty() {
            return false;
        }
        self.command_history
            .push_executed(Box::new(SetImageCropCommand::from_states(before, after)));
        true
    }

    /// 导入和清空会整表替换场景，所以这里不把基线写回去。
    pub(crate) fn cancel_crop_preview(&mut self) {
        self.crop_preview = None;
    }

    pub(crate) fn crop_handles_active(&self) -> bool {
        self.crop_mode && self.sole_crop_image().is_some()
    }

    /// 裁剪模式下的覆盖层：藏起旋转手柄，并在窗口小于整张图时画出原框。
    pub(crate) fn crop_overlay(&self) -> Option<CropChrome> {
        if !self.crop_handles_active() {
            return None;
        }
        let id = self.sole_crop_image()?;
        let AnyElement::Image(image) = self.scene.get(id)? else {
            return None;
        };
        let full_frame = if image.crop_is_full() {
            None
        } else {
            Some(SelectionFrame::from_local_rect(
                &image.props.transform,
                Rect::new(0.0, 0.0, image.size.x, image.size.y),
            ))
        };
        Some(CropChrome {
            full_frame,
            hide_rotate: true,
        })
    }

    pub(crate) fn begin_crop_drag(
        &mut self,
        event: &infinitecanvas_engine::PointerEvent,
        handle: ResizeHandle,
    ) -> bool {
        self.commit_crop_preview();
        let Some(id) = self.sole_crop_image() else {
            return false;
        };
        if self.scene.is_effectively_locked(id) {
            return false;
        }
        let Some(AnyElement::Image(image)) = self.scene.get(id) else {
            return false;
        };
        let before = image.normalized_crop();
        let rotation = image.rotation();
        self.selection_box.cancel();
        self.crop_drag = Some(CropDrag {
            pointer_id: event.pointer_id,
            image_id: id,
            handle,
            before,
            rotation,
        });
        true
    }

    pub(crate) fn update_crop_drag(&mut self, event: &infinitecanvas_engine::PointerEvent) -> bool {
        let Some(drag) = &self.crop_drag else {
            return false;
        };
        if drag.pointer_id != event.pointer_id {
            return false;
        }
        let image_id = drag.image_id;
        let handle = drag.handle;
        let before = drag.before;
        let Some(AnyElement::Image(image)) = self.scene.get(image_id) else {
            return true;
        };
        let next = resize_image_crop(image, before, handle, event.world_position);
        let _ = apply_image_crop(&mut self.scene, image_id, next);
        true
    }

    pub(crate) fn finish_crop_drag(&mut self, pointer_id: i32) -> bool {
        let matches = self
            .crop_drag
            .as_ref()
            .is_some_and(|drag| drag.pointer_id == pointer_id);
        if matches {
            self.commit_crop_gesture();
        }
        matches
    }

    pub(crate) fn commit_crop_gesture(&mut self) {
        let Some(drag) = self.crop_drag.take() else {
            return;
        };
        let Some(current) = image_crop(&self.scene, drag.image_id) else {
            return;
        };
        if crops_close(drag.before, current) {
            return;
        }
        self.command_history
            .push_executed(Box::new(SetImageCropCommand::from_states(
                vec![(drag.image_id, drag.before)],
                vec![(drag.image_id, current)],
            )));
    }

    pub(crate) fn cancel_crop_gesture(&mut self) {
        let Some(drag) = self.crop_drag.take() else {
            return;
        };
        let _ = apply_image_crop(&mut self.scene, drag.image_id, drag.before);
    }

    fn sole_crop_image(&self) -> Option<ElementId> {
        let elements = self.selected_element_refs();
        if elements.len() != 1 || elements[0].type_name() != "Image" {
            return None;
        }
        Some(elements[0].props().id)
    }

    fn crop_baselines(&self) -> Vec<(ElementId, Rect)> {
        infinitecanvas_engine::crop_target_ids(&self.scene, &self.selection.selected_ids())
            .into_iter()
            .filter_map(|id| image_crop(&self.scene, id).map(|crop| (id, crop)))
            .collect()
    }
}

fn image_crop(scene: &infinitecanvas_engine::Scene, id: ElementId) -> Option<Rect> {
    match scene.get(id)? {
        AnyElement::Image(image) => Some(image.normalized_crop()),
        _ => None,
    }
}

fn crops_close(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() <= 1.0e-5
        && (a.y - b.y).abs() <= 1.0e-5
        && (a.width - b.width).abs() <= 1.0e-5
        && (a.height - b.height).abs() <= 1.0e-5
}

fn parse_crop_patch(value: &JsValue) -> Option<CropPatch> {
    if !value.is_object() {
        return None;
    }
    let patch = CropPatch {
        x: read_f32(value, "x"),
        y: read_f32(value, "y"),
        width: read_f32(value, "width").or_else(|| read_f32(value, "w")),
        height: read_f32(value, "height").or_else(|| read_f32(value, "h")),
    };
    if patch.is_empty() {
        None
    } else {
        Some(patch)
    }
}

fn read_f32(value: &JsValue, key: &str) -> Option<f32> {
    let raw = Reflect::get(value, &JsValue::from_str(key)).ok()?;
    if raw.is_undefined() || raw.is_null() {
        return None;
    }
    raw.as_f64().map(|number| number as f32)
}

fn write_crop_number(obj: &Object, name: &str, field: &StyleField<f32>) {
    match field {
        StyleField::Uniform(value) => {
            js_set(obj, name, JsValue::from(*value));
            js_set(obj, &format!("{name}Mixed"), JsValue::from(false));
        }
        StyleField::Mixed => {
            js_set(obj, name, JsValue::NULL);
            js_set(obj, &format!("{name}Mixed"), JsValue::from(true));
        }
        StyleField::Absent => {
            js_set(obj, name, JsValue::NULL);
            js_set(obj, &format!("{name}Mixed"), JsValue::from(false));
        }
    }
}

fn js_set(obj: &Object, key: &str, value: JsValue) {
    let _ = Reflect::set(obj, &JsValue::from_str(key), &value);
}
