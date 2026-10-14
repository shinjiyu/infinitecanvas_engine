//! 复制、粘贴、克隆。
//!
//! ```text
//! 选中根 ──duplicate──▶ 新 ID 子树（同一父节点下的兄弟）
//!                │
//!                ▼
//!         z 按原相对顺序排成一块，整体高于场景里已有元素
//!
//! 选中根 ──copy_elements_json──▶ version 1 场景 JSON（#6 元素编码）
//!                                      │
//!                                      ▼
//!                               paste_json（新 ID，可把包围盒中心对准焦点）
//! ```
//!
//! 组会连同整棵子树一起复制。副本内部的 `parent` / `children` 改写成新 ID。
//! 粘贴时，文档里指向外部的父节点会被丢掉，新元素挂在根上。
//! 原地克隆则保留场景里仍然存在的父组，成为它的新兄弟。
//! 每个新元素都 [`Scene::mark_dirty`](super::Scene::mark_dirty)。

use super::{AnyElement, Scene};
use crate::element::ElementId;
use crate::persist::{PersistError, SceneFile, SCENE_FORMAT_VERSION};
use infinitecanvas_math::Vec2;
use std::collections::{HashMap, HashSet};

impl Scene {
    /// 克隆 `ids` 及其子树。
    ///
    /// 某个 id 的祖先也在列表里时，只随祖先复制一次。不存在的 id 被跳过。
    /// 新元素使用新 ID，样式、可见性、锁定和变换与原元素一致，再整体平移 `offset`
    /// （组的子孙是世界变换，所以每个副本都平移，而不只是根）。
    /// 相对 z 顺序保持不变，整块排在当前场景最大 z 之上，因此画在已有元素上面。
    /// 父节点不在本次副本里、且仍是场景中的组时，新根挂到那个组的末尾（最上层）。
    ///
    /// # 参数
    /// - `ids`: 要克隆的元素。顺序决定返回的根 ID 顺序
    /// - `offset`: 加到每个副本世界平移上的位移。非有限值按 0 处理
    ///
    /// # 返回
    /// 与输入根一一对应的新 ID，不含仅因组子树被带上的后代。没有可克隆元素时为空。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{Rectangle, Scene};
    /// use infinitecanvas_math::Vec2;
    ///
    /// let mut scene = Scene::new();
    /// scene.add_rect(Rectangle::new(1, Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)));
    /// let copies = scene.duplicate(&[1], Vec2::new(10.0, 10.0));
    /// assert_eq!(copies.len(), 1);
    /// assert_ne!(copies[0], 1);
    /// let moved = scene.get(copies[0]).unwrap().as_element().props().transform.translation;
    /// assert_eq!(moved, Vec2::new(10.0, 10.0));
    /// ```
    pub fn duplicate(&mut self, ids: &[ElementId], offset: Vec2) -> Vec<ElementId> {
        let roots = self.duplicate_roots(ids);
        if roots.is_empty() {
            return Vec::new();
        }
        let elements = self.clone_forest(&roots);
        self.spawn_cloned(elements, finite_vec(offset), true)
    }

    /// 把 `ids` 写成与 [`Scene::to_json`](super::Scene::to_json) 相同的 version 1 JSON。
    ///
    /// 只包含这些根和它们的子孙。指向文档外的 `parent` 写成 `null`，组的 `children`
    /// 只保留文档内的 ID，这样粘贴到另一场景时父子关系仍然闭合。`selected` 写成 false。
    /// 元素按 id 升序排列。
    ///
    /// # 参数
    /// - `ids`: 要写入剪贴板的元素。祖先也在列表里时不单独再写一遍
    ///
    /// # 返回
    /// 紧凑 JSON。没有任何元素时仍是合法的空场景文档。
    ///
    /// # 错误
    /// 序列化失败时返回 [`PersistError::Json`]
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{Rectangle, Scene};
    /// use infinitecanvas_math::Vec2;
    ///
    /// let mut scene = Scene::new();
    /// scene.add_rect(Rectangle::new(1, Vec2::new(4.0, 6.0), Vec2::new(2.0, 2.0)));
    /// let json = scene.copy_elements_json(&[1]).unwrap();
    /// let ids = scene.paste_json(&json, Vec2::ZERO, None).unwrap();
    /// assert_eq!(ids.len(), 1);
    /// assert_ne!(ids[0], 1);
    /// ```
    pub fn copy_elements_json(&self, ids: &[ElementId]) -> Result<String, PersistError> {
        let roots = self.duplicate_roots(ids);
        let mut elements = self.clone_forest(&roots);
        let included: HashSet<ElementId> = elements.iter().map(|element| element.id()).collect();
        for element in &mut elements {
            let parent = element.as_element().props().parent;
            if parent.is_some_and(|parent_id| !included.contains(&parent_id)) {
                element.as_element_mut().props_mut().parent = None;
            }
            element.as_element_mut().props_mut().selected = false;
            if let Some(group) = element.as_group_mut() {
                group.children.retain(|child| included.contains(child));
            }
        }
        elements.sort_by_key(|element| element.id());
        let next_id = elements
            .iter()
            .map(|element| element.id())
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let file = SceneFile {
            version: SCENE_FORMAT_VERSION,
            next_id,
            elements,
        };
        serde_json::to_string(&file).map_err(|err| PersistError::Json(err.to_string()))
    }

