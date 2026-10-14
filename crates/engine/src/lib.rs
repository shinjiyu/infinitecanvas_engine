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
pub mod persist;
pub mod quadtree;
pub mod scene;
pub mod scene_tree;
pub mod selection;
pub mod selection_box;
pub mod snap;
pub mod tool;
pub mod transform_handles;
pub mod vertex_cache;
pub mod viewport;

// 重导出核心类型
pub use command::{
    AddElementCommand, Command, CommandHistory, MoveCommand, RemoveElementCommand,
    TransformElementsCommand, UpdateTextCommand,
};
pub use element::{
    Circle, Element, ElementId, ElementProps, ElementStyle, Group, Image, Line, Path, Rectangle,
    Text, TextAlign, TextClickResolver, TextPlacement, TextVisual,
};
pub use event::{
    Event, EventQueue, KeyboardEvent, Modifiers, PointerButton, PointerEvent, WheelDeltaMode,
    WheelEvent,
};
pub use hit_test::{hit_test_point, HitTestOptions, HitTestResult};
pub use infinitecanvas_math as math;
pub use persist::{
    export_selection_png, export_viewport_png, PersistError, SceneFile, SCENE_FORMAT_VERSION,
};
pub use quadtree::QuadTree;
pub use scene::{AnyElement, GroupError, Scene};
pub use scene_tree::{SceneTree, SceneTreeNode};
pub use selection::SelectionManager;
pub use selection_box::SelectionBox;
pub use snap::{
    bounds_union, snap_bounds, snap_drag, AlignmentGuide, DragSnap, GuideAxis, SnapResult,
    SnapSettings,
};
pub use tool::{Tool, ToolManager};
pub use vertex_cache::{
    rebuild_all_vertices, GpuVertexWrite, RebuiltGeometry, TexturedDraw, VertexBufferKind,
    VertexCache, VertexSync,
};
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
