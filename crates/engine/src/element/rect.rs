//! 矩形元素
use super::blur::{blur_active, gaussian_fill_vertices, gaussian_stroke_vertices};
use super::gradient::paint_gradient_fill;
use super::shadow::{soft_inner_shadow_vertices, soft_shadow_vertices};
use super::stroke::{
    aligned_rect_metrics, color_with_opacity, dash_is_solid, stroke_centerline, stroke_styled,
    transform_vertices, StrokePaint,
};
use super::{Element, ElementId, ElementProps, ElementStyle};
use infinitecanvas_math::{Color, Rect, Vec2, Vertex};
use serde::{Deserialize, Serialize};
use std::f32::consts::{FRAC_PI_2, PI};

/// 每个圆角采样的圆弧段数。四角相同，合计约一圈。
const CORNER_SEGMENTS: usize = 8;

/// 矩形元素
///
/// # 属性
/// - 位置、尺寸（局部坐标系，与世界单位在单位缩放下相同）
/// - 填充和描边样式
/// - 四角相同的圆角半径（[`Self::corner_radius`]）
/// - 变换（通过 ElementProps）
///
/// # 命中与包围盒
/// 命中沿用轴对齐外接矩形（保守包围盒）：圆角切掉的角落仍然算命中。
/// [`Element::local_bounds`] / [`Element::world_bounds`] 不随圆角缩小，仍是外接矩形。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rectangle {
    /// 元素属性
    pub props: ElementProps,
    /// 尺寸
    pub size: Vec2,
    /// 样式。JSON 中可省略，缺省为 [`ElementStyle::new`]。
    #[serde(default)]
    pub style: ElementStyle,
    /// 四角相同的圆角半径，单位与 [`Self::size`] 相同（单位缩放下即世界单位）。
    ///
    /// `0` 为直角。JSON 缺省为 0。超过短边一半时，绘制和 [`Self::set_corner_radius`]
    /// 会钳到该上限。
    #[serde(default, skip_serializing_if = "is_zero_radius")]
    pub corner_radius: f32,
}

