//! 场景级图片资源表
//!
//! 同一张图被多个 [`crate::Image`] 引用时，像素只存一份。
//! 元素增删通过引用计数维护；最后一个引用离开场景时，资源从这张表里移出。
//!
//! ```text
//! PNG/JPEG bytes
//!       │
//!       ▼
//!  decode ── edge > 4096 ──► downsample ──► re-encode PNG
//!       │
//!       ▼
//!  AssetTable (id = sha256 of stored bytes)
//!       │
//!       ├── Image.asset_id  (many elements, one asset)
//!       ├── JSON data URL    (written once)
//!       └── TextureManager   (GPU texture id = asset id)
//! ```

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;

/// 降采样后的最大边长。WebGPU 常见纹理上限是 8192，这里留一倍余量。
pub const MAX_IMAGE_EDGE: u32 = 4096;

/// 解码前拒绝的源图像素数，避免一张超大图把内存撑满。
const MAX_SOURCE_PIXELS: u64 = 80_000_000;

const HEX: &[u8; 16] = b"0123456789abcdef";

/// 放入资源表或从 JSON 读回失败的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetError {
    /// 文件头和 mime 都不是 PNG / JPEG。
    UnsupportedFormat(String),
    /// 解码或 data URL 解析失败。
    Decode(String),
    /// 宽或高为 0。
    EmptyImage,
    /// 两份资源使用了同一个 id。
    DuplicateId(String),
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetError::UnsupportedFormat(mime) => {
                write!(f, "unsupported image format: {mime}")
            }
            AssetError::Decode(message) => write!(f, "image decode failed: {message}"),
            AssetError::EmptyImage => write!(f, "image has zero width or height"),
            AssetError::DuplicateId(id) => write!(f, "duplicate image asset id {id}"),
        }
    }
}

impl std::error::Error for AssetError {}

/// 解码后的 RGBA8 图像。
struct Decoded {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// 一张图片资源。
///
/// `encoded` 是写入 JSON 的字节（降采样后会重新编码成 PNG）。
/// `rgba` 给 GPU 上传和 PNG 导出用。`refs` 是当前场景里引用它的元素个数。
#[derive(Clone, PartialEq, Eq)]
pub struct ImageAsset {
    /// 内容哈希 id，形如 `img_` + sha256。
    pub id: String,
    /// `image/png` 或 `image/jpeg`。
    pub mime: String,
    /// 像素宽（降采样之后）。
    pub width: u32,
    /// 像素高（降采样之后）。
    pub height: u32,
    /// RGBA8，长度是 `width * height * 4`。
    pub rgba: Vec<u8>,
    /// 持久化用的编码字节。
    pub encoded: Vec<u8>,
    refs: usize,
}

impl std::fmt::Debug for ImageAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageAsset")
            .field("id", &self.id)
            .field("mime", &self.mime)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("rgba_len", &self.rgba.len())
            .field("encoded_len", &self.encoded.len())
            .field("refs", &self.refs)
            .finish()
    }
}

impl ImageAsset {
    /// 当前有多少个元素引用这张图。
    pub fn refs(&self) -> usize {
        self.refs
    }

    /// `data:<mime>;base64,...`，JSON 里每个资源只写一次。
    pub fn to_data_url(&self) -> String {
        let encoded = base64::engine::general_purpose::STANDARD.encode(&self.encoded);
        format!("data:{};base64,{encoded}", self.mime)
    }

    /// 序列化到场景文件的那一行。
    pub fn to_record(&self) -> AssetRecord {
        AssetRecord {
            id: self.id.clone(),
            mime: self.mime.clone(),
            width: self.width,
            height: self.height,
            data: self.to_data_url(),
        }
    }
}

/// 场景 JSON 里的一条图片资源。
///
/// `data` 是 data URL 或纯 base64。缺省的 `mime` / `width` / `height`
/// 会在解码后用真实像素覆盖。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetRecord {
    /// 元素 `asset_id` 引用的 id。
    pub id: String,
    /// `image/png` 或 `image/jpeg`。可省略，这时看 data URL 和文件头。
    #[serde(default)]
    pub mime: String,
    /// 声明的像素宽。解码结果不一致时以解码为准。
    #[serde(default)]
    pub width: u32,
    /// 声明的像素高。
    #[serde(default)]
    pub height: u32,
    /// data URL 或 base64。
    pub data: String,
}

