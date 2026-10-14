//! 属性面板的 WASM 接口。
//!
//! `get_selection_style` 汇总当前选择。多选字段不一致时对应的 `*Mixed` 为 true。
//! 滑块拖动调用 `preview_selection_style`，松手调用 `commit_style_preview`，
//! 这样连续拖动只进一条撤销记录。颜色选择器也可以走同一条预览路径。

use infinitecanvas_engine::{
    apply_style_snapshot, capture_style, math::Color, selection_style, style_locked,
    style_target_ids, ElementId, SetStyleCommand, StyleField, StylePatch, StyleSnapshot,
};
use js_sys::{Object, Reflect};
use wasm_bindgen::prelude::*;

use crate::InfiniteCanvas;

/// 滑块按下时的样式基线。预览每次都从这里重放，避免增量叠加。
pub(crate) struct StylePreview {
    before: Vec<(ElementId, StyleSnapshot)>,
}

#[wasm_bindgen]
impl InfiniteCanvas {
    /// 读取当前选择的公共样式。
    ///
    /// 没有选中元素时 `count` 为 0，`hasShape` / `hasText` / `hasImage` 为 false。
    /// 组会展开到子孙。图片只出现在 `opacity` 里。字段含义：
    ///
    /// - `fill` / `stroke` / `textColor`：`#rrggbbaa`，没有颜色时为 null
    /// - `fillMixed` 等：多选不一致
    /// - `fillNone`：大家都没有填充或描边
    /// - `fillSupported`：选择里有这个字段
    /// - `strokeWidth` / `opacity` / `fontSize`：统一数值，混合或不支持时为 null
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const style = canvas.get_selection_style();
    /// if (style.hasShape && !style.fillMixed) colorInput.value = style.fill.slice(0, 7);
    /// ```
    #[wasm_bindgen]
    pub fn get_selection_style(&self) -> JsValue {
        let ids = self.selection.selected_ids();
        let style = selection_style(&self.scene, &ids);
        let obj = Object::new();
        js_set(&obj, "count", JsValue::from(ids.len() as u32));
        js_set(&obj, "hasShape", JsValue::from(style.has_shape));
        js_set(&obj, "hasText", JsValue::from(style.has_text));
        js_set(&obj, "hasImage", JsValue::from(style.has_image));
        write_optional_color(&obj, "fill", &style.fill);
        write_optional_color(&obj, "stroke", &style.stroke);
        write_number(&obj, "strokeWidth", &style.stroke_width);
        write_number(&obj, "opacity", &style.opacity);
        write_number(&obj, "fontSize", &style.font_size);
        write_color(&obj, "textColor", &style.text_color);
        obj.into()
    }

    /// 把补丁写进当前选择并记一条撤销记录。
    ///
    /// 对象里出现的字段才会修改。`fill` / `stroke` 传 `null` 或 `""` 表示清掉。
    /// `strokeWidth`、`opacity`、`fontSize` 是数字。`textColor` 是十六进制颜色。
    /// 没有任何变化时返回 false，也不入栈。
    #[wasm_bindgen]
    pub fn set_selection_style(&mut self, patch: JsValue) -> bool {
        self.commit_style_preview();
        let Some(patch) = parse_style_patch(&patch) else {
            return false;
        };
        let ids = self.selection.selected_ids();
        let Some(command) = SetStyleCommand::from_patch(&self.scene, &ids, &patch) else {
            return false;
        };
        let textures = self.snapshot_text_textures();
        self.command_history
            .execute(Box::new(command), &mut self.scene);
        self.release_text_textures_except(textures);
        true
    }

    /// 预览样式。第一次调用记下基线，之后每次都从基线重放。
    ///
    /// 不进入撤销栈。松手后调用 [`Self::commit_style_preview`]。
    #[wasm_bindgen]
    pub fn preview_selection_style(&mut self, patch: JsValue) -> bool {
        let Some(patch) = parse_style_patch(&patch) else {
            return false;
        };
        if self.style_preview.is_none() {
            let before = self.editable_style_snapshots();
            if before.is_empty() {
                return false;
            }
            self.style_preview = Some(StylePreview { before });
        }
        let before = self
            .style_preview
            .as_ref()
            .map(|preview| preview.before.clone())
            .unwrap_or_default();
        let textures = self.snapshot_text_textures();
        let mut changed = false;
        for (id, snapshot) in &before {
            apply_style_snapshot(&mut self.scene, *id, snapshot);
            let mut next = snapshot.clone();
            if next.apply_patch(&patch) {
                apply_style_snapshot(&mut self.scene, *id, &next);
                changed = true;
            }
        }
        self.release_text_textures_except(textures);
        changed
    }

    /// 把进行中的样式预览收成一条撤销记录。
    ///
    /// 场景已经是预览后的值，所以只入栈、不再执行。没有变化时返回 false。
    #[wasm_bindgen]
    pub fn commit_style_preview(&mut self) -> bool {
        let Some(preview) = self.style_preview.take() else {
            return false;
        };
        let mut before = Vec::new();
        let mut after = Vec::new();
        for (id, snapshot) in preview.before {
            let Some(current) = capture_style(&self.scene, id) else {
                continue;
            };
            if current != snapshot {
                before.push((id, snapshot));
                after.push((id, current));
            }
        }
        if before.is_empty() {
            return false;
        }
        self.command_history
            .push_executed(Box::new(SetStyleCommand::from_states(before, after)));
        true
    }
}

