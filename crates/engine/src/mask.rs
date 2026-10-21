//! 形状遮罩：用一个闭合形状的填充轮廓裁剪目标元素的可见区域。
//!
//! ```text
//! 选中 ≥2 个未锁定叶子
//!        │  最上层作 mask 源（需有填充轮廓）
//!        ▼
//! 其余目标 props.masked_by = mask_id
//!        │  一条 SetMask / ReleaseMask 撤销
//!        ▼
//! 绘制 / PNG：目标像素仅在 mask 填充内保留
//! SVG：clipPath；画布：三角形中心在 mask 内才保留
//! ```
//!
//! 遮罩源本身照常绘制（含自身不透明度 / 混合）。裁剪只作用于目标；
//! 目标自身的 opacity / blend 在裁剪之后仍按原逻辑合成。
//! 删除 mask 或解组拆掉的节点时，清理仍指向它的 `masked_by`。

use crate::command::{Command, CommandHistory};
use crate::element::{Element, ElementId};
use crate::math::Vec2;
use crate::{AnyElement, Scene};
use std::collections::HashSet;

/// 可作为遮罩源的叶子：矩形、圆、多边形、闭合填充路径、画板。
pub fn can_be_mask(scene: &Scene, id: ElementId) -> bool {
    if scene.is_effectively_locked(id) || !scene.contains(id) {
        return false;
    }
    match scene.get(id) {
        Some(AnyElement::Rectangle(_))
        | Some(AnyElement::Circle(_))
        | Some(AnyElement::Polygon(_))
        | Some(AnyElement::Frame(_)) => true,
        Some(AnyElement::Path(path)) => path.is_filled() || path_looks_closed(path),
        _ => false,
    }
}

fn path_looks_closed(path: &crate::element::Path) -> bool {
    let points = path.points();
    if points.len() < 4 {
        return false;
    }
    let first = points[0];
    let last = *points.last().unwrap();
    (first - last).length() <= 1.0e-3
}

/// 世界坐标点是否落在遮罩源的填充轮廓内。
///
/// mask 不存在、不可见时返回 `false`（目标整块隐藏）。
/// 命中用元素自己的 [`Element::contains_point`]（填充盒 / 轮廓），不读描边宽度外扩。
pub fn mask_covers(scene: &Scene, mask_id: ElementId, world: Vec2) -> bool {
    let Some(mask) = scene.get(mask_id) else {
        return false;
    };
    if !scene.is_effectively_visible(mask_id) {
        return false;
    }
    Element::contains_point(mask.as_element(), world)
}

/// 当前选择里可参与「设为遮罩」的目标（非 mask 源侧），以及候选 mask。
///
/// 返回 `(mask_id, target_ids)`。可作 mask 的叶子少于 1、或其余可绑定目标少于 1 时为 `None`。
/// mask 取 `(z_index, id)` 最大者。
pub fn mask_pair_from_selection(
    scene: &Scene,
    selected: &[ElementId],
) -> Option<(ElementId, Vec<ElementId>)> {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for id in selected {
        if !seen.insert(*id) || scene.is_effectively_locked(*id) || !scene.contains(*id) {
            continue;
        }
        // 组 / 画板容器本身不作为「被遮罩目标」叶子；画板可作 mask。
        if matches!(scene.get(*id), Some(AnyElement::Group(_))) {
            continue;
        }
        candidates.push(*id);
    }
    if candidates.len() < 2 {
        return None;
    }
    candidates.sort_by_key(|id| stack_key(scene, *id));
    let mask_id = *candidates.last()?;
    if !can_be_mask(scene, mask_id) {
        return None;
    }
    let targets: Vec<ElementId> = candidates
        .iter()
        .copied()
        .filter(|id| *id != mask_id)
        .collect();
    if targets.is_empty() {
        return None;
    }
    Some((mask_id, targets))
}

fn stack_key(scene: &Scene, id: ElementId) -> (i32, ElementId) {
    let z = scene
        .get(id)
        .map(|el| el.as_element().props().z_index)
        .unwrap_or(0);
    (z, id)
}

/// 选择是否可以「设为遮罩」。
pub fn can_set_mask(scene: &Scene, selected: &[ElementId]) -> bool {
    mask_pair_from_selection(scene, selected).is_some()
}

/// 选择是否可以「解除遮罩」：选中项自身带 `masked_by`，或有人把它当作 mask。
pub fn can_release_mask(scene: &Scene, selected: &[ElementId]) -> bool {
    !release_targets(scene, selected).is_empty()
}

