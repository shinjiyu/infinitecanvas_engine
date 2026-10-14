//! 选中元素的样式读取和修改。
//!
//! 矩形、圆、直线、路径共用 [`ElementStyle`]。文本另外记录字号、文字颜色、
//! 不透明度和可选描边。组没有自己的样式：读和写都会展开到子孙叶子。
//!
//! ```text
//! 选中 id ──► style_target_ids（组展开为子孙）
//!                 │
//!                 ├─ selection_style  多选不一致 → StyleField::Mixed
//!                 └─ StylePatch ──► SetStyleCommand ──► mark_dirty
//!                                      │
//!                                      └─ 文本字号 / 颜色 / 描边变化时丢掉光栅
//! ```

use crate::element::{ElementId, ElementStyle};
use crate::scene::{AnyElement, Scene};
use infinitecanvas_math::Color;

/// 一个样式字段在当前选择上的取值。
#[derive(Debug, Clone, PartialEq)]
pub enum StyleField<T> {
    /// 选中的元素都没有这个字段。
    Absent,
    /// 所有支持该字段的元素取值相同。
    Uniform(T),
    /// 支持该字段的元素取值不一致。
    Mixed,
}

/// 文本上属性面板能改的外观。不含内容、对齐和字体，那些走文本编辑命令。
#[derive(Debug, Clone, PartialEq)]
pub struct TextPaint {
    /// 世界单位字号。
    pub font_size: f32,
    /// 填充色。
    pub color: Color,
    /// 元素不透明度，乘在纹理 tint 上。
    pub opacity: f32,
    /// 描边色。`None` 时若线宽大于 0，光栅用填充色描边。
    pub stroke: Option<Color>,
    /// 世界单位描边宽度。0 表示不描边。
    pub stroke_width: f32,
}

/// 一个元素上可撤销的样式快照。
#[derive(Debug, Clone, PartialEq)]
pub enum StyleSnapshot {
    /// 矩形、圆、直线或路径。
    Shape(ElementStyle),
    /// 文本。
    Text(TextPaint),
}

/// 一次样式修改。`None` 表示这个字段不动。
///
/// 颜色用双层 [`Option`]：外层 `None` 表示不改，`Some(None)` 表示清掉填充或描边。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StylePatch {
    /// 形状填充色。
    pub fill: Option<Option<Color>>,
    /// 形状或文本的描边色。
    pub stroke: Option<Option<Color>>,
    /// 形状或文本的描边宽度。
    pub stroke_width: Option<f32>,
    /// 形状或文本的不透明度，范围 0 到 1。
    pub opacity: Option<f32>,
    /// 文本字号（世界单位）。
    pub font_size: Option<f32>,
    /// 文本填充色。
    pub text_color: Option<Color>,
}

/// 当前选择的公共样式。
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionStyle {
    /// 形状填充。文本不参与。
    pub fill: StyleField<Option<Color>>,
    /// 形状和文本的描边色。
    pub stroke: StyleField<Option<Color>>,
    /// 形状和文本的描边宽度。
    pub stroke_width: StyleField<f32>,
    /// 形状 [`ElementStyle::opacity`] 和文本不透明度。
    pub opacity: StyleField<f32>,
    /// 文本字号。
    pub font_size: StyleField<f32>,
    /// 文本填充色。
    pub text_color: StyleField<Color>,
    /// 选择里有形状（含组内的形状）。
    pub has_shape: bool,
    /// 选择里有文本（含组内的文本）。
    pub has_text: bool,
}

impl SelectionStyle {
    /// 没有任何可编辑样式。
    pub fn is_empty(&self) -> bool {
        !self.has_shape && !self.has_text
    }
}

