//! 场景持久化
//!
//! 把场景写成带 `version` 的 JSON，并可以把视口或选中区域栅格化成 PNG。
//!
//! ```text
//! Scene ── to_json ──▶ { version, next_id, assets[], elements[], guides[] }
//!                          │  type + data          │ data URL，同一张图只写一次
//!                          ▼                       ▼
//!                     rectangle / circle / polygon / image / line / path / text / group / frame
//! Scene ◀─ from_json ── 缺省字段用 serde default，未知字段忽略
//! ```
//!
//! 只接受 [`SCENE_FORMAT_VERSION`]。新增可选字段时保持同一版本，并为字段加上
//! `#[serde(default)]`。新增元素种类时扩展解码分支；未知种类会报错，避免静默丢数据。

use crate::asset::{AssetError, AssetRecord, AssetTable};
use crate::blend::{composite_straight, BlendMode};
use crate::element::blur::{
    blur_active, blurred_mask, blurred_paint, kernel_color, sanitize_blur, BLUR_EXTENT,
};
use crate::element::shadow::{composite_over, feather_coverage};
use crate::element::{
    Circle, DropShadow, Element, ElementStyle, Image, InnerShadow, Polygon, PolygonKind, Rectangle,
    Text,
};
use crate::guides::{GuideStore, ManualGuide};
use crate::scene::{AnyElement, Scene};
use crate::snap::GuideAxis;
use crate::{ElementId, Viewport};
use infinitecanvas_math::{Color, Rect, Transform, Vec2};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// 当前场景 JSON 版本。不兼容的结构必须递增。
pub const SCENE_FORMAT_VERSION: u32 = 1;

/// PNG 单边像素上限，避免一次导出分配过大的缓冲。
const MAX_PNG_DIMENSION: u32 = 2048;

/// 选区导出在世界包围盒外再留出的边距，避免描边被裁掉。
const SELECTION_PADDING: f32 = 1.0;

/// 没有像素资源的图片在 PNG 中的占位色
///
/// 只带 GPU `texture_id`、资源表里没有字节的图片仍用纯色矩形。
/// 资源表里有像素时，导出直接采样这些像素。
pub const IMAGE_PNG_PLACEHOLDER: Color = Color::new(0.62, 0.70, 0.86, 1.0);

/// 场景文件
///
/// # 字段
/// - `version`: 格式版本
/// - `next_id`: 下一个元素 ID
/// - `assets`: 图片资源，按 id 排序。同一张图只出现一次
/// - `elements`: 按 id 升序排列的元素
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneFile {
    /// 格式版本
    pub version: u32,
    /// 下一个可分配的元素 ID
    pub next_id: ElementId,
    /// 图片资源（data URL）。没有图片时是空数组。
    pub assets: Vec<AssetRecord>,
    /// 场景元素
    pub elements: Vec<AnyElement>,
    /// 手动参考线。没有参考线时不写入文件。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub guides: Vec<ManualGuide>,
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
    /// `pixels_per_unit` 或导出 `scale` 不是正的有限数
    InvalidScale,
    /// PNG 编码失败
    Encode(String),
    /// 导出目标不是画板
    NotAFrame,
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
                write!(
                    f,
                    "pixels per unit and export scale must be positive finite numbers"
                )
            }
            PersistError::Encode(message) => write!(f, "png encode failed: {message}"),
            PersistError::NotAFrame => write!(f, "element is not a frame"),
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
    assets: Vec<AssetRecord>,
    #[serde(default)]
    elements: Vec<Value>,
    #[serde(default)]
    guides: Vec<ManualGuide>,
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
            let mut element = decode_element(value)?;
            crate::element::blur::sanitize_element(&mut element);
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
        let mut assets = AssetTable::from_records(raw.assets).map_err(asset_error)?;
        let asset_ids: Vec<String> = elements
            .values()
            .filter_map(|element| match element {
                AnyElement::Image(image) if !image.asset_id.is_empty() => {
                    Some(image.asset_id.clone())
                }
                _ => None,
            })
            .collect();
        assets.assign_references(asset_ids);
        let guides = GuideStore::from_records(raw.guides).map_err(PersistError::Json)?;
        let mut scene = Scene::from_parts(elements, next_id, assets);
        scene.set_guide_store(guides);
        Ok(scene)
    }

    /// 场景的内存文档，元素按 id 排序
    pub fn to_file(&self) -> SceneFile {
        let mut elements: Vec<AnyElement> = self.iter().cloned().collect();
        elements.sort_by_key(|element| element.id());
        SceneFile {
            version: SCENE_FORMAT_VERSION,
            next_id: self.peek_next_id(),
            assets: self.image_asset_records(),
            elements,
            guides: self.guides().records(),
        }
    }
}

fn asset_error(err: AssetError) -> PersistError {
    PersistError::Json(err.to_string())
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
        "polygon" | "star" => decode_polygon(&raw.kind, data),
        "image" => {
            let mut image: Image =
                serde_json::from_value(data).map_err(|err| PersistError::Json(err.to_string()))?;
            image.finish_load();
            Ok(AnyElement::Image(image))
        }
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
        "frame" => serde_json::from_value(data)
            .map(AnyElement::Frame)
            .map_err(|err| PersistError::Json(err.to_string())),
        other => Err(PersistError::UnknownElementType(other.to_string())),
    }
}

/// `polygon` 按数据里的 `kind` 读。`star` 强制为星形；缺省边数时用 5 而不是 6。
fn decode_polygon(kind: &str, data: Value) -> Result<AnyElement, PersistError> {
    let sides_missing = data.get("sides").is_none() && data.get("points").is_none();
    let mut polygon: Polygon =
        serde_json::from_value(data).map_err(|err| PersistError::Json(err.to_string()))?;
    polygon.normalize();
    if kind == "star" {
        polygon.set_kind(PolygonKind::Star);
        if sides_missing {
            polygon.set_sides(crate::element::polygon::DEFAULT_STAR_POINTS);
        }
    }
    Ok(AnyElement::Polygon(polygon))
}

/// 把当前视口栅格化成 RGBA PNG
///
/// 一个世界单位对应 `viewport.zoom()` 个像素，并限制在 [`MAX_PNG_DIMENSION`] 以内。
/// 直线、箭头和自由路径按半线宽上色，不含点选余量。文本用 `font8x8` 点阵上色，
/// 没有点阵的字符画成方框。有资源像素的图片按像素采样；只有 GPU 纹理 id 的图片用纯色占位。
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
    export_viewport_png_with_guides(scene, viewport, false)
}

/// 同 [`export_viewport_png`]。`include_guides` 为 true 时用一条青色像素画出参考线。
///
/// 默认导出不画参考线。标尺是屏幕装饰，两种导出都不画标尺。
pub fn export_viewport_png_with_guides(
    scene: &Scene,
    viewport: &Viewport,
    include_guides: bool,
) -> Result<Vec<u8>, PersistError> {
    export_viewport_png_scaled(scene, viewport, include_guides, 1.0)
}

