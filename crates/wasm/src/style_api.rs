//! 属性面板的 WASM 接口。
//!
//! `get_selection_style` 汇总当前选择。多选字段不一致时对应的 `*Mixed` 为 true。
//! 滑块拖动调用 `preview_selection_style`，松手调用 `commit_style_preview`，
//! 这样连续拖动只进一条撤销记录。颜色选择器也可以走同一条预览路径。

use infinitecanvas_engine::{
    apply_sampled_paint, apply_style_snapshot, capture_style, math::Color, math::Vec2,
    sample_paint_at, selection_style, style_locked, style_target_ids, ElementId, SampledPaint,
    SetStyleCommand, StrokeCap, StrokeJoin, StyleField, StylePatch, StyleSnapshot,
};
use js_sys::{Array, Object, Reflect};
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
    /// - `strokeWidth` / `opacity` / `fontSize` / `cornerRadius`：统一数值，混合或不支持时为 null
    /// - `strokeDash`：数字数组，空数组是实线。`strokeCap` 为 `butt` / `round` / `square`，
    ///   `strokeJoin` 为 `miter` / `round` / `bevel`。这三项只统计形状，文本描边不参与
    /// - `cornerRadius` 只在选择里有矩形时出现（`hasRect` / `cornerRadiusSupported`）。多选半径不一致时 `cornerRadiusMixed` 为 true
    /// - `sides` 是多边形边数或星形角数（`hasPolygon` / `hasStar` / `sidesSupported`）。`innerRadius` 只统计星形
    /// - `shadowOffsetX` / `shadowOffsetY` / `shadowBlur` / `shadowOpacity` 是数字。`shadowColor` 是 `#rrggbbaa`（面板只用 RGB，浓度在 `shadowOpacity`）。没有形状、文本或图片时 `shadowSupported` 为 false
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
        js_set(&obj, "hasRect", JsValue::from(style.has_rect));
        js_set(&obj, "hasText", JsValue::from(style.has_text));
        js_set(&obj, "hasImage", JsValue::from(style.has_image));
        write_optional_color(&obj, "fill", &style.fill);
        write_optional_color(&obj, "stroke", &style.stroke);
        write_number(&obj, "strokeWidth", &style.stroke_width);
        write_number(&obj, "opacity", &style.opacity);
        write_number(&obj, "fontSize", &style.font_size);
        write_number(&obj, "cornerRadius", &style.corner_radius);
        write_number(&obj, "sides", &style.sides);
        write_number(&obj, "innerRadius", &style.inner_radius);
        js_set(&obj, "hasPolygon", JsValue::from(style.has_polygon));
        js_set(&obj, "hasStar", JsValue::from(style.has_star));
        write_number(&obj, "shadowOffsetX", &style.shadow_offset_x);
        write_number(&obj, "shadowOffsetY", &style.shadow_offset_y);
        write_number(&obj, "shadowBlur", &style.shadow_blur);
        write_number(&obj, "shadowOpacity", &style.shadow_opacity);
        write_color(&obj, "shadowColor", &style.shadow_color);
        js_set(
            &obj,
            "shadowSupported",
            JsValue::from(style.shadow_supported),
        );
        write_color(&obj, "textColor", &style.text_color);
        write_dash(&obj, &style.stroke_dash);
        write_label(&obj, "strokeCap", &style.stroke_cap, StrokeCap::as_str);
        write_label(&obj, "strokeJoin", &style.stroke_join, StrokeJoin::as_str);
        obj.into()
    }

    /// 把补丁写进当前选择并记一条撤销记录。
    ///
    /// 对象里出现的字段才会修改。`fill` / `stroke` 传 `null` 或 `""` 表示清掉。
    /// `strokeWidth`、`opacity`、`fontSize`、`cornerRadius`、`sides`、`innerRadius` 是数字。
    /// `cornerRadius` 也可以写成 `corner_radius`。`sides` 也可以写成 `points`，并钳到 3–64。
    /// `innerRadius` 也可以写成 `inner_radius`，只作用于星形，并钳到 0.05–0.95。
    /// `textColor` 是十六进制颜色。圆角只写到矩形上。
    /// `shadowOffsetX` / `shadowOffsetY` / `shadowBlur` / `shadowOpacity` 是数字，也可以写成 `shadow_offset_x` 等蛇形名字。
    /// `shadowColor` 只改 RGB，浓度用 `shadowOpacity`。形状、文本和图片都接受投影。空选择或没有变化时返回 false。
    /// `strokeDash` 是数字数组（空数组改回实线）。`strokeCap` / `strokeJoin` 是小写名字。
    /// 线型和线帽线结只改形状，文本描边保持原来的光栅描边。
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

    /// 采样世界坐标上最上层可见叶子的填充和描边。
    ///
    /// 返回对象：
    /// - `id`：命中的元素。空白处为 null
    /// - `fill` / `stroke`：`#rrggbbaa`。元素没有该通道，或没有命中时为 null
    ///
    /// 文本的 `fill` 是文字颜色。图片命中时 `id` 有值，两个颜色都是 null。
    /// 锁定元素仍可采样。隐藏元素（含被隐藏的祖先）会跳过，露出下面的叶子。
    /// 不读像素。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const sample = canvas.sample_style_at(worldX, worldY);
    /// if (sample.fill) colorInput.value = sample.fill.slice(0, 7);
    /// ```
    #[wasm_bindgen]
    pub fn sample_style_at(&self, x: f32, y: f32) -> JsValue {
        let obj = Object::new();
        let sample = if x.is_finite() && y.is_finite() {
            sample_paint_at(&self.scene, Vec2::new(x, y))
        } else {
            None
        };
        match sample {
            Some(sample) => {
                js_set(&obj, "id", JsValue::from(sample.id));
                write_hex(&obj, "fill", sample.fill);
                write_hex(&obj, "stroke", sample.stroke);
            }
            None => {
                js_set(&obj, "id", JsValue::NULL);
                js_set(&obj, "fill", JsValue::NULL);
                js_set(&obj, "stroke", JsValue::NULL);
            }
        }
        obj.into()
    }

    /// 采样该世界点，并把颜色写到当前选择。
    ///
    /// 填充写到形状 `fill` 和文本颜色，描边写到 `stroke`。缺的通道不改，也不清掉。
    /// 多选合成一条撤销记录。空选、全部锁定、全部隐藏、点在空白、采样没有颜色，
    /// 或颜色与目标相同，都返回 false，不入栈。
    ///
    /// 不切换工具。页面上的吸管工具在松开指针后再回到之前的工具。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const applied = canvas.apply_sampled_style_at(worldX, worldY);
    /// ```
    #[wasm_bindgen]
    pub fn apply_sampled_style_at(&mut self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let Some(sample) = sample_paint_at(&self.scene, Vec2::new(x, y)) else {
            return false;
        };
        self.apply_paint_sample(&sample)
    }
}

