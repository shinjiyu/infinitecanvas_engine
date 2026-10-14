//! 场景持久化
//!
//! 把场景写成带 `version` 的 JSON，并可以把视口或选中区域栅格化成 PNG。
//!
//! ```text
//! Scene ── to_json ──▶ { version, next_id, elements[] }
//!                          │  type + data
//!                          ▼
//!                     rectangle / circle / image / line / path / text / group
//! Scene ◀─ from_json ── 缺省字段用 serde default，未知字段忽略
//! ```
//!
//! 只接受 [`SCENE_FORMAT_VERSION`]。新增可选字段时保持同一版本，并为字段加上
//! `#[serde(default)]`。新增元素种类时扩展解码分支；未知种类会报错，避免静默丢数据。

use crate::element::{Circle, Element, Image, Rectangle, Text};
use crate::scene::{AnyElement, Scene};
use crate::{ElementId, Viewport};
use infinitecanvas_math::{Color, Rect, Transform, Vec2};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

/// 当前场景 JSON 版本。不兼容的结构必须递增。
pub const SCENE_FORMAT_VERSION: u32 = 1;

/// PNG 单边像素上限，避免一次导出分配过大的缓冲。
const MAX_PNG_DIMENSION: u32 = 2048;

/// 选区导出在世界包围盒外再留出的边距，避免描边被裁掉。
const SELECTION_PADDING: f32 = 1.0;

/// 图片元素在 PNG 中的占位色
///
/// 纹理像素存在 GPU 上，不写入场景 JSON，因此导出时用纯色矩形代替。
pub const IMAGE_PNG_PLACEHOLDER: Color = Color::new(0.62, 0.70, 0.86, 1.0);

/// 场景文件
///
/// # 字段
/// - `version`: 格式版本
/// - `next_id`: 下一个元素 ID
/// - `elements`: 按 id 升序排列的元素
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneFile {
    /// 格式版本
    pub version: u32,
    /// 下一个可分配的元素 ID
    pub next_id: ElementId,
    /// 场景元素
    pub elements: Vec<AnyElement>,
}

/// 导入或导出失败的原因
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistError {
    /// JSON 无法解析，或元素字段类型不对
    Json(String),
    /// `version` 不是本引擎支持的版本
    UnsupportedVersion {
        /// 文件里的版本
        found: u32,
        /// 本引擎接受的版本
        supported: u32,
    },
    /// 文件里出现了尚未实现的元素种类
    UnknownElementType(String),
    /// 两个元素使用了同一个 ID
    DuplicateId(ElementId),
    /// 导出区域面积为 0 或不是有限数
    EmptyRegion,
    /// 要求导出选区，但没有选中元素
    NothingSelected,
    /// `pixels_per_unit` 不是正的有限数
    InvalidScale,
    /// PNG 编码失败
    Encode(String),
}

impl std::fmt::Display for PersistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PersistError::Json(message) => write!(f, "invalid scene JSON: {message}"),
            PersistError::UnsupportedVersion { found, supported } => {
                write!(
                    f,
                    "unsupported scene version {found} (supported: {supported})"
                )
            }
            PersistError::UnknownElementType(kind) => {
                write!(f, "unknown element type \"{kind}\"")
            }
            PersistError::DuplicateId(id) => write!(f, "duplicate element id {id}"),
            PersistError::EmptyRegion => write!(f, "export region is empty"),
            PersistError::NothingSelected => write!(f, "no elements selected"),
            PersistError::InvalidScale => {
                write!(f, "pixels per unit must be a positive finite number")
            }
            PersistError::Encode(message) => write!(f, "png encode failed: {message}"),
        }
    }
}

impl std::error::Error for PersistError {}

#[derive(Debug, Deserialize)]
struct RawSceneFile {
    version: u32,
    #[serde(default)]
    next_id: ElementId,
    #[serde(default)]
    elements: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct RawElement {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    data: Value,
}

impl Scene {
    /// 把场景写成版本化 JSON
    ///
    /// 元素按 id 升序排列，因此同一场景多次导出的文本相同。
    /// 直线的 `arrow`、路径采样点、变换里的 `rotation` / `scale` 都会写入。
    ///
    /// # 返回
    /// 紧凑 JSON 字符串
    ///
    /// # 错误
    /// 序列化失败时返回 [`PersistError::Json`]
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{ElementStyle, Line, Path, Rectangle, Scene};
    /// use infinitecanvas_engine::math::{Color, Vec2};
    ///
    /// let mut scene = Scene::new();
    /// let rect_id = scene.next_id();
    /// scene.add_rect(Rectangle::new(rect_id, Vec2::new(10.0, 20.0), Vec2::new(4.0, 6.0)));
    /// let line_id = scene.next_id();
    /// scene.add_line(Line::from_world(
    ///     line_id,
    ///     Vec2::ZERO,
    ///     Vec2::new(30.0, 0.0),
    ///     ElementStyle::stroke_only(Color::BLACK, 2.0),
    ///     true,
    /// ));
    /// let path_id = scene.next_id();
    /// scene.add_path(Path::from_world_points(
    ///     path_id,
    ///     &[Vec2::new(0.0, 10.0), Vec2::new(20.0, 10.0)],
    ///     ElementStyle::stroke_only(Color::RED, 2.0),
    /// ));
    /// let json = scene.to_json().expect("scene serializes");
    /// let restored = Scene::from_json(&json).expect("scene deserializes");
    /// assert_eq!(restored, scene);
    /// ```
    pub fn to_json(&self) -> Result<String, PersistError> {
        serde_json::to_string(&self.to_file()).map_err(|err| PersistError::Json(err.to_string()))
    }

