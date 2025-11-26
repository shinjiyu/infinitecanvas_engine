///! 圆形元素

use super::{Element, ElementId, ElementProps, ElementStyle};
use infinitecanvas_math::{Rect, Vec2, Vertex};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

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

    /// 生成填充顶点（三角扇形）
    ///
    /// # 算法
    /// - 中心点 + N 个圆周点形成 N 个三角形
    /// - segments: 圆周分段数（越多越圆滑）
    ///
    /// # 顶点数
    /// segments * 3 个顶点
    pub fn generate_fill_vertices(&self) -> Vec<Vertex> {
        self.generate_fill_vertices_with_segments(32)
    }

    /// 生成填充顶点（指定分段数）
    pub fn generate_fill_vertices_with_segments(&self, segments: usize) -> Vec<Vertex> {
        if self.style.fill.is_none() || segments < 3 {
            return Vec::new();
        }

        let color = self.style.fill.unwrap();
        let transform = &self.props.transform;
        
        // 中心点（局部坐标系）
        let center = Vec2::ZERO;
        let center_world = transform.transform_point(center);

        let mut vertices = Vec::new();
        let angle_step = 2.0 * PI / segments as f32;

        for i in 0..segments {
            let angle1 = i as f32 * angle_step;
            let angle2 = (i + 1) as f32 * angle_step;

            // 局部坐标
            let p1 = Vec2::new(angle1.cos() * self.radius, angle1.sin() * self.radius);
            let p2 = Vec2::new(angle2.cos() * self.radius, angle2.sin() * self.radius);

            // 世界坐标
            let p1_world = transform.transform_point(p1);
            let p2_world = transform.transform_point(p2);

            // 三角形: 中心 -> p1 -> p2
            vertices.push(Vertex::new(center_world.x, center_world.y, color));
            vertices.push(Vertex::new(p1_world.x, p1_world.y, color));
            vertices.push(Vertex::new(p2_world.x, p2_world.y, color));
        }

        vertices
    }

    /// 生成描边顶点（线条循环）
    ///
    /// # 顶点数
    /// segments * 2 个顶点（每条边2个点）
    pub fn generate_stroke_vertices(&self) -> Vec<Vertex> {
        self.generate_stroke_vertices_with_segments(32)
    }

    /// 生成描边顶点（指定分段数）
    pub fn generate_stroke_vertices_with_segments(&self, segments: usize) -> Vec<Vertex> {
        if self.style.stroke.is_none() || segments < 3 {
            return Vec::new();
        }

        let color = self.style.stroke.unwrap();
        let transform = &self.props.transform;

        let mut vertices = Vec::new();
        let angle_step = 2.0 * PI / segments as f32;

        for i in 0..segments {
            let angle1 = i as f32 * angle_step;
            let angle2 = ((i + 1) % segments) as f32 * angle_step;

            // 局部坐标
            let p1 = Vec2::new(angle1.cos() * self.radius, angle1.sin() * self.radius);
            let p2 = Vec2::new(angle2.cos() * self.radius, angle2.sin() * self.radius);

            // 世界坐标
            let p1_world = transform.transform_point(p1);
            let p2_world = transform.transform_point(p2);

            // 线段
            vertices.push(Vertex::new(p1_world.x, p1_world.y, color));
            vertices.push(Vertex::new(p2_world.x, p2_world.y, color));
        }

        vertices
    }

    /// 生成所有顶点（填充 + 描边）
    pub fn generate_vertices(&self) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        vertices.extend(self.generate_fill_vertices());
        vertices.extend(self.generate_stroke_vertices());
        vertices
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
    use infinitecanvas_math::Color;

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

    #[test]
    fn test_generate_fill_vertices() {
        let circle = Circle::new(1, Vec2::new(100.0, 100.0), 50.0)
            .with_style(ElementStyle::fill_only(Color::RED));

        let vertices = circle.generate_fill_vertices_with_segments(16);
        
        // 16 个三角形 = 48 个顶点
        assert_eq!(vertices.len(), 48);
        
        // 检查颜色
        for v in &vertices {
            assert_eq!(v.color, [1.0, 0.0, 0.0, 1.0]);
        }
    }

    #[test]
    fn test_generate_stroke_vertices() {
        let circle = Circle::new(1, Vec2::new(100.0, 100.0), 50.0)
            .with_style(ElementStyle::stroke_only(Color::BLUE, 2.0));

        let vertices = circle.generate_stroke_vertices_with_segments(16);
        
        // 16 条边 = 32 个顶点
        assert_eq!(vertices.len(), 32);
        
        // 检查颜色
        for v in &vertices {
            assert_eq!(v.color, [0.0, 0.0, 1.0, 1.0]);
        }
    }

    #[test]
    fn test_generate_vertices_both() {
        let mut style = ElementStyle::fill_only(Color::RED);
        style.stroke = Some(Color::BLUE);
        
        let circle = Circle::new(1, Vec2::new(100.0, 100.0), 50.0)
            .with_style(style);

        let vertices = circle.generate_vertices();
        
        // 32 个三角形 + 32 条边 = 96 + 64 = 160 个顶点
        assert_eq!(vertices.len(), 160);
    }
}