impl InfiniteCanvas {
    /// 丢掉未提交的样式预览。导入和清空会整表替换场景，所以这里不把基线写回去。
    pub(crate) fn cancel_style_preview(&mut self) {
        self.style_preview = None;
    }

    fn editable_style_snapshots(&self) -> Vec<(ElementId, StyleSnapshot)> {
        style_target_ids(&self.scene, &self.selection.selected_ids())
            .into_iter()
            .filter(|id| !style_locked(&self.scene, *id))
            .filter_map(|id| capture_style(&self.scene, id).map(|snapshot| (id, snapshot)))
            .collect()
    }
}

fn js_set(obj: &Object, key: &str, value: JsValue) {
    let _ = Reflect::set(obj, &JsValue::from_str(key), &value);
}

fn write_optional_color(obj: &Object, name: &str, field: &StyleField<Option<Color>>) {
    match field {
        StyleField::Absent => {
            js_set(obj, name, JsValue::NULL);
            js_set(obj, &format!("{name}Mixed"), JsValue::from(false));
            js_set(obj, &format!("{name}None"), JsValue::from(false));
            js_set(obj, &format!("{name}Supported"), JsValue::from(false));
        }
        StyleField::Mixed => {
            js_set(obj, name, JsValue::NULL);
            js_set(obj, &format!("{name}Mixed"), JsValue::from(true));
            js_set(obj, &format!("{name}None"), JsValue::from(false));
            js_set(obj, &format!("{name}Supported"), JsValue::from(true));
        }
        StyleField::Uniform(None) => {
            js_set(obj, name, JsValue::NULL);
            js_set(obj, &format!("{name}Mixed"), JsValue::from(false));
            js_set(obj, &format!("{name}None"), JsValue::from(true));
            js_set(obj, &format!("{name}Supported"), JsValue::from(true));
        }
        StyleField::Uniform(Some(color)) => {
            js_set(obj, name, JsValue::from_str(&color.to_hex()));
            js_set(obj, &format!("{name}Mixed"), JsValue::from(false));
            js_set(obj, &format!("{name}None"), JsValue::from(false));
            js_set(obj, &format!("{name}Supported"), JsValue::from(true));
        }
    }
}

fn write_color(obj: &Object, name: &str, field: &StyleField<Color>) {
    match field {
        StyleField::Absent => {
            js_set(obj, name, JsValue::NULL);
            js_set(obj, &format!("{name}Mixed"), JsValue::from(false));
            js_set(obj, &format!("{name}Supported"), JsValue::from(false));
        }
        StyleField::Mixed => {
            js_set(obj, name, JsValue::NULL);
            js_set(obj, &format!("{name}Mixed"), JsValue::from(true));
            js_set(obj, &format!("{name}Supported"), JsValue::from(true));
        }
        StyleField::Uniform(color) => {
            js_set(obj, name, JsValue::from_str(&color.to_hex()));
            js_set(obj, &format!("{name}Mixed"), JsValue::from(false));
            js_set(obj, &format!("{name}Supported"), JsValue::from(true));
        }
    }
}

fn write_number(obj: &Object, name: &str, field: &StyleField<f32>) {
    match field {
        StyleField::Absent => {
            js_set(obj, name, JsValue::NULL);
            js_set(obj, &format!("{name}Mixed"), JsValue::from(false));
            js_set(obj, &format!("{name}Supported"), JsValue::from(false));
        }
        StyleField::Mixed => {
            js_set(obj, name, JsValue::NULL);
            js_set(obj, &format!("{name}Mixed"), JsValue::from(true));
            js_set(obj, &format!("{name}Supported"), JsValue::from(true));
        }
        StyleField::Uniform(value) => {
            js_set(obj, name, JsValue::from(*value));
            js_set(obj, &format!("{name}Mixed"), JsValue::from(false));
            js_set(obj, &format!("{name}Supported"), JsValue::from(true));
        }
    }
}

fn parse_style_patch(value: &JsValue) -> Option<StylePatch> {
    if !value.is_object() {
        return None;
    }
    Some(StylePatch {
        fill: read_color_update(value, "fill"),
        stroke: read_color_update(value, "stroke"),
        stroke_width: read_f32(value, "strokeWidth"),
        opacity: read_f32(value, "opacity"),
        font_size: read_f32(value, "fontSize"),
        text_color: read_color(value, "textColor"),
    })
}

fn read_color_update(value: &JsValue, key: &str) -> Option<Option<Color>> {
    if !has_key(value, key) {
        return None;
    }
    let current = Reflect::get(value, &JsValue::from_str(key)).ok()?;
    if current.is_null() || current.is_undefined() {
        return Some(None);
    }
    let text = current.as_string()?;
    if text.is_empty() || text == "none" {
        return Some(None);
    }
    Color::from_hex(&text).ok().map(Some)
}

fn read_color(value: &JsValue, key: &str) -> Option<Color> {
    if !has_key(value, key) {
        return None;
    }
    let current = Reflect::get(value, &JsValue::from_str(key)).ok()?;
    let text = current.as_string()?;
    Color::from_hex(&text).ok()
}

fn read_f32(value: &JsValue, key: &str) -> Option<f32> {
    if !has_key(value, key) {
        return None;
    }
    let current = Reflect::get(value, &JsValue::from_str(key)).ok()?;
    let number = current.as_f64()?;
    if number.is_finite() {
        Some(number as f32)
    } else {
        None
    }
}

fn has_key(value: &JsValue, key: &str) -> bool {
    Reflect::has(value, &JsValue::from_str(key)).unwrap_or(false)
}
