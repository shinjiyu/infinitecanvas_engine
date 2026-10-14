//! 文本元素
//!
//! 文本不在 GPU 里直接排版。元素保存内容、字号、颜色和对齐，
//! 由浏览器 Canvas2D / OffscreenCanvas 按缩放层级光栅化成 RGBA 纹理，
//! 再交给现有的纹理管线绘制。SDF 留到后续。
//!
//! ```text
//! content + font + zoom
//!         │
//!         ▼
//!   LOD bucket (1/4 octave)
//!         │
//!         ▼
//!   Canvas2D raster ──upload──► TextureManager
//!         │
//!         ▼
//!   textured quad (world size stays stable)
//! ```

use crate::blend::BlendMode;
use font8x8::UnicodeFonts;
use infinitecanvas_math::{Color, Rect, Transform, Vec2, Vertex};
use serde::{Deserialize, Serialize};

use super::shadow::quad_shadow;
use super::DropShadow;
use web_time::{Duration, Instant};

use super::{Element, ElementId, ElementProps};

/// 每个二倍缩放切成几档。4 档时相邻档约差 19%，档内最大缩放误差约 9%。
pub const LOD_STEPS_PER_OCTAVE: f32 = 4.0;

/// 新文本的默认字号（世界单位）。缩放 1 时约等于 32 个屏幕像素。
pub const DEFAULT_FONT_SIZE: f32 = 32.0;

/// 画布底色是深色，默认文字用浅色才看得见。
pub const DEFAULT_TEXT_COLOR: Color = Color::new(0.957, 0.969, 0.984, 1.0);

/// 点击创建时的占位内容。双击或创建后的编辑框会选中它。
pub const DEFAULT_TEXT_CONTENT: &str = "Text";

/// 字号的内边距比例。光栅化时按世界单位换算成像素，避免抗锯齿被裁掉，
/// 同时让不同 LOD 的世界尺寸保持稳定。
pub const TEXT_PADDING_EM: f32 = 0.15;

/// 能画中文和英文的字体栈。前面是常见的 CJK 字体，最后回退到 sans-serif。
pub const DEFAULT_FONT_FAMILY: &str = "\"WenQuanYi Micro Hei\", \"Noto Sans CJK SC\", \"Noto Sans SC\", \"Source Han Sans SC\", \"PingFang SC\", \"Microsoft YaHei\", \"Droid Sans Fallback\", sans-serif";

/// 单张文字纹理的边长上限，留在 WebGPU 常见限制之内。
const MAX_RASTER_EDGE: f32 = 4096.0;

/// 水平对齐。
///
/// 对齐同时决定两件事：点击锚点相对文字块的位置，以及多行里较短行在块内的位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    /// 锚点在文字块左上角。
    #[default]
    Left,
    /// 锚点在文字块上边中点。
    Center,
    /// 锚点在文字块右上角。
    Right,
}

impl TextAlign {
    /// 传给 Canvas2D `textAlign` 和前端编辑框的名字。
    pub fn as_str(self) -> &'static str {
        match self {
            TextAlign::Left => "left",
            TextAlign::Center => "center",
            TextAlign::Right => "right",
        }
    }

    /// 从工具或接口字符串解析对齐。无法识别时返回 `None`。
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "left" | "start" => Some(TextAlign::Left),
            "center" | "middle" => Some(TextAlign::Center),
            "right" | "end" => Some(TextAlign::Right),
            _ => None,
        }
    }
}

/// 把缩放量化到 LOD 档。
///
/// 同一档内不重新光栅化。档位按 `log2(zoom)` 四舍五入到 1/4，
/// 所以放大约 19% 才会升一档，文字保持接近 1:1 像素而不会每滚一下就重画。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::element::{lod_bucket, lod_scale};
///
/// assert_eq!(lod_bucket(1.0), 0);
/// assert!((lod_scale(0) - 1.0).abs() < 1e-5);
/// assert_eq!(lod_bucket(2.0), 4);
/// ```
pub fn lod_bucket(zoom: f32) -> i32 {
    let zoom = if zoom.is_finite() { zoom } else { 1.0 };
    let zoom = zoom.max(1.0e-3);
    (zoom.log2() * LOD_STEPS_PER_OCTAVE).round() as i32
}

/// 某一档对应的像素 / 世界单位比例。
pub fn lod_scale(bucket: i32) -> f32 {
    let bucket = bucket.clamp(-32, 32);
    2f32.powf(bucket as f32 / LOD_STEPS_PER_OCTAVE)
}

/// 光栅化时在文字周围留出的世界单位边距。
pub fn padding_world(font_size: f32) -> f32 {
    font_size.max(1.0) * TEXT_PADDING_EM
}

/// 一次光栅化要交给 Canvas2D 的参数。
///
/// `scale` 是这次实际使用的像素/世界单位比例。它通常等于 [`lod_scale`]，
/// 超长文本会被缩小，保证纹理边长不超过 4096。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterParams {
    /// 请求对应的 LOD 档。
    pub lod: i32,
    /// 实际像素 / 世界单位。
    pub scale: f32,
    /// 传给 `ctx.font` 的像素字号。
    pub font_size_px: f32,
    /// 世界单位边距。前端再乘 `scale` 得到像素边距。
    pub padding_world: f32,
}

