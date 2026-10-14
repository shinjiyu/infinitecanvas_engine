/// Image 元素 - 用于在画布上显示图片/纹理
///
/// 支持：
/// - 位置和尺寸控制
/// - 旋转和缩放变换
/// - UV 裁剪（显示纹理的一部分）
/// - 透明度（通过 alpha）
use infinitecanvas_math::{Color, Rect, Transform, Vec2, Vertex};
use serde::{Deserialize, Serialize};

use crate::element::shadow::quad_shadow;
use crate::element::{DropShadow, Element, ElementId, ElementProps};

fn default_image_alpha() -> f32 {
    1.0
}

/// Image 元素
///
/// `asset_id` 指向场景资源表里的一份像素。`intrinsic_width` / `intrinsic_height`
/// 是这份像素的原始尺寸（降采样之后）。`size` 是画布上的显示尺寸，和变换分开存。
/// 没有资源、只引用已经上传的 GPU 纹理时，`asset_id` 为空，`texture_id` 仍是采样键。
///
/// # 示例
///
/// ```
/// use infinitecanvas_engine::element::Image;
/// use infinitecanvas_math::{Rect, Vec2};
///
/// // 创建一个 256x256 的图片元素
/// let img = Image::new(
///     1,
///     Vec2::new(100.0, 100.0),
///     Vec2::new(256.0, 256.0),
///     "logo".to_string(),
/// );
///
/// // 使用 UV 裁剪显示纹理的左上角 1/4
/// let cropped = Image::with_uv_crop(
///     2,
///     Vec2::new(200.0, 100.0),
///     Vec2::new(128.0, 128.0),
///     "sprite_sheet".to_string(),
///     Rect::new(0.0, 0.0, 0.5, 0.5),
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Image {
    /// 元素通用属性（ID, transform, visible, z_index）
    pub props: ElementProps,
    /// 图片显示尺寸（像素）
    pub size: Vec2,
    /// 纹理 ID（对应 TextureManager 中的纹理）
    ///
    /// 资源图片里它和 [`Self::asset_id`] 相同。JSON 可省略，导入时会从 `asset_id` 补上。
    #[serde(default)]
    pub texture_id: String,
    /// 场景资源 ID。空字符串表示像素不在资源表里。
    #[serde(default)]
    pub asset_id: String,
    /// 资源像素宽。没有资源时为 0。
    #[serde(default)]
    pub intrinsic_width: u32,
    /// 资源像素高。没有资源时为 0。
    #[serde(default)]
    pub intrinsic_height: u32,
    /// UV 裁剪区域（可选）
    ///
    /// 坐标范围 [0, 1]，表示纹理的归一化坐标：
    /// - (0.0, 0.0) = 左上角
    /// - (1.0, 1.0) = 右下角
    ///
    /// 如果为 None，使用完整纹理 (0,0)-(1,1)。JSON 中可省略。
    #[serde(default)]
    pub uv_rect: Option<Rect>,
    /// 透明度 [0.0 - 1.0]
    ///
    /// - 0.0 = 完全透明
    /// - 1.0 = 完全不透明
    ///
    /// JSON 中缺省为 1。
    #[serde(default = "default_image_alpha")]
    pub alpha: f32,
    /// 投影。缺省关闭，关闭时不写入 JSON。命中测试不读它。
    ///
    /// 按显示矩形画偏移羽化，不模糊纹理像素。
    #[serde(default, skip_serializing_if = "DropShadow::is_unset")]
    pub shadow: DropShadow,
}

impl Image {
    /// 创建新的 Image 元素
    ///
    /// # 参数
    ///
    /// - `id`: 元素唯一 ID
    /// - `position`: 左上角位置（像素坐标）
    /// - `size`: 显示尺寸（width, height）
    /// - `texture_id`: 纹理 ID（必须在 TextureManager 中存在）
    pub fn new(id: ElementId, position: Vec2, size: Vec2, texture_id: String) -> Self {
        let mut props = ElementProps::new(id);
        props.transform = Transform::from_translation(position);
        Self {
            props,
            size,
            texture_id,
            asset_id: String::new(),
            intrinsic_width: 0,
            intrinsic_height: 0,
            uv_rect: None,
            alpha: 1.0,
            shadow: DropShadow::default(),
        }
    }