    /// 把剪贴板 JSON 插入当前场景。
    ///
    /// 格式与 [`Scene::from_json`](super::Scene::from_json) 相同。每个元素分配新 ID，
    /// 组内 `parent` / `children` 按新 ID 重映射。文档外的父节点不会挂到当前场景里同号的组上。
    /// `focus` 有值时，先把叶子包围盒中心移到该世界坐标，再加上 `offset`
    /// （连续粘贴用 `offset` 把后一次往旁边错开）。`focus` 为空时只应用 `offset`。
    /// 新元素的 z 整块高于当前场景。
    ///
    /// # 参数
    /// - `json`: [`Scene::copy_elements_json`] 或 [`Scene::to_json`](super::Scene::to_json) 的文本
    /// - `offset`: 额外世界位移
    /// - `focus`: 粘贴后叶子包围盒中心要落到的世界坐标。`None` 表示保持文档里的位置
    ///
    /// # 返回
    /// 新的根元素 ID（组只返回组本身）。空文档返回空列表。
    ///
    /// # 错误
    /// 见 [`PersistError`]。非法 JSON 不会改动场景。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{Rectangle, Scene};
    /// use infinitecanvas_math::Vec2;
    ///
    /// let mut source = Scene::new();
    /// source.add_rect(Rectangle::new(1, Vec2::new(0.0, 0.0), Vec2::new(8.0, 8.0)));
    /// let json = source.copy_elements_json(&[1]).unwrap();
    /// let mut scene = Scene::new();
    /// let ids = scene.paste_json(&json, Vec2::new(10.0, 10.0), None).unwrap();
    /// let translation = scene.get(ids[0]).unwrap().as_element().props().transform.translation;
    /// assert_eq!(translation, Vec2::new(10.0, 10.0));
    /// ```
    pub fn paste_json(
        &mut self,
        json: &str,
        offset: Vec2,
        focus: Option<Vec2>,
    ) -> Result<Vec<ElementId>, PersistError> {
        let incoming = Scene::from_json(json)?;
        let mut elements: Vec<AnyElement> = incoming.iter().cloned().collect();
        if elements.is_empty() {
            return Ok(Vec::new());
        }
        elements.sort_by_key(|element| element.id());
        let shift = paste_shift(&elements, offset, focus);
        Ok(self.spawn_cloned(elements, shift, false))
    }

    /// 把已经分配好 ID 的副本插回场景。
    ///
    /// 撤销克隆 / 粘贴后再重做时使用。元素上的 ID、父子关系和 z 保持不变。
    /// 父节点不在这批元素里时，挂到场景中同 ID 的组末尾；那个组已经不在了就把父指针清掉。
    /// 每个元素都会 [`Scene::mark_dirty`](super::Scene::mark_dirty)。
    pub(crate) fn insert_copied(&mut self, elements: &[AnyElement]) {
        if elements.is_empty() {
            return;
        }
        let spawned: HashSet<ElementId> = elements.iter().map(|element| element.id()).collect();
        for element in elements {
            let id = element.id();
            if self.contains(id) {
                continue;
            }
            self.insert_element(element.clone());
            self.mark_dirty(id);
        }
        let mut attachments: Vec<(i32, ElementId, ElementId)> = Vec::new();
        for element in elements {
            let id = element.id();
            let Some(parent) = element.as_element().props().parent else {
                continue;
            };
            if spawned.contains(&parent) {
                continue;
            }
            if self.get(parent).and_then(AnyElement::as_group).is_none() {
                self.set_parent(id, None);
                continue;
            }
            let z_index = self
                .get(id)
                .map(|stored| stored.as_element().props().z_index)
                .unwrap_or(0);
            attachments.push((z_index, id, parent));
        }
        attachments.sort_unstable();
        for (_, id, parent) in attachments {
            self.insert_child(parent, id, usize::MAX);
        }
        self.refresh_copied_bounds(&spawned);
    }