/// 按字号、LOD 和内容计算光栅化参数。
///
/// 世界尺寸 = 纹理像素 / `scale`。调用方用这个比例回写元素尺寸，
/// 这样换档之后四边形大小不变，只是纹理更清晰。
pub fn raster_params(font_size: f32, lod: i32, content: &str) -> RasterParams {
    let font_size = font_size.max(1.0);
    let mut scale = lod_scale(lod);
    let mut font_px = font_size * scale;
    if font_px < 4.0 {
        font_px = 4.0;
        scale = font_px / font_size;
    } else if font_px > 512.0 {
        font_px = 512.0;
        scale = font_px / font_size;
    }

    let padding = padding_world(font_size) * scale;
    let max_chars = max_line_chars(content).max(1) as f32;
    let lines = line_count(content).max(1) as f32;
    let est_w = max_chars * font_px + padding * 2.0;
    let est_h = lines * font_px * 1.25 + padding * 2.0;
    let fit = (MAX_RASTER_EDGE / est_w.max(1.0))
        .min(MAX_RASTER_EDGE / est_h.max(1.0))
        .min(1.0);
    if fit < 1.0 {
        font_px *= fit;
        scale *= fit;
    }

    RasterParams {
        lod,
        scale,
        font_size_px: font_px,
        padding_world: padding_world(font_size),
    }
}

/// 纹理 ID。包含元素、内容版本和 LOD，避免覆盖仍在使用的旧纹理。
pub fn raster_texture_id(element_id: ElementId, revision: u64, lod: i32) -> String {
    format!("text-{element_id}-r{revision}-l{lod}")
}

/// 在第一次真实测量回来之前，用字号估算命中盒。
///
/// 中文大约 1em 宽，英文更窄。估算宁大勿小，提交光栅化结果后会被替换。
pub fn estimate_size(content: &str, font_size: f32) -> Vec2 {
    let font_size = font_size.max(1.0);
    let chars = max_line_chars(content).max(1) as f32;
    let lines = line_count(content).max(1) as f32;
    Vec2::new(font_size * chars, font_size * 1.25 * lines)
}

fn line_count(content: &str) -> usize {
    if content.is_empty() {
        1
    } else {
        content.split('\n').count()
    }
}

fn max_line_chars(content: &str) -> usize {
    content
        .split('\n')
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0)
}

/// [`Text::commit_raster`] 成功时的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedRaster {
    /// 被换下来的纹理 ID。调用方应释放它。
    pub previous: Option<String>,
}

/// 可撤销的文字外观。不含纹理：撤销后按当前 LOD 重新光栅化。
#[derive(Debug, Clone)]
pub struct TextVisual {
    /// 文本内容，保留换行。
    pub content: String,
    /// 世界单位字号。
    pub font_size: f32,
    /// 填充色。
    pub color: Color,
    /// 水平对齐。
    pub align: TextAlign,
    /// CSS font-family。
    pub font_family: String,
    /// 世界单位布局尺寸。
    pub size: Vec2,
}

fn default_font_size() -> f32 {
    DEFAULT_FONT_SIZE
}

fn default_text_color() -> Color {
    DEFAULT_TEXT_COLOR
}

fn default_text_opacity() -> f32 {
    1.0
}

fn default_font_family() -> String {
    DEFAULT_FONT_FAMILY.to_string()
}