    /// 用场景资源创建图片。
    ///
    /// 显示尺寸默认等于像素宽高（1 像素 = 1 世界单位）。
    /// `texture_id` 与 `asset_id` 相同，纹理管线按这个 id 采样。
    ///
    /// # 参数
    /// - `id`: 元素 ID
    /// - `position`: 左上角（世界坐标）
    /// - `asset_id`: [`crate::asset::AssetTable`] 里的资源 ID
    /// - `intrinsic_width`, `intrinsic_height`: 资源像素尺寸
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::element::Image;
    /// use infinitecanvas_math::Vec2;
    ///
    /// let image = Image::from_asset(1, Vec2::ZERO, "img_abc", 32, 16);
    /// assert_eq!(image.asset_id, "img_abc");
    /// assert_eq!(image.size.x, 32.0);
    /// assert_eq!(image.intrinsic_height, 16);
    /// ```
    pub fn from_asset(
        id: ElementId,
        position: Vec2,
        asset_id: &str,
        intrinsic_width: u32,
        intrinsic_height: u32,
    ) -> Self {
        let mut image = Self::new(
            id,
            position,
            Vec2::new(
                intrinsic_width.max(1) as f32,
                intrinsic_height.max(1) as f32,
            ),
            asset_id.to_string(),
        );
        image.asset_id = asset_id.to_string();
        image.intrinsic_width = intrinsic_width;
        image.intrinsic_height = intrinsic_height;
        image
    }

    /// 创建带 UV 裁剪的 Image 元素
    ///
    /// # 参数
    ///
    /// - `uv_rect`: UV 坐标裁剪区域
    ///   - x, y: 左上角（归一化坐标 0-1）
    ///   - width, height: 尺寸（归一化坐标 0-1）
    ///
    /// # 示例
    ///
    /// ```
    /// use infinitecanvas_engine::element::Image;
    /// use infinitecanvas_math::{Rect, Vec2};
    ///
    /// // 显示纹理的右下角 1/4
    /// let rect = Rect::new(0.5, 0.5, 0.5, 0.5);
    /// let pos = Vec2::new(0.0, 0.0);
    /// let size = Vec2::new(64.0, 64.0);
    /// let img = Image::with_uv_crop(1, pos, size, "tex".to_string(), rect);
    /// assert!(img.uv_rect.is_some());
    /// ```
    pub fn with_uv_crop(
        id: ElementId,
        position: Vec2,
        size: Vec2,
        texture_id: String,
        uv_rect: Rect,
    ) -> Self {
        let mut props = ElementProps::new(id);
        props.transform = Transform::from_translation(position);
        Self {
            props,
            size,
            texture_id,
            asset_id: String::new(),
            intrinsic_width: 0,
            intrinsic_height: 0,
            uv_rect: Some(uv_rect),
            alpha: 1.0,
            shadow: DropShadow::default(),
        }
    }

    /// 导入后把采样键对齐到资源 id。
    ///
    /// 旧文件只有 `texture_id`、没有 `asset_id`，这里不会改它。
    pub fn finish_load(&mut self) {
        if !self.asset_id.is_empty() {
            self.texture_id = self.asset_id.clone();
        }
        self.shadow = self.shadow.sanitized();
    }

