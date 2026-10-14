//! 小地图：场景包围盒与固定像素框之间的等比映射。
//!
//! 缩略图只画元素的轴对齐外框，不走 WebGPU。相机变化不改变映射，
//! 因此平移和缩放只需要重算视口框。
//!
//! ```text
//! 可见叶子 world_bounds ──► scene_bounds ──► fit_minimap
//!                                              │
//!                    元素外框 ◄── world_to_minimap
//!                    视口框   ◄── 同一映射（不扫描场景）
//!                    点击/拖拽 ──► minimap_to_world ──► 相机中心
//! ```

use crate::scene::{AnyElement, Scene};
use crate::snap::bounds_union;
use crate::viewport::Viewport;
use infinitecanvas_math::{Rect, Vec2};
use std::collections::HashSet;

/// 小地图默认宽度（像素）。
pub const DEFAULT_MINIMAP_WIDTH: f32 = 200.0;
/// 小地图默认高度（像素）。
pub const DEFAULT_MINIMAP_HEIGHT: f32 = 150.0;
/// 小地图默认内边距（像素）。内容在边距内等比适配。
pub const DEFAULT_MINIMAP_PADDING: f32 = 8.0;
/// 世界包围盒任一轴短于该值时，绕中心扩到这个长度，避免除以 0。
const MIN_WORLD_EXTENT: f32 = 1.0;
/// 边长上限，避免异常入参分配巨大的像素网格。
const MAX_MINIMAP_SIDE: f32 = 1024.0;
/// 映射后宽高都不超过该值的外框并入像素网格，避免 10 万个亚像素矩形逐个上屏。
const BIN_MAX_PIXELS: f32 = 1.5;

/// 世界矩形到小地图像素矩形的等比映射。
///
/// `world` 铺满 `content`。空场景时 `empty` 为 true，`world` 是绕原点、
/// 宽高比与内容区一致的占位矩形，反算仍然成立。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::math::Vec2;
/// use infinitecanvas_engine::{
///     fit_minimap, minimap_to_world, scene_bounds, world_to_minimap, Rectangle, Scene,
/// };
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(
///     1,
///     Vec2::new(50.0, 25.0),
///     Vec2::new(100.0, 50.0),
/// ));
/// let frame = fit_minimap(scene_bounds(&scene), Vec2::new(200.0, 150.0), 8.0);
/// let pixel = world_to_minimap(&frame, frame.world.center());
/// let world = minimap_to_world(&frame, pixel);
/// assert!((world.x - frame.world.center().x).abs() < 1e-3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimapFrame {
    /// 小地图像素尺寸。
    pub size: Vec2,
    /// 内容区四周留白（像素）。
    pub padding: f32,
    /// 映射到 `content` 的世界矩形。
    pub world: Rect,
    /// `world` 在小地图里占的像素矩形（在留白内居中，保持宽高比）。
    pub content: Rect,
    /// 世界坐标乘上它再加 `content` 原点，得到像素坐标。
    pub scale: f32,
    /// 场景没有可画的叶子时为 true。
    pub empty: bool,
}

/// 一帧缩略图：映射，加上可直接填充的像素外框（`x, y, width, height` 交错）。
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapOverview {
    /// 本帧使用的映射。
    pub frame: MinimapFrame,
    /// 元素外框，像素坐标。亚像素外框按像素去重。
    pub rects: Vec<f32>,
}

/// 把调用方传入的尺寸和边距收成可用值。
///
/// 非有限、过小的尺寸退回 [`DEFAULT_MINIMAP_WIDTH`] × [`DEFAULT_MINIMAP_HEIGHT`]。
/// 边距过大时缩小，保证内容区至少有 1 像素。
pub fn normalize_minimap_view(size: Vec2, padding: f32) -> (Vec2, f32) {
    let width = sanitize_side(size.x, DEFAULT_MINIMAP_WIDTH);
    let height = sanitize_side(size.y, DEFAULT_MINIMAP_HEIGHT);
    let mut padding = if padding.is_finite() && padding > 0.0 {
        padding
    } else {
        0.0
    };
    let max_padding = ((width.min(height) - 1.0) * 0.5).max(0.0);
    if padding > max_padding {
        padding = max_padding;
    }
    (Vec2::new(width, height), padding)
}

