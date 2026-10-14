//! 吸管：从世界坐标采样填充/描边，并写到当前选择。
//!
//! 采样不读帧缓冲。命中最上层可见叶子，读它的样式。
//! 应用走 [`SetStyleCommand`]，多选是一条撤销记录。
//!
//! ```text
//! 世界点
//!   │  可见叶子，z 大者在上，z 相同则 id 大者在上
//!   ▼
//! SampledPaint { fill, stroke }
//!   │  填充 → 形状 fill + 文本颜色；描边 → stroke
//!   ▼
//! 可见且未锁定的叶子 ── SetStyleCommand ── 一条历史
//! ```

use crate::command::{CommandHistory, SetStyleCommand};
use crate::element::{ElementId, ElementStyle, FillKind};
use crate::hit_test::{hit_test_point, HitTestOptions};
use crate::math::Vec2;
use crate::style::{capture_style, style_locked, style_target_ids, StylePatch, StyleSnapshot};
use crate::Scene;
use infinitecanvas_math::Color;

/// 某一点上最上层可见叶子的填充和描边。
///
/// 文本的填充是文字颜色。图片两个通道都是 `None`。
/// 缺的通道表示元素没有这种颜色，不是“清掉目标上的颜色”。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampledPaint {
    /// 被采样的叶子。
    pub id: ElementId,
    /// 形状填充，或文本颜色。没有时为 `None`。
    pub fill: Option<Color>,
    /// 形状或文本的描边色。没有时为 `None`。
    pub stroke: Option<Color>,
}

impl SampledPaint {
    /// 变成样式补丁。
    ///
    /// 有填充时同时写形状 `fill` 和文本 `text_color`，并把形状收成纯色，
    /// 这样采到的颜色会盖过原来的渐变。有描边时写 `stroke`。
    /// 缺的通道保持 `None`，不会把目标颜色清掉。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::SampledPaint;
    /// use infinitecanvas_math::Color;
    ///
    /// let sample = SampledPaint {
    ///     id: 1,
    ///     fill: Some(Color::RED),
    ///     stroke: None,
    /// };
    /// let patch = sample.to_style_patch();
    /// assert_eq!(patch.fill, Some(Some(Color::RED)));
    /// assert_eq!(patch.text_color, Some(Color::RED));
    /// assert!(patch.stroke.is_none());
    /// ```
    pub fn to_style_patch(self) -> StylePatch {
        StylePatch {
            fill: self.fill.map(Some),
            stroke: self.stroke.map(Some),
            text_color: self.fill,
            fill_kind: self.fill.map(|_| FillKind::Solid),
            ..StylePatch::default()
        }
    }
}

/// 采样 `point` 处最上层的可见叶子。
///
/// 隐藏元素（含被隐藏的祖先挡住的）跳过。锁定元素仍可采样，因为它们还画在画面上。
/// 组没有填充，不会抢走命中。叠在一起时，`z_index` 大的在上；`z_index` 相同则 id 大的在上，
/// 和顶点缓存的绘制顺序一致。点在空白处，或坐标不是有限值时返回 `None`。
/// 点到图片时返回该图片，填充和描边都是 `None`。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{sample_paint_at, ElementStyle, Rectangle, Scene};
/// use infinitecanvas_math::{Color, Vec2};
///
/// let mut scene = Scene::new();
/// scene.add_rect(
///     Rectangle::new(1, Vec2::ZERO, Vec2::new(20.0, 20.0))
///         .with_style(ElementStyle::fill_only(Color::RED)),
/// );
/// let sample = sample_paint_at(&scene, Vec2::ZERO).unwrap();
/// assert_eq!(sample.id, 1);
/// assert_eq!(sample.fill, Some(Color::RED));
/// assert!(sample_paint_at(&scene, Vec2::new(100.0, 100.0)).is_none());
/// ```
pub fn sample_paint_at(scene: &Scene, point: Vec2) -> Option<SampledPaint> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return None;
    }
    let options = HitTestOptions {
        max_results: usize::MAX,
        include_invisible: false,
        include_locked: true,
    };
    let hits = hit_test_point(scene, point, &options);
    let hit = hits
        .into_iter()
        .filter(|hit| {
            scene
                .get(hit.id)
                .is_some_and(|element| element.as_group().is_none())
        })
        .max_by_key(|hit| (hit.z_index, hit.id))?;
    let Some(snapshot) = capture_style(scene, hit.id) else {
        return Some(SampledPaint {
            id: hit.id,
            fill: None,
            stroke: None,
        });
    };
    Some(paint_from_snapshot(scene, hit.id, &snapshot, point))
}