/// 解除时要清空 `masked_by` 的元素 id（去重）。
pub fn release_targets(scene: &Scene, selected: &[ElementId]) -> Vec<ElementId> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let selected_set: HashSet<ElementId> = selected.iter().copied().collect();

    for id in selected {
        if scene.is_effectively_locked(*id) {
            continue;
        }
        if let Some(el) = scene.get(*id) {
            if el.as_element().props().masked_by.is_some() && seen.insert(*id) {
                out.push(*id);
            }
        }
    }
    // 选中的是 mask 源：清掉所有指向它的目标（未锁定）。
    for el in scene.iter() {
        let id = el.id();
        if scene.is_effectively_locked(id) {
            continue;
        }
        if let Some(mask) = el.as_element().props().masked_by {
            if selected_set.contains(&mask) && seen.insert(id) {
                out.push(id);
            }
        }
    }
    out
}

/// 一次可撤销的「设为遮罩」。空选 / 锁定 / 不足两个有效叶子为 no-op。
///
/// 成功时给目标与 mask [`Scene::mark_dirty`]。
pub fn apply_set_mask(
    history: &mut CommandHistory,
    scene: &mut Scene,
    selected: &[ElementId],
) -> bool {
    let Some((mask_id, targets)) = mask_pair_from_selection(scene, selected) else {
        return false;
    };
    let mut befores = Vec::new();
    let mut changed = false;
    for id in &targets {
        let before = scene
            .get(*id)
            .map(|el| el.as_element().props().masked_by)
            .unwrap_or(None);
        if before == Some(mask_id) {
            continue;
        }
        befores.push((*id, before));
        changed = true;
    }
    if !changed {
        return false;
    }
    let mut cmd = SetMaskCommand {
        mask_id,
        entries: befores
            .into_iter()
            .map(|(id, before)| MaskEntry {
                target: id,
                before,
                after: Some(mask_id),
            })
            .collect(),
    };
    cmd.execute(scene);
    history.push_executed(Box::new(cmd));
    true
}

/// 一次可撤销的「解除遮罩」。无可清目标时为 no-op。
pub fn apply_release_mask(
    history: &mut CommandHistory,
    scene: &mut Scene,
    selected: &[ElementId],
) -> bool {
    let targets = release_targets(scene, selected);
    if targets.is_empty() {
        return false;
    }
    let entries: Vec<MaskEntry> = targets
        .into_iter()
        .filter_map(|id| {
            let before = scene.get(id)?.as_element().props().masked_by;
            before.map(|mask| MaskEntry {
                target: id,
                before: Some(mask),
                after: None,
            })
        })
        .collect();
    if entries.is_empty() {
        return false;
    }
    let mut cmd = SetMaskCommand {
        mask_id: entries[0].before.unwrap_or(0),
        entries,
    };
    cmd.execute(scene);
    history.push_executed(Box::new(cmd));
    true
}

#[derive(Debug, Clone)]
struct MaskEntry {
    target: ElementId,
    before: Option<ElementId>,
    after: Option<ElementId>,
}

/// 设为 / 解除遮罩共用一条命令，按条目写入 `masked_by`。
#[derive(Debug)]
pub struct SetMaskCommand {
    mask_id: ElementId,
    entries: Vec<MaskEntry>,
}

impl Command for SetMaskCommand {
    fn execute(&mut self, scene: &mut Scene) {
        for entry in &self.entries {
            apply_masked_by(scene, entry.target, entry.after);
        }
        scene.mark_dirty(self.mask_id);
    }

    fn undo(&mut self, scene: &mut Scene) {
        for entry in &self.entries {
            apply_masked_by(scene, entry.target, entry.before);
        }
        scene.mark_dirty(self.mask_id);
    }

    fn name(&self) -> &'static str {
        if self.entries.iter().all(|e| e.after.is_none()) {
            "ReleaseMask"
        } else {
            "SetMask"
        }
    }
}

fn apply_masked_by(scene: &mut Scene, target: ElementId, mask: Option<ElementId>) {
    {
        let Some(el) = scene.get_mut(target) else {
            return;
        };
        el.as_element_mut().props_mut().masked_by = mask;
    }
    scene.mark_dirty(target);
}

