//! InfiniteCanvas WASM - WebAssembly bindings
//!
//! 提供 Rust 引擎到 JavaScript 的绑定接口。
//!
//! # 模块职责
//! - 暴露 WASM API 给 JavaScript
//! - 管理跨界调用和数据转换
//! - 处理 Web 平台特定的初始化
//!
//! # 接口粒度
//! - 高层 API：初始化、渲染循环控制
//! - 批量处理：事件队列、元素操作
//! - 避免高频跨界调用

// `cargo build` / `cargo clippy` / `cargo test` run on the host target.
// The WebGPU canvas surface and several browser-only helpers are wasm32-only,
// so native checks must not treat that unused-on-host code as a failure.
#![cfg_attr(
    not(target_arch = "wasm32"),
    allow(dead_code, clippy::too_many_arguments)
)]

mod canvas;
mod input;
mod renderer;
mod texture_manager;

use infinitecanvas_engine::{
    export_selection_png, export_viewport_png,
    hit_test::{hit_test_point, HitTestOptions, HitTestResult},
    math::{Color, Transform, Vec2},
    transform_handles::{
        cursor_for, transforms_differ, HandleGesture, HandleMetrics, SelectionFrame,
    },
    AddElementCommand, AnyElement, Circle, CommandHistory, ElementId, ElementStyle, Event,
    EventQueue, Image, Line, MoveCommand, Path, PointerButton, PointerEvent, Rectangle, Scene,
    SelectionBox, SelectionManager, Tool, ToolManager, TransformElementsCommand, Viewport,
};

/// 直线 / 画笔抬笔时短于该长度则丢弃
const MIN_STROKE_LENGTH: f32 = 2.0;
/// 画笔采样的最小间距，避免点过密
const PEN_SAMPLE_SPACING: f32 = 1.5;
use input::{
    keyboard_event_from_js, modifiers_from_flags, pointer_event_from_js, wheel_event_from_js,
    KeyboardEventKind, PointerEventKind,
};
use js_sys::{Array, Object, Reflect};
use renderer::WebGPURenderer;
use std::str::FromStr;
use wasm_bindgen::prelude::*;

const BUILD_SIGNATURE: &str = match option_env!("BUILD_SIGNATURE") {
    Some(sig) => sig,
    None => "dev-local",
};
const BUILD_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 拖动状态
#[derive(Default)]
struct DragState {
    /// 是否正在拖动
    is_dragging: bool,
    /// 拖动的 pointer ID
    pointer_id: Option<i32>,
    /// 拖动开始位置（世界坐标）
    start_position: Vec2,
    /// 上一次鼠标位置（世界坐标）
    last_position: Vec2,
    /// 总移动量（用于创建命令）
    total_delta: Vec2,
    /// 被拖动的元素 ID 列表
    dragged_ids: Vec<ElementId>,
    /// 自由画笔采样点（世界坐标）
    pen_points: Vec<Vec2>,
    /// 绘制过程中的预览元素；抬笔后才进入撤销栈
    preview_id: Option<ElementId>,
}

/// 缩放 / 旋转手柄的拖动。预览已经写进场景，松手后才进入撤销栈。
struct HandleInteraction {
    pointer_id: i32,
    gesture: HandleGesture,
    live_frame: SelectionFrame,
    latest: Vec<(ElementId, Transform)>,
}

/// 性能统计
#[derive(Default)]
struct PerformanceStats {
    /// 帧计数
    frame_count: u64,
    /// 上一帧渲染的顶点数
    last_vertex_count: usize,
    /// 上一帧的绘制调用数
    last_draw_calls: usize,
    /// 上一帧渲染时间（毫秒）
    last_frame_time_ms: f64,
    /// 上一帧重新生成顶点的元素数
    regenerated_elements: usize,
    /// 上一帧写入顶点缓冲的字节数（不含 uniform）
    gpu_bytes_written: u64,
    /// 上一帧是否只更新了视口 uniform
    geometry_unchanged: bool,
}

/// WASM 主接口，提供无限画布的所有功能
///
/// # 职责
/// - 初始化 WebGPU 渲染引擎
/// - 管理场景和视口
/// - 接收并处理 JavaScript 事件
/// - 管理渲染循环
///
/// # 示例（JavaScript）
/// ```js
/// const canvas = new InfiniteCanvas();
/// await canvas.init(document.getElementById('canvas'));
/// canvas.render();
/// ```
#[wasm_bindgen]
pub struct InfiniteCanvas {
    webgpu_renderer: Option<WebGPURenderer>,
    scene: Scene,
    viewport: Viewport,
    engine: infinitecanvas_engine::Renderer,
    event_queue: EventQueue,
    selection: SelectionManager,
    selection_box: SelectionBox,
    drag_state: DragState,
    handle_drag: Option<HandleInteraction>,
    command_history: CommandHistory,
    tool_manager: ToolManager,
    perf_stats: PerformanceStats,
}

#[wasm_bindgen]
impl InfiniteCanvas {
    /// 创建新的画布实例（构造器，实际初始化在 init() 中完成）
    ///
    /// # 返回
    /// - `Ok(InfiniteCanvas)`: 成功创建
    /// - `Err(JsValue)`: 初始化失败
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<InfiniteCanvas, JsValue> {
        // 初始化 panic hook（更好的错误信息）
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        // 初始化日志系统
        console_log::init_with_level(log::Level::Debug)
            .map_err(|e| JsValue::from_str(&format!("Failed to init logger: {:?}", e)))?;

        log::info!("InfiniteCanvas WASM module initialized");
        log::info!(
            "🆔 InfiniteCanvas build info: version={}, signature={}",
            BUILD_VERSION,
            BUILD_SIGNATURE
        );

        Ok(InfiniteCanvas {
            webgpu_renderer: None,
            scene: Scene::new(),
            viewport: Viewport::default(),
            engine: infinitecanvas_engine::Renderer::new(),
            event_queue: EventQueue::new(),
            selection: SelectionManager::new(),
            selection_box: SelectionBox::new(),
            drag_state: DragState::default(),
            handle_drag: None,
            command_history: CommandHistory::new(),
            tool_manager: ToolManager::new(),
            perf_stats: PerformanceStats::default(),
        })
    }

    /// 初始化画布（异步，因为 WebGPU 初始化需要异步）
    ///
    /// # 参数
    /// - `canvas`: HTMLCanvasElement
    ///
    /// # 返回
    /// - `Ok(())`: WebGPU 初始化成功
    /// - `Err(JsValue)`: WebGPU 不可用或初始化失败
    #[wasm_bindgen]
    pub async fn init(&mut self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
        log::info!("Initializing InfiniteCanvas with WebGPU...");

        let width = canvas.width();
        let height = canvas.height();

        // 更新视口尺寸
        self.viewport.resize(Vec2::new(width as f32, height as f32));

        // 设置视口中心，使屏幕左上角 (0,0) 对应世界坐标 (0,0)
        // 视口中心应该在画布中心对应的世界位置
        self.viewport
            .move_to(Vec2::new(width as f32 / 2.0, height as f32 / 2.0));

        // 初始化 WebGPU（不再有 Canvas 2D 后备方案）
        let renderer = WebGPURenderer::new(canvas, width, height).await
            .map_err(|e| JsValue::from_str(&format!("WebGPU initialization failed: {:?}. Please use a browser that supports WebGPU.", e)))?;

        log::info!("✅ WebGPU renderer initialized successfully");
        self.webgpu_renderer = Some(renderer);

        // 不再初始化测试场景 - 用户会手动添加元素
        // self.init_test_scene();

        log::info!("✅ InfiniteCanvas initialization complete");
        Ok(())
    }

