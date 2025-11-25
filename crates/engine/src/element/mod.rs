///! 元素定义
///!
///! 提供无限画布上的各种图形元素

pub mod circle;
pub mod rect;

pub use circle::Circle;
pub use rect::Rectangle;

use infinitecanvas_math::{Rect, Transform, Vec2};
use serde::{Deserialize, Serialize};

/// 元素 ID 类型
pub type ElementId = u64;

/// 元素通用属性
///
/// # 职责
/// - 唯一标识
/// - 位置和变换
/// - 可见性和选中状态
/// - 层级顺序
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementProps {
    /// 唯一 ID
    pub id: ElementId,
    /// 变换（位置、旋转、缩放）
    pub transform: Transform,
    /// 是否可见
    pub visible: bool,
    /// 是否选中
    pub selected: bool,
    /// 层级（z-index）
    pub z_index: i32,
}

impl ElementProps {
    /// 创建元素属性
    pub fn new(id: ElementId) -> Self {
        Self {
            id,
            transform: Transform::identity(),
            visible: true,
            selected: false,
            z_index: 0,
        }
    }
}

/// 元素样式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementStyle {
    /// 填充颜色（None 表示不填充）
    pub fill: Option<String>,
    /// 描边颜色（None 表示不描边）
    pub stroke: Option<String>,
    /// 描边宽度
    pub stroke_width: f32,
    /// 不透明度 (0.0-1.0)
    pub opacity: f32,
}

impl ElementStyle {
    /// 创建默认样式
    pub fn new() -> Self {
        Self {
            fill: Some("#000000".to_string()),
            stroke: None,
            stroke_width: 1.0,
            opacity: 1.0,
        }
    }

    /// 只有填充
    pub fn fill_only(color: &str) -> Self {
        Self {
            fill: Some(color.to_string()),
            stroke: None,
            stroke_width: 1.0,
            opacity: 1.0,
        }
    }

    /// 只有描边
    pub fn stroke_only(color: &str, width: f32) -> Self {
        Self {
            fill: None,
            stroke: Some(color.to_string()),
            stroke_width: width,
            opacity: 1.0,
        }
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
    fn contains_point(&self, point: Vec2) -> bool {
        // 转换到局部坐标系
        let local_point = self.props().transform.inverse_transform_point(point);
        self.local_bounds().contains(local_point)
    }
}
