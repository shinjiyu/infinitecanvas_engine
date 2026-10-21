//! Path 节点编辑（锚点与可选贝塞尔控制柄）。
//!
//! ```text
//! 选中单一 Path ──► path_node_mode / 双击
//!                      │
//!                      ▼
//!              拖拽锚点或控制柄 ──► PathGeometry
//!                      │
//!                      ▼
//!              EditPathCommand ──► mark_dirty ──► 顶点 / 命中 / 导出
//! ```
//!
//! 旧文档只有 `points` / `shapes`，没有 `point_handles`，仍可加载。
//! 没有控制柄时几何与原先折线一致。空选、锁定、非 Path、无变化不入历史。

use crate::element::{ElementId, FillShape, Path, PathPointHandles};
use crate::scene::{AnyElement, Scene};
use crate::{math::Vec2, Command};

/// 节点在路径上的位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathNodeIndex {
    /// 自由画笔 `points[i]`。
    Stroke(usize),
    /// `shapes[shape].rings[ring][vertex]`。
    Ring {
        shape: usize,
        ring: usize,
        vertex: usize,
    },
}

/// 命中的是锚点还是控制柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathNodePart {
    /// 锚点。
    Anchor,
    /// 入向控制柄（相对锚点的偏移）。
    HandleIn,
    /// 出向控制柄（相对锚点的偏移）。
    HandleOut,
}

/// 一次节点命中。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathNodeHit {
    /// 节点下标。
    pub index: PathNodeIndex,
    /// 锚点或控制柄。
    pub part: PathNodePart,
}

/// 可编辑节点的世界坐标快照，供覆盖层绘制。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathNodeView {
    /// 节点下标。
    pub index: PathNodeIndex,
    /// 锚点世界坐标。
    pub anchor: Vec2,
    /// 入向控制柄世界坐标（有柄时）。
    pub handle_in: Option<Vec2>,
    /// 出向控制柄世界坐标（有柄时）。
    pub handle_out: Option<Vec2>,
}

/// Path 的可编辑几何快照。
#[derive(Debug, Clone, PartialEq)]
pub struct PathGeometry {
    /// 自由画笔采样点（局部）。
    pub points: Vec<Vec2>,
    /// 与 `points` 对齐的可选控制柄。
    pub point_handles: Vec<PathPointHandles>,
    /// 闭合填充环。
    pub shapes: Vec<FillShape>,
}

impl PathGeometry {
    /// 从现有 Path 读取。
    pub fn from_path(path: &Path) -> Self {
        Self {
            points: path.points.clone(),
            point_handles: path.point_handles.clone(),
            shapes: path.shapes.clone(),
        }
    }

    /// 写回 Path（不 `mark_dirty`）。
    pub fn apply_to(&self, path: &mut Path) {
        path.points = self.points.clone();
        path.point_handles = self.point_handles.clone();
        path.shapes = self.shapes.clone();
    }
}

fn geometries_equal(a: &PathGeometry, b: &PathGeometry) -> bool {
    a == b
}

/// 当前选择里唯一可编辑的未锁定 Path。
///
/// 空选、多选、组、锁定、非 Path 返回 `None`。
pub fn sole_editable_path(scene: &Scene, ids: &[ElementId]) -> Option<ElementId> {
    if ids.len() != 1 {
        return None;
    }
    let id = ids[0];
    if scene.is_effectively_locked(id) {
        return None;
    }
    match scene.get(id) {
        Some(AnyElement::Path(path)) if path.props.visible => {
            if path.is_filled() {
                let has_vertex = path.shapes.iter().any(|shape| {
                    shape
                        .rings
                        .iter()
                        .any(|ring| ring.iter().any(|_| true) && !ring.is_empty())
                });
                if has_vertex {
                    Some(id)
                } else {
                    None
                }
            } else if path.points.is_empty() {
                None
            } else {
                Some(id)
            }
        }
        _ => None,
    }
}

