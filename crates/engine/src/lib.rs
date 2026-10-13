//! InfiniteCanvas Engine - Core rendering engine
//!
//! 提供高性能无限画布的核心渲染能力。
//!
//! # 模块职责
//! - 元素定义和管理
//! - 渲染管理和批处理
//! - 视口管理和坐标变换
//! - 场景图和空间索引
//!
//! # 模块关系
//! ```text
//! infinitecanvas-wasm (WASM 绑定)
//!        ↓
//! infinitecanvas-engine (本模块)
//!        ↓
//! infinitecanvas-math (数学库)
//! ```

pub mod command;
pub mod element;
pub mod event;
pub mod hit_test;
pub mod quadtree;
pub mod scene;
pub mod selection;
pub mod selection_box;
pub mod tool;
pub mod viewport;

// 重导出核心类型
pub use command::{AddElementCommand, Command, CommandHistory, MoveCommand, RemoveElementCommand};
pub use element::{Circle, Element, ElementId, ElementProps, ElementStyle, Image, Rectangle};
pub use event::{
    Event, EventQueue, KeyboardEvent, Modifiers, PointerButton, PointerEvent, WheelDeltaMode,
    WheelEvent,
};
pub use hit_test::{hit_test_point, HitTestOptions, HitTestResult};
pub use infinitecanvas_math as math;
pub use quadtree::QuadTree;
pub use scene::{AnyElement, Scene};
pub use selection::SelectionManager;
pub use selection_box::SelectionBox;
pub use tool::{Tool, ToolManager};
pub use viewport::Viewport;

/// 渲染器，负责渲染循环控制
///
/// # 职责
/// - 管理渲染管线和资源
/// - 执行每帧渲染
///
/// # 示例
/// ```no_run
/// use infinitecanvas_engine::Renderer;
/// let renderer = Renderer::new();
/// ```
#[derive(Default)]
pub struct Renderer;

impl Renderer {
    /// 创建新的渲染器实例
    pub fn new() -> Self {
        Self
    }
}
