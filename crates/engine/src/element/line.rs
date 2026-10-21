//! 直线元素（可选起止箭头）
//!
//! 两个端点之间生成带线宽的三角形带。命中测试使用点到线段距离。

use super::blur::{blur_active, gaussian_capsule_vertices, gaussian_fill_vertices};
use super::shadow::{
    soft_inner_shadow_vertices, soft_shadow_vertices, stroke_capsule_inner_shadow,
    stroke_capsule_shadow,
};
use super::stroke::{
    arrow_points, bounds_containing, effective_width, marker_length_for, marker_width_for,
    paint_color, point_in_triangle, point_to_segment_distance, shorten_end, shorten_start,
    stroke_styled, transform_vertices, StrokePaint, HIT_SLOP, MITER_LIMIT,
};
use super::{Element, ElementId, ElementProps, ElementStyle, LineMarker};
use infinitecanvas_math::{Color, Rect, Transform, Vec2, Vertex};
use serde::{Deserialize, Serialize};

/// 直线元素
///
/// 端点存放在局部坐标。拖动通过 `ElementProps::transform` 平移，
/// 因此撤销移动与其他元素共用同一条命令路径。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::element::{ElementStyle, Line, LineMarker};
/// use infinitecanvas_math::{Color, Vec2};
///
/// let style = ElementStyle::stroke_only(Color::BLACK, 4.0);
/// let line = Line::from_world(1, Vec2::ZERO, Vec2::new(100.0, 0.0), style, true);
/// assert!(line.arrow());
/// assert_eq!(line.end_marker(), LineMarker::Arrow);
/// assert_eq!(line.generate_vertices().len(), 9);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "LineSerde", into = "LineSerde")]
pub struct Line {
    /// 元素属性
    pub props: ElementProps,
    /// 起点（局部坐标）
    pub start: Vec2,
    /// 终点（局部坐标）
    pub end: Vec2,
    /// 样式（描边色、线宽、虚线、线帽、线结）。JSON 中可省略。
    pub style: ElementStyle,
    /// 起点箭头。缺省 [`LineMarker::None`]。
    pub start_marker: LineMarker,
    /// 终点箭头。缺省 [`LineMarker::None`]。
    pub end_marker: LineMarker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LineSerde {
    props: ElementProps,
    start: Vec2,
    end: Vec2,
    #[serde(default)]
    style: ElementStyle,
    /// 旧文档的终点箭头开关。缺省 false。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    arrow: bool,
    #[serde(default, skip_serializing_if = "LineMarker::is_none")]
    start_marker: LineMarker,
    #[serde(default, skip_serializing_if = "LineMarker::is_none")]
    end_marker: LineMarker,
}

impl From<LineSerde> for Line {
    fn from(value: LineSerde) -> Self {
        let end_marker = if value.end_marker.is_some() {
            value.end_marker
        } else if value.arrow {
            LineMarker::Arrow
        } else {
            LineMarker::None
        };
        Self {
            props: value.props,
            start: value.start,
            end: value.end,
            style: value.style,
            start_marker: value.start_marker,
            end_marker,
        }
    }
}

impl From<Line> for LineSerde {
    fn from(value: Line) -> Self {
        Self {
            props: value.props,
            start: value.start,
            end: value.end,
            style: value.style,
            // 旧读者仍认 `arrow`；有任意终点标记时写成 true。
            arrow: value.end_marker.is_some(),
            start_marker: value.start_marker,
            end_marker: value.end_marker,
        }
    }
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
    /// - `arrow`: 终点是否带箭头（等价于 [`LineMarker::Arrow`]）
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
            start_marker: LineMarker::None,
            end_marker: if arrow {
                LineMarker::Arrow
            } else {
                LineMarker::None
            },
        }
    }

    /// 设置样式
    pub fn with_style(mut self, style: ElementStyle) -> Self {
        self.style = style;
        self
    }

    /// 设置起止箭头
    pub fn with_markers(mut self, start: LineMarker, end: LineMarker) -> Self {
        self.start_marker = start;
        self.end_marker = end;
        self
    }

    /// 终点是否带箭头（任意非 [`LineMarker::None`] 的终点标记）
    pub fn arrow(&self) -> bool {
        self.end_marker.is_some()
    }

    /// 打开或关闭终点箭头。打开时写成 [`LineMarker::Arrow`]。
    pub fn set_arrow(&mut self, arrow: bool) {
        self.end_marker = if arrow {
            LineMarker::Arrow
        } else {
            LineMarker::None
        };
    }

    /// 起点箭头
    pub fn start_marker(&self) -> LineMarker {
        self.start_marker
    }

    /// 终点箭头
    pub fn end_marker(&self) -> LineMarker {
        self.end_marker
    }

    /// 设置起点箭头
    pub fn set_start_marker(&mut self, marker: LineMarker) {
        self.start_marker = marker;
    }

    /// 设置终点箭头
    pub fn set_end_marker(&mut self, marker: LineMarker) {
        self.end_marker = marker;
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
    pub fn color_at(&self, world: Vec2) -> Option<Color> {
        if !self.props.visible {
            return None;
        }
        let local = try_local(&self.props.transform, world)?;
        let half = self.stroke_width() * 0.5;
        let on_shaft = point_to_segment_distance(local, self.start, self.end) <= half;
        let on_start = self
            .start_marker_triangle_local()
            .is_some_and(|tri| point_in_triangle(local, tri));
        let on_end = self
            .end_marker_triangle_local()
            .is_some_and(|tri| point_in_triangle(local, tri));
        if !on_shaft && !on_start && !on_end {
            return None;
        }
        opaque_color(paint_color(&self.style))
    }

    /// 局部点到笔画剪影的有符号距离。负值在笔画内。PNG 阴影用它，命中测试不用。
    pub(crate) fn silhouette_distance(&self, local: Vec2) -> f32 {
        let (start, end, start_arrow, end_arrow) = self.local_shaft_and_markers();
        let mut distance = point_to_segment_distance(local, start, end) - self.stroke_width() * 0.5;
        for tri in [start_arrow, end_arrow].into_iter().flatten() {
            let edge = point_to_segment_distance(local, tri[0], tri[1])
                .min(point_to_segment_distance(local, tri[1], tri[2]))
                .min(point_to_segment_distance(local, tri[2], tri[0]));
            let signed = if point_in_triangle(local, tri) {
                -edge
            } else {
                edge
            };
            distance = distance.min(signed);
        }
        distance
    }

    /// 局部坐标下的箭杆端点和起止箭头三角形。
    ///
    /// 有箭头时箭杆会缩短，给三角形留出位置。尺寸与 [`Line::generate_vertices`]
    /// 相同，SVG 导出直接使用这份几何，避免和画布上的箭头分叉。
    ///
    /// # 返回
    /// `(箭杆起点, 箭杆终点, 起点三角形, 终点三角形)`。
    pub fn local_shaft_and_markers(&self) -> (Vec2, Vec2, Option<[Vec2; 3]>, Option<[Vec2; 3]>) {
        let (shaft, start_arrow, end_arrow) = self.shaft_and_markers();
        (shaft[0], shaft[1], start_arrow, end_arrow)
    }

    /// 兼容旧名：只返回箭杆和终点箭头。
    pub fn local_shaft_and_arrow(&self) -> (Vec2, Vec2, Option<[Vec2; 3]>) {
        let (start, end, _start_arrow, end_arrow) = self.local_shaft_and_markers();
        (start, end, end_arrow)
    }

    /// 投影顶点。笔画是胶囊，箭头是单独的三角形剪影。关闭时为空。
    pub(crate) fn shadow_vertices(&self) -> Vec<Vertex> {
        if self.style.shadow.is_off() {
            return Vec::new();
        }
        let (start, end, start_arrow, end_arrow) = self.local_shaft_and_markers();
        let mut vertices = stroke_capsule_shadow(
            &[start, end],
            self.stroke_width() * 0.5,
            self.style.shadow,
            self.style.opacity,
            &self.props.transform,
        );
        for tri in [start_arrow, end_arrow].into_iter().flatten() {
            vertices.extend(soft_shadow_vertices(
                &[tri[0], tri[1], tri[2], tri[0]],
                self.style.shadow,
                self.style.opacity,
                &self.props.transform,
            ));
        }
        vertices
    }

    /// 内阴影顶点。笔画胶囊与起止箭头剪影，放在本体之后。
    pub(crate) fn inner_shadow_vertices(&self) -> Vec<Vertex> {
        if self.style.inner_shadow.is_off() {
            return Vec::new();
        }
        let (start, end, start_arrow, end_arrow) = self.local_shaft_and_markers();
        let mut vertices = stroke_capsule_inner_shadow(
            &[start, end],
            self.stroke_width() * 0.5,
            self.style.inner_shadow,
            self.style.opacity,
            &self.props.transform,
        );
        for tri in [start_arrow, end_arrow].into_iter().flatten() {
            vertices.extend(soft_inner_shadow_vertices(
                &[tri[0], tri[1], tri[2], tri[0]],
                self.style.inner_shadow,
                self.style.opacity,
                &self.props.transform,
            ));
        }
        vertices
    }

    /// 生成描边三角形（含可选起止箭头），坐标已变换到世界空间。
    pub fn generate_vertices(&self) -> Vec<Vertex> {
        let color = paint_color(&self.style);
        if blur_active(self.style.blur_radius) {
            let (start, end, start_arrow, end_arrow) = self.local_shaft_and_markers();
            let mut vertices = gaussian_capsule_vertices(
                &[start, end],
                self.stroke_width() * 0.5,
                color,
                self.style.blur_radius,
                &self.props.transform,
            );
            for tri in [start_arrow, end_arrow].into_iter().flatten() {
                vertices.extend(gaussian_fill_vertices(
                    &[tri[0], tri[1], tri[2], tri[0]],
                    color,
                    self.style.blur_radius,
                    &self.props.transform,
                ));
            }
            return vertices;
        }
        let width = self.stroke_width();
        let (shaft, start_arrow, end_arrow) = self.shaft_and_markers();
        let mut vertices = stroke_styled(
            &shaft,
            StrokePaint {
                width,
                color,
                dash: &self.style.stroke_dash,
                cap: self.style.stroke_cap,
                join: self.style.stroke_join,
            },
        );
        for tri in [start_arrow, end_arrow].into_iter().flatten() {
            for point in tri {
                vertices.push(Vertex::new(point.x, point.y, color));
            }
        }
        transform_vertices(vertices, &self.props.transform)
    }

    /// 局部箭杆，以及可选的起止箭头三角形。有箭头时箭杆已缩短。
    fn shaft_and_markers(&self) -> ([Vec2; 2], Option<[Vec2; 3]>, Option<[Vec2; 3]>) {
        let mut shaft = [self.start, self.end];
        let delta = self.end - self.start;
        if delta.length_squared() <= 1e-12 {
            return (shaft, None, None);
        }
        let width = self.stroke_width();
        let start_len = marker_length_for(self.start_marker, width);
        let end_len = marker_length_for(self.end_marker, width);
        let start_arrow = if self.start_marker.is_some() {
            arrow_points(
                self.start,
                self.start - self.end,
                start_len,
                marker_width_for(self.start_marker, width),
            )
        } else {
            None
        };
        let end_arrow = if self.end_marker.is_some() {
            arrow_points(
                self.end,
                delta,
                end_len,
                marker_width_for(self.end_marker, width),
            )
        } else {
            None
        };
        if start_arrow.is_some() {
            shorten_start(&mut shaft, start_len);
        }
        if end_arrow.is_some() {
            shorten_end(&mut shaft, end_len);
        }
        (shaft, start_arrow, end_arrow)
    }

    fn hit_radius(&self) -> f32 {
        self.stroke_width() * 0.5 + HIT_SLOP
    }

    fn start_marker_triangle_local(&self) -> Option<[Vec2; 3]> {
        if self.start_marker.is_none() {
            return None;
        }
        let width = self.stroke_width();
        arrow_points(
            self.start,
            self.start - self.end,
            marker_length_for(self.start_marker, width),
            marker_width_for(self.start_marker, width),
        )
    }

    fn end_marker_triangle_local(&self) -> Option<[Vec2; 3]> {
        if self.end_marker.is_none() {
            return None;
        }
        let width = self.stroke_width();
        let delta = self.end - self.start;
        arrow_points(
            self.end,
            delta,
            marker_length_for(self.end_marker, width),
            marker_width_for(self.end_marker, width),
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
        self.start_marker_triangle_local()
            .is_some_and(|tri| point_in_triangle(local, tri))
            || self
                .end_marker_triangle_local()
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
        assert_eq!(line.generate_vertices().len(), 9);
    }

    #[test]
    fn both_markers_add_two_triangles() {
        let line = Line::from_world(
            1,
            Vec2::ZERO,
            Vec2::new(80.0, 0.0),
            ElementStyle::stroke_only(Color::BLACK, 4.0),
            false,
        )
        .with_markers(LineMarker::Triangle, LineMarker::Arrow);
        // 箭杆 6 + 起点 3 + 终点 3
        assert_eq!(line.generate_vertices().len(), 12);
    }

    #[test]
    fn legacy_arrow_json_becomes_end_marker() {
        let json = r#"{
            "props": { "id": 1 },
            "start": [0.0, 0.0],
            "end": [10.0, 0.0],
            "arrow": true
        }"#;
        let line: Line = serde_json::from_str(json).unwrap();
        assert_eq!(line.end_marker, LineMarker::Arrow);
        assert!(line.arrow());
        let value = serde_json::to_value(&line).unwrap();
        assert_eq!(value["arrow"], true);
        assert_eq!(value["end_marker"], "arrow");
        assert!(value.get("start_marker").is_none());
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