/// 场景拥有的图片资源。
///
/// 相等比较只看资源内容和引用计数，不看「这份字节曾经从哪个输入哈希进来」。
#[derive(Debug, Clone, Default)]
pub struct AssetTable {
    assets: HashMap<String, ImageAsset>,
    /// 原始输入字节的哈希 → 资源 id。避免同一文件重复解码。不写入 JSON。
    by_input: HashMap<String, String>,
}

impl PartialEq for AssetTable {
    fn eq(&self, other: &Self) -> bool {
        self.assets == other.assets
    }
}

impl AssetTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 表里的资源数量，包含引用计数已经降到 0、但还没被取走的条目。
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    /// 是否没有任何资源。
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// 按 id 取资源。
    pub fn get(&self, id: &str) -> Option<&ImageAsset> {
        self.assets.get(id)
    }

    /// 引用计数。不存在时为 0。
    pub fn refs(&self, id: &str) -> usize {
        self.assets.get(id).map(|asset| asset.refs).unwrap_or(0)
    }

    /// 仍被元素引用的资源 id。
    pub fn referenced_ids(&self) -> HashSet<String> {
        self.assets
            .iter()
            .filter(|(_, asset)| asset.refs > 0)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// 解码 PNG/JPEG 并放入表中。
    ///
    /// 相同的输入字节（或降采样后相同的编码）返回同一个 id，不增加引用计数。
    /// 调用方在创建 [`crate::Image`] 时再 [`Self::retain`]。
    ///
    /// `max_edge` 超过这个边长会先降采样。传 [`MAX_IMAGE_EDGE`] 即是产品行为。
    ///
    /// # 错误
    /// 格式不支持、解码失败，或图像为空 / 超过像素上限。
    pub fn insert_encoded(
        &mut self,
        bytes: &[u8],
        mime: &str,
        max_edge: u32,
    ) -> Result<String, AssetError> {
        let input_key = content_hash(bytes);
        if let Some(id) = self.by_input.get(&input_key) {
            if self.assets.contains_key(id) {
                return Ok(id.clone());
            }
        }

        let decoded = decode_image(bytes, mime)?;
        let stored = store_payload(decoded, bytes, mime, max_edge)?;
        let id = format!("img_{}", content_hash(&stored.encoded));
        self.by_input.insert(input_key, id.clone());
        if self.assets.contains_key(&id) {
            return Ok(id);
        }
        self.assets.insert(
            id.clone(),
            ImageAsset {
                id: id.clone(),
                mime: stored.mime,
                width: stored.width,
                height: stored.height,
                rgba: stored.rgba,
                encoded: stored.encoded,
                refs: 0,
            },
        );
        Ok(id)
    }

    /// 元素开始引用这个 id。id 为空或不在表里时什么都不做。
    pub fn retain(&mut self, id: &str) {
        if let Some(asset) = self.assets.get_mut(id) {
            asset.refs = asset.refs.saturating_add(1);
        }
    }

    /// 元素不再引用这个 id。
    ///
    /// 引用计数降到 0 时把资源移出表并返回，调用方可以把它暂存到撤销栈。
    /// 仍有引用、或 id 不存在时返回 `None`。
    pub fn release(&mut self, id: &str) -> Option<ImageAsset> {
        if id.is_empty() {
            return None;
        }
        let refs = self.assets.get(id)?.refs;
        if refs == 0 {
            return self.take_zero(id);
        }
        let next = refs - 1;
        if next > 0 {
            self.assets.get_mut(id)?.refs = next;
            return None;
        }
        self.take_zero(id)
    }

    fn take_zero(&mut self, id: &str) -> Option<ImageAsset> {
        let asset = self.assets.remove(id)?;
        self.by_input.retain(|_, mapped| mapped != id);
        Some(asset)
    }

    /// 把撤销栈里暂存的资源放回表中，引用计数置 0。
    ///
    /// 表里已经有同 id 时丢掉这份副本（像素是一样的）。
    pub fn reinsert(&mut self, mut asset: ImageAsset) {
        if self.assets.contains_key(&asset.id) {
            return;
        }
        asset.refs = 0;
        self.assets.insert(asset.id.clone(), asset);
    }

    /// 按元素上的 `asset_id` 重算引用计数，并丢掉没有元素引用的资源。
    pub fn assign_references<I, S>(&mut self, ids: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for asset in self.assets.values_mut() {
            asset.refs = 0;
        }
        for id in ids {
            self.retain(id.as_ref());
        }
        self.assets.retain(|_, asset| asset.refs > 0);
        let live: HashSet<&str> = self.assets.keys().map(String::as_str).collect();
        self.by_input.retain(|_, id| live.contains(id.as_str()));
    }

    /// 清空。
    pub fn clear(&mut self) {
        self.assets.clear();
        self.by_input.clear();
    }

    /// 引用计数大于 0 的资源，按 id 排序，写成 JSON 记录。
    pub fn to_records(&self) -> Vec<AssetRecord> {
        let mut records: Vec<AssetRecord> = self
            .assets
            .values()
            .filter(|asset| asset.refs > 0)
            .map(ImageAsset::to_record)
            .collect();
        records.sort_by(|left, right| left.id.cmp(&right.id));
        records
    }

    /// 从 JSON 记录解码像素。引用计数先置 0，由 [`Self::assign_references`] 填上。
    ///
    /// # 错误
    /// id 重复，或 data URL 解不开。
    pub fn from_records(records: Vec<AssetRecord>) -> Result<Self, AssetError> {
        let mut table = Self::new();
        for record in records {
            if record.id.is_empty() {
                return Err(AssetError::Decode(
                    "image asset is missing an id".to_string(),
                ));
            }
            if table.assets.contains_key(&record.id) {
                return Err(AssetError::DuplicateId(record.id));
            }
            let (mime, encoded) = parse_data_url(&record.data)?;
            let mime = if record.mime.trim().is_empty() {
                mime
            } else {
                record.mime
            };
            let decoded = decode_image(&encoded, &mime)?;
            table.assets.insert(
                record.id.clone(),
                ImageAsset {
                    id: record.id,
                    mime: canonical_mime(&decoded_mime(&mime, &encoded)),
                    width: decoded.width,
                    height: decoded.height,
                    rgba: decoded.rgba,
                    encoded,
                    refs: 0,
                },
            );
        }
        Ok(table)
    }
}