    /// 在世界坐标采样资源像素。
    ///
    /// 先逆变换到显示矩形，再按 UV 裁剪映射到像素，使用最近邻。
    /// `pixels` 是 RGBA8，长度应为 `width * height * 4`。完全透明的像素返回 `None`。
    ///
    /// # 参数
    /// - `world`: 世界坐标
    /// - `width`, `height`: 像素尺寸
    /// - `pixels`: RGBA8
    pub fn sample_pixels(
        &self,
        world: Vec2,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Option<Color> {
        if !self.props.visible || width == 0 || height == 0 || !self.contains_point(world) {
            return None;
        }
        if self.size.x.abs() < 1e-6 || self.size.y.abs() < 1e-6 {
            return None;
        }
        let local = self.props.transform.inverse_transform_point(world);
        let mut u = local.x / self.size.x;
        let mut v = local.y / self.size.y;
        if let Some(rect) = self.uv_rect {
            u = rect.x + u * rect.width;
            v = rect.y + v * rect.height;
        }
        if !u.is_finite() || !v.is_finite() {
            return None;
        }
        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);
        let x = (u * width as f32).floor() as u32;
        let y = (v * height as f32).floor() as u32;
        let x = x.min(width - 1);
        let y = y.min(height - 1);
        let index = ((y * width + x) * 4) as usize;
        if index + 3 >= pixels.len() {
            return None;
        }
        let alpha = (pixels[index + 3] as f32 / 255.0) * self.alpha.clamp(0.0, 1.0);
        if alpha <= 0.0 {
            return None;
        }
        Some(Color::new(
            pixels[index] as f32 / 255.0,
            pixels[index + 1] as f32 / 255.0,
            pixels[index + 2] as f32 / 255.0,
            alpha,
        ))
    }

    /// 设置透明度
    ///
    /// # 参数
    ///
    /// - `alpha`: 透明度 [0.0 - 1.0]
    pub fn set_alpha(&mut self, alpha: f32) {
        self.alpha = alpha.clamp(0.0, 1.0);
    }

    /// 设置 UV 裁剪区域
    pub fn set_uv_rect(&mut self, uv_rect: Option<Rect>) {
        self.uv_rect = uv_rect;
    }

    /// 元素 ID
    pub fn id(&self) -> ElementId {
        self.props.id
    }

    /// 元素当前是否可见
    pub fn is_visible(&self) -> bool {
        self.props.visible
    }

    /// 设置元素可见性
    pub fn set_visible(&mut self, visible: bool) {
        self.props.visible = visible;
    }

    /// 元素层级
    pub fn z_index(&self) -> i32 {
        self.props.z_index
    }

    /// 设置元素层级
    pub fn set_z_index(&mut self, z_index: i32) {
        self.props.z_index = z_index;
    }

    /// 获取 UV 坐标（用于顶点生成）
    ///
    /// # 返回
    ///
    /// `((uv_min_x, uv_min_y), (uv_max_x, uv_max_y))`
    pub fn get_uv_coords(&self) -> ((f32, f32), (f32, f32)) {
        match &self.uv_rect {
            Some(rect) => (
                (rect.x, rect.y),
                (rect.x + rect.width, rect.y + rect.height),
            ),
            None => ((0.0, 0.0), (1.0, 1.0)),
        }
    }

    /// 计算世界空间中的边界框
    ///
    /// # 返回
    ///
    /// 包含此图片的最小轴对齐矩形，已计入平移、旋转和缩放。
    pub fn bounding_box(&self) -> Rect {
        self.world_bounds()
    }

    /// 显示矩形的投影。关闭或不可见时为空。命中仍是显示矩形。
    pub(crate) fn shadow_vertices(&self) -> Vec<Vertex> {
        if !self.props.visible {
            return Vec::new();
        }
        quad_shadow(
            Vec2::ZERO,
            self.size,
            self.shadow,
            self.alpha,
            &self.props.transform,
        )
    }

