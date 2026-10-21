//! Path 节点编辑的 WASM 接口。
//!
//! 选中单一 Path 后 `set_path_node_mode(true)`，或双击 Path 进入。
//! 拖拽锚点/控制柄实时改几何，松手记一条 `EditPath`。
//! Esc / 点空白结束模式（与裁剪模式相同：Esc 关模式，清空选择后自然无手柄）。
//! 不新增与 V/L/P/R/O/T/H/I/M/G/F/N/K 冲突的快捷键。

use infinitecanvas_engine::{
    apply_path_geometry, hit_path_node, move_path_node, path_node_views, sole_editable_path,
    transform_handles::HandleMetrics, AnyElement, EditPathCommand, ElementId, PathGeometry,
    PathNodeHit, PathNodePart, PathNodeView,
};
use js_sys::{Array, Object, Reflect};
use wasm_bindgen::prelude::*;

use crate::InfiniteCanvas;

/// 一次节点拖动。松手前场景里已经是新几何。
pub(crate) struct PathNodeDrag {
    pub pointer_id: i32,
    path_id: ElementId,
    hit: PathNodeHit,
    before: PathGeometry,
}

/// 覆盖层绘制用的节点列表。
#[derive(Debug, Clone, PartialEq)]
pub struct PathNodeChrome {
    pub nodes: Vec<PathNodeView>,
}

#[wasm_bindgen]
impl InfiniteCanvas {
    /// 打开或关闭 Path 节点编辑。
    ///
    /// 只在选中单一未锁定可见 Path 时显示节点手柄，并隐藏包围盒缩放/旋转手柄。
    /// 关闭时提交未结束的拖动。没有新快捷键。
    #[wasm_bindgen]
    pub fn set_path_node_mode(&mut self, enabled: bool) {
        if !enabled {
            self.commit_path_node_gesture();
        }
        self.path_node_mode = enabled;
    }

    /// 节点编辑模式是否打开。
    #[wasm_bindgen]
    pub fn path_node_mode(&self) -> bool {
        self.path_node_mode
    }

    /// 当前选择是否可进入 Path 节点编辑（单一未锁定可见 Path）。
    #[wasm_bindgen]
    pub fn path_node_edit_supported(&self) -> bool {
        self.sole_path_for_nodes().is_some()
    }

    /// 当前是否正在显示 Path 节点手柄。
    #[wasm_bindgen(js_name = path_node_handles_active)]
    pub fn path_node_handles_active_js(&self) -> bool {
        self.path_node_handles_active()
    }

    /// 在屏幕坐标处尝试进入节点编辑。
    ///
    /// 命中未锁定 Path 时选中它并打开节点模式，返回 true。
    /// 供双击使用；不占用工具快捷键。
    #[wasm_bindgen]
    pub fn begin_path_node_edit_at(&mut self, x: f32, y: f32) -> bool {
        let world = self
            .viewport
            .screen_to_world(infinitecanvas_engine::math::Vec2::new(x, y));
        let options = infinitecanvas_engine::HitTestOptions {
            max_results: 8,
            include_invisible: false,
            include_locked: false,
        };
        let hits = infinitecanvas_engine::hit_test_point(&self.scene, world, &options);
        let Some(id) = hits
            .into_iter()
            .find_map(|hit| match self.scene.get(hit.id) {
                Some(AnyElement::Path(_)) if !self.scene.is_effectively_locked(hit.id) => {
                    Some(hit.id)
                }
                _ => None,
            })
        else {
            return false;
        };
        self.commit_path_node_gesture();
        self.cancel_handle_gesture();
        self.selection_box.cancel();
        self.selection.clear(&mut self.scene);
        self.selection.select_element(&mut self.scene, id, false);
        self.path_node_mode = true;
        true
    }

    /// 当前选中 Path 的节点列表（世界坐标），供调试或自定义 UI。
    #[wasm_bindgen]
    pub fn get_path_nodes(&self) -> JsValue {
        let Some(id) = self.sole_path_for_nodes() else {
            return Array::new().into();
        };
        let Some(AnyElement::Path(path)) = self.scene.get(id) else {
            return Array::new().into();
        };
        let out = Array::new();
        for view in path_node_views(path) {
            out.push(&node_view_to_js(&view));
        }
        out.into()
    }
}

impl InfiniteCanvas {
    pub(crate) fn path_node_handles_active(&self) -> bool {
        self.path_node_mode && self.sole_path_for_nodes().is_some()
    }

    pub(crate) fn path_node_overlay(&self) -> Option<PathNodeChrome> {
        if !self.path_node_handles_active() {
            return None;
        }
        let id = self.sole_path_for_nodes()?;
        let AnyElement::Path(path) = self.scene.get(id)? else {
            return None;
        };
        Some(PathNodeChrome {
            nodes: path_node_views(path),
        })
    }

    fn sole_path_for_nodes(&self) -> Option<ElementId> {
        sole_editable_path(&self.scene, &self.selection.selected_ids())
    }