/// 同 [`export_viewport_png_with_guides`]，并乘上导出倍率 `scale`。
///
/// `scale` 默认语义是 1.0（与旧行为一致）。2.0 / 3.0 时宽高各约 ×2 / ×3，
/// 面积约 ×4 / ×9。像素中心采样；倍率越高，每个世界单位覆盖的像素越多，
/// 边缘更细（等价于按更高分辨率栅格化，不是先出 1x 再放大）。
///
/// 空视口（宽或高 ≤ 0）仍返回 [`PersistError::EmptyRegion`]。
pub fn export_viewport_png_scaled(
    scene: &Scene,
    viewport: &Viewport,
    include_guides: bool,
    scale: f32,
) -> Result<Vec<u8>, PersistError> {
    let scale = resolve_export_scale(scale)?;
    export_region_png(
        scene,
        viewport.bounds(),
        viewport.zoom() * scale,
        include_guides,
    )
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
    export_selection_png_scaled(scene, pixels_per_unit, 1.0)
}

/// 同 [`export_selection_png`]，并乘上导出倍率 `scale`（1.0 = 旧行为）。
///
/// 有效分辨率是 `pixels_per_unit * scale`。选中根全是画板时按画板裁剪导出，同样尊重倍率。
/// 空选区返回 [`PersistError::NothingSelected`]。
pub fn export_selection_png_scaled(
    scene: &Scene,
    pixels_per_unit: f32,
    scale: f32,
) -> Result<Vec<u8>, PersistError> {
    if !pixels_per_unit.is_finite() || pixels_per_unit <= 0.0 {
        return Err(PersistError::InvalidScale);
    }
    let scale = resolve_export_scale(scale)?;
    let ppu = pixels_per_unit * scale;
    if let Some(bounds) = selected_frame_crop(scene) {
        let allow = selected_frame_ids(scene);
        return export_region_filtered(scene, bounds, ppu, false, Some(&allow));
    }
    let bounds = selected_bounds(scene).ok_or(PersistError::NothingSelected)?;
    export_region_png(scene, bounds.inflate(SELECTION_PADDING), ppu, false)
}

/// 把一张画板栅格化成 PNG。区域就是画板的世界包围盒，不含额外留白。
///
/// 只绘制这张画板和它的子孙。空画板仍导出矩形尺寸。
/// 画布上子元素可以画出边界；导出时超出部分被裁掉。
///
/// # 错误
/// 元素不存在、不是画板、尺寸无效或编码失败。
pub fn export_frame_png(
    scene: &Scene,
    frame_id: ElementId,
    pixels_per_unit: f32,
) -> Result<Vec<u8>, PersistError> {
    export_frame_png_scaled(scene, frame_id, pixels_per_unit, 1.0)
}

/// 同 [`export_frame_png`]，并乘上导出倍率 `scale`（1.0 = 旧行为）。
pub fn export_frame_png_scaled(
    scene: &Scene,
    frame_id: ElementId,
    pixels_per_unit: f32,
    scale: f32,
) -> Result<Vec<u8>, PersistError> {
    if !pixels_per_unit.is_finite() || pixels_per_unit <= 0.0 {
        return Err(PersistError::InvalidScale);
    }
    let scale = resolve_export_scale(scale)?;
    let bounds = frame_export_bounds(scene, frame_id)?;
    let allow: HashSet<ElementId> = scene.subtree_ids(frame_id).into_iter().collect();
    export_region_filtered(scene, bounds, pixels_per_unit * scale, false, Some(&allow))
}

/// 校验导出倍率：必须是正的有限数。常用 1 / 2 / 3。
fn resolve_export_scale(scale: f32) -> Result<f32, PersistError> {
    if scale.is_finite() && scale > 0.0 {
        Ok(scale)
    } else {
        Err(PersistError::InvalidScale)
    }
}

/// 画板的轴对齐世界包围盒。零尺寸或非有限值返回 [`PersistError::EmptyRegion`]。
pub(crate) fn frame_export_bounds(
    scene: &Scene,
    frame_id: ElementId,
) -> Result<Rect, PersistError> {
    let Some(element) = scene.get(frame_id) else {
        return Err(PersistError::NotAFrame);
    };
    let AnyElement::Frame(frame) = element else {
        return Err(PersistError::NotAFrame);
    };
    let bounds = frame.world_bounds();
    if !bounds.x.is_finite()
        || !bounds.y.is_finite()
        || !bounds.width.is_finite()
        || !bounds.height.is_finite()
        || bounds.width <= 0.0
        || bounds.height <= 0.0
    {
        return Err(PersistError::EmptyRegion);
    }
    Ok(bounds)
}

/// 选区根全是画板时，返回这些画板世界包围盒的并集。混有其它根元素时返回 `None`。
pub(crate) fn selected_frame_crop(scene: &Scene) -> Option<Rect> {
    let roots = selected_frame_roots(scene)?;
    let mut combined: Option<Rect> = None;
    for id in roots {
        let bounds = frame_export_bounds(scene, id).ok()?;
        combined = Some(match combined {
            Some(existing) => existing.union(&bounds),
            None => bounds,
        });
    }
    combined
}

fn selected_frame_roots(scene: &Scene) -> Option<Vec<ElementId>> {
    let mut frames = Vec::new();
    for element in scene.iter() {
        if !element.as_element().props().selected || has_selected_ancestor(scene, element.id()) {
            continue;
        }
        if matches!(element, AnyElement::Frame(_)) {
            frames.push(element.id());
        } else {
            return None;
        }
    }
    if frames.is_empty() {
        None
    } else {
        Some(frames)
    }
}

fn has_selected_ancestor(scene: &Scene, id: ElementId) -> bool {
    let mut current = scene.parent_of(id);
    while let Some(parent_id) = current {
        if scene
            .get(parent_id)
            .is_some_and(|element| element.as_element().props().selected)
        {
            return true;
        }
        current = scene.parent_of(parent_id);
    }
    false
}

pub(crate) fn selected_frame_ids(scene: &Scene) -> HashSet<ElementId> {
    let mut allow = HashSet::new();
    if let Some(roots) = selected_frame_roots(scene) {
        for id in roots {
            allow.extend(scene.subtree_ids(id));
        }
    }
    allow
}

/// 按绘制顺序返回元素。
///
/// 先按 `z_index` 升序，同一层级再按 id 升序。后出现的元素画在上面。
/// PNG 栅格化和 SVG 导出共用这一次遍历，避免两套排序。
pub(crate) fn elements_in_paint_order(scene: &Scene) -> Vec<&AnyElement> {
    let mut ordered: Vec<&AnyElement> = scene.iter().collect();
    ordered.sort_by_key(|element| (element.as_element().props().z_index, element.id()));
    ordered
}

