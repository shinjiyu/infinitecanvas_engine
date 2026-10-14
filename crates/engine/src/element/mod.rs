//! 元素定义
//!
//! 提供无限画布上的各种图形元素
pub mod circle;
pub mod group;
pub mod image;
pub mod line;
pub mod path;
pub mod rect;
mod stroke;
pub mod text;

pub use circle::Circle;
pub use group::Group;
pub use image::Image;
pub use line::Line;
pub use path::Path;
pub use rect::Rectangle;
pub use text::{
    estimate_size, lod_bucket, lod_scale, padding_world, raster_params, raster_texture_id,
    RasterParams, Text, TextAlign, TextClickResolver, TextPlacement, TextVisual,
};

use infinitecanvas_math::{Color, Rect, Transform, Vec2};
use serde::{Deserialize, Serialize};

/// 元素 ID 类型
pub type ElementId = u64;

fn default_visible() -> bool {
    true
}

fn default_fill() -> Option<Color> {
    Some(Color::BLACK)
}

fn default_stroke_width() -> f32 {
    1.0
}

fn default_opacity() -> f32 {
    1.0
}

fn cap_is_butt(cap: &StrokeCap) -> bool {
    *cap == StrokeCap::Butt
}

fn join_is_miter(join: &StrokeJoin) -> bool {
    *join == StrokeJoin::Miter
}

/// 描边端点。
///
/// 闭合实线（矩形、圆）没有端点，线帽只作用在开口路径和每一段虚线的两端。
/// JSON 用小写名字，缺省为 [`StrokeCap::Butt`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StrokeCap {
    /// 在端点处截平，不额外伸出。
    #[default]
    Butt,
    /// 以半线宽为半径的半圆。
    Round,
    /// 沿切线方向伸出半线宽的矩形。
    Square,
}

impl StrokeCap {
    /// SVG `stroke-linecap` 和属性面板用的名字。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Butt => "butt",
            Self::Round => "round",
            Self::Square => "square",
        }
    }

    /// 解析 `butt` / `round` / `square`。无法识别时返回 `None`。
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "butt" => Some(Self::Butt),
            "round" => Some(Self::Round),
            "square" => Some(Self::Square),
            _ => None,
        }
    }
}

/// 折线拐角。
///
/// 斜接长度超过线宽的 4 倍时退化为斜角，和实线描边原来的限制相同。
/// JSON 用小写名字，缺省为 [`StrokeJoin::Miter`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StrokeJoin {
    /// 沿两侧外轮廓延长到交点。缺省值。
    #[default]
    Miter,
    /// 以外侧圆弧连接。
    Round,
    /// 直接连两侧外轮廓的端点。
    Bevel,
}

impl StrokeJoin {
    /// SVG `stroke-linejoin` 和属性面板用的名字。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Miter => "miter",
            Self::Round => "round",
            Self::Bevel => "bevel",
        }
    }

    /// 解析 `miter` / `round` / `bevel`。无法识别时返回 `None`。
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "miter" => Some(Self::Miter),
            "round" => Some(Self::Round),
            "bevel" => Some(Self::Bevel),
            _ => None,
        }
    }
}

/// 元素通用属性
///
/// # 职责
/// - 唯一标识
/// - 位置和变换
/// - 可见性和选中状态
/// - 层级顺序
///
/// JSON 里除 `id` 以外的字段都可省略：变换缺省为单位变换（旋转 0、缩放 1），
/// `visible` 缺省为 true。未知字段会被忽略。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementProps {
    /// 唯一 ID
    pub id: ElementId,
    /// 变换（位置、旋转、缩放）
    #[serde(default)]
    pub transform: Transform,
    /// 是否可见
    #[serde(default = "default_visible")]
    pub visible: bool,
    /// 是否选中
    #[serde(default)]
    pub selected: bool,
    /// 层级（z-index）
    #[serde(default)]
    pub z_index: i32,
    /// 父元素。`None` 表示挂在场景根上。只有 [`crate::Group`] 会成为父节点。
    ///
    /// `serde(default)` 让还没有层级字段的旧 JSON 仍能反序列化。
    #[serde(default)]
    pub parent: Option<ElementId>,
    /// 是否锁定。锁定的元素及其子孙拒绝变换；大纲面板用它画锁图标。
    #[serde(default)]
    pub locked: bool,
    /// 几何、样式或变换已变化，需要重新生成顶点。
    ///
    /// 运行时状态，不写入场景 JSON。新建和导入的元素默认为 `true`。
    /// 视口平移/缩放不修改此标记。场景通过 [`crate::Scene::mark_dirty`] 入队；
    /// 只改这个字段不会入队。相等比较忽略它，避免污染持久化往返。
    #[serde(skip, default = "default_dirty")]
    pub dirty: bool,
}

impl PartialEq for ElementProps {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.transform == other.transform
            && self.visible == other.visible
            && self.selected == other.selected
            && self.z_index == other.z_index
            && self.parent == other.parent
            && self.locked == other.locked
    }
}

fn default_dirty() -> bool {
    true
}

impl ElementProps {
    /// 创建元素属性
    ///
    /// 新元素的 `dirty` 为 `true`，第一次同步顶点缓存时会生成顶点。
    pub fn new(id: ElementId) -> Self {
        Self {
            id,
            transform: Transform::identity(),
            visible: true,
            selected: false,
            z_index: 0,
            parent: None,
            locked: false,
            dirty: true,
        }
    }

    /// 绕变换原点的旋转角，单位弧度。
    ///
    /// 正角为逆时针，与 [`Transform::rotation`] 是同一个字段。顶点生成和命中测试
    /// 都读它：世界坐标会先逆变换回局部空间，所以旋转后的矩形、圆、直线和路径
    /// 仍能被点中。
    pub fn rotation(&self) -> f32 {
        self.transform.rotation
    }