    fn duplicate_roots(&self, ids: &[ElementId]) -> Vec<ElementId> {
        let present: HashSet<ElementId> = ids
            .iter()
            .copied()
            .filter(|id| self.contains(*id))
            .collect();
        let mut roots = Vec::new();
        for id in ids {
            if !present.contains(id) || roots.contains(id) {
                continue;
            }
            if self.has_ancestor_in(*id, &present) {
                continue;
            }
            roots.push(*id);
        }
        roots
    }

    fn clone_forest(&self, roots: &[ElementId]) -> Vec<AnyElement> {
        let mut seen = HashSet::new();
        let mut elements = Vec::new();
        for root in roots {
            for id in self.subtree_ids(*root) {
                if seen.insert(id) {
                    if let Some(element) = self.get(id) {
                        elements.push(element.clone());
                    }
                }
            }
        }
        elements
    }

    fn spawn_cloned(
        &mut self,
        elements: Vec<AnyElement>,
        offset: Vec2,
        keep_external_parent: bool,
    ) -> Vec<ElementId> {
        if elements.is_empty() {
            return Vec::new();
        }
        let mut id_of = HashMap::with_capacity(elements.len());
        for element in &elements {
            id_of.insert(element.id(), self.alloc_id());
        }
        let original_ids: HashSet<ElementId> = id_of.keys().copied().collect();
        let z_of = self.z_block(&elements);
        let mut new_roots = Vec::new();
        let mut spawned = Vec::with_capacity(elements.len());
        for mut element in elements {
            let old_id = element.id();
            let new_id = id_of[&old_id];
            let old_parent = element.as_element().props().parent;
            let parent = remap_parent(
                self,
                old_parent,
                &original_ids,
                &id_of,
                keep_external_parent,
            );
            if old_parent.is_none_or(|parent_id| !original_ids.contains(&parent_id)) {
                new_roots.push(new_id);
            }
            {
                let props = element.as_element_mut().props_mut();
                props.id = new_id;
                props.parent = parent;
                props.selected = false;
                props.z_index = z_of.get(&old_id).copied().unwrap_or(0);
                props.dirty = true;
                props.transform.translate(offset);
            }
            if let Some(group) = element.as_group_mut() {
                group.children = group
                    .children
                    .iter()
                    .filter_map(|child| id_of.get(child).copied())
                    .collect();
            }
            if let AnyElement::Text(text) = &mut element {
                text.detach_raster_for_clone();
            }
            spawned.push(element);
        }
        self.insert_copied(&spawned);
        new_roots
    }

    fn z_block(&self, elements: &[AnyElement]) -> HashMap<ElementId, i32> {
        let mut ranked: Vec<(i32, ElementId)> = elements
            .iter()
            .map(|element| (element.as_element().props().z_index, element.id()))
            .collect();
        ranked.sort_unstable();
        let base = self
            .iter()
            .map(|element| element.as_element().props().z_index)
            .max()
            .unwrap_or(-1);
        let mut z_of = HashMap::with_capacity(ranked.len());
        for (index, (_, old_id)) in ranked.into_iter().enumerate() {
            let step = i32::try_from(index).unwrap_or(i32::MAX);
            z_of.insert(old_id, base.saturating_add(1).saturating_add(step));
        }
        z_of
    }

    fn refresh_copied_bounds(&mut self, spawned: &HashSet<ElementId>) {
        let mut groups: Vec<(usize, ElementId)> = Vec::new();
        for id in spawned {
            if self.get(*id).and_then(AnyElement::as_group).is_none() {
                continue;
            }
            let mut depth = 0usize;
            let mut current = self.parent_of(*id);
            while let Some(parent) = current {
                depth += 1;
                if depth > spawned.len().saturating_add(1) {
                    break;
                }
                current = self.parent_of(parent);
            }
            groups.push((depth, *id));
        }
        groups.sort_unstable_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
        for (_, id) in &groups {
            self.recompute_group_bounds(*id);
        }
        let roots: Vec<ElementId> = spawned
            .iter()
            .copied()
            .filter(|id| match self.parent_of(*id) {
                Some(parent) => !spawned.contains(&parent),
                None => true,
            })
            .collect();
        for root in roots {
            self.refresh_bounds_upwards(root);
        }
    }
}