/// 文本元素。
///
/// 位置是锚点，不是几何中心。左对齐时锚点在左上角，居中和右对齐分别在上边中点、右上角。
///
/// 场景 JSON 只保存排版字段。`texture_id`、LOD、`revision`、`editing` 用 `#[serde(skip)]`，
/// 导入后按当前缩放重新光栅化。缺省的 `font_size`、`color`、`opacity`、`stroke`、
/// `stroke_width`、`align`、`font_family`、`size` 使用 serde 默认值；未知字段会被忽略。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::element::{Text, TextAlign};
/// use infinitecanvas_math::Vec2;
///
/// let text = Text::new(1, Vec2::new(10.0, 20.0), "Hello 你好")
///     .with_font_size(24.0)
///     .with_align(TextAlign::Left);
/// assert_eq!(text.content, "Hello 你好");
/// assert!(text.needs_raster(0));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    /// 元素属性。
    pub props: ElementProps,
    /// 文本内容。JSON 中可省略，缺省为空字符串。
    #[serde(default)]
    pub content: String,
    /// 世界单位字号。JSON 中可省略，缺省为 [`DEFAULT_FONT_SIZE`]。
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    /// 文字颜色。JSON 中可省略，缺省为浅色。
    #[serde(default = "default_text_color")]
    pub color: Color,
    /// 元素不透明度，乘在纹理 tint 上。JSON 中可省略，缺省为 1。
    #[serde(default = "default_text_opacity")]
    pub opacity: f32,
    /// 描边色。JSON 中可省略。`None` 且线宽大于 0 时，光栅用填充色描边。
    #[serde(default)]
    pub stroke: Option<Color>,
    /// 世界单位描边宽度。JSON 中可省略，缺省为 0（不描边）。
    #[serde(default)]
    pub stroke_width: f32,
    /// 混合模式。JSON 可省略，缺省 [`BlendMode::Normal`]。Normal 不写出。
    #[serde(
        default = "crate::blend::default_blend_mode",
        skip_serializing_if = "crate::blend::blend_is_normal"
    )]
    pub blend_mode: BlendMode,
    /// 水平对齐。JSON 中可省略，缺省为左对齐。值为 `left` / `center` / `right`。
    #[serde(default)]
    pub align: TextAlign,
    /// CSS font-family。JSON 中可省略。
    #[serde(default = "default_font_family")]
    pub font_family: String,
    /// 文字块世界尺寸。JSON 中可省略；省略或非正时按内容和字号估算。
    #[serde(default)]
    pub size: Vec2,
    /// 当前纹理。`None` 表示还没光栅化，或内容已变。不写入场景 JSON。
    #[serde(skip)]
    pub texture_id: Option<String>,
    /// 当前纹理对应的 LOD 档。不写入场景 JSON。
    #[serde(skip)]
    pub raster_lod: Option<i32>,
    /// 内容版本。编辑后递增，用来丢弃过期的光栅化结果。不写入场景 JSON。
    #[serde(skip)]
    pub revision: u64,
    /// 纹理对应的内容版本。不写入场景 JSON。
    #[serde(skip)]
    pub raster_revision: u64,
    /// 已经发给前端、还没提交的 LOD。不写入场景 JSON。
    #[serde(skip)]
    pub pending_lod: Option<i32>,
    /// 待提交任务对应的内容版本。不写入场景 JSON。
    #[serde(skip)]
    pub pending_revision: u64,
    /// 为 true 时不绘制纹理，把画面让给 DOM 编辑框。不写入场景 JSON。
    #[serde(skip)]
    pub editing: bool,
    /// 投影。缺省关闭，关闭时不写入 JSON。命中测试不读它。
    ///
    /// 有纹理且不在编辑时，按文字块包围盒画偏移羽化，不进字形光栅。
    #[serde(default, skip_serializing_if = "DropShadow::is_unset")]
    pub shadow: DropShadow,
}

impl Text {
    /// 创建文本。锚点由 `position` 给出，默认左对齐、32 世界单位、浅色。
    pub fn new(id: ElementId, position: Vec2, content: impl Into<String>) -> Self {
        let content = content.into();
        let font_size = DEFAULT_FONT_SIZE;
        let mut props = ElementProps::new(id);
        props.transform.translation = position;
        Self {
            props,
            size: estimate_size(&content, font_size),
            content,
            font_size,
            color: DEFAULT_TEXT_COLOR,
            opacity: 1.0,
            stroke: None,
            stroke_width: 0.0,
            blend_mode: BlendMode::Normal,
            align: TextAlign::Left,
            font_family: DEFAULT_FONT_FAMILY.to_string(),
            texture_id: None,
            raster_lod: None,
            revision: 1,
            raster_revision: 0,
            pending_lod: None,
            pending_revision: 0,
            editing: false,
            shadow: DropShadow::default(),
        }
    }