/// 可见叶子的世界包围盒并集。组本身不参与，隐藏元素及其子孙也不参与。
///
/// 空场景返回 `None`。复用各元素的 [`Element::world_bounds`] 和 [`bounds_union`]。
///
/// # 参数
/// - `scene`: 场景
///
/// # 返回
/// 轴对齐并集；没有可画叶子时为 `None`。
pub fn scene_bounds(scene: &Scene) -> Option<Rect> {
    bounds_union(drawable_bounds(scene))
}

/// 把世界包围盒装进固定小地图，保持宽高比并在留白内居中。
///
/// `world` 为 `None`、非有限、或某一轴为 0 时仍返回可逆映射：
/// 空场景绕原点放置一块与内容区同比例的占位世界矩形；
/// 退化包围盒绕自身中心把短轴扩到 1 个世界单位。
///
/// # 参数
/// - `world`: [`scene_bounds`] 的结果
/// - `size`: 小地图像素尺寸
/// - `padding`: 四周留白（像素）
pub fn fit_minimap(world: Option<Rect>, size: Vec2, padding: f32) -> MinimapFrame {
    let (size, padding) = normalize_minimap_view(size, padding);
    let inner_w = (size.x - padding * 2.0).max(1.0);
    let inner_h = (size.y - padding * 2.0).max(1.0);
    let (world, empty) = usable_world(world, inner_w, inner_h);
    let world_aspect = world.width / world.height;
    let inner_aspect = inner_w / inner_h;
    let (content_w, content_h) = if world_aspect >= inner_aspect {
        (inner_w, inner_w / world_aspect)
    } else {
        (inner_h * world_aspect, inner_h)
    };
    let content = Rect::new(
        padding + (inner_w - content_w) * 0.5,
        padding + (inner_h - content_h) * 0.5,
        content_w,
        content_h,
    );
    let scale = content_w / world.width;
    MinimapFrame {
        size,
        padding,
        world,
        content,
        scale,
        empty,
    }
}

/// 世界坐标 → 小地图像素。映射是线性的，包围盒外的点会落到内容区之外。
pub fn world_to_minimap(frame: &MinimapFrame, world: Vec2) -> Vec2 {
    Vec2::new(
        frame.content.x + (world.x - frame.world.x) * frame.scale,
        frame.content.y + (world.y - frame.world.y) * frame.scale,
    )
}

/// 小地图像素 → 世界坐标。[`world_to_minimap`] 的逆变换，留白区域同样可反算。
pub fn minimap_to_world(frame: &MinimapFrame, pixel: Vec2) -> Vec2 {
    let scale = if frame.scale.is_finite() && frame.scale > f32::EPSILON {
        frame.scale
    } else {
        1.0
    };
    Vec2::new(
        frame.world.x + (pixel.x - frame.content.x) / scale,
        frame.world.y + (pixel.y - frame.content.y) / scale,
    )
}

/// 把世界轴对齐矩形映射成小地图像素矩形。
pub fn world_rect_to_minimap(frame: &MinimapFrame, rect: Rect) -> Rect {
    let min = world_to_minimap(frame, rect.min());
    let max = world_to_minimap(frame, Vec2::new(rect.right(), rect.bottom()));
    Rect::from_corners(min, max)
}

/// 视口在小地图上的像素矩形。只读相机，不扫描元素。
pub fn viewport_minimap_rect(frame: &MinimapFrame, viewport: &Viewport) -> Rect {
    world_rect_to_minimap(frame, viewport.bounds())
}

/// 拖拽视口框时的新相机中心。
///
/// `grab_offset` 是按下时指针相对视口框中心的像素偏移。
/// 指针不动时中心不变；指针移动多少像素，框中心就移动多少像素。
///
/// # 参数
/// - `frame`: 当前映射（只依赖场景包围盒）
/// - `pointer`: 当前指针的小地图像素坐标
/// - `grab_offset`: 按下点减去当时的视口框中心
pub fn drag_camera_center(frame: &MinimapFrame, pointer: Vec2, grab_offset: Vec2) -> Vec2 {
    minimap_to_world(frame, pointer - grab_offset)
}