    pub(crate) fn try_begin_path_node_drag(
        &mut self,
        event: &infinitecanvas_engine::PointerEvent,
    ) -> bool {
        if !self.path_node_handles_active() {
            return false;
        }
        self.commit_path_node_gesture();
        let Some(id) = self.sole_path_for_nodes() else {
            return false;
        };
        if self.scene.is_effectively_locked(id) {
            return false;
        }
        let Some(AnyElement::Path(path)) = self.scene.get(id) else {
            return false;
        };
        let metrics = HandleMetrics::from_zoom(self.viewport.zoom());
        let Some(hit) = hit_path_node(path, event.world_position, metrics.hit_radius) else {
            return false;
        };
        let before = PathGeometry::from_path(path);
        self.selection_box.cancel();
        self.path_node_drag = Some(PathNodeDrag {
            pointer_id: event.pointer_id,
            path_id: id,
            hit,
            before,
        });
        true
    }

    pub(crate) fn update_path_node_drag(
        &mut self,
        event: &infinitecanvas_engine::PointerEvent,
    ) -> bool {
        let Some(drag) = &self.path_node_drag else {
            return false;
        };
        if drag.pointer_id != event.pointer_id {
            return false;
        }
        let path_id = drag.path_id;
        let hit = drag.hit;
        let Some(AnyElement::Path(path)) = self.scene.get(path_id) else {
            return true;
        };
        let next = move_path_node(path, hit, event.world_position);
        let _ = apply_path_geometry(&mut self.scene, path_id, &next);
        true
    }

    pub(crate) fn finish_path_node_drag(&mut self, pointer_id: i32) -> bool {
        let matches = self
            .path_node_drag
            .as_ref()
            .is_some_and(|drag| drag.pointer_id == pointer_id);
        if matches {
            self.commit_path_node_gesture();
        }
        matches
    }

    pub(crate) fn commit_path_node_gesture(&mut self) {
        let Some(drag) = self.path_node_drag.take() else {
            return;
        };
        let Some(AnyElement::Path(path)) = self.scene.get(drag.path_id) else {
            return;
        };
        let after = PathGeometry::from_path(path);
        if let Some(command) = EditPathCommand::from_states(drag.path_id, drag.before, after) {
            self.command_history.push_executed(Box::new(command));
        }
    }

    pub(crate) fn cancel_path_node_gesture(&mut self) {
        let Some(drag) = self.path_node_drag.take() else {
            return;
        };
        let _ = apply_path_geometry(&mut self.scene, drag.path_id, &drag.before);
    }

    pub(crate) fn path_node_cursor_at(
        &self,
        world: infinitecanvas_engine::math::Vec2,
    ) -> Option<&'static str> {
        if let Some(drag) = &self.path_node_drag {
            return Some(cursor_for_part(drag.hit.part));
        }
        if !self.path_node_handles_active() {
            return None;
        }
        let id = self.sole_path_for_nodes()?;
        let AnyElement::Path(path) = self.scene.get(id)? else {
            return None;
        };
        let metrics = HandleMetrics::from_zoom(self.viewport.zoom());
        hit_path_node(path, world, metrics.hit_radius).map(|hit| cursor_for_part(hit.part))
    }
}

fn cursor_for_part(part: PathNodePart) -> &'static str {
    match part {
        PathNodePart::Anchor => "move",
        PathNodePart::HandleIn | PathNodePart::HandleOut => "crosshair",
    }
}

fn node_view_to_js(view: &PathNodeView) -> JsValue {
    let obj = Object::new();
    let _ = Reflect::set(
        &obj,
        &JsValue::from_str("kind"),
        &JsValue::from_str(match view.index {
            infinitecanvas_engine::PathNodeIndex::Stroke(_) => "stroke",
            infinitecanvas_engine::PathNodeIndex::Ring { .. } => "ring",
        }),
    );
    match view.index {
        infinitecanvas_engine::PathNodeIndex::Stroke(i) => {
            let _ = Reflect::set(&obj, &JsValue::from_str("index"), &JsValue::from(i as f64));
        }
        infinitecanvas_engine::PathNodeIndex::Ring {
            shape,
            ring,
            vertex,
        } => {
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("shape"),
                &JsValue::from(shape as f64),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("ring"),
                &JsValue::from(ring as f64),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("vertex"),
                &JsValue::from(vertex as f64),
            );
        }
    }
    write_vec2(&obj, "anchor", view.anchor);
    if let Some(hin) = view.handle_in {
        write_vec2(&obj, "handleIn", hin);
    }
    if let Some(hout) = view.handle_out {
        write_vec2(&obj, "handleOut", hout);
    }
    obj.into()
}

fn write_vec2(obj: &Object, name: &str, value: infinitecanvas_engine::math::Vec2) {
    let point = Object::new();
    let _ = Reflect::set(&point, &JsValue::from_str("x"), &JsValue::from(value.x));
    let _ = Reflect::set(&point, &JsValue::from_str("y"), &JsValue::from(value.y));
    let _ = Reflect::set(obj, &JsValue::from_str(name), &point);
}