    /// 设置世界单位字号。还没有纹理时同步更新估算尺寸。
    pub fn with_font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size.max(1.0);
        if self.texture_id.is_none() {
            self.size = estimate_size(&self.content, self.font_size);
        }
        self
    }

    /// 设置文字颜色。
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// 设置水平对齐。
    pub fn with_align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// 设置 CSS font-family。
    pub fn with_font_family(mut self, font_family: impl Into<String>) -> Self {
        self.font_family = font_family.into();
        self
    }

    /// 把从 JSON 读入的文本收成和 [`Text::new`] 一样的运行时状态。
    ///
    /// 字号小于 1 或非有限时抬到可用值。尺寸缺失时按内容和字号估算。
    /// 纹理、LOD 和编辑标记一律清掉，`revision` 从 1 开始，导入后会重新光栅化。
    pub(crate) fn finish_load(&mut self) {
        if !self.font_size.is_finite() {
            self.font_size = DEFAULT_FONT_SIZE;
        } else if self.font_size < 1.0 {
            self.font_size = 1.0;
        }
        if self.font_family.trim().is_empty() {
            self.font_family = DEFAULT_FONT_FAMILY.to_string();
        }
        if !self.opacity.is_finite() {
            self.opacity = 1.0;
        } else {
            self.opacity = self.opacity.clamp(0.0, 1.0);
        }
        if !self.stroke_width.is_finite() || self.stroke_width < 0.0 {
            self.stroke_width = 0.0;
        }
        if !self.size.x.is_finite()
            || !self.size.y.is_finite()
            || self.size.x <= 0.0
            || self.size.y <= 0.0
        {
            self.size = estimate_size(&self.content, self.font_size);
        }
        self.texture_id = None;
        self.raster_lod = None;
        self.revision = 1;
        self.raster_revision = 0;
        self.pending_lod = None;
        self.pending_revision = 0;
        self.editing = false;
        self.shadow = self.shadow.sanitized();
    }

    /// 丢掉纹理和编辑状态，保留文案、字号和对齐。
    ///
    /// 克隆出来的文本必须重新光栅化。纹理 ID 含着原来的元素 ID，
    /// 继续共用会在原文本删除或改字时把副本的字形一起释放。
    pub(crate) fn detach_raster_for_clone(&mut self) {
        self.texture_id = None;
        self.raster_lod = None;
        self.raster_revision = 0;
        self.pending_lod = None;
        self.pending_revision = 0;
        self.editing = false;
    }

    /// 世界坐标处的 PNG 颜色。
    ///
    /// 屏幕上的字形在 Canvas2D 纹理里，不进场景文件。这里用 `font8x8` 的 8×8 点阵
    /// 把该 crate 覆盖的字符画进文字块；没有点阵的字符（包括中文）画成方框。
    /// 旋转和缩放走 [`Transform`] 的逆变换。不可见、完全透明或落在笔画外时返回 `None`。
    ///
    /// # 参数
    /// - `world`: 世界坐标
    pub fn color_at(&self, world: Vec2) -> Option<Color> {
        if !self.props.visible || self.color.a <= 0.0 || self.opacity <= 0.0 {
            return None;
        }
        let local = local_point(&self.props.transform, world)?;
        let (bounds, cell_w, cell_h) = cell_metrics(self)?;
        if !bounds.contains(local) {
            return None;
        }
        let line_index = ((local.y - bounds.y) / cell_h).floor() as usize;
        let line = self.content.split('\n').nth(line_index)?;
        if line.is_empty() {
            return None;
        }
        let origin_x = line_origin_x(self, bounds, cell_w, line);
        let rel_x = local.x - origin_x;
        if rel_x < 0.0 {
            return None;
        }
        let char_index = (rel_x / cell_w).floor() as usize;
        let ch = line.chars().nth(char_index)?;
        let u = rel_x / cell_w - char_index as f32;
        let v = (local.y - bounds.y) / cell_h - line_index as f32;
        if glyph_covers(ch, u, v) {
            let mut color = self.color;
            color.a = (color.a * self.opacity.clamp(0.0, 1.0)).clamp(0.0, 1.0);
            Some(color)
        } else {
            None
        }
    }

    /// 某个字形像素中心的局部坐标，供 PNG 测试对准笔画。
    ///
    /// `col` / `row` 是 8×8 点阵里的列和行，0 是左上。
    #[cfg(test)]
    pub(crate) fn glyph_sample_local(
        &self,
        line_index: usize,
        char_index: usize,
        col: u32,
        row: u32,
    ) -> Option<Vec2> {
        if col >= 8 || row >= 8 {
            return None;
        }
        let (bounds, cell_w, cell_h) = cell_metrics(self)?;
        let line = self.content.split('\n').nth(line_index)?;
        line.chars().nth(char_index)?;
        let (u, v) = glyph_pixel_uv(col, row);
        let origin_x = line_origin_x(self, bounds, cell_w, line);
        Some(Vec2::new(
            origin_x + (char_index as f32 + u) * cell_w,
            bounds.y + (line_index as f32 + v) * cell_h,
        ))
    }

    /// 复制当前外观，供撤销命令保存。
    pub fn visual(&self) -> TextVisual {
        TextVisual {
            content: self.content.clone(),
            font_size: self.font_size,
            color: self.color,
            align: self.align,
            font_family: self.font_family.clone(),
            size: self.size,
        }
    }

    /// 恢复外观并丢掉纹理。调用方负责释放不再被引用的 GPU 纹理。
    ///
    /// # 返回
    /// 被替换掉的纹理 ID。
    pub fn apply_visual(&mut self, visual: &TextVisual) -> Option<String> {
        self.content = visual.content.clone();
        self.font_size = visual.font_size.max(1.0);
        self.color = visual.color;
        self.align = visual.align;
        self.font_family = visual.font_family.clone();
        self.size = visual.size;
        self.editing = false;
        self.invalidate_raster()
    }

    /// 修改内容。内容没变时返回 `None` 且不增加版本号。
    ///
    /// # 返回
    /// 失效的旧纹理 ID。
    pub fn set_content(&mut self, content: impl Into<String>) -> Option<String> {
        let content = content.into();
        if self.content == content {
            return None;
        }
        self.content = content;
        self.size = estimate_size(&self.content, self.font_size);
        self.invalidate_raster()
    }

    /// 写入属性面板改过的外观。
    ///
    /// 字号、填充色或描边变化时丢掉纹理，下一帧按新参数光栅化。
    /// 只改不透明度时保留纹理，绘制用 tint。
    pub(crate) fn set_paint(
        &mut self,
        font_size: f32,
        color: Color,
        opacity: f32,
        stroke: Option<Color>,
        stroke_width: f32,
    ) {
        let font_size = if font_size.is_finite() {
            font_size.max(1.0)
        } else {
            self.font_size
        };
        let stroke_width = if stroke_width.is_finite() {
            stroke_width.max(0.0)
        } else {
            self.stroke_width
        };
        let opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            self.opacity
        };
        let raster_changed = self.font_size != font_size
            || self.color != color
            || self.stroke != stroke
            || self.stroke_width != stroke_width;
        self.font_size = font_size;
        self.color = color;
        self.opacity = opacity;
        self.stroke = stroke;
        self.stroke_width = stroke_width;
        if raster_changed {
            self.size = estimate_size(&self.content, self.font_size);
            let _ = self.invalidate_raster();
        }
    }

    /// 进入或离开 DOM 编辑。编辑期间不请求光栅化，避免把旧字形画在输入框下面。
    pub fn set_editing(&mut self, editing: bool) {
        self.editing = editing;
    }

    /// 当前 LOD 是否还没有一张匹配内容版本的纹理。
    pub fn needs_raster(&self, lod: i32) -> bool {
        if self.editing || self.content.is_empty() {
            return false;
        }
        let fresh = self.texture_id.is_some()
            && self.raster_lod == Some(lod)
            && self.raster_revision == self.revision;
        if fresh {
            return false;
        }
        self.pending_lod != Some(lod) || self.pending_revision != self.revision
    }

    /// 标记这一档已经发给前端。
    pub fn mark_raster_pending(&mut self, lod: i32) {
        self.pending_lod = Some(lod);
        self.pending_revision = self.revision;
    }

    /// 前端光栅化失败时清除等待标记，下一帧可以重试。
    pub fn abandon_raster(&mut self) {
        self.pending_lod = None;
    }

    /// 提交一张光栅化结果。
    ///
    /// `revision` 必须等于当前内容版本，否则视为过期任务并返回 `None`。
    ///
    /// # 返回
    /// `None` 表示版本过期，纹理没有绑定。
    pub fn commit_raster(
        &mut self,
        revision: u64,
        lod: i32,
        texture_id: String,
        world_size: Vec2,
    ) -> Option<AppliedRaster> {
        if revision != self.revision {
            if self.pending_revision != self.revision {
                self.pending_lod = None;
            }
            return None;
        }
        let previous = self.texture_id.replace(texture_id);
        self.raster_lod = Some(lod);
        self.raster_revision = revision;
        if world_size.x.is_finite()
            && world_size.y.is_finite()
            && world_size.x > 0.0
            && world_size.y > 0.0
        {
            self.size = world_size;
        }
        if self.pending_lod == Some(lod) && self.pending_revision == revision {
            self.pending_lod = None;
        }
        Some(AppliedRaster {
            previous: previous.filter(|id| self.texture_id.as_deref() != Some(id.as_str())),
        })
    }

    /// 使当前纹理失效。
    ///
    /// # 返回
    /// 需要释放的纹理 ID。
    pub fn invalidate_raster(&mut self) -> Option<String> {
        self.revision = self.revision.saturating_add(1);
        self.raster_lod = None;
        self.raster_revision = 0;
        self.pending_lod = None;
        self.pending_revision = 0;
        self.texture_id.take()
    }

    /// 文字块的投影。没有纹理或正在编辑时为空，和 [`Self::generate_vertices`] 一起出现。
    ///
    /// 这是包围盒的偏移羽化，不是逐字形模糊。命中测试仍按文字块，不含偏移。
    pub(crate) fn shadow_vertices(&self) -> Vec<Vertex> {
        if self.editing || self.texture_id.is_none() || !self.props.visible {
            return Vec::new();
        }
        let bounds = self.local_bounds();
        quad_shadow(
            Vec2::new(bounds.x, bounds.y),
            Vec2::new(bounds.width, bounds.height),
            self.shadow,
            self.opacity,
            &self.props.transform,
        )
    }

    /// 生成纹理四边形。没有纹理或正在编辑时返回空，渲染器就跳过它。
    pub fn generate_vertices(&self) -> Vec<Vertex> {
        if self.editing || self.texture_id.is_none() || !self.props.visible {
            return Vec::new();
        }
        let bounds = self.local_bounds();
        let transform = &self.props.transform;
        let corners = [
            transform.transform_point(Vec2::new(bounds.x, bounds.y)),
            transform.transform_point(Vec2::new(bounds.x + bounds.width, bounds.y)),
            transform.transform_point(Vec2::new(bounds.x, bounds.y + bounds.height)),
            transform.transform_point(Vec2::new(bounds.x + bounds.width, bounds.y + bounds.height)),
        ];
        // 字形颜色在纹理里。tint 的 alpha 是元素不透明度，拖动滑块不必重新光栅化。
        let color = Color::new(1.0, 1.0, 1.0, self.opacity.clamp(0.0, 1.0));
        let v0 = Vertex::with_uv(corners[0].x, corners[0].y, color, 0.0, 0.0);
        let v1 = Vertex::with_uv(corners[1].x, corners[1].y, color, 1.0, 0.0);
        let v2 = Vertex::with_uv(corners[2].x, corners[2].y, color, 0.0, 1.0);
        let v3 = Vertex::with_uv(corners[3].x, corners[3].y, color, 1.0, 1.0);
        vec![v0, v2, v1, v1, v2, v3]
    }
}

