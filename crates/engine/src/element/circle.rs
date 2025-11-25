///! 圆形元素

use super::{Element, ElementId, ElementProps, ElementStyle};
use infinitecanvas_math::{Rect, Vec2};
use serde::{Deserialize, Serialize};

/// 圆形元素
///
/// # 属性
/// - 位置（中心点）、半径
/// - 填充和描边样式
/// - 变换（通过 ElementProps）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Circle {
    /// 元素属性
    pub props: ElementProps,
    /// 半径
    pub radius: f32,
    /// 样式
    pub style: ElementStyle,
}

impl Circle {
    /// 创建圆形
    ///
    /// # 参数
    /// - `id`: 元素 ID
    /// - `center`: 中心点
    /// - `radius`: 半径
    pub fn new(id: ElementId, center: Vec2, radius: f32) -> Self {
        let mut props = ElementProps::new(id);
        props.transform.translation = center;

        Self {
            props,
            radius,
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

    /// 获取半径
    pub fn radius(&self) -> f32 {
        self.radius
    }
}

impl Element for Circle {
    fn props(&self) -> &ElementProps {
        &self.props
    }

    fn props_mut(&mut self) -> &mut ElementProps {
        &mut self.props
    }

    fn local_bounds(&self) -> Rect {
        Rect::from_center_size(Vec2::ZERO, Vec2::splat(self.radius * 2.0))
    }

    fn type_name(&self) -> &'static str {
        "Circle"
    }

    fn contains_point(&self, point: Vec2) -> bool {
        // 圆形使用精确的距离检测
        let local_point = self.props().transform.inverse_transform_point(point);
        local_point.length() <= self.radius
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circle_bounds() {
        let circle = Circle::new(1, Vec2::new(100.0, 100.0), 50.0);

        let bounds = circle.local_bounds();
        assert_eq!(bounds.width, 100.0);
        assert_eq!(bounds.height, 100.0);
        assert_eq!(bounds.center(), Vec2::ZERO);
    }

    #[test]
    fn test_circle_world_bounds() {
        let circle = Circle::new(1, Vec2::new(100.0, 100.0), 50.0);

        let world_bounds = circle.world_bounds();
        assert_eq!(world_bounds.center(), Vec2::new(100.0, 100.0));
        assert_eq!(world_bounds.width, 100.0);
    }

    #[test]
    fn test_contains_point() {
        let circle = Circle::new(1, Vec2::new(100.0, 100.0), 50.0);

        // 中心点
        assert!(circle.contains_point(Vec2::new(100.0, 100.0)));

        // 边界内
        assert!(circle.contains_point(Vec2::new(130.0, 100.0)));

        // 边界外
        assert!(!circle.contains_point(Vec2::new(200.0, 100.0)));
    }
}
