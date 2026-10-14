//! 自由画笔路径
//!
//! 沿采样点生成带线宽的三角形带，拐角按斜接/斜角处理。
//! 命中测试使用点到各线段的距离，而不是包围盒。

use super::stroke::{
    bounds_containing, distance_to_polyline, effective_width, paint_color, stroke_styled,
    transform_vertices, StrokePaint, HIT_SLOP, MITER_LIMIT,
};
use super::{Element, ElementId, ElementProps, ElementStyle};
use infinitecanvas_math::{Color, Rect, Transform, Vec2, Vertex};
use serde::{Deserialize, Serialize};

/// 自由绘制路径
///
/// 采样点存放在局部坐标。`from_world_points` 把变换原点放在第一个点，
/// 之后拖动只平移变换，路径形状保持不变。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::element::{Element, ElementStyle, Path};
/// use infinitecanvas_math::{Color, Vec2};
///
/// let style = ElementStyle::stroke_only(Color::BLACK, 4.0);
/// let path = Path::from_world_points(
///     1,
///     &[Vec2::ZERO, Vec2::new(30.0, 0.0), Vec2::new(30.0, 40.0)],
///     style,
/// );
/// assert!(path.contains_point(Vec2::new(30.0, 20.0)));
/// assert!(!path.contains_point(Vec2::new(0.0, 40.0)));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Path {
    /// 元素属性
    pub props: ElementProps,
    /// 采样点（局部坐标）。JSON 中缺省为空路径。
    #[serde(default)]
    pub points: Vec<Vec2>,
    /// 样式（描边色和线宽）。JSON 中可省略。
    #[serde(default)]
    pub style: ElementStyle,
}

impl Path {
    /// 从世界坐标采样点创建路径。
    ///
    /// # 参数
    /// - `id`: 元素 ID
    /// - `world_points`: 世界坐标采样点，按绘制顺序
    /// - `style`: 描边样式
    ///
    /// # 返回
    /// 变换原点在第一个点上的路径。空输入得到空路径。
    pub fn from_world_points(id: ElementId, world_points: &[Vec2], style: ElementStyle) -> Self {
        let origin = world_points.first().copied().unwrap_or(Vec2::ZERO);
        let mut props = ElementProps::new(id);
        props.transform.translation = origin;
        props.z_index = id.min(i32::MAX as u64) as i32;
        let points = world_points.iter().map(|point| *point - origin).collect();
        Self {
            props,
            points,
            style,
        }
    }

    /// 设置样式
    pub fn with_style(mut self, style: ElementStyle) -> Self {
        self.style = style;
        self
    }

    /// 有效线宽
    pub fn stroke_width(&self) -> f32 {
        effective_width(self.style.stroke_width)
    }

    /// 用世界坐标替换采样点。变换保持不变，点会变回局部坐标。
    pub fn set_world_points(&mut self, world_points: &[Vec2]) {
        self.points = world_points
            .iter()
            .map(|point| self.props.transform.inverse_transform_point(*point))
            .collect();
    }

    /// 局部采样点
    pub fn points(&self) -> &[Vec2] {
        &self.points
    }

    /// 世界坐标处的导出颜色
    ///
    /// 点到折线的距离不超过半线宽时返回描边色。不含点选余量。
    /// 不可见、空路径或完全透明时返回 `None`。
    ///
    /// # 参数
    /// - `world`: 世界坐标
    ///
    /// # 返回
    /// 该点的描边颜色；点在笔画外时为 `None`
    pub fn color_at(&self, world: Vec2) -> Option<Color> {
        if !self.props.visible || self.points.is_empty() {
            return None;
        }
        let local = try_local(&self.props.transform, world)?;
        if distance_to_polyline(local, &self.points) > self.stroke_width() * 0.5 {
            return None;
        }
        let color = paint_color(&self.style);
        if color.a <= 0.0 {
            None
        } else {
            Some(color)
        }
    }