impl Element for Text {
    fn props(&self) -> &ElementProps {
        &self.props
    }

    fn props_mut(&mut self) -> &mut ElementProps {
        &mut self.props
    }

    fn local_bounds(&self) -> Rect {
        // 尺寸来自光栅化测量。不要抬到一个 em，否则纹理会在垂直方向被拉高。
        let width = self.size.x.max(1.0);
        let height = self.size.y.max(1.0);
        let x = match self.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => -width / 2.0,
            TextAlign::Right => -width,
        };
        Rect::new(x, 0.0, width, height)
    }

    fn type_name(&self) -> &'static str {
        "Text"
    }
}

/// T 工具一次点击的结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextPlacement {
    /// 在空白处创建新文本。
    Create(Vec2),
    /// 编辑已有文本。`select_all` 为 true 时选中全部占位文字。
    Edit { id: ElementId, select_all: bool },
}

/// 点阵周围留出的单元格比例，避免笔画贴边。
const GLYPH_MARGIN: f32 = 0.08;

fn cell_metrics(text: &Text) -> Option<(Rect, f32, f32)> {
    let bounds = text.local_bounds();
    let line_count = line_count(&text.content).max(1) as f32;
    let max_chars = max_line_chars(&text.content).max(1) as f32;
    let cell_w = bounds.width / max_chars;
    let cell_h = bounds.height / line_count;
    if cell_w.is_finite() && cell_h.is_finite() && cell_w > 0.0 && cell_h > 0.0 {
        Some((bounds, cell_w, cell_h))
    } else {
        None
    }
}

