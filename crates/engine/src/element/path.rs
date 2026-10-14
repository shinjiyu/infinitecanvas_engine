//! 自由画笔路径
//!
//! 沿采样点生成带线宽的三角形带，拐角按斜接/斜角处理。
//! 命中测试使用点到各线段的距离，而不是包围盒。

use super::gradient::paint_gradient_fill;
use super::shadow::{soft_shadow_vertices, stroke_capsule_shadow};
use super::stroke::{
    bounds_containing, color_with_opacity, distance_to_polyline, effective_width, paint_color,
    stroke_styled, transform_vertices, StrokePaint, HIT_SLOP, MITER_LIMIT,
};
use super::{Element, ElementId, ElementProps, ElementStyle, FillKind};
use infinitecanvas_math::{Color, Rect, Transform, Vec2, Vertex};
use serde::{Deserialize, Serialize};

/// 一条闭合环，局部坐标，不重复首点。
///
/// JSON 缺省为空。布尔运算把外轮廓和孔都写成环。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FillShape {
    /// 环列表。第一条是外轮廓，其余是孔。
    #[serde(default)]
    pub rings: Vec<Vec<Vec2>>,
}

/// 自由绘制路径，或布尔运算得到的闭合填充路径。
///
/// 采样点存放在局部坐标。`from_world_points` 把变换原点放在第一个点，
/// 之后拖动只平移变换，路径形状保持不变。
///
/// [`Path::shapes`] 非空时，这是一块可填充的复合路径（可含孔和多个外轮廓）。
/// 自由画笔保持 `shapes` 为空，只沿 `points` 描边。旧文档没有这个字段，仍然有效。
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
    /// 闭合填充轮廓。空表示自由画笔。JSON 缺省为空，空时不写出。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shapes: Vec<FillShape>,
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
            shapes: Vec::new(),
        }
    }

    /// 闭合填充轮廓。自由画笔返回空切片。
    pub fn shapes(&self) -> &[FillShape] {
        &self.shapes
    }

    /// 是否按填充轮廓绘制，而不是自由画笔折线。
    pub fn is_filled(&self) -> bool {
        self.shapes
            .iter()
            .any(|shape| shape.rings.first().is_some_and(|ring| ring.len() >= 3))
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
        if !self.props.visible {
            return None;
        }
        if self.is_filled() {
            return self.filled_color_at(world);
        }
        if self.points.is_empty() {
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

    /// 局部点到剪影的有符号距离。负值在笔画或填充内。命中测试仍加点击余量。
    ///
    /// 自由画笔量到折线。填充路径量到环：外轮廓内且不在孔里为负。
    pub(crate) fn silhouette_distance(&self, local: Vec2) -> f32 {
        if self.is_filled() {
            let distance = self.distance_to_rings(local);
            if self.contains_local_fill(local) {
                -distance
            } else {
                distance
            }
        } else {
            distance_to_polyline(local, &self.points) - self.stroke_width() * 0.5
        }
    }

    /// 投影顶点。自由画笔沿笔画胶囊；填充路径沿每条外轮廓。关闭时为空。
    ///
    /// 命中仍是点到折线或填充的距离，不包含偏移和模糊。
    pub(crate) fn shadow_vertices(&self) -> Vec<Vertex> {
        if self.is_filled() {
            let mut vertices = Vec::new();
            for shape in &self.shapes {
                let Some(exterior) = shape.rings.first() else {
                    continue;
                };
                vertices.extend(soft_shadow_vertices(
                    exterior,
                    self.style.shadow,
                    self.style.opacity,
                    &self.props.transform,
                ));
            }
            return vertices;
        }
        stroke_capsule_shadow(
            &self.points,
            self.stroke_width() * 0.5,
            self.style.shadow,
            self.style.opacity,
            &self.props.transform,
        )
    }

    fn distance_to_rings(&self, local: Vec2) -> f32 {
        let mut best = f32::MAX;
        for shape in &self.shapes {
            for ring in &shape.rings {
                let mut closed = ring.clone();
                if let Some(first) = ring.first().copied() {
                    closed.push(first);
                }
                best = best.min(distance_to_polyline(local, &closed));
            }
        }
        if best.is_finite() {
            best
        } else {
            f32::MAX
        }
    }

    /// 填充路径的取样：描边盖在填充上。点在孔里、或在所有外轮廓外时返回 `None`。
    fn filled_color_at(&self, world: Vec2) -> Option<Color> {
        let local = try_local(&self.props.transform, world)?;
        if self.on_shape_stroke(local) {
            if let Some(stroke) = self.style.stroke {
                let color = color_with_opacity(stroke, self.style.opacity);
                if color.a > 0.0 {
                    return Some(color);
                }
            }
        }
        if self.contains_local_fill(local) {
            let fill = self.style.sample_fill(local, self.local_bounds())?;
            let color = color_with_opacity(fill, self.style.opacity);
            if color.a > 0.0 {
                return Some(color);
            }
        }
        None
    }

    fn contains_local_fill(&self, local: Vec2) -> bool {
        self.shapes.iter().any(|shape| shape_contains(local, shape))
    }

    fn on_shape_stroke(&self, local: Vec2) -> bool {
        let Some(stroke) = self.style.stroke else {
            return false;
        };
        if stroke.a <= 0.0 || self.style.stroke_width <= 0.0 {
            return false;
        }
        let limit = self.style.stroke_width * 0.5;
        self.shapes.iter().any(|shape| {
            shape.rings.iter().any(|ring| {
                let mut closed = ring.clone();
                if let Some(first) = ring.first().copied() {
                    closed.push(first);
                }
                distance_to_polyline(local, &closed) <= limit + 1.0e-4
            })
        })
    }

    /// 生成三角形，坐标在世界空间。
    ///
    /// 自由画笔是描边带。填充路径是耳切三角形加上闭合描边带。
    /// 线性渐变按色标切开每张三角形；径向在顶点取样，孔仍留在耳切结果之外。
    /// 投影由 [`Path::shadow_vertices`] 另画。
    pub fn generate_vertices(&self) -> Vec<Vertex> {
        if self.is_filled() {
            let mut vertices = self.fill_vertices();
            vertices.extend(self.shape_stroke_vertices());
            return vertices;
        }
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

    fn fill_vertices(&self) -> Vec<Vertex> {
        if !self.style.has_paintable_fill() {
            return Vec::new();
        }
        let transform = &self.props.transform;
        let bounds = self.local_bounds();
        let mut vertices = Vec::new();
        if self.style.uses_gradient() {
            for shape in &self.shapes {
                for triangle in triangulate_shape(shape) {
                    if self.style.fill_kind == FillKind::Linear {
                        vertices.extend(paint_gradient_fill(
                            &triangle,
                            &self.style,
                            bounds,
                            transform,
                        ));
                    } else {
                        for point in triangle {
                            let color = color_with_opacity(
                                self.style
                                    .sample_fill(point, bounds)
                                    .unwrap_or(Color::BLACK),
                                self.style.opacity,
                            );
                            let world = transform.transform_point(point);
                            vertices.push(Vertex::new(world.x, world.y, color));
                        }
                    }
                }
            }
            return vertices;
        }
        let Some(fill) = self.style.fill else {
            return Vec::new();
        };
        let color = color_with_opacity(fill, self.style.opacity);
        if color.a <= 0.0 {
            return Vec::new();
        }
        for shape in &self.shapes {
            for triangle in triangulate_shape(shape) {
                for point in triangle {
                    let world = transform.transform_point(point);
                    vertices.push(Vertex::new(world.x, world.y, color));
                }
            }
        }
        vertices
    }

    fn shape_stroke_vertices(&self) -> Vec<Vertex> {
        let Some(stroke) = self.style.stroke else {
            return Vec::new();
        };
        if self.style.stroke_width <= 0.0 || stroke.a <= 0.0 {
            return Vec::new();
        }
        let color = color_with_opacity(stroke, self.style.opacity);
        let mut vertices = Vec::new();
        for shape in &self.shapes {
            for ring in &shape.rings {
                if ring.len() < 2 {
                    continue;
                }
                let mut closed = ring.clone();
                if let Some(first) = ring.first().copied() {
                    closed.push(first);
                }
                vertices.extend(transform_vertices(
                    stroke_styled(
                        &closed,
                        StrokePaint {
                            width: self.style.stroke_width,
                            color,
                            dash: &self.style.stroke_dash,
                            cap: self.style.stroke_cap,
                            join: self.style.stroke_join,
                        },
                    ),
                    &self.props.transform,
                ));
            }
        }
        vertices
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
        if self.is_filled() {
            let mut points = Vec::new();
            for shape in &self.shapes {
                for ring in &shape.rings {
                    points.extend_from_slice(ring);
                }
            }
            return bounds_containing(&points, 0.0);
        }
        let pad = self.stroke_width() * 0.5 * MITER_LIMIT;
        bounds_containing(&self.points, pad)
    }

    fn type_name(&self) -> &'static str {
        "Path"
    }

    fn contains_point(&self, point: Vec2) -> bool {
        if self.is_filled() {
            let local = self.props.transform.inverse_transform_point(point);
            return self.contains_local_fill(local) || self.filled_stroke_hit(local);
        }
        if self.points.is_empty() {
            return false;
        }
        let local = self.props.transform.inverse_transform_point(point);
        distance_to_polyline(local, &self.points) <= self.hit_radius()
    }
}

fn filled_stroke_limit(path: &Path) -> f32 {
    if path.style.stroke.is_none() || path.style.stroke_width <= 0.0 {
        return 0.0;
    }
    path.style.stroke_width * 0.5 + HIT_SLOP
}

impl Path {
    fn filled_stroke_hit(&self, local: Vec2) -> bool {
        let limit = filled_stroke_limit(self);
        if limit <= 0.0 {
            return false;
        }
        self.shapes.iter().any(|shape| {
            shape.rings.iter().any(|ring| {
                let mut closed = ring.clone();
                if let Some(first) = ring.first().copied() {
                    closed.push(first);
                }
                distance_to_polyline(local, &closed) <= limit
            })
        })
    }
}

/// 奇偶射线。水平边不参与，避免除以零。
fn point_in_ring(point: Vec2, ring: &[Vec2]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = ring.len() - 1;
    for current in 0..ring.len() {
        let a = ring[current];
        let b = ring[previous];
        let crosses = (a.y > point.y) != (b.y > point.y);
        if crosses {
            let x = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
            if point.x < x {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

fn shape_contains(point: Vec2, shape: &FillShape) -> bool {
    let Some(exterior) = shape.rings.first() else {
        return false;
    };
    if !point_in_ring(point, exterior) {
        return false;
    }
    shape
        .rings
        .iter()
        .skip(1)
        .all(|hole| !point_in_ring(point, hole))
}

/// 把一个外轮廓（加孔）耳切成局部三角形。失败时返回空，描边仍然绘制。
fn triangulate_shape(shape: &FillShape) -> Vec<[Vec2; 3]> {
    let Some(exterior) = shape.rings.first() else {
        return Vec::new();
    };
    if exterior.len() < 3 {
        return Vec::new();
    }
    let mut coords = Vec::with_capacity((exterior.len() + 8) * 2);
    let mut holes = Vec::new();
    push_ring(&mut coords, exterior);
    for hole in shape.rings.iter().skip(1) {
        if hole.len() < 3 {
            continue;
        }
        holes.push(coords.len() / 2);
        push_ring(&mut coords, hole);
    }
    let indices = earcutr::earcut(&coords, &holes, 2).unwrap_or_default();
    let (chunks, _) = indices.as_chunks::<3>();
    let mut triangles = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let Some(a) = coord_at(&coords, chunk[0]) else {
            continue;
        };
        let Some(b) = coord_at(&coords, chunk[1]) else {
            continue;
        };
        let Some(c) = coord_at(&coords, chunk[2]) else {
            continue;
        };
        triangles.push([a, b, c]);
    }
    triangles
}

fn push_ring(coords: &mut Vec<f64>, ring: &[Vec2]) {
    for point in ring {
        coords.push(point.x as f64);
        coords.push(point.y as f64);
    }
}

fn coord_at(coords: &[f64], index: usize) -> Option<Vec2> {
    let offset = index.checked_mul(2)?;
    let x = *coords.get(offset)?;
    let y = *coords.get(offset + 1)?;
    if x.is_finite() && y.is_finite() {
        Some(Vec2::new(x as f32, y as f32))
    } else {
        None
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
