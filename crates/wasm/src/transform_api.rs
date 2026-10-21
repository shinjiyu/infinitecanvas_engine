//! 属性面板数值变换的 WASM 接口。
//!
//! `get_selection_transform` 汇总当前选择的 X/Y/W/H/旋转。
//! `set_selection_transform` 一次写入并记一条 [`TransformElementsCommand`]。
//! Enter / blur 提交；非法输入返回 false，不入栈。

use infinitecanvas_engine::{
    apply_selection_transform, selection_transform, SelectionTransform, StyleField,
    TransformNumericPatch,
};
use js_sys::{Object, Reflect};
use wasm_bindgen::prelude::*;

use crate::InfiniteCanvas;

#[wasm_bindgen]
impl InfiniteCanvas {
    /// 读取当前选择的变换数值。
    ///
    /// - `supported`：有未锁定的选中根
    /// - `sizeSupported`：选择里有非直线目标，可改 W/H
    /// - `x` / `y`：本地原点世界坐标（`transform.translation`）
    /// - `width` / `height`：世界尺寸 `|local| * |scale|`；直线不参与
    /// - `rotation`：旋转角度（度），与旋转手柄同一字段
    /// - 多选不一致时对应 `*Mixed` 为 true，数值为 null
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const t = canvas.get_selection_transform();
    /// if (t.supported && !t.xMixed) xInput.value = t.x;
    /// ```
    #[wasm_bindgen]
    pub fn get_selection_transform(&self) -> JsValue {
        let summary = selection_transform(&self.scene, &self.selection.selected_ids());
        write_selection_transform(&summary)
    }

    /// 把变换补丁写进当前选择并记一条撤销记录。
    ///
    /// 对象里出现的 `x` / `y` / `width`（或 `w`）/ `height`（或 `h`）/ `rotation`
    /// （或 `rotationDeg` / `rotation_deg`）才会修改。宽高必须为正有限数。
    /// 直线忽略宽高。多选时每个根写入同一绝对值。
    /// 空选、锁定、非法输入或没有变化时返回 false，不入栈。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.set_selection_transform({ x: 100, rotation: 15 });
    /// ```
    #[wasm_bindgen]
    pub fn set_selection_transform(&mut self, patch: JsValue) -> bool {
        self.commit_style_preview();
        let Some(patch) = parse_transform_patch(&patch) else {
            return false;
        };
        apply_selection_transform(
            &mut self.command_history,
            &mut self.scene,
            &self.selection.selected_ids(),
            patch,
        )
    }
}

fn write_selection_transform(summary: &SelectionTransform) -> JsValue {
    let obj = Object::new();
    js_set(&obj, "supported", JsValue::from(summary.supported));
    js_set(&obj, "sizeSupported", JsValue::from(summary.size_supported));
    write_number(&obj, "x", &summary.x);
    write_number(&obj, "y", &summary.y);
    write_number(&obj, "width", &summary.width);
    write_number(&obj, "height", &summary.height);
    write_number(&obj, "rotation", &summary.rotation_deg);
    obj.into()
}

fn write_number(obj: &Object, name: &str, field: &StyleField<f32>) {
    match field {
        StyleField::Uniform(value) => {
            js_set(obj, name, JsValue::from(*value));
            js_set(obj, &format!("{name}Mixed"), JsValue::from(false));
            js_set(obj, &format!("{name}Supported"), JsValue::from(true));
        }
        StyleField::Mixed => {
            js_set(obj, name, JsValue::NULL);
            js_set(obj, &format!("{name}Mixed"), JsValue::from(true));
            js_set(obj, &format!("{name}Supported"), JsValue::from(true));
        }
        StyleField::Absent => {
            js_set(obj, name, JsValue::NULL);
            js_set(obj, &format!("{name}Mixed"), JsValue::from(false));
            js_set(obj, &format!("{name}Supported"), JsValue::from(false));
        }
    }
}

fn parse_transform_patch(value: &JsValue) -> Option<TransformNumericPatch> {
    if !value.is_object() {
        return None;
    }
    let x = read_optional_f32(value, "x").ok()?;
    let y = read_optional_f32(value, "y").ok()?;
    let width = read_first_f32(value, &["width", "w"]).ok()?;
    let height = read_first_f32(value, &["height", "h"]).ok()?;
    let rotation_deg = read_first_f32(value, &["rotation", "rotationDeg", "rotation_deg"]).ok()?;

    let patch = TransformNumericPatch {
        x,
        y,
        width,
        height,
        rotation_deg,
    };
    if patch.is_empty() || !patch.is_valid() {
        None
    } else {
        Some(patch)
    }
}

fn read_first_f32(value: &JsValue, keys: &[&str]) -> Result<Option<f32>, ()> {
    for key in keys {
        if Reflect::has(value, &JsValue::from_str(key)).unwrap_or(false) {
            return read_optional_f32(value, key);
        }
    }
    Ok(None)
}

/// `Ok(None)`：键不存在或 null。`Err(())`：键在但非法（拒绝整次提交）。
fn read_optional_f32(value: &JsValue, key: &str) -> Result<Option<f32>, ()> {
    if !Reflect::has(value, &JsValue::from_str(key)).unwrap_or(false) {
        return Ok(None);
    }
    let raw = Reflect::get(value, &JsValue::from_str(key)).map_err(|_| ())?;
    if raw.is_undefined() || raw.is_null() {
        return Ok(None);
    }
    let number = raw.as_f64().ok_or(())?;
    if !number.is_finite() {
        return Err(());
    }
    Ok(Some(number as f32))
}

fn js_set(obj: &Object, key: &str, value: JsValue) {
    let _ = Reflect::set(obj, &JsValue::from_str(key), &value);
}
