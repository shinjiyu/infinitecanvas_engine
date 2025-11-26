/// Image 元素 - 用于在画布上显示图片/纹理
/// 
/// 支持：
/// - 位置和尺寸控制
/// - 旋转和缩放变换
/// - UV 裁剪（显示纹理的一部分）
/// - 透明度（通过 alpha）
use infinitecanvas_math::{Rect, Transform, Vec2};

use crate::element::{Element, ElementId, ElementProps};

/// Image 元素
/// 
/// # 示例
/// 
/// ```
/// use infinitecanvas_engine::element::Image;
/// use infinitecanvas_math::Vec2;
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
#[derive(Debug, Clone)]
pub struct Image {
    /// 元素通用属性（ID, transform, visible, z_index）
    pub props: ElementProps,
    /// 图片显示尺寸（像素）
    pub size: Vec2,
    /// 纹理 ID（对应 TextureManager 中的纹理）
    pub texture_id: String,
    /// UV 裁剪区域（可选）
    /// 
    /// 坐标范围 [0, 1]，表示纹理的归一化坐标：
    /// - (0.0, 0.0) = 左上角
    /// - (1.0, 1.0) = 右下角
    /// 
    /// 如果为 None，使用完整纹理 (0,0)-(1,1)
    pub uv_rect: Option<Rect>,
    /// 透明度 [0.0 - 1.0]
    /// 
    /// - 0.0 = 完全透明
    /// - 1.0 = 完全不透明
    pub alpha: f32,
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
        Self {
            props: ElementProps {
                id,
                transform: Transform::from_translation(position),
                visible: true,
                selected: false,
                z_index: 0,
            },
            size,
            texture_id,
            uv_rect: None,
            alpha: 1.0,
        }
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
    /// // 显示纹理的右下角 1/4
    /// let rect = Rect::new(0.5, 0.5, 0.5, 0.5);
    /// let img = Image::with_uv_crop(1, pos, size, "tex".to_string(), rect);
    /// ```
    pub fn with_uv_crop(
        id: ElementId,
        position: Vec2,
        size: Vec2,
        texture_id: String,
        uv_rect: Rect,
    ) -> Self {
        Self {
            props: ElementProps {
                id,
                transform: Transform::from_translation(position),
                visible: true,
                selected: false,
                z_index: 0,
            },
            size,
            texture_id,
            uv_rect: Some(uv_rect),
            alpha: 1.0,
        }
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
    /// 包含此图片的最小矩形（考虑变换）
    pub fn bounding_box(&self) -> Rect {
        let pos = self.props.transform.translation;
        let scale = self.props.transform.scale;
        let size = Vec2::new(self.size.x * scale.x, self.size.y * scale.y);

        // TODO: 处理旋转的情况（需要计算旋转后的 AABB）
        Rect::new(pos.x, pos.y, size.x, size.y)
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
        let img = Image::new(1, Vec2::new(10.0, 20.0), Vec2::new(100.0, 100.0), "test".to_string());
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
        let img = Image::new(1, Vec2::new(10.0, 20.0), Vec2::new(100.0, 50.0), "test".to_string());
        let bounds = img.bounding_box();
        
        assert_eq!(bounds.x, 10.0);
        assert_eq!(bounds.y, 20.0);
        assert_eq!(bounds.width, 100.0);
        assert_eq!(bounds.height, 50.0);
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

