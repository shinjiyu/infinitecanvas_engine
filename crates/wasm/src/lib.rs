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
mod style_api;
mod texture_manager;

use infinitecanvas_engine::minimap::{self, MinimapFrame};
use infinitecanvas_engine::{
    apply_align, apply_distribute, apply_flip, apply_nudge, apply_rename_frame,
    apply_reparent_into_frame, apply_rotate_quarter, apply_set_locked, apply_set_visibility,
    apply_stack_reorder, apply_toggle_locked, apply_toggle_visibility,
    element::text::{
        lod_bucket, raster_params, raster_texture_id, DEFAULT_FONT_SIZE, DEFAULT_TEXT_COLOR,
        DEFAULT_TEXT_CONTENT,
    },
    export_frame_png, export_frame_svg, export_selection_png, export_svg, export_viewport_png,
    guides::{
        handle_outward, moving_edge, resize_edge_correction, ruler_band, ruler_ticks,
        GuideSnapshot, RulerBand, RulerOrientation, GUIDE_HIT_PX, RULER_THICKNESS_PX,
    },
    hit_test::{click_target, hit_test_point, HitTestOptions, HitTestResult},
    import_svg as import_svg_scene,
    math::{Color, Rect, Transform, Vec2},
    sample_paint_at,
    snap::{
        bounds_union, snap_drag_with_guides, AlignmentGuide, DragSnap, GuideAxis, GuideLines,
        SnapSettings,
    },
    transform_handles::{
        cursor_for, selection_locks_image_aspect, transforms_differ, HandleGesture, HandleKind,
        HandleMetrics, SelectionFrame,
    },
    zoom_viewport_to_scene, zoom_viewport_to_selection, AddElementCommand, AlignMode, AnyElement,
    Circle, CommandHistory, DistributeAxis, DuplicateElementsCommand, EditGuidesCommand, Element,
    ElementId, ElementStyle, Event, EventQueue, FlipAxis, Frame, Image, Line, MoveCommand,
    NudgeDirection, OutlineSnapshot, PasteElementsCommand, Path, PointerButton, PointerEvent,
    Polygon, QuarterTurn, Rectangle, RemoveElementCommand, RemoveElementsCommand,
    ReorderOutlineCommand, Scene, SceneTreeNode, SelectionBox, SelectionManager, StackOrderAction,
    Text, TextAlign, TextClickResolver, TextPlacement, Tool, ToolManager, TransformElementsCommand,
    UpdateTextCommand, Viewport, ZOOM_STEP,
};

/// 直线 / 画笔抬笔时短于该长度则丢弃
const MIN_STROKE_LENGTH: f32 = 2.0;
/// 粘贴图片时显示尺寸的长边上限（世界单位）。像素尺寸仍记在资源上。
const MAX_PASTED_DISPLAY_EDGE: f32 = 512.0;
/// 画笔采样的最小间距，避免点过密
const PEN_SAMPLE_SPACING: f32 = 1.5;
use input::{
    keyboard_event_from_js, modifiers_from_flags, pointer_event_from_js, wheel_event_from_js,
    KeyboardEventKind, PointerEventKind,
};
use js_sys::{Array, Object, Reflect};
use renderer::WebGPURenderer;
use std::collections::HashSet;
use std::str::FromStr;
use wasm_bindgen::prelude::*;
use web_time::Instant;

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
    /// 按下时选区的世界包围盒。吸附以它为对齐主体，多选时是并集。
    origin_bounds: Option<Rect>,
    /// 当前帧生效的对齐参考线。松手或关闭吸附后清空。
    guides: Vec<AlignmentGuide>,
    /// 自由画笔采样点（世界坐标）
    pen_points: Vec<Vec2>,
    /// 绘制过程中的预览元素；抬笔后才进入撤销栈
    preview_id: Option<ElementId>,
}

/// 从标尺拖出或拖动一条手动参考线。松手后才进撤销栈。
struct GuideGesture {
    pointer_id: i32,
    axis: GuideAxis,
    /// 已经写进场景的参考线。还在标尺上的新建手势是 `None`。
    id: Option<u64>,
    /// 拖动已有参考线时的原始位置。从标尺新建时是 `None`。
    origin: Option<f32>,
    before: GuideSnapshot,
}

/// 缩放 / 旋转手柄的拖动。预览已经写进场景，松手后才进入撤销栈。
struct HandleInteraction {
    pointer_id: i32,
    gesture: HandleGesture,
    live_frame: SelectionFrame,
    latest: Vec<(ElementId, Transform)>,
}

