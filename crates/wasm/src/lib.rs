//! InfiniteCanvas WASM - WebAssembly bindings

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct InfiniteCanvas {
    _engine: infinitecanvas_engine::Renderer,
}

#[wasm_bindgen]
impl InfiniteCanvas {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<InfiniteCanvas, JsValue> {
        console_log::init_with_level(log::Level::Debug).ok();
        log::info!("InfiniteCanvas initialized");
        
        Ok(InfiniteCanvas {
            _engine: infinitecanvas_engine::Renderer::new(),
        })
    }
}