/// 把宽高收进 `max_edge`，另一边按比例四舍五入，至少为 1。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::asset::fitted_size;
///
/// assert_eq!(fitted_size(5000, 2500, 4096), (4096, 2048));
/// assert_eq!(fitted_size(32, 16, 4096), (32, 16));
/// ```
pub fn fitted_size(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (width, height);
    }
    let max_edge = max_edge.max(1);
    let longest = width.max(height);
    if longest <= max_edge {
        return (width, height);
    }
    let scale = f64::from(max_edge) / f64::from(longest);
    let fit = |value: u32| {
        let scaled = (f64::from(value) * scale).round();
        let clamped = scaled.clamp(1.0, f64::from(max_edge));
        clamped as u32
    };
    (fit(width), fit(height))
}

/// 区域平均，把 RGBA8 缩小到 `dw` × `dh`。
pub fn downsample_rgba(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    let mut dst = vec![0u8; (dw as usize) * (dh as usize) * 4];
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return dst;
    }
    for y in 0..dh {
        let y0 = (y * sh) / dh;
        let y1 = (((y + 1) * sh) / dh).max(y0 + 1).min(sh);
        for x in 0..dw {
            let x0 = (x * sw) / dw;
            let x1 = (((x + 1) * sw) / dw).max(x0 + 1).min(sw);
            let mut acc = [0u32; 4];
            let mut count = 0u32;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let index = ((sy * sw + sx) * 4) as usize;
                    if index + 3 >= src.len() {
                        continue;
                    }
                    for channel in 0..4 {
                        acc[channel] += u32::from(src[index + channel]);
                    }
                    count += 1;
                }
            }
            let offset = ((y * dw + x) * 4) as usize;
            let count = count.max(1);
            for channel in 0..4 {
                dst[offset + channel] = (acc[channel] / count) as u8;
            }
        }
    }
    dst
}

