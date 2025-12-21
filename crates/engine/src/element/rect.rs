///! 矩形元素

use super::{Element, ElementId, ElementProps, ElementStyle};
use infinitecanvas_math::{Rect, Vec2, Vertex};
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

    /// 生成填充顶点（2个三角形 = 6个顶点）
    ///
    /// # 顶点顺序
    /// ```text
    ///  0 --- 2
    ///  | \   |
    ///  |  \  |
    ///  |   \ |
    ///  1 --- 3
    /// ```
    /// 三角形1: [0, 1, 2]
    /// 三角形2: [2, 1, 3]
    pub fn generate_fill_vertices(&self) -> Vec<Vertex> {
        if self.style.fill.is_none() {
            return Vec::new();
        }

        let color = self.style.fill.unwrap();
        let half_w = self.size.x / 2.0;
        let half_h = self.size.y / 2.0;

        // 局部坐标系（中心为原点）
        let positions = [
            [-half_w, -half_h], // 左上
            [-half_w, half_h],  // 左下
            [half_w, -half_h],  // 右上
            [half_w, half_h],   // 右下
        ];

        // 应用变换
        let transform = &self.props.transform;
        let world_positions: Vec<Vec2> = positions
            .iter()
            .map(|&[x, y]| transform.transform_point(Vec2::new(x, y)))
            .collect();

        // 生成顶点（顺时针）
        vec![
            // 三角形1: 左上、左下、右上
            Vertex::new(world_positions[0].x, world_positions[0].y, color),
            Vertex::new(world_positions[1].x, world_positions[1].y, color),
            Vertex::new(world_positions[2].x, world_positions[2].y, color),
            // 三角形2: 右上、左下、右下
            Vertex::new(world_positions[2].x, world_positions[2].y, color),
            Vertex::new(world_positions[1].x, world_positions[1].y, color),
            Vertex::new(world_positions[3].x, world_positions[3].y, color),
        ]
    }

    /// 生成描边顶点（4条线段 = 8个顶点）
    ///
    /// # 注意
    /// WebGPU 使用线条列表（line-list）渲染模式
    pub fn generate_stroke_vertices(&self) -> Vec<Vertex> {
        if self.style.stroke.is_none() {
            return Vec::new();
        }

        let color = self.style.stroke.unwrap();
        let half_w = self.size.x / 2.0;
        let half_h = self.size.y / 2.0;

        // 局部坐标系
        let corners = [
            Vec2::new(-half_w, -half_h), // 左上
            Vec2::new(half_w, -half_h),  // 右上
            Vec2::new(half_w, half_h),   // 右下
            Vec2::new(-half_w, half_h),  // 左下
        ];

        // 应用变换
        let transform = &self.props.transform;
        let world_corners: Vec<Vec2> = corners
            .iter()
            .map(|&pos| transform.transform_point(pos))
            .collect();

        // 生成线段（4条边）
        vec![
            // 上边
            Vertex::new(world_corners[0].x, world_corners[0].y, color),
            Vertex::new(world_corners[1].x, world_corners[1].y, color),
            // 右边
            Vertex::new(world_corners[1].x, world_corners[1].y, color),
            Vertex::new(world_corners[2].x, world_corners[2].y, color),
            // 下边
            Vertex::new(world_corners[2].x, world_corners[2].y, color),
            Vertex::new(world_corners[3].x, world_corners[3].y, color),
            // 左边
            Vertex::new(world_corners[3].x, world_corners[3].y, color),
            Vertex::new(world_corners[0].x, world_corners[0].y, color),
        ]
    }

    /// 生成所有顶点（填充 + 描边）
    pub fn generate_vertices(&self) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        vertices.extend(self.generate_fill_vertices());
        vertices.extend(self.generate_stroke_vertices());
        vertices
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
    use infinitecanvas_math::Color;

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

    #[test]
    fn test_generate_fill_vertices() {
        let rect = Rectangle::new(1, Vec2::new(100.0, 100.0), Vec2::new(50.0, 30.0))
            .with_style(ElementStyle::fill_only(Color::RED));

        let vertices = rect.generate_fill_vertices();
        
        // 2个三角形 = 6个顶点
        assert_eq!(vertices.len(), 6);
        
        // 检查颜色
        for v in &vertices {
            assert_eq!(v.color, [1.0, 0.0, 0.0, 1.0]);
        }
    }

    #[test]
    fn test_generate_stroke_vertices() {
        let rect = Rectangle::new(1, Vec2::new(100.0, 100.0), Vec2::new(50.0, 30.0))
            .with_style(ElementStyle::stroke_only(Color::BLUE, 2.0));

        let vertices = rect.generate_stroke_vertices();
        
        // 4条边 = 8个顶点
        assert_eq!(vertices.len(), 8);
        
        // 检查颜色
        for v in &vertices {
            assert_eq!(v.color, [0.0, 0.0, 1.0, 1.0]);
        }
    }

    #[test]
    fn test_generate_vertices_both() {
        let mut style = ElementStyle::fill_only(Color::RED);
        style.stroke = Some(Color::BLUE);
        
        let rect = Rectangle::new(1, Vec2::new(100.0, 100.0), Vec2::new(50.0, 30.0))
            .with_style(style);

        let vertices = rect.generate_vertices();
        
        // 填充6个 + 描边8个 = 14个顶点
        assert_eq!(vertices.len(), 14);
    }
}