/// T 工具或双击产生的编辑请求。前端取走后打开 DOM 输入框。
struct TextEditRequest {
    id: ElementId,
    select_all: bool,
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

/// 吸管一次点击的结果。按下时记下，松开后交给页面。
struct EyedropperGesture {
    /// 点到了可见叶子。
    hit: bool,
    /// 颜色写进了当前选择。
    applied: bool,
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
    text_clicks: TextClickResolver,
    pending_text_edit: Option<TextEditRequest>,
    perf_stats: PerformanceStats,
    /// 拖拽吸附。默认开启，阈值是屏幕像素。
    snap: SnapSettings,
    /// 标尺和手动参考线是否显示。隐藏后不命中、不吸附。
    rulers_visible: bool,
    /// 背景网格是否显示。默认可见，隐藏不进撤销栈。
    grid_visible: bool,
    /// 正在拖的参考线。
    guide_gesture: Option<GuideGesture>,
    /// 当前选中的参考线。和元素选择互斥。
    selected_guide: Option<u64>,
    /// 属性面板拖动中的样式基线。松手后收成一条撤销记录。
    style_preview: Option<style_api::StylePreview>,
    /// 小地图缩略图。几何版本或尺寸变了才重算，平移缩放不扫描元素。
    minimap_cache: Option<MinimapCache>,
    /// 已经上传到 GPU 的图片资源 id。和场景引用比较，多退少补。
    gpu_image_assets: HashSet<String>,
    /// 吸管按下后、松开前的结果。
    eyedropper_gesture: Option<EyedropperGesture>,
    /// 页面下一帧读走的吸管结果。
    eyedropper_notice: Option<EyedropperGesture>,
}

/// 已映射到像素的小地图外框。`epoch` 来自 [`infinitecanvas_engine::Scene::geometry_epoch`]。
struct MinimapCache {
    epoch: u64,
    width: f32,
    height: f32,
    padding: f32,
    frame: MinimapFrame,
    /// `x, y, width, height` 交错的像素外框。
    rects: Vec<f32>,
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
            text_clicks: TextClickResolver::new(),
            pending_text_edit: None,
            perf_stats: PerformanceStats::default(),
            snap: SnapSettings::default(),
            rulers_visible: true,
            grid_visible: true,
            guide_gesture: None,
            selected_guide: None,
            style_preview: None,
            minimap_cache: None,
            gpu_image_assets: HashSet::new(),
            eyedropper_gesture: None,
            eyedropper_notice: None,
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
        self.sync_image_textures();
        let selection_box = self.selection_box.bounds();
        let transform_frame = self.display_selection_frame();
        if let Some(renderer) = &mut self.webgpu_renderer {
            let started = js_sys::Date::now();
            renderer.render(
                &mut self.scene,
                &self.viewport,
                selection_box,
                transform_frame,
                self.grid_visible,
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

    /// 选中顶层的可见元素。
    ///
    /// 组只选中组本身，不选中组内元素。隐藏元素不选。锁定的元素，以及被锁定祖先挡住的组，也不选。
    ///
    /// # 返回
    /// 选中数量。空场景返回 0。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.select_all();
    /// ```
    #[wasm_bindgen]
    pub fn select_all(&mut self) -> u32 {
        self.commit_handle_gesture();
        self.selection_box.cancel();
        self.selection.select_all(&mut self.scene) as u32
    }

    /// 删除当前选中的元素。
    ///
    /// 多个元素合成一条撤销记录。选中的是组时，整棵子树一起删除。
    /// 没有选中时什么也不做。快照里的选中标记会随撤销恢复。
    /// 删除后会像 [`Self::remove_element`] 一样，把这些 id（含子孙）从选择集里去掉，
    /// 并释放不再被场景使用的文本纹理。
    ///
    /// # 返回
    /// 从场景中删除的元素数量，含组的子孙
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.delete_selection();
    /// ```
    #[wasm_bindgen]
    pub fn delete_selection(&mut self) -> u32 {
        self.commit_handle_gesture();
        self.cancel_drawing();
        self.selection_box.cancel();
        self.drag_state.guides.clear();
        let ids = self.selection.selected_ids();
        if ids.is_empty() {
            return 0;
        }
        let mut affected = Vec::new();
        for id in &ids {
            for element_id in self.scene.subtree_ids(*id) {
                if !affected.contains(&element_id) {
                    affected.push(element_id);
                }
            }
        }
        let textures = self.snapshot_text_textures();
        let before = self.scene.count();
        self.command_history
            .execute(Box::new(RemoveElementsCommand::new(ids)), &mut self.scene);
        for id in affected {
            let _ = self.selection.deselect(&mut self.scene, id);
            self.selection.remove_id(id);
        }
        self.release_text_textures_except(textures);
        before.saturating_sub(self.scene.count()) as u32
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
        let selected_ids: Vec<ElementId> = self
            .selection
            .selected_ids()
            .into_iter()
            .filter(|id| !self.scene.is_effectively_locked(*id))
            .collect();
        if selected_ids.is_empty() {
            return;
        }

        // 创建并执行移动命令。锁定的选中项留在选择集里，但不参与平移。
        let cmd = MoveCommand::new(selected_ids, delta);
        self.command_history.execute(Box::new(cmd), &mut self.scene);
    }

    /// 用方向键平移当前选中元素，并记入一条撤销记录。
    ///
    /// 只移动顶层、可见、未锁定的选中元素。选中组时移动组根，子孙跟着走，只改平移。
    /// 步长是世界单位，与缩放无关：默认 1，`shift` 为 true 时为 10。
    /// 多选合成一条历史。空选、锁定或无法识别的方向不入栈。不经过拖拽吸附。
    ///
    /// 进行中的拖动、变换手柄和样式预览会先各自入栈，再做这次平移。
    ///
    /// # 参数
    /// - `direction`: `"left"`、`"right"`、`"up"`、`"down"`，或 `"ArrowLeft"` 等
    /// - `shift`: 为 true 时步长 ×10
    ///
    /// # 返回
    /// 是否写入了撤销栈
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.nudge_selection("right", event.shiftKey);
    /// ```
    #[wasm_bindgen]
    pub fn nudge_selection(&mut self, direction: &str, shift: bool) -> bool {
        let Some(direction) = NudgeDirection::parse(direction) else {
            return false;
        };
        if self.drag_state.is_dragging {
            self.finish_drag();
        }
        self.commit_handle_gesture();
        self.commit_style_preview();
        let selected = self.selection.selected_ids();
        apply_nudge(
            &mut self.command_history,
            &mut self.scene,
            &selected,
            direction,
            shift,
        )
    }

    /// 水平或垂直翻转当前选中元素，并记入一条撤销记录。
    ///
    /// 绕选区中心镜像。只有一个目标时，中心是该元素的世界包围盒中心。
    /// 选中组时翻转组根，子孙跟着走，不拆组。只改平移、旋转和缩放符号。
    /// 空选、锁定或无法识别的轴不入栈。
    ///
    /// 进行中的拖动、变换手柄和样式预览会先各自入栈，再做这次翻转。
    ///
    /// # 参数
    /// - `axis`: `"horizontal"` / `"h"` / `"x"`，或 `"vertical"` / `"v"` / `"y"`
    ///
    /// # 返回
    /// 是否写入了撤销栈
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.flip_selection("horizontal");
    /// ```
    #[wasm_bindgen]
    pub fn flip_selection(&mut self, axis: &str) -> bool {
        let Some(axis) = FlipAxis::parse(axis) else {
            return false;
        };
        self.finish_active_gesture();
        let selected = self.selection.selected_ids();
        apply_flip(&mut self.command_history, &mut self.scene, &selected, axis)
    }

    /// 把当前选中元素顺时针或逆时针转 90°，并记入一条撤销记录。
    ///
    /// 绕选区中心旋转。屏幕 Y 向下，顺时针是正角度。缩放（含符号）保持不变。
    /// 选中组时旋转组根。空选、锁定或无法识别的方向不入栈。
    ///
    /// # 参数
    /// - `dir`: `"cw"` / `"clockwise"`，或 `"ccw"` / `"counterclockwise"`
    ///
    /// # 返回
    /// 是否写入了撤销栈
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.rotate_selection_quarter("cw");
    /// ```
    #[wasm_bindgen]
    pub fn rotate_selection_quarter(&mut self, dir: &str) -> bool {
        let Some(turn) = QuarterTurn::parse(dir) else {
            return false;
        };
        self.finish_active_gesture();
        let selected = self.selection.selected_ids();
        apply_rotate_quarter(&mut self.command_history, &mut self.scene, &selected, turn)
    }

    /// 把进行中的拖动、手柄和样式预览收进撤销栈，避免和翻转/旋转叠在同一次指针手势上。
    fn finish_active_gesture(&mut self) {
        if self.drag_state.is_dragging {
            self.finish_drag();
        }
        self.commit_handle_gesture();
        self.commit_style_preview();
    }

    /// 按包围盒对齐当前选择，并记入一条撤销记录。
    ///
    /// 至少两个顶层可见、未锁定的选中元素才生效。选中组时用组的世界包围盒，只平移组根。
    /// 单选、空选或已经贴齐时不入栈。不经过拖拽吸附。
    ///
    /// # 参数
    /// - `mode`: `left` / `center` / `right` / `top` / `middle` / `bottom`
    ///
    /// # 返回
    /// 是否写入了撤销栈
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.align_selection("left");
    /// ```
    #[wasm_bindgen]
    pub fn align_selection(&mut self, mode: &str) -> bool {
        let Some(mode) = AlignMode::parse(mode) else {
            return false;
        };
        if self.drag_state.is_dragging {
            self.finish_drag();
        }
        self.commit_handle_gesture();
        self.commit_style_preview();
        let selected = self.selection.selected_ids();
        apply_align(&mut self.command_history, &mut self.scene, &selected, mode)
    }

    /// 按包围盒间隙均匀分布当前选择，并记入一条撤销记录。
    ///
    /// 至少三个顶层可见、未锁定的选中元素才生效。两端元素不动。
    /// 选中组时移动组根，不拆开子树。单选、空选或只有两个目标时不入栈。
    /// 不经过拖拽吸附。
    ///
    /// # 参数
    /// - `axis`: `horizontal` / `vertical`（也接受 `x` / `y`）
    ///
    /// # 返回
    /// 是否写入了撤销栈
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.distribute_selection("horizontal");
    /// ```
    #[wasm_bindgen]
    pub fn distribute_selection(&mut self, axis: &str) -> bool {
        let Some(axis) = DistributeAxis::parse(axis) else {
            return false;
        };
        if self.drag_state.is_dragging {
            self.finish_drag();
        }
        self.commit_handle_gesture();
        self.commit_style_preview();
        let selected = self.selection.selected_ids();
        apply_distribute(&mut self.command_history, &mut self.scene, &selected, axis)
    }

    // ========== 撤销/重做 API ==========

    /// 撤销上一个操作
    ///
    /// # 返回
    /// 是否成功撤销
    #[wasm_bindgen]
    pub fn undo(&mut self) -> bool {
        self.cancel_guide_gesture();
        self.commit_handle_gesture();
        self.commit_style_preview();
        let textures = self.snapshot_text_textures();
        let undone = self.command_history.undo(&mut self.scene);
        if undone {
            self.release_text_textures_except(textures);
            self.sync_image_textures();
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
        self.cancel_guide_gesture();
        self.commit_handle_gesture();
        self.commit_style_preview();
        let textures = self.snapshot_text_textures();
        let redone = self.command_history.redo(&mut self.scene);
        if redone {
            self.release_text_textures_except(textures);
            self.sync_image_textures();
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
    /// - `tool_name`: 工具名称 ("select", "rectangle", "circle", "polygon", "star", "text", "pan", "line", "pen", "eyedropper")
    ///
    /// 直线快捷键 `l`，画笔快捷键 `p`，文本快捷键 `t`，吸管快捷键 `i`。
    /// 吸管不带 Shift、Ctrl 或 Alt，避免和 V / L / P / R / O / T / H 以及翻转键抢键。
    /// 切换工具会丢掉尚未抬笔的预览。
    ///
    /// # 返回
    /// 是否成功设置
    #[wasm_bindgen]
    pub fn set_tool(&mut self, tool_name: String) -> bool {
        if let Ok(tool) = Tool::from_str(&tool_name) {
            if self.tool_manager.current() != tool {
                if self.tool_manager.current() == Tool::Eyedropper {
                    self.finish_eyedropper_gesture(false);
                }
                self.commit_handle_gesture();
                self.cancel_drawing();
                self.drag_state.guides.clear();
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

    /// 取走最近一次吸管点击的结果。没有新点击时返回 null。
    ///
    /// - `hit`：点到了可见叶子
    /// - `applied`：颜色已经写进当前选择，并占一条撤销记录
    ///
    /// 点在空白处时工具保持吸管。点到元素后，工具回到进入吸管之前的那个。
    #[wasm_bindgen]
    pub fn take_eyedropper_notice(&mut self) -> JsValue {
        let Some(notice) = self.eyedropper_notice.take() else {
            return JsValue::NULL;
        };
        let obj = Object::new();
        let _ = Reflect::set(&obj, &JsValue::from_str("hit"), &JsValue::from(notice.hit));
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("applied"),
            &JsValue::from(notice.applied),
        );
        obj.into()
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
    /// - snapEnabled: 拖拽吸附是否开启
    /// - snapThreshold: 吸附阈值（屏幕像素）
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
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("snapEnabled"),
            &JsValue::from(self.snap.enabled),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("snapThreshold"),
            &JsValue::from(self.snap.threshold_px),
        );
        obj.into()
    }

    /// 开启或关闭拖拽吸附。
    ///
    /// 关闭后元素跟着指针走，不再产生对齐参考线。
    ///
    /// # 参数
    /// - `enabled`: `true` 开启吸附
    #[wasm_bindgen]
    pub fn set_snap_enabled(&mut self, enabled: bool) {
        self.snap.enabled = enabled;
        if !enabled {
            self.drag_state.guides.clear();
        }
    }

    /// 拖拽吸附是否开启。
    #[wasm_bindgen]
    pub fn is_snap_enabled(&self) -> bool {
        self.snap.enabled
    }

    /// 设置吸附阈值（屏幕像素）。
    ///
    /// 非有限值会被忽略。有效值限制在 0 到 64 之间。
    /// 世界距离会除以当前缩放，因此任意缩放下抓取范围保持同样的屏幕像素。
    ///
    /// # 参数
    /// - `threshold_px`: 屏幕像素阈值
    #[wasm_bindgen]
    pub fn set_snap_threshold(&mut self, threshold_px: f32) {
        self.snap.set_threshold_px(threshold_px);
    }

    /// 当前吸附阈值（屏幕像素）。
    #[wasm_bindgen]
    pub fn get_snap_threshold(&self) -> f32 {
        self.snap.threshold_px
    }

    /// 当前生效的对齐参考线。
    ///
    /// 只在吸附开启且正在拖拽、并且选区进入阈值时非空。
    /// 每条记录包含 `axis`（`"horizontal"` 或 `"vertical"`）和 `position`（世界坐标）。
    ///
    /// # 返回
    /// 参考线数组。没有参考线时为空数组。
    #[wasm_bindgen]
    pub fn get_alignment_guides(&self) -> Array {
        let array = Array::new();
        for guide in &self.drag_state.guides {
            let obj = Object::new();
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("axis"),
                &JsValue::from_str(guide.axis.name()),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("position"),
                &JsValue::from(guide.position),
            );
            array.push(&obj);
        }
        array
    }

    /// 显示或隐藏标尺和手动参考线。
    ///
    /// 隐藏后参考线仍留在场景 JSON 里，但不再命中、不再吸附。
    /// 正在拖的参考线会取消，场景回到按下前的参考线。
    ///
    /// # 参数
    /// - `visible`: `true` 显示
    #[wasm_bindgen]
    pub fn set_rulers_visible(&mut self, visible: bool) {
        if !visible {
            self.cancel_guide_gesture();
            self.selected_guide = None;
        }
        self.rulers_visible = visible;
    }

    /// 标尺和手动参考线是否显示。
    #[wasm_bindgen]
    pub fn are_rulers_visible(&self) -> bool {
        self.rulers_visible
    }

    /// 显示或隐藏背景网格。
    ///
    /// 默认可见，和第一次打开时的表现一致。下一帧 `render` 生效，不进撤销栈。
    ///
    /// # 参数
    /// - `visible`: `true` 显示网格
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.set_grid_visible(false);
    /// canvas.grid_visible();
    /// ```
    #[wasm_bindgen]
    pub fn set_grid_visible(&mut self, visible: bool) {
        self.grid_visible = visible;
    }

    /// 背景网格是否可见。
    ///
    /// # 返回
    /// 可见时为 `true`
    #[wasm_bindgen]
    pub fn grid_visible(&self) -> bool {
        self.grid_visible
    }

    /// 标尺厚度（屏幕像素）。
    #[wasm_bindgen]
    pub fn get_ruler_thickness(&self) -> f32 {
        RULER_THICKNESS_PX
    }

    /// 主刻度的世界步长。缩放越大，步长越小。
    #[wasm_bindgen]
    pub fn get_ruler_major_step(&self) -> f32 {
        infinitecanvas_engine::ruler_major_step(self.viewport.zoom())
    }

    /// 一条标尺上的刻度。
    ///
    /// `axis` 为 `"horizontal"`（顶边，世界 x）或 `"vertical"`（左边，世界 y）。
    /// 每条记录含 `world`、`screen` 和 `major`。
    ///
    /// # 参数
    /// - `axis`: `"horizontal"` 或 `"vertical"`
    #[wasm_bindgen]
    pub fn get_ruler_ticks(&self, axis: &str) -> Array {
        let orientation = match axis {
            "vertical" => RulerOrientation::Vertical,
            _ => RulerOrientation::Horizontal,
        };
        let array = Array::new();
        if !self.rulers_visible {
            return array;
        }
        for tick in ruler_ticks(&self.viewport, orientation) {
            let obj = Object::new();
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("world"),
                &JsValue::from(tick.world),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("screen"),
                &JsValue::from(tick.screen),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("major"),
                &JsValue::from(tick.major),
            );
            array.push(&obj);
        }
        array
    }

    /// 场景里的手动参考线。
    ///
    /// 每条记录含 `id`、`axis`（`"horizontal"` / `"vertical"`）、`position`（世界坐标）
    /// 和 `selected`。隐藏标尺时仍返回数据，绘制由调用方决定。
    #[wasm_bindgen]
    pub fn get_manual_guides(&self) -> Array {
        let array = Array::new();
        for guide in self.scene.guides().iter() {
            let obj = Object::new();
            let _ = Reflect::set(&obj, &JsValue::from_str("id"), &JsValue::from(guide.id));
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("axis"),
                &JsValue::from_str(guide.axis.name()),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("position"),
                &JsValue::from(guide.position),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("selected"),
                &JsValue::from(self.selected_guide == Some(guide.id)),
            );
            array.push(&obj);
        }
        array
    }

    /// 删除当前选中的参考线，并记入撤销栈。
    ///
    /// # 返回
    /// 确实删掉了一条参考线时为 `true`。
    #[wasm_bindgen]
    pub fn delete_selected_guide(&mut self) -> bool {
        let Some(id) = self.selected_guide else {
            return false;
        };
        self.delete_guide_id(id)
    }

    /// 双击命中参考线时删除它。
    ///
    /// # 参数
    /// - `x`: 画布像素 x
    /// - `y`: 画布像素 y
    ///
    /// # 返回
    /// 命中并删除时为 `true`。标尺隐藏时为 `false`。
    #[wasm_bindgen]
    pub fn delete_guide_at(&mut self, x: f32, y: f32) -> bool {
        if !self.rulers_visible {
            return false;
        }
        let world = self.viewport.screen_to_world(Vec2::new(x, y));
        let threshold = GUIDE_HIT_PX / self.viewport.zoom().max(0.05);
        let Some(id) = self.scene.guides().hit_test(world, threshold) else {
            return false;
        };
        self.delete_guide_id(id)
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
            let parent = match props.parent {
                Some(parent_id) => JsValue::from(parent_id),
                None => JsValue::NULL,
            };
            let _ = Reflect::set(&obj, &JsValue::from_str("parentId"), &parent);
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("locked"),
                &JsValue::from(props.locked),
            );
            if let Some(child_ids) = element.container_children() {
                let children = Array::new();
                for child_id in child_ids {
                    children.push(&JsValue::from(*child_id));
                }
                let _ = Reflect::set(&obj, &JsValue::from_str("childIds"), &children);
            }
            if let AnyElement::Frame(frame) = element {
                let _ = Reflect::set(
                    &obj,
                    &JsValue::from_str("name"),
                    &JsValue::from_str(&frame.name),
                );
            }
            if let AnyElement::Polygon(polygon) = element {
                let _ = Reflect::set(
                    &obj,
                    &JsValue::from_str("kind"),
                    &JsValue::from_str(polygon.kind().as_str()),
                );
                let _ = Reflect::set(
                    &obj,
                    &JsValue::from_str("sides"),
                    &JsValue::from(polygon.sides()),
                );
                let _ = Reflect::set(
                    &obj,
                    &JsValue::from_str("innerRadius"),
                    &JsValue::from(polygon.inner_radius()),
                );
                let _ = Reflect::set(
                    &obj,
                    &JsValue::from_str("radius"),
                    &JsValue::from(polygon.radius()),
                );
            }
            if let AnyElement::Text(text) = element {
                let _ = Reflect::set(
                    &obj,
                    &JsValue::from_str("content"),
                    &JsValue::from_str(&text.content),
                );
                let _ = Reflect::set(
                    &obj,
                    &JsValue::from_str("fontSize"),
                    &JsValue::from(text.font_size),
                );
                let _ = Reflect::set(
                    &obj,
                    &JsValue::from_str("color"),
                    &JsValue::from_str(&text.color.to_hex()),
                );
                let _ = Reflect::set(
                    &obj,
                    &JsValue::from_str("align"),
                    &JsValue::from_str(text.align.as_str()),
                );
                let _ = Reflect::set(
                    &obj,
                    &JsValue::from_str("fontFamily"),
                    &JsValue::from_str(&text.font_family),
                );
            }
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

    /// 添加画板，并记入撤销栈。
    ///
    /// 中心是 `(x, y)`。空名称写成 `"画板"`。宽或高不是正数时返回 0，不入栈。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const id = canvas.add_frame(40, 30, 80, 60, "封面");
    /// canvas.rename_frame(id, "内页");
    /// ```
    #[wasm_bindgen]
    pub fn add_frame(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        name: Option<String>,
    ) -> u64 {
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            return 0;
        }
        let id = self.scene.next_id();
        let label = name
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "画板".to_string());
        let frame = Frame::new(id, Vec2::new(x, y), Vec2::new(width, height)).with_name(label);
        self.command_history.execute(
            Box::new(AddElementCommand::new(AnyElement::Frame(frame))),
            &mut self.scene,
        );
        id
    }

    /// 重命名画板。不是画板，或名称没变时返回 false，不入栈。
    #[wasm_bindgen]
    pub fn rename_frame(&mut self, id: u64, name: &str) -> bool {
        apply_rename_frame(&mut self.command_history, &mut self.scene, id, name)
    }

    /// 画板名称。不是画板时返回 `null`。
    #[wasm_bindgen]
    pub fn frame_name(&self, id: u64) -> Option<String> {
        match self.scene.get(id) {
            Some(AnyElement::Frame(frame)) => Some(frame.name.clone()),
            _ => None,
        }
    }

    /// 把元素移进画板。空列表、目标不是画板、或没有元素移动时返回 false，不入栈。
    #[wasm_bindgen]
    pub fn reparent_into_frame(&mut self, ids: &Array, frame_id: u64) -> bool {
        let ids = element_ids_from_js(ids);
        apply_reparent_into_frame(&mut self.command_history, &mut self.scene, &ids, frame_id)
    }

    /// 把当前选择移进画板。空选返回 false，不入栈。
    #[wasm_bindgen]
    pub fn reparent_selection_into_frame(&mut self, frame_id: u64) -> bool {
        let ids = self.selection.selected_ids();
        apply_reparent_into_frame(&mut self.command_history, &mut self.scene, &ids, frame_id)
    }

    /// 添加正多边形，并记入撤销栈。
    ///
    /// # 参数
    /// - `x, y`: 中心点
    /// - `radius`: 外接圆半径
    /// - `sides`: 边数，钳到 3–64。缺省调用方应传 6
    /// - `fill_color` / `stroke_color` / `stroke_width`: 与 [`Self::add_rectangle`] 相同
    ///
    /// # 返回
    /// 元素 ID
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const id = canvas.add_polygon(0, 0, 40, 6, "#f59e0b", null, 1);
    /// canvas.undo();
    /// ```
    #[wasm_bindgen]
    pub fn add_polygon(
        &mut self,
        x: f32,
        y: f32,
        radius: f32,
        sides: u32,
        fill_color: Option<String>,
        stroke_color: Option<String>,
        stroke_width: f32,
    ) -> u64 {
        let id = self.scene.next_id();
        let mut style = ElementStyle::new();
        style.fill = fill_color.and_then(|hex| Color::from_hex(&hex).ok());
        style.stroke = stroke_color.and_then(|hex| Color::from_hex(&hex).ok());
        style.stroke_width = stroke_width;
        let polygon = Polygon::regular(id, Vec2::new(x, y), radius, sides).with_style(style);
        self.command_history.execute(
            Box::new(AddElementCommand::new(AnyElement::Polygon(polygon))),
            &mut self.scene,
        );
        log::info!("Added polygon {} at ({}, {}), sides {}", id, x, y, sides);
        id
    }

    /// 添加星形，并记入撤销栈。
    ///
    /// # 参数
    /// - `points`: 角数，钳到 3–64。五角星传 5
    /// - `inner_radius`: 内点半径相对外接圆的比例，钳到 0.05–0.95
    ///
    /// # 返回
    /// 元素 ID
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const id = canvas.add_star(0, 0, 40, 5, 0.5, "#a855f7", null, 1);
    /// ```
    #[wasm_bindgen]
    pub fn add_star(
        &mut self,
        x: f32,
        y: f32,
        radius: f32,
        points: u32,
        inner_radius: f32,
        fill_color: Option<String>,
        stroke_color: Option<String>,
        stroke_width: f32,
    ) -> u64 {
        let id = self.scene.next_id();
        let mut style = ElementStyle::new();
        style.fill = fill_color.and_then(|hex| Color::from_hex(&hex).ok());
        style.stroke = stroke_color.and_then(|hex| Color::from_hex(&hex).ok());
        style.stroke_width = stroke_width;
        let mut star = Polygon::star(id, Vec2::new(x, y), radius)
            .with_style(style)
            .with_inner_radius(inner_radius);
        star.set_sides(points);
        self.command_history.execute(
            Box::new(AddElementCommand::new(AnyElement::Polygon(star))),
            &mut self.scene,
        );
        log::info!("Added star {} at ({}, {}), points {}", id, x, y, points);
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
        let textures = self.snapshot_text_textures();
        let subtree = self.scene.subtree_ids(id);
        for element_id in &subtree {
            let _ = self.selection.deselect(&mut self.scene, *element_id);
            self.selection.remove_id(*element_id);
        }
        let removed = self.scene.remove(id).is_some();
        if removed {
            let _ = self.scene.take_detached_assets();
            self.release_text_textures_except(textures);
            self.sync_image_textures();
            log::info!("Removed element {} (subtree {})", id, subtree.len());
        }
        removed
    }

    /// 清空场景
    #[wasm_bindgen]
    pub fn clear_scene(&mut self) {
        self.cancel_guide_gesture();
        self.selected_guide = None;
        self.cancel_handle_gesture();
        self.cancel_style_preview();
        let textures = self.snapshot_text_textures();
        self.selection.clear(&mut self.scene);
        self.selection_box.cancel();
        self.drag_state.guides.clear();
        self.pending_text_edit = None;
        self.scene.clear();
        self.release_text_textures_except(textures);
        self.sync_image_textures();
        log::info!("Scene cleared");
    }

    /// 获取元素数量
    #[wasm_bindgen]
    pub fn get_element_count(&self) -> usize {
        self.scene.count()
    }

    /// 导出场景为版本化 JSON
    ///
    /// 文档含 `version`、`next_id` 和 `elements`。矩形、圆、图片、直线（含箭头）、
    /// 自由路径、文本和分组都会写入。组的 `children` 以及元素的 `parent` / `locked`
    /// 在 `data` 里。变换的 `rotation` 与 `scale` 在 `props.transform` 里。
    /// `dirty` 和文本光栅字段是运行时标记，不会出现在 JSON 里。
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
        self.cancel_guide_gesture();
        self.selected_guide = None;
        self.cancel_handle_gesture();
        self.cancel_style_preview();
        let textures = self.snapshot_text_textures();
        self.pending_text_edit = None;
        self.scene = scene;
        self.selection.sync_from_scene(&mut self.scene);
        self.selection_box.cancel();
        self.command_history.clear();
        self.drag_state = DragState::default();
        self.release_text_textures_except(textures);
        self.sync_image_textures();
        if let Some(renderer) = &mut self.webgpu_renderer {
            renderer.invalidate_geometry();
        }
        log::info!("Imported scene with {} elements", self.scene.count());
        Ok(())
    }