impl StyleSnapshot {
    /// 把补丁写进快照。
    ///
    /// # 返回
    /// 至少有一个字段发生变化时为 true。非有限数字会被忽略。
    pub fn apply_patch(&mut self, patch: &StylePatch) -> bool {
        let original = self.clone();
        match self {
            Self::Shape(style) => {
                if let Some(fill) = patch.fill {
                    style.fill = fill;
                }
                if let Some(stroke) = patch.stroke {
                    style.stroke = stroke;
                }
                if let Some(width) = finite_width(patch.stroke_width) {
                    style.stroke_width = width;
                }
                if let Some(opacity) = finite_opacity(patch.opacity) {
                    style.opacity = opacity;
                }
            }
            Self::Text(text) => {
                if let Some(color) = patch.text_color {
                    text.color = color;
                }
                if let Some(size) = finite_font(patch.font_size) {
                    text.font_size = size;
                }
                if let Some(opacity) = finite_opacity(patch.opacity) {
                    text.opacity = opacity;
                }
                if let Some(stroke) = patch.stroke {
                    text.stroke = stroke;
                }
                if let Some(width) = finite_width(patch.stroke_width) {
                    text.stroke_width = width;
                }
            }
        }
        *self != original
    }
}

/// 选择里要改样式的元素。组会展开成自己和全部子孙，重复 id 只保留第一次。
pub fn style_target_ids(scene: &Scene, ids: &[ElementId]) -> Vec<ElementId> {
    let mut out = Vec::new();
    for id in ids {
        for node in scene.subtree_ids(*id) {
            if !out.contains(&node) {
                out.push(node);
            }
        }
    }
    out
}

/// 元素自己或某个祖先被锁定。
pub fn style_locked(scene: &Scene, id: ElementId) -> bool {
    let mut current = Some(id);
    while let Some(element_id) = current {
        let Some(element) = scene.get(element_id) else {
            return true;
        };
        if element.as_element().props().locked {
            return true;
        }
        current = element.as_element().props().parent;
    }
    false
}

/// 读取一个元素的样式。图片和组返回 `None`。
pub fn capture_style(scene: &Scene, id: ElementId) -> Option<StyleSnapshot> {
    match scene.get(id)? {
        AnyElement::Rectangle(rect) => Some(StyleSnapshot::Shape(rect.style.clone())),
        AnyElement::Circle(circle) => Some(StyleSnapshot::Shape(circle.style.clone())),
        AnyElement::Line(line) => Some(StyleSnapshot::Shape(line.style.clone())),
        AnyElement::Path(path) => Some(StyleSnapshot::Shape(path.style.clone())),
        AnyElement::Text(text) => Some(StyleSnapshot::Text(TextPaint {
            font_size: text.font_size,
            color: text.color,
            opacity: text.opacity,
            stroke: text.stroke,
            stroke_width: text.stroke_width,
        })),
        AnyElement::Image(_) | AnyElement::Group(_) => None,
    }
}

/// 写回样式快照，并 [`Scene::mark_dirty`]。
///
/// 文本的字号、颜色或描边变化会丢掉光栅纹理。不透明度只改 tint，不重新光栅化。
pub fn apply_style_snapshot(scene: &mut Scene, id: ElementId, snapshot: &StyleSnapshot) {
    let applied = match scene.get_mut(id) {
        Some(AnyElement::Rectangle(rect)) => assign_shape(&mut rect.style, snapshot),
        Some(AnyElement::Circle(circle)) => assign_shape(&mut circle.style, snapshot),
        Some(AnyElement::Line(line)) => assign_shape(&mut line.style, snapshot),
        Some(AnyElement::Path(path)) => assign_shape(&mut path.style, snapshot),
        Some(AnyElement::Text(text)) => {
            if let StyleSnapshot::Text(paint) = snapshot {
                text.set_paint(
                    paint.font_size,
                    paint.color,
                    paint.opacity,
                    paint.stroke,
                    paint.stroke_width,
                );
                true
            } else {
                false
            }
        }
        _ => false,
    };
    if applied {
        scene.mark_dirty(id);
    }
}