/// 列出世界坐标下的可编辑节点。
pub fn path_node_views(path: &Path) -> Vec<PathNodeView> {
    let transform = &path.props.transform;
    let mut views = Vec::new();
    if path.is_filled() {
        for (shape_i, shape) in path.shapes.iter().enumerate() {
            for (ring_i, ring) in shape.rings.iter().enumerate() {
                for (vertex_i, anchor) in ring.iter().enumerate() {
                    views.push(PathNodeView {
                        index: PathNodeIndex::Ring {
                            shape: shape_i,
                            ring: ring_i,
                            vertex: vertex_i,
                        },
                        anchor: transform.transform_point(*anchor),
                        handle_in: None,
                        handle_out: None,
                    });
                }
            }
        }
        return views;
    }
    for (i, anchor) in path.points.iter().enumerate() {
        let handles = path.point_handles.get(i).copied().unwrap_or_default();
        let handle_in = handles
            .hin
            .map(|offset| transform.transform_point(*anchor + offset));
        let handle_out = handles
            .hout
            .map(|offset| transform.transform_point(*anchor + offset));
        views.push(PathNodeView {
            index: PathNodeIndex::Stroke(i),
            anchor: transform.transform_point(*anchor),
            handle_in,
            handle_out,
        });
    }
    views
}

/// 在世界坐标命中最近的节点或控制柄。
///
/// `hit_radius` 是世界单位。优先控制柄，再锚点。未命中返回 `None`。
pub fn hit_path_node(path: &Path, world: Vec2, hit_radius: f32) -> Option<PathNodeHit> {
    let radius = hit_radius.max(1.0e-4);
    let radius_sq = radius * radius;
    let mut best: Option<(PathNodeHit, f32)> = None;
    for view in path_node_views(path) {
        for (part, position) in [
            (PathNodePart::HandleIn, view.handle_in),
            (PathNodePart::HandleOut, view.handle_out),
            (PathNodePart::Anchor, Some(view.anchor)),
        ] {
            let Some(position) = position else {
                continue;
            };
            let dist_sq = (position - world).length_squared();
            if dist_sq > radius_sq {
                continue;
            }
            if best.is_none_or(|(_, best_dist)| dist_sq < best_dist) {
                best = Some((
                    PathNodeHit {
                        index: view.index,
                        part,
                    },
                    dist_sq,
                ));
            }
        }
    }
    best.map(|(hit, _)| hit)
}

/// 把节点或控制柄拖到世界坐标，返回新几何。失败时返回原几何。
pub fn move_path_node(path: &Path, hit: PathNodeHit, world: Vec2) -> PathGeometry {
    let mut geometry = PathGeometry::from_path(path);
    let transform = &path.props.transform;
    if !transform.is_invertible() {
        return geometry;
    }
    let local = transform.inverse_transform_point(world);
    if !local.x.is_finite() || !local.y.is_finite() {
        return geometry;
    }
    match hit.index {
        PathNodeIndex::Stroke(i) => {
            if i >= geometry.points.len() {
                return geometry;
            }
            match hit.part {
                PathNodePart::Anchor => {
                    geometry.points[i] = local;
                }
                PathNodePart::HandleIn => {
                    ensure_handle_slot(&mut geometry.point_handles, i);
                    geometry.point_handles[i].hin = Some(local - geometry.points[i]);
                }
                PathNodePart::HandleOut => {
                    ensure_handle_slot(&mut geometry.point_handles, i);
                    geometry.point_handles[i].hout = Some(local - geometry.points[i]);
                }
            }
        }
        PathNodeIndex::Ring {
            shape,
            ring,
            vertex,
        } => {
            if hit.part != PathNodePart::Anchor {
                return geometry;
            }
            let Some(shape_ref) = geometry.shapes.get_mut(shape) else {
                return geometry;
            };
            let Some(ring_ref) = shape_ref.rings.get_mut(ring) else {
                return geometry;
            };
            if vertex >= ring_ref.len() {
                return geometry;
            }
            ring_ref[vertex] = local;
        }
    }
    geometry
}

