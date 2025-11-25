///! Canvas 2D Context 封装
///!
///! 提供对 HTMLCanvasElement 和 Canvas 2D API 的 Rust 封装

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

/// Canvas 上下文封装
///
/// # 职责
/// - 管理 HTMLCanvasElement
/// - 管理 CanvasRenderingContext2d
/// - 处理 Canvas 尺寸和 DPI
/// - 提供绘制 API 封装
pub struct CanvasContext {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    width: u32,
    height: u32,
    dpr: f64,
}

impl CanvasContext {
    /// 创建新的 Canvas 上下文
    ///
    /// # 参数
    /// - `canvas`: HTMLCanvasElement
    ///
    /// # 返回
    /// - `Ok(CanvasContext)`: 创建成功
    /// - `Err(JsValue)`: Canvas 无效或获取 Context 失败
    pub fn new(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        // 获取 2D 渲染上下文
        let context = canvas
            .get_context("2d")
            .map_err(|e| JsValue::from_str(&format!("Failed to get context: {:?}", e)))?
            .ok_or_else(|| JsValue::from_str("Context is null"))?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|e| JsValue::from_str(&format!("Failed to cast to 2d context: {:?}", e)))?;

        // 获取设备像素比
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window"))?;
        let dpr = window.device_pixel_ratio();

        // 获取 Canvas 的 CSS 尺寸
        let rect = canvas.get_bounding_client_rect();
        let width = rect.width() as u32;
        let height = rect.height() as u32;

        // 设置物理像素尺寸（支持高 DPI）
        canvas.set_width((width as f64 * dpr) as u32);
        canvas.set_height((height as f64 * dpr) as u32);

        // 设置 CSS 尺寸
        let _ = canvas
            .style()
            .set_property("width", &format!("{}px", width));
        let _ = canvas
            .style()
            .set_property("height", &format!("{}px", height));

        // 缩放上下文以匹配 DPI
        context.scale(dpr, dpr).ok();

        log::debug!(
            "Canvas created: {}x{} (physical: {}x{}, dpr: {})",
            width,
            height,
            (width as f64 * dpr) as u32,
            (height as f64 * dpr) as u32,
            dpr
        );

        Ok(CanvasContext {
            canvas,
            context,
            width,
            height,
            dpr,
        })
    }

    /// 获取 Canvas 宽度（CSS 像素）
    pub fn width(&self) -> u32 {
        self.width
    }

    /// 获取 Canvas 高度（CSS 像素）
    pub fn height(&self) -> u32 {
        self.height
    }

    /// 获取设备像素比
    pub fn dpr(&self) -> f64 {
        self.dpr
    }

    /// 获取 Canvas 元素引用
    pub fn canvas(&self) -> &HtmlCanvasElement {
        &self.canvas
    }

    /// 获取 2D 渲染上下文引用
    pub fn context(&self) -> &CanvasRenderingContext2d {
        &self.context
    }

    /// 清空画布
    ///
    /// 使用白色背景清空整个画布
    pub fn clear(&self) {
        self.context.clear_rect(
            0.0,
            0.0,
            self.width as f64,
            self.height as f64,
        );
        
        // 填充白色背景
        self.context.set_fill_style(&"#ffffff".into());
        self.context.fill_rect(
            0.0,
            0.0,
            self.width as f64,
            self.height as f64,
        );
    }
}