    /// 初始化测试场景
    fn init_test_scene(&mut self) {
        let id1 = self.scene.next_id();
        let rect = Rectangle::new(id1, Vec2::new(100.0, 100.0), Vec2::new(200.0, 150.0))
            .with_style(ElementStyle::fill_only(Color::from_hex("#3b82f6").unwrap()));
        self.scene.add_rect(rect);

        let id2 = self.scene.next_id();
        let rect2 =
            Rectangle::new(id2, Vec2::new(375.0, 125.0), Vec2::new(200.0, 150.0)).with_style(
                ElementStyle::stroke_only(Color::from_hex("#ef4444").unwrap(), 3.0),
            );
        self.scene.add_rect(rect2);

        let id3 = self.scene.next_id();
        let mut circle_style = ElementStyle::fill_only(Color::from_hex("#10b981").unwrap());
        circle_style.stroke = Some(Color::from_hex("#059669").unwrap());
        circle_style.stroke_width = 2.0;
        let circle = Circle::new(id3, Vec2::new(450.0, 350.0), 50.0).with_style(circle_style);
        self.scene.add_circle(circle);

        let id4 = self.scene.next_id();
        let circle2 = Circle::new(id4, Vec2::new(200.0, 350.0), 60.0).with_style(
            ElementStyle::stroke_only(Color::from_hex("#f59e0b").unwrap(), 3.0),
        );
        self.scene.add_circle(circle2);

        log::info!(
            "Test scene initialized with {} elements",
            self.scene.count()
        );
    }

    /// 渲染一帧
    ///
    /// # 返回
    /// - `Ok(())`: 渲染成功
    /// - `Err(JsValue)`: 渲染失败（渲染器未初始化）
    #[wasm_bindgen]
    pub fn render(&mut self) -> Result<(), JsValue> {
        self.dispatch_pending_events();
        let selection_box = self.selection_box.bounds();
        let transform_frame = self.display_selection_frame();
        if let Some(renderer) = &mut self.webgpu_renderer {
            let started = js_sys::Date::now();
            renderer.render(
                &mut self.scene,
                &self.viewport,
                selection_box,
                transform_frame,
            )?;
            let stats = renderer.last_stats();

            self.perf_stats.frame_count += 1;
            self.perf_stats.last_vertex_count = stats.vertex_count;
            self.perf_stats.last_draw_calls = stats.draw_calls;
            self.perf_stats.last_frame_time_ms = js_sys::Date::now() - started;
            self.perf_stats.regenerated_elements = stats.regenerated_elements;
            self.perf_stats.gpu_bytes_written = stats.gpu_bytes_written;
            self.perf_stats.geometry_unchanged = stats.geometry_unchanged;

            Ok(())
        } else {
            Err(JsValue::from_str(
                "WebGPU renderer not initialized. Call init() first.",
            ))
        }
    }

    // ========== 输入事件 API ==========
    #[wasm_bindgen]
    pub fn handle_pointer_event(
        &mut self,
        kind: String,
        pointer_id: i32,
        x: f32,
        y: f32,
        movement_x: f32,
        movement_y: f32,
        button: i16,
        buttons_mask: u16,
        shift_key: bool,
        ctrl_key: bool,
        alt_key: bool,
        meta_key: bool,
    ) {
        if let Ok(event_kind) = PointerEventKind::from_str(&kind) {
            let modifiers = modifiers_from_flags(shift_key, ctrl_key, alt_key, meta_key);
            let screen_pos = Vec2::new(x, y);
            let screen_delta = Vec2::new(movement_x, movement_y);
            let event = pointer_event_from_js(
                event_kind,
                pointer_id,
                screen_pos,
                screen_delta,
                button,
                buttons_mask,
                modifiers,
                &self.viewport,
            );
            self.event_queue.push(event);
        } else {
            log::warn!("Unknown pointer event kind: {}", kind);
        }
    }

    #[wasm_bindgen]
    pub fn handle_wheel_event(
        &mut self,
        delta_x: f32,
        delta_y: f32,
        delta_mode: u32,
        shift_key: bool,
        ctrl_key: bool,
        alt_key: bool,
        meta_key: bool,
    ) {
        let modifiers = modifiers_from_flags(shift_key, ctrl_key, alt_key, meta_key);
        let delta = Vec2::new(delta_x, delta_y);
        let event = wheel_event_from_js(delta, delta_mode, modifiers);
        self.event_queue.push(event);
    }

    #[wasm_bindgen]
    pub fn handle_keyboard_event(
        &mut self,
        kind: String,
        key: String,
        code: String,
        repeat: bool,
        shift_key: bool,
        ctrl_key: bool,
        alt_key: bool,
        meta_key: bool,
    ) {
        if let Ok(event_kind) = KeyboardEventKind::from_str(&kind) {
            let modifiers = modifiers_from_flags(shift_key, ctrl_key, alt_key, meta_key);
            let event = keyboard_event_from_js(event_kind, key, code, repeat, modifiers);
            self.event_queue.push(event);
        } else {
            log::warn!("Unknown keyboard event kind: {}", kind);
        }
    }

    #[wasm_bindgen]
    pub fn debug_pending_event_count(&self) -> u32 {
        self.event_queue.len() as u32
    }

    #[wasm_bindgen]
    pub fn debug_clear_events(&mut self) {
        self.event_queue.clear();
    }

    #[wasm_bindgen]
    pub fn hit_test_world_point(&self, x: f32, y: f32, max_results: Option<u32>) -> Array {
        let mut options = HitTestOptions::default();
        if let Some(limit) = max_results {
            if limit > 0 {
                options.max_results = limit as usize;
            }
        }
        let hits = hit_test_point(&self.scene, Vec2::new(x, y), &options);
        hit_results_to_js_array(hits)
    }

    #[wasm_bindgen]
    pub fn select_element(&mut self, element_id: u64, additive: bool) -> bool {
        self.selection
            .select_element(&mut self.scene, element_id, additive)
    }

    #[wasm_bindgen]
    pub fn deselect_element(&mut self, element_id: u64) -> bool {
        self.selection.deselect(&mut self.scene, element_id)
    }

    #[wasm_bindgen]
    pub fn clear_selection(&mut self) {
        self.cancel_handle_gesture();
        self.selection_box.cancel();
        self.selection.clear(&mut self.scene);
    }

    #[wasm_bindgen]
    pub fn get_selection_count(&self) -> u32 {
        self.selection.len() as u32
    }

    #[wasm_bindgen]
    pub fn get_selected_ids(&self) -> Array {
        selection_ids_to_js_array(self.selection.selected_ids())
    }

