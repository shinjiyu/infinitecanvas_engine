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

mod canvas;
mod renderer;

use infinitecanvas_engine::{
    math::{Color, Vec2},
    AnyElement, Circle, Element, ElementStyle, Rectangle, Scene, Viewport,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use canvas::CanvasContext;
use renderer::WebGPURenderer;

/// WASM 主接口，提供无限画布的所有功能
///
/// # 职责
/// - 初始化渲染引擎
/// - 管理 Canvas 上下文
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
    canvas_ctx: Option<CanvasContext>,
    webgpu_renderer: Option<WebGPURenderer>,
    scene: Scene,
    viewport: Viewport,
    engine: infinitecanvas_engine::Renderer,
    use_webgpu: bool,
}

#[wasm_bindgen]
impl InfiniteCanvas {
    /// 创建新的画布实例
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
        
        Ok(InfiniteCanvas {
            canvas_ctx: None,
            webgpu_renderer: None,
            scene: Scene::new(),
            viewport: Viewport::default(),
            engine: infinitecanvas_engine::Renderer::new(),
            use_webgpu: true, // ✅ 升级到 wgpu 27 解决了兼容性问题
        })
    }
    
    /// 初始化画布
    ///
    /// # 参数
    /// - `canvas`: HTMLCanvasElement
    ///
    /// # 返回
    /// - `Ok(())`: 初始化成功
    /// - `Err(JsValue)`: Canvas 无效或初始化失败
    /// 初始化（异步）
    #[wasm_bindgen]
    pub async fn init(&mut self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
        log::info!("Initializing InfiniteCanvas...");
        
        let width = canvas.width();
        let height = canvas.height();
        
        // 更新视口尺寸
        self.viewport.resize(Vec2::new(width as f32, height as f32));
        
        // 尝试初始化 WebGPU
        if self.use_webgpu {
            match WebGPURenderer::new(canvas.clone(), width, height).await {
                Ok(renderer) => {
                    log::info!("WebGPU renderer initialized successfully");
                    self.webgpu_renderer = Some(renderer);
                }
                Err(e) => {
                    log::warn!("WebGPU initialization failed: {:?}, falling back to Canvas 2D", e);
                    self.use_webgpu = false;
                    let canvas_ctx = CanvasContext::new(canvas)?;
                    self.canvas_ctx = Some(canvas_ctx);
                }
            }
        } else {
            // 使用 Canvas 2D
            let canvas_ctx = CanvasContext::new(canvas)?;
            self.canvas_ctx = Some(canvas_ctx);
        }
        
        // 初始化测试场景
        self.init_test_scene();
        
        log::info!("InfiniteCanvas initialization complete (WebGPU: {})", self.use_webgpu);
        Ok(())
    }
    
    /// 初始化测试场景
    fn init_test_scene(&mut self) {
        let id1 = self.scene.next_id();
        let rect = Rectangle::new(id1, Vec2::new(100.0, 100.0), Vec2::new(200.0, 150.0))
            .with_style(ElementStyle::fill_only(Color::from_hex("#3b82f6").unwrap()));
        self.scene.add_rect(rect);
        
        let id2 = self.scene.next_id();
        let rect2 = Rectangle::new(id2, Vec2::new(375.0, 125.0), Vec2::new(200.0, 150.0))
            .with_style(ElementStyle::stroke_only(Color::from_hex("#ef4444").unwrap(), 3.0));
        self.scene.add_rect(rect2);
        
        let id3 = self.scene.next_id();
        let mut circle_style = ElementStyle::fill_only(Color::from_hex("#10b981").unwrap());
        circle_style.stroke = Some(Color::from_hex("#059669").unwrap());
        circle_style.stroke_width = 2.0;
        let circle = Circle::new(id3, Vec2::new(450.0, 350.0), 50.0).with_style(circle_style);
        self.scene.add_circle(circle);
        
        let id4 = self.scene.next_id();
        let circle2 = Circle::new(id4, Vec2::new(200.0, 350.0), 60.0)
            .with_style(ElementStyle::stroke_only(Color::from_hex("#f59e0b").unwrap(), 3.0));
        self.scene.add_circle(circle2);
        
        log::info!("Test scene initialized with {} elements", self.scene.count());
    }
    
    /// 渲染一帧
    ///
    /// # 返回
    /// - `Ok(())`: 渲染成功
    /// - `Err(JsValue)`: 渲染失败
    #[wasm_bindgen]
    pub fn render(&mut self) -> Result<(), JsValue> {
        if self.use_webgpu {
            // 使用 WebGPU 渲染
            if let Some(renderer) = &mut self.webgpu_renderer {
                renderer.render(&self.scene, &self.viewport)?;
            } else {
                return Err(JsValue::from_str("WebGPU renderer not initialized"));
            }
        } else {
            // 使用 Canvas 2D 渲染
            if let Some(ctx) = &self.canvas_ctx {
                ctx.clear();
                self.render_grid(ctx);
                self.render_scene(ctx);
            } else {
                return Err(JsValue::from_str("Canvas not initialized. Call init() first."));
            }
        }
        Ok(())
    }

    /// 渲染背景网格
    fn render_grid(&self, ctx: &CanvasContext) {
        let width = ctx.width() as f32;
        let height = ctx.height() as f32;
        let grid_size = 20.0; // 网格大小
        let grid_color = "#e5e7eb"; // 浅灰色

        // 绘制垂直线
        let mut x = 0.0;
        while x <= width {
            ctx.draw_line(x, 0.0, x, height, grid_color, 1.0);
            x += grid_size;
        }

        // 绘制水平线
        let mut y = 0.0;
        while y <= height {
            ctx.draw_line(0.0, y, width, y, grid_color, 1.0);
            y += grid_size;
        }
    }

    /// 渲染场景
    fn render_scene(&self, ctx: &CanvasContext) {
        // 获取所有元素（按 z-index 排序）
        let elements = self.scene.elements_sorted();
        
        for element in elements {
            // 视口裁剪（未来优化）
            // if !self.viewport.is_visible(&element.as_element().world_bounds()) {
            //     continue;
            // }
            
            // 渲染元素
            self.render_element(ctx, element);
        }
    }
    
    /// 渲染单个元素
    fn render_element(&self, ctx: &CanvasContext, element: &AnyElement) {
        match element {
            AnyElement::Rectangle(rect) => self.render_rectangle(ctx, rect),
            AnyElement::Circle(circle) => self.render_circle(ctx, circle),
        }
    }
    
    /// 渲染矩形
    fn render_rectangle(&self, ctx: &CanvasContext, rect: &Rectangle) {
        let props = rect.props();
        if !props.visible {
            return;
        }
        
        let pos = props.transform.translation;
        let size = rect.size();
        
        // 计算左上角坐标
        let x = pos.x - size.x / 2.0;
        let y = pos.y - size.y / 2.0;
        
        // 填充
        if let Some(fill) = &rect.style.fill {
            ctx.fill_rect(x, y, size.x, size.y, &fill.to_hex());
        }
        
        // 描边
        if let Some(stroke) = &rect.style.stroke {
            ctx.stroke_rect(x, y, size.x, size.y, &stroke.to_hex(), rect.style.stroke_width);
        }
    }
    
    /// 渲染圆形
    fn render_circle(&self, ctx: &CanvasContext, circle: &Circle) {
        let props = circle.props();
        if !props.visible {
            return;
        }
        
        let pos = props.transform.translation;
        let radius = circle.radius();
        
        let fill_hex = circle.style.fill.map(|c| c.to_hex());
        let stroke_hex = circle.style.stroke.map(|c| c.to_hex());
        
        ctx.draw_circle(
            pos.x,
            pos.y,
            radius,
            fill_hex.as_deref(),
            stroke_hex.as_deref(),
            circle.style.stroke_width,
        );
    }
    
    /// 获取 Canvas 宽度
    #[wasm_bindgen]
    pub fn width(&self) -> u32 {
        self.canvas_ctx.as_ref().map(|c| c.width()).unwrap_or(0)
    }
    
    /// 获取 Canvas 高度
    #[wasm_bindgen]
    pub fn height(&self) -> u32 {
        self.canvas_ctx.as_ref().map(|c| c.height()).unwrap_or(0)
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

    /// 删除元素
    ///
    /// # 参数
    /// - `id`: 元素 ID
    ///
    /// # 返回
    /// 是否成功删除
    #[wasm_bindgen]
    pub fn remove_element(&mut self, id: u64) -> bool {
        let removed = self.scene.remove(id).is_some();
        if removed {
            log::info!("Removed element {}", id);
        }
        removed
    }

    /// 清空场景
    #[wasm_bindgen]
    pub fn clear_scene(&mut self) {
        self.scene.clear();
        log::info!("Scene cleared");
    }

    /// 获取元素数量
    #[wasm_bindgen]
    pub fn get_element_count(&self) -> usize {
        self.scene.count()
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
}