fn selected_bounds(scene: &Scene) -> Option<Rect> {
    let mut combined: Option<Rect> = None;
    for element in scene.iter() {
        if !element.as_element().props().selected {
            continue;
        }
        let mut bounds = element.as_element().world_bounds();
        // 画板导出区域保持世界包围盒，不把画板自己的光晕算进去。
        if !matches!(element, AnyElement::Frame(_)) {
            bounds = bounds.inflate(blur_reach(element));
        }
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
    include_guides: bool,
) -> Result<Vec<u8>, PersistError> {
    export_region_filtered(scene, region, pixels_per_unit, include_guides, None)
}

fn export_region_filtered(
    scene: &Scene,
    region: Rect,
    pixels_per_unit: f32,
    include_guides: bool,
    allow: Option<&HashSet<ElementId>>,
) -> Result<Vec<u8>, PersistError> {
    let (width, height) = raster_size(region, pixels_per_unit)?;
    let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
    let ordered = elements_in_paint_order(scene);

    for element in ordered {
        // 组不绘制。隐藏的组会把它的子孙一并跳过。
        if matches!(element, AnyElement::Group(_)) || !scene.is_effectively_visible(element.id()) {
            continue;
        }
        if allow.is_some_and(|ids| !ids.contains(&element.id())) {
            continue;
        }
        let Some((x0, y0, x1, y1)) = pixel_span(paint_bounds(element), region, width, height)
        else {
            continue;
        };
        for y in y0..y1 {
            for x in x0..x1 {
                let world = pixel_center_world(region, width, height, x, y);
                if let Some(color) = sample_element(scene, element, world) {
                    let index = ((y as usize) * (width as usize) + (x as usize)) * 4;
                    blend(&mut pixels[index..index + 4], color, element.blend_mode());
                }
            }
        }
    }
    if include_guides {
        paint_guides(&mut pixels, scene, region, width, height);
    }
    encode_rgba(width, height, &pixels)
}

/// 参考线在 PNG 里的颜色。只在显式打开 `include_guides` 时使用。
const GUIDE_PNG: [u8; 4] = [61, 183, 255, 255];

fn paint_guides(pixels: &mut [u8], scene: &Scene, region: Rect, width: u32, height: u32) {
    for guide in scene.guides().iter() {
        match guide.axis {
            GuideAxis::Vertical => {
                let x = ((guide.position - region.x) / region.width * width as f32).floor();
                if x < 0.0 || x >= width as f32 {
                    continue;
                }
                let x = x as u32;
                for y in 0..height {
                    put_pixel(pixels, width, x, y, GUIDE_PNG);
                }
            }
            GuideAxis::Horizontal => {
                let y = ((guide.position - region.y) / region.height * height as f32).floor();
                if y < 0.0 || y >= height as f32 {
                    continue;
                }
                let y = y as u32;
                for x in 0..width {
                    put_pixel(pixels, width, x, y, GUIDE_PNG);
                }
            }
        }
    }
}

fn put_pixel(pixels: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let index = ((y as usize) * (width as usize) + (x as usize)) * 4;
    if index + 3 < pixels.len() {
        pixels[index..index + 4].copy_from_slice(&color);
    }
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
        AnyElement::Polygon(polygon) => {
            polygon.style.stroke_width.abs() * scale_extent(&polygon.props.transform)
        }
        AnyElement::Frame(frame) => {
            frame.style.stroke_width.abs() * scale_extent(&frame.props.transform)
        }
        AnyElement::Line(_)
        | AnyElement::Path(_)
        | AnyElement::Image(_)
        | AnyElement::Text(_)
        | AnyElement::Group(_) => 0.0,
    };
    let (shadow, transform) = element_shadow(element);
    base.inflate(extra.max(0.0) + shadow_reach(shadow, transform) + blur_reach(element))
}

fn element_blur_radius(element: &AnyElement) -> f32 {
    match element {
        AnyElement::Rectangle(rect) => rect.style.blur_radius,
        AnyElement::Circle(circle) => circle.style.blur_radius,
        AnyElement::Polygon(polygon) => polygon.style.blur_radius,
        AnyElement::Line(line) => line.style.blur_radius,
        AnyElement::Path(path) => path.style.blur_radius,
        AnyElement::Text(text) => text.blur_radius,
        AnyElement::Image(image) => image.blur_radius,
        AnyElement::Frame(frame) => frame.style.blur_radius,
        AnyElement::Group(_) => 0.0,
    }
}

/// 光晕在世界空间里伸出包围盒的距离。关闭时为 0。命中盒不调用这里。
fn blur_reach(element: &AnyElement) -> f32 {
    let sigma = sanitize_blur(element_blur_radius(element));
    if sigma <= 1.0e-4 {
        return 0.0;
    }
    let (_, transform) = element_shadow(element);
    BLUR_EXTENT * sigma * scale_extent(transform)
}

fn element_shadow(element: &AnyElement) -> (DropShadow, &Transform) {
    match element {
        AnyElement::Rectangle(rect) => (rect.style.shadow, &rect.props.transform),
        AnyElement::Circle(circle) => (circle.style.shadow, &circle.props.transform),
        AnyElement::Polygon(polygon) => (polygon.style.shadow, &polygon.props.transform),
        AnyElement::Line(line) => (line.style.shadow, &line.props.transform),
        AnyElement::Path(path) => (path.style.shadow, &path.props.transform),
        AnyElement::Text(text) => (text.shadow, &text.props.transform),
        AnyElement::Image(image) => (image.shadow, &image.props.transform),
        AnyElement::Frame(frame) => (frame.style.shadow, &frame.props.transform),
        AnyElement::Group(group) => (DropShadow::default(), &group.props.transform),
    }
}

