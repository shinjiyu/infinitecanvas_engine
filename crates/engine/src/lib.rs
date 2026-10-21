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

pub mod align;
pub mod asset;
pub mod blend;
pub mod boolean;
pub mod camera_fit;
pub mod command;
pub mod crop;
pub mod element;
pub mod event;
pub mod eyedropper;
pub mod grid;
pub mod guides;
pub mod hit_test;
pub mod mask;
pub mod minimap;
pub mod orient;
pub mod path_edit;
pub mod persist;
pub mod quadtree;
pub mod scene;
pub mod scene_tree;
pub mod selection;
pub mod selection_box;
pub mod snap;
pub mod style;
pub mod svg;
pub mod svg_import;
pub mod tool;
pub mod transform_handles;
pub mod transform_numeric;
pub mod vertex_cache;
pub mod viewport;

// 重导出核心类型
pub use align::{
    apply_align, apply_distribute, AlignElementsCommand, AlignMode, DistributeAxis,
    DistributeElementsCommand,
};
pub use asset::{fitted_size, AssetError, AssetTable, ImageAsset, MAX_IMAGE_EDGE};
pub use blend::{composite_straight, BlendMode, BLEND_MODES};
pub use boolean::{apply_boolean, boolean_targets, BooleanElementsCommand, BooleanOp};
pub use camera_fit::{
    camera_for_bounds, selection_content_bounds, zoom_viewport_to_scene,
    zoom_viewport_to_selection, CameraPose, DEFAULT_FIT_PADDING,
};
pub use command::{
    apply_nudge, apply_rename_element, apply_rename_frame, apply_reparent_into_frame,
    apply_set_locked, apply_set_visibility, apply_stack_reorder, apply_toggle_locked,
    apply_toggle_visibility, AddElementCommand, Command, CommandHistory, DuplicateElementsCommand,
    EditGuidesCommand, MoveCommand, NudgeDirection, NudgeElementsCommand, PasteElementsCommand,
    RemoveElementCommand, RemoveElementsCommand, RenameElementCommand, RenameFrameCommand,
    ReorderOutlineCommand, ReorderSelectionCommand, ReparentIntoFrameCommand, SetLockedCommand,
    SetStyleCommand, SetVisibilityCommand, StackOrderAction, TransformElementsCommand,
    UpdateTextCommand, NUDGE_SHIFT_FACTOR, NUDGE_STEP,
};
pub use crop::{
    apply_image_crop, crop_target_ids, image_crop_selection, resize_image_crop, CropPatch,
    ImageCropSelection, SetImageCropCommand,
};
pub use element::{
    Circle, DropShadow, Element, ElementId, ElementProps, ElementStyle, FillKind, FillShape,
    FontWeight, Frame, GradientStop, Group, Image, InnerShadow, Line, LineMarker, Path,
    PathPointHandles, Polygon, PolygonKind, Rectangle, StrokeAlign, StrokeCap, StrokeJoin, Text,
    TextAlign, TextClickResolver, TextPlacement, TextVisual,
};
pub use event::{
    Event, EventQueue, KeyboardEvent, Modifiers, PointerButton, PointerEvent, WheelDeltaMode,
    WheelEvent,
};
pub use eyedropper::{apply_sampled_paint, sample_paint_at, SampledPaint};
pub use grid::{
    grid_lines, grid_vertices, on_grid_line, visible_grid_spacing, GridLine, GRID_AXIS_COLOR,
    GRID_COLOR, GRID_SPACING,
};
pub use guides::{
    edge_is_axis_aligned, handle_outward, resize_edge_correction, ruler_band, ruler_major_step,
    ruler_ticks, GuideSnapshot, GuideStore, ManualGuide, RulerBand, RulerOrientation, RulerTick,
    GUIDE_HIT_PX, RULER_THICKNESS_PX,
};
pub use hit_test::{click_target, hit_test_point, HitTestOptions, HitTestResult};
pub use infinitecanvas_math as math;
pub use mask::{
    apply_release_mask, apply_set_mask, can_be_mask, can_release_mask, can_set_mask,
    clear_mask_refs_to, filter_lines_by_mask, filter_tris_by_mask, mask_covers,
    mask_pair_from_selection, release_targets, SetMaskCommand,
};
pub use minimap::{
    build_overview, drag_camera_center, fit_minimap, minimap_to_world, scene_bounds,
    viewport_minimap_rect, world_rect_to_minimap, world_to_minimap, MinimapFrame, MinimapOverview,
};
pub use orient::{apply_flip, apply_rotate_quarter, FlipAxis, OrientSelectionCommand, QuarterTurn};
pub use path_edit::{
    apply_path_geometry, hit_path_node, move_path_node, path_node_views, sole_editable_path,
    EditPathCommand, PathGeometry, PathNodeHit, PathNodeIndex, PathNodePart, PathNodeView,
};
pub use persist::{
    export_frame_png, export_frame_png_scaled, export_selection_png, export_selection_png_scaled,
    export_viewport_png, export_viewport_png_scaled, export_viewport_png_with_guides, PersistError,
    SceneFile, SCENE_FORMAT_VERSION,
};
pub use quadtree::QuadTree;
pub use scene::OutlineSnapshot;
pub use scene::{AnyElement, GroupError, Scene};
pub use scene_tree::{SceneTree, SceneTreeNode};
pub use selection::SelectionManager;
pub use selection_box::SelectionBox;
pub use snap::{
    bounds_union, snap_bounds, snap_bounds_with_guides, snap_drag, snap_drag_with_guides,
    AlignmentGuide, DragSnap, GuideAxis, GuideLines, SnapResult, SnapSettings,
};
pub use style::{
    apply_style_snapshot, capture_style, selection_style, style_locked, style_target_ids,
    SelectionStyle, StyleField, StylePatch, StyleSnapshot, TextPaint,
};
pub use svg::{export_frame_svg, export_svg, export_svg_with_guides};
pub use svg_import::{apply_svg_import, import_svg, ImportSvgCommand, SvgImportError};
pub use tool::{Tool, ToolManager};
pub use transform_numeric::{
    apply_selection_transform, selection_transform, SelectionTransform, TransformNumericPatch,
};
pub use vertex_cache::{
    rebuild_all_vertices, GpuVertexWrite, PaintSpan, RebuiltGeometry, TexturedDraw,
    VertexBufferKind, VertexCache, VertexSync,
};
pub use viewport::{Viewport, ZOOM_STEP};

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