    /// 从版本化 JSON 恢复场景
    ///
    /// 缺省的可选字段使用 serde 默认值（旋转 0、缩放 1、箭头关闭、可见等）。
    /// 对象上的未知字段会被忽略，因此以后增加可选字段时旧引擎仍能打开文件。
    /// 未知元素种类、重复 ID 或不支持的 `version` 会返回错误。
    ///
    /// # 参数
    /// - `json`: [`Scene::to_json`] 产生的文本，或省略了可选字段的同类文档
    ///
    /// # 返回
    /// 与文件内容一致的场景。`next_id` 若小于等于已有最大 ID，会抬到 `max_id + 1`。
    ///
    /// # 错误
    /// 见 [`PersistError`]
    pub fn from_json(json: &str) -> Result<Self, PersistError> {
        let raw: RawSceneFile =
            serde_json::from_str(json).map_err(|err| PersistError::Json(err.to_string()))?;
        if raw.version != SCENE_FORMAT_VERSION {
            return Err(PersistError::UnsupportedVersion {
                found: raw.version,
                supported: SCENE_FORMAT_VERSION,
            });
        }

        let mut elements = HashMap::with_capacity(raw.elements.len());
        let mut max_id = 0u64;
        for value in raw.elements {
            let element = decode_element(value)?;
            let id = element.id();
            max_id = max_id.max(id);
            if elements.insert(id, element).is_some() {
                return Err(PersistError::DuplicateId(id));
            }
        }
        let next_id = if raw.next_id > max_id {
            raw.next_id
        } else {
            max_id.saturating_add(1)
        };
        Ok(Scene::from_parts(elements, next_id))
    }

    /// 场景的内存文档，元素按 id 排序
    pub fn to_file(&self) -> SceneFile {
        let mut elements: Vec<AnyElement> = self.iter().cloned().collect();
        elements.sort_by_key(|element| element.id());
        SceneFile {
            version: SCENE_FORMAT_VERSION,
            next_id: self.peek_next_id(),
            elements,
        }
    }
}

fn decode_element(value: Value) -> Result<AnyElement, PersistError> {
    let raw: RawElement =
        serde_json::from_value(value).map_err(|err| PersistError::Json(err.to_string()))?;
    let data = if raw.data.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        raw.data
    };
    match raw.kind.as_str() {
        "rectangle" => serde_json::from_value(data)
            .map(AnyElement::Rectangle)
            .map_err(|err| PersistError::Json(err.to_string())),
        "circle" => serde_json::from_value(data)
            .map(AnyElement::Circle)
            .map_err(|err| PersistError::Json(err.to_string())),
        "image" => serde_json::from_value(data)
            .map(AnyElement::Image)
            .map_err(|err| PersistError::Json(err.to_string())),
        "line" => serde_json::from_value(data)
            .map(AnyElement::Line)
            .map_err(|err| PersistError::Json(err.to_string())),
        "path" => serde_json::from_value(data)
            .map(AnyElement::Path)
            .map_err(|err| PersistError::Json(err.to_string())),
        "text" => {
            let mut text: Text =
                serde_json::from_value(data).map_err(|err| PersistError::Json(err.to_string()))?;
            text.finish_load();
            Ok(AnyElement::Text(text))
        }
        "group" => serde_json::from_value(data)
            .map(AnyElement::Group)
            .map_err(|err| PersistError::Json(err.to_string())),
        other => Err(PersistError::UnknownElementType(other.to_string())),
    }
}

/// 把当前视口栅格化成 RGBA PNG
///
/// 一个世界单位对应 `viewport.zoom()` 个像素，并限制在 [`MAX_PNG_DIMENSION`] 以内。
/// 直线、箭头和自由路径按半线宽上色，不含点选余量。文本用 `font8x8` 点阵上色，
/// 没有点阵的字符画成方框。图片元素用纯色占位。
///
/// # 参数
/// - `scene`: 要导出的场景
/// - `viewport`: 视口；其世界包围盒就是导出范围
///
/// # 返回
/// PNG 字节
///
/// # 错误
/// 视口面积无效或编码失败
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{export_viewport_png, Scene, Viewport};
/// use infinitecanvas_engine::math::Vec2;
///
/// let scene = Scene::new();
/// let viewport = Viewport::new(Vec2::new(4.0, 4.0));
/// let png = export_viewport_png(&scene, &viewport).unwrap();
/// assert!(png.starts_with(b"\x89PNG"));
/// ```
pub fn export_viewport_png(scene: &Scene, viewport: &Viewport) -> Result<Vec<u8>, PersistError> {
    export_region_png(scene, viewport.bounds(), viewport.zoom())
}

