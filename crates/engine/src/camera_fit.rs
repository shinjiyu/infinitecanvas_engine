//! 把相机对准一块世界包围盒。
//!
//! 只改视口的中心和缩放，不改元素几何，也不进撤销栈
//! （[`crate::Command`] 只作用于场景）。
//!
//! ```text
//! 选中 id ──可见且未锁定──► world_bounds（组不展开，用组根）
//! 可见叶子 ── scene_bounds ─┤
//!                           ▼
//!              camera_for_bounds（每边留视口的 10%）
//!                           │
//!                           ▼
//!                 Viewport center + zoom
//!
//! 空选   ──► 相机不动
//! 空场景 ──► 默认相机（原点，1×）
//! ```

use crate::minimap::scene_bounds;
use crate::scene::Scene;
use crate::snap::bounds_union;
use crate::viewport::Viewport;
use crate::ElementId;
use infinitecanvas_math::{Rect, Vec2};

/// 适应时每边留下的视口比例。`0.1` 表示左右各 10%、上下各 10%。
///
/// 内容被装进视口的 80%。宽高比不一致时，更紧的那条轴刚好留出这个比例，
/// 另一条轴留白更大。缩放仍限制在视口的最小/最大倍数之间。
pub const DEFAULT_FIT_PADDING: f32 = 0.1;

/// 空场景「适应全部」回到的缩放。中心是世界原点。
const DEFAULT_CAMERA_ZOOM: f32 = 1.0;

/// 某一轴短于该值时，绕中心扩到这个长度，避免除以 0。
const MIN_CONTENT_EXTENT: f32 = 1.0;

/// 适应后的相机姿态。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraPose {
    /// 视口中心（世界坐标）。
    pub center: Vec2,
    /// 缩放倍数。已经限制在调用方给出的最小/最大缩放之间。
    pub zoom: f32,
}

/// 计算能把 `content` 装进画布的相机。
///
/// `padding` 是每一边占视口的比例，不是像素。非有限或负数按 0，大于 0.45 时收到 0.45，
/// 保证可用区域仍为正。内容某一轴为 0 时绕中心扩成 1 个世界单位。
/// 画布尺寸非正、或包围盒非有限时返回 `None`，调用方不应改相机。
///
/// # 参数
/// - `content`: 世界空间轴对齐包围盒
/// - `canvas_size`: 画布像素尺寸
/// - `padding`: 每边留白占视口的比例，通常是 [`DEFAULT_FIT_PADDING`]
/// - `min_zoom`, `max_zoom`: 缩放上下限
///
/// # 示例
/// ```
/// use infinitecanvas_engine::camera_for_bounds;
/// use infinitecanvas_engine::math::{Rect, Vec2};
///
/// let pose = camera_for_bounds(
///     Rect::new(0.0, 0.0, 200.0, 100.0),
///     Vec2::new(800.0, 600.0),
///     0.1,
///     0.1,
///     10.0,
/// )
/// .unwrap();
/// assert!((pose.center.x - 100.0).abs() < 1e-3);
/// assert!((pose.zoom - 3.2).abs() < 1e-3);
/// ```
pub fn camera_for_bounds(
    content: Rect,
    canvas_size: Vec2,
    padding: f32,
    min_zoom: f32,
    max_zoom: f32,
) -> Option<CameraPose> {
    if !canvas_size.x.is_finite()
        || !canvas_size.y.is_finite()
        || canvas_size.x <= 0.0
        || canvas_size.y <= 0.0
    {
        return None;
    }
    let content = usable_content(content)?;
    let padding = usable_padding(padding);
    let usable = 1.0 - 2.0 * padding;
    let zoom_x = canvas_size.x * usable / content.width;
    let zoom_y = canvas_size.y * usable / content.height;
    if !zoom_x.is_finite() || !zoom_y.is_finite() {
        return None;
    }
    let (min_zoom, max_zoom) = ordered_zoom(min_zoom, max_zoom);
    Some(CameraPose {
        center: content.center(),
        zoom: zoom_x.min(zoom_y).clamp(min_zoom, max_zoom),
    })
}