    /// 生成纹理顶点（6个顶点 = 2个三角形）
    ///
    /// # 返回
    ///
    /// 顶点数组，包含位置、颜色（用于 tint）和 UV 坐标
    ///
    /// # 布局
    ///
    /// ```text
    /// v0 ---- v1
    /// |     / |
    /// |   /   |
    /// | /     |
    /// v2 ---- v3
    ///
    /// Triangle 1: v0, v2, v1
    /// Triangle 2: v1, v2, v3
    /// ```
    pub fn generate_vertices(&self) -> Vec<Vertex> {
        let (uv_min, uv_max) = self.get_uv_coords();
        let color = Color::new(1.0, 1.0, 1.0, self.alpha);
        let transform = &self.props.transform;
        let local = [
            (Vec2::ZERO, uv_min.0, uv_min.1),
            (Vec2::new(self.size.x, 0.0), uv_max.0, uv_min.1),
            (Vec2::new(0.0, self.size.y), uv_min.0, uv_max.1),
            (Vec2::new(self.size.x, self.size.y), uv_max.0, uv_max.1),
        ];
        let world: Vec<(Vec2, f32, f32)> = local
            .iter()
            .map(|(point, u, v)| (transform.transform_point(*point), *u, *v))
            .collect();
        let vertex = |index: usize| {
            Vertex::with_uv(
                world[index].0.x,
                world[index].0.y,
                color,
                world[index].1,
                world[index].2,
            )
        };

        // 无旋转且缩放为 1 时，顶点仍落在以平移为左上角的矩形上。
        vec![
            vertex(0),
            vertex(2),
            vertex(1),
            vertex(1),
            vertex(2),
            vertex(3),
        ]
    }
}

impl Element for Image {
    fn props(&self) -> &ElementProps {
        &self.props
    }

    fn props_mut(&mut self) -> &mut ElementProps {
        &mut self.props
    }

    fn local_bounds(&self) -> Rect {
        // Image 的局部边界是从 (0, 0) 开始的矩形
        Rect::new(0.0, 0.0, self.size.x, self.size.y)
    }

    fn type_name(&self) -> &'static str {
        "Image"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_creation() {
        let img = Image::new(
            1,
            Vec2::new(10.0, 20.0),
            Vec2::new(100.0, 100.0),
            "test".to_string(),
        );
        assert_eq!(img.id(), 1);
        assert_eq!(img.size.x, 100.0);
        assert_eq!(img.size.y, 100.0);
        assert_eq!(img.texture_id, "test");
        assert_eq!(img.alpha, 1.0);
        assert!(img.uv_rect.is_none());
    }

    #[test]
    fn test_uv_coords_default() {
        let img = Image::new(1, Vec2::ZERO, Vec2::new(100.0, 100.0), "test".to_string());
        let (min, max) = img.get_uv_coords();
        assert_eq!(min, (0.0, 0.0));
        assert_eq!(max, (1.0, 1.0));
    }

    #[test]
    fn test_uv_coords_cropped() {
        let uv_rect = Rect::new(0.25, 0.25, 0.5, 0.5);
        let img = Image::with_uv_crop(
            1,
            Vec2::ZERO,
            Vec2::new(100.0, 100.0),
            "test".to_string(),
            uv_rect,
        );
        let (min, max) = img.get_uv_coords();
        assert_eq!(min, (0.25, 0.25));
        assert_eq!(max, (0.75, 0.75));
    }

    #[test]
    fn test_alpha_clamping() {
        let mut img = Image::new(1, Vec2::ZERO, Vec2::new(100.0, 100.0), "test".to_string());

        img.set_alpha(1.5);
        assert_eq!(img.alpha, 1.0);

        img.set_alpha(-0.5);
        assert_eq!(img.alpha, 0.0);

        img.set_alpha(0.5);
        assert_eq!(img.alpha, 0.5);
    }

    #[test]
    fn test_bounding_box() {
        let img = Image::new(
            1,
            Vec2::new(10.0, 20.0),
            Vec2::new(100.0, 50.0),
            "test".to_string(),
        );
        let bounds = img.bounding_box();

        assert_eq!(bounds.x, 10.0);
        assert_eq!(bounds.y, 20.0);
        assert_eq!(bounds.width, 100.0);
        assert_eq!(bounds.height, 50.0);
    }

