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

use infinitecanvas_engine::{
    math::Vec2, AnyElement, Circle, Element, ElementStyle, Rectangle, Scene, Viewport,
};
use wasm_bindgen::prelude::*;
use canvas::CanvasContext;

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
    scene: Scene,
    viewport: Viewport,
    engine: infinitecanvas_engine::Renderer,
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
            scene: Scene::new(),
            viewport: Viewport::default(),
            engine: infinitecanvas_engine::Renderer::new(),
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
    #[wasm_bindgen]
    pub fn init(&mut self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
        log::info!("Initializing canvas context...");
        
        let canvas_ctx = CanvasContext::new(canvas)?;
        log::info!(
            "Canvas context created: {}x{} (DPR: {})", 
            canvas_ctx.width(), 
            canvas_ctx.height(),
            canvas_ctx.dpr()
        );
        
        // 更新视口尺寸
        self.viewport.resize(Vec2::new(canvas_ctx.width() as f32, canvas_ctx.height() as f32));
        
        self.canvas_ctx = Some(canvas_ctx);
        
        // 初始化测试场景
        self.init_test_scene();
        
        log::info!("InfiniteCanvas initialization complete");
        Ok(())
    }
    
    /// 初始化测试场景
    fn init_test_scene(&mut self) {
        let id1 = self.scene.next_id();
        let rect = Rectangle::new(id1, Vec2::new(100.0, 100.0), Vec2::new(200.0, 150.0))
            .with_style(ElementStyle::fill_only("#3b82f6"));
        self.scene.add_rect(rect);
        
        let id2 = self.scene.next_id();
        let rect2 = Rectangle::new(id2, Vec2::new(375.0, 125.0), Vec2::new(200.0, 150.0))
            .with_style(ElementStyle::stroke_only("#ef4444", 3.0));
        self.scene.add_rect(rect2);
        
        let id3 = self.scene.next_id();
        let mut circle_style = ElementStyle::fill_only("#10b981");
        circle_style.stroke = Some("#059669".to_string());
        circle_style.stroke_width = 2.0;
        let circle = Circle::new(id3, Vec2::new(450.0, 350.0), 50.0).with_style(circle_style);
        self.scene.add_circle(circle);
        
        let id4 = self.scene.next_id();
        let circle2 = Circle::new(id4, Vec2::new(200.0, 350.0), 60.0)
            .with_style(ElementStyle::stroke_only("#f59e0b", 3.0));
        self.scene.add_circle(circle2);
        
        log::info!("Test scene initialized with {} elements", self.scene.count());
    }
    
    /// 渲染一帧
    ///
    /// # 返回
    /// - `Ok(())`: 渲染成功
    /// - `Err(JsValue)`: 渲染失败
    #[wasm_bindgen]
    pub fn render(&self) -> Result<(), JsValue> {
        if let Some(ctx) = &self.canvas_ctx {
            // 清空画布
            ctx.clear();
            
            // 绘制背景网格
            self.render_grid(ctx);
            
            // 渲染场景
            self.render_scene(ctx);
        } else {
            return Err(JsValue::from_str("Canvas not initialized. Call init() first."));
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
            ctx.fill_rect(x, y, size.x, size.y, fill);
        }
        
        // 描边
        if let Some(stroke) = &rect.style.stroke {
            ctx.stroke_rect(x, y, size.x, size.y, stroke, rect.style.stroke_width);
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
        
        ctx.draw_circle(
            pos.x,
            pos.y,
            radius,
            circle.style.fill.as_deref(),
            circle.style.stroke.as_deref(),
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
}