fn line_origin_x(text: &Text, bounds: Rect, cell_w: f32, line: &str) -> f32 {
    let line_w = line.chars().count() as f32 * cell_w;
    let offset = match text.align {
        TextAlign::Left => 0.0,
        TextAlign::Center => (bounds.width - line_w) * 0.5,
        TextAlign::Right => bounds.width - line_w,
    };
    bounds.x + offset
}

#[cfg(test)]
fn glyph_pixel_uv(col: u32, row: u32) -> (f32, f32) {
    let span = 1.0 - 2.0 * GLYPH_MARGIN;
    let u = GLYPH_MARGIN + (col as f32 + 0.5) / 8.0 * span;
    let v = GLYPH_MARGIN + (row as f32 + 0.5) / 8.0 * span;
    (u, v)
}

fn uv_to_pixel(u: f32, v: f32) -> Option<(usize, usize)> {
    let span = 1.0 - 2.0 * GLYPH_MARGIN;
    let iu = (u - GLYPH_MARGIN) / span;
    let iv = (v - GLYPH_MARGIN) / span;
    if !(0.0..1.0).contains(&iu) || !(0.0..1.0).contains(&iv) {
        return None;
    }
    Some((
        (iu * 8.0).floor().clamp(0.0, 7.0) as usize,
        (iv * 8.0).floor().clamp(0.0, 7.0) as usize,
    ))
}

fn bitmap(ch: char) -> Option<[u8; 8]> {
    font8x8::BASIC_FONTS
        .get(ch)
        .or_else(|| font8x8::LATIN_FONTS.get(ch))
        .or_else(|| font8x8::GREEK_FONTS.get(ch))
        .or_else(|| font8x8::HIRAGANA_FONTS.get(ch))
        .or_else(|| font8x8::BLOCK_FONTS.get(ch))
        .or_else(|| font8x8::BOX_FONTS.get(ch))
        .or_else(|| font8x8::MISC_FONTS.get(ch))
        .or_else(|| font8x8::SGA_FONTS.get(ch))
}

/// `font8x8` 的最低位是最左一列。没有点阵时，8×8 外圈当方框。
fn glyph_covers(ch: char, u: f32, v: f32) -> bool {
    if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
        return false;
    }
    let Some((gx, gy)) = uv_to_pixel(u, v) else {
        return false;
    };
    match bitmap(ch) {
        Some(rows) => (rows[gy] >> gx) & 1 == 1,
        None => gx == 0 || gy == 0 || gx == 7 || gy == 7,
    }
}

fn local_point(transform: &Transform, world: Vec2) -> Option<Vec2> {
    if !transform.scale.x.is_finite()
        || !transform.scale.y.is_finite()
        || transform.scale.x.abs() < 1e-6
        || transform.scale.y.abs() < 1e-6
    {
        return None;
    }
    let local = transform.inverse_transform_point(world);
    if local.x.is_finite() && local.y.is_finite() {
        Some(local)
    } else {
        None
    }
}

/// 区分单击创建和双击编辑。
///
/// 浏览器的 dblclick 到来时，两次 pointerup 都已经进了队列。
/// 短时间内、很近的第二次点击复用刚创建的元素，而不是再叠一个。
///
/// 时间戳是 [`web_time::Instant`]。`std::time::Instant::now` 在 `wasm32-unknown-unknown` 上会 panic。
#[derive(Debug, Clone)]
pub struct TextClickResolver {
    recent_id: Option<ElementId>,
    recent_position: Vec2,
    recent_at: Option<Instant>,
}

impl TextClickResolver {
    /// 创建空的解析器。
    pub fn new() -> Self {
        Self {
            recent_id: None,
            recent_position: Vec2::ZERO,
            recent_at: None,
        }
    }

    /// 决定这次点击是创建还是编辑。
    ///
    /// `hit_text_id` 是该位置最上层的文本元素。450ms 内、16 世界单位内的重复点击
    /// 优先回到刚创建的元素。`now` 用 [`web_time::Instant::now`]，wasm32 上走 `Performance.now`。
    pub fn resolve(
        &self,
        position: Vec2,
        hit_text_id: Option<ElementId>,
        now: Instant,
    ) -> TextPlacement {
        if let (Some(id), Some(at)) = (self.recent_id, self.recent_at) {
            let close = (self.recent_position - position).length() < 16.0;
            let quick = now.saturating_duration_since(at) < Duration::from_millis(450);
            if close && quick {
                return TextPlacement::Edit {
                    id,
                    select_all: true,
                };
            }
        }
        if let Some(id) = hit_text_id {
            return TextPlacement::Edit {
                id,
                select_all: false,
            };
        }
        TextPlacement::Create(position)
    }