/// 阴影在世界空间里可能伸出包围盒的距离。关闭时为 0，命中盒不受影响。
fn shadow_reach(shadow: DropShadow, transform: &Transform) -> f32 {
    if shadow.is_off() {
        return 0.0;
    }
    let shadow = shadow.sanitized();
    let offset = transform.transform_vector(shadow.offset);
    let scale = transform.scale.x.abs().max(transform.scale.y.abs());
    offset.length() + shadow.blur * scale
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

fn sample_element(scene: &Scene, element: &AnyElement, world: Vec2) -> Option<Color> {
    match element {
        AnyElement::Rectangle(rect) => sample_rectangle(rect, world),
        AnyElement::Circle(circle) => sample_circle(circle, world),
        AnyElement::Polygon(polygon) => sample_polygon(polygon, world),
        AnyElement::Frame(frame) => sample_rectangle(&frame.shape_proxy(), world),
        AnyElement::Image(image) => sample_image(scene, image, world),
        AnyElement::Line(line) => sample_line(line, world),
        AnyElement::Path(path) => sample_path(path, world),
        AnyElement::Text(text) => sample_text(text, world),
        AnyElement::Group(_) => None,
    }
}

/// 圆角矩形的有符号距离。负值在内部。半径为 0 时退化为直角盒。
fn rounded_box_sdf(point: Vec2, half: Vec2, radius: f32) -> f32 {
    let radius = radius.max(0.0).min(half.x).min(half.y);
    let q = Vec2::new(
        point.x.abs() - half.x + radius,
        point.y.abs() - half.y + radius,
    );
    let outside = Vec2::new(q.x.max(0.0), q.y.max(0.0)).length();
    let inside = q.x.max(q.y).min(0.0);
    outside + inside - radius
}

fn sample_rectangle(rect: &Rectangle, world: Vec2) -> Option<Color> {
    if !rect.props.visible {
        return None;
    }
    let local = try_local(&rect.props.transform, world)?;
    let half = rect.size * 0.5;
    let stroke = rect.style.stroke_width.max(0.0) * 0.5;
    let radius = rect.effective_corner_radius();
    let half_abs = Vec2::new(half.x.abs(), half.y.abs());
    let distance = |point: Vec2| rounded_box_sdf(point, half_abs, radius);
    if blur_active(rect.style.blur_radius) {
        let body = blurred_shape_body(&rect.style, local, rect.local_bounds(), distance(local));
        return paint_with_shadows(
            body,
            local,
            rect.style.shadow,
            rect.style.inner_shadow,
            rect.style.opacity,
            distance,
        );
    }
    let (in_fill, in_stroke) = if radius > 0.0 {
        let sdf = distance(local);
        (sdf <= 1.0e-4, stroke > 0.0 && sdf.abs() <= stroke + 1.0e-4)
    } else {
        let ax = local.x.abs();
        let ay = local.y.abs();
        (
            ax <= half.x && ay <= half.y,
            stroke > 0.0
                && ax <= half.x + stroke
                && ay <= half.y + stroke
                && (ax >= (half.x - stroke).max(0.0) || ay >= (half.y - stroke).max(0.0)),
        )
    };
    let mut body = None;
    if in_stroke {
        if let Some(color) = rect.style.stroke {
            body = with_opacity(color, rect.style.opacity);
        }
    }
    if body.is_none() && in_fill {
        if let Some(color) = rect.style.sample_fill(local, rect.local_bounds()) {
            body = with_opacity(color, rect.style.opacity);
        }
    }
    paint_with_shadows(
        body,
        local,
        rect.style.shadow,
        rect.style.inner_shadow,
        rect.style.opacity,
        distance,
    )
}

fn sample_polygon(polygon: &Polygon, world: Vec2) -> Option<Color> {
    if !polygon.props.visible {
        return None;
    }
    let local = try_local(&polygon.props.transform, world)?;
    let distance = |point: Vec2| polygon.silhouette_distance(point);
    let body = if blur_active(polygon.style.blur_radius) {
        blurred_shape_body(
            &polygon.style,
            local,
            polygon.local_bounds(),
            distance(local),
        )
    } else {
        polygon.sample_local(local)
    };
    paint_with_shadows(
        body,
        local,
        polygon.style.shadow,
        polygon.style.inner_shadow,
        polygon.style.opacity,
        distance,
    )
}

fn sample_circle(circle: &Circle, world: Vec2) -> Option<Color> {
    if !circle.props.visible {
        return None;
    }
    let local = try_local(&circle.props.transform, world)?;
    let radius = circle.radius;
    let distance = |point: Vec2| point.length() - radius;
    if blur_active(circle.style.blur_radius) {
        let body = blurred_shape_body(&circle.style, local, circle.local_bounds(), distance(local));
        return paint_with_shadows(
            body,
            local,
            circle.style.shadow,
            circle.style.inner_shadow,
            circle.style.opacity,
            distance,
        );
    }
    let dist = local.length();
    let stroke = circle.style.stroke_width.max(0.0) * 0.5;
    let in_fill = dist <= circle.radius;
    let in_stroke =
        stroke > 0.0 && dist <= circle.radius + stroke && dist >= (circle.radius - stroke).max(0.0);
    let mut body = None;
    if in_stroke {
        if let Some(color) = circle.style.stroke {
            body = with_opacity(color, circle.style.opacity);
        }
    }
    if body.is_none() && in_fill {
        if let Some(color) = circle.style.sample_fill(local, circle.local_bounds()) {
            body = with_opacity(color, circle.style.opacity);
        }
    }
    paint_with_shadows(
        body,
        local,
        circle.style.shadow,
        circle.style.inner_shadow,
        circle.style.opacity,
        distance,
    )
}

fn sample_image(scene: &Scene, image: &Image, world: Vec2) -> Option<Color> {
    if !image.props.visible {
        return None;
    }
    let local = try_local(&image.props.transform, world)?;
    let body = if blur_active(image.blur_radius) {
        let sigma = sanitize_blur(image.blur_radius);
        kernel_color(|i, j| {
            image_color_at_local(
                scene,
                image,
                local + Vec2::new(i as f32 * sigma, j as f32 * sigma),
            )
        })
    } else {
        image_color_at_world(scene, image, world)
    };
    let visible = image.visible_local_rect();
    paint_with_shadows(
        body,
        local,
        image.shadow,
        image.inner_shadow,
        image.alpha,
        |point| {
            axis_box_sdf(
                point,
                Vec2::new(visible.x, visible.y),
                Vec2::new(visible.width, visible.height),
            )
        },
    )
}

fn image_color_at_local(scene: &Scene, image: &Image, local: Vec2) -> Option<Color> {
    if !local.x.is_finite() || !local.y.is_finite() {
        return None;
    }
    image_color_at_world(scene, image, image.props.transform.transform_point(local))
}

fn image_color_at_world(scene: &Scene, image: &Image, world: Vec2) -> Option<Color> {
    if !image.contains_point(world) {
        return None;
    }
    if let Some(asset) = scene.image_asset(&image.asset_id) {
        image.sample_pixels(world, asset.width, asset.height, &asset.rgba)
    } else {
        let mut color = IMAGE_PNG_PLACEHOLDER;
        color.a *= image.alpha.clamp(0.0, 1.0);
        if color.a <= 0.0 {
            None
        } else {
            Some(color)
        }
    }
}

fn sample_line(line: &crate::element::Line, world: Vec2) -> Option<Color> {
    if blur_active(line.style.blur_radius) {
        if !line.props.visible {
            return None;
        }
        let local = try_local(&line.props.transform, world)?;
        let body = blurred_mask(
            stroke_paint_color(&line.style),
            line.silhouette_distance(local),
            line.style.blur_radius,
        );
        return paint_with_shadows(
            body,
            local,
            line.style.shadow,
            line.style.inner_shadow,
            line.style.opacity,
            |point| line.silhouette_distance(point),
        );
    }
    let body = line.color_at(world);
    let Some(local) = try_local(&line.props.transform, world) else {
        return body;
    };
    paint_with_shadows(
        body,
        local,
        line.style.shadow,
        line.style.inner_shadow,
        line.style.opacity,
        |point| line.silhouette_distance(point),
    )
}

fn sample_path(path: &crate::element::Path, world: Vec2) -> Option<Color> {
    if blur_active(path.style.blur_radius) {
        if !path.props.visible {
            return None;
        }
        let local = try_local(&path.props.transform, world)?;
        let body = if path.is_filled() {
            blurred_shape_body(
                &path.style,
                local,
                path.local_bounds(),
                path.silhouette_distance(local),
            )
        } else if path.points().len() < 2 {
            return None;
        } else {
            blurred_mask(
                stroke_paint_color(&path.style),
                path.silhouette_distance(local),
                path.style.blur_radius,
            )
        };
        return paint_with_shadows(
            body,
            local,
            path.style.shadow,
            path.style.inner_shadow,
            path.style.opacity,
            |point| path.silhouette_distance(point),
        );
    }
    let body = path.color_at(world);
    let Some(local) = try_local(&path.props.transform, world) else {
        return body;
    };
    paint_with_shadows(
        body,
        local,
        path.style.shadow,
        path.style.inner_shadow,
        path.style.opacity,
        |point| path.silhouette_distance(point),
    )
}

fn sample_text(text: &Text, world: Vec2) -> Option<Color> {
    if text.editing {
        return text.color_at(world);
    }
    let Some(local) = try_local(&text.props.transform, world) else {
        return text.color_at(world);
    };
    let body = if blur_active(text.blur_radius) {
        let sigma = sanitize_blur(text.blur_radius);
        kernel_color(|i, j| {
            let point = local + Vec2::new(i as f32 * sigma, j as f32 * sigma);
            text.color_at(text.props.transform.transform_point(point))
        })
    } else {
        text.color_at(world)
    };
    let bounds = text.local_bounds();
    paint_with_shadows(
        body,
        local,
        text.shadow,
        text.inner_shadow,
        text.opacity,
        |point| {
            axis_box_sdf(
                point,
                Vec2::new(bounds.x, bounds.y),
                Vec2::new(bounds.width, bounds.height),
            )
        },
    )
}

/// 填充盖住内部，描边再盖在上面。颜色已经乘过不透明度。σ 接近 0 时不要调用。
fn blurred_shape_body(
    style: &ElementStyle,
    local: Vec2,
    bounds: Rect,
    signed_distance: f32,
) -> Option<Color> {
    let fill = style
        .sample_fill(local, bounds)
        .and_then(|color| with_opacity(color, style.opacity));
    let stroke = if style.stroke_width.max(0.0) > 0.0 {
        style
            .stroke
            .and_then(|color| with_opacity(color, style.opacity))
    } else {
        None
    };
    blurred_paint(
        fill,
        stroke,
        style.stroke_width.max(0.0) * 0.5,
        signed_distance,
        style.blur_radius,
    )
}

/// 描边色优先，否则填充色，并乘上不透明度。与 `element::stroke::paint_color` 一致。
fn stroke_paint_color(style: &ElementStyle) -> Color {
    let color = style.stroke.or(style.fill).unwrap_or(Color::BLACK);
    Color::new(
        color.r,
        color.g,
        color.b,
        (color.a * style.opacity.clamp(0.0, 1.0)).clamp(0.0, 1.0),
    )
}

fn axis_box_sdf(point: Vec2, origin: Vec2, size: Vec2) -> f32 {
    let min_x = origin.x.min(origin.x + size.x);
    let min_y = origin.y.min(origin.y + size.y);
    let max_x = origin.x.max(origin.x + size.x);
    let max_y = origin.y.max(origin.y + size.y);
    let half = Vec2::new((max_x - min_x) * 0.5, (max_y - min_y) * 0.5);
    let center = Vec2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
    rounded_box_sdf(point - center, half, 0.0)
}

fn paint_with_shadows(
    body: Option<Color>,
    local: Vec2,
    shadow: DropShadow,
    inner: InnerShadow,
    opacity: f32,
    distance_at: impl Fn(Vec2) -> f32,
) -> Option<Color> {
    let mut painted = body;
    if !shadow.is_off() {
        let shadow = shadow.sanitized();
        let coverage = feather_coverage(distance_at(local - shadow.offset), shadow.blur);
        let alpha = shadow.color.a * opacity.clamp(0.0, 1.0) * coverage;
        if alpha > 1.0e-4 {
            let shadow_color = Color::new(
                shadow.color.r,
                shadow.color.g,
                shadow.color.b,
                alpha.clamp(0.0, 1.0),
            );
            painted = match painted {
                Some(paint) => Some(composite_over(paint, shadow_color)),
                None => Some(shadow_color),
            };
        }
    }
    if !inner.is_off() && distance_at(local) <= 0.0 {
        let inner = inner.sanitized();
        let d = distance_at(local - inner.offset) + inner.spread;
        let coverage = feather_coverage(-d, inner.blur);
        let alpha = inner.color.a * opacity.clamp(0.0, 1.0) * coverage;
        if alpha > 1.0e-4 {
            let shade = Color::new(
                inner.color.r,
                inner.color.g,
                inner.color.b,
                alpha.clamp(0.0, 1.0),
            );
            painted = match painted {
                Some(paint) => Some(composite_over(shade, paint)),
                None => Some(shade),
            };
        }
    }
    painted
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

fn blend(dst: &mut [u8], src: Color, mode: BlendMode) {
    let backdrop = Color::new(
        dst[0] as f32 / 255.0,
        dst[1] as f32 / 255.0,
        dst[2] as f32 / 255.0,
        dst[3] as f32 / 255.0,
    );
    let out = composite_straight(backdrop, src, mode);
    dst[0] = channel(out.r);
    dst[1] = channel(out.g);
    dst[2] = channel(out.b);
    dst[3] = channel(out.a);
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
        channel, export_frame_png, export_frame_png_scaled, export_selection_png,
        export_selection_png_scaled, export_viewport_png, export_viewport_png_scaled,
        export_viewport_png_with_guides, sample_rectangle, PersistError, IMAGE_PNG_PLACEHOLDER,
        MAX_PNG_DIMENSION, SCENE_FORMAT_VERSION,
    };
    use crate::element::{
        Circle, DropShadow, Element, ElementStyle, FillKind, Frame, GradientStop, Image, Line,
        Path, Polygon, PolygonKind, Rectangle, Text, TextAlign,
    };
    use crate::snap::GuideAxis;
    use crate::{AnyElement, BlendMode, Scene, Viewport};
    use infinitecanvas_math::{Color, Rect, Transform, Vec2};
    use serde_json::{json, Value};

    #[test]
    fn drop_shadow_samples_beside_the_shape_without_replacing_an_opaque_fill() {
        let mut rect = Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0))
            .with_style(ElementStyle::fill_only(Color::WHITE));
        rect.style.shadow =
            DropShadow::new(Vec2::new(20.0, 0.0), 0.0, Color::new(0.0, 0.0, 0.0, 0.5));
        let center = sample_rectangle(&rect, Vec2::ZERO).unwrap();
        assert!(center.r > 0.9);
        let shadow = sample_rectangle(&rect, Vec2::new(20.0, 0.0)).unwrap();
        assert!(shadow.a > 0.4 && shadow.r < 0.05);
        assert!(sample_rectangle(&rect, Vec2::new(80.0, 0.0)).is_none());
        assert!(!rect.contains_point(Vec2::new(20.0, 0.0)));
    }

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
            ..ElementStyle::new()
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
                assert_eq!(rect.corner_radius(), 0.0);
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
        value["elements"][3]["data"]
            .as_object_mut()
            .unwrap()
            .remove("end_marker");
        value["elements"][3]["data"]
            .as_object_mut()
            .unwrap()
            .remove("start_marker");

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
                assert_eq!(text.opacity, 1.0);
                assert_eq!(text.stroke, None);
                assert_eq!(text.stroke_width, 0.0);
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
    fn polygon_and_star_round_trip_and_old_documents_stay_valid() {
        let mut scene = Scene::new();
        let star_id = scene.next_id();
        let mut star = Polygon::star(star_id, Vec2::new(4.0, 5.0), 30.0).with_inner_radius(0.25);
        star.set_sides(7);
        star.set_rotation(0.2);
        scene.add_polygon(star);
        let hex_id = scene.next_id();
        scene.add_polygon(Polygon::hexagon(hex_id, Vec2::new(-8.0, 1.0), 12.0));
        let json = scene.to_json().unwrap();
        assert!(json.contains("\"type\":\"polygon\""));
        assert!(json.contains("\"kind\":\"star\""));
        let restored = Scene::from_json(&json).unwrap();
        assert_eq!(restored, scene);
        match restored.get(1).unwrap() {
            AnyElement::Polygon(shape) => {
                assert_eq!(shape.kind(), PolygonKind::Star);
                assert_eq!(shape.sides(), 7);
                assert_eq!(shape.inner_radius(), 0.25);
            }
            other => panic!("expected star, got {other:?}"),
        }

        let size = serde_json::to_value(Vec2::new(4.0, 2.0)).unwrap();
        let legacy = json!({
            "version": 1,
            "elements": [
                {
                    "type": "rectangle",
                    "data": {
                        "props": { "id": 1 },
                        "size": size
                    }
                },
                {
                    "type": "polygon",
                    "data": { "props": { "id": 2 }, "radius": 10.0, "sides": 100 }
                },
                {
                    "type": "star",
                    "data": { "props": { "id": 3 }, "radius": 8.0 }
                }
            ]
        });
        let old = Scene::from_json(&legacy.to_string()).unwrap();
        match old.get(1).unwrap() {
            AnyElement::Rectangle(rect) => assert_eq!(rect.corner_radius(), 0.0),
            other => panic!("expected rectangle, got {other:?}"),
        }
        match old.get(2).unwrap() {
            AnyElement::Polygon(shape) => {
                assert_eq!(shape.kind(), PolygonKind::Polygon);
                assert_eq!(shape.sides(), 64);
            }
            other => panic!("expected polygon, got {other:?}"),
        }
        match old.get(3).unwrap() {
            AnyElement::Polygon(shape) => {
                assert_eq!(shape.kind(), PolygonKind::Star);
                assert_eq!(shape.sides(), 5);
                assert_eq!(shape.inner_radius(), 0.5);
            }
            other => panic!("expected star, got {other:?}"),
        }

        let mut painted = Scene::new();
        painted.add_polygon(
            Polygon::hexagon(1, Vec2::ZERO, 10.0).with_style(ElementStyle::fill_only(Color::RED)),
        );
        assert_pixel(sample_pixel(&painted, Vec2::ZERO), [255, 0, 0, 255]);
        assert_pixel(sample_pixel(&painted, Vec2::new(9.5, 0.0)), [0, 0, 0, 0]);
        assert_pixel(
            sample_pixel(&painted, Vec2::new(5.0, 0.0)),
            [255, 0, 0, 255],
        );
    }

    #[test]
    fn png_rounded_rect_leaves_the_outer_corner_empty() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::ZERO, Vec2::new(20.0, 20.0))
                .with_style(ElementStyle::fill_only(Color::BLUE))
                .with_corner_radius(10.0),
        );
        assert_pixel(sample_pixel(&scene, Vec2::ZERO), [0, 0, 255, 255]);
        assert_pixel(sample_pixel(&scene, Vec2::new(9.5, 0.0)), [0, 0, 255, 255]);
        assert_pixel(sample_pixel(&scene, Vec2::new(9.0, 9.0)), [0, 0, 0, 0]);
    }

    #[test]
    fn json_round_trip_stores_image_bytes_once_and_png_samples_them() {
        let mut pixels = vec![0u8; 2 * 2 * 4];
        pixels[0..4].copy_from_slice(&[255, 0, 0, 255]);
        pixels[4..8].copy_from_slice(&[0, 0, 255, 255]);
        pixels[8..12].copy_from_slice(&[0, 255, 0, 255]);
        pixels[12..16].copy_from_slice(&[255, 255, 0, 255]);
        let png = crate::asset::encode_png_rgba(2, 2, &pixels).unwrap();

        let mut scene = Scene::new();
        let asset_id = scene.insert_image_bytes(&png, "image/png").unwrap();
        let first = scene.next_id();
        scene.add_image(Image::from_asset(first, Vec2::ZERO, &asset_id, 2, 2));
        let second = scene.next_id();
        let mut shifted = Image::from_asset(second, Vec2::new(20.0, 0.0), &asset_id, 2, 2);
        shifted.set_alpha(1.0);
        scene.add_image(shifted);

        let json = scene.to_json().unwrap();
        assert_eq!(json.matches("data:image/png;base64,").count(), 1);
        assert!(json.contains(&asset_id));
        let restored = Scene::from_json(&json).unwrap();
        assert_eq!(restored, scene);
        assert_eq!(restored.image_asset_count(), 1);
        assert_eq!(restored.asset_refcount(&asset_id), 2);
        let asset = restored.image_asset(&asset_id).unwrap();
        assert_eq!(asset.width, 2);
        assert_eq!(asset.height, 2);
        assert_eq!(asset.rgba, pixels);

        assert_pixel(
            sample_pixel(&restored, Vec2::new(0.5, 0.5)),
            [255, 0, 0, 255],
        );
        assert_pixel(
            sample_pixel(&restored, Vec2::new(1.5, 0.5)),
            [0, 0, 255, 255],
        );
        assert_pixel(
            sample_pixel(&restored, Vec2::new(21.5, 1.5)),
            [255, 255, 0, 255],
        );
        let placeholder = [
            channel(IMAGE_PNG_PLACEHOLDER.r),
            channel(IMAGE_PNG_PLACEHOLDER.g),
            channel(IMAGE_PNG_PLACEHOLDER.b),
            255,
        ];
        assert_ne!(sample_pixel(&restored, Vec2::new(0.5, 0.5)), placeholder);
    }

    #[test]
    fn image_crop_defaults_to_full_and_png_samples_only_the_window() {
        let size = serde_json::to_value(Vec2::new(4.0, 1.0)).unwrap();
        let legacy = json!({
            "version": 1,
            "elements": [{
                "type": "image",
                "data": {
                    "props": { "id": 1 },
                    "size": size,
                    "texture_id": "old"
                }
            }]
        });
        let old = Scene::from_json(&legacy.to_string()).unwrap();
        match old.get(1).unwrap() {
            AnyElement::Image(image) => {
                assert!(image.crop_is_full());
                assert!(image.contains_point(Vec2::new(3.5, 0.5)));
            }
            other => panic!("expected image, got {other:?}"),
        }
        assert!(!old.to_json().unwrap().contains("\"crop\""));

        let mut pixels = vec![0u8; 4 * 4];
        pixels[0..4].copy_from_slice(&[255, 0, 0, 255]);
        pixels[4..8].copy_from_slice(&[0, 255, 0, 255]);
        pixels[8..12].copy_from_slice(&[0, 0, 255, 255]);
        pixels[12..16].copy_from_slice(&[255, 255, 0, 255]);
        let encoded = crate::asset::encode_png_rgba(4, 1, &pixels).unwrap();
        let mut scene = Scene::new();
        let asset_id = scene.insert_image_bytes(&encoded, "image/png").unwrap();
        let mut image = Image::from_asset(1, Vec2::ZERO, &asset_id, 4, 1);
        let transform = image.props.transform;
        image.set_crop(Rect::new(0.5, 0.0, 0.5, 1.0));
        scene.add_image(image);

        let json = scene.to_json().unwrap();
        assert!(json.contains("\"crop\""));
        let restored = Scene::from_json(&json).unwrap();
        match restored.get(1).unwrap() {
            AnyElement::Image(image) => {
                assert_eq!(image.props.transform, transform);
                assert_eq!(image.normalized_crop(), Rect::new(0.5, 0.0, 0.5, 1.0));
            }
            other => panic!("expected image, got {other:?}"),
        }
        assert_pixel(sample_pixel(&restored, Vec2::new(0.5, 0.5)), [0, 0, 0, 0]);
        assert_pixel(
            sample_pixel(&restored, Vec2::new(2.5, 0.5)),
            [0, 0, 255, 255],
        );
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
    fn png_export_scale_multiplies_pixel_dimensions() {
        let mut scene = Scene::new();
        let mut rect = Rectangle::new(1, Vec2::ZERO, Vec2::new(20.0, 10.0))
            .with_style(ElementStyle::fill_only(Color::RED));
        rect.props.selected = true;
        scene.add_rect(rect);

        let (w1, h1, _) = decode_png(&export_selection_png(&scene, 1.0).unwrap());
        let (w2, h2, _) = decode_png(&export_selection_png_scaled(&scene, 1.0, 2.0).unwrap());
        let (w3, h3, _) = decode_png(&export_selection_png_scaled(&scene, 1.0, 3.0).unwrap());
        assert_eq!((w2, h2), (w1 * 2, h1 * 2));
        assert_eq!((w3, h3), (w1 * 3, h1 * 3));
        assert_eq!(w2 as u64 * h2 as u64, (w1 as u64) * (h1 as u64) * 4);

        let mut viewport = Viewport::new(Vec2::new(40.0, 30.0));
        viewport.resize(Vec2::new(40.0, 30.0));
        let (vw1, vh1, _) = decode_png(&export_viewport_png(&scene, &viewport).unwrap());
        let (vw2, vh2, _) =
            decode_png(&export_viewport_png_scaled(&scene, &viewport, false, 2.0).unwrap());
        assert_eq!((vw2, vh2), (vw1 * 2, vh1 * 2));

        let frame_id = 7;
        scene.add_frame(Frame::new(
            frame_id,
            Vec2::new(100.0, 100.0),
            Vec2::new(50.0, 40.0),
        ));
        let (fw1, fh1, _) = decode_png(&export_frame_png(&scene, frame_id, 1.0).unwrap());
        let (fw2, fh2, _) =
            decode_png(&export_frame_png_scaled(&scene, frame_id, 1.0, 2.0).unwrap());
        assert_eq!((fw1, fh1), (50, 40));
        assert_eq!((fw2, fh2), (100, 80));

        assert_eq!(
            export_selection_png_scaled(&scene, 1.0, 0.0).unwrap_err(),
            PersistError::InvalidScale
        );
        assert_eq!(
            export_viewport_png_scaled(&scene, &viewport, false, f32::NAN).unwrap_err(),
            PersistError::InvalidScale
        );
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

    #[test]
    fn guides_round_trip_and_old_documents_still_load() {
        let legacy = Scene::from_json(r#"{"version":1,"elements":[]}"#).unwrap();
        assert!(legacy.guides().is_empty());

        let mut scene = Scene::new();
        scene.add_guide(GuideAxis::Horizontal, 12.5).unwrap();
        scene.add_guide(GuideAxis::Vertical, -4.0).unwrap();
        let json = scene.to_json().unwrap();
        assert!(json.contains("\"guides\""));
        assert!(json.contains("\"horizontal\""));
        let restored = Scene::from_json(&json).unwrap();
        assert_eq!(restored, scene);
        assert_eq!(restored.to_json().unwrap(), json);

        let duplicate = r#"{"version":1,"elements":[],"guides":[{"id":1,"axis":"vertical","position":0},{"id":1,"axis":"horizontal","position":1}]}"#;
        assert!(Scene::from_json(duplicate).is_err());
    }

    #[test]
    fn png_export_skips_guides_unless_asked() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(8.0, 8.0)));
        let viewport = Viewport::new(Vec2::new(32.0, 32.0));
        let plain = export_viewport_png(&scene, &viewport).unwrap();
        scene.add_guide(GuideAxis::Vertical, 0.0).unwrap();
        assert_eq!(export_viewport_png(&scene, &viewport).unwrap(), plain);
        assert_eq!(
            export_viewport_png_with_guides(&scene, &viewport, false).unwrap(),
            plain
        );
        assert_ne!(
            export_viewport_png_with_guides(&scene, &viewport, true).unwrap(),
            plain
        );
    }

    #[test]
    fn png_samples_gradient_fill_and_keeps_solid_stroke() {
        let mut scene = Scene::new();
        let mut under = Rectangle::new(1, Vec2::ZERO, Vec2::new(40.0, 20.0))
            .with_style(ElementStyle::fill_only(Color::GREEN));
        under.props.z_index = 0;
        scene.add_rect(under);

        let mut over = Rectangle::new(2, Vec2::new(10.0, 0.0), Vec2::new(20.0, 16.0));
        over.props.z_index = 1;
        over.style.fill_kind = FillKind::Linear;
        over.style.fill_angle = 0.0;
        over.style.opacity = 1.0;
        over.style.stroke = Some(Color::RED);
        over.style.stroke_width = 4.0;
        over.style.fill_stops = vec![
            GradientStop {
                offset: 0.0,
                color: Color::RED,
            },
            GradientStop {
                offset: 1.0,
                color: Color::BLUE,
            },
        ];
        scene.add_rect(over);

        assert_pixel(
            sample_pixel(&scene, Vec2::new(-15.0, 0.0)),
            [0, 255, 0, 255],
        );
        let left = sample_pixel(&scene, Vec2::new(3.0, 0.0));
        let right = sample_pixel(&scene, Vec2::new(17.0, 0.0));
        assert!(left[0] > 180 && left[2] < 80 && left[1] < 10, "{left:?}");
        assert!(
            right[2] > 180 && right[0] < 80 && right[1] < 10,
            "{right:?}"
        );
        assert_pixel(sample_pixel(&scene, Vec2::new(10.0, 9.0)), [255, 0, 0, 255]);

        scene.set_visible(2, false);
        assert_pixel(sample_pixel(&scene, Vec2::new(10.0, 0.0)), [0, 255, 0, 255]);
    }

    #[test]
    fn png_blend_respects_mode_opacity_visibility_and_paint_order() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::ZERO, Vec2::new(8.0, 8.0))
                .with_style(ElementStyle::fill_only(Color::new(1.0, 0.0, 0.0, 1.0))),
        );
        let mut top = Rectangle::new(2, Vec2::ZERO, Vec2::new(8.0, 8.0))
            .with_style(ElementStyle::fill_only(Color::new(0.0, 0.0, 1.0, 1.0)));
        top.props.z_index = 2;
        top.style.blend_mode = BlendMode::Multiply;
        scene.add_rect(top);

        // 蓝乘红 = 黑。普通叠加上层应仍是蓝。
        assert_pixel(sample_pixel(&scene, Vec2::ZERO), [0, 0, 0, 255]);

        match scene.get_mut(2).unwrap() {
            AnyElement::Rectangle(rect) => {
                rect.style.opacity = 0.5;
            }
            other => panic!("rect, got {other:?}"),
        }
        // 半透明正片叠底：0.5 * 黑 + 0.5 * 红 = 暗红。
        assert_pixel(sample_pixel(&scene, Vec2::ZERO), [128, 0, 0, 255]);

        match scene.get_mut(2).unwrap() {
            AnyElement::Rectangle(rect) => {
                rect.props.visible = false;
                rect.style.opacity = 1.0;
            }
            other => panic!("rect, got {other:?}"),
        }
        assert_pixel(sample_pixel(&scene, Vec2::ZERO), [255, 0, 0, 255]);

        match scene.get_mut(1).unwrap() {
            AnyElement::Rectangle(rect) => rect.props.z_index = 2,
            other => panic!("rect, got {other:?}"),
        }
        match scene.get_mut(2).unwrap() {
            AnyElement::Rectangle(rect) => {
                rect.props.visible = true;
                rect.props.z_index = 0;
                rect.style.opacity = 1.0;
                rect.style.blend_mode = BlendMode::Multiply;
                rect.style.fill = Some(Color::new(0.0, 0.0, 1.0, 1.0));
            }
            other => panic!("rect, got {other:?}"),
        }
        // 正片叠底在下层，上层不透明红盖住它。若按 id 而不是 z 绘制，这里会变成黑。
        assert_pixel(sample_pixel(&scene, Vec2::ZERO), [255, 0, 0, 255]);
    }

    #[test]
    fn gaussian_blur_softens_png_without_enlarging_hits() {
        let mut solid = ElementStyle::fill_only(Color::RED);
        solid.blur_radius = 4.0;
        let rect = Rectangle::new(1, Vec2::ZERO, Vec2::new(80.0, 80.0)).with_style(solid.clone());
        let center = sample_rectangle(&rect, Vec2::ZERO).unwrap();
        assert!(center.a > 0.95);
        let edge = sample_rectangle(&rect, Vec2::new(40.0, 0.0)).unwrap();
        assert!((edge.a - 0.5).abs() < 0.05);
        let halo = sample_rectangle(&rect, Vec2::new(44.0, 0.0)).unwrap();
        assert!((halo.a - 0.1587).abs() < 0.02);
        assert!(sample_rectangle(&rect, Vec2::new(60.0, 0.0)).is_none());

        solid.opacity = 0.5;
        let faint = Rectangle::new(2, Vec2::ZERO, Vec2::new(80.0, 80.0)).with_style(solid);
        let faded = sample_rectangle(&faint, Vec2::ZERO).unwrap();
        assert!((faded.a - 0.5).abs() < 0.05);

        let mut scene = Scene::new();
        scene.add_rect(rect);
        let pixel = sample_pixel(&scene, Vec2::new(44.0, 0.0));
        assert!(pixel[0] > 200, "{pixel:?}");
        assert!((20..=80).contains(&pixel[3]), "{pixel:?}");
        assert!(crate::hit_test_point(
            &scene,
            Vec2::new(44.0, 0.0),
            &crate::HitTestOptions::default()
        )
        .is_empty());

        let mut cover = Rectangle::new(3, Vec2::ZERO, Vec2::new(20.0, 20.0))
            .with_style(ElementStyle::fill_only(Color::BLUE));
        cover.props.z_index = 5;
        scene.add_rect(cover);
        let top = sample_pixel(&scene, Vec2::ZERO);
        assert!(top[2] > 250 && top[0] < 5, "{top:?}");

        let mut image = Image::new(
            4,
            Vec2::new(400.0, 0.0),
            Vec2::new(80.0, 80.0),
            "missing".to_string(),
        );
        image.blur_radius = 4.0;
        scene.add_image(image);
        let plate = sample_pixel(&scene, Vec2::new(440.0, 40.0));
        let expect = [
            (IMAGE_PNG_PLACEHOLDER.r * 255.0).round() as u8,
            (IMAGE_PNG_PLACEHOLDER.g * 255.0).round() as u8,
            (IMAGE_PNG_PLACEHOLDER.b * 255.0).round() as u8,
        ];
        for (got, want) in plate[..3].iter().zip(expect) {
            assert!((*got as i16 - want as i16).abs() <= 2, "{plate:?}");
        }
        assert!(plate[3] > 240, "{plate:?}");

        let mut stroke = ElementStyle::stroke_only(Color::RED, 4.0);
        stroke.blur_radius = 4.0;
        let line = Line::from_world(
            5,
            Vec2::new(0.0, 300.0),
            Vec2::new(100.0, 300.0),
            stroke,
            false,
        );
        let outside = Vec2::new(50.0, 306.0);
        assert!(!line.contains_point(outside));
        let ink = super::sample_line(&line, outside).unwrap();
        assert!(ink.r > 0.9, "{ink:?}");
        assert!((ink.a - 0.1587).abs() < 0.02, "{ink:?}");
        scene.add_line(line);
        assert!(
            crate::hit_test_point(&scene, outside, &crate::HitTestOptions::default()).is_empty()
        );

        let size = serde_json::to_value(Vec2::new(12.0, 12.0)).unwrap();
        let negative = json!({
            "version": 1,
            "elements": [{
                "type": "rectangle",
                "data": {
                    "props": { "id": 8 },
                    "size": size,
                    "style": { "blur_radius": -8.0 }
                }
            }]
        });
        let loaded = Scene::from_json(&negative.to_string()).unwrap();
        match loaded.get(8).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.style.blur_radius, 0.0);
                assert!(!serde_json::to_value(rect).unwrap()["style"]
                    .get("blur_radius")
                    .is_some());
            }
            other => panic!("rect, got {other:?}"),
        }
    }
}
