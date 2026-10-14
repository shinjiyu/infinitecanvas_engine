//! 直线元素（可选箭头）
//!
//! 两个端点之间生成带线宽的三角形带。命中测试使用点到线段距离。

use super::stroke::{
    arrow_length_for, arrow_points, arrow_width_for, bounds_containing, effective_width,
    paint_color, point_in_triangle, point_to_segment_distance, shorten_end, stroke_triangles,
    transform_vertices, HIT_SLOP, MITER_LIMIT,
};
use super::{Element, ElementId, ElementProps, ElementStyle};
use infinitecanvas_math::{Color, Rect, Transform, Vec2, Vertex};
use serde::{Deserialize, Serialize};

/// 直线元素
///
/// 端点存放在局部坐标。拖动通过 `ElementProps::transform` 平移，
/// 因此撤销移动与其他元素共用同一条命令路径。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::element::{ElementStyle, Line};
/// use infinitecanvas_math::{Color, Vec2};
///
/// let style = ElementStyle::stroke_only(Color::BLACK, 4.0);
/// let line = Line::from_world(1, Vec2::ZERO, Vec2::new(100.0, 0.0), style, true);
/// assert!(line.arrow());
/// assert_eq!(line.generate_vertices().len(), 9);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    /// 元素属性
    pub props: ElementProps,
    /// 起点（局部坐标）
    pub start: Vec2,
    /// 终点（局部坐标）
    pub end: Vec2,
    /// 样式（描边色和线宽）。JSON 中可省略。
    #[serde(default)]
    pub style: ElementStyle,
    /// 是否在终点绘制箭头。JSON 中缺省为 false。
    #[serde(default)]
    pub arrow: bool,
}

impl Line {
    /// 从世界坐标端点创建直线。
    ///
    /// 变换原点放在起点，局部起点为原点，局部终点为 `end - start`。
    ///
    /// # 参数
    /// - `id`: 元素 ID
    /// - `start`: 世界坐标起点
    /// - `end`: 世界坐标终点
    /// - `style`: 描边样式
    /// - `arrow`: 终点是否带箭头
    pub fn from_world(
        id: ElementId,
        start: Vec2,
        end: Vec2,
        style: ElementStyle,
        arrow: bool,
    ) -> Self {
        let mut props = ElementProps::new(id);
        props.transform.translation = start;
        props.z_index = id.min(i32::MAX as u64) as i32;
        Self {
            props,
            start: Vec2::ZERO,
            end: end - start,
            style,
            arrow,
        }
    }

    /// 设置样式
    pub fn with_style(mut self, style: ElementStyle) -> Self {
        self.style = style;
        self
    }

    /// 终点是否带箭头
    pub fn arrow(&self) -> bool {
        self.arrow
    }

    /// 打开或关闭终点箭头
    pub fn set_arrow(&mut self, arrow: bool) {
        self.arrow = arrow;
    }

    /// 有效线宽
    pub fn stroke_width(&self) -> f32 {
        effective_width(self.style.stroke_width)
    }

    /// 用世界坐标更新终点。起点和变换保持不变。
    pub fn set_world_end(&mut self, world_end: Vec2) {
        self.end = self.props.transform.inverse_transform_point(world_end);
    }

    /// 世界坐标起点
    pub fn world_start(&self) -> Vec2 {
        self.props.transform.transform_point(self.start)
    }

    /// 世界坐标终点
    pub fn world_end(&self) -> Vec2 {
        self.props.transform.transform_point(self.end)
    }

    /// 世界坐标处的导出颜色
    ///
    /// 点在半线宽以内，或落在箭头三角形内时返回描边色。
    /// 不包含点选余量，供 PNG 导出使用。不可见或完全透明时返回 `None`。
    ///
    /// # 参数
    /// - `world`: 世界坐标
    ///
    /// # 返回
    /// 该点的描边颜色；点在笔画外时为 `None`
    pub fn color_at(&self, world: Vec2) -> Option<Color> {
        if !self.props.visible {
            return None;
        }
        let local = try_local(&self.props.transform, world)?;
        let half = self.stroke_width() * 0.5;
        let on_shaft = point_to_segment_distance(local, self.start, self.end) <= half;
        let on_arrow = self
            .arrow_triangle_local()
            .is_some_and(|tri| point_in_triangle(local, tri));
        if !on_shaft && !on_arrow {
            return None;
        }
        opaque_color(paint_color(&self.style))
    }

    /// 生成描边三角形（含可选箭头），坐标已变换到世界空间。
    ///
    /// 直线本身是一条三角形带；箭头是额外的一个三角形。
    pub fn generate_vertices(&self) -> Vec<Vertex> {
        let color = paint_color(&self.style);
        let width = self.stroke_width();
        let mut shaft = [self.start, self.end];
        let mut arrow = Vec::new();
        if self.arrow {
            let delta = self.end - self.start;
            if delta.length_squared() > 1e-12 {
                let length = arrow_length_for(width);
                if let Some(tri) = arrow_points(self.end, delta, length, arrow_width_for(width)) {
                    shorten_end(&mut shaft, length);
                    for point in tri {
                        arrow.push(Vertex::new(point.x, point.y, color));
                    }
                }
            }
        }
        let mut vertices = stroke_triangles(&shaft, width, color);
        vertices.extend(arrow);
        transform_vertices(vertices, &self.props.transform)
    }