impl InfiniteCanvas {
    /// 把一次采样写进当前选择。采样没有颜色时不提交进行中的样式预览。
    pub(crate) fn apply_paint_sample(&mut self, sample: &SampledPaint) -> bool {
        if sample.fill.is_none() && sample.stroke.is_none() {
            return false;
        }
        self.commit_style_preview();
        let textures = self.snapshot_text_textures();
        let applied = apply_sampled_paint(
            &mut self.command_history,
            &mut self.scene,
            &self.selection.selected_ids(),
            sample,
        );
        self.release_text_textures_except(textures);
        applied
    }

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

fn write_hex(obj: &Object, name: &str, color: Option<Color>) {
    match color {
        Some(color) => js_set(obj, name, JsValue::from_str(&color.to_hex())),
        None => js_set(obj, name, JsValue::NULL),
    }
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

fn write_dash(obj: &Object, field: &StyleField<Vec<f32>>) {
    match field {
        StyleField::Absent => {
            js_set(obj, "strokeDash", JsValue::NULL);
            js_set(obj, "strokeDashMixed", JsValue::from(false));
            js_set(obj, "strokeDashSupported", JsValue::from(false));
        }
        StyleField::Mixed => {
            js_set(obj, "strokeDash", JsValue::NULL);
            js_set(obj, "strokeDashMixed", JsValue::from(true));
            js_set(obj, "strokeDashSupported", JsValue::from(true));
        }
        StyleField::Uniform(values) => {
            let array = Array::new();
            for value in values {
                array.push(&JsValue::from(*value));
            }
            js_set(obj, "strokeDash", array.into());
            js_set(obj, "strokeDashMixed", JsValue::from(false));
            js_set(obj, "strokeDashSupported", JsValue::from(true));
        }
    }
}

fn write_label<T: Copy>(
    obj: &Object,
    name: &str,
    field: &StyleField<T>,
    label: fn(T) -> &'static str,
) {
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
            js_set(obj, name, JsValue::from_str(label(*value)));
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
        stroke_dash: read_dash(value, "strokeDash"),
        stroke_cap: read_label(value, "strokeCap", StrokeCap::parse),
        stroke_join: read_label(value, "strokeJoin", StrokeJoin::parse),
        corner_radius: read_f32(value, "cornerRadius").or_else(|| read_f32(value, "corner_radius")),
        sides: read_sides(value),
        inner_radius: read_f32(value, "innerRadius").or_else(|| read_f32(value, "inner_radius")),
        shadow_offset_x: read_f32(value, "shadowOffsetX")
            .or_else(|| read_f32(value, "shadow_offset_x")),
        shadow_offset_y: read_f32(value, "shadowOffsetY")
            .or_else(|| read_f32(value, "shadow_offset_y")),
        shadow_blur: read_f32(value, "shadowBlur").or_else(|| read_f32(value, "shadow_blur")),
        shadow_color: read_color(value, "shadowColor")
            .or_else(|| read_color(value, "shadow_color")),
        shadow_opacity: read_f32(value, "shadowOpacity")
            .or_else(|| read_f32(value, "shadow_opacity")),
    })
}

fn read_sides(value: &JsValue) -> Option<u32> {
    let number = read_f32(value, "sides").or_else(|| read_f32(value, "points"))?;
    if !number.is_finite() {
        return None;
    }
    let rounded = number.round();
    if rounded < 0.0 || rounded > u32::MAX as f32 {
        return None;
    }
    Some(rounded as u32)
}

fn read_dash(value: &JsValue, key: &str) -> Option<Vec<f32>> {
    if !has_key(value, key) {
        return None;
    }
    let current = Reflect::get(value, &JsValue::from_str(key)).ok()?;
    if current.is_null() || current.is_undefined() {
        return Some(Vec::new());
    }
    if !Array::is_array(&current) {
        return None;
    }
    let array = Array::from(&current);
    let mut dash = Vec::with_capacity(array.length() as usize);
    for item in array.iter() {
        let Some(number) = item.as_f64() else {
            continue;
        };
        if number.is_finite() {
            dash.push(number as f32);
        }
    }
    Some(dash)
}

fn read_label<T>(value: &JsValue, key: &str, parse: fn(&str) -> Option<T>) -> Option<T> {
    if !has_key(value, key) {
        return None;
    }
    let current = Reflect::get(value, &JsValue::from_str(key)).ok()?;
    let text = current.as_string()?;
    parse(&text)
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