fn remap_parent(
    scene: &Scene,
    old_parent: Option<ElementId>,
    original_ids: &HashSet<ElementId>,
    id_of: &HashMap<ElementId, ElementId>,
    keep_external_parent: bool,
) -> Option<ElementId> {
    match old_parent {
        Some(parent) if original_ids.contains(&parent) => id_of.get(&parent).copied(),
        Some(parent)
            if keep_external_parent
                && scene.get(parent).and_then(AnyElement::as_group).is_some() =>
        {
            Some(parent)
        }
        _ => None,
    }
}

fn paste_shift(elements: &[AnyElement], offset: Vec2, focus: Option<Vec2>) -> Vec2 {
    let offset = finite_vec(offset);
    let Some(focus) = focus.filter(|point| point.x.is_finite() && point.y.is_finite()) else {
        return offset;
    };
    let Some(center) = forest_center(elements) else {
        return offset;
    };
    finite_vec((focus - center) + offset)
}

fn forest_center(elements: &[AnyElement]) -> Option<Vec2> {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut any = false;
    for element in elements {
        if element.as_group().is_some() {
            continue;
        }
        let bounds = element.as_element().world_bounds();
        if !bounds.width.is_finite() || !bounds.height.is_finite() {
            continue;
        }
        any = true;
        min_x = min_x.min(bounds.x);
        min_y = min_y.min(bounds.y);
        max_x = max_x.max(bounds.right());
        max_y = max_y.max(bounds.bottom());
    }
    if !any {
        return None;
    }
    Some(Vec2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5))
}