    fn hit_radius(&self) -> f32 {
        self.stroke_width() * 0.5 + HIT_SLOP
    }

    fn arrow_triangle_local(&self) -> Option<[Vec2; 3]> {
        if !self.arrow {
            return None;
        }
        let delta = self.end - self.start;
        arrow_points(
            self.end,
            delta,
            arrow_length_for(self.stroke_width()),
            arrow_width_for(self.stroke_width()),
        )
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

fn opaque_color(color: Color) -> Option<Color> {
    if color.a <= 0.0 {
        None
    } else {
        Some(color)
    }
}

impl Element for Line {
    fn props(&self) -> &ElementProps {
        &self.props
    }

    fn props_mut(&mut self) -> &mut ElementProps {
        &mut self.props
    }

    fn local_bounds(&self) -> Rect {
        let pad = self.stroke_width() * 0.5 * MITER_LIMIT;
        bounds_containing(&[self.start, self.end], pad)
    }

    fn type_name(&self) -> &'static str {
        "Line"
    }

    fn contains_point(&self, point: Vec2) -> bool {
        let local = self.props.transform.inverse_transform_point(point);
        if point_to_segment_distance(local, self.start, self.end) <= self.hit_radius() {
            return true;
        }
        self.arrow_triangle_local()
            .is_some_and(|tri| point_in_triangle(local, tri))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{AddElementCommand, CommandHistory};
    use crate::scene::{AnyElement, Scene};
    use crate::Element;
    use infinitecanvas_math::Color;

    #[test]
    fn bounds_cover_stroke() {
        let line = Line::from_world(
            1,
            Vec2::new(10.0, 20.0),
            Vec2::new(110.0, 20.0),
            ElementStyle::stroke_only(Color::BLACK, 10.0),
            false,
        );
        let bounds = line.local_bounds();
        assert!(bounds.width >= 100.0);
        assert!(bounds.height >= 10.0);
        let world = line.world_bounds();
        assert!((world.center().x - 60.0).abs() < bounds.height);
    }

    #[test]
    fn hit_test_uses_segment_distance() {
        let line = Line::from_world(
            1,
            Vec2::ZERO,
            Vec2::new(100.0, 0.0),
            ElementStyle::stroke_only(Color::BLACK, 4.0),
            false,
        );
        assert!(line.contains_point(Vec2::new(50.0, 0.0)));
        assert!(line.contains_point(Vec2::new(50.0, 4.0)));
        assert!(!line.contains_point(Vec2::new(50.0, 40.0)));
        // 包围盒角落不在线段上
        assert!(!line.contains_point(Vec2::new(100.0, 30.0)));
    }

    #[test]
    fn arrow_tip_is_selectable_beyond_shaft_radius() {
        let line = Line::from_world(
            1,
            Vec2::ZERO,
            Vec2::new(100.0, 0.0),
            ElementStyle::stroke_only(Color::BLACK, 4.0),
            true,
        );
        // 箭翼在线段半宽 + slop 之外，仍应命中
        assert!(line.contains_point(Vec2::new(84.0, 5.5)));
        assert!(!line.contains_point(Vec2::new(120.0, 0.0)));
    }

    #[test]
    fn vertices_follow_translation() {
        let mut line = Line::from_world(
            1,
            Vec2::new(10.0, 20.0),
            Vec2::new(110.0, 20.0),
            ElementStyle::stroke_only(Color::RED, 10.0),
            false,
        );
        let vertices = line.generate_vertices();
        assert_eq!(vertices.len(), 6);
        assert!(vertices.iter().all(|vertex| {
            (10.0..=110.0).contains(&vertex.position[0])
                && (vertex.position[1] - 20.0).abs() <= 5.01
        }));

        line.props_mut().transform.translate(Vec2::new(0.0, 30.0));
        assert!(!line.contains_point(Vec2::new(60.0, 20.0)));
        assert!(line.contains_point(Vec2::new(60.0, 50.0)));
    }

    #[test]
    fn arrow_adds_a_triangle() {
        let line = Line::from_world(
            1,
            Vec2::ZERO,
            Vec2::new(80.0, 0.0),
            ElementStyle::stroke_only(Color::BLACK, 4.0),
            true,
        );
        // 箭杆四边形 6 顶点 + 箭头 3 顶点
        assert_eq!(line.generate_vertices().len(), 9);
    }

    #[test]
    fn add_command_undo_redo() {
        let mut scene = Scene::new();
        let mut history = CommandHistory::new();
        let line = Line::from_world(
            1,
            Vec2::ZERO,
            Vec2::new(40.0, 0.0),
            ElementStyle::stroke_only(Color::BLACK, 4.0),
            false,
        );
        history.execute(
            Box::new(AddElementCommand::new(AnyElement::Line(line))),
            &mut scene,
        );
        assert_eq!(scene.find_at(Vec2::new(20.0, 0.0)), Some(1));
        assert!(history.undo(&mut scene));
        assert!(scene.get(1).is_none());
        assert!(history.redo(&mut scene));
        assert_eq!(scene.find_at(Vec2::new(20.0, 1.0)), Some(1));
        assert_eq!(scene.find_at(Vec2::new(20.0, 80.0)), None);
    }
}