/// 收集缩略图外框。组不画；隐藏叶子不画。
///
/// 小于 [`BIN_MAX_PIXELS`] 的外框按像素去重，每个被盖住的像素最多输出一个方块。
/// 更大的外框原样输出，并保证至少 1×1 像素，避免细线消失。
pub fn build_overview(scene: &Scene, size: Vec2, padding: f32) -> MinimapOverview {
    let world_rects: Vec<Rect> = drawable_bounds(scene).collect();
    let frame = fit_minimap(bounds_union(world_rects.iter().copied()), size, padding);
    let width = frame.size.x as i32;
    let height = frame.size.y as i32;
    let mut occupied: HashSet<(i32, i32)> = HashSet::new();
    let mut rects = Vec::new();

    for world in world_rects {
        let pixel = world_rect_to_minimap(&frame, world);
        if !is_finite_rect(pixel) {
            continue;
        }
        if pixel.width <= BIN_MAX_PIXELS && pixel.height <= BIN_MAX_PIXELS {
            let x = pixel.center().x.floor() as i32;
            let y = pixel.center().y.floor() as i32;
            if x < 0 || y < 0 || x >= width || y >= height {
                continue;
            }
            if occupied.insert((x, y)) {
                rects.extend_from_slice(&[x as f32, y as f32, 1.0, 1.0]);
            }
            continue;
        }
        push_visible_block(&mut rects, pixel);
    }

    MinimapOverview { frame, rects }
}

fn drawable_bounds(scene: &Scene) -> impl Iterator<Item = Rect> + '_ {
    scene.iter().filter_map(|element| {
        if matches!(element, AnyElement::Group(_)) || !scene.is_effectively_visible(element.id()) {
            return None;
        }
        let bounds = element.as_element().world_bounds();
        is_finite_rect(bounds).then_some(bounds)
    })
}

fn sanitize_side(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value >= 2.0 {
        value.round().clamp(2.0, MAX_MINIMAP_SIDE)
    } else {
        fallback
    }
}

fn usable_world(bounds: Option<Rect>, inner_w: f32, inner_h: f32) -> (Rect, bool) {
    let Some(bounds) = bounds.filter(|rect| is_finite_rect(*rect)) else {
        return (fallback_world(inner_w, inner_h), true);
    };
    let center = bounds.center();
    if !center.x.is_finite() || !center.y.is_finite() {
        return (fallback_world(inner_w, inner_h), true);
    }
    let width = bounds.width.abs().max(MIN_WORLD_EXTENT);
    let height = bounds.height.abs().max(MIN_WORLD_EXTENT);
    (
        Rect::from_center_size(center, Vec2::new(width, height)),
        false,
    )
}

fn fallback_world(inner_w: f32, inner_h: f32) -> Rect {
    let short = 100.0;
    let (width, height) = if inner_w >= inner_h {
        (short * inner_w / inner_h, short)
    } else {
        (short, short * inner_h / inner_w)
    };
    Rect::from_center_size(Vec2::ZERO, Vec2::new(width, height))
}

fn is_finite_rect(rect: Rect) -> bool {
    rect.x.is_finite() && rect.y.is_finite() && rect.width.is_finite() && rect.height.is_finite()
}