fn ensure_handle_slot(handles: &mut Vec<PathPointHandles>, index: usize) {
    while handles.len() <= index {
        handles.push(PathPointHandles::default());
    }
}

/// 写回几何并 [`Scene::mark_dirty`]。不是 Path 时返回 false。
pub fn apply_path_geometry(scene: &mut Scene, id: ElementId, geometry: &PathGeometry) -> bool {
    {
        let Some(AnyElement::Path(path)) = scene.get_mut(id) else {
            return false;
        };
        let current = PathGeometry::from_path(path);
        if geometries_equal(&current, geometry) {
            return false;
        }
        geometry.apply_to(path);
    }
    scene.mark_dirty(id);
    true
}

/// 一次可撤销的 Path 几何补丁。
#[derive(Debug)]
pub struct EditPathCommand {
    id: ElementId,
    before: PathGeometry,
    after: PathGeometry,
}

impl EditPathCommand {
    /// 按前后快照生成命令。没有实际变化时返回 `None`。
    pub fn from_states(id: ElementId, before: PathGeometry, after: PathGeometry) -> Option<Self> {
        if geometries_equal(&before, &after) {
            None
        } else {
            Some(Self { id, before, after })
        }
    }

    /// 对当前选择应用几何。空选 / 锁定 / 非 Path / 无变化返回 `None`。
    pub fn from_geometry(scene: &Scene, ids: &[ElementId], after: PathGeometry) -> Option<Self> {
        let id = sole_editable_path(scene, ids)?;
        let AnyElement::Path(path) = scene.get(id)? else {
            return None;
        };
        let before = PathGeometry::from_path(path);
        Self::from_states(id, before, after)
    }
}

impl Command for EditPathCommand {
    fn execute(&mut self, scene: &mut Scene) {
        let _ = apply_path_geometry(scene, self.id, &self.after);
    }

    fn undo(&mut self, scene: &mut Scene) {
        let _ = apply_path_geometry(scene, self.id, &self.before);
    }

    fn name(&self) -> &'static str {
        "EditPath"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::ElementStyle;
    use crate::{CommandHistory, Element};
    use infinitecanvas_math::Color;

    fn stroke_path() -> Path {
        Path::from_world_points(
            1,
            &[
                Vec2::new(0.0, 0.0),
                Vec2::new(40.0, 0.0),
                Vec2::new(40.0, 30.0),
            ],
            ElementStyle::stroke_only(Color::BLACK, 2.0),
        )
    }

    #[test]
    fn old_path_json_has_no_handles_and_nodes_match_points() {
        let path = stroke_path();
        assert!(path.point_handles.is_empty());
        let views = path_node_views(&path);
        assert_eq!(views.len(), 3);
        assert!(views.iter().all(|view| view.handle_in.is_none()));
        assert!((views[1].anchor - Vec2::new(40.0, 0.0)).length() < 1e-4);
    }

    #[test]
    fn hit_test_prefers_node_over_distant_segment_midpoint() {
        let path = stroke_path();
        let hit = hit_path_node(&path, Vec2::new(40.0, 0.0), 6.0).unwrap();
        assert_eq!(
            hit,
            PathNodeHit {
                index: PathNodeIndex::Stroke(1),
                part: PathNodePart::Anchor,
            }
        );
        assert!(hit_path_node(&path, Vec2::new(20.0, 20.0), 4.0).is_none());
    }