    /// 获取当前框选矩形（如果正在框选）
    ///
    /// # 返回
    /// 如果正在框选，返回 { x, y, width, height }；否则返回 null
    #[wasm_bindgen]
    pub fn get_selection_box_bounds(&self) -> JsValue {
        if let Some(rect) = self.selection_box.bounds() {
            let obj = Object::new();
            let _ = Reflect::set(&obj, &JsValue::from_str("x"), &JsValue::from(rect.x));
            let _ = Reflect::set(&obj, &JsValue::from_str("y"), &JsValue::from(rect.y));
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("width"),
                &JsValue::from(rect.width),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("height"),
                &JsValue::from(rect.height),
            );
            obj.into()
        } else {
            JsValue::NULL
        }
    }

    /// 检查是否正在框选
    #[wasm_bindgen]
    pub fn is_selection_box_active(&self) -> bool {
        self.selection_box.is_active()
    }

    /// 检查是否正在拖动
    #[wasm_bindgen]
    pub fn is_dragging(&self) -> bool {
        self.drag_state.is_dragging
    }

    /// 移动选中的元素
    ///
    /// # 参数
    /// - `dx`: X 方向移动量（世界坐标）
    /// - `dy`: Y 方向移动量（世界坐标）
    #[wasm_bindgen]
    pub fn move_selection(&mut self, dx: f32, dy: f32) {
        let delta = Vec2::new(dx, dy);
        let selected_ids = self.selection.selected_ids();
        if selected_ids.is_empty() {
            return;
        }

        // 创建并执行移动命令
        let cmd = MoveCommand::new(selected_ids, delta);
        self.command_history.execute(Box::new(cmd), &mut self.scene);
    }

    // ========== 撤销/重做 API ==========

    /// 撤销上一个操作
    ///
    /// # 返回
    /// 是否成功撤销
    #[wasm_bindgen]
    pub fn undo(&mut self) -> bool {
        self.commit_handle_gesture();
        let undone = self.command_history.undo(&mut self.scene);
        if undone {
            self.sync_selection_after_history();
        }
        undone
    }

    /// 重做上一个撤销的操作
    ///
    /// # 返回
    /// 是否成功重做
    #[wasm_bindgen]
    pub fn redo(&mut self) -> bool {
        self.commit_handle_gesture();
        let redone = self.command_history.redo(&mut self.scene);
        if redone {
            self.sync_selection_after_history();
        }
        redone
    }

    /// 是否可以撤销
    #[wasm_bindgen]
    pub fn can_undo(&self) -> bool {
        self.command_history.can_undo()
    }

    /// 是否可以重做
    #[wasm_bindgen]
    pub fn can_redo(&self) -> bool {
        self.command_history.can_redo()
    }

    /// 获取撤销栈大小
    #[wasm_bindgen]
    pub fn get_undo_count(&self) -> usize {
        self.command_history.undo_count()
    }

    /// 获取重做栈大小
    #[wasm_bindgen]
    pub fn get_redo_count(&self) -> usize {
        self.command_history.redo_count()
    }

    // ========== 工具 API ==========

    /// 设置当前工具
    ///
    /// # 参数
    /// - `tool_name`: 工具名称 ("select", "rectangle", "circle", "pan", "line", "pen")
    ///
    /// 直线快捷键 `l`，画笔快捷键 `p`。切换工具会丢掉尚未抬笔的预览。
    ///
    /// # 返回
    /// 是否成功设置
    #[wasm_bindgen]
    pub fn set_tool(&mut self, tool_name: String) -> bool {
        if let Ok(tool) = Tool::from_str(&tool_name) {
            if self.tool_manager.current() != tool {
                self.commit_handle_gesture();
                self.cancel_drawing();
            }
            self.tool_manager.set_tool(tool);
            true
        } else {
            false
        }
    }

    /// 获取当前工具名称
    #[wasm_bindgen]
    pub fn get_current_tool(&self) -> String {
        self.tool_manager.current().name().to_string()
    }

    /// 获取当前工具的光标样式
    #[wasm_bindgen]
    pub fn get_tool_cursor(&self) -> String {
        self.tool_manager.current().cursor().to_string()
    }

    /// 临时切换到手型工具（按住空格）
    #[wasm_bindgen]
    pub fn push_hand_tool(&mut self) {
        self.tool_manager.push_temporary(Tool::Hand);
    }

    /// 恢复到之前的工具
    #[wasm_bindgen]
    pub fn pop_temporary_tool(&mut self) {
        self.tool_manager.pop_temporary();
    }

    // ========== UI 状态查询 API ==========

    /// 获取完整的画布状态（用于 UI 同步）
    ///
    /// # 返回
    /// 包含以下字段的对象：
    /// - tool: 当前工具名称
    /// - cursor: 光标样式
    /// - selectionCount: 选中元素数量
    /// - elementCount: 总元素数量
    /// - canUndo: 是否可撤销
    /// - canRedo: 是否可重做
    /// - isDragging: 是否正在拖动
    /// - isSelectionBoxActive: 是否正在框选
    /// - zoom: 当前缩放比例
    #[wasm_bindgen]
    pub fn get_state(&self) -> JsValue {
        let obj = Object::new();
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("tool"),
            &JsValue::from_str(self.tool_manager.current().name()),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("cursor"),
            &JsValue::from_str(self.tool_manager.current().cursor()),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("selectionCount"),
            &JsValue::from(self.selection.len() as u32),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("elementCount"),
            &JsValue::from(self.scene.count() as u32),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("canUndo"),
            &JsValue::from(self.command_history.can_undo()),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("canRedo"),
            &JsValue::from(self.command_history.can_redo()),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("isDragging"),
            &JsValue::from(self.drag_state.is_dragging),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("isSelectionBoxActive"),
            &JsValue::from(self.selection_box.is_active()),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("zoom"),
            &JsValue::from(self.viewport.zoom()),
        );
        obj.into()
    }

    /// 获取选中元素的边界框
    ///
    /// # 返回
    /// 如果有选中元素，返回 { x, y, width, height }；否则返回 null
    #[wasm_bindgen]
    pub fn get_selection_bounds(&self) -> JsValue {
        let selected_ids = self.selection.selected_ids();
        if selected_ids.is_empty() {
            return JsValue::NULL;
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for id in selected_ids {
            if let Some(element) = self.scene.get(id) {
                let bounds = element.as_element().world_bounds();
                min_x = min_x.min(bounds.x);
                min_y = min_y.min(bounds.y);
                max_x = max_x.max(bounds.x + bounds.width);
                max_y = max_y.max(bounds.y + bounds.height);
            }
        }

        if min_x == f32::MAX {
            return JsValue::NULL;
        }

        let obj = Object::new();
        let _ = Reflect::set(&obj, &JsValue::from_str("x"), &JsValue::from(min_x));
        let _ = Reflect::set(&obj, &JsValue::from_str("y"), &JsValue::from(min_y));
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("width"),
            &JsValue::from(max_x - min_x),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("height"),
            &JsValue::from(max_y - min_y),
        );
        obj.into()
    }

    /// 获取元素信息
    ///
    /// # 返回
    /// 如果元素存在，返回元素信息对象；否则返回 null
    #[wasm_bindgen]
    pub fn get_element_info(&self, id: u64) -> JsValue {
        if let Some(element) = self.scene.get(id) {
            let elem = element.as_element();
            let props = elem.props();
            let bounds = elem.world_bounds();

            let obj = Object::new();
            let _ = Reflect::set(&obj, &JsValue::from_str("id"), &JsValue::from(props.id));
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("type"),
                &JsValue::from_str(elem.type_name()),
            );
            let _ = Reflect::set(&obj, &JsValue::from_str("x"), &JsValue::from(bounds.x));
            let _ = Reflect::set(&obj, &JsValue::from_str("y"), &JsValue::from(bounds.y));
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("width"),
                &JsValue::from(bounds.width),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("height"),
                &JsValue::from(bounds.height),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("visible"),
                &JsValue::from(props.visible),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("selected"),
                &JsValue::from(props.selected),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("zIndex"),
                &JsValue::from(props.z_index),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("rotation"),
                &JsValue::from(props.rotation()),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("scaleX"),
                &JsValue::from(props.transform.scale.x),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("scaleY"),
                &JsValue::from(props.transform.scale.y),
            );
            obj.into()
        } else {
            JsValue::NULL
        }
    }

    #[wasm_bindgen]
    pub fn hit_test_screen_point(&self, x: f32, y: f32, max_results: Option<u32>) -> Array {
        let world = self.viewport.screen_to_world(Vec2::new(x, y));
        self.hit_test_world_point(world.x, world.y, max_results)
    }

    // ========== 场景管理 API ==========

    /// 添加矩形
    ///
    /// # 参数
    /// - `x, y`: 中心点坐标
    /// - `width, height`: 尺寸
    /// - `fill_color`: 填充颜色（可选，如 "#ff0000"）
    /// - `stroke_color`: 描边颜色（可选）
    /// - `stroke_width`: 描边宽度
    ///
    /// # 返回
    /// 元素 ID
    #[wasm_bindgen]
    pub fn add_rectangle(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        fill_color: Option<String>,
        stroke_color: Option<String>,
        stroke_width: f32,
    ) -> u64 {
        let id = self.scene.next_id();
        let mut style = ElementStyle::new();

        // 转换颜色字符串到 Color
        style.fill = fill_color.and_then(|s| Color::from_hex(&s).ok());
        style.stroke = stroke_color.and_then(|s| Color::from_hex(&s).ok());
        style.stroke_width = stroke_width;

        let rect = Rectangle::new(id, Vec2::new(x, y), Vec2::new(width, height)).with_style(style);
        self.scene.add_rect(rect);

        log::info!("Added rectangle {} at ({}, {})", id, x, y);
        id
    }

    /// 添加圆形
    ///
    /// # 参数
    /// - `x, y`: 中心点坐标
    /// - `radius`: 半径
    /// - `fill_color`: 填充颜色（可选）
    /// - `stroke_color`: 描边颜色（可选）
    /// - `stroke_width`: 描边宽度
    ///
    /// # 返回
    /// 元素 ID
    #[wasm_bindgen]
    pub fn add_circle(
        &mut self,
        x: f32,
        y: f32,
        radius: f32,
        fill_color: Option<String>,
        stroke_color: Option<String>,
        stroke_width: f32,
    ) -> u64 {
        let id = self.scene.next_id();
        let mut style = ElementStyle::new();

        // 转换颜色字符串到 Color
        style.fill = fill_color.and_then(|s| Color::from_hex(&s).ok());
        style.stroke = stroke_color.and_then(|s| Color::from_hex(&s).ok());
        style.stroke_width = stroke_width;

        let circle = Circle::new(id, Vec2::new(x, y), radius).with_style(style);
        self.scene.add_circle(circle);

        log::info!("Added circle {} at ({}, {})", id, x, y);
        id
    }

    /// 添加直线
    ///
    /// # 参数
    /// - `x1, y1`: 起点（世界坐标）
    /// - `x2, y2`: 终点（世界坐标）
    /// - `color`: 描边颜色（可选，如 "#111827"）
    /// - `stroke_width`: 线宽
    /// - `arrow`: 终点是否绘制箭头
    ///
    /// # 返回
    /// 元素 ID
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const id = canvas.add_line(10, 20, 180, 20, "#111827", 3, true);
    /// ```
    #[wasm_bindgen]
    pub fn add_line(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Option<String>,
        stroke_width: f32,
        arrow: bool,
    ) -> u64 {
        let id = self.scene.next_id();
        let stroke = color
            .and_then(|hex| Color::from_hex(&hex).ok())
            .unwrap_or(Color::BLACK);
        let style = ElementStyle::stroke_only(stroke, stroke_width);
        let line = Line::from_world(id, Vec2::new(x1, y1), Vec2::new(x2, y2), style, arrow);
        self.scene.add_line(line);
        log::info!("Added line {} ({}, {}) -> ({}, {})", id, x1, y1, x2, y2);
        id
    }

    /// 添加自由画笔路径
    ///
    /// # 参数
    /// - `points`: 扁平坐标 `[x0, y0, x1, y1, ...]`（世界坐标）
    /// - `color`: 描边颜色（可选）
    /// - `stroke_width`: 线宽
    ///
    /// # 返回
    /// 元素 ID
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const id = canvas.add_path(new Float32Array([0, 0, 40, 10, 40, 80]), "#111827", 2.5);
    /// ```
    #[wasm_bindgen]
    pub fn add_path(&mut self, points: &[f32], color: Option<String>, stroke_width: f32) -> u64 {
        let id = self.scene.next_id();
        let stroke = color
            .and_then(|hex| Color::from_hex(&hex).ok())
            .unwrap_or(Color::BLACK);
        let style = ElementStyle::stroke_only(stroke, stroke_width);
        let mut world_points = Vec::with_capacity(points.len() / 2);
        for chunk in points.chunks(2) {
            if chunk.len() == 2 {
                world_points.push(Vec2::new(chunk[0], chunk[1]));
            }
        }
        let path = Path::from_world_points(id, &world_points, style);
        self.scene.add_path(path);
        log::info!("Added path {} with {} points", id, world_points.len());
        id
    }

    /// 删除元素
    ///
    /// # 参数
    /// - `id`: 元素 ID
    ///
    /// # 返回
    /// 是否成功删除
    #[wasm_bindgen]
    pub fn remove_element(&mut self, id: u64) -> bool {
        let _ = self.selection.deselect(&mut self.scene, id);
        let removed = self.scene.remove(id).is_some();
        if removed {
            self.selection.remove_id(id);
            log::info!("Removed element {}", id);
        }
        removed
    }

    /// 清空场景
    #[wasm_bindgen]
    pub fn clear_scene(&mut self) {
        self.cancel_handle_gesture();
        self.selection.clear(&mut self.scene);
        self.selection_box.cancel();
        self.scene.clear();
        log::info!("Scene cleared");
    }

    /// 获取元素数量
    #[wasm_bindgen]
    pub fn get_element_count(&self) -> usize {
        self.scene.count()
    }

    /// 导出场景为版本化 JSON
    ///
    /// 文档含 `version`、`next_id` 和 `elements`。矩形、圆、图片、直线（含箭头）
    /// 和自由路径都会写入；变换的 `rotation` 与 `scale` 在 `props.transform` 里。
    /// `dirty` 是运行时标记，不会出现在 JSON 里。
    ///
    /// # 返回
    /// 紧凑 JSON 字符串
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const json = canvas.export_json();
    /// localStorage.setItem("infinitecanvas.scene.v1", json);
    /// ```
    #[wasm_bindgen]
    pub fn export_json(&self) -> Result<String, JsValue> {
        self.scene
            .to_json()
            .map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// 用版本化 JSON 替换当前场景
    ///
    /// 缺省字段使用默认值，未知字段会被忽略。导入会按 `selected` 标记重建选择，
    /// 并清空撤销栈和未完成的拖动。解析失败时场景保持不变。
    /// 成功后顶点缓存整表失效，下一帧全量上传。
    ///
    /// # 参数
    /// - `json`: [`export_json`](Self::export_json) 生成的文本
    ///
    /// # 错误
    /// JSON 无法解析、版本不受支持、元素种类未知或 ID 重复
    #[wasm_bindgen]
    pub fn import_json(&mut self, json: &str) -> Result<(), JsValue> {
        let scene = Scene::from_json(json).map_err(|err| JsValue::from_str(&err.to_string()))?;
        self.cancel_handle_gesture();
        self.scene = scene;
        self.selection.sync_from_scene(&self.scene);
        self.selection_box.cancel();
        self.command_history.clear();
        self.drag_state = DragState::default();
        if let Some(renderer) = &mut self.webgpu_renderer {
            renderer.invalidate_geometry();
        }
        log::info!("Imported scene with {} elements", self.scene.count());
        Ok(())
    }

    /// 把当前视口或选中区域导出为 PNG
    ///
    /// # 参数
    /// - `selection_only`: `true` 时导出选中元素的包围盒；`false` 时导出当前视口
    ///
    /// # 返回
    /// RGBA PNG 字节。直线、箭头和自由路径按线宽栅格化。图片纹理不在场景文件里，
    /// 导出为纯色占位。
    ///
    /// # 错误
    /// 没有选中元素、区域为空或编码失败
    #[wasm_bindgen]
    pub fn export_png(&self, selection_only: bool) -> Result<Vec<u8>, JsValue> {
        let bytes = if selection_only {
            export_selection_png(&self.scene, self.viewport.zoom())
        } else {
            export_viewport_png(&self.scene, &self.viewport)
        };
        bytes.map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// 用 `count` 个矩形替换当前场景，供压测页切换 1k/10k/100k。
    ///
    /// 元素按网格铺开。第一次渲染会建立顶点缓存，之后平移/缩放只更新 uniform。
    #[wasm_bindgen]
    pub fn load_stress_scene(&mut self, count: u32) {
        self.selection.clear(&mut self.scene);
        self.selection_box.cancel();
        self.scene.clear();

        let colors = [
            Color::from_hex("#4090f2").unwrap_or(Color::BLUE),
            Color::from_hex("#36d399").unwrap_or(Color::GREEN),
            Color::from_hex("#f24040").unwrap_or(Color::RED),
            Color::from_hex("#f2c94c").unwrap_or(Color::WHITE),
            Color::from_hex("#9b59b6").unwrap_or(Color::BLACK),
        ];
        let columns = 400u32;
        let spacing = 24.0f32;
        for index in 0..count {
            let id = self.scene.next_id();
            let column = (index % columns) as f32;
            let row = (index / columns) as f32;
            let color = colors[(index as usize) % colors.len()];
            let rect = Rectangle::new(
                id,
                Vec2::new(column * spacing, row * spacing),
                Vec2::new(16.0, 12.0),
            )
            .with_style(ElementStyle::fill_only(color));
            self.scene.add_rect(rect);
        }
        if let Some(renderer) = &mut self.webgpu_renderer {
            renderer.invalidate_geometry();
        }
        log::info!("Loaded stress scene with {} rectangles", count);
    }

    // ========== 纹理/图片 API ==========

    /// 上传纹理到 GPU
    ///
    /// # 参数
    /// - `texture_id`: 纹理的唯一标识符
    /// - `width`: 图片宽度（像素）
    /// - `height`: 图片高度（像素）
    /// - `data`: RGBA8 图像数据（Uint8Array，长度应为 width * height * 4）
    ///
    /// # 返回
    /// - `Ok(())`: 上传成功
    /// - `Err(JsValue)`: 上传失败（错误信息）
    ///
    /// # JavaScript 示例
    /// ```js
    /// // 从 ImageData 上传
    /// const imageData = ctx.getImageData(0, 0, img.width, img.height);
    /// canvas.upload_texture("logo", img.width, img.height, imageData.data);
    /// ```
    #[wasm_bindgen]
    pub fn upload_texture(
        &mut self,
        texture_id: String,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<(), JsValue> {
        log::info!("📥 Receiving texture data from JS:");
        log::info!("  - Texture ID: {}", texture_id);
        log::info!("  - Width: {}", width);
        log::info!("  - Height: {}", height);
        log::info!("  - Data length: {} bytes", data.len());
        log::info!("  - Expected length: {} bytes", width * height * 4);

        // 检查前几个字节
        if data.len() >= 20 {
            log::info!("  - First 20 bytes: {:?}", &data[0..20]);
            log::info!(
                "  - Sample pixel at (0,0): R={}, G={}, B={}, A={}",
                data[0],
                data[1],
                data[2],
                data[3]
            );
        }

        let renderer = self.webgpu_renderer.as_mut().ok_or_else(|| {
            JsValue::from_str("WebGPU renderer not initialized. Call init() first.")
        })?;

        renderer
            .upload_texture(texture_id.clone(), width, height, data)
            .map_err(|e| JsValue::from_str(&e))?;

        log::info!(
            "✅ Uploaded texture '{}': {}x{} ({} KB)",
            texture_id,
            width,
            height,
            data.len() / 1024
        );

        Ok(())
    }

    /// 添加图片元素
    ///
    /// # 参数
    /// - `x, y`: 左上角位置
    /// - `width, height`: 显示尺寸
    /// - `texture_id`: 纹理 ID（必须先通过 upload_texture 上传）
    ///
    /// # 返回
    /// 元素 ID
    ///
    /// # JavaScript 示例
    /// ```js
    /// // 上传纹理
    /// await canvas.upload_texture("logo", 256, 256, imageData);
    ///
    /// // 添加图片元素
    /// const imageId = canvas.add_image(100, 100, 200, 150, "logo");
    /// ```
    #[wasm_bindgen]
    pub fn add_image(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        texture_id: String,
    ) -> u64 {
        let id = self.scene.next_id();
        let image = Image::new(
            id,
            Vec2::new(x, y),
            Vec2::new(width, height),
            texture_id.clone(),
        );
        self.scene.add_image(image);

        log::info!(
            "Added image {} at ({}, {}) with texture '{}'",
            id,
            x,
            y,
            texture_id
        );
        id
    }

    /// 设置图片透明度
    ///
    /// # 参数
    /// - `image_id`: 图片元素 ID
    /// - `alpha`: 透明度 [0.0 - 1.0]
    ///
    /// # 返回
    /// 是否成功设置
    #[wasm_bindgen]
    pub fn set_image_alpha(&mut self, image_id: u64, alpha: f32) -> bool {
        let updated = if let Some(AnyElement::Image(image)) = self.scene.get_mut(image_id) {
            image.set_alpha(alpha);
            true
        } else {
            false
        };
        if updated {
            self.scene.mark_dirty(image_id);
            log::debug!("Set image {} alpha to {}", image_id, alpha);
        } else {
            log::warn!("Image {} not found", image_id);
        }
        updated
    }

    /// 清空场景中所有元素
    ///
    /// # JavaScript 示例
    /// ```js
    /// canvas.clear(); // 清空所有元素
    /// ```
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        let count = self.scene.count();
        self.cancel_handle_gesture();
        self.selection.clear(&mut self.scene);
        self.selection_box.cancel();
        self.scene.clear();
        log::info!("Cleared scene ({} elements removed)", count);
    }

    // ========== 视口操作 API ==========

    /// 平移视口
    ///
    /// # 参数
    /// - `dx, dy`: 移动量（世界坐标）
    #[wasm_bindgen]
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.viewport.translate(Vec2::new(dx, dy));
        log::debug!("Viewport panned by ({}, {})", dx, dy);
    }

    /// 缩放视口
    ///
    /// # 参数
    /// - `anchor_x, anchor_y`: 缩放中心点（屏幕坐标）
    /// - `delta_zoom`: 缩放倍数（例如 1.1 放大 10%）
    #[wasm_bindgen]
    pub fn zoom_at(&mut self, anchor_x: f32, anchor_y: f32, delta_zoom: f32) {
        self.viewport
            .zoom_at(Vec2::new(anchor_x, anchor_y), delta_zoom);
        log::debug!(
            "Viewport zoomed at ({}, {}) by {}",
            anchor_x,
            anchor_y,
            delta_zoom
        );
    }

    /// 获取视口缩放
    #[wasm_bindgen]
    pub fn get_zoom(&self) -> f32 {
        self.viewport.zoom()
    }

    /// 获取视口中心 X
    #[wasm_bindgen]
    pub fn get_viewport_center_x(&self) -> f32 {
        self.viewport.center().x
    }

    /// 获取视口中心 Y
    #[wasm_bindgen]
    pub fn get_viewport_center_y(&self) -> f32 {
        self.viewport.center().y
    }

    // ========== 性能监控 API ==========

    /// 获取性能统计信息
    ///
    /// # 返回
    /// 包含以下字段的对象：
    /// - frameCount: 总帧数
    /// - vertexCount: 上一帧顶点数
    /// - drawCalls: 上一帧绘制调用数
    /// - frameTimeMs: 上一帧渲染时间（毫秒，CPU 提交，不含 GPU 完成）
    /// - elementCount: 场景元素总数
    /// - regeneratedElements: 本帧重生成顶点的元素数
    /// - gpuBytesWritten: 本帧写入顶点缓冲的字节数
    /// - geometryUnchanged: 为 true 时本帧只更新了视口 uniform
    #[wasm_bindgen]
    pub fn get_performance_stats(&self) -> JsValue {
        let obj = Object::new();
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("frameCount"),
            &JsValue::from(self.perf_stats.frame_count as f64),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("vertexCount"),
            &JsValue::from(self.perf_stats.last_vertex_count as u32),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("drawCalls"),
            &JsValue::from(self.perf_stats.last_draw_calls as u32),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("frameTimeMs"),
            &JsValue::from(self.perf_stats.last_frame_time_ms),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("elementCount"),
            &JsValue::from(self.scene.count() as u32),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("regeneratedElements"),
            &JsValue::from(self.perf_stats.regenerated_elements as u32),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("gpuBytesWritten"),
            &JsValue::from(self.perf_stats.gpu_bytes_written as f64),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("geometryUnchanged"),
            &JsValue::from(self.perf_stats.geometry_unchanged),
        );
        obj.into()
    }

    /// 获取帧计数
    #[wasm_bindgen]
    pub fn get_frame_count(&self) -> u64 {
        self.perf_stats.frame_count
    }

    /// 获取上一帧顶点数
    #[wasm_bindgen]
    pub fn get_last_vertex_count(&self) -> usize {
        self.perf_stats.last_vertex_count
    }

    /// 获取上一帧绘制调用数
    #[wasm_bindgen]
    pub fn get_last_draw_calls(&self) -> usize {
        self.perf_stats.last_draw_calls
    }

    /// 指针下的光标。选择工具命中缩放/旋转手柄时返回对应光标。
    ///
    /// # 参数
    /// - `x`, `y`: 屏幕坐标（与指针事件相同）
    #[wasm_bindgen]
    pub fn cursor_at(&self, x: f32, y: f32) -> String {
        if self.tool_manager.current() != Tool::Select {
            return self.tool_manager.current().cursor().to_string();
        }
        if let Some(drag) = &self.handle_drag {
            return cursor_for(drag.gesture.kind(), drag.live_frame.rotation).to_string();
        }
        let world = self.viewport.screen_to_world(Vec2::new(x, y));
        if let Some(frame) = self.compute_selection_frame() {
            let metrics = HandleMetrics::from_zoom(self.viewport.zoom());
            if let Some(kind) = frame.hit_test(world, metrics.hit_radius, metrics.rotate_gap) {
                return cursor_for(kind, frame.rotation).to_string();
            }
        }
        self.tool_manager.current().cursor().to_string()
    }
}

