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

use wasm_bindgen::prelude::*;

/// WASM 主接口，提供无限画布的所有功能
///
/// # 职责
/// - 初始化 WebGPU 渲染引擎
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
    _engine: infinitecanvas_engine::Renderer,
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
        // 初始化日志系统
        console_log::init_with_level(log::Level::Debug).ok();
        log::info!("InfiniteCanvas WASM initialized");
        
        Ok(InfiniteCanvas {
            _engine: infinitecanvas_engine::Renderer::new(),
        })
    }
    
    /// 渲染一帧（用于测试）
    pub fn render(&self) -> Result<(), JsValue> {
        log::info!("Rendering frame");
        Ok(())
    }
}