    /// 把当前选中元素写成剪贴板 JSON。
    ///
    /// 格式与 [`export_json`](Self::export_json) 相同，只含选中根和它们的子孙。
    /// 组会带上整棵子树，父子 ID 仍指向这份文档里的元素。图片会带上它们的 `assets[]`。
    /// 没有选中元素时返回空字符串。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const json = canvas.copy_selection_json();
    /// await navigator.clipboard.writeText(json);
    /// ```
    #[wasm_bindgen]
    pub fn copy_selection_json(&self) -> Result<String, JsValue> {
        let ids = self.selection.selected_ids();
        if ids.is_empty() {
            return Ok(String::new());
        }
        self.scene
            .copy_elements_json(&ids)
            .map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// 原地克隆当前选中元素，并让新元素成为选中。
    ///
    /// 走 [`DuplicateElementsCommand`]，Ctrl+Z 会删掉副本，Ctrl+Y 按原 ID 恢复。
    /// 新元素标脏，下一帧顶点缓存会写上它们。组会克隆整棵子树。
    ///
    /// # 参数
    /// - `dx`, `dy`: 世界位移。Demo 的 Ctrl+D 使用 `(10, 10)`
    ///
    /// # 返回
    /// 新根元素的 ID。没有选中元素时为空数组。
    #[wasm_bindgen]
    pub fn duplicate_selection(&mut self, dx: f32, dy: f32) -> Array {
        let ids = self.selection.selected_ids();
        if ids.is_empty() {
            return Array::new();
        }
        self.selection.clear(&mut self.scene);
        let offset = Vec2::new(dx, dy);
        let roots = self.scene.duplicate(&ids, offset);
        if roots.is_empty() {
            return Array::new();
        }
        for id in &roots {
            self.selection.select_element(&mut self.scene, *id, true);
        }
        self.command_history
            .push_executed(Box::new(DuplicateElementsCommand::applied(
                ids,
                offset,
                roots.clone(),
                &self.scene,
            )));
        log::info!("Duplicated selection into {} roots", roots.len());
        self.sync_image_textures();
        selection_ids_to_js_array(roots)
    }

    /// 把剪贴板 JSON 粘贴进场景，并选中新根。
    ///
    /// `focus_x` / `focus_y` 都给出时，叶子包围盒中心对准该世界坐标，再加 `(dx, dy)`。
    /// 缺一个焦点时只应用位移（鼠标不在画布上时 Demo 传 `(10, 10)` 的倍数）。
    /// 非法 JSON 不修改场景。
    ///
    /// # 参数
    /// - `json`: [`copy_selection_json`](Self::copy_selection_json) 的文本
    /// - `dx`, `dy`: 额外世界位移，连续粘贴用来错开
    /// - `focus_x`, `focus_y`: 可选的粘贴中心
    ///
    /// # 返回
    /// 新根元素 ID
    ///
    /// # 错误
    /// JSON 无法解析、版本不受支持或元素种类未知
    #[wasm_bindgen]
    pub fn paste_elements(
        &mut self,
        json: &str,
        dx: f32,
        dy: f32,
        focus_x: Option<f32>,
        focus_y: Option<f32>,
    ) -> Result<Array, JsValue> {
        if json.trim().is_empty() {
            return Ok(Array::new());
        }
        let focus = match (focus_x, focus_y) {
            (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Some(Vec2::new(x, y)),
            _ => None,
        };
        // 先解析，失败时不改选中、不入栈。
        let incoming = Scene::from_json(json).map_err(|err| JsValue::from_str(&err.to_string()))?;
        if incoming.is_empty() {
            return Ok(Array::new());
        }
        self.selection.clear(&mut self.scene);
        let offset = Vec2::new(dx, dy);
        let roots = self
            .scene
            .paste_json(json, offset, focus)
            .map_err(|err| JsValue::from_str(&err.to_string()))?;
        if roots.is_empty() {
            return Ok(Array::new());
        }
        for id in &roots {
            self.selection.select_element(&mut self.scene, *id, true);
        }
        self.command_history
            .push_executed(Box::new(PasteElementsCommand::applied(
                json.to_string(),
                offset,
                focus,
                roots.clone(),
                &self.scene,
            )));
        log::info!("Pasted {} roots", roots.len());
        self.sync_image_textures();
        Ok(selection_ids_to_js_array(roots))
    }

    /// 把当前视口或选中区域导出为 PNG
    ///
    /// # 参数
    /// - `selection_only`: `true` 时导出选中元素的包围盒；`false` 时导出当前视口
    ///
    /// # 返回
    /// RGBA PNG 字节。直线、箭头和自由路径按线宽栅格化。文本用 font8x8 点阵，
    /// 没有点阵的字符画方框。资源表里有像素的图片按显示矩形采样；只有 GPU 纹理
    /// id 的图片仍是纯色占位。分组本身不绘制；隐藏的组会连同子孙一起跳过。
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

    /// 把场景导出为 SVG 文本。
    ///
    /// 矩形、圆、直线（含箭头）、自由路径、文本，以及带像素的图片按绘制顺序写入。
    /// 变换使用与画布相同的 `matrix`（缩放、旋转、平移）。`viewBox` 默认是全部可见元素的包围盒。
    ///
    /// # 参数
    /// - `selected_only`: `true` 时只导出当前选中元素，以及选中组下的可见子孙。包围盒也只包住这些元素
    ///
    /// # 返回
    /// SVG 文档。有资源字节的图片写成 `<image>`，`href` 是 data URL。
    /// 没有字节的纹理会被跳过并在文档中注明。文本写成 `<text>`，锚点与对齐方式一致。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const svg = canvas.export_svg(false);
    /// ```
    #[wasm_bindgen]
    pub fn export_svg(&self, selected_only: bool) -> String {
        export_svg(&self.scene, selected_only)
    }

    /// 把一张画板导出为 PNG。区域是画板的世界包围盒，不含额外留白。
    ///
    /// 只绘制这张画板和它的子孙。空画板仍按矩形尺寸导出。
    #[wasm_bindgen]
    pub fn export_frame_png(&self, frame_id: u64) -> Result<Vec<u8>, JsValue> {
        export_frame_png(&self.scene, frame_id, self.viewport.zoom())
            .map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// 把一张画板导出为 SVG。`viewBox` 是画板包围盒，超出部分隐藏。
    #[wasm_bindgen]
    pub fn export_frame_svg(&self, frame_id: u64) -> Result<String, JsValue> {
        export_frame_svg(&self.scene, frame_id).map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// 把 SVG 文本解析成场景元素，并记一条撤销记录。
    ///
    /// 支持 `rect`、`circle`、`ellipse`、`line`、`polyline`、`polygon`、`path`、
    /// `text`，以及 `translate` / `scale` / `rotate` / `matrix` 的嵌套 `g`。
    /// `image` 只接受 PNG/JPEG 的 data URL。一次导入是一条历史，Ctrl+Z 删掉整批。
    ///
    /// # 参数
    /// - `svg`: SVG 文档
    /// - `x`, `y`: viewBox 原点（没有 viewBox 时是用户坐标原点）的世界坐标
    ///
    /// # 返回
    /// 新元素 ID，文档顺序。这些元素成为当前选择。
    ///
    /// # 错误
    /// 空文本、非法 XML、根节点不是 `svg`，或没有可识别图元。失败时场景不变。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const ids = canvas.import_svg('<svg><rect width="10" height="10"/></svg>', worldX, worldY);
    /// ```
    #[wasm_bindgen]
    pub fn import_svg(&mut self, svg: &str, x: f32, y: f32) -> Result<Array, JsValue> {
        if !x.is_finite() || !y.is_finite() {
            return Err(JsValue::from_str("import anchor must be finite"));
        }
        let ids = import_svg_scene(
            &mut self.scene,
            &mut self.command_history,
            svg,
            Vec2::new(x, y),
        )
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
        self.selection.clear(&mut self.scene);
        for id in &ids {
            self.selection.select_element(&mut self.scene, *id, true);
        }
        self.sync_image_textures();
        log::info!("Imported SVG into {} elements", ids.len());
        Ok(selection_ids_to_js_array(ids))
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

    /// 把至少两个元素组成一个 Group。
    ///
    /// # 参数
    /// - `ids`: 元素 ID。可以是 `BigInt`（`add_*` 的返回值）或数字。
    ///
    /// # 返回
    /// 新组的 ID。失败时返回 0（ID 从 1 开始）。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const groupId = canvas.group_elements([rectA, rectB]);
    /// canvas.translate_element(groupId, 20, 0);
    /// canvas.ungroup(groupId);
    /// ```
    #[wasm_bindgen]
    pub fn group_elements(&mut self, ids: &Array) -> u64 {
        let ids = element_ids_from_js(ids);
        match self.scene.group(&ids) {
            Ok(group_id) => {
                log::info!("Grouped {:?} into {}", ids, group_id);
                group_id
            }
            Err(error) => {
                log::warn!("group_elements failed: {error}");
                0
            }
        }
    }

    /// 解散组。子元素留在场景里。
    #[wasm_bindgen]
    pub fn ungroup(&mut self, group_id: u64) -> bool {
        match self.scene.ungroup(group_id) {
            Ok(children) => {
                log::info!("Ungrouped {group_id}, children {children:?}");
                true
            }
            Err(error) => {
                log::warn!("ungroup failed: {error}");
                false
            }
        }
    }

    /// 把元素放进组，世界坐标不变。
    #[wasm_bindgen]
    pub fn enter_group(&mut self, element_id: u64, group_id: u64) -> bool {
        match self.scene.enter_group(element_id, group_id) {
            Ok(()) => true,
            Err(error) => {
                log::warn!("enter_group failed: {error}");
                false
            }
        }
    }

    /// 把元素移到大纲中的新位置，并记入撤销栈。
    ///
    /// 同一父节点下改变绘制顺序，或移入 / 移出组。世界坐标不变。
    /// `parent_id` 为 `0` 时挂到根上（元素 ID 从 1 开始）。`index` 是摘下该元素之后
    /// 在目标兄弟列表中的下标，`0` 为最底层。
    ///
    /// 锁定、成环或目标不是组时返回 `false`，不入栈。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.reorder_element(id, groupId, 0);
    /// canvas.undo();
    /// ```
    #[wasm_bindgen]
    pub fn reorder_element(&mut self, id: u64, parent_id: u64, index: u32) -> bool {
        let parent = if parent_id == 0 {
            None
        } else {
            Some(parent_id)
        };
        let index = index as usize;
        let before: OutlineSnapshot = self.scene.outline_snapshot();
        match self.scene.move_in_outline(id, parent, index) {
            Ok(Some(refresh)) => {
                self.command_history
                    .push_executed(Box::new(ReorderOutlineCommand::applied(
                        id, parent, index, before, refresh,
                    )));
                true
            }
            Ok(None) => false,
            Err(error) => {
                log::warn!("reorder_element failed: {error}");
                false
            }
        }
    }

    /// 按当前选中重排同一层的兄弟，并记入一条撤销记录。
    ///
    /// `action` 为 `front`（置顶）、`back`（置底）、`forward`（前移一层）或 `backward`（后移一层）。
    /// 只在同一个父节点下移动。选中组时移动组根，不拆开子树。
    /// 空选、跨父节点、全部锁定（含锁定的祖先）或顺序不变时返回 `false`，不入栈。
    /// 同一层里锁定的选中项会跳过。隐藏不阻止重排。
    ///
    /// 进行中的拖动、变换手柄和样式预览会先各自入栈，再做这次重排。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.reorder_selection("front");
    /// canvas.undo();
    /// ```
    #[wasm_bindgen]
    pub fn reorder_selection(&mut self, action: &str) -> bool {
        let Some(action) = StackOrderAction::parse(action) else {
            return false;
        };
        if self.drag_state.is_dragging {
            self.finish_drag();
        }
        self.commit_handle_gesture();
        self.commit_style_preview();
        let selected = self.selection.selected_ids();
        apply_stack_reorder(
            &mut self.command_history,
            &mut self.scene,
            &selected,
            action,
        )
    }

    /// 场景结构版本。增删元素或大纲重排后变化，图层面板用它决定是否重读场景树。
    #[wasm_bindgen]
    pub fn get_structure_epoch(&self) -> u64 {
        self.scene.structure_epoch()
    }

    /// 把元素移出当前组，挂回组的父节点或根。
    #[wasm_bindgen]
    pub fn leave_group(&mut self, element_id: u64) -> bool {
        match self.scene.leave_group(element_id) {
            Ok(()) => true,
            Err(error) => {
                log::warn!("leave_group failed: {error}");
                false
            }
        }
    }

    /// 平移一个元素。目标是组时，子孙的世界坐标一起移动。
    #[wasm_bindgen]
    pub fn translate_element(&mut self, id: u64, dx: f32, dy: f32) -> bool {
        self.scene.translate_element(id, Vec2::new(dx, dy)).is_ok()
    }

    /// 设置元素可见性，并记入一条撤销记录。
    ///
    /// 与图层面板的眼睛按钮、[`Self::toggle_selection_visible`] 共用 [`infinitecanvas_engine::SetVisibilityCommand`]。
    /// 隐藏组不改子孙自己的标志。元素不存在时返回 `false`。已经是目标值时不入栈，仍返回 `true`。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.set_element_visible(id, false);
    /// canvas.undo();
    /// ```
    #[wasm_bindgen]
    pub fn set_element_visible(&mut self, id: u64, visible: bool) -> bool {
        if !self.scene.contains(id) {
            return false;
        }
        self.finish_active_gesture();
        let _ = apply_set_visibility(&mut self.command_history, &mut self.scene, &[id], visible);
        true
    }

    /// 设置元素锁定，并记入一条撤销记录。
    ///
    /// 与图层面板的锁按钮、[`Self::toggle_selection_locked`] 共用 [`infinitecanvas_engine::SetLockedCommand`]。
    /// 锁定后组和平移、变换会拒绝修改它以及它的子孙。元素仍留在选择集里，图层面板可以继续选中它；
    /// 画布命中、框选和全选会跳过。元素不存在时返回 `false`。已经是目标值时不入栈，仍返回 `true`。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.set_element_locked(id, true);
    /// canvas.undo();
    /// ```
    #[wasm_bindgen]
    pub fn set_element_locked(&mut self, id: u64, locked: bool) -> bool {
        if !self.scene.contains(id) {
            return false;
        }
        self.finish_active_gesture();
        let _ = apply_set_locked(&mut self.command_history, &mut self.scene, &[id], locked);
        true
    }

    /// 切换当前选中元素的可见性，并记入一条撤销记录。
    ///
    /// 全部已经隐藏时改为显示，否则隐藏仍可见的选中项。选中组时只改组自己的标志，
    /// 子孙的命中和框选按有效可见性跳过。空选或不存在的 id 不入栈。
    ///
    /// # 返回
    /// 是否写入了撤销栈
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.toggle_selection_visible();
    /// canvas.undo();
    /// ```
    #[wasm_bindgen]
    pub fn toggle_selection_visible(&mut self) -> bool {
        self.finish_active_gesture();
        let selected = self.selection.selected_ids();
        apply_toggle_visibility(&mut self.command_history, &mut self.scene, &selected)
    }

    /// 切换当前选中元素的锁定，并记入一条撤销记录。
    ///
    /// 可改的选中项里只要有一个未锁定，就把它们锁定；全部已锁定则解锁。
    /// 祖先已锁定、自己未锁定的元素跳过。锁定后仍保持选中，但不能平移或变换。
    /// 空选时不入栈。
    ///
    /// # 返回
    /// 是否写入了撤销栈
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.toggle_selection_locked();
    /// canvas.undo();
    /// ```
    #[wasm_bindgen]
    pub fn toggle_selection_locked(&mut self) -> bool {
        self.finish_active_gesture();
        let selected = self.selection.selected_ids();
        apply_toggle_locked(&mut self.command_history, &mut self.scene, &selected)
    }

    /// 可见性或锁定的版本。图层面板用它重读眼睛和锁，不代表场景结构变了。
    #[wasm_bindgen]
    pub fn get_flags_epoch(&self) -> u64 {
        self.scene.flags_epoch()
    }

    /// 场景树。`roots` 里每个节点含 `id`、`elementType`、`parentId`、`visible`、`locked`、`zIndex`、`children`。
    #[wasm_bindgen]
    pub fn get_scene_tree(&self) -> JsValue {
        let tree = self.scene.scene_tree();
        let roots = Array::new();
        for root in &tree.roots {
            roots.push(&scene_tree_node_to_js(root));
        }
        let obj = Object::new();
        let _ = Reflect::set(&obj, &JsValue::from_str("roots"), &roots);
        obj.into()
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

    /// 从 PNG 或 JPEG 字节创建图片元素。
    ///
    /// 像素进入场景资源表：同一份字节多次调用只存一份。边长超过 4096 时先降采样。
    /// 显示尺寸默认等于像素尺寸；长边超过 512 世界单位时等比缩小，避免一张照片铺满视口。
    /// 元素进入撤销栈，并成为当前选择。
    ///
    /// # 参数
    /// - `bytes`: 文件字节
    /// - `mime`: `image/png` 或 `image/jpeg`。空字符串时按文件头识别
    /// - `x`, `y`: 左上角（世界坐标）
    ///
    /// # 返回
    /// 元素 ID
    ///
    /// # 错误
    /// 格式不支持或解码失败
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const bytes = new Uint8Array(await file.arrayBuffer());
    /// const id = canvas.add_image_bytes(bytes, file.type, worldX, worldY);
    /// ```
    #[wasm_bindgen]
    pub fn add_image_bytes(
        &mut self,
        bytes: &[u8],
        mime: &str,
        x: f32,
        y: f32,
    ) -> Result<u64, JsValue> {
        let asset_id = self
            .scene
            .insert_image_bytes(bytes, mime)
            .map_err(|err| JsValue::from_str(&err.to_string()))?;
        let (width, height) = {
            let asset = self.scene.image_asset(&asset_id).ok_or_else(|| {
                JsValue::from_str("image asset disappeared before the element was created")
            })?;
            (asset.width, asset.height)
        };
        let id = self.scene.next_id();
        let mut image = Image::from_asset(id, Vec2::new(x, y), &asset_id, width, height);
        let longest = image.size.x.max(image.size.y);
        if longest > MAX_PASTED_DISPLAY_EDGE {
            let scale = MAX_PASTED_DISPLAY_EDGE / longest;
            image.size.x *= scale;
            image.size.y *= scale;
        }
        self.command_history.execute(
            Box::new(AddElementCommand::new(AnyElement::Image(image))),
            &mut self.scene,
        );
        self.scene.mark_dirty(id);
        let _ = self.selection.select_element(&mut self.scene, id, false);
        self.sync_image_textures();
        log::info!(
            "Added image {} from {} bytes at ({}, {}) asset {}",
            id,
            bytes.len(),
            x,
            y,
            asset_id
        );
        Ok(id)
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
        let textures = self.snapshot_text_textures();
        self.selection.clear(&mut self.scene);
        self.selection_box.cancel();
        self.drag_state.guides.clear();
        self.pending_text_edit = None;
        self.scene.clear();
        self.release_text_textures_except(textures);
        self.sync_image_textures();
        log::info!("Cleared scene ({} elements removed)", count);
    }

    /// 添加文本元素。
    ///
    /// 位置是锚点：左对齐为左上角，居中为上边中点，右对齐为右上角。
    /// 这个接口和 `add_rectangle` 一样直接写入场景，不进入撤销栈。
    /// T 工具创建的文本会进入撤销栈。
    ///
    /// # 参数
    /// - `x`, `y`: 锚点（世界坐标）
    /// - `content`: 文本，支持换行
    /// - `font_size`: 世界单位字号，`<= 0` 时用 32
    /// - `color`: `#RRGGBB` 或 `#RRGGBBAA`，空则用浅色
    /// - `align`: `"left"` / `"center"` / `"right"`
    ///
    /// # 返回
    /// 元素 ID
    #[wasm_bindgen]
    pub fn add_text(
        &mut self,
        x: f32,
        y: f32,
        content: String,
        font_size: f32,
        color: Option<String>,
        align: Option<String>,
    ) -> u64 {
        let id = self.scene.next_id();
        let size = if font_size > 0.0 {
            font_size
        } else {
            DEFAULT_FONT_SIZE
        };
        let mut text = Text::new(id, Vec2::new(x, y), content)
            .with_font_size(size)
            .with_color(DEFAULT_TEXT_COLOR);
        if let Some(parsed) = color.and_then(|value| Color::from_hex(&value).ok()) {
            text = text.with_color(parsed);
        }
        if let Some(parsed) = align.and_then(|value| TextAlign::parse(&value)) {
            text = text.with_align(parsed);
        }
        self.scene.add_text(text);
        self.scene.mark_dirty(id);
        log::info!("Added text {} at ({}, {})", id, x, y);
        id
    }

    /// 取出当前缩放档下还没光栅化的文本。
    ///
    /// 每个元素在同一内容版本和 LOD 上只返回一次。前端用 Canvas2D 画完后调用
    /// [`Self::commit_text_raster`]。失败时调用 [`Self::abandon_text_raster`] 以便重试。
    ///
    /// # 返回
    /// 对象数组：`id`, `revision`, `lod`, `rasterScale`, `content`, `fontSizePx`,
    /// `color`, `align`, `fontFamily`, `paddingWorld`, `textureId`
    #[wasm_bindgen]
    pub fn take_text_raster_jobs(&mut self) -> Array {
        let lod = lod_bucket(self.viewport.zoom());
        let ids: Vec<ElementId> = self.scene.iter().map(|element| element.id()).collect();
        let jobs = Array::new();
        for id in ids {
            let Some(AnyElement::Text(text)) = self.scene.get_mut(id) else {
                continue;
            };
            if !text.needs_raster(lod) {
                continue;
            }
            let params = raster_params(text.font_size, lod, &text.content);
            let texture_id = raster_texture_id(id, text.revision, lod);
            let content = text.content.clone();
            let color = text.color.to_hex();
            let align = text.align.as_str();
            let font_family = text.font_family.clone();
            let revision = text.revision as f64;
            text.mark_raster_pending(lod);

            let obj = Object::new();
            let _ = Reflect::set(&obj, &JsValue::from_str("id"), &JsValue::from(id));
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("revision"),
                &JsValue::from(revision),
            );
            let _ = Reflect::set(&obj, &JsValue::from_str("lod"), &JsValue::from(lod));
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("rasterScale"),
                &JsValue::from(params.scale),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("content"),
                &JsValue::from_str(&content),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("fontSizePx"),
                &JsValue::from(params.font_size_px),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("color"),
                &JsValue::from_str(&color),
            );
            let _ = Reflect::set(&obj, &JsValue::from_str("align"), &JsValue::from_str(align));
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("fontFamily"),
                &JsValue::from_str(&font_family),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("paddingWorld"),
                &JsValue::from(params.padding_world),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("textureId"),
                &JsValue::from_str(&texture_id),
            );
            let stroke_color = text.stroke.map(|color| color.to_hex()).unwrap_or_default();
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("strokeColor"),
                &JsValue::from_str(&stroke_color),
            );
            let _ = Reflect::set(
                &obj,
                &JsValue::from_str("strokeWidthWorld"),
                &JsValue::from(text.stroke_width),
            );
            jobs.push(&obj);
        }
        jobs
    }

    /// 把已经上传的纹理绑定到文本元素。
    ///
    /// `revision` 必须等于取任务时的版本，否则结果被丢弃并删除这张纹理。
    /// `world_width` / `world_height` 是纹理像素除以 `rasterScale`，用来固定世界尺寸。
    ///
    /// # 返回
    /// 是否接受了这张纹理。
    #[wasm_bindgen]
    pub fn commit_text_raster(
        &mut self,
        id: u64,
        revision: f64,
        lod: i32,
        texture_id: String,
        world_width: f32,
        world_height: f32,
    ) -> bool {
        let uploaded = self
            .webgpu_renderer
            .as_ref()
            .is_some_and(|renderer| renderer.has_texture(&texture_id));
        if !uploaded {
            if let Some(AnyElement::Text(text)) = self.scene.get_mut(id) {
                text.abandon_raster();
            }
            return false;
        }

        let textures = self.snapshot_text_textures();
        let revision = if revision.is_finite() && revision > 0.0 {
            revision as u64
        } else {
            0
        };
        let committed = match self.scene.get_mut(id) {
            Some(AnyElement::Text(text)) => text
                .commit_raster(
                    revision,
                    lod,
                    texture_id.clone(),
                    Vec2::new(world_width, world_height),
                )
                .is_some(),
            _ => false,
        };
        if committed {
            self.release_text_textures_except(textures);
            self.scene.mark_dirty(id);
            true
        } else {
            let still_used = self.snapshot_text_textures().contains(&texture_id);
            if !still_used {
                if let Some(renderer) = &mut self.webgpu_renderer {
                    renderer.remove_texture(&texture_id);
                }
            }
            false
        }
    }

    /// 光栅化失败后允许下一帧重新取任务。
    #[wasm_bindgen]
    pub fn abandon_text_raster(&mut self, id: u64) {
        if let Some(AnyElement::Text(text)) = self.scene.get_mut(id) {
            text.abandon_raster();
        }
    }

    /// 取走 T 工具留下的编辑请求。没有请求时返回 `null`。
    #[wasm_bindgen]
    pub fn take_text_edit_request(&mut self) -> JsValue {
        let Some(request) = self.pending_text_edit.take() else {
            return JsValue::NULL;
        };
        self.text_edit_payload(request.id, request.select_all)
    }

    /// 双击命中文本时返回编辑框几何。没命中文本返回 `null`。
    ///
    /// # 参数
    /// - `x`, `y`: 屏幕像素坐标（和 `handle_pointer_event` 用的是同一套）
    #[wasm_bindgen]
    pub fn begin_text_edit_at(&mut self, x: f32, y: f32) -> JsValue {
        let world = self.viewport.screen_to_world(Vec2::new(x, y));
        let hits = hit_test_point(
            &self.scene,
            world,
            &HitTestOptions {
                max_results: 1,
                include_invisible: false,
                include_locked: false,
            },
        );
        let Some(hit) = hits.first() else {
            return JsValue::NULL;
        };
        if hit.element_type != "Text" {
            return JsValue::NULL;
        }
        let _ = self
            .selection
            .select_element(&mut self.scene, hit.id, false);
        self.pending_text_edit = None;
        self.text_edit_payload(hit.id, false)
    }

    /// 查询文本在屏幕上的位置，编辑框跟着平移和缩放移动。
    #[wasm_bindgen]
    pub fn get_text_editor_metrics(&self, id: u64) -> JsValue {
        self.text_edit_payload(id, false)
    }

    /// 编辑时隐藏 GPU 文本，避免和 DOM 输入框叠两层字。
    #[wasm_bindgen]
    pub fn set_text_editing(&mut self, id: u64, editing: bool) -> bool {
        let updated = if let Some(AnyElement::Text(text)) = self.scene.get_mut(id) {
            text.set_editing(editing);
            true
        } else {
            false
        };
        if updated {
            self.scene.mark_dirty(id);
        }
        updated
    }

    /// 提交编辑内容。
    ///
    /// 内容有变化时写入撤销栈。只剩空白时删除元素，撤销会把原文放回来。
    ///
    /// # 返回
    /// 元素存在并且这次调用处理完了编辑状态。
    #[wasm_bindgen]
    pub fn commit_text_content(&mut self, id: u64, content: String) -> bool {
        let Some(AnyElement::Text(_)) = self.scene.get(id) else {
            return false;
        };
        if content.trim().is_empty() {
            if let Some(AnyElement::Text(text)) = self.scene.get_mut(id) {
                text.set_editing(false);
            }
            let textures = self.snapshot_text_textures();
            let _ = self.selection.deselect(&mut self.scene, id);
            self.selection.remove_id(id);
            self.command_history
                .execute(Box::new(RemoveElementCommand::new(id)), &mut self.scene);
            self.release_text_textures_except(textures);
            return true;
        }

        let textures = self.snapshot_text_textures();
        let edited = {
            let Some(AnyElement::Text(text)) = self.scene.get_mut(id) else {
                return false;
            };
            text.set_editing(false);
            if text.content == content {
                None
            } else {
                let before = text.visual();
                let _ = text.set_content(content);
                Some((before, text.visual()))
            }
        };
        if let Some((before, after)) = edited {
            self.command_history
                .push_executed(Box::new(UpdateTextCommand::new(id, before, after)));
        }
        self.scene.mark_dirty(id);
        self.release_text_textures_except(textures);
        true
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

    /// 绕视口中心按倍数缩放。
    ///
    /// 与 [`Self::zoom_at`] 同一条钳制路径，上下限是视口的最小/最大缩放，
    /// 也是 [`infinitecanvas_engine::camera_for_bounds`] 使用的那一组。
    /// 锚点是画布中心，相机中心不动。非有限或非正倍数会被忽略。
    /// 键盘放大传入 [`Self::zoom_step`]，缩小传入它的倒数。
    ///
    /// # 参数
    /// - `factor`: 相对倍数
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.zoom_by(canvas.zoom_step());
    /// ```
    #[wasm_bindgen]
    pub fn zoom_by(&mut self, factor: f32) {
        self.viewport.zoom_by(factor);
    }

    /// 滚轮和键盘共用的一步缩放倍数。
    ///
    /// 大于 1 表示放大。Demo 滚轮把指针交给 [`Self::zoom_at`]，
    /// 键盘把返回值交给 [`Self::zoom_by`]。
    #[wasm_bindgen]
    pub fn zoom_step(&self) -> f32 {
        ZOOM_STEP
    }

    /// 获取视口缩放
    #[wasm_bindgen]
    pub fn get_zoom(&self) -> f32 {
        self.viewport.zoom()
    }

    /// 屏幕坐标转世界坐标。拖放和粘贴图片时用它定位左上角。
    ///
    /// # 返回
    /// `[x, y]`
    #[wasm_bindgen]
    pub fn screen_to_world(&self, x: f32, y: f32) -> Vec<f32> {
        let world = self.viewport.screen_to_world(Vec2::new(x, y));
        vec![world.x, world.y]
    }

    /// 把缩放设为给定倍数，视口中心保持不变。
    ///
    /// 结果会被限制在 0.1 到 10 之间。非有限值会被忽略。
    ///
    /// # 参数
    /// - `zoom`: 缩放倍数，`1` 表示 100%
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.set_zoom(1);
    /// ```
    #[wasm_bindgen]
    pub fn set_zoom(&mut self, zoom: f32) {
        if zoom.is_finite() {
            self.viewport.set_zoom(zoom);
        }
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

    /// 把相机中心移到世界坐标。
    ///
    /// 点击小地图时，把这里传入 [`Self::minimap_to_world`] 的结果。
    /// 非有限值会被忽略。
    ///
    /// # 参数
    /// - `x`, `y`: 世界坐标
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// const world = canvas.minimap_to_world(px, py, 200, 150, 8);
    /// canvas.set_camera_center(world[0], world[1]);
    /// ```
    #[wasm_bindgen]
    pub fn set_camera_center(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            self.viewport.move_to(Vec2::new(x, y));
        }
    }

    /// 把相机对准当前选中内容。
    ///
    /// 只用可见、未锁定的选中元素。选中组时用组根的世界包围盒，不把子元素再展开。
    /// 每边留出视口的 10%，然后设置中心和缩放。不改元素几何，也不写入撤销栈。
    /// 空选，或过滤后没有可用包围盒时不改相机。
    ///
    /// # 返回
    /// 已经对准选中内容时为 `true`；没有可对准的内容时为 `false`。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.zoom_to_selection();
    /// ```
    #[wasm_bindgen]
    pub fn zoom_to_selection(&mut self) -> bool {
        let ids = self.selection.selected_ids();
        zoom_viewport_to_selection(&mut self.viewport, &self.scene, &ids)
    }

    /// 把相机对准全部可见叶子，使整张场景落在视口内并留出 10% 边距。
    ///
    /// 包围盒与 [`Self::scene_bounds`] 相同。不改元素几何，也不写入撤销栈。
    /// 没有可见叶子时把相机放回默认姿态（原点、缩放 1）并返回 `false`。
    /// [`Self::fit_scene`] 是同一个命令。
    ///
    /// # 返回
    /// 有内容并完成适应时为 `true`；场景为空（相机已回到默认）时为 `false`。
    ///
    /// # 示例（JavaScript）
    /// ```js
    /// canvas.zoom_to_fit();
    /// ```
    #[wasm_bindgen]
    pub fn zoom_to_fit(&mut self) -> bool {
        self.fit_visible_scene()
    }

    /// [`Self::zoom_to_fit`] 的别名。
    #[wasm_bindgen]
    pub fn fit_scene(&mut self) -> bool {
        self.fit_visible_scene()
    }

    fn fit_visible_scene(&mut self) -> bool {
        zoom_viewport_to_scene(&mut self.viewport, &self.scene)
    }

    /// 可见叶子的世界包围盒。
    ///
    /// 组不计入，隐藏元素及其子孙也不计入。空场景返回 `null`。
    ///
    /// # 返回
    /// `{ x, y, width, height }`，或 `null`。
    #[wasm_bindgen]
    pub fn scene_bounds(&self) -> JsValue {
        match minimap::scene_bounds(&self.scene) {
            Some(rect) => rect_to_js(rect),
            None => JsValue::NULL,
        }
    }

    /// 小地图几何版本。
    ///
    /// 与 [`infinitecanvas_engine::Scene::geometry_epoch`] 相同。
    /// 元素增删或几何变化时递增，平移/缩放相机不变。
    /// 页面应在版本变化时才调用 [`Self::minimap_element_rects`]，
    /// 相机变化只调用 [`Self::minimap_viewport_rect`]。
    #[wasm_bindgen]
    pub fn minimap_epoch(&self) -> u64 {
        self.scene.geometry_epoch()
    }

    /// 元素在小地图上的简化外框。
    ///
    /// 同一几何版本和尺寸会复用缓存，避免 10 万元素时每帧重扫。
    /// 返回 `Float32Array`，每 4 个数是一个像素矩形 `x, y, width, height`。
    ///
    /// # 参数
    /// - `width`, `height`: 小地图像素尺寸，例如 200×150
    /// - `padding`: 四周留白（像素）
    #[wasm_bindgen]
    pub fn minimap_element_rects(
        &mut self,
        width: f32,
        height: f32,
        padding: f32,
    ) -> js_sys::Float32Array {
        self.ensure_minimap_cache(width, height, padding);
        float32_array(&self.minimap_cache.as_ref().unwrap().rects)
    }

    /// 当前视口在小地图上的像素矩形。
    ///
    /// 映射跟元素外框共用缓存，本身不扫描场景。
    ///
    /// # 返回
    /// `Float32Array` `[x, y, width, height]`。
    #[wasm_bindgen]
    pub fn minimap_viewport_rect(
        &mut self,
        width: f32,
        height: f32,
        padding: f32,
    ) -> js_sys::Float32Array {
        self.ensure_minimap_cache(width, height, padding);
        let frame = self.minimap_cache.as_ref().unwrap().frame;
        let rect = minimap::viewport_minimap_rect(&frame, &self.viewport);
        float32_array(&[rect.x, rect.y, rect.width, rect.height])
    }

    /// 小地图像素坐标反算世界坐标。
    ///
    /// 点击处要成为相机中心时，把返回值交给 [`Self::set_camera_center`]。
    /// 留白和内容区用的是同一个线性映射。
    ///
    /// # 返回
    /// `Float32Array` `[x, y]`。
    #[wasm_bindgen]
    pub fn minimap_to_world(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        padding: f32,
    ) -> js_sys::Float32Array {
        self.ensure_minimap_cache(width, height, padding);
        let frame = self.minimap_cache.as_ref().unwrap().frame;
        let world = minimap::minimap_to_world(&frame, Vec2::new(x, y));
        float32_array(&[world.x, world.y])
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
        if let Some(cursor) = self.guide_cursor(x, y) {
            return cursor.to_string();
        }
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

fn rect_to_js(rect: Rect) -> JsValue {
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
}

fn float32_array(values: &[f32]) -> js_sys::Float32Array {
    let array = js_sys::Float32Array::new_with_length(values.len() as u32);
    array.copy_from(values);
    array
}

impl InfiniteCanvas {
    /// 几何版本或小地图尺寸变化时重算外框缓存。相机变化不进入这里的扫描。
    fn ensure_minimap_cache(&mut self, width: f32, height: f32, padding: f32) {
        let (size, padding) = minimap::normalize_minimap_view(Vec2::new(width, height), padding);
        let epoch = self.scene.geometry_epoch();
        let fresh = self.minimap_cache.as_ref().is_some_and(|cache| {
            cache.epoch == epoch
                && cache.width == size.x
                && cache.height == size.y
                && cache.padding == padding
        });
        if fresh {
            return;
        }
        let overview = minimap::build_overview(&self.scene, size, padding);
        self.minimap_cache = Some(MinimapCache {
            epoch,
            width: size.x,
            height: size.y,
            padding,
            frame: overview.frame,
            rects: overview.rects,
        });
    }

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
        if self.try_begin_guide_gesture(&event) {
            return;
        }
        self.selected_guide = None;

        let current_tool = self.tool_manager.current();

        match current_tool {
            Tool::Select => self.handle_select_tool_down(event),
            Tool::Rectangle | Tool::Circle | Tool::Polygon | Tool::Star | Tool::Frame => {
                self.handle_draw_tool_down(event, current_tool)
            }
            Tool::Line => self.handle_line_tool_down(event),
            Tool::Pen => self.handle_pen_tool_down(event),
            Tool::Pan | Tool::Hand => self.handle_pan_tool_down(event),
            Tool::Text => self.handle_text_tool_down(event),
            Tool::Eyedropper => self.handle_eyedropper_down(event),
        }
    }

    fn handle_pointer_move(&mut self, event: PointerEvent) {
        if self.update_guide_gesture(&event) {
            return;
        }
        let current_tool = self.tool_manager.current();

        match current_tool {
            Tool::Select => self.handle_select_tool_move(event),
            Tool::Rectangle | Tool::Circle | Tool::Polygon | Tool::Star | Tool::Frame => {
                self.handle_draw_tool_move(event)
            }
            Tool::Line => self.handle_line_tool_move(event),
            Tool::Pen => self.handle_pen_tool_move(event),
            Tool::Pan | Tool::Hand => self.handle_pan_tool_move(event),
            Tool::Text => self.handle_text_tool_move(event),
            Tool::Eyedropper => {}
        }
    }

    fn handle_pointer_up(&mut self, event: PointerEvent) {
        if self.finish_guide_gesture(&event) {
            return;
        }
        let current_tool = self.tool_manager.current();

        match current_tool {
            Tool::Select => self.handle_select_tool_up(event),
            Tool::Rectangle | Tool::Circle | Tool::Polygon | Tool::Star | Tool::Frame => {
                self.handle_draw_tool_up(event, current_tool)
            }
            Tool::Line => self.handle_line_tool_up(event),
            Tool::Pen => self.handle_pen_tool_up(event),
            Tool::Pan | Tool::Hand => self.handle_pan_tool_up(event),
            Tool::Text => self.handle_text_tool_up(event),
            Tool::Eyedropper => self.finish_eyedropper_gesture(true),
        }
    }

    /// 吸管按下：采样并尝试写到当前选择。不改选择，也不开始拖动。
    fn handle_eyedropper_down(&mut self, event: PointerEvent) {
        let sample = sample_paint_at(&self.scene, event.world_position);
        let hit = sample.is_some();
        let applied = sample
            .map(|sample| self.apply_paint_sample(&sample))
            .unwrap_or(false);
        self.eyedropper_gesture = Some(EyedropperGesture { hit, applied });
    }

    /// 松开时把结果交给页面。点到元素才离开吸管。
    fn finish_eyedropper_gesture(&mut self, restore_tool: bool) {
        let Some(gesture) = self.eyedropper_gesture.take() else {
            return;
        };
        if restore_tool && gesture.hit {
            self.tool_manager.exit_eyedropper();
        }
        self.eyedropper_notice = Some(gesture);
    }

    // ========== 选择工具事件处理 ==========

    fn handle_select_tool_down(&mut self, event: PointerEvent) {
        if self.try_begin_handle_gesture(&event) {
            return;
        }
        let hit_id = if event.modifiers.alt {
            // Alt：光标下最上层的画板优先于它上面的子元素。
            click_target(&self.scene, event.world_position, true)
        } else {
            let options = HitTestOptions {
                max_results: 1,
                include_invisible: false,
                include_locked: false,
            };
            hit_test_point(&self.scene, event.world_position, &options)
                .first()
                .map(|hit| hit.id)
        };
        let additive = event.modifiers.ctrl || event.modifiers.meta || event.modifiers.shift;

        if let Some(id) = hit_id {
            // 点击到已选中的元素：开始拖动
            if self.selection.is_selected(id) {
                self.start_drag(event.pointer_id, event.world_position);
                self.selection_box.cancel();
            } else {
                // 点击到未选中的元素：选中它，并开始拖动
                self.selection.select_element(&mut self.scene, id, additive);
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
            self.drag_selected_to_pointer(event.world_position);
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
        self.discard_preview();
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
            let tool = self.tool_manager.current();
            if matches!(tool, Tool::Polygon | Tool::Star) {
                let _ = self.update_regular_preview(tool, event.world_position);
            }
        }
    }

    fn handle_draw_tool_up(&mut self, event: PointerEvent, tool: Tool) {
        if !self.drag_state.is_dragging || self.drag_state.pointer_id != Some(event.pointer_id) {
            return;
        }

        let start = self.drag_state.start_position;
        let end = event.world_position;

        if matches!(tool, Tool::Polygon | Tool::Star) {
            let created = self.update_regular_preview(tool, end);
            self.drag_state.is_dragging = false;
            self.drag_state.pointer_id = None;
            if created {
                self.commit_preview();
            } else {
                self.discard_preview();
            }
            return;
        }

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
                Tool::Frame => {
                    let frame = Frame::new(id, center, Vec2::new(width, height)).with_name("画板");
                    self.command_history.execute(
                        Box::new(AddElementCommand::new(AnyElement::Frame(frame))),
                        &mut self.scene,
                    );
                    log::info!(
                        "Created frame {} at {:?}, size {:?}",
                        id,
                        center,
                        (width, height)
                    );
                }
                _ => {}
            }

            // 选中新创建的元素
            self.selection.select_element(&mut self.scene, id, false);
        }

        self.drag_state.is_dragging = false;
        self.drag_state.pointer_id = None;
    }

    /// 拖拽过程中更新正多边形或星形预览。尺寸太小时返回 false，调用方丢掉预览。
    fn update_regular_preview(&mut self, tool: Tool, end: Vec2) -> bool {
        let start = self.drag_state.start_position;
        let width = (end.x - start.x).abs();
        let height = (end.y - start.y).abs();
        const MIN_SIZE: f32 = 5.0;
        if width <= MIN_SIZE || height <= MIN_SIZE {
            return false;
        }
        let radius = width.min(height) / 2.0;
        let center = Vec2::new(
            start.x.min(end.x) + width * 0.5,
            start.y.min(end.y) + height * 0.5,
        );
        if let Some(id) = self.drag_state.preview_id {
            if let Some(AnyElement::Polygon(polygon)) = self.scene.get_mut(id) {
                polygon.props.transform.translation = center;
                polygon.set_radius(radius);
                self.scene.mark_dirty(id);
                return true;
            }
        }
        let id = self.scene.next_id();
        let (polygon, label) = match tool {
            Tool::Star => (
                Polygon::star(id, center, radius).with_style(ElementStyle::fill_only(
                    Color::from_hex("#a855f7").unwrap_or(Color::BLUE),
                )),
                "star",
            ),
            _ => (
                Polygon::hexagon(id, center, radius).with_style(ElementStyle::fill_only(
                    Color::from_hex("#f59e0b").unwrap_or(Color::BLUE),
                )),
                "polygon",
            ),
        };
        self.scene.add_polygon(polygon);
        self.drag_state.preview_id = Some(id);
        log::trace!("Preview {label} {id} radius {radius}");
        true
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
        // 预览元素离开场景时引用可能降到 0。立刻放回，再由添加命令重新计数。
        let parked = self.scene.take_detached_assets();
        self.scene.restore_assets(parked);
        self.command_history
            .execute(Box::new(AddElementCommand::new(element)), &mut self.scene);
        self.selection.select_element(&mut self.scene, id, false);
        log::info!("Committed element {}", id);
    }

    fn discard_preview(&mut self) {
        if let Some(id) = self.drag_state.preview_id.take() {
            self.scene.remove(id);
            let _ = self.scene.take_detached_assets();
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
        if self
            .selected_guide
            .is_some_and(|id| self.scene.guides().get(id).is_none())
        {
            self.selected_guide = None;
        }
    }

    // ========== 文本工具 ==========

    fn handle_text_tool_down(&mut self, event: PointerEvent) {
        self.drag_state.is_dragging = true;
        self.drag_state.pointer_id = Some(event.pointer_id);
        self.drag_state.start_position = event.world_position;
        self.drag_state.last_position = event.world_position;
        self.drag_state.total_delta = Vec2::ZERO;
    }

    fn handle_text_tool_move(&mut self, event: PointerEvent) {
        if self.drag_state.is_dragging && self.drag_state.pointer_id == Some(event.pointer_id) {
            self.drag_state.last_position = event.world_position;
        }
    }

    fn handle_text_tool_up(&mut self, event: PointerEvent) {
        if !self.drag_state.is_dragging || self.drag_state.pointer_id != Some(event.pointer_id) {
            return;
        }
        self.drag_state.is_dragging = false;
        self.drag_state.pointer_id = None;

        let start = self.drag_state.start_position;
        let zoom = self.viewport.zoom().max(0.01);
        // 拖动不是创建。阈值按屏幕像素算，避免缩小后轻轻一抖就被当成拖动。
        if (event.world_position - start).length() > 6.0 / zoom {
            return;
        }

        let hit_text = hit_test_point(
            &self.scene,
            start,
            &HitTestOptions {
                max_results: 8,
                include_invisible: false,
                include_locked: false,
            },
        )
        .into_iter()
        .find(|hit| hit.element_type == "Text")
        .map(|hit| hit.id);

        match self.text_clicks.resolve(start, hit_text, Instant::now()) {
            TextPlacement::Edit { id, select_all } => {
                if self.is_text(id) {
                    let _ = self.selection.select_element(&mut self.scene, id, false);
                    self.pending_text_edit = Some(TextEditRequest { id, select_all });
                } else {
                    self.create_text_element(start);
                }
            }
            TextPlacement::Create(position) => {
                self.create_text_element(position);
            }
        }
    }

    /// 用撤销命令创建一段默认文本，并请求前端打开编辑框。
    fn create_text_element(&mut self, position: Vec2) -> ElementId {
        let id = self.scene.next_id();
        let text = Text::new(id, position, DEFAULT_TEXT_CONTENT)
            .with_font_size(DEFAULT_FONT_SIZE)
            .with_color(DEFAULT_TEXT_COLOR);
        self.command_history.execute(
            Box::new(AddElementCommand::new(AnyElement::Text(text))),
            &mut self.scene,
        );
        self.scene.mark_dirty(id);
        let _ = self.selection.select_element(&mut self.scene, id, false);
        self.text_clicks.remember(id, position, Instant::now());
        self.pending_text_edit = Some(TextEditRequest {
            id,
            select_all: true,
        });
        log::info!("Created text {} at {:?}", id, position);
        id
    }

    fn is_text(&self, id: ElementId) -> bool {
        matches!(self.scene.get(id), Some(AnyElement::Text(_)))
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
        let dragged_ids: Vec<ElementId> = self
            .selection
            .selected_ids()
            .into_iter()
            .filter(|id| !self.scene.is_effectively_locked(*id))
            .collect();
        if dragged_ids.is_empty() {
            return;
        }
        self.drag_state.is_dragging = true;
        self.drag_state.pointer_id = Some(pointer_id);
        self.drag_state.start_position = position;
        self.drag_state.last_position = position;
        self.drag_state.total_delta = Vec2::ZERO;
        self.drag_state.dragged_ids = dragged_ids;
        self.drag_state.origin_bounds = self.dragged_origin_bounds();
        self.drag_state.guides.clear();
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
        self.drag_state.origin_bounds = None;
        self.drag_state.guides.clear();
    }

    /// 让选区跟着指针走，并在阈值内贴齐其他可见元素。
    ///
    /// 位移从按下时的包围盒重算，不在上一帧的吸附结果上累加，
    /// 这样离开阈值后会回到指针对应的位置。关闭吸附时修正为 0。
    fn drag_selected_to_pointer(&mut self, pointer: Vec2) {
        let raw_delta = pointer - self.drag_state.start_position;
        let snapped = match (self.snap.enabled, self.drag_state.origin_bounds) {
            (true, Some(origin)) => {
                let targets = self.snap_targets();
                snap_drag_with_guides(
                    origin,
                    raw_delta,
                    &targets,
                    &self.manual_guide_lines(),
                    &self.snap,
                    self.viewport.zoom(),
                )
            }
            _ => DragSnap {
                translation: raw_delta,
                guides: Vec::new(),
            },
        };
        let correction = snapped.translation - self.drag_state.total_delta;
        if correction != Vec2::ZERO {
            // 吸附修正和普通拖动同一条路径：选中的组会带着子孙一起移动。
            self.scene
                .translate_ids(&self.drag_state.dragged_ids, correction);
        }
        self.drag_state.total_delta = snapped.translation;
        self.drag_state.last_position = pointer;
        self.drag_state.guides = snapped.guides;
    }

    /// 按下时、被拖元素还没移动前的可见包围盒。
    ///
    /// 组没有几何，对齐主体是子孙叶子的世界包围盒并集。祖先被隐藏时整棵子树不参与。
    fn dragged_origin_bounds(&self) -> Option<Rect> {
        let mut leaves = Vec::new();
        for id in &self.drag_state.dragged_ids {
            self.collect_visible_leaf_bounds(*id, &mut leaves);
        }
        bounds_union(leaves)
    }

    /// 其他可见叶子的世界包围盒。
    ///
    /// 组本身不参与。正在拖动的元素及其子孙也不参与，避免吸到自己。
    /// 祖先不可见的元素和命中测试一样跳过。
    fn snap_targets(&self) -> Vec<Rect> {
        let mut excluded = HashSet::new();
        for id in &self.drag_state.dragged_ids {
            for element_id in self.scene.subtree_ids(*id) {
                excluded.insert(element_id);
            }
        }
        self.scene
            .iter()
            .filter_map(|element| {
                if element.as_group().is_some() {
                    return None;
                }
                let id = element.id();
                if excluded.contains(&id) || !self.scene.is_effectively_visible(id) {
                    return None;
                }
                Some(element.as_element().world_bounds())
            })
            .collect()
    }

    /// 组展开成可见叶子的世界包围盒。叶子自己不可见，或任一祖先不可见，则跳过。
    fn collect_visible_leaf_bounds(&self, id: ElementId, out: &mut Vec<Rect>) {
        if !self.scene.is_effectively_visible(id) {
            return;
        }
        let Some(element) = self.scene.get(id) else {
            return;
        };
        if element.as_group().is_some() {
            for child in self.scene.children_of(id) {
                self.collect_visible_leaf_bounds(child, out);
            }
            return;
        }
        out.push(element.as_element().world_bounds());
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
            if self.scene.is_effectively_locked(id) {
                continue;
            }
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
            .filter(|id| !self.scene.is_effectively_locked(*id))
            .filter_map(|id| {
                self.scene
                    .get(id)
                    .map(|element| (id, element.as_element().props().transform))
            })
            .collect()
    }

    fn apply_transform_list(&mut self, transforms: &[(ElementId, Transform)]) {
        self.scene.apply_world_transforms(transforms);
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
        let lock_aspect = self.image_selection_locks_aspect();
        let pointer = self.snapped_resize_pointer(event, lock_aspect);
        let preview = {
            let Some(drag) = &self.handle_drag else {
                return false;
            };
            if drag.pointer_id != event.pointer_id {
                return false;
            }
            let metrics = HandleMetrics::from_zoom(self.viewport.zoom());
            drag.gesture.preview_with(
                pointer,
                event.modifiers.shift,
                metrics.min_half,
                lock_aspect,
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

    /// 隐藏标尺时不把参考线交给吸附。
    fn manual_guide_lines(&self) -> GuideLines {
        if self.rulers_visible {
            self.scene.guides().lines()
        } else {
            GuideLines::default()
        }
    }

    fn guide_cursor(&self, x: f32, y: f32) -> Option<&'static str> {
        if !self.rulers_visible {
            return None;
        }
        if let Some(gesture) = &self.guide_gesture {
            return Some(axis_cursor(gesture.axis));
        }
        let screen = Vec2::new(x, y);
        match ruler_band(screen, RULER_THICKNESS_PX) {
            Some(RulerBand::Top) => return Some("ns-resize"),
            Some(RulerBand::Left) => return Some("ew-resize"),
            Some(RulerBand::Corner) => return Some("default"),
            None => {}
        }
        if self.tool_manager.current() != Tool::Select {
            return None;
        }
        let world = self.viewport.screen_to_world(screen);
        let threshold = GUIDE_HIT_PX / self.viewport.zoom().max(0.05);
        let id = self.scene.guides().hit_test(world, threshold)?;
        Some(axis_cursor(self.scene.guides().get(id)?.axis))
    }

    /// 标尺上开始新建，或在选择工具里抓住已有参考线。手柄优先于参考线。
    fn try_begin_guide_gesture(&mut self, event: &PointerEvent) -> bool {
        if !self.rulers_visible {
            return false;
        }
        if let Some(band) = ruler_band(event.screen_position, RULER_THICKNESS_PX) {
            let axis = match band {
                RulerBand::Top => GuideAxis::Horizontal,
                RulerBand::Left => GuideAxis::Vertical,
                RulerBand::Corner => {
                    self.selected_guide = None;
                    return true;
                }
            };
            self.begin_guide_drag(event.pointer_id, axis, None, None);
            return true;
        }
        if self.tool_manager.current() != Tool::Select
            || self.pointer_hits_handle(event.world_position)
        {
            return false;
        }
        let threshold = GUIDE_HIT_PX / self.viewport.zoom().max(0.05);
        let Some(id) = self
            .scene
            .guides()
            .hit_test(event.world_position, threshold)
        else {
            return false;
        };
        let Some(guide) = self.scene.guides().get(id).copied() else {
            return false;
        };
        self.selection.clear(&mut self.scene);
        self.begin_guide_drag(event.pointer_id, guide.axis, Some(id), Some(guide.position));
        true
    }

    fn begin_guide_drag(
        &mut self,
        pointer_id: i32,
        axis: GuideAxis,
        id: Option<u64>,
        origin: Option<f32>,
    ) {
        self.cancel_handle_gesture();
        self.selection_box.cancel();
        self.selected_guide = id;
        self.guide_gesture = Some(GuideGesture {
            pointer_id,
            axis,
            id,
            origin,
            before: self.scene.guide_snapshot(),
        });
    }

    fn pointer_hits_handle(&self, world: Vec2) -> bool {
        let Some(frame) = self.compute_selection_frame() else {
            return false;
        };
        let metrics = HandleMetrics::from_zoom(self.viewport.zoom());
        frame
            .hit_test(world, metrics.hit_radius, metrics.rotate_gap)
            .is_some()
    }

    fn update_guide_gesture(&mut self, event: &PointerEvent) -> bool {
        let Some(gesture) = self.guide_gesture.as_ref() else {
            return false;
        };
        if gesture.pointer_id != event.pointer_id {
            return false;
        }
        let axis = gesture.axis;
        let existing = gesture.id;
        let creating = gesture.origin.is_none();
        let position = match axis {
            GuideAxis::Horizontal => event.world_position.y,
            GuideAxis::Vertical => event.world_position.x,
        };
        if !position.is_finite() {
            return true;
        }
        let on_ruler = pointer_on_source_ruler(event.screen_position, axis);
        if on_ruler && creating {
            if let Some(id) = existing {
                self.scene.remove_guide(id);
                if let Some(gesture) = &mut self.guide_gesture {
                    gesture.id = None;
                }
                self.selected_guide = None;
            }
            return true;
        }
        match existing {
            Some(id) => {
                self.scene.move_guide(id, position);
                self.selected_guide = Some(id);
            }
            None => {
                if let Some(guide) = self.scene.add_guide(axis, position) {
                    if let Some(gesture) = &mut self.guide_gesture {
                        gesture.id = Some(guide.id);
                    }
                    self.selected_guide = Some(guide.id);
                }
            }
        }
        true
    }

    fn finish_guide_gesture(&mut self, event: &PointerEvent) -> bool {
        let Some(gesture) = self.guide_gesture.as_ref() else {
            return false;
        };
        if gesture.pointer_id != event.pointer_id {
            return false;
        }
        let gesture = self.guide_gesture.take().expect("gesture exists");
        let on_ruler = pointer_on_source_ruler(event.screen_position, gesture.axis);
        if on_ruler {
            if gesture.origin.is_none() {
                self.scene.restore_guides(&gesture.before);
                self.selected_guide = None;
            } else if gesture.id.is_some() {
                if let Some(id) = gesture.id {
                    self.scene.remove_guide(id);
                }
                self.selected_guide = None;
                self.push_guide_edit(gesture.before);
            }
            return true;
        }
        if gesture.id.is_some() {
            self.push_guide_edit(gesture.before);
        }
        true
    }

    fn cancel_guide_gesture(&mut self) {
        if let Some(gesture) = self.guide_gesture.take() {
            self.scene.restore_guides(&gesture.before);
            self.selected_guide = gesture.origin.and(gesture.id);
        }
    }

    fn push_guide_edit(&mut self, before: GuideSnapshot) {
        let after = self.scene.guide_snapshot();
        if after != before {
            self.command_history
                .push_executed(Box::new(EditGuidesCommand::new(before, after)));
        }
    }

    fn delete_guide_id(&mut self, id: u64) -> bool {
        if self.guide_gesture.is_some() {
            self.cancel_guide_gesture();
        }
        if self.scene.guides().get(id).is_none() {
            self.selected_guide = None;
            return false;
        }
        let before = self.scene.guide_snapshot();
        self.scene.remove_guide(id);
        self.push_guide_edit(before);
        self.selected_guide = None;
        true
    }

    /// 缩放手柄把元素边吸到参考线。旋转仍只吃 Shift 的 15°。
    fn snapped_resize_pointer(&mut self, event: &PointerEvent, lock_aspect: bool) -> Vec2 {
        let pointer = event.world_position;
        if !self.snap.enabled || !self.rulers_visible {
            return pointer;
        }
        let guides = self.scene.guides().lines();
        if guides.horizontal.is_empty() && guides.vertical.is_empty() {
            return pointer;
        }
        let Some(drag) = &self.handle_drag else {
            return pointer;
        };
        if drag.pointer_id != event.pointer_id {
            return pointer;
        }
        let HandleKind::Resize(handle) = drag.gesture.kind() else {
            return pointer;
        };
        let gesture = drag.gesture.clone();
        let ids: Vec<ElementId> = gesture.originals().iter().map(|(id, _)| *id).collect();
        let metrics = HandleMetrics::from_zoom(self.viewport.zoom());
        let shift = event.modifiers.shift;
        let outward = handle_outward(handle, gesture.origin().rotation);
        let uniform = lock_aspect != shift;
        let base = gesture.preview_with(pointer, shift, metrics.min_half, lock_aspect);
        self.apply_transform_list(&base.transforms);
        let Some(base_bounds) = self.bounds_for_ids(&ids) else {
            return pointer;
        };
        let x_correction = self.edge_pointer_correction(
            &gesture,
            pointer,
            Vec2::new(1.0, 0.0),
            shift,
            metrics.min_half,
            lock_aspect,
            &ids,
            moving_edge(base_bounds, outward.x, false),
            &guides.vertical,
            outward.x.abs() > 0.25,
            outward.x,
        );
        let y_correction = self.edge_pointer_correction(
            &gesture,
            pointer,
            Vec2::new(0.0, 1.0),
            shift,
            metrics.min_half,
            lock_aspect,
            &ids,
            moving_edge(base_bounds, outward.y, true),
            &guides.horizontal,
            outward.y.abs() > 0.25,
            outward.y,
        );
        if uniform {
            if x_correction.abs() <= y_correction.abs() && x_correction != 0.0 {
                Vec2::new(pointer.x + x_correction, pointer.y)
            } else {
                Vec2::new(pointer.x, pointer.y + y_correction)
            }
        } else {
            Vec2::new(pointer.x + x_correction, pointer.y + y_correction)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn edge_pointer_correction(
        &mut self,
        gesture: &HandleGesture,
        pointer: Vec2,
        nudge: Vec2,
        shift: bool,
        min_half: f32,
        lock_aspect: bool,
        ids: &[ElementId],
        edge: f32,
        guides: &[f32],
        active: bool,
        edge_sign: f32,
    ) -> f32 {
        if !active || guides.is_empty() {
            return 0.0;
        }
        let preview = gesture.preview_with(pointer + nudge, shift, min_half, lock_aspect);
        self.apply_transform_list(&preview.transforms);
        let Some(nudged) = self.bounds_for_ids(ids) else {
            return 0.0;
        };
        let horizontal = nudge.y != 0.0;
        let edge_nudged = moving_edge(nudged, edge_sign, horizontal);
        resize_edge_correction(
            edge,
            edge_nudged - edge,
            guides,
            self.snap.world_threshold(self.viewport.zoom()),
        )
    }

    fn bounds_for_ids(&self, ids: &[ElementId]) -> Option<Rect> {
        let mut leaves = Vec::new();
        for id in ids {
            self.collect_visible_leaf_bounds(*id, &mut leaves);
        }
        bounds_union(leaves)
    }
}

fn axis_cursor(axis: GuideAxis) -> &'static str {
    match axis {
        GuideAxis::Horizontal => "ns-resize",
        GuideAxis::Vertical => "ew-resize",
    }
}

fn pointer_on_source_ruler(screen: Vec2, axis: GuideAxis) -> bool {
    match ruler_band(screen, RULER_THICKNESS_PX) {
        Some(RulerBand::Corner) => true,
        Some(RulerBand::Top) => axis == GuideAxis::Horizontal,
        Some(RulerBand::Left) => axis == GuideAxis::Vertical,
        None => false,
    }
}

fn scene_tree_node_to_js(node: &SceneTreeNode) -> Object {
    let obj = Object::new();
    let _ = Reflect::set(&obj, &JsValue::from_str("id"), &JsValue::from(node.id));
    let _ = Reflect::set(
        &obj,
        &JsValue::from_str("elementType"),
        &JsValue::from_str(&node.element_type),
    );
    let parent = match node.parent_id {
        Some(parent_id) => JsValue::from(parent_id),
        None => JsValue::NULL,
    };
    let _ = Reflect::set(&obj, &JsValue::from_str("parentId"), &parent);
    let _ = Reflect::set(
        &obj,
        &JsValue::from_str("visible"),
        &JsValue::from(node.visible),
    );
    let _ = Reflect::set(
        &obj,
        &JsValue::from_str("locked"),
        &JsValue::from(node.locked),
    );
    let _ = Reflect::set(
        &obj,
        &JsValue::from_str("zIndex"),
        &JsValue::from(node.z_index),
    );
    let _ = Reflect::set(
        &obj,
        &JsValue::from_str("name"),
        &JsValue::from_str(&node.name),
    );
    let children = Array::new();
    for child in &node.children {
        children.push(&scene_tree_node_to_js(child));
    }
    let _ = Reflect::set(&obj, &JsValue::from_str("children"), &children);
    obj
}

fn element_ids_from_js(ids: &Array) -> Vec<ElementId> {
    let mut out = Vec::with_capacity(ids.length() as usize);
    for index in 0..ids.length() {
        if let Some(id) = element_id_from_js(ids.get(index)) {
            out.push(id);
        }
    }
    out
}

fn element_id_from_js(value: JsValue) -> Option<ElementId> {
    if let Some(number) = value.as_f64() {
        if number.is_finite() && number >= 0.0 {
            return Some(number as ElementId);
        }
    }
    let Ok(bigint) = js_sys::BigInt::new(&value) else {
        return None;
    };
    bigint
        .to_string(10)
        .ok()
        .and_then(|text| text.as_string())
        .and_then(|text| text.parse().ok())
}

fn polyline_length(points: &[Vec2]) -> f32 {
    points
        .windows(2)
        .map(|window| (window[1] - window[0]).length())
        .sum()
}

impl InfiniteCanvas {
    fn image_selection_locks_aspect(&self) -> bool {
        selection_locks_image_aspect(self.selected_element_refs())
    }

    /// 按场景资源表上传或释放 GPU 纹理。
    ///
    /// 纹理 id 就是资源 id。最后一个元素引用消失后，GPU 纹理一起删掉。
    /// 渲染器还没初始化时先不动，等下一帧再传。
    fn sync_image_textures(&mut self) {
        if self.webgpu_renderer.is_none() {
            return;
        }
        let needed = self.scene.referenced_asset_ids();
        if needed == self.gpu_image_assets {
            return;
        }
        let uploads: Vec<(String, u32, u32, Vec<u8>)> = needed
            .iter()
            .filter(|id| !self.gpu_image_assets.contains(*id))
            .filter_map(|id| {
                let asset = self.scene.image_asset(id)?;
                Some((
                    asset.id.clone(),
                    asset.width,
                    asset.height,
                    asset.rgba.clone(),
                ))
            })
            .collect();
        let stale: Vec<String> = self.gpu_image_assets.difference(&needed).cloned().collect();
        if let Some(renderer) = self.webgpu_renderer.as_mut() {
            for id in &stale {
                renderer.remove_texture(id);
            }
            for (id, width, height, rgba) in &uploads {
                if renderer.has_texture(id) {
                    continue;
                }
                if let Err(err) = renderer.upload_texture(id.clone(), *width, *height, rgba) {
                    log::error!("Failed to upload image texture {id}: {err}");
                }
            }
        }
        self.gpu_image_assets = needed;
    }

    fn snapshot_text_textures(&self) -> HashSet<String> {
        self.scene
            .iter()
            .filter_map(|element| match element {
                AnyElement::Text(text) => text.texture_id.clone(),
                _ => None,
            })
            .collect()
    }

    fn release_text_textures_except(&mut self, before: HashSet<String>) {
        let after = self.snapshot_text_textures();
        let Some(renderer) = &mut self.webgpu_renderer else {
            return;
        };
        for id in before {
            if !after.contains(&id) {
                renderer.remove_texture(&id);
            }
        }
    }

    /// 编辑框需要的屏幕几何。元素不存在或不是文本时返回 `null`。
    fn text_edit_payload(&self, id: ElementId, select_all: bool) -> JsValue {
        let Some(AnyElement::Text(text)) = self.scene.get(id) else {
            return JsValue::NULL;
        };
        let bounds = text.world_bounds();
        let origin = self.viewport.world_to_screen(Vec2::new(bounds.x, bounds.y));
        let zoom = self.viewport.zoom();
        let obj = Object::new();
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("id"),
            &JsValue::from(text.props.id),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("content"),
            &JsValue::from_str(&text.content),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("fontSize"),
            &JsValue::from(text.font_size),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("screenFontSize"),
            &JsValue::from(text.font_size * zoom),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("color"),
            &JsValue::from_str(&text.color.to_hex()),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("align"),
            &JsValue::from_str(text.align.as_str()),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("fontFamily"),
            &JsValue::from_str(&text.font_family),
        );
        let _ = Reflect::set(&obj, &JsValue::from_str("x"), &JsValue::from(origin.x));
        let _ = Reflect::set(&obj, &JsValue::from_str("y"), &JsValue::from(origin.y));
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("width"),
            &JsValue::from(bounds.width * zoom),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("height"),
            &JsValue::from(bounds.height * zoom),
        );
        let _ = Reflect::set(
            &obj,
            &JsValue::from_str("selectAll"),
            &JsValue::from(select_all),
        );
        obj.into()
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use infinitecanvas_engine::VertexCache;

    /// 不初始化浏览器日志，只给拖拽吸附测试用。
    fn blank_canvas() -> InfiniteCanvas {
        InfiniteCanvas {
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
            text_clicks: TextClickResolver::new(),
            pending_text_edit: None,
            perf_stats: PerformanceStats::default(),
            snap: SnapSettings::default(),
            rulers_visible: true,
            grid_visible: true,
            guide_gesture: None,
            selected_guide: None,
            style_preview: None,
            minimap_cache: None,
            gpu_image_assets: HashSet::new(),
            eyedropper_gesture: None,
            eyedropper_notice: None,
        }
    }

    /// 背景网格默认可见，切换不进撤销栈。
    #[test]
    fn grid_is_visible_by_default_and_the_toggle_is_not_undoable() {
        let mut canvas = blank_canvas();
        assert!(canvas.grid_visible());
        canvas.set_grid_visible(false);
        assert!(!canvas.grid_visible());
        assert_eq!(canvas.get_undo_count(), 0);
        canvas.set_grid_visible(true);
        assert!(canvas.grid_visible());
        assert_eq!(canvas.get_undo_count(), 0);
    }

    fn translation(scene: &Scene, id: ElementId) -> Vec2 {
        scene
            .get(id)
            .unwrap()
            .as_element()
            .props()
            .transform
            .translation
    }

    #[test]
    fn dragging_a_group_snaps_children_to_an_external_edge() {
        let mut canvas = blank_canvas();
        let left = canvas.scene.next_id();
        canvas.scene.add_rect(Rectangle::new(
            left,
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let right = canvas.scene.next_id();
        canvas.scene.add_rect(Rectangle::new(
            right,
            Vec2::new(20.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let group_id = canvas.scene.group(&[left, right]).unwrap();

        // 可见目标的左边缘在 x=90。指针位移 59 会把组的右边缘放到 84，刚好在 6px 阈值内。
        let external = canvas.scene.next_id();
        canvas.scene.add_rect(Rectangle::new(
            external,
            Vec2::new(100.0, 40.0),
            Vec2::new(20.0, 10.0),
        ));

        // 隐藏组里有一条更近的边。祖先不可见时不能抢吸附。
        let hidden_child = canvas.scene.next_id();
        canvas.scene.add_rect(Rectangle::new(
            hidden_child,
            Vec2::new(90.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let hidden_decoy = canvas.scene.next_id();
        canvas.scene.add_rect(Rectangle::new(
            hidden_decoy,
            Vec2::new(110.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let hidden_group = canvas.scene.group(&[hidden_child, hidden_decoy]).unwrap();
        assert!(canvas.scene.set_visible(hidden_group, false));

        canvas
            .selection
            .select_element(&mut canvas.scene, group_id, false);
        canvas.start_drag(1, Vec2::ZERO);

        let origin = canvas.drag_state.origin_bounds.expect("leaf union");
        assert_eq!(origin.x, -5.0);
        assert_eq!(origin.width, 30.0);
        assert!(canvas
            .snap_targets()
            .iter()
            .all(|rect| (rect.x - 90.0).abs() < 1e-3 && (rect.width - 20.0).abs() < 1e-3));
        assert_eq!(canvas.snap_targets().len(), 1);

        canvas.drag_selected_to_pointer(Vec2::new(59.0, 0.0));

        assert_eq!(translation(&canvas.scene, left), Vec2::new(65.0, 0.0));
        assert_eq!(translation(&canvas.scene, right), Vec2::new(85.0, 0.0));
        assert_eq!(translation(&canvas.scene, group_id), Vec2::new(75.0, 0.0));
        assert_eq!(canvas.drag_state.total_delta, Vec2::new(65.0, 0.0));
        assert!(canvas
            .drag_state
            .guides
            .iter()
            .any(|guide| (guide.position - 90.0).abs() < 1e-3));
    }

    #[test]
    fn dragging_a_group_moves_rect_and_text_and_json_keeps_parent() {
        let mut canvas = blank_canvas();
        let rect_id = canvas.scene.next_id();
        canvas.scene.add_rect(Rectangle::new(
            rect_id,
            Vec2::new(0.0, 0.0),
            Vec2::new(20.0, 10.0),
        ));
        let text_id = canvas.scene.next_id();
        let mut text = Text::new(text_id, Vec2::new(40.0, 0.0), "Hi");
        text.commit_raster(text.revision, 0, "glyph".into(), Vec2::new(16.0, 8.0))
            .unwrap();
        canvas.scene.add_text(text);
        let group_id = canvas.scene.group(&[rect_id, text_id]).unwrap();

        let mut cache = VertexCache::new();
        let first = cache.sync(&mut canvas.scene);
        assert!(first.full_upload);
        let before = cache.textured().to_vec();
        assert_eq!(before.len(), 6);

        canvas
            .selection
            .select_element(&mut canvas.scene, group_id, false);
        canvas.start_drag(1, Vec2::ZERO);
        canvas.drag_selected_to_pointer(Vec2::new(15.0, 4.0));

        assert_eq!(translation(&canvas.scene, rect_id), Vec2::new(15.0, 4.0));
        assert_eq!(translation(&canvas.scene, text_id), Vec2::new(55.0, 4.0));
        assert!(canvas.scene.get(text_id).unwrap().as_element().is_dirty());

        let sync = cache.sync(&mut canvas.scene);
        assert!(sync.regenerated >= 1);
        let expected = match canvas.scene.get(text_id).unwrap() {
            AnyElement::Text(text) => text.generate_vertices(),
            _ => panic!("text"),
        };
        assert_eq!(cache.textured(), expected.as_slice());
        assert_ne!(cache.textured(), before.as_slice());
        assert!(!canvas.scene.get(text_id).unwrap().as_element().is_dirty());

        let json = canvas.scene.to_json().unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"type\":\"group\""));
        assert!(!json.contains("texture_id"));
        assert!(!json.contains("editing"));
        assert!(!json.contains("raster_lod"));
        let restored = Scene::from_json(&json).unwrap();
        assert_eq!(restored.parent_of(text_id), Some(group_id));
        assert_eq!(restored.parent_of(rect_id), Some(group_id));
        assert_eq!(translation(&restored, text_id), Vec2::new(55.0, 4.0));
        assert_eq!(translation(&restored, rect_id), Vec2::new(15.0, 4.0));
        match restored.get(text_id).unwrap() {
            AnyElement::Text(text) => {
                assert!(text.texture_id.is_none());
                assert!(!text.editing);
                assert_eq!(text.props.parent, Some(group_id));
            }
            _ => panic!("text"),
        }
    }

    /// 方向键 API 走当前选择和撤销栈。空选、锁定和未知方向不入栈。
    #[test]
    fn nudge_selection_moves_current_selection_and_undoes() {
        let mut canvas = blank_canvas();
        canvas.scene.add_rect(Rectangle::new(
            1,
            Vec2::new(10.0, 20.0),
            Vec2::new(4.0, 4.0),
        ));
        canvas.scene.add_rect(Rectangle::new(
            2,
            Vec2::new(40.0, 20.0),
            Vec2::new(4.0, 4.0),
        ));
        assert!(canvas.select_element(1, false));
        assert!(canvas.select_element(2, true));

        assert!(!canvas.nudge_selection("zoom", false));
        assert_eq!(canvas.get_undo_count(), 0);

        assert!(canvas.nudge_selection("right", false));
        assert_eq!(translation(&canvas.scene, 1), Vec2::new(11.0, 20.0));
        assert_eq!(translation(&canvas.scene, 2), Vec2::new(41.0, 20.0));
        assert_eq!(canvas.get_undo_count(), 1);

        assert!(canvas.nudge_selection("ArrowUp", true));
        assert_eq!(translation(&canvas.scene, 1), Vec2::new(11.0, 10.0));
        assert_eq!(translation(&canvas.scene, 2), Vec2::new(41.0, 10.0));
        assert_eq!(canvas.get_undo_count(), 2);

        assert!(canvas.undo());
        assert_eq!(translation(&canvas.scene, 1), Vec2::new(11.0, 20.0));
        assert!(canvas.redo());
        assert_eq!(translation(&canvas.scene, 1), Vec2::new(11.0, 10.0));
        canvas.undo();

        canvas.scene.set_locked(1, true);
        canvas.scene.set_locked(2, true);
        let undo_count = canvas.get_undo_count();
        assert!(!canvas.nudge_selection("left", true));
        assert_eq!(translation(&canvas.scene, 1), Vec2::new(11.0, 20.0));
        assert_eq!(canvas.get_undo_count(), undo_count);

        canvas.clear_selection();
        assert!(!canvas.nudge_selection("down", false));
        assert_eq!(canvas.get_undo_count(), undo_count);
    }

    /// 翻转和 90° 旋转走当前选择。空选和未知参数不入栈，撤销恢复变换。
    #[test]
    fn flip_and_rotate_selection_round_trip() {
        let mut canvas = blank_canvas();
        canvas.scene.add_rect(Rectangle::new(
            1,
            Vec2::new(10.0, 20.0),
            Vec2::new(8.0, 4.0),
        ));
        canvas.scene.add_rect(Rectangle::new(
            2,
            Vec2::new(50.0, 20.0),
            Vec2::new(8.0, 4.0),
        ));
        assert!(canvas.select_element(1, false));

        assert!(!canvas.flip_selection("spin"));
        assert!(!canvas.rotate_selection_quarter("180"));
        assert_eq!(canvas.get_undo_count(), 0);

        assert!(canvas.flip_selection("horizontal"));
        assert!(translation(&canvas.scene, 1).x.abs() - 10.0 < 1e-3);
        assert!(
            canvas
                .scene
                .get(1)
                .unwrap()
                .as_element()
                .props()
                .transform
                .scale
                .x
                < 0.0
        );
        assert_eq!(translation(&canvas.scene, 2), Vec2::new(50.0, 20.0));
        assert_eq!(canvas.get_undo_count(), 1);
        assert!(canvas.undo());
        assert!(
            canvas
                .scene
                .get(1)
                .unwrap()
                .as_element()
                .props()
                .transform
                .scale
                .x
                > 0.0
        );

        assert!(canvas.rotate_selection_quarter("ccw"));
        let rotation = canvas
            .scene
            .get(1)
            .unwrap()
            .as_element()
            .props()
            .transform
            .rotation;
        assert!((rotation + std::f32::consts::FRAC_PI_2).abs() < 1e-3);
        assert!(canvas.undo());
        assert!(canvas.redo());
        assert!(canvas.undo());

        canvas.clear_selection();
        assert!(!canvas.flip_selection("vertical"));
        assert!(!canvas.rotate_selection_quarter("cw"));
        assert_eq!(canvas.get_undo_count(), 0);
    }

    /// 对齐和分布走当前选择。空选、单选和未知参数不入栈，撤销恢复平移。
    #[test]
    fn align_and_distribute_selection_round_trip() {
        let mut canvas = blank_canvas();
        canvas.scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        canvas.scene.add_rect(Rectangle::new(
            2,
            Vec2::new(40.0, 20.0),
            Vec2::new(20.0, 10.0),
        ));
        canvas.scene.add_rect(Rectangle::new(
            3,
            Vec2::new(90.0, -10.0),
            Vec2::new(10.0, 40.0),
        ));

        assert!(!canvas.align_selection("left"));
        assert!(!canvas.distribute_selection("horizontal"));
        assert_eq!(canvas.get_undo_count(), 0);

        assert!(canvas.select_element(1, false));
        assert!(!canvas.align_selection("center"));
        assert!(!canvas.distribute_selection("vertical"));
        assert!(canvas.select_element(2, true));
        assert!(canvas.select_element(3, true));
        assert!(!canvas.align_selection("nope"));
        assert!(!canvas.distribute_selection("both"));

        assert!(canvas.align_selection("left"));
        let left = |canvas: &InfiniteCanvas, id| {
            canvas
                .scene
                .get(id)
                .unwrap()
                .as_element()
                .world_bounds()
                .left()
        };
        assert!((left(&canvas, 1) - left(&canvas, 2)).abs() < 1e-3);
        assert!((left(&canvas, 2) - left(&canvas, 3)).abs() < 1e-3);
        assert_eq!(canvas.get_undo_count(), 1);
        assert!(canvas.undo());
        assert!((translation(&canvas.scene, 2).x - 40.0).abs() < 1e-3);
        assert!(canvas.redo());
        assert!(canvas.undo());

        assert!(canvas.distribute_selection("vertical"));
        let mut bands = [1u64, 2, 3].map(|id| {
            let bounds = canvas.scene.get(id).unwrap().as_element().world_bounds();
            (bounds.top(), bounds.bottom())
        });
        bands.sort_by(|a, b| a.0.total_cmp(&b.0));
        let gap_ab = bands[1].0 - bands[0].1;
        let gap_bc = bands[2].0 - bands[1].1;
        assert!((gap_ab - gap_bc).abs() < 1e-3);
        assert!((translation(&canvas.scene, 1).x).abs() < 1e-3);
        assert!(canvas.undo());
        assert!((translation(&canvas.scene, 1).y).abs() < 1e-3);
        assert!((translation(&canvas.scene, 3).y + 10.0).abs() < 1e-3);
        assert_eq!(canvas.get_undo_count(), 0);
    }

    /// `reorder_selection` 走当前选择。未知动作、空选不入栈；置顶后撤销和重做恢复兄弟顺序。
    #[test]
    fn reorder_selection_uses_current_selection_and_undoes() {
        let mut canvas = blank_canvas();
        canvas
            .scene
            .add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(8.0, 8.0)));
        canvas
            .scene
            .add_rect(Rectangle::new(2, Vec2::new(20.0, 0.0), Vec2::new(8.0, 8.0)));
        canvas
            .scene
            .add_rect(Rectangle::new(3, Vec2::new(40.0, 0.0), Vec2::new(8.0, 8.0)));
        assert!(canvas.select_element(1, false));
        assert!(!canvas.reorder_selection("nope"));
        assert_eq!(canvas.get_undo_count(), 0);
        assert!(canvas.reorder_selection("front"));
        assert_eq!(canvas.scene.sibling_order(None), vec![2, 3, 1]);
        assert_eq!(canvas.get_undo_count(), 1);
        assert!(canvas.undo());
        assert_eq!(canvas.scene.sibling_order(None), vec![1, 2, 3]);
        assert!(canvas.redo());
        assert_eq!(canvas.scene.sibling_order(None), vec![2, 3, 1]);
        canvas.clear_selection();
        assert!(!canvas.reorder_selection("back"));
        assert_eq!(canvas.get_undo_count(), 1);
    }

    /// 锁定和隐藏快捷键与图层面板按钮走同一条命令。空选不入栈，撤销恢复标志。
    #[test]
    fn toggle_lock_and_hide_share_the_layer_commands() {
        let mut canvas = blank_canvas();
        canvas.scene.add_rect(Rectangle::new(
            1,
            Vec2::new(10.0, 10.0),
            Vec2::new(20.0, 20.0),
        ));
        canvas.scene.add_rect(Rectangle::new(
            2,
            Vec2::new(40.0, 10.0),
            Vec2::new(12.0, 12.0),
        ));
        let structure = canvas.get_structure_epoch();
        assert!(!canvas.toggle_selection_locked());
        assert!(!canvas.toggle_selection_visible());
        assert_eq!(canvas.get_undo_count(), 0);

        assert!(canvas.select_element(1, false));
        assert!(canvas.select_element(2, true));
        let flags = canvas.get_flags_epoch();
        assert!(canvas.toggle_selection_locked());
        assert!(canvas.selection.is_selected(1));
        assert!(canvas.selection.is_selected(2));
        assert!(canvas.scene.is_effectively_locked(1));
        assert!(canvas.scene.is_effectively_locked(2));
        assert!(canvas.get_flags_epoch() > flags);
        assert_eq!(canvas.get_structure_epoch(), structure);
        assert_eq!(canvas.get_undo_count(), 1);
        let undo_count = canvas.get_undo_count();
        assert!(!canvas.nudge_selection("right", false));
        canvas.move_selection(5.0, 0.0);
        assert_eq!(translation(&canvas.scene, 1), Vec2::new(10.0, 10.0));
        assert_eq!(canvas.get_undo_count(), undo_count);
        assert!(hit_test_point(
            &canvas.scene,
            Vec2::new(10.0, 10.0),
            &HitTestOptions::default(),
        )
        .is_empty());

        assert!(canvas.toggle_selection_locked());
        assert!(!canvas.scene.is_effectively_locked(1));
        assert!(canvas.undo());
        assert!(canvas.scene.is_effectively_locked(1));
        assert!(canvas.undo());
        assert!(!canvas.scene.is_effectively_locked(1));
        assert!(canvas.redo());
        assert!(canvas.scene.is_effectively_locked(1));
        canvas.undo();

        assert!(canvas.toggle_selection_visible());
        assert!(!canvas.scene.is_effectively_visible(1));
        assert!(!canvas.scene.get(2).unwrap().as_element().props().visible);
        assert!(hit_test_point(
            &canvas.scene,
            Vec2::new(10.0, 10.0),
            &HitTestOptions::default(),
        )
        .is_empty());
        let hits = canvas.selection.select_in_rect(
            &mut canvas.scene,
            Rect::new(-10.0, -10.0, 80.0, 40.0),
            false,
        );
        assert!(!hits.contains(&1));
        assert!(!hits.contains(&2));
        assert!(canvas.undo());
        assert!(canvas.scene.is_effectively_visible(1));
        assert!(!hit_test_point(
            &canvas.scene,
            Vec2::new(10.0, 10.0),
            &HitTestOptions::default(),
        )
        .is_empty());

        canvas.selection.clear(&mut canvas.scene);
        assert!(canvas.select_element(1, false));
        assert!(canvas.set_element_visible(1, false));
        assert_eq!(canvas.get_undo_count(), 1);
        assert!(canvas.set_element_locked(1, true));
        assert!(canvas.selection.is_selected(1));
        assert!(canvas.scene.is_effectively_locked(1));
        assert!(canvas.undo());
        assert!(!canvas.scene.is_effectively_locked(1));
        assert!(canvas.undo());
        assert!(canvas.scene.is_effectively_visible(1));
        assert!(!canvas.set_element_visible(99, false));
        assert!(!canvas.set_element_locked(99, true));
    }

    #[test]
    fn select_all_selects_the_group_and_not_its_children() {
        let mut canvas = blank_canvas();
        let left = canvas.scene.next_id();
        canvas.scene.add_rect(Rectangle::new(
            left,
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let right = canvas.scene.next_id();
        canvas.scene.add_rect(Rectangle::new(
            right,
            Vec2::new(30.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let group_id = canvas.scene.group(&[left, right]).unwrap();
        let hidden = canvas.scene.next_id();
        let mut hidden_rect = Rectangle::new(hidden, Vec2::new(80.0, 0.0), Vec2::new(10.0, 10.0));
        hidden_rect.props.visible = false;
        canvas.scene.add_rect(hidden_rect);
        let locked = canvas.scene.next_id();
        canvas.scene.add_rect(Rectangle::new(
            locked,
            Vec2::new(120.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        canvas.scene.set_locked(locked, true);

        assert_eq!(canvas.select_all(), 1);
        assert!(canvas.selection.is_selected(group_id));
        assert!(!canvas.selection.is_selected(left));
        assert!(!canvas.selection.is_selected(right));
        assert!(!canvas.selection.is_selected(hidden));
        assert!(!canvas.selection.is_selected(locked));
    }

    #[test]
    fn delete_selection_removes_group_subtree_and_clears_selection() {
        let mut canvas = blank_canvas();
        let left = canvas.scene.next_id();
        canvas.scene.add_rect(Rectangle::new(
            left,
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let right = canvas.scene.next_id();
        canvas.scene.add_rect(Rectangle::new(
            right,
            Vec2::new(30.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let text_id = canvas.scene.next_id();
        let mut text = Text::new(text_id, Vec2::new(70.0, 0.0), "Hi");
        text.commit_raster(text.revision, 0, "glyph".into(), Vec2::new(16.0, 8.0))
            .unwrap();
        canvas.scene.add_text(text);
        let group_id = canvas.scene.group(&[left, right]).unwrap();
        canvas
            .selection
            .select_element(&mut canvas.scene, group_id, false);
        canvas
            .selection
            .select_element(&mut canvas.scene, text_id, true);
        canvas
            .selection
            .select_element(&mut canvas.scene, left, true);

        // 没有 WebGPU 渲染器时，文本纹理释放是空操作；选择集仍必须清掉。
        assert_eq!(canvas.delete_selection(), 4);
        assert!(!canvas.scene.contains(group_id));
        assert!(!canvas.scene.contains(left));
        assert!(!canvas.scene.contains(right));
        assert!(!canvas.scene.contains(text_id));
        assert!(canvas.selection.is_empty());

        assert!(canvas.undo());
        assert_eq!(canvas.scene.children_of(group_id), vec![left, right]);
        assert!(canvas.scene.contains(text_id));
        assert!(canvas.selection.is_selected(group_id));
        assert!(canvas.selection.is_selected(text_id));
        assert!(canvas.selection.is_selected(left));
        assert!(!canvas.selection.is_selected(right));
    }

    /// 键盘步进绕中心缩放，倍数和上下限与滚轮的 `zoom_at`、视口 min/max 相同。
    /// 回到 100% 用 `set_zoom(1)`，中心不动，也不进撤销栈。
    #[test]
    fn keyboard_zoom_uses_the_wheel_step_and_viewport_limits() {
        let mut canvas = blank_canvas();
        canvas.viewport.resize(Vec2::new(800.0, 600.0));
        canvas.set_camera_center(30.0, -10.0);
        let step = canvas.zoom_step();
        assert!((step - ZOOM_STEP).abs() < 1e-6);

        let mut wheeled = blank_canvas();
        wheeled.viewport.resize(Vec2::new(800.0, 600.0));
        wheeled.set_camera_center(30.0, -10.0);
        wheeled.zoom_at(400.0, 300.0, wheeled.zoom_step());

        let undo = canvas.get_undo_count();
        canvas.zoom_by(step);
        assert!((canvas.get_zoom() - wheeled.get_zoom()).abs() < 1e-5);
        assert!((canvas.get_zoom() - step).abs() < 1e-5);
        assert_eq!(canvas.get_viewport_center_x(), 30.0);
        assert_eq!(canvas.get_viewport_center_y(), -10.0);

        canvas.zoom_by(1.0 / step);
        assert!((canvas.get_zoom() - 1.0).abs() < 1e-4);
        assert_eq!(canvas.get_viewport_center_x(), 30.0);

        canvas.set_zoom(1.0);
        assert!((canvas.get_zoom() - 1.0).abs() < 1e-5);
        assert_eq!(canvas.get_viewport_center_x(), 30.0);
        assert_eq!(canvas.get_viewport_center_y(), -10.0);

        let max_zoom = canvas.viewport.max_zoom();
        let min_zoom = canvas.viewport.min_zoom();
        canvas.zoom_by(1.0e6);
        assert!((canvas.get_zoom() - max_zoom).abs() < 1e-4);
        canvas.zoom_by(1.0e-6);
        assert!((canvas.get_zoom() - min_zoom).abs() < 1e-4);
        assert_eq!(canvas.get_viewport_center_x(), 30.0);
        assert_eq!(canvas.get_viewport_center_y(), -10.0);

        canvas.set_zoom(1.0);
        canvas.zoom_by(f32::NAN);
        canvas.zoom_by(0.0);
        canvas.set_zoom(f32::NAN);
        assert!((canvas.get_zoom() - 1.0).abs() < 1e-5);
        assert_eq!(canvas.get_viewport_center_x(), 30.0);
        assert_eq!(canvas.get_undo_count(), undo);
    }

    /// 缩放到选中 / 适应全部只改相机。空选不动，空场景回到默认相机。
    /// 之后平移、缩放、设置相机中心和框选仍然可用，几何版本和撤销栈不变。
    #[test]
    fn zoom_commands_frame_the_camera_and_leave_scene_tools_alone() {
        let mut canvas = blank_canvas();
        canvas.viewport.resize(Vec2::new(800.0, 600.0));
        canvas.pan(30.0, -10.0);
        assert!(!canvas.zoom_to_selection());
        assert_eq!(canvas.get_viewport_center_x(), 30.0);
        assert_eq!(canvas.get_viewport_center_y(), -10.0);

        assert!(!canvas.zoom_to_fit());
        assert_eq!(canvas.get_viewport_center_x(), 0.0);
        assert_eq!(canvas.get_zoom(), 1.0);
        assert!(!canvas.fit_scene());
        assert_eq!(canvas.get_zoom(), 1.0);

        canvas.scene.add_rect(Rectangle::new(
            1,
            Vec2::new(400.0, 200.0),
            Vec2::new(200.0, 100.0),
        ));
        let placed = translation(&canvas.scene, 1);
        let bounds = minimap::scene_bounds(&canvas.scene);

        assert!(!canvas.zoom_to_selection());
        assert_eq!(canvas.get_zoom(), 1.0);

        assert!(canvas.select_element(1, false));
        let epoch = canvas.scene.geometry_epoch();
        assert!(canvas.zoom_to_selection());
        assert!((canvas.get_viewport_center_x() - 400.0).abs() < 1e-3);
        assert!((canvas.get_viewport_center_y() - 200.0).abs() < 1e-3);
        assert!((canvas.get_zoom() - 3.2).abs() < 1e-3);
        assert_eq!(translation(&canvas.scene, 1), placed);
        assert_eq!(canvas.scene.geometry_epoch(), epoch);
        assert_eq!(canvas.get_undo_count(), 0);
        assert!(!canvas.can_undo());

        assert!(canvas.zoom_to_fit());
        assert!((canvas.get_zoom() - 3.2).abs() < 1e-3);
        let center = canvas.viewport.center();
        let zoom = canvas.get_zoom();
        assert!(canvas.fit_scene());
        assert_eq!(canvas.viewport.center(), center);
        assert_eq!(canvas.get_zoom(), zoom);
        assert_eq!(minimap::scene_bounds(&canvas.scene), bounds);
        assert_eq!(canvas.scene.geometry_epoch(), epoch);

        canvas.set_camera_center(10.0, 20.0);
        assert_eq!(canvas.get_viewport_center_x(), 10.0);
        assert_eq!(canvas.get_viewport_center_y(), 20.0);
        canvas.zoom_at(400.0, 300.0, 1.1);
        assert!((canvas.get_zoom() - zoom * 1.1).abs() < 1e-3);
        let center = canvas.viewport.center();
        canvas.pan(15.0, -4.0);
        assert!((canvas.get_viewport_center_x() - center.x - 15.0).abs() < 1e-3);
        assert!((canvas.get_viewport_center_y() - center.y + 4.0).abs() < 1e-3);

        canvas.clear_selection();
        let hits = canvas.selection.select_in_rect(
            &mut canvas.scene,
            Rect::new(300.0, 140.0, 200.0, 120.0),
            false,
        );
        assert_eq!(hits, vec![1]);
        assert_eq!(translation(&canvas.scene, 1), placed);
    }

    fn pointer_at(canvas: &InfiniteCanvas, screen: Vec2) -> PointerEvent {
        PointerEvent {
            pointer_id: 1,
            screen_position: screen,
            world_position: canvas.viewport.screen_to_world(screen),
            screen_delta: Vec2::ZERO,
            world_delta: Vec2::ZERO,
            button: Some(PointerButton::Primary),
            buttons_mask: 1,
            modifiers: infinitecanvas_engine::Modifiers::new(false, false, false, false),
        }
    }

    fn rect_fill(scene: &Scene, id: ElementId) -> Option<Color> {
        match infinitecanvas_engine::capture_style(scene, id) {
            Some(
                infinitecanvas_engine::StyleSnapshot::Rectangle { style, .. }
                | infinitecanvas_engine::StyleSnapshot::Shape(style),
            ) => style.fill,
            other => panic!("shape {id}, got {other:?}"),
        }
    }

    #[test]
    fn dragging_from_the_top_ruler_creates_a_horizontal_guide() {
        let mut canvas = blank_canvas();
        let down = pointer_at(&canvas, Vec2::new(200.0, 8.0));
        canvas.handle_pointer_down(down);
        assert!(canvas.scene.guides().is_empty());
        let moved = pointer_at(&canvas, Vec2::new(200.0, 140.0));
        let expected_y = moved.world_position.y;
        canvas.handle_pointer_move(moved);
        assert_eq!(canvas.scene.guides().len(), 1);
        let up = pointer_at(&canvas, Vec2::new(200.0, 140.0));
        canvas.handle_pointer_up(up);
        let guide = canvas.scene.guides().iter().next().unwrap();
        assert_eq!(guide.axis, GuideAxis::Horizontal);
        assert!((guide.position - expected_y).abs() < 1e-3);
        assert_eq!(canvas.get_undo_count(), 1);
        assert!(canvas.undo());
        assert!(canvas.scene.guides().is_empty());
        assert!(canvas.redo());
        assert_eq!(canvas.scene.guides().len(), 1);
    }

    #[test]
    fn dropping_a_new_guide_back_on_the_ruler_discards_it() {
        let mut canvas = blank_canvas();
        canvas.handle_pointer_down(pointer_at(&canvas, Vec2::new(8.0, 200.0)));
        canvas.handle_pointer_move(pointer_at(&canvas, Vec2::new(180.0, 200.0)));
        assert_eq!(canvas.scene.guides().len(), 1);
        canvas.handle_pointer_up(pointer_at(&canvas, Vec2::new(8.0, 200.0)));
        assert!(canvas.scene.guides().is_empty());
        assert_eq!(canvas.get_undo_count(), 0);
    }

    #[test]
    fn dragging_snaps_the_selection_edge_to_a_manual_guide() {
        let mut canvas = blank_canvas();
        canvas
            .scene
            .add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(40.0, 20.0)));
        canvas.scene.add_guide(GuideAxis::Vertical, 50.0).unwrap();
        canvas.selection.select_element(&mut canvas.scene, 1, false);
        canvas.start_drag(1, Vec2::ZERO);
        canvas.drag_selected_to_pointer(Vec2::new(28.0, 0.0));
        let right = canvas
            .scene
            .get(1)
            .unwrap()
            .as_element()
            .world_bounds()
            .right();
        assert!((right - 50.0).abs() < 1e-3);
        assert!(
            canvas
                .drag_state
                .guides
                .iter()
                .any(|guide| guide.axis == GuideAxis::Vertical
                    && (guide.position - 50.0).abs() < 1e-3)
        );

        canvas.finish_drag();
        canvas.set_rulers_visible(false);
        canvas.selection.select_element(&mut canvas.scene, 1, false);
        let before = canvas
            .scene
            .get(1)
            .unwrap()
            .as_element()
            .props()
            .transform
            .translation;
        canvas.start_drag(1, Vec2::ZERO);
        canvas.drag_selected_to_pointer(Vec2::new(4.0, 0.0));
        let after = canvas
            .scene
            .get(1)
            .unwrap()
            .as_element()
            .props()
            .transform
            .translation;
        assert!((after.x - before.x - 4.0).abs() < 1e-3);
    }

    #[test]
    fn resizing_snaps_the_element_edge_to_a_manual_guide() {
        let mut canvas = blank_canvas();
        canvas
            .scene
            .add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(40.0, 20.0)));
        canvas.selection.select_element(&mut canvas.scene, 1, false);
        canvas.scene.add_guide(GuideAxis::Vertical, 24.0).unwrap();
        let handle = canvas.viewport.world_to_screen(Vec2::new(23.0, 0.0));
        let down = pointer_at(&canvas, handle);
        assert!(canvas.try_begin_handle_gesture(&down));
        canvas.update_handle_gesture(&down);
        let right = canvas
            .scene
            .get(1)
            .unwrap()
            .as_element()
            .world_bounds()
            .right();
        assert!(
            (right - 24.0).abs() < 0.05,
            "right edge {right} should land on the guide"
        );
    }

    #[test]
    fn eyedropper_click_copies_style_without_changing_the_selection() {
        let mut canvas = blank_canvas();
        let source = canvas.scene.next_id();
        canvas.scene.add_rect(
            Rectangle::new(source, Vec2::ZERO, Vec2::new(40.0, 40.0))
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        let target = canvas.scene.next_id();
        canvas.scene.add_rect(
            Rectangle::new(target, Vec2::new(120.0, 0.0), Vec2::new(40.0, 40.0))
                .with_style(ElementStyle::fill_only(Color::BLUE)),
        );
        assert!(canvas
            .selection
            .select_element(&mut canvas.scene, target, false));
        assert!(canvas
            .selection
            .select_element(&mut canvas.scene, source, true));
        assert!(canvas.set_tool("rectangle".into()));
        assert!(canvas.set_tool("eyedropper".into()));

        canvas.handle_pointer_down(pointer_at(
            &canvas,
            canvas.viewport.world_to_screen(Vec2::ZERO),
        ));
        assert_eq!(canvas.get_current_tool(), "eyedropper");
        assert!(canvas.selection.is_selected(source));
        assert!(canvas.selection.is_selected(target));
        canvas.finish_eyedropper_gesture(true);

        assert_eq!(canvas.get_current_tool(), "rectangle");
        let notice = canvas.eyedropper_notice.take().unwrap();
        assert!(notice.hit && notice.applied);
        assert_eq!(canvas.get_undo_count(), 1);
        assert_eq!(rect_fill(&canvas.scene, target), Some(Color::RED));
        assert_eq!(rect_fill(&canvas.scene, source), Some(Color::RED));
        assert!(canvas.undo());
        assert_eq!(rect_fill(&canvas.scene, target), Some(Color::BLUE));

        assert!(canvas.scene.set_locked(target, true));
        assert!(!canvas.apply_sampled_style_at(0.0, 0.0));
        assert!(canvas.scene.set_locked(target, false));
        assert!(canvas.scene.set_visible(target, false));
        assert!(!canvas.apply_sampled_style_at(0.0, 0.0));
        assert_eq!(canvas.get_undo_count(), 0);
        assert_eq!(rect_fill(&canvas.scene, target), Some(Color::BLUE));

        assert!(canvas.set_tool("eyedropper".into()));
        canvas.handle_pointer_down(pointer_at(
            &canvas,
            canvas.viewport.world_to_screen(Vec2::new(800.0, 800.0)),
        ));
        canvas.finish_eyedropper_gesture(true);
        assert_eq!(canvas.get_current_tool(), "eyedropper");
        let miss = canvas.eyedropper_notice.take().unwrap();
        assert!(!miss.hit);
        assert!(!miss.applied);
    }
}