impl InfiniteCanvas {
    fn dispatch_pending_events(&mut self) {
        while let Some(event) = self.event_queue.pop() {
            self.handle_event(event);
        }
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::PointerDown(pointer) => self.handle_pointer_down(pointer),
            Event::PointerMove(pointer) => self.handle_pointer_move(pointer),
            Event::PointerUp(pointer) => self.handle_pointer_up(pointer),
            _ => {}
        }
    }

    fn handle_pointer_down(&mut self, event: PointerEvent) {
        if event.button != Some(PointerButton::Primary) {
            return;
        }

        let current_tool = self.tool_manager.current();

        match current_tool {
            Tool::Select => self.handle_select_tool_down(event),
            Tool::Rectangle | Tool::Circle => self.handle_draw_tool_down(event, current_tool),
            Tool::Line => self.handle_line_tool_down(event),
            Tool::Pen => self.handle_pen_tool_down(event),
            Tool::Pan | Tool::Hand => self.handle_pan_tool_down(event),
        }
    }

    fn handle_pointer_move(&mut self, event: PointerEvent) {
        let current_tool = self.tool_manager.current();

        match current_tool {
            Tool::Select => self.handle_select_tool_move(event),
            Tool::Rectangle | Tool::Circle => self.handle_draw_tool_move(event),
            Tool::Line => self.handle_line_tool_move(event),
            Tool::Pen => self.handle_pen_tool_move(event),
            Tool::Pan | Tool::Hand => self.handle_pan_tool_move(event),
        }
    }

    fn handle_pointer_up(&mut self, event: PointerEvent) {
        let current_tool = self.tool_manager.current();

        match current_tool {
            Tool::Select => self.handle_select_tool_up(event),
            Tool::Rectangle | Tool::Circle => self.handle_draw_tool_up(event, current_tool),
            Tool::Line => self.handle_line_tool_up(event),
            Tool::Pen => self.handle_pen_tool_up(event),
            Tool::Pan | Tool::Hand => self.handle_pan_tool_up(event),
        }
    }

    // ========== 选择工具事件处理 ==========

    fn handle_select_tool_down(&mut self, event: PointerEvent) {
        if self.try_begin_handle_gesture(&event) {
            return;
        }
        let options = HitTestOptions {
            max_results: 1,
            include_invisible: false,
        };
        let hits = hit_test_point(&self.scene, event.world_position, &options);
        let additive = event.modifiers.ctrl || event.modifiers.meta || event.modifiers.shift;

        if let Some(hit) = hits.first() {
            // 点击到已选中的元素：开始拖动
            if self.selection.is_selected(hit.id) {
                self.start_drag(event.pointer_id, event.world_position);
                self.selection_box.cancel();
            } else {
                // 点击到未选中的元素：选中它，并开始拖动
                self.selection
                    .select_element(&mut self.scene, hit.id, additive);
                self.start_drag(event.pointer_id, event.world_position);
                self.selection_box.cancel();
            }
        } else {
            // 点击空白区域：开始框选
            if !additive {
                self.selection.clear(&mut self.scene);
            }
            self.selection_box
                .start(event.pointer_id, event.world_position, additive);
        }
    }

    fn handle_select_tool_move(&mut self, event: PointerEvent) {
        if self.update_handle_gesture(&event) {
            return;
        }
        // 如果正在拖动，移动选中的元素
        if self.drag_state.is_dragging && self.drag_state.pointer_id == Some(event.pointer_id) {
            let delta = event.world_position - self.drag_state.last_position;
            self.move_selected_elements_internal(delta);
            self.drag_state.last_position = event.world_position;
            self.drag_state.total_delta += delta;
        } else {
            // 否则更新框选
            self.selection_box
                .update(event.pointer_id, event.world_position);
        }
    }

    fn handle_select_tool_up(&mut self, event: PointerEvent) {
        if self.finish_handle_gesture(event.pointer_id) {
            return;
        }
        // 结束拖动
        if self.drag_state.is_dragging && self.drag_state.pointer_id == Some(event.pointer_id) {
            self.finish_drag();
            return;
        }

        // 结束框选
        if let Some((rect, additive)) = self.selection_box.finish(event.pointer_id) {
            let hits = self
                .selection
                .select_in_rect(&mut self.scene, rect, additive);
            log::trace!("Selection box picked {} elements", hits.len());
        }
    }

    // ========== 绘制工具事件处理 ==========

    fn handle_draw_tool_down(&mut self, event: PointerEvent, _tool: Tool) {
        // 开始绘制：记录起始点
        self.drag_state.is_dragging = true;
        self.drag_state.pointer_id = Some(event.pointer_id);
        self.drag_state.start_position = event.world_position;
        self.drag_state.last_position = event.world_position;
        self.drag_state.total_delta = Vec2::ZERO;

        // 清除当前选择
        self.selection.clear(&mut self.scene);
    }

    fn handle_draw_tool_move(&mut self, event: PointerEvent) {
        if self.drag_state.is_dragging && self.drag_state.pointer_id == Some(event.pointer_id) {
            self.drag_state.last_position = event.world_position;
            // 绘制预览由渲染器处理
        }
    }

    fn handle_draw_tool_up(&mut self, event: PointerEvent, tool: Tool) {
        if !self.drag_state.is_dragging || self.drag_state.pointer_id != Some(event.pointer_id) {
            return;
        }

        let start = self.drag_state.start_position;
        let end = event.world_position;

        // 计算矩形尺寸
        let min_x = start.x.min(end.x);
        let min_y = start.y.min(end.y);
        let width = (end.x - start.x).abs();
        let height = (end.y - start.y).abs();

        // 只有在有意义的尺寸时才创建元素
        const MIN_SIZE: f32 = 5.0;
        if width > MIN_SIZE && height > MIN_SIZE {
            let id = self.scene.next_id();
            let center = Vec2::new(min_x + width / 2.0, min_y + height / 2.0);

            match tool {
                Tool::Rectangle => {
                    let style =
                        ElementStyle::fill_only(Color::from_hex("#4090f2").unwrap_or(Color::BLUE));
                    let rect =
                        Rectangle::new(id, center, Vec2::new(width, height)).with_style(style);
                    self.scene.add_rect(rect);
                    log::info!(
                        "Created rectangle {} at {:?}, size {:?}",
                        id,
                        center,
                        (width, height)
                    );
                }
                Tool::Circle => {
                    let radius = width.min(height) / 2.0;
                    let style =
                        ElementStyle::fill_only(Color::from_hex("#36d399").unwrap_or(Color::GREEN));
                    let circle = Circle::new(id, center, radius).with_style(style);
                    self.scene.add_circle(circle);
                    log::info!("Created circle {} at {:?}, radius {}", id, center, radius);
                }
                _ => {}
            }

            // 选中新创建的元素
            self.selection.select_element(&mut self.scene, id, false);
        }

        self.drag_state.is_dragging = false;
        self.drag_state.pointer_id = None;
    }

    // ========== 直线 / 画笔 ==========

    fn handle_line_tool_down(&mut self, event: PointerEvent) {
        self.selection.clear(&mut self.scene);
        self.cancel_drawing();
        self.drag_state.is_dragging = true;
        self.drag_state.pointer_id = Some(event.pointer_id);
        self.drag_state.start_position = event.world_position;
        self.drag_state.last_position = event.world_position;

        let id = self.scene.next_id();
        let style =
            ElementStyle::stroke_only(Color::from_hex("#1f2937").unwrap_or(Color::BLACK), 3.0);
        let line = Line::from_world(
            id,
            event.world_position,
            event.world_position,
            style,
            event.modifiers.shift,
        );
        self.scene.add_line(line);
        self.drag_state.preview_id = Some(id);
    }

    fn handle_line_tool_move(&mut self, event: PointerEvent) {
        if !self.drag_state.is_dragging || self.drag_state.pointer_id != Some(event.pointer_id) {
            return;
        }
        self.drag_state.last_position = event.world_position;
        self.update_line_preview(event.world_position, event.modifiers.shift);
    }

    fn handle_line_tool_up(&mut self, event: PointerEvent) {
        if !self.drag_state.is_dragging || self.drag_state.pointer_id != Some(event.pointer_id) {
            return;
        }
        let end = event.world_position;
        self.update_line_preview(end, event.modifiers.shift);
        let long_enough = (end - self.drag_state.start_position).length() >= MIN_STROKE_LENGTH;
        self.drag_state.is_dragging = false;
        self.drag_state.pointer_id = None;
        if long_enough {
            self.commit_preview();
        } else {
            self.discard_preview();
        }
    }

    fn update_line_preview(&mut self, world_end: Vec2, arrow: bool) {
        let Some(id) = self.drag_state.preview_id else {
            return;
        };
        if let Some(AnyElement::Line(line)) = self.scene.get_mut(id) {
            line.set_world_end(world_end);
            line.set_arrow(arrow);
        }
        self.scene.mark_dirty(id);
    }

    fn handle_pen_tool_down(&mut self, event: PointerEvent) {
        self.selection.clear(&mut self.scene);
        self.cancel_drawing();
        self.drag_state.is_dragging = true;
        self.drag_state.pointer_id = Some(event.pointer_id);
        self.drag_state.start_position = event.world_position;
        self.drag_state.last_position = event.world_position;
        self.drag_state.pen_points.push(event.world_position);
    }

    fn handle_pen_tool_move(&mut self, event: PointerEvent) {
        if !self.drag_state.is_dragging || self.drag_state.pointer_id != Some(event.pointer_id) {
            return;
        }
        self.push_pen_point(event.world_position, PEN_SAMPLE_SPACING);
    }

    fn handle_pen_tool_up(&mut self, event: PointerEvent) {
        if !self.drag_state.is_dragging || self.drag_state.pointer_id != Some(event.pointer_id) {
            return;
        }
        self.push_pen_point(event.world_position, 0.5);
        let long_enough = self.drag_state.pen_points.len() >= 2
            && polyline_length(&self.drag_state.pen_points) >= MIN_STROKE_LENGTH;
        self.drag_state.is_dragging = false;
        self.drag_state.pointer_id = None;
        if long_enough {
            self.commit_preview();
        } else {
            self.discard_preview();
        }
        self.drag_state.pen_points.clear();
    }

    fn push_pen_point(&mut self, point: Vec2, min_spacing: f32) {
        let last = self.drag_state.pen_points.last().copied().unwrap_or(point);
        if (point - last).length() < min_spacing {
            return;
        }
        self.drag_state.pen_points.push(point);
        self.drag_state.last_position = point;
        self.sync_pen_preview();
    }

    fn sync_pen_preview(&mut self) {
        if self.drag_state.pen_points.len() < 2 {
            return;
        }
        let points = self.drag_state.pen_points.clone();
        if let Some(id) = self.drag_state.preview_id {
            let updated = if let Some(AnyElement::Path(path)) = self.scene.get_mut(id) {
                path.set_world_points(&points);
                true
            } else {
                false
            };
            if updated {
                self.scene.mark_dirty(id);
                return;
            }
        }
        let id = self.scene.next_id();
        let style =
            ElementStyle::stroke_only(Color::from_hex("#111827").unwrap_or(Color::BLACK), 2.5);
        let path = Path::from_world_points(id, &points, style);
        self.scene.add_path(path);
        self.drag_state.preview_id = Some(id);
    }

    /// 把预览元素正式写入场景，并记入撤销栈。
    fn commit_preview(&mut self) {
        let Some(id) = self.drag_state.preview_id.take() else {
            return;
        };
        let Some(element) = self.scene.remove(id) else {
            return;
        };
        self.command_history
            .execute(Box::new(AddElementCommand::new(element)), &mut self.scene);
        self.selection.select_element(&mut self.scene, id, false);
        log::info!("Committed stroke {}", id);
    }

    fn discard_preview(&mut self) {
        if let Some(id) = self.drag_state.preview_id.take() {
            self.scene.remove(id);
        }
        self.drag_state.pen_points.clear();
    }

    fn cancel_drawing(&mut self) {
        self.discard_preview();
        self.drag_state.is_dragging = false;
        self.drag_state.pointer_id = None;
    }

    /// 撤销/重做后，选择集与元素上的 selected 标记保持一致。
    fn sync_selection_after_history(&mut self) {
        let stale: Vec<ElementId> = self
            .selection
            .selected_ids()
            .into_iter()
            .filter(|id| !self.scene.contains(*id))
            .collect();
        for id in stale {
            self.selection.remove_id(id);
        }
        let flagged: Vec<ElementId> = self
            .scene
            .iter()
            .filter(|element| element.as_element().props().selected)
            .map(|element| element.id())
            .collect();
        for id in flagged {
            self.selection.select_element(&mut self.scene, id, true);
        }
    }

    // ========== 平移工具事件处理 ==========

    fn handle_pan_tool_down(&mut self, event: PointerEvent) {
        self.drag_state.is_dragging = true;
        self.drag_state.pointer_id = Some(event.pointer_id);
        self.drag_state.last_position = event.screen_position;
    }

    fn handle_pan_tool_move(&mut self, event: PointerEvent) {
        if self.drag_state.is_dragging && self.drag_state.pointer_id == Some(event.pointer_id) {
            let delta = event.screen_position - self.drag_state.last_position;
            self.viewport.translate(delta);
            self.drag_state.last_position = event.screen_position;
        }
    }

    fn handle_pan_tool_up(&mut self, event: PointerEvent) {
        if self.drag_state.is_dragging && self.drag_state.pointer_id == Some(event.pointer_id) {
            self.drag_state.is_dragging = false;
            self.drag_state.pointer_id = None;
        }
    }

    /// 开始拖动
    fn start_drag(&mut self, pointer_id: i32, position: Vec2) {
        self.drag_state.is_dragging = true;
        self.drag_state.pointer_id = Some(pointer_id);
        self.drag_state.start_position = position;
        self.drag_state.last_position = position;
        self.drag_state.total_delta = Vec2::ZERO;
        self.drag_state.dragged_ids = self.selection.selected_ids();
    }

    /// 结束拖动（创建移动命令添加到历史）
    fn finish_drag(&mut self) {
        // 只有在有实际移动时才创建命令
        const MIN_MOVE_THRESHOLD: f32 = 0.1;
        if self.drag_state.total_delta.length() > MIN_MOVE_THRESHOLD
            && !self.drag_state.dragged_ids.is_empty()
        {
            // 注意：元素已经被移动了，这里我们需要先撤销移动，然后让命令重新执行
            // 或者直接创建一个已执行的命令添加到历史
            // 为了简单起见，我们直接记录命令（不重复执行）
            let cmd = MoveCommand::new(
                self.drag_state.dragged_ids.clone(),
                self.drag_state.total_delta,
            );
            // 直接添加到撤销栈，而不是 execute（因为移动已经发生了）
            self.command_history.push_executed(Box::new(cmd));
        }

        self.drag_state.is_dragging = false;
        self.drag_state.pointer_id = None;
        self.drag_state.total_delta = Vec2::ZERO;
        self.drag_state.dragged_ids.clear();
    }

    /// 把当前选中元素收成一个选中框。单选跟元素旋转对齐，多选用包围盒。
    fn compute_selection_frame(&self) -> Option<SelectionFrame> {
        let metrics = HandleMetrics::from_zoom(self.viewport.zoom());
        let elements = self.selected_element_refs();
        SelectionFrame::from_elements(&elements).map(|frame| frame.inflated(metrics.pad))
    }

    /// 绘制用的选中框。手柄拖动中用实时预览，否则按当前选中重算。
    fn display_selection_frame(&self) -> Option<SelectionFrame> {
        self.handle_drag
            .as_ref()
            .map(|drag| drag.live_frame)
            .or_else(|| self.compute_selection_frame())
    }

    fn selected_element_refs(&self) -> Vec<&dyn infinitecanvas_engine::Element> {
        let mut elements = Vec::new();
        for id in self.selection.selected_ids() {
            if let Some(element) = self.scene.get(id) {
                let element = element.as_element();
                if element.props().visible {
                    elements.push(element);
                }
            }
        }
        elements
    }

    fn selected_transforms(&self) -> Vec<(ElementId, Transform)> {
        self.selection
            .selected_ids()
            .into_iter()
            .filter_map(|id| {
                self.scene
                    .get(id)
                    .map(|element| (id, element.as_element().props().transform))
            })
            .collect()
    }

    fn apply_transform_list(&mut self, transforms: &[(ElementId, Transform)]) {
        for (id, transform) in transforms {
            if let Some(element) = self.scene.get_mut(*id) {
                element.as_element_mut().props_mut().transform = *transform;
            }
            // 实时缩放/旋转、多选和取消都走这里。撤销走 TransformElementsCommand。
            self.scene.mark_dirty(*id);
        }
    }

    /// 指针落在手柄上时开始缩放或旋转，不改选择集。
    fn try_begin_handle_gesture(&mut self, event: &PointerEvent) -> bool {
        if self.handle_drag.is_some() {
            self.commit_handle_gesture();
        }
        let Some(frame) = self.compute_selection_frame() else {
            return false;
        };
        let metrics = HandleMetrics::from_zoom(self.viewport.zoom());
        let Some(kind) =
            frame.hit_test(event.world_position, metrics.hit_radius, metrics.rotate_gap)
        else {
            return false;
        };
        let originals = self.selected_transforms();
        if originals.is_empty() {
            return false;
        }
        let gesture = HandleGesture::start(frame, kind, event.world_position, originals.clone());
        self.selection_box.cancel();
        self.handle_drag = Some(HandleInteraction {
            pointer_id: event.pointer_id,
            gesture,
            live_frame: frame,
            latest: originals,
        });
        true
    }

    fn update_handle_gesture(&mut self, event: &PointerEvent) -> bool {
        let preview = {
            let Some(drag) = &self.handle_drag else {
                return false;
            };
            if drag.pointer_id != event.pointer_id {
                return false;
            }
            let metrics = HandleMetrics::from_zoom(self.viewport.zoom());
            drag.gesture.preview(
                event.world_position,
                event.modifiers.shift,
                metrics.min_half,
            )
        };
        self.apply_transform_list(&preview.transforms);
        if let Some(drag) = &mut self.handle_drag {
            drag.live_frame = preview.frame;
            drag.latest = preview.transforms;
        }
        true
    }

    fn finish_handle_gesture(&mut self, pointer_id: i32) -> bool {
        let matches = self
            .handle_drag
            .as_ref()
            .is_some_and(|drag| drag.pointer_id == pointer_id);
        if matches {
            self.commit_handle_gesture();
        }
        matches
    }

    /// 把手势写入撤销栈。场景里已经是最新变换，所以不再 execute。
    fn commit_handle_gesture(&mut self) {
        let Some(drag) = self.handle_drag.take() else {
            return;
        };
        let before = drag.gesture.originals().to_vec();
        if transforms_differ(&before, &drag.latest) {
            self.command_history
                .push_executed(Box::new(TransformElementsCommand::new(before, drag.latest)));
        }
    }

    /// 丢掉未提交的手柄拖动，元素回到按下时的变换。
    fn cancel_handle_gesture(&mut self) {
        if let Some(drag) = self.handle_drag.take() {
            let originals = drag.gesture.originals().to_vec();
            self.apply_transform_list(&originals);
        }
    }

    /// 移动所有选中的元素（内部方法，不创建命令）
    fn move_selected_elements_internal(&mut self, delta: Vec2) {
        for id in &self.drag_state.dragged_ids {
            if let Some(element) = self.scene.get_mut(*id) {
                element
                    .as_element_mut()
                    .props_mut()
                    .transform
                    .translate(delta);
            }
            self.scene.mark_dirty(*id);
        }
    }
}

fn polyline_length(points: &[Vec2]) -> f32 {
    points
        .windows(2)
        .map(|window| (window[1] - window[0]).length())
        .sum()
}

fn hit_results_to_js_array(hits: Vec<HitTestResult>) -> Array {
    let array = Array::new();
    for hit in hits {
        let obj = Object::new();
        let _ = Reflect::set(&obj, &JsValue::from_str("id"), &JsValue::from(hit.id));
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("zIndex"),
            &JsValue::from(hit.z_index),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("elementType"),
            &JsValue::from_str(hit.element_type),
        );
        array.push(&obj);
    }
    array
}
fn selection_ids_to_js_array(ids: Vec<ElementId>) -> Array {
    let array = Array::new();
    for id in ids {
        array.push(&JsValue::from(id));
    }
    array
}
