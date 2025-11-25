///! 矩形元素

use super::{Element, ElementId, ElementProps, ElementStyle};
use infinitecanvas_math::{Rect, Vec2};
use serde::{Deserialize, Serialize};

/// 矩形元素
///
/// # 属性
/// - 位置、尺寸（局部坐标系）
/// - 填充和描边样式
/// - 变换（通过 ElementProps）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rectangle {
    /// 元素属性
    pub props: ElementProps,
    /// 尺寸
    pub size: Vec2,
    /// 样式
    pub style: ElementStyle,
}

impl Rectangle {
    /// 创建矩形
    ///
    /// # 参数
    /// - `id`: 元素 ID
    /// - `position`: 位置（中心点）
    /// - `size`: 尺寸
    pub fn new(id: ElementId, position: Vec2, size: Vec2) -> Self {
        let mut props = ElementProps::new(id);
        props.transform.translation = position;

        Self {
            props,
            size,
            style: ElementStyle::default(),
        }
    }

    /// 设置样式
    pub fn with_style(mut self, style: ElementStyle) -> Self {
        self.style = style;
        self
    }

    /// 获取中心点（局部坐标系）
    pub fn center(&self) -> Vec2 {
        Vec2::ZERO
    }

    /// 获取尺寸
    pub fn size(&self) -> Vec2 {
        self.size
    }
}

impl Element for Rectangle {
    fn props(&self) -> &ElementProps {
        &self.props
    }

    fn props_mut(&mut self) -> &mut ElementProps {
        &mut self.props
    }

    fn local_bounds(&self) -> Rect {
        Rect::from_center_size(Vec2::ZERO, self.size)
    }

    fn type_name(&self) -> &'static str {
        "Rectangle"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rectangle_bounds() {
        let rect = Rectangle::new(1, Vec2::new(100.0, 100.0), Vec2::new(50.0, 30.0));

        let bounds = rect.local_bounds();
        assert_eq!(bounds.width, 50.0);
        assert_eq!(bounds.height, 30.0);
        assert_eq!(bounds.center(), Vec2::ZERO);
    }

    #[test]
    fn test_rectangle_world_bounds() {
        let rect = Rectangle::new(1, Vec2::new(100.0, 100.0), Vec2::new(50.0, 30.0));

        let world_bounds = rect.world_bounds();
        assert_eq!(world_bounds.center(), Vec2::new(100.0, 100.0));
    }

    #[test]
    fn test_contains_point() {
        let rect = Rectangle::new(1, Vec2::new(100.0, 100.0), Vec2::new(50.0, 30.0));

        // 中心点
        assert!(rect.contains_point(Vec2::new(100.0, 100.0)));

        // 内部点
        assert!(rect.contains_point(Vec2::new(110.0, 105.0)));

        // 外部点
        assert!(!rect.contains_point(Vec2::new(200.0, 200.0)));
    }
}