fn finite_vec(value: Vec2) -> Vec2 {
    if value.x.is_finite() && value.y.is_finite() {
        value
    } else {
        Vec2::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{ElementStyle, Image, Rectangle, Text, TextAlign};
    use crate::math::Color;
    use crate::{
        AnyElement, CommandHistory, DuplicateElementsCommand, PasteElementsCommand, Scene,
    };

    fn translation(scene: &Scene, id: ElementId) -> Vec2 {
        scene
            .get(id)
            .unwrap()
            .as_element()
            .props()
            .transform
            .translation
    }

    fn z_index(scene: &Scene, id: ElementId) -> i32 {
        scene.get(id).unwrap().as_element().props().z_index
    }

    #[test]
    fn duplicate_assigns_new_ids_matching_attrs_and_stacks_above() {
        let mut scene = Scene::new();
        let low = Rectangle::new(1, Vec2::new(0.0, 0.0), Vec2::new(10.0, 6.0))
            .with_style(ElementStyle::stroke_only(Color::RED, 3.0));
        let high = Rectangle::new(2, Vec2::new(30.0, 4.0), Vec2::new(8.0, 8.0))
            .with_style(ElementStyle::fill_only(Color::BLUE));
        let above = Rectangle::new(3, Vec2::new(80.0, 0.0), Vec2::new(4.0, 4.0));
        scene.add_rect(low);
        scene.add_rect(high);
        scene.add_rect(above);
        scene
            .get_mut(1)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .z_index = 1;
        scene
            .get_mut(2)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .z_index = 4;
        scene
            .get_mut(3)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .z_index = 9;
        scene
            .get_mut(2)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .transform
            .rotation = 0.4;

        assert!(!scene.has_dirty_elements());
        let copies = scene.duplicate(&[2, 1, 99], Vec2::new(10.0, 10.0));
        assert_eq!(copies.len(), 2);
        assert_ne!(copies[0], 2);
        assert_ne!(copies[1], 1);
        assert!(scene.contains(1) && scene.contains(2) && scene.contains(3));
        assert_eq!(scene.count(), 5);
        assert_eq!(z_index(&scene, 1), 1);
        assert_eq!(z_index(&scene, 2), 4);
        assert_eq!(z_index(&scene, 3), 9);
        assert!(z_index(&scene, copies[0]) > 9);
        assert!(z_index(&scene, copies[1]) > 9);
        assert!(z_index(&scene, copies[1]) < z_index(&scene, copies[0]));

        let AnyElement::Rectangle(copy_high) = scene.get(copies[0]).unwrap() else {
            panic!("high copy should stay a rectangle");
        };
        assert_eq!(copy_high.size, Vec2::new(8.0, 8.0));
        assert_eq!(copy_high.style.fill, Some(Color::BLUE));
        assert!((copy_high.props.transform.rotation - 0.4).abs() < 1e-5);
        assert_eq!(copy_high.props.transform.translation, Vec2::new(40.0, 14.0));
        assert!(!copy_high.props.selected);

        let AnyElement::Rectangle(copy_low) = scene.get(copies[1]).unwrap() else {
            panic!("low copy should stay a rectangle");
        };
        assert_eq!(copy_low.style.stroke, Some(Color::RED));
        assert!((copy_low.style.stroke_width - 3.0).abs() < 1e-5);
        assert_eq!(copy_low.props.transform.translation, Vec2::new(10.0, 10.0));

        let dirty: HashSet<ElementId> = scene.take_dirty_ids().into_iter().collect();
        assert!(dirty.contains(&copies[0]) && dirty.contains(&copies[1]));
        assert!(!dirty.contains(&1) && !dirty.contains(&2) && !dirty.contains(&3));
    }

    #[test]
    fn duplicate_group_remaps_subtree_and_keeps_sibling_parent() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(40.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            3,
            Vec2::new(0.0, 40.0),
            Vec2::new(10.0, 10.0),
        ));
        let inner = scene.group(&[1, 2]).unwrap();
        let outer = scene.group(&[inner, 3]).unwrap();
        let before = scene.count();

        let copies = scene.duplicate(&[outer, 1], Vec2::new(10.0, 0.0));
        assert_eq!(copies.len(), 1);
        // 子树是 outer、inner、矩形 1、矩形 2、矩形 3。
        assert_eq!(scene.count(), before + 5);
        let new_outer = copies[0];
        assert_ne!(new_outer, outer);
        assert!(scene.parent_of(new_outer).is_none());
        let outer_children = scene.children_of(new_outer);
        assert_eq!(outer_children.len(), 2);
        let new_inner = *outer_children
            .iter()
            .find(|id| scene.get(**id).unwrap().as_group().is_some())
            .unwrap();
        assert_eq!(scene.parent_of(new_inner), Some(new_outer));
        let leaves = scene.children_of(new_inner);
        assert_eq!(leaves.len(), 2);
        assert!(leaves
            .iter()
            .all(|id| scene.parent_of(*id) == Some(new_inner)));
        assert_eq!(translation(&scene, leaves[0]), Vec2::new(10.0, 0.0));
        assert_eq!(translation(&scene, leaves[1]), Vec2::new(50.0, 0.0));
        assert_eq!(scene.children_of(inner), vec![1, 2]);
        assert_eq!(translation(&scene, 1), Vec2::ZERO);

        let mut history = CommandHistory::new();
        history.execute(
            Box::new(DuplicateElementsCommand::new(vec![1], Vec2::new(5.0, 0.0))),
            &mut scene,
        );
        let nested = scene
            .children_of(inner)
            .into_iter()
            .find(|id| *id != 1 && *id != 2)
            .unwrap();
        assert_eq!(scene.parent_of(nested), Some(inner));
        assert_eq!(scene.children_of(inner), vec![1, 2, nested]);
        assert_eq!(translation(&scene, nested), Vec2::new(5.0, 0.0));
        history.undo(&mut scene);
        assert_eq!(scene.children_of(inner), vec![1, 2]);
        assert!(!scene.contains(nested));
        history.redo(&mut scene);
        assert!(scene.contains(nested));
        assert_eq!(scene.children_of(inner), vec![1, 2, nested]);
    }

    #[test]
    fn paste_json_round_trip_matches_attrs_and_centers_on_focus() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0))
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(20.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        scene
            .get_mut(1)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .z_index = 2;
        scene
            .get_mut(2)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .z_index = 5;
        scene
            .get_mut(2)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .locked = true;

        let json = scene.copy_elements_json(&[1, 2]).unwrap();
        assert!(json.contains("\"type\":\"rectangle\""));
        let restored = Scene::from_json(&json).unwrap();
        assert_eq!(restored.count(), 2);

        // 并集是 x=-5..25, y=-5..5，中心 (10, 0)。焦点 (100, 50) 的位移是 (90, 50)。
        let centered = scene
            .paste_json(&json, Vec2::ZERO, Some(Vec2::new(100.0, 50.0)))
            .unwrap();
        assert_eq!(centered.len(), 2);
        assert_eq!(translation(&scene, centered[0]), Vec2::new(90.0, 50.0));
        assert_eq!(translation(&scene, centered[1]), Vec2::new(110.0, 50.0));
        assert!(z_index(&scene, centered[0]) > 5);
        assert!(z_index(&scene, centered[1]) > z_index(&scene, centered[0]));
        assert!(scene.get(centered[1]).unwrap().as_element().props().locked);
        let AnyElement::Rectangle(copy) = scene.get(centered[0]).unwrap() else {
            panic!("rectangle");
        };
        assert_eq!(copy.style.fill, Some(Color::RED));
        assert_eq!(copy.size, Vec2::new(10.0, 10.0));

        let nudged = scene
            .paste_json(&json, Vec2::new(10.0, 10.0), None)
            .unwrap();
        assert_eq!(translation(&scene, nudged[0]), Vec2::new(10.0, 10.0));
        assert_eq!(translation(&scene, nudged[1]), Vec2::new(30.0, 10.0));
        assert!(scene.paste_json("{", Vec2::ZERO, None).is_err());
        assert_eq!(scene.count(), 6);
    }

    #[test]
    fn duplicate_text_drops_raster_and_keeps_wording() {
        let mut scene = Scene::new();
        let mut text = Text::new(1, Vec2::new(10.0, 20.0), "Hello").with_align(TextAlign::Center);
        text.texture_id = Some("text-1-r1-l0".to_string());
        text.raster_lod = Some(0);
        scene.add_text(text);
        scene.add_image(Image::new(
            2,
            Vec2::new(0.0, 0.0),
            Vec2::new(16.0, 16.0),
            "badge".to_string(),
        ));
        let json = scene.copy_elements_json(&[1, 2]).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"texture_id\":\"badge\""));
        assert!(!json.contains("text-1-r1-l0"));

        let copies = scene.duplicate(&[1, 2], Vec2::new(1.0, 2.0));
        let AnyElement::Text(copy) = scene.get(copies[0]).unwrap() else {
            panic!("text stays text");
        };
        assert_eq!(copy.content, "Hello");
        assert_eq!(copy.align, TextAlign::Center);
        assert!(copy.texture_id.is_none());
        assert_eq!(copy.props.transform.translation, Vec2::new(11.0, 22.0));
        let AnyElement::Image(image) = scene.get(copies[1]).unwrap() else {
            panic!("image stays image");
        };
        assert_eq!(image.texture_id, "badge");
    }

    #[test]
    fn duplicate_and_paste_commands_undo_and_redo() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(30.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let group_id = scene.group(&[1, 2]).unwrap();
        let json = scene.copy_elements_json(&[1]).unwrap();
        assert!(json.contains("\"parent\":null") || !json.contains("\"parent\""));

        let mut history = CommandHistory::new();
        history.execute(
            Box::new(DuplicateElementsCommand::new(
                vec![group_id],
                Vec2::new(10.0, 10.0),
            )),
            &mut scene,
        );
        assert_eq!(scene.count(), 6);
        let copy_root = scene
            .iter()
            .map(|element| element.id())
            .find(|id| *id != group_id && *id != 1 && *id != 2 && scene.parent_of(*id).is_none())
            .unwrap();
        assert!(scene.get(copy_root).unwrap().as_element().props().selected);

        assert!(history.undo(&mut scene));
        assert_eq!(scene.count(), 3);
        assert!(scene.get(copy_root).is_none());
        assert_eq!(scene.children_of(group_id), vec![1, 2]);
        assert!(history.redo(&mut scene));
        assert!(scene.contains(copy_root));
        assert_eq!(scene.count(), 6);
        let moved: Vec<Vec2> = scene
            .children_of(copy_root)
            .into_iter()
            .map(|id| translation(&scene, id))
            .collect();
        assert!(moved.contains(&Vec2::new(10.0, 10.0)));
        assert!(moved.contains(&Vec2::new(40.0, 10.0)));

        history.execute(
            Box::new(PasteElementsCommand::new(json, Vec2::new(10.0, 10.0), None)),
            &mut scene,
        );
        assert!(scene.count() > 6);
        let after_paste = scene.count();
        assert!(history.undo(&mut scene));
        assert_eq!(scene.count(), 6);
        assert!(history.redo(&mut scene));
        assert_eq!(scene.count(), after_paste);
    }
}
