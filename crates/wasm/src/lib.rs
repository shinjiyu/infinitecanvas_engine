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
mod texture_manager;

use infinitecanvas_engine::{
    math::{Color, Vec2},
    AnyElement, Circle, ElementStyle, Image, Rectangle, Scene, Viewport,
};
use wasm_bindgen::prelude::*;
use renderer::WebGPURenderer;

const BUILD_SIGNATURE: &str = match option_env!("BUILD_SIGNATURE") {
    Some(sig) => sig,
    None => "dev-local",
};
const BUILD_VERSION: &str = env!("CARGO_PKG_VERSION");

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
    /// - `Err(JsValue)`: 渲染失败（渲染器未初始化）
    #[wasm_bindgen]
    pub fn render(&mut self) -> Result<(), JsValue> {
        if let Some(renderer) = &mut self.webgpu_renderer {
            renderer.render(&self.scene, &self.viewport)?;
            Ok(())
        } else {
            Err(JsValue::from_str("WebGPU renderer not initialized. Call init() first."))
        }
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
            log::info!("  - Sample pixel at (0,0): R={}, G={}, B={}, A={}", 
                data[0], data[1], data[2], data[3]);
        }
        
        let renderer = self
            .webgpu_renderer
            .as_mut()
            .ok_or_else(|| JsValue::from_str("WebGPU renderer not initialized. Call init() first."))?;

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
        let image = Image::new(id, Vec2::new(x, y), Vec2::new(width, height), texture_id.clone());
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
        if let Some(AnyElement::Image(image)) = self.scene.get_mut(image_id) {
            image.set_alpha(alpha);
            log::debug!("Set image {} alpha to {}", image_id, alpha);
            true
        } else {
            log::warn!("Image {} not found", image_id);
            false
        }
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
}