/// 把 RGBA8 编码成 PNG。测试和降采样后的持久化都走这里。
pub fn encode_png_rgba(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, String> {
    let expected = (width as usize) * (height as usize) * 4;
    if pixels.len() != expected {
        return Err(format!(
            "rgba length {expected} expected, got {}",
            pixels.len()
        ));
    }
    let mut buffer = Vec::new();
    let mut encoder = png::Encoder::new(&mut buffer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|err| err.to_string())?;
    writer
        .write_image_data(pixels)
        .map_err(|err| err.to_string())?;
    writer.finish().map_err(|err| err.to_string())?;
    Ok(buffer)
}

struct StoredPayload {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    encoded: Vec<u8>,
    mime: String,
}

fn store_payload(
    decoded: Decoded,
    original: &[u8],
    mime: &str,
    max_edge: u32,
) -> Result<StoredPayload, AssetError> {
    let (width, height) = fitted_size(decoded.width, decoded.height, max_edge);
    if width == 0 || height == 0 {
        return Err(AssetError::EmptyImage);
    }
    if width == decoded.width && height == decoded.height {
        return Ok(StoredPayload {
            rgba: decoded.rgba,
            width,
            height,
            encoded: original.to_vec(),
            mime: canonical_mime(&decoded_mime(mime, original)),
        });
    }
    let rgba = downsample_rgba(&decoded.rgba, decoded.width, decoded.height, width, height);
    let encoded = encode_png_rgba(width, height, &rgba).map_err(AssetError::Decode)?;
    Ok(StoredPayload {
        rgba,
        width,
        height,
        encoded,
        mime: "image/png".to_string(),
    })
}

fn decode_image(bytes: &[u8], mime: &str) -> Result<Decoded, AssetError> {
    match sniff(bytes, mime)? {
        ImageKind::Png => decode_png(bytes),
        ImageKind::Jpeg => decode_jpeg(bytes),
    }
}

enum ImageKind {
    Png,
    Jpeg,
}

fn sniff(bytes: &[u8], mime: &str) -> Result<ImageKind, AssetError> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Ok(ImageKind::Png);
    }
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Ok(ImageKind::Jpeg);
    }
    match normalize_mime(mime) {
        Some(kind) => Ok(kind),
        None => Err(AssetError::UnsupportedFormat(if mime.is_empty() {
            "unknown".to_string()
        } else {
            mime.to_string()
        })),
    }
}

fn normalize_mime(mime: &str) -> Option<ImageKind> {
    let mime = mime
        .split(';')
        .next()
        .unwrap_or(mime)
        .trim()
        .to_ascii_lowercase();
    match mime.as_str() {
        "image/png" | "png" => Some(ImageKind::Png),
        "image/jpeg" | "image/jpg" | "image/pjpeg" | "jpeg" | "jpg" => Some(ImageKind::Jpeg),
        _ => None,
    }
}

fn decoded_mime(mime: &str, bytes: &[u8]) -> String {
    match sniff(bytes, mime) {
        Ok(ImageKind::Png) => "image/png".to_string(),
        Ok(ImageKind::Jpeg) => "image/jpeg".to_string(),
        Err(_) => "application/octet-stream".to_string(),
    }
}

fn canonical_mime(mime: &str) -> String {
    match normalize_mime(mime) {
        Some(ImageKind::Png) => "image/png".to_string(),
        Some(ImageKind::Jpeg) => "image/jpeg".to_string(),
        None => {
            if mime.trim().is_empty() {
                "application/octet-stream".to_string()
            } else {
                mime.trim().to_string()
            }
        }
    }
}

fn content_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(HEX[(byte >> 4) as usize] as char);
        hex.push(HEX[(byte & 0x0f) as usize] as char);
    }
    hex
}

fn check_pixels(width: u32, height: u32) -> Result<(), AssetError> {
    if width == 0 || height == 0 {
        return Err(AssetError::EmptyImage);
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > MAX_SOURCE_PIXELS {
        return Err(AssetError::Decode(format!(
            "image is {width}x{height}, above the {MAX_SOURCE_PIXELS} pixel limit"
        )));
    }
    Ok(())
}

fn decode_png(bytes: &[u8]) -> Result<Decoded, AssetError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|err| AssetError::Decode(err.to_string()))?;
    let (width, height) = {
        let info = reader.info();
        (info.width, info.height)
    };
    check_pixels(width, height)?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let output = reader
        .next_frame(&mut buf)
        .map_err(|err| AssetError::Decode(err.to_string()))?;
    let size = output.buffer_size();
    buf.truncate(size);
    let rgba = expand_to_rgba(&buf, width, height, output.color_type)?;
    Ok(Decoded {
        width,
        height,
        rgba,
    })
}