    /// 生成描边三角形带（展开为三角形列表），坐标在世界空间。
    pub fn generate_vertices(&self) -> Vec<Vertex> {
        let vertices = stroke_styled(
            &self.points,
            StrokePaint {
                width: self.stroke_width(),
                color: paint_color(&self.style),
                dash: &self.style.stroke_dash,
                cap: self.style.stroke_cap,
                join: self.style.stroke_join,
            },
        );
        transform_vertices(vertices, &self.props.transform)
    }

    fn hit_radius(&self) -> f32 {
        self.stroke_width() * 0.5 + HIT_SLOP
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

impl Element for Path {
    fn props(&self) -> &ElementProps {
        &self.props
    }

    fn props_mut(&mut self) -> &mut ElementProps {
        &mut self.props
    }

    fn local_bounds(&self) -> Rect {
        let pad = self.stroke_width() * 0.5 * MITER_LIMIT;
        bounds_containing(&self.points, pad)
    }

    fn type_name(&self) -> &'static str {
        "Path"
    }

    fn contains_point(&self, point: Vec2) -> bool {
        if self.points.is_empty() {
            return false;
        }
        let local = self.props.transform.inverse_transform_point(point);
        distance_to_polyline(local, &self.points) <= self.hit_radius()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{AddElementCommand, CommandHistory};
    use crate::scene::{AnyElement, Scene};
    use crate::Element;
    use infinitecanvas_math::Color;

    fn sample() -> Path {
        Path::from_world_points(
            7,
            &[
                Vec2::new(0.0, 0.0),
                Vec2::new(50.0, 50.0),
                Vec2::new(100.0, 0.0),
            ],
            ElementStyle::stroke_only(Color::BLACK, 4.0),
        )
    }

    #[test]
    fn hit_test_follows_segments_not_bbox() {
        let path = sample();
        assert!(path.contains_point(Vec2::new(25.0, 25.0)));
        assert!(path.contains_point(Vec2::new(75.0, 25.0)));
        // V 形包围盒内部、但远离两段折线
        assert!(!path.contains_point(Vec2::new(50.0, 0.0)));
        assert!(!path.contains_point(Vec2::new(200.0, 200.0)));
    }

    #[test]
    fn corner_mesh_has_join_vertices() {
        let path = sample();
        let vertices = path.generate_vertices();
        // 三个点的斜接折线：两个四边形
        assert_eq!(vertices.len(), 12);
        assert!(vertices.iter().all(|vertex| vertex.position[0].is_finite()));
    }

    #[test]
    fn drag_translates_hit_region() {
        let mut path = sample();
        path.props_mut().transform.translate(Vec2::new(10.0, -5.0));
        assert!(!path.contains_point(Vec2::new(25.0, 25.0)));
        assert!(path.contains_point(Vec2::new(35.0, 20.0)));
    }

    #[test]
    fn set_world_points_keeps_origin() {
        let mut path = Path::from_world_points(
            1,
            &[Vec2::new(10.0, 10.0), Vec2::new(20.0, 10.0)],
            ElementStyle::stroke_only(Color::BLACK, 2.0),
        );
        path.set_world_points(&[
            Vec2::new(10.0, 10.0),
            Vec2::new(20.0, 10.0),
            Vec2::new(20.0, 30.0),
        ]);
        assert_eq!(path.points().len(), 3);
        assert!(path.contains_point(Vec2::new(20.0, 20.0)));
        assert!((path.props().transform.translation - Vec2::new(10.0, 10.0)).length() < 1e-4);
    }

    #[test]
    fn undo_removes_path() {
        let mut scene = Scene::new();
        let mut history = CommandHistory::new();
        let path = sample();
        history.execute(
            Box::new(AddElementCommand::new(AnyElement::Path(path))),
            &mut scene,
        );
        assert_eq!(scene.count(), 1);
        history.undo(&mut scene);
        assert!(scene.is_empty());
        history.redo(&mut scene);
        assert_eq!(scene.find_at(Vec2::new(25.0, 25.0)), Some(7));
    }
}
