//! Selection manager keeps track of selected element IDs and syncs the `selected` flag
//! within element properties.

use std::collections::HashSet;

use crate::{ElementId, Scene};
use infinitecanvas_math::Rect;

/// Manages currently selected elements.
#[derive(Default, Debug)]
pub struct SelectionManager {
    selected: HashSet<ElementId>,
}

impl SelectionManager {
    pub fn new() -> Self {
        Self {
            selected: HashSet::new(),
        }
    }

    /// Select a single element. When `additive` is false, previous selection is cleared.
    ///
    /// Locked elements, and elements under a locked ancestor, are rejected. The previous
    /// selection is left unchanged in that case.
    pub fn select_element(&mut self, scene: &mut Scene, id: ElementId, additive: bool) -> bool {
        if !scene.contains(id) || scene.is_effectively_locked(id) {
            return false;
        }

        let mut changed = false;
        if !additive {
            changed |= self.clear(scene);
        }

        if self.selected.insert(id) {
            if let Some(element) = scene.get_mut(id) {
                element.as_element_mut().props_mut().selected = true;
            }
            scene.bump_selection_epoch();
            changed = true;
        }

        changed
    }

    /// Deselect a specific element.
    pub fn deselect(&mut self, scene: &mut Scene, id: ElementId) -> bool {
        if self.selected.remove(&id) {
            if let Some(element) = scene.get_mut(id) {
                element.as_element_mut().props_mut().selected = false;
            }
            scene.bump_selection_epoch();
            true
        } else {
            false
        }
    }

    /// Clear the current selection.
    pub fn clear(&mut self, scene: &mut Scene) -> bool {
        if self.selected.is_empty() {
            return false;
        }

        let ids: Vec<ElementId> = self.selected.drain().collect();
        for id in ids {
            if let Some(element) = scene.get_mut(id) {
                element.as_element_mut().props_mut().selected = false;
            }
        }
        scene.bump_selection_epoch();
        true
    }

    /// Remove an element without touching the scene (used when elements are deleted).
    pub fn remove_id(&mut self, id: ElementId) {
        self.selected.remove(&id);
    }

    /// 选中顶层的可见元素。
    ///
    /// 组只选中组本身，不选中它的子孙。元素不可见、祖先被隐藏，或自身及祖先被锁定时，不选。
    /// 先清空当前选择，再逐个选中。返回选中数量。
    ///
    /// # 参数
    /// - `scene`: 要选择的场景
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{Rectangle, Scene, SelectionManager};
    /// use infinitecanvas_math::Vec2;
    /// let mut scene = Scene::new();
    /// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::ONE));
    /// let mut selection = SelectionManager::new();
    /// assert_eq!(selection.select_all(&mut scene), 1);
    /// ```
    pub fn select_all(&mut self, scene: &mut Scene) -> usize {
        let ids: Vec<ElementId> = scene
            .iter()
            .filter(|element| {
                let id = element.id();
                scene.parent_of(id).is_none()
                    && scene.is_effectively_visible(id)
                    && !scene.is_effectively_locked(id)
            })
            .map(|element| element.id())
            .collect();
        self.clear(scene);
        for id in ids {
            self.select_element(scene, id, true);
        }
        self.len()
    }

    /// Returns a sorted list of selected IDs.
    pub fn selected_ids(&self) -> Vec<ElementId> {
        let mut ids: Vec<ElementId> = self.selected.iter().copied().collect();
        ids.sort_unstable();
        ids
    }

    pub fn select_in_rect(
        &mut self,
        scene: &mut Scene,
        rect: Rect,
        additive: bool,
    ) -> Vec<ElementId> {
        const MIN_SIZE: f32 = 2.0;
        if rect.width.abs() < MIN_SIZE || rect.height.abs() < MIN_SIZE {
            if !additive {
                self.clear(scene);
            }
            return Vec::new();
        }

        if !additive {
            self.clear(scene);
        }

        let mut hits = Vec::new();
        for element in scene.iter() {
            if scene.is_effectively_locked(element.id()) {
                continue;
            }
            let bounds = element.as_element().world_bounds();
            if rect.contains_rect(&bounds) || rect.intersects(&bounds) {
                hits.push(element.id());
            }
        }

        let mut changed = false;
        for id in &hits {
            if self.selected.insert(*id) {
                if let Some(element) = scene.get_mut(*id) {
                    element.as_element_mut().props_mut().selected = true;
                }
                changed = true;
            }
        }
        if changed {
            scene.bump_selection_epoch();
        }

        hits
    }

