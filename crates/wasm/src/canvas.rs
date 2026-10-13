//! Canvas 2D Context 封装
//!
//! 提供对 HTMLCanvasElement 和 Canvas 2D API 的 Rust 封装
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
        self.context
            .clear_rect(0.0, 0.0, self.width as f64, self.height as f64);

        // 填充白色背景
        self.context.set_fill_style_str("#ffffff");
        self.context
            .fill_rect(0.0, 0.0, self.width as f64, self.height as f64);
    }

    /// 绘制填充矩形
    ///
    /// # 参数
    /// - `x`: X 坐标
    /// - `y`: Y 坐标
    /// - `width`: 宽度
    /// - `height`: 高度
    /// - `color`: 颜色（CSS 颜色字符串，如 "#ff0000"）
    pub fn fill_rect(&self, x: f32, y: f32, width: f32, height: f32, color: &str) {
        self.context.set_fill_style_str(color);
        self.context
            .fill_rect(x as f64, y as f64, width as f64, height as f64);
    }

    /// 绘制描边矩形
    ///
    /// # 参数
    /// - `x`: X 坐标
    /// - `y`: Y 坐标
    /// - `width`: 宽度
    /// - `height`: 高度
    /// - `color`: 颜色
    /// - `line_width`: 线宽
    pub fn stroke_rect(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: &str,
        line_width: f32,
    ) {
        self.context.set_stroke_style_str(color);
        self.context.set_line_width(line_width as f64);
        self.context
            .stroke_rect(x as f64, y as f64, width as f64, height as f64);
    }

    /// 绘制圆形
    ///
    /// # 参数
    /// - `x`: 圆心 X 坐标
    /// - `y`: 圆心 Y 坐标
    /// - `radius`: 半径
    /// - `fill_color`: 填充颜色（None 表示不填充）
    /// - `stroke_color`: 描边颜色（None 表示不描边）
    /// - `line_width`: 线宽
    pub fn draw_circle(
        &self,
        x: f32,
        y: f32,
        radius: f32,
        fill_color: Option<&str>,
        stroke_color: Option<&str>,
        line_width: f32,
    ) {
        self.context.begin_path();
        self.context
            .arc(
                x as f64,
                y as f64,
                radius as f64,
                0.0,
                std::f64::consts::PI * 2.0,
            )
            .ok();

        if let Some(color) = fill_color {
            self.context.set_fill_style_str(color);
            self.context.fill();
        }

        if let Some(color) = stroke_color {
            self.context.set_stroke_style_str(color);
            self.context.set_line_width(line_width as f64);
            self.context.stroke();
        }
    }

    /// 保存上下文状态
    pub fn save(&self) {
        self.context.save();
    }

    /// 恢复上下文状态
    pub fn restore(&self) {
        self.context.restore();
    }

    /// 应用变换矩阵
    ///
    /// # 参数
    /// - `a`: 水平缩放
    /// - `b`: 水平倾斜
    /// - `c`: 垂直倾斜
    /// - `d`: 垂直缩放
    /// - `e`: 水平移动
    /// - `f`: 垂直移动
    pub fn set_transform(&self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        self.context
            .set_transform(a as f64, b as f64, c as f64, d as f64, e as f64, f as f64)
            .ok();
    }

    /// 重置变换为单位矩阵
    pub fn reset_transform(&self) {
        self.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    }

    /// 绘制线段
    ///
    /// # 参数
    /// - `x1, y1`: 起点
    /// - `x2, y2`: 终点
    /// - `color`: 颜色
    /// - `width`: 线宽
    pub fn draw_line(&self, x1: f32, y1: f32, x2: f32, y2: f32, color: &str, width: f32) {
        self.context.set_stroke_style_str(color);
        self.context.set_line_width(width as f64);
        self.context.begin_path();
        self.context.move_to(x1 as f64, y1 as f64);
        self.context.line_to(x2 as f64, y2 as f64);
        self.context.stroke();
    }
}