/// 汇总选择的公共样式。锁定元素也参与显示，但命令不会改它们。
pub fn selection_style(scene: &Scene, ids: &[ElementId]) -> SelectionStyle {
    let mut fills = Vec::new();
    let mut strokes = Vec::new();
    let mut widths = Vec::new();
    let mut opacities = Vec::new();
    let mut font_sizes = Vec::new();
    let mut text_colors = Vec::new();
    let mut has_shape = false;
    let mut has_text = false;

    for id in style_target_ids(scene, ids) {
        match capture_style(scene, id) {
            Some(StyleSnapshot::Shape(style)) => {
                has_shape = true;
                fills.push(style.fill);
                strokes.push(style.stroke);
                widths.push(style.stroke_width);
                opacities.push(style.opacity);
            }
            Some(StyleSnapshot::Text(text)) => {
                has_text = true;
                strokes.push(text.stroke);
                widths.push(text.stroke_width);
                opacities.push(text.opacity);
                font_sizes.push(text.font_size);
                text_colors.push(text.color);
            }
            None => {}
        }
    }

    SelectionStyle {
        fill: fold(fills),
        stroke: fold(strokes),
        stroke_width: fold(widths),
        opacity: fold(opacities),
        font_size: fold(font_sizes),
        text_color: fold(text_colors),
        has_shape,
        has_text,
    }
}

fn assign_shape(style: &mut ElementStyle, snapshot: &StyleSnapshot) -> bool {
    if let StyleSnapshot::Shape(next) = snapshot {
        *style = next.clone();
        true
    } else {
        false
    }
}

fn fold<T: PartialEq>(values: Vec<T>) -> StyleField<T> {
    let mut iter = values.into_iter();
    let Some(first) = iter.next() else {
        return StyleField::Absent;
    };
    if iter.all(|value| value == first) {
        StyleField::Uniform(first)
    } else {
        StyleField::Mixed
    }
}

fn finite_opacity(value: Option<f32>) -> Option<f32> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0))
}

fn finite_width(value: Option<f32>) -> Option<f32> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.max(0.0))
}