/// 清掉仍指向 `removed` 里 id 的 `masked_by`（删除 / 解组后调用）。
pub fn clear_mask_refs_to(scene: &mut Scene, removed: &HashSet<ElementId>) {
    if removed.is_empty() {
        return;
    }
    let victims: Vec<ElementId> = scene
        .iter()
        .filter_map(|el| {
            let mask = el.as_element().props().masked_by?;
            if removed.contains(&mask) {
                Some(el.id())
            } else {
                None
            }
        })
        .collect();
    for id in victims {
        if let Some(el) = scene.get_mut(id) {
            el.as_element_mut().props_mut().masked_by = None;
        }
        scene.mark_dirty(id);
    }
}

/// 过滤三角形：中心点不在 mask 内则丢掉。用于画布顶点缓存。
pub fn filter_tris_by_mask(
    scene: &Scene,
    mask_id: ElementId,
    verts: &[crate::math::Vertex],
) -> Vec<crate::math::Vertex> {
    if verts.len() < 3 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(verts.len());
    for tri in verts.as_chunks::<3>().0 {
        let cx = (tri[0].position[0] + tri[1].position[0] + tri[2].position[0]) / 3.0;
        let cy = (tri[0].position[1] + tri[1].position[1] + tri[2].position[1]) / 3.0;
        if mask_covers(scene, mask_id, Vec2::new(cx, cy)) {
            out.extend_from_slice(tri);
        }
    }
    out
}

