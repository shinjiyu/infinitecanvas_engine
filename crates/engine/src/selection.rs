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
    pub fn select_element(&mut self, scene: &mut Scene, id: ElementId, additive: bool) -> bool {
        if !scene.contains(id) {
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
        true
    }

    /// Remove an element without touching the scene (used when elements are deleted).
    pub fn remove_id(&mut self, id: ElementId) {
        self.selected.remove(&id);
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
            let bounds = element.as_element().world_bounds();
            if rect.contains_rect(&bounds) || rect.intersects(&bounds) {
                hits.push(element.id());
            }
        }

        for id in &hits {
            if self.selected.insert(*id) {
                if let Some(element) = scene.get_mut(*id) {
                    element.as_element_mut().props_mut().selected = true;
                }
            }
        }

        hits
    }

    /// 按元素上的 `selected` 标记重建选择集
    ///
    /// 导入场景后调用。不修改元素本身的标记。
    ///
    /// # 参数
    /// - `scene`: 已载入的场景
    pub fn sync_from_scene(&mut self, scene: &Scene) {
        self.selected.clear();
        for element in scene.iter() {
            if element.as_element().props().selected {
                self.selected.insert(element.id());
            }
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
    fn sync_from_scene_reads_selected_flags() {
        let mut scene = Scene::new();
        let mut selected = Rectangle::new(1, Vec2::ZERO, Vec2::ONE);
        selected.props.selected = true;
        scene.add_rect(selected);
        scene.add_rect(Rectangle::new(2, Vec2::ZERO, Vec2::ONE));

        let mut selection = SelectionManager::new();
        selection.sync_from_scene(&scene);
        assert!(selection.is_selected(1));
        assert!(!selection.is_selected(2));
        assert_eq!(selection.len(), 1);
    }
}