/// 把选中元素的包围盒栅格化成 RGBA PNG
///
/// 包围盒向外扩展 1 个世界单位。没有选中元素时返回 [`PersistError::NothingSelected`]。
///
/// # 参数
/// - `scene`: 要导出的场景。选中状态来自元素的 `selected` 标记。
/// - `pixels_per_unit`: 每个世界单位的像素数，通常传视口缩放
///
/// # 返回
/// PNG 字节
pub fn export_selection_png(scene: &Scene, pixels_per_unit: f32) -> Result<Vec<u8>, PersistError> {
    if !pixels_per_unit.is_finite() || pixels_per_unit <= 0.0 {
        return Err(PersistError::InvalidScale);
    }
    let bounds = selected_bounds(scene).ok_or(PersistError::NothingSelected)?;
    export_region_png(scene, bounds.inflate(SELECTION_PADDING), pixels_per_unit)
}

fn selected_bounds(scene: &Scene) -> Option<Rect> {
    let mut combined: Option<Rect> = None;
    for element in scene.iter() {
        if !element.as_element().props().selected {
            continue;
        }
        let bounds = element.as_element().world_bounds();
        if !bounds.width.is_finite() || !bounds.height.is_finite() {
            continue;
        }
        combined = Some(match combined {
            Some(existing) => existing.union(&bounds),
            None => bounds,
        });
    }
    combined
}

fn export_region_png(
    scene: &Scene,
    region: Rect,
    pixels_per_unit: f32,
) -> Result<Vec<u8>, PersistError> {
    let (width, height) = raster_size(region, pixels_per_unit)?;
    let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
    let mut ordered: Vec<&AnyElement> = scene.iter().collect();
    ordered.sort_by_key(|element| (element.as_element().props().z_index, element.id()));

    for element in ordered {
        // 组不绘制。隐藏的组会把它的子孙一并跳过。
        if matches!(element, AnyElement::Group(_)) || !scene.is_effectively_visible(element.id()) {
            continue;
        }
        let Some((x0, y0, x1, y1)) = pixel_span(paint_bounds(element), region, width, height)
        else {
            continue;
        };
        for y in y0..y1 {
            for x in x0..x1 {
                let world = pixel_center_world(region, width, height, x, y);
                if let Some(color) = sample_element(element, world) {
                    let index = ((y as usize) * (width as usize) + (x as usize)) * 4;
                    blend(&mut pixels[index..index + 4], color);
                }
            }
        }
    }
    encode_rgba(width, height, &pixels)
}

fn raster_size(region: Rect, pixels_per_unit: f32) -> Result<(u32, u32), PersistError> {
    if !pixels_per_unit.is_finite() || pixels_per_unit <= 0.0 {
        return Err(PersistError::InvalidScale);
    }
    if !region.width.is_finite()
        || !region.height.is_finite()
        || region.width <= 0.0
        || region.height <= 0.0
    {
        return Err(PersistError::EmptyRegion);
    }
    let max = MAX_PNG_DIMENSION as f32;
    let scale = (max / (region.width * pixels_per_unit))
        .min(max / (region.height * pixels_per_unit))
        .min(1.0);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(PersistError::EmptyRegion);
    }
    let width = (region.width * pixels_per_unit * scale)
        .round()
        .clamp(1.0, max) as u32;
    let height = (region.height * pixels_per_unit * scale)
        .round()
        .clamp(1.0, max) as u32;
    Ok((width, height))
}

fn paint_bounds(element: &AnyElement) -> Rect {
    let base = element.as_element().world_bounds();
    let extra = match element {
        AnyElement::Rectangle(rect) => {
            rect.style.stroke_width.abs() * scale_extent(&rect.props.transform)
        }
        AnyElement::Circle(circle) => {
            circle.style.stroke_width.abs() * scale_extent(&circle.props.transform)
        }
        AnyElement::Line(_)
        | AnyElement::Path(_)
        | AnyElement::Image(_)
        | AnyElement::Text(_)
        | AnyElement::Group(_) => 0.0,
    };
    base.inflate(extra.max(0.0))
}

fn scale_extent(transform: &Transform) -> f32 {
    transform.scale.x.abs().max(transform.scale.y.abs())
}

fn pixel_span(bounds: Rect, region: Rect, width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
    let intersection = bounds.intersection(&region)?;
    if intersection.width <= 0.0 || intersection.height <= 0.0 {
        return None;
    }
    let x0 = ((intersection.x - region.x) / region.width * width as f32)
        .floor()
        .clamp(0.0, width as f32) as u32;
    let y0 = ((intersection.y - region.y) / region.height * height as f32)
        .floor()
        .clamp(0.0, height as f32) as u32;
    let x1 = ((intersection.right() - region.x) / region.width * width as f32)
        .ceil()
        .clamp(0.0, width as f32) as u32;
    let y1 = ((intersection.bottom() - region.y) / region.height * height as f32)
        .ceil()
        .clamp(0.0, height as f32) as u32;
    if x0 >= x1 || y0 >= y1 {
        None
    } else {
        Some((x0.min(width), y0.min(height), x1.min(width), y1.min(height)))
    }
}

fn pixel_center_world(region: Rect, width: u32, height: u32, x: u32, y: u32) -> Vec2 {
    Vec2::new(
        region.x + (x as f32 + 0.5) * region.width / width as f32,
        region.y + (y as f32 + 0.5) * region.height / height as f32,
    )
}

fn sample_element(element: &AnyElement, world: Vec2) -> Option<Color> {
    match element {
        AnyElement::Rectangle(rect) => sample_rectangle(rect, world),
        AnyElement::Circle(circle) => sample_circle(circle, world),
        AnyElement::Image(image) => sample_image(image, world),
        AnyElement::Line(line) => line.color_at(world),
        AnyElement::Path(path) => path.color_at(world),
        AnyElement::Text(text) => text.color_at(world),
        AnyElement::Group(_) => None,
    }
}