/// 当前选中目标的世界包围盒。
///
/// 只保留可见且未锁定的 id。选中组时用组根的 [`crate::Element::world_bounds`]，
/// 不把子元素再展开一遍，因此组里隐藏的子元素仍算在组根包围盒内。
/// 选中的是子元素而不是组时，只用该子元素自己的包围盒。
/// 过滤后没有可用包围盒时返回 `None`。
///
/// # 参数
/// - `scene`: 场景
/// - `ids`: 选择集里的元素 id
pub fn selection_content_bounds(scene: &Scene, ids: &[ElementId]) -> Option<Rect> {
    bounds_union(ids.iter().filter_map(|id| {
        if !scene.is_effectively_visible(*id) || scene.is_effectively_locked(*id) {
            return None;
        }
        let element = scene.get(*id)?;
        let bounds = element.as_element().world_bounds();
        is_finite_rect(bounds).then_some(bounds)
    }))
}

/// 把相机对准当前选中内容。
///
/// 空选，或过滤后没有可见且未锁定的包围盒时不改相机，返回 `false`。
/// 成功时返回 `true`。重复调用得到同一姿态。
///
/// # 参数
/// - `viewport`: 要改的相机
/// - `scene`: 场景
/// - `ids`: 选择集
///
/// # 示例
/// ```
/// use infinitecanvas_engine::math::Vec2;
/// use infinitecanvas_engine::{
///     zoom_viewport_to_selection, Rectangle, Scene, Viewport,
/// };
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(
///     1,
///     Vec2::new(100.0, 50.0),
///     Vec2::new(200.0, 100.0),
/// ));
/// let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));
/// assert!(!zoom_viewport_to_selection(&mut viewport, &scene, &[]));
/// assert!(zoom_viewport_to_selection(&mut viewport, &scene, &[1]));
/// assert!((viewport.center().x - 100.0).abs() < 1e-3);
/// assert!((viewport.zoom() - 3.2).abs() < 1e-3);
/// ```
pub fn zoom_viewport_to_selection(
    viewport: &mut Viewport,
    scene: &Scene,
    ids: &[ElementId],
) -> bool {
    let Some(bounds) = selection_content_bounds(scene, ids) else {
        return false;
    };
    apply_pose(viewport, bounds)
}

/// 把相机对准全部可见叶子。
///
/// 包围盒与小地图的 [`scene_bounds`] 相同：组不单独计入，隐藏叶子及其子孙跳过。
/// 没有可见叶子时把相机放回默认姿态（世界原点、缩放 1），并返回 `false`。
/// 有内容并完成适应时返回 `true`。
///
/// # 参数
/// - `viewport`: 要改的相机
/// - `scene`: 场景
pub fn zoom_viewport_to_scene(viewport: &mut Viewport, scene: &Scene) -> bool {
    match scene_bounds(scene) {
        Some(bounds) => apply_pose(viewport, bounds),
        None => {
            viewport.move_to(Vec2::ZERO);
            viewport.set_zoom(DEFAULT_CAMERA_ZOOM);
            false
        }
    }
}

fn apply_pose(viewport: &mut Viewport, content: Rect) -> bool {
    let Some(pose) = camera_for_bounds(
        content,
        viewport.canvas_size(),
        DEFAULT_FIT_PADDING,
        viewport.min_zoom(),
        viewport.max_zoom(),
    ) else {
        return false;
    };
    viewport.move_to(pose.center);
    viewport.set_zoom(pose.zoom);
    true
}

fn usable_content(rect: Rect) -> Option<Rect> {
    if !is_finite_rect(rect) {
        return None;
    }
    let center = rect.center();
    if !center.x.is_finite() || !center.y.is_finite() {
        return None;
    }
    Some(Rect::from_center_size(
        center,
        Vec2::new(
            rect.width.abs().max(MIN_CONTENT_EXTENT),
            rect.height.abs().max(MIN_CONTENT_EXTENT),
        ),
    ))
}

fn usable_padding(padding: f32) -> f32 {
    if !padding.is_finite() || padding < 0.0 {
        0.0
    } else {
        padding.min(0.45)
    }
}

fn ordered_zoom(min_zoom: f32, max_zoom: f32) -> (f32, f32) {
    let min_zoom = if min_zoom.is_finite() && min_zoom > 0.0 {
        min_zoom
    } else {
        0.1
    };
    let max_zoom = if max_zoom.is_finite() && max_zoom > 0.0 {
        max_zoom
    } else {
        10.0
    };
    if min_zoom <= max_zoom {
        (min_zoom, max_zoom)
    } else {
        (max_zoom, min_zoom)
    }
}

fn is_finite_rect(rect: Rect) -> bool {
    rect.x.is_finite() && rect.y.is_finite() && rect.width.is_finite() && rect.height.is_finite()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minimap::{fit_minimap, viewport_minimap_rect};
    use crate::selection::SelectionManager;
    use crate::Rectangle;

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

    fn translation(scene: &Scene, id: ElementId) -> Vec2 {
        scene
            .get(id)
            .unwrap()
            .as_element()
            .props()
            .transform
            .translation
    }

    /// 未触到缩放上下限时，更紧的那条轴每边留白等于 `padding`。
    fn assert_padded(viewport: &Viewport, content: Rect, padding: f32) {
        assert_vec_close(viewport.center(), content.center());
        let view = viewport.bounds();
        let zoom = viewport.zoom();
        if zoom <= viewport.min_zoom() + 1e-4 {
            return;
        }
        assert!(content.left() >= view.left() - 1e-2);
        assert!(content.right() <= view.right() + 1e-2);
        assert!(content.top() >= view.top() - 1e-2);
        assert!(content.bottom() <= view.bottom() + 1e-2);
        if zoom >= viewport.max_zoom() - 1e-4 {
            return;
        }
        let margin_x = (view.width - content.width) * 0.5 / view.width;
        let margin_y = (view.height - content.height) * 0.5 / view.height;
        assert_close(margin_x.min(margin_y), padding);
        assert!(margin_x + 1e-3 >= padding);
        assert!(margin_y + 1e-3 >= padding);
    }

    #[test]
    fn empty_selection_leaves_the_camera_alone() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(40.0, 20.0),
            Vec2::new(80.0, 40.0),
        ));
        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));
        viewport.move_to(Vec2::new(12.0, -4.0));
        viewport.set_zoom(2.5);
        let epoch = scene.geometry_epoch();

        assert!(!zoom_viewport_to_selection(&mut viewport, &scene, &[]));
        assert_eq!(viewport.center(), Vec2::new(12.0, -4.0));
        assert_eq!(viewport.zoom(), 2.5);
        assert_eq!(scene.geometry_epoch(), epoch);
        assert_eq!(translation(&scene, 1), Vec2::new(40.0, 20.0));
    }

    #[test]
    fn single_selection_frames_that_element() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(100.0, 50.0),
            Vec2::new(200.0, 100.0),
        ));
        let mut selection = SelectionManager::new();
        assert!(selection.select_element(&mut scene, 1, false));
        let before = translation(&scene, 1);
        let epoch = scene.geometry_epoch();
        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));

        assert!(zoom_viewport_to_selection(
            &mut viewport,
            &scene,
            &selection.selected_ids(),
        ));

        let content = Rect::new(0.0, 0.0, 200.0, 100.0);
        assert_padded(&viewport, content, DEFAULT_FIT_PADDING);
        assert_close(viewport.zoom(), 3.2);
        assert_eq!(translation(&scene, 1), before);
        assert_eq!(scene.geometry_epoch(), epoch);
    }

    #[test]
    fn multi_selection_frames_the_union_and_ignores_the_rest() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 50.0),
        ));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(300.0, 80.0),
            Vec2::new(40.0, 40.0),
        ));
        scene.add_rect(Rectangle::new(
            3,
            Vec2::new(-400.0, -400.0),
            Vec2::new(20.0, 20.0),
        ));
        let mut selection = SelectionManager::new();
        selection.select_element(&mut scene, 1, false);
        selection.select_element(&mut scene, 2, true);
        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));

        assert!(zoom_viewport_to_selection(
            &mut viewport,
            &scene,
            &selection.selected_ids(),
        ));

        // 1: [-50, -25, 100, 50]，2: [280, 60, 40, 40]，并集不含 3。
        let content = Rect::new(-50.0, -25.0, 370.0, 125.0);
        assert_rect_close(
            selection_content_bounds(&scene, &selection.selected_ids()).unwrap(),
            content,
        );
        assert_padded(&viewport, content, DEFAULT_FIT_PADDING);
        assert!(viewport.bounds().left() > -200.0);
    }

    #[test]
    fn group_selection_uses_the_group_root_not_one_child() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(20.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(200.0, 0.0),
            Vec2::new(20.0, 10.0),
        ));
        let group_id = scene.group(&[1, 2]).unwrap();
        scene.set_visible(2, false);
        let root = scene.get(group_id).unwrap().as_element().world_bounds();
        assert_rect_close(selection_content_bounds(&scene, &[group_id]).unwrap(), root);
        let child = selection_content_bounds(&scene, &[1]).unwrap();
        assert!(child.width < root.width - 1.0);

        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));
        assert!(zoom_viewport_to_selection(
            &mut viewport,
            &scene,
            &[group_id]
        ));
        assert_vec_close(viewport.center(), root.center());

        let mut child_view = Viewport::new(Vec2::new(800.0, 600.0));
        assert!(zoom_viewport_to_selection(&mut child_view, &scene, &[1]));
        assert!(child_view.zoom() > viewport.zoom());
    }

    #[test]
    fn hidden_or_locked_selection_is_a_noop() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(30.0, 30.0),
            Vec2::new(40.0, 40.0),
        ));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(80.0, 10.0),
            Vec2::new(40.0, 20.0),
        ));
        let mut selection = SelectionManager::new();
        selection.select_element(&mut scene, 1, false);
        scene.set_visible(1, false);
        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));
        viewport.move_to(Vec2::new(3.0, 4.0));
        assert!(!zoom_viewport_to_selection(
            &mut viewport,
            &scene,
            &selection.selected_ids(),
        ));
        assert_eq!(viewport.center(), Vec2::new(3.0, 4.0));

        scene.set_visible(1, true);
        scene.set_locked(1, true);
        assert!(!zoom_viewport_to_selection(
            &mut viewport,
            &scene,
            &selection.selected_ids(),
        ));
        assert_eq!(viewport.zoom(), 1.0);

        selection.select_element(&mut scene, 2, true);
        assert!(zoom_viewport_to_selection(
            &mut viewport,
            &scene,
            &selection.selected_ids(),
        ));
        assert_vec_close(viewport.center(), Vec2::new(80.0, 10.0));
    }

    #[test]
    fn extreme_aspect_ratios_keep_padding_on_the_tight_axis() {
        let canvas = Vec2::new(800.0, 600.0);
        let wide = Rect::new(-500.0, -5.0, 1000.0, 10.0);
        let wide_pose = camera_for_bounds(wide, canvas, 0.1, 0.1, 10.0).unwrap();
        assert_close(wide_pose.zoom, 0.64);
        assert_vec_close(wide_pose.center, wide.center());

        let tall = Rect::new(-5.0, -500.0, 10.0, 1000.0);
        let tall_pose = camera_for_bounds(tall, canvas, 0.1, 0.1, 10.0).unwrap();
        assert_close(tall_pose.zoom, 0.48);
        assert_vec_close(tall_pose.center, tall.center());

        let mut viewport = Viewport::new(canvas);
        viewport.move_to(wide_pose.center);
        viewport.set_zoom(wide_pose.zoom);
        assert_padded(&viewport, wide, 0.1);

        viewport.move_to(tall_pose.center);
        viewport.set_zoom(tall_pose.zoom);
        assert_padded(&viewport, tall, 0.1);
    }

    #[test]
    fn tiny_and_huge_content_clamp_zoom() {
        let canvas = Vec2::new(800.0, 600.0);
        let tiny =
            camera_for_bounds(Rect::new(10.0, 20.0, 2.0, 2.0), canvas, 0.1, 0.1, 10.0).unwrap();
        assert_eq!(tiny.zoom, 10.0);
        assert_vec_close(tiny.center, Vec2::new(11.0, 21.0));

        let huge = camera_for_bounds(
            Rect::new(0.0, 0.0, 100_000.0, 100_000.0),
            canvas,
            0.1,
            0.1,
            10.0,
        )
        .unwrap();
        assert_eq!(huge.zoom, 0.1);
        assert_vec_close(huge.center, Vec2::new(50_000.0, 50_000.0));
    }

    #[test]
    fn already_fitted_viewport_stays_put_and_flush_viewport_gains_padding() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(800.0, 600.0)));
        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));
        assert_eq!(viewport.zoom(), 1.0);
        assert!(viewport.bounds().width - 800.0 < 1e-3);

        assert!(zoom_viewport_to_scene(&mut viewport, &scene));
        assert_close(viewport.zoom(), 0.8);
        assert_padded(
            &viewport,
            Rect::new(-400.0, -300.0, 800.0, 600.0),
            DEFAULT_FIT_PADDING,
        );

        let center = viewport.center();
        let zoom = viewport.zoom();
        assert!(zoom_viewport_to_scene(&mut viewport, &scene));
        assert_eq!(viewport.center(), center);
        assert_eq!(viewport.zoom(), zoom);
        assert_eq!(translation(&scene, 1), Vec2::ZERO);
    }

    #[test]
    fn empty_scene_resets_to_the_default_camera() {
        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));
        viewport.move_to(Vec2::new(500.0, -20.0));
        viewport.set_zoom(4.0);
        assert!(!zoom_viewport_to_scene(&mut viewport, &Scene::new()));
        assert_eq!(viewport.center(), Vec2::ZERO);
        assert_eq!(viewport.zoom(), 1.0);

        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(10.0, 10.0),
            Vec2::new(20.0, 20.0),
        ));
        scene.set_visible(1, false);
        viewport.move_to(Vec2::new(8.0, 9.0));
        viewport.set_zoom(2.0);
        assert!(!zoom_viewport_to_scene(&mut viewport, &scene));
        assert_eq!(viewport.center(), Vec2::ZERO);
        assert_eq!(viewport.zoom(), 1.0);
        assert_eq!(translation(&scene, 1), Vec2::new(10.0, 10.0));
    }

    #[test]
    fn fit_does_not_change_scene_bounds_and_minimap_follows_the_camera() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(100.0, 40.0),
            Vec2::new(200.0, 80.0),
        ));
        let before = scene_bounds(&scene).unwrap();
        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));
        let frame = fit_minimap(Some(before), Vec2::new(200.0, 150.0), 8.0);
        let viewport_before = viewport_minimap_rect(&frame, &viewport);

        assert!(zoom_viewport_to_scene(&mut viewport, &scene));
        assert_rect_close(scene_bounds(&scene).unwrap(), before);
        let viewport_after = viewport_minimap_rect(&frame, &viewport);
        assert!(
            (viewport_after.width - viewport_before.width).abs() > 1.0
                || (viewport_after.x - viewport_before.x).abs() > 1.0
        );

        viewport.move_to(Vec2::new(1.0, 2.0));
        assert_eq!(viewport.center(), Vec2::new(1.0, 2.0));

        let mut selection = SelectionManager::new();
        let hits = selection.select_in_rect(&mut scene, Rect::new(0.0, 0.0, 250.0, 120.0), false);
        assert_eq!(hits, vec![1]);
    }

    #[test]
    fn zero_canvas_does_not_move_the_camera() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(10.0, 10.0),
            Vec2::new(40.0, 20.0),
        ));
        let mut viewport = Viewport::new(Vec2::new(800.0, 600.0));
        viewport.resize(Vec2::ZERO);
        viewport.move_to(Vec2::new(5.0, 6.0));
        assert!(!zoom_viewport_to_scene(&mut viewport, &scene));
        assert_eq!(viewport.center(), Vec2::new(5.0, 6.0));
        assert_eq!(viewport.zoom(), 1.0);
    }
}