/// 把采样到的颜色写到 `selected_ids`。
///
/// 组展开成叶子。不可见（含祖先被隐藏）或已锁定（含祖先被锁定）的目标跳过。
/// 图片没有填充和描边，补丁写上去也不会变化。
/// 空选、全部锁定、全部隐藏、采样没有颜色，或颜色和目标相同，都返回 `false`，不入栈。
/// 有变化时整批目标合成一条 [`SetStyleCommand`]。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{
///     apply_sampled_paint, sample_paint_at, CommandHistory, ElementStyle, Rectangle, Scene,
/// };
/// use infinitecanvas_math::{Color, Vec2};
///
/// let mut scene = Scene::new();
/// scene.add_rect(
///     Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0))
///         .with_style(ElementStyle::fill_only(Color::RED)),
/// );
/// scene.add_rect(
///     Rectangle::new(2, Vec2::new(40.0, 0.0), Vec2::new(10.0, 10.0))
///         .with_style(ElementStyle::fill_only(Color::BLUE)),
/// );
/// let sample = sample_paint_at(&scene, Vec2::ZERO).unwrap();
/// let mut history = CommandHistory::new();
/// assert!(apply_sampled_paint(&mut history, &mut scene, &[2], &sample));
/// assert_eq!(history.undo_count(), 1);
/// ```
pub fn apply_sampled_paint(
    history: &mut CommandHistory,
    scene: &mut Scene,
    selected_ids: &[ElementId],
    sample: &SampledPaint,
) -> bool {
    if sample.fill.is_none() && sample.stroke.is_none() {
        return false;
    }
    let targets = editable_leaves(scene, selected_ids);
    if targets.is_empty() {
        return false;
    }
    let Some(command) = SetStyleCommand::from_patch(scene, &targets, &sample.to_style_patch())
    else {
        return false;
    };
    history.execute(Box::new(command), scene);
    true
}

fn editable_leaves(scene: &Scene, selected_ids: &[ElementId]) -> Vec<ElementId> {
    style_target_ids(scene, selected_ids)
        .into_iter()
        .filter(|id| capture_style(scene, *id).is_some())
        .filter(|id| scene.is_effectively_visible(*id) && !style_locked(scene, *id))
        .collect()
}

fn paint_from_snapshot(
    scene: &Scene,
    id: ElementId,
    snapshot: &StyleSnapshot,
    point: Vec2,
) -> SampledPaint {
    match snapshot {
        StyleSnapshot::Shape(style)
        | StyleSnapshot::Rectangle { style, .. }
        | StyleSnapshot::Polygon { style, .. } => SampledPaint {
            id,
            fill: fill_at(scene, id, style, point),
            stroke: style.stroke,
        },
        StyleSnapshot::Text(text) => SampledPaint {
            id,
            fill: Some(text.color),
            stroke: text.stroke,
        },
        StyleSnapshot::Image { .. } => SampledPaint {
            id,
            fill: None,
            stroke: None,
        },
    }
}

