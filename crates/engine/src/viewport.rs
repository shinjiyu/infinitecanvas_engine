///! 视口管理器
///!
///! 管理无限画布的视口位置和缩放

use infinitecanvas_math::{Rect, Transform, Vec2};
use serde::{Deserialize, Serialize};

/// 视口管理器
///
/// # 职责
/// - 管理视口的位置、缩放
/// - 提供平移、缩放操作
/// - 坐标转换（屏幕空间 ↔ 世界空间）
/// - 视口边界计算
///
/// # 坐标系
/// - 屏幕空间：像素坐标，原点在左上角
/// - 世界空间：无限画布坐标，原点在 (0, 0)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    /// 视口中心在世界空间中的位置
    center: Vec2,
    /// 缩放倍数（1.0 = 100%）
    zoom: f32,
    /// 画布尺寸（像素）
    canvas_size: Vec2,
    /// 最小缩放
    min_zoom: f32,
    /// 最大缩放
    max_zoom: f32,
}

impl Viewport {
    /// 创建视口
    ///
    /// # 参数
    /// - `canvas_size`: Canvas 尺寸（像素）
    pub fn new(canvas_size: Vec2) -> Self {
        Self {
            center: Vec2::ZERO,
            zoom: 1.0,
            canvas_size,
            min_zoom: 0.1,
            max_zoom: 10.0,
        }
    }

    /// 获取视口中心
    pub fn center(&self) -> Vec2 {
        self.center
    }

    /// 获取缩放
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// 获取 Canvas 尺寸
    pub fn canvas_size(&self) -> Vec2 {
        self.canvas_size
    }

    /// 更新 Canvas 尺寸
    pub fn resize(&mut self, new_size: Vec2) {
        self.canvas_size = new_size;
    }

    /// 平移视口
    ///
    /// # 参数
    /// - `delta`: 在世界空间中的偏移量
    pub fn translate(&mut self, delta: Vec2) {
        self.center += delta;
    }

    /// 在指定点进行缩放
    ///
    /// # 参数
    /// - `anchor`: 缩放中心点（屏幕空间）
    /// - `delta_zoom`: 缩放增量（例如 1.1 放大 10%）
    pub fn zoom_at(&mut self, anchor: Vec2, delta_zoom: f32) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * delta_zoom).clamp(self.min_zoom, self.max_zoom);

        // 如果缩放没有实际变化，直接返回
        if (self.zoom - old_zoom).abs() < 1e-6 {
            return;
        }

        // 计算锚点在世界空间中的位置（缩放前）
        let world_anchor_before = self.screen_to_world(anchor);

        // 计算锚点在世界空间中的位置（缩放后）
        let world_anchor_after = self.screen_to_world(anchor);

        // 调整视口中心，使锚点在世界空间中的位置保持不变
        self.center += world_anchor_before - world_anchor_after;
    }

    /// 设置缩放
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(self.min_zoom, self.max_zoom);
    }

    /// 移动到指定位置
    pub fn move_to(&mut self, center: Vec2) {
        self.center = center;
    }

    /// 获取视口边界（世界空间）
    pub fn bounds(&self) -> Rect {
        let half_size = self.canvas_size / (2.0 * self.zoom);
        Rect::from_center_size(self.center, half_size * 2.0)
    }

    /// 屏幕空间 → 世界空间
    ///
    /// # 参数
    /// - `screen_pos`: 屏幕空间坐标
    ///
    /// # 返回
    /// 世界空间坐标
    pub fn screen_to_world(&self, screen_pos: Vec2) -> Vec2 {
        let offset = screen_pos - self.canvas_size / 2.0;
        self.center + offset / self.zoom
    }

    /// 世界空间 → 屏幕空间
    ///
    /// # 参数
    /// - `world_pos`: 世界空间坐标
    ///
    /// # 返回
    /// 屏幕空间坐标
    pub fn world_to_screen(&self, world_pos: Vec2) -> Vec2 {
        let offset = (world_pos - self.center) * self.zoom;
        offset + self.canvas_size / 2.0
    }

    /// 获取视口变换矩阵（世界 → 屏幕）
    pub fn transform(&self) -> Transform {
        let mut transform = Transform::identity();
        // 1. 平移到视口中心
        transform.translate(-self.center);
        // 2. 缩放
        transform.scale_by(Vec2::splat(self.zoom));
        // 3. 平移到屏幕中心
        transform.translate(self.canvas_size / 2.0);
        transform
    }

    /// 判断矩形是否在视口内（用于裁剪）
    pub fn is_visible(&self, rect: &Rect) -> bool {
        self.bounds().intersects(rect)
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new(Vec2::new(800.0, 600.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_to_world() {
        let viewport = Viewport::new(Vec2::new(800.0, 600.0));
        
        // 屏幕中心 → 世界原点
        let center = Vec2::new(400.0, 300.0);
        assert_eq!(viewport.screen_to_world(center), Vec2::ZERO);

        // 屏幕右下角
        let corner = Vec2::new(800.0, 600.0);
        assert_eq!(viewport.screen_to_world(corner), Vec2::new(400.0, 300.0));
    }

    #[test]
    fn test_world_to_screen() {
        let viewport = Viewport::new(Vec2::new(800.0, 600.0));

        // 世界原点 → 屏幕中心
        assert_eq!(viewport.world_to_screen(Vec2::ZERO), Vec2::new(400.0, 300.0));
    }

    #[test]
    fn test_zoom() {
        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));
        viewport.set_zoom(2.0);

        assert_eq!(viewport.zoom(), 2.0);

        // 缩放后，同样的屏幕偏移对应更小的世界偏移
        let screen_pos = Vec2::new(500.0, 400.0);
        let world_pos = viewport.screen_to_world(screen_pos);
        assert_eq!(world_pos, Vec2::new(50.0, 50.0));
    }

    #[test]
    fn test_translate() {
        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));
        viewport.translate(Vec2::new(100.0, 50.0));

        assert_eq!(viewport.center(), Vec2::new(100.0, 50.0));

        // 屏幕中心现在对应新的世界位置
        let center = Vec2::new(400.0, 300.0);
        assert_eq!(viewport.screen_to_world(center), Vec2::new(100.0, 50.0));
    }

    #[test]
    fn test_bounds() {
        let viewport = Viewport::new(Vec2::new(800.0, 600.0));
        let bounds = viewport.bounds();

        assert_eq!(bounds.center(), Vec2::ZERO);
        assert_eq!(bounds.width, 800.0);
        assert_eq!(bounds.height, 600.0);
    }

    #[test]
    fn test_is_visible() {
        let viewport = Viewport::new(Vec2::new(800.0, 600.0));

        // 在视口内
        let visible_rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert!(viewport.is_visible(&visible_rect));

        // 在视口外
        let invisible_rect = Rect::new(1000.0, 1000.0, 100.0, 100.0);
        assert!(!viewport.is_visible(&invisible_rect));
    }
}