/// 过滤线段（每 2 顶点）：中点在 mask 内才保留。
pub fn filter_lines_by_mask(
    scene: &Scene,
    mask_id: ElementId,
    verts: &[crate::math::Vertex],
) -> Vec<crate::math::Vertex> {
    if verts.len() < 2 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(verts.len());
    for pair in verts.as_chunks::<2>().0 {
        let cx = (pair[0].position[0] + pair[1].position[0]) * 0.5;
        let cy = (pair[0].position[1] + pair[1].position[1]) * 0.5;
        if mask_covers(scene, mask_id, Vec2::new(cx, cy)) {
            out.extend_from_slice(pair);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{ElementStyle, Rectangle};
    use crate::export_svg;
    use crate::math::{Color, Vec2};
    use crate::persist::export_selection_png;
    use crate::CommandHistory;

    fn colored_rect(id: ElementId, center: Vec2, size: Vec2, color: Color) -> Rectangle {
        Rectangle::new(id, center, size).with_style(ElementStyle::fill_only(color))
    }

    #[test]
    fn set_mask_clips_lower_and_undo_restores() {
        let mut scene = Scene::new();
        scene.add_rect(colored_rect(
            1,
            Vec2::ZERO,
            Vec2::new(40.0, 40.0),
            Color::RED,
        ));
        scene.add_rect(colored_rect(
            2,
            Vec2::new(5.0, 5.0),
            Vec2::new(20.0, 20.0),
            Color::BLUE,
        ));
        // bump z so circle-like top is mask: id 2 higher by default same z — use id order
        scene
            .get_mut(2)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .z_index = 1;

        let mut history = CommandHistory::new();
        assert!(apply_set_mask(&mut history, &mut scene, &[1, 2]));
        assert_eq!(
            scene.get(1).unwrap().as_element().props().masked_by,
            Some(2)
        );
        assert_eq!(history.undo_count(), 1);

        // Outside mask (large red corner) should not sample after clip.
        assert!(mask_covers(&scene, 2, Vec2::new(5.0, 5.0)));
        assert!(!mask_covers(&scene, 2, Vec2::new(-15.0, -15.0)));

        assert!(history.undo(&mut scene));
        assert_eq!(scene.get(1).unwrap().as_element().props().masked_by, None);
    }

    #[test]
    fn release_mask_and_empty_or_locked_are_noop() {
        let mut scene = Scene::new();
        scene.add_rect(colored_rect(
            1,
            Vec2::ZERO,
            Vec2::new(20.0, 20.0),
            Color::RED,
        ));
        scene.add_rect(colored_rect(
            2,
            Vec2::ZERO,
            Vec2::new(10.0, 10.0),
            Color::BLUE,
        ));
        scene
            .get_mut(2)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .z_index = 2;
        let mut history = CommandHistory::new();
        assert!(!apply_set_mask(&mut history, &mut scene, &[]));
        assert!(!apply_set_mask(&mut history, &mut scene, &[1]));
        assert!(apply_set_mask(&mut history, &mut scene, &[1, 2]));

        scene
            .get_mut(1)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .locked = true;
        assert!(!apply_release_mask(&mut history, &mut scene, &[1]));
        scene
            .get_mut(1)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .locked = false;
        assert!(apply_release_mask(&mut history, &mut scene, &[2])); // mask selected
        assert_eq!(scene.get(1).unwrap().as_element().props().masked_by, None);
    }

    #[test]
    fn old_json_without_masked_by_loads_and_round_trips() {
        let mut scene = Scene::new();
        scene.add_rect(colored_rect(1, Vec2::ZERO, Vec2::new(8.0, 8.0), Color::RED));
        scene.add_rect(colored_rect(
            2,
            Vec2::ZERO,
            Vec2::new(4.0, 4.0),
            Color::GREEN,
        ));
        scene
            .get_mut(2)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .z_index = 1;
        let mut history = CommandHistory::new();
        assert!(apply_set_mask(&mut history, &mut scene, &[1, 2]));

        let json = scene.to_json().unwrap();
        assert!(json.contains("masked_by"));
        let back = Scene::from_json(&json).unwrap();
        assert_eq!(back.get(1).unwrap().as_element().props().masked_by, Some(2));

        let legacy = serde_json::json!({
            "version": 1,
            "next_id": 3,
            "elements": [{
                "type": "rectangle",
                "data": { "props": { "id": 1 }, "size": [8.0, 8.0] }
            }]
        });
        let legacy_scene = Scene::from_json(&legacy.to_string()).unwrap();
        assert_eq!(
            legacy_scene.get(1).unwrap().as_element().props().masked_by,
            None
        );
    }

    #[test]
    fn delete_mask_clears_targets() {
        let mut scene = Scene::new();
        scene.add_rect(colored_rect(
            1,
            Vec2::ZERO,
            Vec2::new(20.0, 20.0),
            Color::RED,
        ));
        scene.add_rect(colored_rect(
            2,
            Vec2::ZERO,
            Vec2::new(10.0, 10.0),
            Color::BLUE,
        ));
        scene
            .get_mut(2)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .z_index = 1;
        let mut history = CommandHistory::new();
        assert!(apply_set_mask(&mut history, &mut scene, &[1, 2]));
        scene.remove(2);
        clear_mask_refs_to(&mut scene, &HashSet::from([2]));
        assert_eq!(scene.get(1).unwrap().as_element().props().masked_by, None);
    }

    #[test]
    fn set_mask_marks_dirty_and_propagates_to_targets() {
        use crate::math::Vertex;
        use crate::vertex_cache::VertexCache;

        let mut scene = Scene::new();
        scene.add_rect(colored_rect(
            1,
            Vec2::ZERO,
            Vec2::new(40.0, 40.0),
            Color::RED,
        ));
        scene.add_rect(colored_rect(
            2,
            Vec2::new(30.0, 0.0),
            Vec2::new(10.0, 10.0),
            Color::BLUE,
        ));
        scene
            .get_mut(2)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .z_index = 1;
        let mut cache = VertexCache::new();
        let _ = cache.sync(&mut scene);

        let mut history = CommandHistory::new();
        assert!(apply_set_mask(&mut history, &mut scene, &[1, 2]));
        assert!(scene.get(1).unwrap().as_element().is_dirty());
        assert!(scene.get(2).unwrap().as_element().is_dirty());
        let _ = cache.sync(&mut scene);

        // Triangle centered far from the offset mask is dropped.
        let outside = [
            Vertex::new(-20.0, -20.0, Color::RED),
            Vertex::new(-10.0, -20.0, Color::RED),
            Vertex::new(-15.0, -10.0, Color::RED),
        ];
        let filtered = filter_tris_by_mask(&scene, 2, &outside);
        assert!(filtered.is_empty());

        // Mask geometry change dirties dependents.
        scene
            .get_mut(2)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .transform
            .translation = Vec2::new(32.0, 0.0);
        scene.mark_dirty(2);
        assert!(scene.get(1).unwrap().as_element().is_dirty());
    }

    #[test]
    fn png_and_svg_reflect_clip() {
        let mut scene = Scene::new();
        scene.add_rect(colored_rect(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(40.0, 40.0),
            Color::new(1.0, 0.0, 0.0, 1.0),
        ));
        scene.add_rect(colored_rect(
            2,
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
            Color::new(0.0, 0.0, 1.0, 1.0),
        ));
        scene
            .get_mut(2)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .z_index = 1;
        scene
            .get_mut(1)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .selected = true;
        scene
            .get_mut(2)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .selected = true;
        let mut history = CommandHistory::new();
        assert!(apply_set_mask(&mut history, &mut scene, &[1, 2]));

        let png = export_selection_png(&scene, 4.0).unwrap();
        assert!(!png.is_empty());

        let svg = export_svg(&scene, false);
        assert!(svg.contains("clipPath") || svg.contains("clip-path"));
    }
}