    #[test]
    fn test_generate_vertices() {
        let img = Image::new(
            1,
            Vec2::new(100.0, 100.0),
            Vec2::new(200.0, 100.0),
            "test".to_string(),
        );
        let vertices = img.generate_vertices();

        // 应该生成 6 个顶点（2 个三角形）
        assert_eq!(vertices.len(), 6);

        // 检查第一个顶点（左上角）
        assert_eq!(vertices[0].position, [100.0, 100.0]);
        assert_eq!(vertices[0].uv, [0.0, 0.0]);

        // 检查右下角顶点
        assert_eq!(vertices[5].position, [300.0, 200.0]);
        assert_eq!(vertices[5].uv, [1.0, 1.0]);
    }

    #[test]
    fn test_generate_vertices_with_uv_crop() {
        let uv_rect = Rect::new(0.25, 0.25, 0.5, 0.5);
        let img = Image::with_uv_crop(
            1,
            Vec2::ZERO,
            Vec2::new(100.0, 100.0),
            "test".to_string(),
            uv_rect,
        );
        let vertices = img.generate_vertices();

        // UV 坐标应该被裁剪
        assert_eq!(vertices[0].uv, [0.25, 0.25]); // 左上
        assert_eq!(vertices[5].uv, [0.75, 0.75]); // 右下
    }

    #[test]
    fn test_generate_vertices_alpha() {
        let mut img = Image::new(1, Vec2::ZERO, Vec2::new(100.0, 100.0), "test".to_string());
        img.set_alpha(0.5);

        let vertices = img.generate_vertices();

        // 所有顶点的 alpha 应该是 0.5
        for v in &vertices {
            assert_eq!(v.color[3], 0.5);
        }
    }

    #[test]
    fn rotated_image_vertices_and_hit_test_follow_transform() {
        let mut img = Image::new(
            1,
            Vec2::new(100.0, 100.0),
            Vec2::new(200.0, 100.0),
            "test".to_string(),
        );
        img.set_rotation(std::f32::consts::FRAC_PI_2);
        let vertices = img.generate_vertices();
        assert!(vertices.iter().any(|vertex| {
            (vertex.position[0] - 100.0).abs() < 1e-3 && (vertex.position[1] - 300.0).abs() < 1e-3
        }));
        // 局部中心 (100, 50) 旋转 90° 后落到 (50, 200)，仍在图片内。
        // 旋转前会命中的 (150, 100) 转到局部 y 为负，不再命中。
        assert!(img.contains_point(Vec2::new(50.0, 200.0)));
        assert!(!img.contains_point(Vec2::new(150.0, 100.0)));
    }

    #[test]
    fn hit_test_uses_display_size_not_intrinsic_pixels() {
        let image = Image::from_asset(7, Vec2::new(10.0, 20.0), "img_a", 4, 2);
        assert_eq!(image.intrinsic_width, 4);
        assert_eq!(image.bounding_box().width, 4.0);
        assert!(image.contains_point(Vec2::new(10.5, 20.5)));
        assert!(image.contains_point(Vec2::new(13.9, 21.9)));
        assert!(!image.contains_point(Vec2::new(14.1, 20.5)));
        assert!(!image.contains_point(Vec2::new(9.0, 20.5)));

        let mut wide = image.clone();
        wide.size = Vec2::new(40.0, 10.0);
        assert!(wide.contains_point(Vec2::new(49.0, 29.0)));
        // 落在原来的 4×2 像素框外面，但还在放大后的显示框里。
        assert!(wide.contains_point(Vec2::new(30.0, 25.0)));
        assert!(!wide.contains_point(Vec2::new(60.0, 21.0)));
        let bounds = wide.bounding_box();
        assert_eq!(bounds.width, 40.0);
        assert_eq!(bounds.height, 10.0);
    }

    #[test]
    fn test_visibility() {
        let mut img = Image::new(1, Vec2::ZERO, Vec2::new(100.0, 100.0), "test".to_string());

        assert!(img.is_visible());

        img.set_visible(false);
        assert!(!img.is_visible());
    }

    #[test]
    fn test_z_index() {
        let mut img = Image::new(1, Vec2::ZERO, Vec2::new(100.0, 100.0), "test".to_string());

        assert_eq!(img.z_index(), 0);

        img.set_z_index(10);
        assert_eq!(img.z_index(), 10);
    }
}