fn finite_font(value: Option<f32>) -> Option<f32> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.max(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{Line, Path, Rectangle, Text};
    use crate::{Circle, Command, CommandHistory, SetStyleCommand, VertexCache};
    use infinitecanvas_math::Vec2;

    fn shape_fill(scene: &Scene, id: ElementId) -> Option<Color> {
        match capture_style(scene, id) {
            Some(StyleSnapshot::Shape(style)) => style.fill,
            _ => panic!("shape"),
        }
    }

    #[test]
    fn mixed_selection_reports_only_fields_that_differ() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0))
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        scene.add_rect(
            Rectangle::new(2, Vec2::new(20.0, 0.0), Vec2::new(10.0, 10.0))
                .with_style(ElementStyle::fill_only(Color::BLUE)),
        );

        let style = selection_style(&scene, &[1, 2]);
        assert_eq!(style.fill, StyleField::Mixed);
        assert_eq!(style.opacity, StyleField::Uniform(1.0));
        assert_eq!(style.stroke_width, StyleField::Uniform(1.0));
        assert!(style.has_shape);
        assert!(!style.has_text);
        assert_eq!(style.font_size, StyleField::Absent);
    }

    #[test]
    fn group_selection_reads_and_edits_descendants() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0))
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        scene.add_circle(
            Circle::new(2, Vec2::new(30.0, 0.0), 8.0)
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        let group_id = scene.group(&[1, 2]).unwrap();
        let style = selection_style(&scene, &[group_id]);
        assert_eq!(style.fill, StyleField::Uniform(Some(Color::RED)));

        let patch = StylePatch {
            fill: Some(Some(Color::GREEN)),
            ..StylePatch::default()
        };
        let mut command = SetStyleCommand::from_patch(&scene, &[group_id], &patch).unwrap();
        command.execute(&mut scene);
        assert_eq!(shape_fill(&scene, 1), Some(Color::GREEN));
        assert_eq!(shape_fill(&scene, 2), Some(Color::GREEN));
        command.undo(&mut scene);
        assert_eq!(shape_fill(&scene, 1), Some(Color::RED));
    }

    #[test]
    fn locked_element_is_shown_but_not_changed() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0))
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        scene.set_locked(1, true);
        let patch = StylePatch {
            fill: Some(Some(Color::BLUE)),
            ..StylePatch::default()
        };
        assert!(SetStyleCommand::from_patch(&scene, &[1], &patch).is_none());
        assert_eq!(shape_fill(&scene, 1), Some(Color::RED));
    }

    #[test]
    fn style_json_round_trip_keeps_shape_and_text_and_old_files_import() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::new(4.0, 5.0), Vec2::new(12.0, 8.0)));
        scene.add_line(Line::from_world(
            2,
            Vec2::ZERO,
            Vec2::new(30.0, 0.0),
            ElementStyle::stroke_only(Color::BLACK, 2.0),
            false,
        ));
        scene.add_path(Path::from_world_points(
            3,
            &[Vec2::ZERO, Vec2::new(10.0, 4.0), Vec2::new(16.0, 1.0)],
            ElementStyle::stroke_only(Color::BLACK, 2.0),
        ));
        scene.add_text(Text::new(4, Vec2::new(8.0, 8.0), "Hi"));

        let patch = StylePatch {
            fill: Some(Some(Color::RED)),
            stroke: Some(Some(Color::BLUE)),
            stroke_width: Some(6.0),
            opacity: Some(0.5),
            font_size: Some(20.0),
            text_color: Some(Color::GREEN),
        };
        let mut command = SetStyleCommand::from_patch(&scene, &[1, 2, 3, 4], &patch).unwrap();
        command.execute(&mut scene);

        let json = scene.to_json().unwrap();
        let restored = Scene::from_json(&json).unwrap();
        match restored.get(1).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.style.fill, Some(Color::RED));
                assert_eq!(rect.style.stroke, Some(Color::BLUE));
                assert_eq!(rect.style.stroke_width, 6.0);
                assert_eq!(rect.style.opacity, 0.5);
            }
            other => panic!("rect, got {other:?}"),
        }
        match restored.get(2).unwrap() {
            AnyElement::Line(line) => {
                assert_eq!(line.style.stroke, Some(Color::BLUE));
                assert_eq!(line.style.stroke_width, 6.0);
                assert_eq!(line.style.opacity, 0.5);
            }
            other => panic!("line, got {other:?}"),
        }
        match restored.get(4).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.font_size, 20.0);
                assert_eq!(text.color, Color::GREEN);
                assert_eq!(text.opacity, 0.5);
                assert_eq!(text.stroke, Some(Color::BLUE));
                assert_eq!(text.stroke_width, 6.0);
            }
            other => panic!("text, got {other:?}"),
        }

        let size = serde_json::to_value(Vec2::new(4.0, 4.0)).unwrap();
        let legacy = serde_json::json!({
            "version": 1,
            "elements": [
                {
                    "type": "rectangle",
                    "data": { "props": { "id": 1 }, "size": size }
                },
                {
                    "type": "text",
                    "data": { "props": { "id": 2 }, "content": "Old" }
                }
            ]
        });
        let legacy_scene = Scene::from_json(&legacy.to_string()).unwrap();
        match legacy_scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => assert_eq!(rect.style, ElementStyle::new()),
            other => panic!("rect, got {other:?}"),
        }
        match legacy_scene.get(2).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.content, "Old");
                assert_eq!(text.font_size, 32.0);
                assert_eq!(text.opacity, 1.0);
                assert_eq!(text.stroke, None);
                assert_eq!(text.stroke_width, 0.0);
            }
            other => panic!("text, got {other:?}"),
        }
    }

    #[test]
    fn opacity_and_stroke_width_refresh_cached_vertices() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0))
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        let mut cache = VertexCache::new();
        let _ = cache.sync(&mut scene);
        assert!(!scene.has_dirty_elements());

        let patch = StylePatch {
            opacity: Some(0.25),
            stroke: Some(Some(Color::BLUE)),
            stroke_width: Some(8.0),
            ..StylePatch::default()
        };
        let mut history = CommandHistory::new();
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[1], &patch).unwrap()),
            &mut scene,
        );
        assert!(scene.has_dirty_elements());
        let _ = cache.sync(&mut scene);
        let vertices = cache.fill_slice(1).unwrap();
        assert!(vertices.len() > 6);
        assert!(vertices
            .iter()
            .any(|vertex| (vertex.color[3] - 0.25).abs() < 1.0e-5));
        assert!(vertices.iter().any(|vertex| vertex.position[0].abs() > 5.0));
    }
}