fn is_zero_radius(value: &f32) -> bool {
    *value == 0.0
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
            corner_radius: 0.0,
        }
    }

    /// 设置样式
    pub fn with_style(mut self, style: ElementStyle) -> Self {
        self.style = style;
        self
    }

    /// 设置圆角半径后返回自身。半径会钳到短边的一半。
    ///
    /// # 参数
    /// - `radius`: 四角相同的半径。负数和非有限值变成 0。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::Rectangle;
    /// use infinitecanvas_math::Vec2;
    ///
    /// let rect = Rectangle::new(1, Vec2::ZERO, Vec2::new(40.0, 20.0)).with_corner_radius(30.0);
    /// assert_eq!(rect.corner_radius(), 10.0);
    /// ```
    pub fn with_corner_radius(mut self, radius: f32) -> Self {
        self.set_corner_radius(radius);
        self
    }

    /// 当前存储的圆角半径。
    pub fn corner_radius(&self) -> f32 {
        self.corner_radius
    }

    /// 写入圆角半径，并钳到 `[0, 短边的一半]`。
    ///
    /// 非有限值写成 0。调用方还要 [`crate::Scene::mark_dirty`]，顶点才会重生成。
    /// 样式命令会自己标脏。
    pub fn set_corner_radius(&mut self, radius: f32) {
        self.corner_radius = Self::clamp_radius(self.size, radius);
    }

    /// 把半径钳到 `[0, 短边的一半]`。非有限或负数变成 0。
    pub fn clamp_radius(size: Vec2, radius: f32) -> f32 {
        if !radius.is_finite() || radius <= 0.0 {
            return 0.0;
        }
        let limit = size.x.abs().min(size.y.abs()) * 0.5;
        if !limit.is_finite() {
            return 0.0;
        }
        radius.min(limit)
    }

    /// 绘制使用的半径。尺寸变小后也不会超过短边一半。
    pub fn effective_corner_radius(&self) -> f32 {
        Self::clamp_radius(self.size, self.corner_radius)
    }

    /// 局部闭合轮廓，不重复首点。
    ///
    /// 直角是四个角，顺序为左上、右上、右下、左下（屏幕 Y 向下时看起来是顺时针）。
    /// 圆角沿同一方向采样。宽或高不是有限正数时返回空，布尔运算会跳过这个矩形。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::Rectangle;
    /// use infinitecanvas_math::Vec2;
    ///
    /// let rect = Rectangle::new(1, Vec2::ZERO, Vec2::new(20.0, 10.0));
    /// assert_eq!(rect.local_outline().len(), 4);
    /// ```
    pub fn local_outline(&self) -> Vec<Vec2> {
        let width = self.size.x.abs();
        let height = self.size.y.abs();
        if !width.is_finite()
            || !height.is_finite()
            || width <= f32::EPSILON
            || height <= f32::EPSILON
        {
            return Vec::new();
        }
        let radius = self.effective_corner_radius();
        if radius > 0.0 {
            let mut points = self.rounded_outline(radius);
            if let (Some(first), Some(last)) = (points.first().copied(), points.last().copied()) {
                if points.len() >= 2 && (first - last).length() <= 1.0e-4 {
                    points.pop();
                }
            }
            points
        } else {
            let half_w = width * 0.5;
            let half_h = height * 0.5;
            vec![
                Vec2::new(-half_w, -half_h),
                Vec2::new(half_w, -half_h),
                Vec2::new(half_w, half_h),
                Vec2::new(-half_w, half_h),
            ]
        }
    }

    /// 获取中心点（局部坐标系）
    pub fn center(&self) -> Vec2 {
        Vec2::ZERO
    }

    /// 获取尺寸
    pub fn size(&self) -> Vec2 {
        self.size
    }

    /// 投影顶点。关闭时为空。轮廓跟圆角，放在填充之前。
    ///
    /// 不改变 [`Element::local_bounds`] 或命中测试。
    pub(crate) fn shadow_vertices(&self) -> Vec<Vertex> {
        soft_shadow_vertices(
            &self.local_silhouette(),
            self.style.shadow,
            self.style.opacity,
            &self.props.transform,
        )
    }

    /// 内阴影顶点。关闭时为空。轮廓跟圆角，放在填充之后。
    pub(crate) fn inner_shadow_vertices(&self) -> Vec<Vertex> {
        soft_inner_shadow_vertices(
            &self.local_silhouette(),
            self.style.inner_shadow,
            self.style.opacity,
            &self.props.transform,
        )
    }

    /// 局部空间的形状外轮廓。半径为 0 时是四个角，首尾闭合。
    fn local_silhouette(&self) -> Vec<Vec2> {
        let radius = self.effective_corner_radius();
        if radius > 0.0 {
            return self.rounded_outline(radius);
        }
        let half_w = self.size.x / 2.0;
        let half_h = self.size.y / 2.0;
        vec![
            Vec2::new(-half_w, -half_h),
            Vec2::new(half_w, -half_h),
            Vec2::new(half_w, half_h),
            Vec2::new(-half_w, half_h),
            Vec2::new(-half_w, -half_h),
        ]
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
        if !self.style.has_paintable_fill() {
            return Vec::new();
        }

        if blur_active(self.style.blur_radius) {
            let bounds = self.local_bounds();
            let sampled = self
                .style
                .sample_fill(bounds.center(), bounds)
                .unwrap_or(Color::BLACK);
            let color = color_with_opacity(sampled, self.style.opacity);
            return gaussian_fill_vertices(
                &self.local_silhouette(),
                color,
                self.style.blur_radius,
                &self.props.transform,
            );
        }
        let radius = self.effective_corner_radius();
        if self.style.uses_gradient() {
            let outline = if radius > 0.0 {
                self.rounded_outline(radius)
            } else {
                let half_w = self.size.x / 2.0;
                let half_h = self.size.y / 2.0;
                vec![
                    Vec2::new(-half_w, -half_h),
                    Vec2::new(half_w, -half_h),
                    Vec2::new(half_w, half_h),
                    Vec2::new(-half_w, half_h),
                ]
            };
            return paint_gradient_fill(
                &outline,
                &self.style,
                self.local_bounds(),
                &self.props.transform,
            );
        }

        let color = color_with_opacity(self.style.fill.unwrap(), self.style.opacity);
        if radius > 0.0 {
            return self.fill_from_outline(&self.rounded_outline(radius), color);
        }
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
        if self.style.stroke.is_none() || self.style.stroke_width <= 0.0 {
            return Vec::new();
        }

        let color = color_with_opacity(self.style.stroke.unwrap(), self.style.opacity);
        if blur_active(self.style.blur_radius) {
            return Vec::new();
        }
        let radius = self.effective_corner_radius();
        let half_w = self.size.x / 2.0;
        let half_h = self.size.y / 2.0;
        if !dash_is_solid(&self.style.stroke_dash) {
            let local = self.stroke_outline_points();
            return transform_vertices(
                stroke_centerline(&local, color, &self.style.stroke_dash),
                &self.props.transform,
            );
        }
        if radius > 0.0 {
            return self.stroke_lines_from_outline(&self.rounded_outline(radius), color);
        }

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

    /// 按描边对齐得到局部闭合轮廓（用于粗描边和虚线中心线）。
    fn stroke_outline_points(&self) -> Vec<Vec2> {
        let radius = self.effective_corner_radius();
        let half_w = self.size.x.abs() / 2.0;
        let half_h = self.size.y.abs() / 2.0;
        let (hw, hh, aligned_radius) = aligned_rect_metrics(
            half_w,
            half_h,
            radius,
            self.style.stroke_width,
            self.style.stroke_align,
        );
        if aligned_radius > 0.0 {
            return self.rounded_outline_sized(hw, hh, aligned_radius);
        }
        vec![
            Vec2::new(-hw, -hh),
            Vec2::new(hw, -hh),
            Vec2::new(hw, hh),
            Vec2::new(-hw, hh),
            Vec2::new(-hw, -hh),
        ]
    }

    /// 按线宽生成描边三角形，放进填充缓冲后画布才能看出线宽。
    ///
    /// WebGPU 的线段管线宽度固定为 1 像素，所以粗描边不走 [`Self::generate_stroke_vertices`]。
    /// 线宽小于等于 0 或没有描边色时返回空。
    pub(crate) fn stroke_ribbon_vertices(&self) -> Vec<Vertex> {
        let Some(stroke) = self.style.stroke else {
            return Vec::new();
        };
        if self.style.stroke_width <= 0.0 {
            return Vec::new();
        }
        let color = color_with_opacity(stroke, self.style.opacity);
        let points = self.stroke_outline_points();
        if blur_active(self.style.blur_radius) {
            return gaussian_stroke_vertices(
                &points,
                self.style.stroke_width * 0.5,
                color,
                self.style.blur_radius,
                &self.props.transform,
            );
        }
        transform_vertices(
            stroke_styled(
                &points,
                StrokePaint {
                    width: self.style.stroke_width,
                    color,
                    dash: &self.style.stroke_dash,
                    cap: self.style.stroke_cap,
                    join: self.style.stroke_join,
                },
            ),
            &self.props.transform,
        )
    }

    /// 局部空间的圆角轮廓，闭合（首尾同一点）。半径须已钳制且大于 0。
    fn rounded_outline(&self, radius: f32) -> Vec<Vec2> {
        let half_w = self.size.x.abs() * 0.5;
        let half_h = self.size.y.abs() * 0.5;
        self.rounded_outline_sized(half_w, half_h, radius)
    }

    /// 指定半宽半高和圆角的局部圆角轮廓。
    fn rounded_outline_sized(&self, half_w: f32, half_h: f32, radius: f32) -> Vec<Vec2> {
        let radius = radius.min(half_w).min(half_h);
        // 顺时针绕一圈（屏幕 Y 向下）：右上、右下、左下、左上。
        let corners = [
            (Vec2::new(half_w - radius, -half_h + radius), -FRAC_PI_2),
            (Vec2::new(half_w - radius, half_h - radius), 0.0),
            (Vec2::new(-half_w + radius, half_h - radius), FRAC_PI_2),
            (Vec2::new(-half_w + radius, -half_h + radius), PI),
        ];
        let step = FRAC_PI_2 / CORNER_SEGMENTS as f32;
        let mut points = Vec::with_capacity(corners.len() * CORNER_SEGMENTS + 2);
        for (center, start) in corners {
            for index in 0..=CORNER_SEGMENTS {
                let angle = start + index as f32 * step;
                points.push(center + Vec2::new(angle.cos() * radius, angle.sin() * radius));
            }
        }
        if let Some(first) = points.first().copied() {
            points.push(first);
        }
        points
    }

    /// 以中心为扇心，把闭合轮廓铺成填充三角形。
    fn fill_from_outline(&self, points: &[Vec2], color: Color) -> Vec<Vertex> {
        let transform = &self.props.transform;
        let center = transform.transform_point(Vec2::ZERO);
        let mut vertices = Vec::with_capacity(points.len().saturating_sub(1) * 3);
        for pair in points.windows(2) {
            let a = transform.transform_point(pair[0]);
            let b = transform.transform_point(pair[1]);
            vertices.push(Vertex::new(center.x, center.y, color));
            vertices.push(Vertex::new(a.x, a.y, color));
            vertices.push(Vertex::new(b.x, b.y, color));
        }
        vertices
    }

    /// 闭合轮廓的 1 像素线段列表。
    fn stroke_lines_from_outline(&self, points: &[Vec2], color: Color) -> Vec<Vertex> {
        let transform = &self.props.transform;
        let mut vertices = Vec::with_capacity(points.len().saturating_sub(1) * 2);
        for pair in points.windows(2) {
            let a = transform.transform_point(pair[0]);
            let b = transform.transform_point(pair[1]);
            vertices.push(Vertex::new(a.x, a.y, color));
            vertices.push(Vertex::new(b.x, b.y, color));
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

        let rect =
            Rectangle::new(1, Vec2::new(100.0, 100.0), Vec2::new(50.0, 30.0)).with_style(style);

        let vertices = rect.generate_vertices();

        // 填充6个 + 描边8个 = 14个顶点
        assert_eq!(vertices.len(), 14);
    }

    #[test]
    fn corner_radius_defaults_to_sharp_and_clamps_to_half_the_short_side() {
        let mut rect = Rectangle::new(1, Vec2::ZERO, Vec2::new(30.0, 10.0));
        assert_eq!(rect.corner_radius(), 0.0);
        assert_eq!(rect.effective_corner_radius(), 0.0);

        rect.set_corner_radius(100.0);
        assert_eq!(rect.corner_radius(), 5.0);
        rect.set_corner_radius(-4.0);
        assert_eq!(rect.corner_radius(), 0.0);
        rect.set_corner_radius(f32::NAN);
        assert_eq!(rect.corner_radius(), 0.0);

        let wide = Rectangle::new(2, Vec2::ZERO, Vec2::new(40.0, 20.0)).with_corner_radius(6.0);
        assert_eq!(wide.corner_radius(), 6.0);
        assert_eq!(Rectangle::clamp_radius(Vec2::new(8.0, 40.0), 9.0), 4.0);
    }

    #[test]
    fn corner_radius_json_round_trip_and_legacy_defaults_to_zero() {
        let rect =
            Rectangle::new(1, Vec2::new(4.0, 5.0), Vec2::new(20.0, 12.0)).with_corner_radius(4.0);
        let json = serde_json::to_string(&rect).unwrap();
        assert!(json.contains("\"corner_radius\":4"));
        let restored: Rectangle = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.corner_radius(), 4.0);
        assert_eq!(restored.size, rect.size);

        let sharp = Rectangle::new(2, Vec2::ZERO, Vec2::new(8.0, 8.0));
        let sharp_json = serde_json::to_value(&sharp).unwrap();
        assert!(sharp_json.get("corner_radius").is_none());

        let size = serde_json::to_value(Vec2::new(10.0, 20.0)).unwrap();
        let legacy = serde_json::json!({
            "props": { "id": 3 },
            "size": size
        });
        let old: Rectangle = serde_json::from_value(legacy).unwrap();
        assert_eq!(old.corner_radius(), 0.0);
        assert_eq!(old.generate_fill_vertices().len(), 6);
    }

    #[test]
    fn rounded_fill_cuts_the_corner_and_keeps_the_outer_bounds() {
        use super::super::stroke::point_in_triangle;

        let rect = Rectangle::new(1, Vec2::new(100.0, 50.0), Vec2::new(40.0, 20.0))
            .with_style(ElementStyle::fill_only(Color::RED))
            .with_corner_radius(6.0);
        let bounds = rect.local_bounds();
        assert_eq!(bounds.width, 40.0);
        assert_eq!(bounds.height, 20.0);

        let vertices = rect.generate_fill_vertices();
        assert!(vertices.len() > 6);
        assert!(vertices.len().is_multiple_of(3));
        assert!(vertices
            .iter()
            .all(|vertex| vertex.color == [1.0, 0.0, 0.0, 1.0]));
        assert!(vertices
            .iter()
            .all(|vertex| (vertex.position[0] - 100.0).abs() <= 20.0 + 1e-3
                && (vertex.position[1] - 50.0).abs() <= 10.0 + 1e-3));

        let contains = |point: Vec2| {
            vertices.chunks(3).any(|tri| {
                point_in_triangle(
                    point,
                    [
                        Vec2::new(tri[0].position[0], tri[0].position[1]),
                        Vec2::new(tri[1].position[0], tri[1].position[1]),
                        Vec2::new(tri[2].position[0], tri[2].position[1]),
                    ],
                )
            })
        };
        assert!(contains(Vec2::new(100.0, 50.0)));
        // 外接矩形的角被圆角切掉。
        assert!(!contains(Vec2::new(119.0, 59.0)));
        // 直角矩形仍然盖住同一个角。
        let sharp = Rectangle::new(1, Vec2::new(100.0, 50.0), Vec2::new(40.0, 20.0))
            .with_style(ElementStyle::fill_only(Color::RED));
        let sharp_vertices = sharp.generate_fill_vertices();
        assert_eq!(sharp_vertices.len(), 6);
        assert!(sharp_vertices.chunks(3).any(|tri| {
            point_in_triangle(
                Vec2::new(119.0, 59.0),
                [
                    Vec2::new(tri[0].position[0], tri[0].position[1]),
                    Vec2::new(tri[1].position[0], tri[1].position[1]),
                    Vec2::new(tri[2].position[0], tri[2].position[1]),
                ],
            )
        }));
    }

    #[test]
    fn rounded_stroke_follows_the_arc_and_hit_stays_on_the_outer_rect() {
        let rect = Rectangle::new(1, Vec2::ZERO, Vec2::new(40.0, 20.0))
            .with_style(ElementStyle::stroke_only(Color::BLUE, 2.0))
            .with_corner_radius(6.0);
        let lines = rect.generate_stroke_vertices();
        assert!(lines.len() > 8);
        assert_eq!(lines.len() % 2, 0);
        let ribbon = rect.stroke_ribbon_vertices();
        assert!(ribbon.len() > 8);
        assert!(ribbon
            .iter()
            .all(|vertex| vertex.color == [0.0, 0.0, 1.0, 1.0]));

        // 命中是保守的外接矩形：切掉的角仍能点中，框外不行。
        assert!(rect.contains_point(Vec2::new(19.0, 9.0)));
        assert!(rect.contains_point(Vec2::ZERO));
        assert!(!rect.contains_point(Vec2::new(30.0, 0.0)));
        assert_eq!(rect.world_bounds().width, 40.0);
        assert_eq!(rect.world_bounds().height, 20.0);
    }

    #[test]
    fn dashed_rounded_stroke_follows_the_outline_instead_of_a_solid_overlay() {
        let mut style = ElementStyle::stroke_only(Color::BLUE, 2.0);
        style.stroke_dash = vec![4.0, 4.0];
        let dashed = Rectangle::new(1, Vec2::ZERO, Vec2::new(40.0, 20.0))
            .with_style(style)
            .with_corner_radius(6.0);
        let solid = Rectangle::new(1, Vec2::ZERO, Vec2::new(40.0, 20.0))
            .with_style(ElementStyle::stroke_only(Color::BLUE, 2.0))
            .with_corner_radius(6.0);
        let dashed_lines = dashed.generate_stroke_vertices();
        let solid_lines = solid.generate_stroke_vertices();
        assert!(dashed_lines.len() > 8);
        assert_eq!(dashed_lines.len() % 2, 0);
        assert!(dashed_lines.len() < solid_lines.len());
        let dashed_ribbon = dashed.stroke_ribbon_vertices();
        let solid_ribbon = solid.stroke_ribbon_vertices();
        assert!(!dashed_ribbon.is_empty());
        assert_ne!(dashed_ribbon.len(), solid_ribbon.len());
    }
}
