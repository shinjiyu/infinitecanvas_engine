//! 场景树快照
//!
//! 给大纲 / 图层面板用的只读层级。完整场景 JSON 导入导出属于 #6；
//! 这里的节点已经派生 `Serialize` / `Deserialize`，#6 可以直接嵌进文档。

use crate::element::ElementId;
use serde::{Deserialize, Serialize};

/// 一棵场景树。根节点是没有父元素的元素，组节点下面挂着子树。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{Rectangle, Scene};
/// use infinitecanvas_math::Vec2;
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
/// scene.add_rect(Rectangle::new(2, Vec2::new(30.0, 0.0), Vec2::new(10.0, 10.0)));
/// let group_id = scene.group(&[1, 2]).unwrap();
/// let tree = scene.scene_tree();
/// assert_eq!(tree.roots.len(), 1);
/// assert_eq!(tree.roots[0].id, group_id);
/// assert_eq!(tree.roots[0].children.len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneTree {
    /// 根节点，按 `z_index` 再按 id 排序。
    pub roots: Vec<SceneTreeNode>,
}

/// 大纲面板上的一个节点。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneTreeNode {
    /// 元素 ID
    pub id: ElementId,
    /// 与 [`crate::Element::type_name`] 相同，例如 `"Rectangle"`、`"Group"`。
    pub element_type: String,
    /// 父元素。根节点为 `None`。
    pub parent_id: Option<ElementId>,
    /// 元素自己的可见性。祖先被隐藏时，命中和绘制还要看有效可见性。
    pub visible: bool,
    /// 是否锁定。
    pub locked: bool,
    /// 层级
    pub z_index: i32,
    /// 直接子节点。叶子为空。
    pub children: Vec<SceneTreeNode>,
}
