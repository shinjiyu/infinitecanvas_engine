//! 分组容器
//!
//! 组不产生顶点。子元素 ID 按大纲顺序放在 `children` 里，
//! 反向指针在 [`super::ElementProps::parent`]。
//! 子元素的 `transform` 始终是世界变换；组被移动、旋转或缩放时，
//! 由 [`crate::Scene`] 把增量写到每个后代上。

use super::{Element, ElementId, ElementProps};
use infinitecanvas_math::{Rect, Vec2};
use serde::{Deserialize, Serialize};

/// 分组元素。
///
/// # 职责
/// - 持有直接子元素 ID（顺序即大纲面板顺序）
/// - 用 `local_bounds` 记录子内容在组局部空间的包围盒，供选中框使用
/// - 自身不参与命中：点选落到子元素上
///
/// # 示例
/// ```
/// use infinitecanvas_engine::element::Group;
/// use infinitecanvas_engine::Element;
///
/// let mut group = Group::new(3);
/// group.children = vec![1, 2];
/// assert_eq!(group.child_ids(), &[1, 2]);
/// assert_eq!(group.type_name(), "Group");
/// ```
fn default_group_bounds() -> Rect {
    Rect::new(0.0, 0.0, 0.0, 0.0)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    /// 元素属性。`transform` 是组坐标系在世界中的位姿，平移默认落在子内容中心。
    pub props: ElementProps,
    /// 直接子元素，顺序即场景树顺序。JSON 里可以省略，缺省为空。
    #[serde(default)]
    pub children: Vec<ElementId>,
    /// 子内容在组局部坐标中的轴对齐包围盒。场景在结构或变换变化后刷新。
    ///
    /// 旧文档或手写 JSON 可以省略，导入后由场景在下一次变换时重算。
    #[serde(default = "default_group_bounds")]
    pub local_bounds: Rect,
}

impl Group {
    /// 创建空组。子元素和包围盒由 [`crate::Scene::group`] 填上。
    ///
    /// # 参数
    /// - `id`: 元素 ID
    pub fn new(id: ElementId) -> Self {
        Self {
            props: ElementProps::new(id),
            children: Vec::new(),
            local_bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    /// 直接子元素 ID。
    pub fn child_ids(&self) -> &[ElementId] {
        &self.children
    }
}

impl Element for Group {
    fn props(&self) -> &ElementProps {
        &self.props
    }

    fn props_mut(&mut self) -> &mut ElementProps {
        &mut self.props
    }

    fn local_bounds(&self) -> Rect {
        self.local_bounds
    }

    fn type_name(&self) -> &'static str {
        "Group"
    }

    /// 组没有自己的填充。命中测试继续找子元素，避免组的包围盒抢走点击。
    fn contains_point(&self, _point: Vec2) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_does_not_steal_hits() {
        let group = Group::new(1);
        assert!(!group.contains_point(Vec2::ZERO));
        assert!(!group.contains_point(Vec2::new(10.0, 10.0)));
    }

    #[test]
    fn group_serde_round_trip() {
        let mut group = Group::new(7);
        group.children = vec![1, 2, 3];
        group.props.locked = true;
        group.local_bounds = Rect::new(-4.0, -2.0, 8.0, 4.0);
        let json = serde_json::to_string(&group).expect("serialize");
        let back: Group = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.children, vec![1, 2, 3]);
        assert!(back.props.locked);
        assert_eq!(back.props.parent, None);
        assert_eq!(back.local_bounds, group.local_bounds);
    }
}