fn sample_rectangle(rect: &Rectangle, world: Vec2) -> Option<Color> {
    if !rect.props.visible {
        return None;
    }
    let local = try_local(&rect.props.transform, world)?;
    let half = rect.size * 0.5;
    let stroke = rect.style.stroke_width.max(0.0) * 0.5;
    let ax = local.x.abs();
    let ay = local.y.abs();
    let in_fill = ax <= half.x && ay <= half.y;
    let in_stroke = stroke > 0.0
        && ax <= half.x + stroke
        && ay <= half.y + stroke
        && (ax >= (half.x - stroke).max(0.0) || ay >= (half.y - stroke).max(0.0));
    if in_stroke {
        if let Some(color) = rect.style.stroke {
            return with_opacity(color, rect.style.opacity);
        }
    }
    if in_fill {
        if let Some(color) = rect.style.fill {
            return with_opacity(color, rect.style.opacity);
        }
    }
    None
}

fn sample_circle(circle: &Circle, world: Vec2) -> Option<Color> {
    if !circle.props.visible {
        return None;
    }
    let local = try_local(&circle.props.transform, world)?;
    let dist = local.length();
    let stroke = circle.style.stroke_width.max(0.0) * 0.5;
    let in_fill = dist <= circle.radius;
    let in_stroke =
        stroke > 0.0 && dist <= circle.radius + stroke && dist >= (circle.radius - stroke).max(0.0);
    if in_stroke {
        if let Some(color) = circle.style.stroke {
            return with_opacity(color, circle.style.opacity);
        }
    }
    if in_fill {
        if let Some(color) = circle.style.fill {
            return with_opacity(color, circle.style.opacity);
        }
    }
    None
}

fn sample_image(image: &Image, world: Vec2) -> Option<Color> {
    if !image.props.visible || !image.contains_point(world) {
        return None;
    }
    let mut color = IMAGE_PNG_PLACEHOLDER;
    color.a *= image.alpha.clamp(0.0, 1.0);
    if color.a <= 0.0 {
        None
    } else {
        Some(color)
    }
}

fn with_opacity(color: Color, opacity: f32) -> Option<Color> {
    let painted = Color::new(
        color.r,
        color.g,
        color.b,
        (color.a * opacity.clamp(0.0, 1.0)).clamp(0.0, 1.0),
    );
    if painted.a <= 0.0 {
        None
    } else {
        Some(painted)
    }
}