    /// 按元素上的 `selected` 标记重建选择集
    ///
    /// 导入场景后调用。锁定的元素（含被锁定祖先挡住的子孙）会清掉 `selected`，
    /// 不能留在选择集里。
    ///
    /// # 参数
    /// - `scene`: 已载入的场景
    pub fn sync_from_scene(&mut self, scene: &mut Scene) {
        self.selected.clear();
        let flagged: Vec<ElementId> = scene
            .iter()
            .filter(|element| element.as_element().props().selected)
            .map(|element| element.id())
            .collect();
        for id in flagged {
            if scene.is_effectively_locked(id) {
                if let Some(element) = scene.get_mut(id) {
                    element.as_element_mut().props_mut().selected = false;
                }
                continue;
            }
            self.selected.insert(id);
        }
    }

    pub fn len(&self) -> usize {
        self.selected.len()
    }

    /// Returns true when no elements are selected.
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    pub fn is_selected(&self, id: ElementId) -> bool {
        self.selected.contains(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{element::Rectangle, Scene};
    use infinitecanvas_math::{Rect, Vec2};

    #[test]
    fn basic_selection_flow() {
        let mut scene = Scene::new();
        let rect = Rectangle::new(1, Vec2::ZERO, Vec2::ONE);
        scene.add_rect(rect);

        let mut selection = SelectionManager::new();
        assert!(selection.select_element(&mut scene, 1, false));
        assert!(selection.is_selected(1));
        assert_eq!(selection.len(), 1);

        assert!(selection.deselect(&mut scene, 1));
        assert!(!selection.is_selected(1));
    }

    #[test]
    fn additive_selection() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::ONE));
        scene.add_rect(Rectangle::new(2, Vec2::ZERO, Vec2::ONE));

        let mut selection = SelectionManager::new();
        selection.select_element(&mut scene, 1, false);
        selection.select_element(&mut scene, 2, true);