fn decode_jpeg(bytes: &[u8]) -> Result<Decoded, AssetError> {
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    let pixels = decoder
        .decode()
        .map_err(|err| AssetError::Decode(err.to_string()))?;
    let info = decoder
        .info()
        .ok_or_else(|| AssetError::Decode("jpeg is missing a header".to_string()))?;
    let width = u32::from(info.width);
    let height = u32::from(info.height);
    check_pixels(width, height)?;
    let rgba = match info.pixel_format {
        jpeg_decoder::PixelFormat::L8 => expand_gray(&pixels, width, height)?,
        jpeg_decoder::PixelFormat::RGB24 => expand_rgb(&pixels, width, height)?,
        jpeg_decoder::PixelFormat::L16 => {
            expand_gray(&l16_to_l8(&pixels, width, height)?, width, height)?
        }
        jpeg_decoder::PixelFormat::CMYK32 => {
            return Err(AssetError::Decode("cmyk jpeg is not supported".to_string()));
        }
    };
    Ok(Decoded {
        width,
        height,
        rgba,
    })
}

fn expand_to_rgba(
    pixels: &[u8],
    width: u32,
    height: u32,
    color: png::ColorType,
) -> Result<Vec<u8>, AssetError> {
    match color {
        png::ColorType::Rgba => {
            let expected = (width as usize) * (height as usize) * 4;
            if pixels.len() < expected {
                return Err(AssetError::Decode("png rgba buffer is short".to_string()));
            }
            Ok(pixels[..expected].to_vec())
        }
        png::ColorType::Rgb => expand_rgb(pixels, width, height),
        png::ColorType::Grayscale => expand_gray(pixels, width, height),
        png::ColorType::GrayscaleAlpha => {
            let count = (width as usize) * (height as usize);
            if pixels.len() < count * 2 {
                return Err(AssetError::Decode(
                    "png gray+alpha buffer is short".to_string(),
                ));
            }
            let mut rgba = Vec::with_capacity(count * 4);
            for pixel in pixels[..count * 2].as_chunks::<2>().0 {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
            Ok(rgba)
        }
        png::ColorType::Indexed => Err(AssetError::Decode(
            "png palette was not expanded".to_string(),
        )),
    }
}

fn expand_rgb(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, AssetError> {
    let count = (width as usize) * (height as usize);
    if pixels.len() < count * 3 {
        return Err(AssetError::Decode("rgb buffer is short".to_string()));
    }
    let mut rgba = Vec::with_capacity(count * 4);
    for pixel in pixels[..count * 3].as_chunks::<3>().0 {
        rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
    }
    Ok(rgba)
}

fn l16_to_l8(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, AssetError> {
    let count = (width as usize) * (height as usize);
    if pixels.len() < count * 2 {
        return Err(AssetError::Decode(
            "16-bit jpeg buffer is short".to_string(),
        ));
    }
    let mut gray = Vec::with_capacity(count);
    for chunk in pixels[..count * 2].as_chunks::<2>().0 {
        let value = u16::from_be_bytes([chunk[0], chunk[1]]);
        gray.push((value >> 8) as u8);
    }
    Ok(gray)
}

fn expand_gray(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, AssetError> {
    let count = (width as usize) * (height as usize);
    if pixels.len() < count {
        return Err(AssetError::Decode("gray buffer is short".to_string()));
    }
    let mut rgba = Vec::with_capacity(count * 4);
    for gray in &pixels[..count] {
        rgba.extend_from_slice(&[*gray, *gray, *gray, 255]);
    }
    Ok(rgba)
}

fn parse_data_url(value: &str) -> Result<(String, Vec<u8>), AssetError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AssetError::Decode("image asset data is empty".to_string()));
    }
    if let Some(rest) = value.strip_prefix("data:") {
        let (meta, data) = rest
            .split_once(',')
            .ok_or_else(|| AssetError::Decode("image data url is missing a payload".to_string()))?;
        let mime = meta
            .split(';')
            .next()
            .unwrap_or("application/octet-stream")
            .trim()
            .to_string();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .map_err(|err| AssetError::Decode(err.to_string()))?;
        return Ok((mime, bytes));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|err| AssetError::Decode(err.to_string()))?;
    Ok(("application/octet-stream".to_string(), bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{AddElementCommand, CommandHistory, RemoveElementCommand};
    use crate::element::Image;
    use crate::math::Vec2;
    use crate::{AnyElement, Scene};

    fn solid_png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..width * height {
            pixels.extend_from_slice(&rgba);
        }
        encode_png_rgba(width, height, &pixels).unwrap()
    }

    fn patterned_png(width: u32, height: u32) -> Vec<u8> {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let i = ((y * width + x) * 4) as usize;
                let red = x < width / 2;
                pixels[i] = if red { 255 } else { 0 };
                pixels[i + 2] = if red { 0 } else { 255 };
                pixels[i + 3] = 255;
            }
        }
        encode_png_rgba(width, height, &pixels).unwrap()
    }

    #[test]
    fn fitted_size_shrinks_the_long_edge() {
        assert_eq!(fitted_size(5000, 2500, 4096), (4096, 2048));
        assert_eq!(fitted_size(100, 50, 4096), (100, 50));
        assert_eq!(fitted_size(1, 1, 1), (1, 1));
    }

    #[test]
    fn downsample_averages_blocks() {
        let src = [
            0, 0, 0, 255, 100, 0, 0, 255, //
            0, 0, 0, 255, 100, 0, 0, 255,
        ];
        let dst = downsample_rgba(&src, 2, 2, 1, 1);
        assert_eq!(dst.len(), 4);
        assert_eq!(dst[0], 50);
        assert_eq!(dst[3], 255);
    }

    #[test]
    fn asset_table_dedupes_and_releases_the_last_reference() {
        let png = solid_png(2, 2, [10, 20, 30, 255]);
        let mut scene = Scene::new();
        let first = scene.insert_image_bytes(&png, "image/png").unwrap();
        let second = scene.insert_image_bytes(&png, "image/png").unwrap();
        assert_eq!(first, second);
        assert_eq!(scene.image_asset_count(), 1);
        assert_eq!(scene.asset_refcount(&first), 0);

        let a = scene.next_id();
        scene.add_image(Image::from_asset(a, Vec2::ZERO, &first, 2, 2));
        let b = scene.next_id();
        scene.add_image(Image::from_asset(b, Vec2::new(10.0, 0.0), &first, 2, 2));
        assert_eq!(scene.asset_refcount(&first), 2);
        assert_eq!(scene.image_asset(&first).unwrap().rgba.len(), 16);

        scene.remove(a);
        assert_eq!(scene.asset_refcount(&first), 1);
        assert!(scene.take_detached_assets().is_empty());
        assert!(scene.image_asset(&first).is_some());

        scene.remove(b);
        assert!(scene.image_asset(&first).is_none());
        let freed = scene.take_detached_assets();
        assert_eq!(freed.len(), 1);
        assert_eq!(freed[0].id, first);
        assert_eq!(&freed[0].rgba[0..4], &[10, 20, 30, 255]);
        assert!(scene.take_detached_assets().is_empty());
    }

    #[test]
    fn large_edge_is_downsampled_before_storage() {
        let png = patterned_png(8, 4);
        let mut table = AssetTable::new();
        let id = table.insert_encoded(&png, "image/png", 4).unwrap();
        let asset = table.get(&id).unwrap();
        assert_eq!((asset.width, asset.height), (4, 2));
        assert_eq!(asset.mime, "image/png");
        assert_eq!(asset.rgba.len(), 4 * 2 * 4);
        assert_eq!(&asset.rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&asset.rgba[12..16], &[0, 0, 255, 255]);
        assert!(asset.encoded.starts_with(b"\x89PNG"));
        assert!(asset.encoded.len() < png.len() || asset.width < 8);
    }

    #[test]
    fn jpeg_bytes_round_trip_through_the_asset_table() {
        let mut rgb = Vec::new();
        for _ in 0..4 {
            rgb.extend_from_slice(&[255, 0, 0]);
            rgb.extend_from_slice(&[0, 0, 255]);
        }
        let mut encoded = Vec::new();
        jpeg_encoder::Encoder::new(&mut encoded, 90)
            .encode(&rgb, 4, 2, jpeg_encoder::ColorType::Rgb)
            .unwrap();
        assert!(encoded.starts_with(&[0xFF, 0xD8, 0xFF]));

        let mut table = AssetTable::new();
        let id = table.insert_encoded(&encoded, "image/jpeg", 4096).unwrap();
        let asset = table.get(&id).unwrap();
        assert_eq!(asset.mime, "image/jpeg");
        assert_eq!((asset.width, asset.height), (4, 2));
        assert!(asset.rgba[0] > 200, "left pixel stays red-ish");
        assert!(asset.rgba[2] < 40);
        assert_eq!(asset.rgba[3], 255);
        let again = table.insert_encoded(&encoded, "", 4096).unwrap();
        assert_eq!(again, id);
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn undo_add_drops_the_asset_and_redo_restores_pixels() {
        let png = solid_png(2, 1, [1, 2, 3, 255]);
        let mut scene = Scene::new();
        let asset_id = scene.insert_image_bytes(&png, "image/png").unwrap();
        let id = scene.next_id();
        let image = Image::from_asset(id, Vec2::new(4.0, 5.0), &asset_id, 2, 1);
        let mut history = CommandHistory::new();
        history.execute(
            Box::new(AddElementCommand::new(AnyElement::Image(image))),
            &mut scene,
        );
        assert_eq!(scene.asset_refcount(&asset_id), 1);
        assert!(scene
            .get(id)
            .unwrap()
            .as_element()
            .contains_point(Vec2::new(4.5, 5.5)));

        assert!(history.undo(&mut scene));
        assert!(scene.get(id).is_none());
        assert!(scene.image_asset(&asset_id).is_none());

        assert!(history.redo(&mut scene));
        let restored = scene.image_asset(&asset_id).unwrap();
        assert_eq!(restored.refs(), 1);
        assert_eq!(&restored.rgba[..], &[1, 2, 3, 255, 1, 2, 3, 255]);
    }

    #[test]
    fn remove_command_parks_the_asset_until_undo() {
        let png = solid_png(1, 1, [9, 8, 7, 255]);
        let mut scene = Scene::new();
        let asset_id = scene.insert_image_bytes(&png, "image/png").unwrap();
        let id = scene.next_id();
        scene.add_image(Image::from_asset(id, Vec2::ZERO, &asset_id, 1, 1));
        let mut history = CommandHistory::new();
        history.execute(Box::new(RemoveElementCommand::new(id)), &mut scene);
        assert!(scene.image_asset(&asset_id).is_none());
        history.undo(&mut scene);
        assert_eq!(
            scene.image_asset(&asset_id).unwrap().rgba,
            vec![9, 8, 7, 255]
        );
        assert!(scene.get(id).is_some());
    }

    #[test]
    fn removing_a_group_releases_child_images_and_keeps_shared_pixels() {
        let png = solid_png(1, 1, [4, 5, 6, 255]);
        let mut scene = Scene::new();
        let asset_id = scene.insert_image_bytes(&png, "image/png").unwrap();
        let inside = scene.next_id();
        scene.add_image(Image::from_asset(inside, Vec2::ZERO, &asset_id, 1, 1));
        let also_inside = scene.next_id();
        scene.add_image(Image::from_asset(
            also_inside,
            Vec2::new(10.0, 0.0),
            &asset_id,
            1,
            1,
        ));
        let outside = scene.next_id();
        scene.add_image(Image::from_asset(
            outside,
            Vec2::new(30.0, 0.0),
            &asset_id,
            1,
            1,
        ));
        let group_id = scene.group(&[inside, also_inside]).unwrap();
        assert_eq!(scene.asset_refcount(&asset_id), 3);

        scene.remove(group_id);
        assert!(!scene.contains(inside));
        assert!(!scene.contains(also_inside));
        assert!(scene.contains(outside));
        assert_eq!(scene.asset_refcount(&asset_id), 1);
        assert!(scene.image_asset(&asset_id).is_some());
        assert!(scene.take_detached_assets().is_empty());

        scene.remove(outside);
        assert!(scene.image_asset(&asset_id).is_none());
        assert_eq!(scene.take_detached_assets().len(), 1);
    }

    #[test]
    fn unknown_bytes_are_rejected() {
        let mut table = AssetTable::new();
        let err = table.insert_encoded(b"not an image", "text/plain", 64);
        assert!(matches!(err, Err(AssetError::UnsupportedFormat(_))));
    }
}