    /// 设置旋转角（弧度）。
    ///
    /// 只改这个字段不会让顶点缓存入队。渲染前还要调用 [`crate::Scene::mark_dirty`]。
    pub fn set_rotation(&mut self, radians: f32) {
        self.transform.rotation = radians;
    }

    /// 几何或样式是否需要重新生成顶点
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// 标记需要重新生成顶点
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// 顶点已经写回缓存
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

/// 元素样式
///
/// 省略的字段使用与 [`ElementStyle::new`] 相同的缺省值。未知字段会被忽略。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElementStyle {
    /// 填充颜色（None 表示不填充）
    #[serde(default = "default_fill")]
    pub fill: Option<Color>,
    /// 描边颜色（None 表示不描边）
    #[serde(default)]
    pub stroke: Option<Color>,
    /// 描边宽度
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
    /// 不透明度 (0.0-1.0)
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    /// 虚线图案。空数组是实线；奇数长度在绘制时按 SVG 规则重复一遍。
    /// 例如 `[8, 4]` 是虚线，`[2, 4]` 是点线。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stroke_dash: Vec<f32>,
    /// 线帽。缺省平头。
    #[serde(default, skip_serializing_if = "cap_is_butt")]
    pub stroke_cap: StrokeCap,
    /// 线结。缺省斜接。
    #[serde(default, skip_serializing_if = "join_is_miter")]
    pub stroke_join: StrokeJoin,
}

impl ElementStyle {
    /// 创建默认样式
    pub fn new() -> Self {
        Self {
            fill: Some(Color::BLACK),
            stroke: None,
            stroke_width: 1.0,
            opacity: 1.0,
            stroke_dash: Vec::new(),
            stroke_cap: StrokeCap::Butt,
            stroke_join: StrokeJoin::Miter,
        }
    }

    /// 只有填充
    pub fn fill_only(color: Color) -> Self {
        Self {
            fill: Some(color),
            stroke: None,
            ..Self::new()
        }
    }

    /// 从十六进制创建填充
    pub fn fill_only_hex(hex: &str) -> Result<Self, String> {
        Ok(Self {
            fill: Some(Color::from_hex(hex)?),
            stroke: None,
            ..Self::new()
        })
    }

    /// 只有描边
    pub fn stroke_only(color: Color, width: f32) -> Self {
        Self {
            fill: None,
            stroke: Some(color),
            stroke_width: width,
            ..Self::new()
        }
    }

    /// 从十六进制创建描边
    pub fn stroke_only_hex(hex: &str, width: f32) -> Result<Self, String> {
        Ok(Self {
            fill: None,
            stroke: Some(Color::from_hex(hex)?),
            stroke_width: width,
            ..Self::new()
        })
    }
}

impl Default for ElementStyle {
    fn default() -> Self {
        Self::new()
    }
}

/// 元素 Trait
///
/// # 职责
/// - 获取边界框（用于空间索引和碰撞检测）
/// - 获取类型信息
/// - 序列化支持
pub trait Element {
    /// 获取元素属性
    fn props(&self) -> &ElementProps;

    /// 获取可变元素属性
    fn props_mut(&mut self) -> &mut ElementProps;

    /// 获取边界框（局部坐标系）
    fn local_bounds(&self) -> Rect;

    /// 获取边界框（世界坐标系）
    fn world_bounds(&self) -> Rect {
        let local = self.local_bounds();
        let transform = &self.props().transform;

        // 变换四个角点
        let min = local.min();
        let max = local.max();
        let corners = [
            transform.transform_point(min),
            transform.transform_point(Vec2::new(max.x, min.y)),
            transform.transform_point(max),
            transform.transform_point(Vec2::new(min.x, max.y)),
        ];

        // 计算包围盒
        let mut min_x = corners[0].x;
        let mut min_y = corners[0].y;
        let mut max_x = corners[0].x;
        let mut max_y = corners[0].y;

        for corner in &corners[1..] {
            min_x = min_x.min(corner.x);
            min_y = min_y.min(corner.y);
            max_x = max_x.max(corner.x);
            max_y = max_y.max(corner.y);
        }

        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    /// 获取元素类型名称
    fn type_name(&self) -> &'static str;

    /// 判断点是否在元素内（世界坐标）
    ///
    /// 默认实现把点逆变换到局部空间再测包围盒，因此已经计入 `rotation` 和非均匀缩放。
    fn contains_point(&self, point: Vec2) -> bool {
        // 转换到局部坐标系
        let local_point = self.props().transform.inverse_transform_point(point);
        self.local_bounds().contains(local_point)
    }

    /// 元素旋转角（弧度），与 [`ElementProps::rotation`] 相同。
    fn rotation(&self) -> f32 {
        self.props().rotation()
    }

    /// 设置元素旋转角（弧度）。
    ///
    /// 调用方还要 [`crate::Scene::mark_dirty`]，顶点缓存才会重生成。
    fn set_rotation(&mut self, radians: f32) {
        self.props_mut().set_rotation(radians);
    }

    /// 几何、样式或变换是否需要重新生成顶点
    fn is_dirty(&self) -> bool {
        self.props().is_dirty()
    }

    /// 标记需要重新生成顶点。
    ///
    /// 渲染热路径还要把 id 放进 [`crate::Scene::mark_dirty`]，否则缓存不知道该刷新谁。
    fn mark_dirty(&mut self) {
        self.props_mut().mark_dirty();
    }

    /// 清除脏标记
    fn clear_dirty(&mut self) {
        self.props_mut().clear_dirty();
    }
}
