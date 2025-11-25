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

use wasm_bindgen::prelude::*;
use canvas::CanvasContext;

/// WASM 主接口，提供无限画布的所有功能
///
/// # 职责
/// - 初始化渲染引擎
/// - 管理 Canvas 上下文
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
        
        self.canvas_ctx = Some(canvas_ctx);
        
        log::info!("InfiniteCanvas initialization complete");
        Ok(())
    }
    
    /// 渲染一帧
    ///
    /// # 返回
    /// - `Ok(())`: 渲染成功
    /// - `Err(JsValue)`: 渲染失败
    #[wasm_bindgen]
    pub fn render(&self) -> Result<(), JsValue> {
        if let Some(ctx) = &self.canvas_ctx {
            ctx.clear();
            // 未来会调用 self.engine.render()
        } else {
            return Err(JsValue::from_str("Canvas not initialized. Call init() first."));
        }
        Ok(())
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