    #[test]
    fn drag_anchor_updates_geometry_and_undo_restores() {
        let mut scene = Scene::new();
        scene.add_path(stroke_path());
        let before = PathGeometry::from_path(match scene.get(1).unwrap() {
            AnyElement::Path(path) => path,
            _ => panic!("path"),
        });
        let hit = PathNodeHit {
            index: PathNodeIndex::Stroke(1),
            part: PathNodePart::Anchor,
        };
        let path = match scene.get(1).unwrap() {
            AnyElement::Path(path) => path.clone(),
            _ => panic!("path"),
        };
        let after = move_path_node(&path, hit, Vec2::new(60.0, -10.0));
        let epoch = scene.geometry_epoch();
        let mut history = CommandHistory::new();
        let command = EditPathCommand::from_states(1, before.clone(), after.clone()).unwrap();
        history.execute(Box::new(command), &mut scene);
        assert!(scene.geometry_epoch() > epoch);
        match scene.get(1).unwrap() {
            AnyElement::Path(path) => {
                assert!((path.points()[1] - Vec2::new(60.0, -10.0)).length() < 1e-3);
                assert!(path.contains_point(Vec2::new(60.0, -10.0)));
            }
            _ => panic!("path"),
        }
        assert!(history.undo(&mut scene));
        match scene.get(1).unwrap() {
            AnyElement::Path(path) => {
                assert_eq!(PathGeometry::from_path(path), before);
            }
            _ => panic!("path"),
        }
    }

    #[test]
    fn empty_locked_non_path_are_noop() {
        let mut scene = Scene::new();
        scene.add_path(stroke_path());
        scene.set_locked(1, true);
        let after = PathGeometry {
            points: vec![Vec2::ZERO],
            point_handles: Vec::new(),
            shapes: Vec::new(),
        };
        assert!(EditPathCommand::from_geometry(&scene, &[], after.clone()).is_none());
        assert!(EditPathCommand::from_geometry(&scene, &[1], after).is_none());
        assert!(sole_editable_path(&scene, &[1]).is_none());
    }

    #[test]
    fn handles_round_trip_and_move_with_relative_offset() {
        let mut path = stroke_path();
        path.point_handles = vec![
            PathPointHandles::default(),
            PathPointHandles {
                hin: Some(Vec2::new(-10.0, 5.0)),
                hout: Some(Vec2::new(8.0, 0.0)),
            },
            PathPointHandles::default(),
        ];
        let json = serde_json::to_string(&path).unwrap();
        assert!(json.contains("point_handles"));
        let restored: Path = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.point_handles[1].hout, Some(Vec2::new(8.0, 0.0)));

        let old = serde_json::json!({
            "props": {
                "id": 9,
                "transform": { "translation": [0.0, 0.0], "rotation": 0.0, "scale": [1.0, 1.0] },
                "z_index": 9,
                "visible": true,
                "selected": false
            },
            "points": [[0.0, 0.0], [10.0, 0.0]],
            "style": { "stroke": [0.0, 0.0, 0.0, 1.0], "stroke_width": 2.0 }
        });
        let legacy: Path = serde_json::from_value(old).unwrap();
        assert!(legacy.point_handles.is_empty());
        assert_eq!(legacy.points().len(), 2);

        let views = path_node_views(&path);
        assert_eq!(views[1].handle_out.unwrap(), Vec2::new(48.0, 0.0));
        let hit = PathNodeHit {
            index: PathNodeIndex::Stroke(1),
            part: PathNodePart::HandleOut,
        };
        let next = move_path_node(&path, hit, Vec2::new(55.0, 4.0));
        assert_eq!(next.point_handles[1].hout, Some(Vec2::new(15.0, 4.0)));
        assert_eq!(next.points[1], path.points[1]);
    }

    #[test]
    fn filled_ring_anchor_is_editable() {
        let mut path = Path::from_world_points(3, &[], ElementStyle::fill_only(Color::RED));
        path.shapes = vec![FillShape {
            rings: vec![vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(20.0, 0.0),
                Vec2::new(20.0, 20.0),
                Vec2::new(0.0, 20.0),
            ]],
        }];
        let views = path_node_views(&path);
        assert_eq!(views.len(), 4);
        let hit = hit_path_node(&path, Vec2::new(20.0, 0.0), 5.0).unwrap();
        assert_eq!(
            hit.index,
            PathNodeIndex::Ring {
                shape: 0,
                ring: 0,
                vertex: 1
            }
        );
        let next = move_path_node(&path, hit, Vec2::new(30.0, -5.0));
        assert_eq!(next.shapes[0].rings[0][1], Vec2::new(30.0, -5.0));
    }
}