fn try_local(transform: &Transform, world: Vec2) -> Option<Vec2> {
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

fn blend(dst: &mut [u8], src: Color) {
    let sa = src.a.clamp(0.0, 1.0);
    if sa <= 0.0 {
        return;
    }
    let sr = src.r.clamp(0.0, 1.0);
    let sg = src.g.clamp(0.0, 1.0);
    let sb = src.b.clamp(0.0, 1.0);
    let da = dst[3] as f32 / 255.0;
    let dr = dst[0] as f32 / 255.0;
    let dg = dst[1] as f32 / 255.0;
    let db = dst[2] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 1e-6 {
        dst.copy_from_slice(&[0, 0, 0, 0]);
        return;
    }
    let inv = 1.0 - sa;
    dst[0] = channel((sr * sa + dr * da * inv) / out_a);
    dst[1] = channel((sg * sa + dg * da * inv) / out_a);
    dst[2] = channel((sb * sa + db * da * inv) / out_a);
    dst[3] = channel(out_a);
}

fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn encode_rgba(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, PersistError> {
    let mut buffer = Vec::new();
    let mut encoder = png::Encoder::new(&mut buffer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|err| PersistError::Encode(err.to_string()))?;
    writer
        .write_image_data(pixels)
        .map_err(|err| PersistError::Encode(err.to_string()))?;
    writer
        .finish()
        .map_err(|err| PersistError::Encode(err.to_string()))?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::{
        channel, export_selection_png, export_viewport_png, PersistError, IMAGE_PNG_PLACEHOLDER,
        MAX_PNG_DIMENSION, SCENE_FORMAT_VERSION,
    };
    use crate::element::{
        Circle, Element, ElementStyle, Image, Line, Path, Rectangle, Text, TextAlign,
    };
    use crate::{AnyElement, Scene, Viewport};
    use infinitecanvas_math::{Color, Rect, Transform, Vec2};
    use serde_json::{json, Value};

    fn populated_scene() -> Scene {
        let mut scene = Scene::new();

        let rect_id = scene.next_id();
        let mut rect = Rectangle::new(rect_id, Vec2::new(12.0, -4.0), Vec2::new(8.0, 3.5));
        rect.set_rotation(0.4);
        rect.props.transform.scale = Vec2::new(1.5, 0.75);
        rect.props.z_index = 2;
        rect.props.selected = true;
        rect.style = ElementStyle {
            fill: Some(Color::new(0.2, 0.4, 0.6, 0.8)),
            stroke: Some(Color::RED),
            stroke_width: 2.0,
            opacity: 0.9,
        };
        scene.add_rect(rect);

        let circle_id = scene.next_id();
        let mut circle = Circle::new(circle_id, Vec2::new(-3.0, 7.0), 5.5)
            .with_style(ElementStyle::fill_only(Color::GREEN));
        circle.set_rotation(1.1);
        circle.props.transform.scale = Vec2::new(0.5, 2.0);
        circle.props.visible = false;
        scene.add_circle(circle);

        let image_id = scene.next_id();
        let mut image = Image::with_uv_crop(
            image_id,
            Vec2::new(1.0, 2.0),
            Vec2::new(16.0, 8.0),
            "badge".to_string(),
            Rect::new(0.1, 0.2, 0.3, 0.4),
        );
        image.set_rotation(-0.8);
        image.props.transform.scale = Vec2::splat(1.25);
        image.set_alpha(0.5);
        image.props.z_index = 4;
        scene.add_image(image);

        let line_id = scene.next_id();
        let mut line = Line::from_world(
            line_id,
            Vec2::ZERO,
            Vec2::new(40.0, 10.0),
            ElementStyle::stroke_only(Color::rgb(0.1, 0.2, 0.3), 3.0),
            true,
        );
        line.set_rotation(-0.25);
        line.props.transform.scale = Vec2::new(2.0, 1.0);
        scene.add_line(line);

        let path_id = scene.next_id();
        let mut path = Path::from_world_points(
            path_id,
            &[
                Vec2::new(5.0, 5.0),
                Vec2::new(15.0, 8.0),
                Vec2::new(18.0, 20.0),
                Vec2::new(4.0, 16.0),
            ],
            ElementStyle::stroke_only(Color::BLUE, 2.5),
        );
        path.set_rotation(0.2);
        path.props.transform.scale = Vec2::new(1.0, 0.5);
        scene.add_path(path);
        scene
    }

    fn json_value(scene: &Scene) -> Value {
        serde_json::from_str(&scene.to_json().unwrap()).unwrap()
    }

    #[test]
    fn json_round_trip_keeps_line_arrow_path_rotation_and_scale() {
        let scene = populated_scene();
        let json = scene.to_json().unwrap();
        assert!(json.contains("\"version\":1"));
        assert!(json.contains("\"type\":\"line\""));
        assert!(json.contains("\"type\":\"path\""));
        assert!(json.contains("\"arrow\":true"));

        let restored = Scene::from_json(&json).unwrap();
        assert_eq!(restored, scene);
        assert_eq!(restored.to_json().unwrap(), json);
        assert_eq!(restored.peek_next_id(), scene.peek_next_id());

        match restored.get(1).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.rotation(), 0.4);
                assert_eq!(rect.props.transform.scale, Vec2::new(1.5, 0.75));
            }
            other => panic!("expected rectangle, got {other:?}"),
        }
        match restored.get(2).unwrap() {
            AnyElement::Circle(circle) => {
                assert_eq!(circle.rotation(), 1.1);
                assert_eq!(circle.props.transform.scale, Vec2::new(0.5, 2.0));
            }
            other => panic!("expected circle, got {other:?}"),
        }
        match restored.get(3).unwrap() {
            AnyElement::Image(image) => {
                assert_eq!(image.rotation(), -0.8);
                assert_eq!(image.props.transform.scale, Vec2::splat(1.25));
            }
            other => panic!("expected image, got {other:?}"),
        }
        match restored.get(4).unwrap() {
            AnyElement::Line(line) => {
                assert!(line.arrow());
                assert_eq!(line.rotation(), -0.25);
                assert_eq!(line.props.transform.scale, Vec2::new(2.0, 1.0));
            }
            other => panic!("expected line, got {other:?}"),
        }
        match restored.get(5).unwrap() {
            AnyElement::Path(path) => {
                assert_eq!(path.points().len(), 4);
                assert_eq!(path.rotation(), 0.2);
                assert_eq!(path.props.transform.scale, Vec2::new(1.0, 0.5));
            }
            other => panic!("expected path, got {other:?}"),
        }
    }

    #[test]
    fn empty_scene_round_trip_preserves_next_id() {
        let scene = Scene::new();
        let restored = Scene::from_json(&scene.to_json().unwrap()).unwrap();
        assert_eq!(restored, scene);
        assert!(restored.is_empty());
        assert_eq!(restored.peek_next_id(), 1);
    }

    #[test]
    fn missing_optional_fields_use_defaults() {
        let start = serde_json::to_value(Vec2::ZERO).unwrap();
        let end = serde_json::to_value(Vec2::new(10.0, 0.0)).unwrap();
        let size = serde_json::to_value(Vec2::new(4.0, 2.0)).unwrap();
        let json = json!({
            "version": 1,
            "elements": [
                {
                    "type": "line",
                    "data": {
                        "props": { "id": 1 },
                        "start": start,
                        "end": end
                    }
                },
                {
                    "type": "rectangle",
                    "data": {
                        "props": {
                            "id": 2,
                            "transform": { "rotation": 0.5 }
                        },
                        "size": size
                    }
                },
                {
                    "type": "path",
                    "data": { "props": { "id": 3 } }
                }
            ]
        });

        let scene = Scene::from_json(&json.to_string()).unwrap();
        assert_eq!(scene.peek_next_id(), 4);
        match scene.get(1).unwrap() {
            AnyElement::Line(line) => {
                assert!(!line.arrow());
                assert!(line.props.visible);
                assert!(!line.props.selected);
                assert_eq!(line.props.z_index, 0);
                assert_eq!(line.props.transform, Transform::identity());
                assert_eq!(line.style, ElementStyle::new());
            }
            other => panic!("expected line, got {other:?}"),
        }
        match scene.get(2).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.props.transform.rotation, 0.5);
                assert_eq!(rect.props.transform.translation, Vec2::ZERO);
                assert_eq!(rect.props.transform.scale, Vec2::ONE);
                assert_eq!(rect.style.opacity, 1.0);
                assert_eq!(rect.style.stroke_width, 1.0);
                assert_eq!(rect.style.fill, Some(Color::BLACK));
            }
            other => panic!("expected rectangle, got {other:?}"),
        }
        match scene.get(3).unwrap() {
            AnyElement::Path(path) => assert!(path.points().is_empty()),
            other => panic!("expected path, got {other:?}"),
        }
    }

    #[test]
    fn missing_color_alpha_defaults_to_opaque() {
        let mut scene = Scene::new();
        let id = scene.next_id();
        scene.add_line(Line::from_world(
            id,
            Vec2::ZERO,
            Vec2::new(10.0, 0.0),
            ElementStyle::stroke_only(Color::rgb(1.0, 0.0, 0.0), 4.0),
            false,
        ));
        let mut value = json_value(&scene);
        value["elements"][0]["data"]["style"]["stroke"]
            .as_object_mut()
            .unwrap()
            .remove("a");
        let restored = Scene::from_json(&value.to_string()).unwrap();
        match restored.get(id).unwrap() {
            AnyElement::Line(line) => {
                let color = line.style.stroke.unwrap();
                assert_eq!(color.r, 1.0);
                assert_eq!(color.a, 1.0);
            }
            other => panic!("expected line, got {other:?}"),
        }
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let mut value = json_value(&populated_scene());
        value["future_document"] = json!({"note": "later"});
        value["elements"][3]["future_sibling"] = json!(1);
        value["elements"][3]["data"]["rotation_handle"] = json!(15);
        value["elements"][3]["data"]["props"]["transform"]
            .as_object_mut()
            .unwrap()
            .remove("rotation");
        value["elements"][3]["data"]["props"]["transform"]
            .as_object_mut()
            .unwrap()
            .remove("scale");
        value["elements"][3]["data"]
            .as_object_mut()
            .unwrap()
            .remove("arrow");

        let restored = Scene::from_json(&value.to_string()).unwrap();
        match restored.get(4).unwrap() {
            AnyElement::Line(line) => {
                assert!(!line.arrow());
                assert_eq!(line.props.transform.rotation, 0.0);
                assert_eq!(line.props.transform.scale, Vec2::ONE);
            }
            other => panic!("expected line, got {other:?}"),
        }
    }

    #[test]
    fn text_json_round_trip_keeps_content_style_rotation_and_scale() {
        let mut scene = Scene::new();
        let id = scene.next_id();
        let mut text = Text::new(id, Vec2::new(12.0, -3.0), "Hello 你好")
            .with_font_size(18.0)
            .with_color(Color::new(0.2, 0.4, 0.6, 0.8))
            .with_align(TextAlign::Center);
        text.set_rotation(0.7);
        text.props.transform.scale = Vec2::new(1.5, 0.8);
        text.props.z_index = 3;
        scene.add_text(text);

        let json = scene.to_json().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["version"], 1);
        assert_eq!(value["elements"][0]["type"], "text");
        assert_eq!(value["elements"][0]["data"]["content"], "Hello 你好");
        assert_eq!(value["elements"][0]["data"]["font_size"], 18.0);
        assert_eq!(value["elements"][0]["data"]["align"], "center");
        assert_eq!(
            value["elements"][0]["data"]["props"]["transform"]["rotation"],
            0.7
        );
        let scale = &value["elements"][0]["data"]["props"]["transform"]["scale"];
        let scale_x = scale["x"].as_f64().or_else(|| scale[0].as_f64());
        assert_eq!(scale_x, Some(1.5));
        assert!(!json.contains("texture_id"));
        assert!(!json.contains("raster_lod"));
        assert!(!json.contains("\"editing\""));
        assert!(!json.contains("\"revision\""));
        assert!(!json.contains("\"dirty\""));

        let restored = Scene::from_json(&json).unwrap();
        assert_eq!(restored, scene);
        assert_eq!(restored.to_json().unwrap(), json);
        match restored.get(id).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.content, "Hello 你好");
                assert_eq!(text.font_size, 18.0);
                assert_eq!(text.color, Color::new(0.2, 0.4, 0.6, 0.8));
                assert_eq!(text.align, TextAlign::Center);
                assert_eq!(text.rotation(), 0.7);
                assert_eq!(text.props.transform.scale, Vec2::new(1.5, 0.8));
                assert!(text.texture_id.is_none());
                assert_eq!(text.revision, 1);
            }
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn text_missing_optionals_and_unknown_fields_use_defaults() {
        let json = json!({
            "version": 1,
            "elements": [{
                "type": "text",
                "data": {
                    "props": { "id": 7 },
                    "future_field": { "ignored": true },
                    "texture_id": "stale"
                }
            }]
        });
        let scene = Scene::from_json(&json.to_string()).unwrap();
        assert_eq!(scene.peek_next_id(), 8);
        match scene.get(7).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.content, "");
                assert_eq!(text.font_size, 32.0);
                assert_eq!(text.align, TextAlign::Left);
                assert_eq!(text.color.a, 1.0);
                assert_eq!(text.props.transform.rotation, 0.0);
                assert_eq!(text.props.transform.scale, Vec2::ONE);
                assert!(text.props.visible);
                assert!(text.size.x > 0.0 && text.size.y > 0.0);
                assert!(text.texture_id.is_none());
            }
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn unknown_element_type_is_rejected() {
        let json = r#"{"version":1,"elements":[{"type":"widget","data":{"props":{"id":1}}}]}"#;
        match Scene::from_json(json).unwrap_err() {
            PersistError::UnknownElementType(kind) => assert_eq!(kind, "widget"),
            other => panic!("expected unknown type, got {other:?}"),
        }
    }

    #[test]
    fn unsupported_version_and_malformed_json_are_rejected() {
        match Scene::from_json(r#"{"version":9,"elements":[]}"#).unwrap_err() {
            PersistError::UnsupportedVersion { found, supported } => {
                assert_eq!(found, 9);
                assert_eq!(supported, SCENE_FORMAT_VERSION);
            }
            other => panic!("expected version error, got {other:?}"),
        }
        assert!(matches!(
            Scene::from_json("{").unwrap_err(),
            PersistError::Json(_)
        ));
        assert!(matches!(
            Scene::from_json(r#"{"elements":[]}"#).unwrap_err(),
            PersistError::Json(_)
        ));
    }

    #[test]
    fn duplicate_id_is_rejected() {
        let mut value = json_value(&populated_scene());
        let first = value["elements"][0].clone();
        value["elements"].as_array_mut().unwrap().push(first);
        assert_eq!(
            Scene::from_json(&value.to_string()).unwrap_err(),
            PersistError::DuplicateId(1)
        );
    }

    #[test]
    fn missing_required_geometry_is_rejected() {
        let json = json!({
            "version": 1,
            "elements": [{ "type": "line", "data": { "props": { "id": 1 } } }]
        });
        assert!(matches!(
            Scene::from_json(&json.to_string()).unwrap_err(),
            PersistError::Json(_)
        ));
    }

    #[test]
    fn next_id_is_preserved_when_larger_and_repaired_when_too_small() {
        let scene = populated_scene();
        let mut value = json_value(&scene);
        value["next_id"] = json!(40);
        let restored = Scene::from_json(&value.to_string()).unwrap();
        assert_eq!(restored.peek_next_id(), 40);

        value["next_id"] = json!(1);
        let repaired = Scene::from_json(&value.to_string()).unwrap();
        assert_eq!(repaired.peek_next_id(), 6);
    }

    fn sample_pixel(scene: &Scene, world: Vec2) -> [u8; 4] {
        let mut viewport = Viewport::new(Vec2::new(1.0, 1.0));
        viewport.move_to(world);
        let png = export_viewport_png(scene, &viewport).unwrap();
        let (width, _, pixels) = decode_png(&png);
        assert_eq!((width, pixels.len()), (1, 4));
        [pixels[0], pixels[1], pixels[2], pixels[3]]
    }

    fn decode_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        buf.truncate(info.buffer_size());
        (info.width, info.height, buf)
    }

    fn assert_pixel(actual: [u8; 4], expected: [u8; 4]) {
        for (got, want) in actual.iter().zip(expected) {
            assert!(
                (*got as i16 - want as i16).abs() <= 1,
                "pixel {actual:?} != {expected:?}"
            );
        }
    }

    #[test]
    fn png_exports_rectangle_line_arrow_path_and_image_placeholder() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::new(20.0, 0.0), Vec2::new(30.0, 30.0))
                .with_style(ElementStyle::fill_only(Color::BLUE)),
        );
        let mut line = Line::from_world(
            2,
            Vec2::ZERO,
            Vec2::new(100.0, 0.0),
            ElementStyle::stroke_only(Color::RED, 4.0),
            true,
        );
        line.props.z_index = 5;
        scene.add_line(line);
        scene.add_path(Path::from_world_points(
            3,
            &[
                Vec2::new(0.0, 0.0),
                Vec2::new(0.0, 40.0),
                Vec2::new(40.0, 40.0),
            ],
            ElementStyle::stroke_only(Color::BLUE, 4.0),
        ));
        scene.add_image(Image::new(
            4,
            Vec2::ZERO,
            Vec2::new(10.0, 10.0),
            "badge".to_string(),
        ));

        assert_pixel(sample_pixel(&scene, Vec2::new(20.0, 0.0)), [255, 0, 0, 255]);
        assert_pixel(sample_pixel(&scene, Vec2::new(20.0, 8.0)), [0, 0, 255, 255]);
        assert_pixel(sample_pixel(&scene, Vec2::new(90.0, 3.0)), [255, 0, 0, 255]);
        assert_pixel(sample_pixel(&scene, Vec2::new(110.0, 0.0)), [0, 0, 0, 0]);
        assert_pixel(sample_pixel(&scene, Vec2::new(0.0, 20.0)), [0, 0, 255, 255]);
        assert_pixel(sample_pixel(&scene, Vec2::new(20.0, 20.0)), [0, 0, 0, 0]);
        assert_pixel(
            sample_pixel(&scene, Vec2::new(20.0, 40.0)),
            [0, 0, 255, 255],
        );

        let placeholder = [
            channel(IMAGE_PNG_PLACEHOLDER.r),
            channel(IMAGE_PNG_PLACEHOLDER.g),
            channel(IMAGE_PNG_PLACEHOLDER.b),
            255,
        ];
        assert_pixel(sample_pixel(&scene, Vec2::new(5.0, 5.0)), placeholder);
    }

    #[test]
    fn png_draws_text_glyphs_with_rotation_and_scale() {
        let plain = Text::new(1, Vec2::new(30.0, 40.0), "H")
            .with_font_size(64.0)
            .with_color(Color::RED);
        let ink_local = plain.glyph_sample_local(0, 0, 0, 0).unwrap();
        let gap_local = plain.glyph_sample_local(0, 0, 2, 0).unwrap();
        let plain_ink = plain.props.transform.transform_point(ink_local);
        let plain_gap = plain.props.transform.transform_point(gap_local);
        let mut plain_scene = Scene::new();
        plain_scene.add_text(plain);
        assert_pixel(sample_pixel(&plain_scene, plain_ink), [255, 0, 0, 255]);
        assert_pixel(sample_pixel(&plain_scene, plain_gap), [0, 0, 0, 0]);

        let mut turned = Text::new(1, Vec2::new(30.0, 40.0), "H")
            .with_font_size(64.0)
            .with_color(Color::RED);
        turned.set_rotation(std::f32::consts::FRAC_PI_2);
        turned.props.transform.scale = Vec2::new(2.0, 1.5);
        let turned_ink = turned.props.transform.transform_point(ink_local);
        let turned_gap = turned.props.transform.transform_point(gap_local);
        assert_ne!(turned_ink, plain_ink);
        let mut turned_scene = Scene::new();
        turned_scene.add_text(turned);
        assert_pixel(sample_pixel(&turned_scene, turned_ink), [255, 0, 0, 255]);
        assert_pixel(sample_pixel(&turned_scene, turned_gap), [0, 0, 0, 0]);
        assert_pixel(sample_pixel(&turned_scene, plain_ink), [0, 0, 0, 0]);
    }

    #[test]
    fn png_selection_omits_unselected_shapes() {
        let mut scene = Scene::new();
        let mut line = Line::from_world(
            1,
            Vec2::ZERO,
            Vec2::new(40.0, 0.0),
            ElementStyle::stroke_only(Color::RED, 6.0),
            false,
        );
        line.props.selected = true;
        scene.add_line(line);
        scene.add_rect(
            Rectangle::new(2, Vec2::new(400.0, 400.0), Vec2::new(20.0, 20.0))
                .with_style(ElementStyle::fill_only(Color::BLUE)),
        );

        let png = export_selection_png(&scene, 1.0).unwrap();
        let (_, _, pixels) = decode_png(&png);
        let mut saw_red = false;
        for chunk in pixels.chunks(4) {
            let blue = chunk[2] > 200 && chunk[0] < 10 && chunk[3] > 200;
            assert!(!blue, "unselected blue rect leaked into selection png");
            if chunk[0] > 200 && chunk[2] < 10 && chunk[3] > 200 {
                saw_red = true;
            }
        }
        assert!(saw_red);
    }

    #[test]
    fn png_selection_requires_a_selection_and_scale_must_be_positive() {
        let scene = Scene::new();
        assert_eq!(
            export_selection_png(&scene, 1.0).unwrap_err(),
            PersistError::NothingSelected
        );
        assert_eq!(
            export_selection_png(&scene, 0.0).unwrap_err(),
            PersistError::InvalidScale
        );
        let mut viewport = Viewport::new(Vec2::ZERO);
        assert_eq!(
            export_viewport_png(&scene, &viewport).unwrap_err(),
            PersistError::EmptyRegion
        );
        viewport.resize(Vec2::new(4.0, 4.0));
        assert!(export_viewport_png(&scene, &viewport).is_ok());
    }

    #[test]
    fn json_round_trip_keeps_group_children_parent_and_lock() {
        let mut scene = Scene::new();
        let left = scene.next_id();
        scene.add_rect(Rectangle::new(left, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        let right = scene.next_id();
        scene.add_rect(Rectangle::new(
            right,
            Vec2::new(30.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let group_id = scene.group(&[left, right]).unwrap();
        scene
            .translate_element(group_id, Vec2::new(2.0, -1.0))
            .unwrap();
        scene.set_locked(group_id, true);

        let json = scene.to_json().unwrap();
        assert!(json.contains("\"type\":\"group\""));
        assert!(json.contains("\"children\""));
        let restored = Scene::from_json(&json).unwrap();
        assert_eq!(restored, scene);
        assert_eq!(restored.children_of(group_id), vec![left, right]);
        assert_eq!(restored.parent_of(right), Some(group_id));
        assert!(restored.get(group_id).unwrap().as_element().props().locked);
        assert_eq!(
            restored
                .get(left)
                .unwrap()
                .as_element()
                .props()
                .transform
                .translation,
            Vec2::new(2.0, -1.0)
        );
    }

    #[test]
    fn png_dimension_is_capped() {
        let mut viewport = Viewport::new(Vec2::new(4000.0, 8.0));
        viewport.move_to(Vec2::ZERO);
        let png = export_viewport_png(&Scene::new(), &viewport).unwrap();
        let (width, height, _) = decode_png(&png);
        assert_eq!(width, MAX_PNG_DIMENSION);
        assert_eq!(height, 4);
    }
}