        let mut ids = selection.selected_ids();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2]);

        selection.clear(&mut scene);
        assert_eq!(selection.len(), 0);
    }

    #[test]
    fn select_in_rect_picks_elements() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(20.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));

        let mut selection = SelectionManager::new();
        let rect = Rect::new(-5.0, -5.0, 20.0, 20.0);
        let hits = selection.select_in_rect(&mut scene, rect, false);
        assert_eq!(hits, vec![1]);
        assert!(selection.is_selected(1));
        assert!(!selection.is_selected(2));
    }

    #[test]
    fn select_all_selects_every_element() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::ONE));
        scene.add_rect(Rectangle::new(2, Vec2::new(20.0, 0.0), Vec2::ONE));

        let mut selection = SelectionManager::new();
        selection.select_element(&mut scene, 1, false);
        assert_eq!(selection.select_all(&mut scene), 2);
        assert!(selection.is_selected(1));
        assert!(selection.is_selected(2));
        assert!(scene.get(2).unwrap().as_element().props().selected);

        let mut empty = Scene::new();
        assert_eq!(selection.select_all(&mut empty), 0);
        assert!(selection.is_empty());
    }

    #[test]
    fn select_all_selects_visible_top_level_only() {
        let mut scene = Scene::new();
        let left = scene.next_id();
        scene.add_rect(Rectangle::new(left, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        let right = scene.next_id();
        scene.add_rect(Rectangle::new(
            right,
            Vec2::new(30.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let hidden_id = scene.next_id();
        let mut hidden = Rectangle::new(hidden_id, Vec2::new(80.0, 0.0), Vec2::new(10.0, 10.0));
        hidden.props.visible = false;
        scene.add_rect(hidden);
        let group_id = scene.group(&[left, right]).unwrap();
        let top = scene.next_id();
        scene.add_rect(Rectangle::new(
            top,
            Vec2::new(120.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));

        let mut selection = SelectionManager::new();
        selection.select_element(&mut scene, hidden_id, false);
        assert_eq!(selection.select_all(&mut scene), 2);
        assert!(selection.is_selected(group_id));
        assert!(selection.is_selected(top));
        assert!(!selection.is_selected(left));
        assert!(!selection.is_selected(right));
        assert!(!selection.is_selected(hidden_id));
        assert!(scene.get(group_id).unwrap().as_element().props().selected);
        assert!(!scene.get(left).unwrap().as_element().props().selected);
    }

    #[test]
    fn select_all_skips_effectively_locked() {
        let mut scene = Scene::new();
        let open = scene.next_id();
        scene.add_rect(Rectangle::new(open, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        let locked_id = scene.next_id();
        scene.add_rect(Rectangle::new(
            locked_id,
            Vec2::new(30.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        scene.set_locked(locked_id, true);
        let child = scene.next_id();
        scene.add_rect(Rectangle::new(
            child,
            Vec2::new(80.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let sibling = scene.next_id();
        scene.add_rect(Rectangle::new(
            sibling,
            Vec2::new(110.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let group_id = scene.group(&[child, sibling]).unwrap();
        scene.set_locked(group_id, true);
        let free = scene.next_id();
        scene.add_rect(Rectangle::new(
            free,
            Vec2::new(160.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));

        let mut selection = SelectionManager::new();
        assert_eq!(selection.select_all(&mut scene), 2);
        assert!(selection.is_selected(open));
        assert!(selection.is_selected(free));
        assert!(!selection.is_selected(locked_id));
        assert!(!selection.is_selected(group_id));
        assert!(!selection.is_selected(child));
        assert!(!selection.is_selected(sibling));
        assert!(!scene.get(locked_id).unwrap().as_element().props().selected);
        assert!(!scene.get(group_id).unwrap().as_element().props().selected);
    }

    #[test]
    fn sync_from_scene_reads_selected_flags() {
        let mut scene = Scene::new();
        let mut selected = Rectangle::new(1, Vec2::ZERO, Vec2::ONE);
        selected.props.selected = true;
        scene.add_rect(selected);
        scene.add_rect(Rectangle::new(2, Vec2::ZERO, Vec2::ONE));

        let mut selection = SelectionManager::new();
        selection.sync_from_scene(&mut scene);
        assert!(selection.is_selected(1));
        assert!(!selection.is_selected(2));
        assert_eq!(selection.len(), 1);
    }

    #[test]
    fn locked_elements_cannot_be_selected_or_marquee_picked() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(40.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        scene.set_locked(1, true);

        let mut selection = SelectionManager::new();
        assert!(!selection.select_element(&mut scene, 1, false));
        assert!(selection.is_empty());
        assert!(!scene.get(1).unwrap().as_element().props().selected);
        assert!(selection.select_element(&mut scene, 2, false));

        let hits = selection.select_in_rect(&mut scene, Rect::new(-20.0, -20.0, 80.0, 40.0), false);
        assert!(!hits.contains(&1));
        assert!(hits.contains(&2));
        assert!(!selection.is_selected(1));

        scene.set_locked(1, false);
        let group_id = scene.group(&[1, 2]).unwrap();
        scene.set_locked(group_id, true);
        selection.clear(&mut scene);
        assert!(!selection.select_element(&mut scene, 2, false));
        assert!(!selection.select_element(&mut scene, group_id, false));
    }

    #[test]
    fn sync_from_scene_drops_locked_selection() {
        let mut scene = Scene::new();
        let mut locked = Rectangle::new(1, Vec2::ZERO, Vec2::ONE);
        locked.props.selected = true;
        locked.props.locked = true;
        scene.add_rect(locked);

        let mut selection = SelectionManager::new();
        selection.sync_from_scene(&mut scene);
        assert!(selection.is_empty());
        assert!(!scene.get(1).unwrap().as_element().props().selected);
    }
}