/// 渐变上取该点的颜色。纯色仍读 `fill`。逆变换失败时退回纯色通道或第一个色标。
fn fill_at(scene: &Scene, id: ElementId, style: &ElementStyle, point: Vec2) -> Option<Color> {
    if !style.uses_gradient() {
        return style.fill;
    }
    let Some(element) = scene.get(id) else {
        return style.fill;
    };
    let local = element
        .as_element()
        .props()
        .transform
        .inverse_transform_point(point);
    if local.x.is_finite() && local.y.is_finite() {
        return style.sample_fill(local, element.as_element().local_bounds());
    }
    style
        .fill
        .or_else(|| style.normalized_stops().first().map(|stop| stop.color))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{ElementStyle, Image, Polygon, PolygonKind, Rectangle, Text};
    use crate::style::{capture_style, StyleSnapshot};
    use infinitecanvas_math::Color;

    fn shape_style(scene: &Scene, id: ElementId) -> ElementStyle {
        match capture_style(scene, id) {
            Some(
                StyleSnapshot::Shape(style)
                | StyleSnapshot::Rectangle { style, .. }
                | StyleSnapshot::Polygon { style, .. },
            ) => style,
            other => panic!("shape {id}, got {other:?}"),
        }
    }

    fn text_paint(scene: &Scene, id: ElementId) -> crate::style::TextPaint {
        match capture_style(scene, id) {
            Some(StyleSnapshot::Text(paint)) => paint,
            other => panic!("text {id}, got {other:?}"),
        }
    }

    fn rect(id: ElementId, at: Vec2, fill: Color, stroke: Option<Color>, z: i32) -> Rectangle {
        let mut style = ElementStyle::fill_only(fill);
        style.stroke = stroke;
        style.stroke_width = 2.0;
        let mut rect = Rectangle::new(id, at, Vec2::new(40.0, 40.0)).with_style(style);
        rect.props.z_index = z;
        rect
    }

    #[test]
    fn topmost_visible_leaf_wins_and_empty_space_is_clear() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO, Color::RED, Some(Color::BLACK), 1));
        scene.add_rect(rect(2, Vec2::ZERO, Color::BLUE, None, 5));
        let mut hidden = rect(3, Vec2::ZERO, Color::GREEN, Some(Color::WHITE), 9);
        hidden.props.visible = false;
        scene.add_rect(hidden);

        let sample = sample_paint_at(&scene, Vec2::ZERO).unwrap();
        assert_eq!(sample.id, 2);
        assert_eq!(sample.fill, Some(Color::BLUE));
        assert_eq!(sample.stroke, None);

        assert!(sample_paint_at(&scene, Vec2::new(400.0, 400.0)).is_none());
        assert!(sample_paint_at(&scene, Vec2::new(f32::NAN, 0.0)).is_none());
    }

    #[test]
    fn equal_z_uses_higher_id_and_locked_source_still_samples() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO, Color::RED, None, 0));
        scene.add_rect(rect(4, Vec2::ZERO, Color::BLUE, None, 0));
        assert_eq!(sample_paint_at(&scene, Vec2::ZERO).unwrap().id, 4);

        let mut locked = rect(2, Vec2::new(80.0, 0.0), Color::GREEN, None, 1);
        locked.props.locked = true;
        scene.add_rect(locked);
        scene.add_rect(rect(3, Vec2::new(80.0, 0.0), Color::WHITE, None, 0));
        let sample = sample_paint_at(&scene, Vec2::new(80.0, 0.0)).unwrap();
        assert_eq!(sample.id, 2);
        assert_eq!(sample.fill, Some(Color::GREEN));
    }

    #[test]
    fn hidden_ancestor_is_skipped_and_group_does_not_steal_the_hit() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO, Color::RED, None, 1));
        scene.add_rect(rect(2, Vec2::new(80.0, 0.0), Color::BLUE, None, 1));
        let group = scene.group(&[1, 2]).unwrap();
        assert_eq!(
            sample_paint_at(&scene, Vec2::ZERO).unwrap().fill,
            Some(Color::RED)
        );

        assert!(scene.set_visible(group, false));
        assert!(sample_paint_at(&scene, Vec2::ZERO).is_none());
        assert!(sample_paint_at(&scene, Vec2::new(80.0, 0.0)).is_none());
    }

    #[test]
    fn text_fill_is_the_glyph_color_and_image_returns_clear_nulls() {
        let mut scene = Scene::new();
        let mut text = Text::new(1, Vec2::ZERO, "Hi");
        text.color = Color::GREEN;
        text.stroke = Some(Color::BLUE);
        text.props.z_index = 1;
        scene.add_text(text);

        let sample = sample_paint_at(&scene, Vec2::new(8.0, 8.0)).unwrap();
        assert_eq!(sample.id, 1);
        assert_eq!(sample.fill, Some(Color::GREEN));
        assert_eq!(sample.stroke, Some(Color::BLUE));

        let mut image = Image::new(
            2,
            Vec2::new(200.0, 0.0),
            Vec2::new(40.0, 40.0),
            "tex".into(),
        );
        image.props.z_index = 4;
        scene.add_image(image);
        scene.add_rect(rect(
            3,
            Vec2::new(220.0, 20.0),
            Color::RED,
            Some(Color::BLACK),
            1,
        ));
        let sample = sample_paint_at(&scene, Vec2::new(210.0, 10.0)).unwrap();
        assert_eq!(sample.id, 2);
        assert_eq!(sample.fill, None);
        assert_eq!(sample.stroke, None);
    }

    #[test]
    fn polygon_and_star_sample_fill_and_stroke_without_changing_sides() {
        let mut scene = Scene::new();
        let mut style = ElementStyle::fill_only(Color::RED);
        style.stroke = Some(Color::BLUE);
        scene.add_polygon(Polygon::star(1, Vec2::ZERO, 20.0).with_style(style));

        let sample = sample_paint_at(&scene, Vec2::ZERO).unwrap();
        assert_eq!(sample.id, 1);
        assert_eq!(sample.fill, Some(Color::RED));
        assert_eq!(sample.stroke, Some(Color::BLUE));

        scene.add_rect(rect(2, Vec2::new(80.0, 0.0), Color::GREEN, None, 1));
        let mut history = CommandHistory::new();
        assert!(apply_sampled_paint(&mut history, &mut scene, &[2], &sample));
        assert_eq!(shape_style(&scene, 2).fill, Some(Color::RED));
        assert_eq!(shape_style(&scene, 2).stroke, Some(Color::BLUE));
        match capture_style(&scene, 1) {
            Some(StyleSnapshot::Polygon { sides, kind, .. }) => {
                assert_eq!(sides, 5);
                assert_eq!(kind, PolygonKind::Star);
            }
            other => panic!("star, got {other:?}"),
        }
    }

    #[test]
    fn multi_selection_is_one_undo_and_selected_source_copies_onto_the_others() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO, Color::RED, Some(Color::BLACK), 1));
        let mut target =
            rect(2, Vec2::new(80.0, 0.0), Color::BLUE, None, 1).with_corner_radius(6.0);
        target.style.stroke = Some(Color::WHITE);
        scene.add_rect(target);
        scene.add_rect(rect(3, Vec2::new(160.0, 0.0), Color::GREEN, None, 1));

        let sample = sample_paint_at(&scene, Vec2::ZERO).unwrap();
        let mut history = CommandHistory::new();
        assert!(apply_sampled_paint(
            &mut history,
            &mut scene,
            &[1, 2, 3],
            &sample
        ));
        assert_eq!(history.undo_count(), 1);
        assert_eq!(shape_style(&scene, 1).fill, Some(Color::RED));
        assert_eq!(shape_style(&scene, 2).fill, Some(Color::RED));
        assert_eq!(shape_style(&scene, 2).stroke, Some(Color::BLACK));
        assert_eq!(shape_style(&scene, 3).fill, Some(Color::RED));
        assert_eq!(shape_style(&scene, 3).stroke, Some(Color::BLACK));
        match capture_style(&scene, 2) {
            Some(StyleSnapshot::Rectangle { corner_radius, .. }) => {
                assert!((corner_radius - 6.0).abs() < 1e-4);
            }
            other => panic!("rect, got {other:?}"),
        }

        assert!(history.undo(&mut scene));
        assert_eq!(shape_style(&scene, 2).fill, Some(Color::BLUE));
        assert_eq!(shape_style(&scene, 2).stroke, Some(Color::WHITE));
        assert_eq!(shape_style(&scene, 3).fill, Some(Color::GREEN));
        assert_eq!(shape_style(&scene, 3).stroke, None);
        assert_eq!(history.undo_count(), 0);
        assert!(history.redo(&mut scene));
        assert_eq!(shape_style(&scene, 3).fill, Some(Color::RED));
    }

    #[test]
    fn stroke_only_sample_does_not_clear_existing_fill() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::ZERO, Vec2::new(30.0, 30.0))
                .with_style(ElementStyle::stroke_only(Color::BLACK, 4.0)),
        );
        scene.add_rect(rect(
            2,
            Vec2::new(80.0, 0.0),
            Color::RED,
            Some(Color::BLUE),
            1,
        ));
        let sample = sample_paint_at(&scene, Vec2::ZERO).unwrap();
        assert_eq!(sample.fill, None);
        assert_eq!(sample.stroke, Some(Color::BLACK));

        let mut history = CommandHistory::new();
        assert!(apply_sampled_paint(&mut history, &mut scene, &[2], &sample));
        assert_eq!(shape_style(&scene, 2).fill, Some(Color::RED));
        assert_eq!(shape_style(&scene, 2).stroke, Some(Color::BLACK));
    }

    #[test]
    fn text_target_takes_sampled_fill_as_text_color() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO, Color::RED, Some(Color::BLUE), 1));
        let mut text = Text::new(2, Vec2::new(80.0, 0.0), "Hi");
        text.color = Color::WHITE;
        scene.add_text(text);

        let sample = sample_paint_at(&scene, Vec2::ZERO).unwrap();
        let mut history = CommandHistory::new();
        assert!(apply_sampled_paint(&mut history, &mut scene, &[2], &sample));
        let paint = text_paint(&scene, 2);
        assert_eq!(paint.color, Color::RED);
        assert_eq!(paint.stroke, Some(Color::BLUE));
        assert!(history.undo(&mut scene));
        assert_eq!(text_paint(&scene, 2).color, Color::WHITE);
        assert_eq!(text_paint(&scene, 2).stroke, None);
    }

    #[test]
    fn grouped_selection_updates_visible_leaves_in_one_command() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO, Color::RED, None, 1));
        scene.add_rect(rect(2, Vec2::new(70.0, 0.0), Color::BLUE, None, 1));
        scene.add_rect(rect(3, Vec2::new(200.0, 0.0), Color::GREEN, None, 1));
        let group = scene.group(&[1, 2]).unwrap();
        let sample = sample_paint_at(&scene, Vec2::new(200.0, 0.0)).unwrap();
        let mut history = CommandHistory::new();
        assert!(apply_sampled_paint(
            &mut history,
            &mut scene,
            &[group],
            &sample
        ));
        assert_eq!(history.undo_count(), 1);
        assert_eq!(shape_style(&scene, 1).fill, Some(Color::GREEN));
        assert_eq!(shape_style(&scene, 2).fill, Some(Color::GREEN));
    }

    #[test]
    fn empty_locked_hidden_or_unsupported_selection_is_noop() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO, Color::RED, None, 1));
        scene.add_rect(rect(2, Vec2::new(80.0, 0.0), Color::BLUE, None, 1));
        scene.add_image(Image::new(
            3,
            Vec2::new(160.0, 0.0),
            Vec2::new(20.0, 20.0),
            "tex".into(),
        ));
        let sample = sample_paint_at(&scene, Vec2::ZERO).unwrap();
        let mut history = CommandHistory::new();

        assert!(!apply_sampled_paint(&mut history, &mut scene, &[], &sample));
        assert_eq!(history.undo_count(), 0);

        assert!(scene.set_locked(2, true));
        assert!(!apply_sampled_paint(
            &mut history,
            &mut scene,
            &[2],
            &sample
        ));
        assert_eq!(shape_style(&scene, 2).fill, Some(Color::BLUE));
        assert!(scene.set_locked(2, false));

        assert!(scene.set_visible(2, false));
        assert!(!apply_sampled_paint(
            &mut history,
            &mut scene,
            &[2],
            &sample
        ));
        assert_eq!(shape_style(&scene, 2).fill, Some(Color::BLUE));
        assert!(scene.set_visible(2, true));

        let image_sample = sample_paint_at(&scene, Vec2::new(170.0, 10.0)).unwrap();
        assert!(!apply_sampled_paint(
            &mut history,
            &mut scene,
            &[2],
            &image_sample
        ));
        assert!(!apply_sampled_paint(
            &mut history,
            &mut scene,
            &[3],
            &sample
        ));
        assert_eq!(history.undo_count(), 0);
        assert_eq!(shape_style(&scene, 2).fill, Some(Color::BLUE));
    }

    #[test]
    fn mixed_lock_and_visibility_updates_only_the_editable_targets() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO, Color::RED, None, 1));
        scene.add_rect(rect(2, Vec2::new(80.0, 0.0), Color::BLUE, None, 1));
        scene.add_rect(rect(3, Vec2::new(160.0, 0.0), Color::GREEN, None, 1));
        scene.add_rect(rect(4, Vec2::new(240.0, 0.0), Color::WHITE, None, 1));
        assert!(scene.set_locked(3, true));
        assert!(scene.set_visible(4, false));

        let sample = sample_paint_at(&scene, Vec2::ZERO).unwrap();
        let mut history = CommandHistory::new();
        assert!(apply_sampled_paint(
            &mut history,
            &mut scene,
            &[2, 3, 4],
            &sample
        ));
        assert_eq!(history.undo_count(), 1);
        assert_eq!(shape_style(&scene, 2).fill, Some(Color::RED));
        assert_eq!(shape_style(&scene, 3).fill, Some(Color::GREEN));
        assert_eq!(shape_style(&scene, 4).fill, Some(Color::WHITE));
        assert!(history.undo(&mut scene));
        assert_eq!(shape_style(&scene, 2).fill, Some(Color::BLUE));
    }

    #[test]
    fn same_color_does_not_enter_history() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO, Color::RED, None, 1));
        scene.add_rect(rect(2, Vec2::new(80.0, 0.0), Color::RED, None, 1));
        let sample = sample_paint_at(&scene, Vec2::ZERO).unwrap();
        let mut history = CommandHistory::new();
        assert!(!apply_sampled_paint(
            &mut history,
            &mut scene,
            &[2],
            &sample
        ));
        assert_eq!(history.undo_count(), 0);
    }

    #[test]
    fn gradient_sample_is_the_color_under_the_point_and_apply_flattens() {
        use crate::element::GradientStop;

        let mut scene = Scene::new();
        let mut source = Rectangle::new(1, Vec2::ZERO, Vec2::new(20.0, 20.0));
        source.style.fill_kind = FillKind::Linear;
        source.style.fill_angle = 0.0;
        source.style.fill = Some(Color::WHITE);
        source.style.fill_stops = vec![
            GradientStop {
                offset: 0.0,
                color: Color::RED,
            },
            GradientStop {
                offset: 1.0,
                color: Color::BLUE,
            },
        ];
        scene.add_rect(source);
        scene.add_rect(rect(2, Vec2::new(40.0, 0.0), Color::GREEN, None, 1));

        let left = sample_paint_at(&scene, Vec2::new(-10.0, 0.0)).unwrap();
        assert_eq!(left.fill, Some(Color::RED));
        let right = sample_paint_at(&scene, Vec2::new(10.0, 0.0)).unwrap();
        assert_eq!(right.fill, Some(Color::BLUE));

        let mut history = CommandHistory::new();
        assert!(apply_sampled_paint(&mut history, &mut scene, &[2], &right));
        assert_eq!(history.undo_count(), 1);
        assert_eq!(shape_style(&scene, 2).fill, Some(Color::BLUE));
        assert_eq!(shape_style(&scene, 2).fill_kind, FillKind::Solid);

        assert!(apply_sampled_paint(&mut history, &mut scene, &[1], &left));
        assert_eq!(shape_style(&scene, 1).fill_kind, FillKind::Solid);
        assert_eq!(shape_style(&scene, 1).fill, Some(Color::RED));
        assert!(history.undo(&mut scene));
        assert_eq!(shape_style(&scene, 1).fill_kind, FillKind::Linear);
    }
}