    /// 记住刚创建的文本，供紧接着的第二次点击识别成双击。
    pub fn remember(&mut self, id: ElementId, position: Vec2, now: Instant) {
        self.recent_id = Some(id);
        self.recent_position = position;
        self.recent_at = Some(now);
    }
}

impl Default for TextClickResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{AddElementCommand, CommandHistory, UpdateTextCommand};
    use crate::hit_test::{hit_test_point, HitTestOptions};
    use crate::{AnyElement, Scene};

    #[test]
    fn lod_buckets_change_when_zoom_doubles() {
        assert_eq!(lod_bucket(1.0), 0);
        assert_eq!(lod_bucket(1.02), 0);
        assert_eq!(lod_bucket(2.0), 4);
        assert!((lod_scale(lod_bucket(1.0)) - 1.0).abs() < 1e-5);
        assert!((lod_scale(lod_bucket(2.0)) - 2.0).abs() < 1e-4);
        let ratio = lod_scale(lod_bucket(1.2)) / 1.2;
        assert!(ratio > 0.9 && ratio < 1.1);
    }

    #[test]
    fn lod_bucket_rejects_non_finite_zoom() {
        assert_eq!(lod_bucket(f32::NAN), lod_bucket(1.0));
        assert_eq!(lod_bucket(-5.0), lod_bucket(0.001));
    }

    #[test]
    fn raster_params_shrink_long_runs() {
        let content = "你".repeat(400);
        let params = raster_params(32.0, lod_bucket(4.0), &content);
        let est = content.chars().count() as f32 * params.font_size_px
            + params.padding_world * params.scale * 2.0;
        assert!(est <= MAX_RASTER_EDGE + 1.0);
        assert!(params.font_size_px > 0.0);
    }

    #[test]
    fn align_parse_roundtrip() {
        assert_eq!(TextAlign::parse("center"), Some(TextAlign::Center));
        assert_eq!(TextAlign::parse("RIGHT"), Some(TextAlign::Right));
        assert_eq!(TextAlign::Center.as_str(), "center");
        assert!(TextAlign::parse("justify").is_none());
    }

    #[test]
    fn chinese_and_english_content_is_preserved() {
        let text = Text::new(1, Vec2::ZERO, "Hello 你好");
        assert_eq!(text.content, "Hello 你好");
        assert_eq!(text.type_name(), "Text");
        assert!(text.font_family.contains("WenQuanYi"));
        assert!(text.font_family.contains("sans-serif"));
    }

    #[test]
    fn hit_test_respects_alignment_and_top_most_text() {
        let mut scene = Scene::new();
        let text = Text::new(1, Vec2::new(100.0, 50.0), "Hi").with_font_size(20.0);
        let width = text.size.x;
        scene.add_text(text);

        assert_eq!(scene.find_at(Vec2::new(100.0 + width * 0.5, 55.0)), Some(1));
        assert_eq!(scene.find_at(Vec2::new(90.0, 55.0)), None);

        let mut centered = Text::new(2, Vec2::new(100.0, 50.0), "Hi")
            .with_font_size(20.0)
            .with_align(TextAlign::Center);
        centered.props_mut().z_index = 5;
        let half = centered.size.x / 2.0;
        scene.add_text(centered);
        assert!(scene
            .get(2)
            .unwrap()
            .as_element()
            .contains_point(Vec2::new(100.0 - half + 1.0, 55.0)));

        let hits = hit_test_point(&scene, Vec2::new(100.0, 55.0), &HitTestOptions::default());
        assert_eq!(hits[0].id, 2);
        assert_eq!(hits[0].element_type, "Text");
    }

    #[test]
    fn raster_invalidation_follows_content_and_lod() {
        let mut text = Text::new(1, Vec2::ZERO, "Hello");
        let lod = lod_bucket(1.0);
        assert!(text.needs_raster(lod));
        text.mark_raster_pending(lod);
        assert!(!text.needs_raster(lod));

        let previous = text
            .commit_raster(
                text.revision,
                lod,
                "tex-a".to_string(),
                Vec2::new(40.0, 16.0),
            )
            .unwrap();
        assert!(previous.previous.is_none());
        assert!(!text.needs_raster(lod));
        assert!((text.size.x - 40.0).abs() < 1e-4);

        let zoomed = lod_bucket(2.0);
        assert_ne!(lod, zoomed);
        assert!(text.needs_raster(zoomed));

        text.mark_raster_pending(zoomed);
        let replaced = text
            .commit_raster(
                text.revision,
                zoomed,
                "tex-b".to_string(),
                Vec2::new(40.0, 16.0),
            )
            .unwrap();
        assert_eq!(replaced.previous.as_deref(), Some("tex-a"));

        assert!(text.set_content("你好").is_some());
        assert_eq!(text.content, "你好");
        assert!(text.texture_id.is_none());
        assert!(text.needs_raster(zoomed));
    }

    #[test]
    fn stale_raster_is_rejected_and_editing_suppresses_jobs() {
        let mut text = Text::new(1, Vec2::ZERO, "Hello");
        let lod = 0;
        text.mark_raster_pending(lod);
        assert!(text
            .commit_raster(999, lod, "late".into(), Vec2::new(10.0, 10.0))
            .is_none());
        assert!(text.texture_id.is_none());
        // 当前版本的任务还在飞，错误版本的结果不能把它清掉。
        assert!(!text.needs_raster(lod));
        text.abandon_raster();
        assert!(text.needs_raster(lod));

        text.set_editing(true);
        assert!(!text.needs_raster(lod));
        assert!(text.generate_vertices().is_empty());
    }

    #[test]
    fn vertices_cover_the_committed_quad() {
        let mut text = Text::new(1, Vec2::new(10.0, 20.0), "A");
        text.commit_raster(text.revision, 0, "tex".into(), Vec2::new(30.0, 12.0))
            .unwrap();
        let vertices = text.generate_vertices();
        assert_eq!(vertices.len(), 6);
        assert_eq!(vertices[0].position, [10.0, 20.0]);
        assert_eq!(vertices[0].uv, [0.0, 0.0]);
        assert_eq!(vertices[5].position, [40.0, 32.0]);
        assert_eq!(vertices[5].color, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn add_and_content_edits_are_undoable() {
        let mut scene = Scene::new();
        let mut history = CommandHistory::new();
        let text = Text::new(1, Vec2::new(8.0, 9.0), "Hello");
        history.execute(
            Box::new(AddElementCommand::new(AnyElement::Text(text))),
            &mut scene,
        );
        assert_eq!(scene.count(), 1);

        let before = match scene.get(1) {
            Some(AnyElement::Text(text)) => text.visual(),
            _ => panic!("text missing"),
        };
        {
            let Some(AnyElement::Text(text)) = scene.get_mut(1) else {
                panic!("text missing");
            };
            text.set_content("Hello 你好");
        }
        let after = match scene.get(1) {
            Some(AnyElement::Text(text)) => text.visual(),
            _ => panic!("text missing"),
        };
        history.push_executed(Box::new(UpdateTextCommand::new(1, before, after)));

        match scene.get(1) {
            Some(AnyElement::Text(text)) => assert_eq!(text.content, "Hello 你好"),
            _ => panic!("text missing"),
        }
        history.undo(&mut scene);
        match scene.get(1) {
            Some(AnyElement::Text(text)) => {
                assert_eq!(text.content, "Hello");
                assert!(text.texture_id.is_none());
            }
            _ => panic!("text missing"),
        }
        history.undo(&mut scene);
        assert!(scene.get(1).is_none());
        history.redo(&mut scene);
        assert!(scene.get(1).is_some());
    }

    #[test]
    fn click_resolver_creates_then_treats_second_tap_as_edit() {
        let mut resolver = TextClickResolver::new();
        let t0 = Instant::now();
        assert_eq!(
            resolver.resolve(Vec2::new(10.0, 10.0), None, t0),
            TextPlacement::Create(Vec2::new(10.0, 10.0))
        );
        resolver.remember(7, Vec2::new(10.0, 10.0), t0);
        assert_eq!(
            resolver.resolve(Vec2::new(12.0, 11.0), None, t0 + Duration::from_millis(120)),
            TextPlacement::Edit {
                id: 7,
                select_all: true
            }
        );
        assert_eq!(
            resolver.resolve(
                Vec2::new(80.0, 10.0),
                Some(3),
                t0 + Duration::from_millis(10)
            ),
            TextPlacement::Edit {
                id: 3,
                select_all: false
            }
        );
    }

    #[test]
    fn color_at_hits_font_ink_and_boxes_uncovered_characters() {
        let text = Text::new(1, Vec2::new(4.0, 6.0), "H")
            .with_font_size(64.0)
            .with_color(Color::RED);
        let glyph = font8x8::BASIC_FONTS.get('H').unwrap();
        assert_eq!(glyph[0], 0x33);
        assert_eq!(glyph[0] & (1 << 2), 0);
        assert_ne!(glyph[3] & (1 << 2), 0);

        let ink = text
            .props
            .transform
            .transform_point(text.glyph_sample_local(0, 0, 0, 0).unwrap());
        let gap = text
            .props
            .transform
            .transform_point(text.glyph_sample_local(0, 0, 2, 0).unwrap());
        let bar = text
            .props
            .transform
            .transform_point(text.glyph_sample_local(0, 0, 2, 3).unwrap());
        assert_eq!(text.color_at(ink), Some(Color::RED));
        assert_eq!(text.color_at(gap), None);
        assert_eq!(text.color_at(bar), Some(Color::RED));

        let han = Text::new(2, Vec2::ZERO, "你")
            .with_font_size(64.0)
            .with_color(Color::BLACK);
        assert!(bitmap('你').is_none());
        let border = han
            .props
            .transform
            .transform_point(han.glyph_sample_local(0, 0, 0, 0).unwrap());
        let hole = han
            .props
            .transform
            .transform_point(han.glyph_sample_local(0, 0, 3, 3).unwrap());
        assert_eq!(han.color_at(border), Some(Color::BLACK));
        assert_eq!(han.color_at(hole), None);
    }
}