fn push_visible_block(out: &mut Vec<f32>, rect: Rect) {
    let width = rect.width.max(1.0);
    let height = rect.height.max(1.0);
    let x = rect.center().x - width * 0.5;
    let y = rect.center().y - height * 0.5;
    out.extend_from_slice(&[x, y, width, height]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::Rectangle;
    use crate::scene::Scene;
    use crate::viewport::Viewport;

    fn assert_close(actual: f32, expected: f32) {
        let scale = actual.abs().max(expected.abs()).max(1.0);
        let tol = 1e-3 * scale;
        assert!(
            (actual - expected).abs() <= tol,
            "{actual} != {expected} (tol {tol})"
        );
    }

    fn assert_vec_close(actual: Vec2, expected: Vec2) {
        assert_close(actual.x, expected.x);
        assert_close(actual.y, expected.y);
    }

    fn assert_rect_close(actual: Rect, expected: Rect) {
        assert_close(actual.x, expected.x);
        assert_close(actual.y, expected.y);
        assert_close(actual.width, expected.width);
        assert_close(actual.height, expected.height);
    }

    fn roundtrip(frame: &MinimapFrame, world: Vec2) {
        let pixel = world_to_minimap(frame, world);
        let back = minimap_to_world(frame, pixel);
        assert_vec_close(back, world);
    }

    #[test]
    fn empty_scene_maps_origin_to_content_center_and_back() {
        let scene = Scene::new();
        assert_eq!(scene_bounds(&scene), None);

        let frame = fit_minimap(None, Vec2::new(200.0, 150.0), 8.0);
        assert!(frame.empty);
        assert_vec_close(frame.content.center(), Vec2::new(100.0, 75.0));
        assert_vec_close(minimap_to_world(&frame, frame.content.center()), Vec2::ZERO);
        roundtrip(&frame, Vec2::ZERO);
        roundtrip(&frame, frame.world.min());
        roundtrip(&frame, frame.world.max());

        let world_aspect = frame.world.width / frame.world.height;
        let content_aspect = frame.content.width / frame.content.height;
        assert_close(world_aspect, content_aspect);
    }

    #[test]
    fn single_element_bounds_roundtrip_through_minimap() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(50.0, 25.0),
            Vec2::new(100.0, 50.0),
        ));
        let bounds = scene_bounds(&scene).unwrap();
        assert_rect_close(bounds, Rect::new(0.0, 0.0, 100.0, 50.0));

        let frame = fit_minimap(Some(bounds), Vec2::new(200.0, 150.0), 10.0);
        assert!(!frame.empty);
        // 内容区 180×130，世界 100×50 更宽，横向铺满，纵向居中。
        assert_close(frame.content.x, 10.0);
        assert_close(frame.content.width, 180.0);
        assert_close(frame.content.height, 90.0);
        assert_close(frame.content.y, 30.0);
        assert_close(frame.scale, 1.8);

        assert_vec_close(world_to_minimap(&frame, Vec2::ZERO), Vec2::new(10.0, 30.0));
        assert_vec_close(
            world_to_minimap(&frame, Vec2::new(100.0, 50.0)),
            Vec2::new(190.0, 120.0),
        );
        roundtrip(&frame, bounds.min());
        roundtrip(&frame, bounds.max());
        roundtrip(&frame, bounds.center());

        let outside = minimap_to_world(&frame, Vec2::new(0.0, 0.0));
        assert!(outside.x < bounds.left());
        assert!(outside.y < bounds.top());
    }

    #[test]
    fn extreme_wide_scene_letterboxes_and_inverts() {
        let frame = fit_minimap(
            Some(Rect::new(0.0, 0.0, 10_000.0, 1.0)),
            Vec2::new(200.0, 150.0),
            8.0,
        );
        assert!(frame.content.width > frame.content.height * 100.0);
        assert!(frame.content.x >= frame.padding - 1e-3);
        assert!(frame.content.right() <= frame.size.x - frame.padding + 1e-3);
        let inner_h = frame.size.y - frame.padding * 2.0;
        let spare = inner_h - frame.content.height;
        assert_close(frame.content.y, frame.padding + spare * 0.5);
        roundtrip(&frame, Vec2::ZERO);
        roundtrip(&frame, Vec2::new(10_000.0, 1.0));
        roundtrip(&frame, Vec2::new(10_000.0, 0.0));
    }

    #[test]
    fn extreme_tall_scene_pillarboxes_and_inverts() {
        let frame = fit_minimap(
            Some(Rect::new(-20.0, 5.0, 1.0, 10_000.0)),
            Vec2::new(200.0, 150.0),
            8.0,
        );
        assert!(frame.content.height > frame.content.width * 100.0);
        assert!(frame.content.y >= frame.padding - 1e-3);
        assert!(frame.content.bottom() <= frame.size.y - frame.padding + 1e-3);
        let inner_w = frame.size.x - frame.padding * 2.0;
        let spare = inner_w - frame.content.width;
        assert_close(frame.content.x, frame.padding + spare * 0.5);
        roundtrip(&frame, Vec2::new(-20.0, 5.0));
        roundtrip(&frame, Vec2::new(-19.0, 10_005.0));
    }

    #[test]
    fn degenerate_bounds_expand_around_center() {
        let frame = fit_minimap(
            Some(Rect::new(40.0, -10.0, 0.0, 0.0)),
            Vec2::new(200.0, 150.0),
            8.0,
        );
        assert!(!frame.empty);
        assert_close(frame.world.width, MIN_WORLD_EXTENT);
        assert_close(frame.world.height, MIN_WORLD_EXTENT);
        assert_vec_close(frame.world.center(), Vec2::new(40.0, -10.0));
        roundtrip(&frame, frame.world.center());
    }

    #[test]
    fn groups_and_hidden_leaves_do_not_change_bounds() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(100.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let before = scene_bounds(&scene).unwrap();
        scene.group(&[1, 2]).unwrap();
        assert_rect_close(scene_bounds(&scene).unwrap(), before);

        scene.add_rect(Rectangle::new(
            10,
            Vec2::new(1_000.0, 1_000.0),
            Vec2::new(10.0, 10.0),
        ));
        assert!(scene_bounds(&scene).unwrap().right() > 500.0);
        scene.set_visible(10, false);
        assert_rect_close(scene_bounds(&scene).unwrap(), before);
    }

    #[test]
    fn drag_keeps_grab_point_and_follows_pointer() {
        let frame = fit_minimap(
            Some(Rect::new(0.0, 0.0, 400.0, 200.0)),
            Vec2::new(200.0, 150.0),
            8.0,
        );
        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));
        viewport.move_to(Vec2::new(80.0, 40.0));
        let center_px = world_to_minimap(&frame, viewport.center());
        let grab = center_px + Vec2::new(6.0, -4.0);
        let offset = grab - center_px;

        assert_vec_close(drag_camera_center(&frame, grab, offset), viewport.center());

        let moved = drag_camera_center(&frame, grab + Vec2::new(12.0, -5.0), offset);
        assert_vec_close(
            moved,
            minimap_to_world(&frame, center_px + Vec2::new(12.0, -5.0)),
        );
        assert!(moved.x > viewport.center().x);
    }

    #[test]
    fn camera_change_does_not_move_element_mapping() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(20.0, 10.0),
            Vec2::new(30.0, 20.0),
        ));
        let frame = fit_minimap(scene_bounds(&scene), Vec2::new(200.0, 150.0), 8.0);
        let element_px = world_to_minimap(&frame, Vec2::new(20.0, 10.0));
        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));
        let before = viewport_minimap_rect(&frame, &viewport);
        viewport.move_to(Vec2::new(50.0, -20.0));
        viewport.set_zoom(2.0);
        let after = viewport_minimap_rect(&frame, &viewport);
        assert!((after.x - before.x).abs() > 1.0 || (after.width - before.width).abs() > 1.0);
        assert_vec_close(world_to_minimap(&frame, Vec2::new(20.0, 10.0)), element_px);
        assert_vec_close(minimap_to_world(&frame, after.center()), viewport.center());
    }

    #[test]
    fn tiny_elements_share_a_pixel_and_large_ones_stay_blocks() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(200.0, 80.0),
        ));
        for index in 0..40u64 {
            scene.add_rect(Rectangle::new(
                10 + index,
                Vec2::new(500.0 + (index as f32) * 0.01, 300.0),
                Vec2::new(0.2, 0.2),
            ));
        }
        let overview = build_overview(&scene, Vec2::new(200.0, 150.0), 8.0);
        let blocks = overview.rects.len() / 4;
        assert!(
            blocks >= 2,
            "expected the large rect plus at least one pixel"
        );
        assert!(
            blocks < 20,
            "tiny cluster should collapse, got {blocks} blocks"
        );
        assert!(!overview.frame.empty);
    }

    #[test]
    fn geometry_epoch_advances_on_edits_only() {
        let mut scene = Scene::new();
        assert_eq!(scene.geometry_epoch(), 0);
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        let added = scene.geometry_epoch();
        assert!(added > 0);
        assert_eq!(scene.geometry_epoch(), added);
        scene.mark_dirty(1);
        let once = scene.geometry_epoch();
        assert!(once > added);
        scene.mark_dirty(1);
        assert!(scene.geometry_epoch() > once);
        scene.mark_dirty(99);
        let missing = scene.geometry_epoch();
        scene.clear();
        assert!(scene.geometry_epoch() > missing);
        assert_eq!(scene_bounds(&scene), None);
    }

    #[test]
    #[ignore = "local timing: 100k overview rebuild vs camera-only remap"]
    fn timing_100k_overview_stays_off_camera_path() {
        let mut scene = Scene::new();
        for index in 0..100_000u64 {
            let column = (index % 400) as f32;
            let row = (index / 400) as f32;
            scene.add_rect(Rectangle::new(
                index + 1,
                Vec2::new(column * 24.0, row * 24.0),
                Vec2::new(16.0, 12.0),
            ));
        }
        let started = std::time::Instant::now();
        let overview = build_overview(&scene, Vec2::new(200.0, 150.0), 8.0);
        let built = started.elapsed();
        let mut viewport = Viewport::new(Vec2::new(1200.0, 700.0));
        let started = std::time::Instant::now();
        for step in 0..1_000u32 {
            viewport.move_to(Vec2::new(step as f32, 16.0));
            let rect = viewport_minimap_rect(&overview.frame, &viewport);
            std::hint::black_box(rect);
        }
        let cameras = started.elapsed();
        eprintln!(
            "100k overview {built:?} blocks {} ; 1000 camera maps {cameras:?}",
            overview.rects.len() / 4
        );
        assert!(overview.rects.len() / 4 < 200 * 150);
        assert!(cameras < built);
    }
}
