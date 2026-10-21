//! 选中元素的样式读取和修改。
//!
//! 矩形、圆、多边形、星形、直线、路径共用 [`ElementStyle`]。文本另外记录字号、文字颜色、
//! 不透明度、可选描边、水平对齐和字重。图片参与不透明度（[`crate::element::Image::alpha`]）和投影，
//! 填充和描边不写到图片上，资源表也不动。组没有自己的样式：读和写都会展开到子孙叶子。
//! 画板有自己的填充和描边；选中画板只改画板，不把样式写到子元素上。
//!
//! 投影、渐变、混合模式和图层模糊都是 [`StylePatch`] 上的可选字段，和圆角一样。没出现在补丁里就保持原值。
//! 吸管只写填充和描边，并把渐变收成纯色，不会清掉图层模糊或混合模式。
//!
//! ```text
//! 选中 id ──► style_target_ids（组展开为子孙，画板不展开）
//!                 │
//!                 ├─ selection_style  多选不一致 → StyleField::Mixed
//!                 └─ StylePatch ──► SetStyleCommand ──► mark_dirty
//!                                      │
//!                                      ├─ 文本字号 / 颜色 / 描边 / 对齐 / 字重变化时丢掉光栅
//!                                      ├─ 矩形圆角（其它元素忽略该字段）
//!                                      ├─ 多边形边数 / 星形内径（其它元素忽略）
//!                                      ├─ 形状渐变（种类、色标、角度、圆心、半径）
//!                                      ├─ 投影（形状、文本、图片、画板；不丢文本光栅）
//!                                      ├─ 混合模式写到形状、文本、图片和画板
//!                                      ├─ 图层模糊（同上；不丢文本光栅，不扩大命中盒，也不清渐变或混合模式）
//!                                      └─ 图片只改 alpha、投影、混合模式和图层模糊
//! ```

use crate::blend::BlendMode;
use crate::element::{
    DropShadow, ElementId, ElementStyle, FillKind, FontWeight, GradientStop, Line, LineMarker,
    Polygon, PolygonKind, Rectangle, StrokeAlign, StrokeCap, StrokeJoin, TextAlign,
};
use crate::scene::{AnyElement, Scene};
use infinitecanvas_math::{Color, Vec2};

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

/// 文本上属性面板能改的外观。不含内容和字体族，那些走文本编辑命令。
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
    /// 水平对齐。
    pub align: TextAlign,
    /// 字重。
    pub font_weight: FontWeight,
    /// 投影。文本不把它画进字形纹理。
    pub shadow: DropShadow,
    /// 混合模式。
    pub blend_mode: BlendMode,
    /// 图层模糊标准差。不进字形纹理。
    pub blur_radius: f32,
}

/// 一个元素上可撤销的样式快照。
#[derive(Debug, Clone, PartialEq)]
pub enum StyleSnapshot {
    /// 圆或路径。直线用 [`StyleSnapshot::Line`]，以便带上起止箭头。
    /// 矩形用 [`StyleSnapshot::Rectangle`]，以便带上圆角。
    /// 多边形和星形用 [`StyleSnapshot::Polygon`]。
    Shape(ElementStyle),
    /// 直线的画笔和起止箭头。一次补丁仍是一条撤销记录。
    Line {
        /// 描边色、线宽、虚线、线帽、线结、投影、模糊等。
        style: ElementStyle,
        /// 起点箭头。
        start_marker: LineMarker,
        /// 终点箭头。
        end_marker: LineMarker,
    },
    /// 矩形的画笔和圆角。一次补丁同时改这两项，撤销仍是一条记录。
    Rectangle {
        /// 填充、描边、线宽、不透明度。
        style: ElementStyle,
        /// 四角相同的半径。0 为直角。
        corner_radius: f32,
    },
    /// 正多边形或星形的画笔、边数和内径比。一次补丁仍是一条撤销记录。
    Polygon {
        /// 填充、描边、线宽、不透明度。
        style: ElementStyle,
        /// 边数或角数，已钳到 3–64。
        sides: u32,
        /// 星形内径比。正多边形保留原值，补丁不改它。
        inner_radius: f32,
        /// 种类不随样式补丁改变，只为了快照能比较。
        kind: PolygonKind,
    },
    /// 文本。
    Text(TextPaint),
    /// 图片。不透明度、投影、混合模式和图层模糊。不改资源表。
    Image {
        /// [`crate::element::Image::alpha`]。
        alpha: f32,
        /// 显示矩形的投影。
        shadow: DropShadow,
        /// 混合模式。
        blend_mode: BlendMode,
        /// 图层模糊标准差。
        blur_radius: f32,
    },
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
    /// 形状、文本或图片的不透明度，范围 0 到 1。
    pub opacity: Option<f32>,
    /// 文本字号（世界单位）。
    pub font_size: Option<f32>,
    /// 文本填充色。
    pub text_color: Option<Color>,
    /// 文本水平对齐。只写到文本上。
    pub text_align: Option<TextAlign>,
    /// 文本字重。只写到文本上。
    pub font_weight: Option<FontWeight>,
    /// 形状虚线。空数组表示改回实线。文本描边不读这个字段。
    pub stroke_dash: Option<Vec<f32>>,
    /// 形状线帽。文本描边不读这个字段。
    pub stroke_cap: Option<StrokeCap>,
    /// 形状线结。文本描边不读这个字段。
    pub stroke_join: Option<StrokeJoin>,
    /// 形状描边对齐。文本描边不读这个字段。
    pub stroke_align: Option<StrokeAlign>,
    /// 矩形圆角半径，单位与矩形尺寸相同。只写到矩形上，并钳到短边一半。
    pub corner_radius: Option<f32>,
    /// 正多边形边数或星形角数。只写到 [`Polygon`] 上，并钳到 3–64。
    ///
    /// 和填充、描边放在同一次 [`crate::SetStyleCommand`] 里，拖动滑块松手仍是一条撤销。
    pub sides: Option<u32>,
    /// 星形内径比（相对外接圆）。只写到星形上，并钳到 0.05–0.95。正多边形忽略。
    pub inner_radius: Option<f32>,
    /// 投影局部偏移 X。形状、文本和图片都写。
    pub shadow_offset_x: Option<f32>,
    /// 投影局部偏移 Y。
    pub shadow_offset_y: Option<f32>,
    /// 投影羽化半径。负数收成 0。
    pub shadow_blur: Option<f32>,
    /// 投影颜色的 RGB。不改 alpha，浓度走 [`Self::shadow_opacity`]。
    pub shadow_color: Option<Color>,
    /// 投影浓度，写入颜色 alpha，范围 0 到 1。0 表示不绘制。
    pub shadow_opacity: Option<f32>,
    /// 形状填充种类。文本和图片忽略。
    pub fill_kind: Option<FillKind>,
    /// 渐变色标。空数组清掉色标，线性/径向会退回纯色 [`ElementStyle::fill`]。
    pub fill_stops: Option<Vec<GradientStop>>,
    /// 线性渐变角度，单位度。
    pub fill_angle: Option<f32>,
    /// 径向渐变中心，局部坐标。
    pub fill_center: Option<Vec2>,
    /// 径向渐变半径，局部单位。0 表示自动。
    pub fill_radius: Option<f32>,
    /// 形状、文本、图片或画板的混合模式。组展开到子孙叶子。
    pub blend_mode: Option<BlendMode>,
    /// 图层高斯模糊标准差。形状、文本、图片和画板都写。负数收成 0。
    ///
    /// 没出现在补丁里就保持原值，也不改渐变或混合模式。
    pub blur_radius: Option<f32>,
    /// 直线起点箭头。只写到直线上；空选或非直线是 no-op。
    pub start_marker: Option<LineMarker>,
    /// 直线终点箭头。只写到直线上；空选或非直线是 no-op。
    pub end_marker: Option<LineMarker>,
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
    /// 形状 [`ElementStyle::opacity`]、文本不透明度和图片 [`crate::element::Image::alpha`]。
    pub opacity: StyleField<f32>,
    /// 文本字号。
    pub font_size: StyleField<f32>,
    /// 文本填充色。
    pub text_color: StyleField<Color>,
    /// 文本水平对齐。
    pub text_align: StyleField<TextAlign>,
    /// 文本字重。
    pub font_weight: StyleField<FontWeight>,
    /// 形状虚线。空数组是实线。文本不参与。
    pub stroke_dash: StyleField<Vec<f32>>,
    /// 形状线帽。文本不参与。
    pub stroke_cap: StyleField<StrokeCap>,
    /// 形状线结。文本不参与。
    pub stroke_join: StyleField<StrokeJoin>,
    /// 形状描边对齐。文本不参与。
    pub stroke_align: StyleField<StrokeAlign>,
    /// 矩形圆角。选择里没有矩形时为 [`StyleField::Absent`]。
    pub corner_radius: StyleField<f32>,
    /// 多边形边数或星形角数。选择里没有多边形或星形时为 [`StyleField::Absent`]。
    pub sides: StyleField<f32>,
    /// 星形内径比。选择里没有星形时为 [`StyleField::Absent`]。正多边形不参与。
    pub inner_radius: StyleField<f32>,
    /// 选择里有正多边形（不含星形）。
    pub has_polygon: bool,
    /// 选择里有星形。
    pub has_star: bool,
    /// 投影偏移 X。没有可编辑叶子时为 [`StyleField::Absent`]。
    pub shadow_offset_x: StyleField<f32>,
    /// 投影偏移 Y。
    pub shadow_offset_y: StyleField<f32>,
    /// 投影羽化半径。
    pub shadow_blur: StyleField<f32>,
    /// 投影颜色的 RGB，alpha 固定为 1，浓度见 [`Self::shadow_opacity`]。
    pub shadow_color: StyleField<Color>,
    /// 投影浓度。
    pub shadow_opacity: StyleField<f32>,
    /// 选择里有形状、文本或图片，可以改投影。画板算形状。
    pub shadow_supported: bool,
    /// 形状填充种类。文本和图片不参与。
    pub fill_kind: StyleField<FillKind>,
    /// 形状渐变色标。文本和图片不参与。
    pub fill_stops: StyleField<Vec<GradientStop>>,
    /// 线性渐变角度。文本和图片不参与。
    pub fill_angle: StyleField<f32>,
    /// 径向渐变中心。文本和图片不参与。
    pub fill_center: StyleField<Vec2>,
    /// 径向渐变半径。文本和图片不参与。
    pub fill_radius: StyleField<f32>,
    /// 图层模糊标准差。没有可编辑叶子时为 [`StyleField::Absent`]。
    pub blur_radius: StyleField<f32>,
    /// 选择里有形状、文本、图片或画板，可以改图层模糊。
    pub blur_supported: bool,
    /// 直线起点箭头。选择里没有直线时为 [`StyleField::Absent`]。
    pub start_marker: StyleField<LineMarker>,
    /// 直线终点箭头。选择里没有直线时为 [`StyleField::Absent`]。
    pub end_marker: StyleField<LineMarker>,
    /// 选择里有直线（含组内的直线）。箭头控件只对它们出现。
    pub has_line: bool,
    /// 选择里有形状（含组内的形状）。
    pub has_shape: bool,
    /// 选择里有矩形（含组内的矩形）。圆角只对它们生效。
    pub has_rect: bool,
    /// 选择里有文本（含组内的文本）。
    pub has_text: bool,
    /// 选择里有图片（含组内的图片）。图片只贡献不透明度和混合模式。
    pub has_image: bool,
    /// 形状、文本和图片的混合模式。选择里没有叶子时为 [`StyleField::Absent`]。
    pub blend_mode: StyleField<BlendMode>,
}

impl SelectionStyle {
    /// 没有任何可编辑样式。
    pub fn is_empty(&self) -> bool {
        !self.has_shape && !self.has_text && !self.has_image
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
            Self::Shape(style) => apply_shape_paint(style, patch),
            Self::Line {
                style,
                start_marker,
                end_marker,
            } => {
                apply_shape_paint(style, patch);
                if let Some(marker) = patch.start_marker {
                    *start_marker = marker;
                }
                if let Some(marker) = patch.end_marker {
                    *end_marker = marker;
                }
            }
            Self::Rectangle {
                style,
                corner_radius,
            } => {
                apply_shape_paint(style, patch);
                if let Some(radius) = patch.corner_radius.filter(|value| value.is_finite()) {
                    *corner_radius = radius.max(0.0);
                }
            }
            Self::Polygon {
                style,
                sides,
                inner_radius,
                kind,
            } => {
                apply_shape_paint(style, patch);
                if let Some(next) = patch.sides {
                    *sides = Polygon::clamp_sides(next);
                }
                if *kind == PolygonKind::Star {
                    if let Some(ratio) = patch.inner_radius.filter(|value| value.is_finite()) {
                        *inner_radius = Polygon::clamp_inner_radius(ratio);
                    }
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
                if let Some(align) = patch.text_align {
                    text.align = align;
                }
                if let Some(weight) = patch.font_weight {
                    text.font_weight = weight;
                }
                if let Some(mode) = patch.blend_mode {
                    text.blend_mode = mode;
                }
                apply_shadow(&mut text.shadow, patch);
                apply_layer_blur(&mut text.blur_radius, patch);
            }
            Self::Image {
                alpha,
                shadow,
                blend_mode,
                blur_radius,
            } => {
                if let Some(opacity) = finite_opacity(patch.opacity) {
                    *alpha = opacity;
                }
                if let Some(mode) = patch.blend_mode {
                    *blend_mode = mode;
                }
                apply_shadow(shadow, patch);
                apply_layer_blur(blur_radius, patch);
            }
        }
        *self != original
    }

    /// 按矩形当前尺寸把圆角钳到短边一半。其它快照不动。
    ///
    /// [`SetStyleCommand::from_patch`] 在入栈前调用，撤销记录里不留下超限半径。
    pub fn clamp_corner_radius(&mut self, scene: &Scene, id: ElementId) {
        let Self::Rectangle { corner_radius, .. } = self else {
            return;
        };
        let Some(AnyElement::Rectangle(rect)) = scene.get(id) else {
            return;
        };
        *corner_radius = Rectangle::clamp_radius(rect.size, *corner_radius);
    }
}

/// 选择里要改样式的元素。组会展开成自己和全部子孙。
/// 画板只改自己，不把填充写到子元素上。重复 id 只保留第一次。
pub fn style_target_ids(scene: &Scene, ids: &[ElementId]) -> Vec<ElementId> {
    let mut out = Vec::new();
    for id in ids {
        if scene
            .get(*id)
            .is_some_and(|element| matches!(element, AnyElement::Frame(_)))
        {
            if !out.contains(id) {
                out.push(*id);
            }
            continue;
        }
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

/// 读取一个元素的样式。组返回 `None`，图片只返回不透明度。
/// 画板按形状读取，这样填充和描边面板可以改画板矩形。
pub fn capture_style(scene: &Scene, id: ElementId) -> Option<StyleSnapshot> {
    match scene.get(id)? {
        AnyElement::Rectangle(rect) => Some(StyleSnapshot::Rectangle {
            style: rect.style.clone(),
            corner_radius: rect.effective_corner_radius(),
        }),
        AnyElement::Circle(circle) => Some(StyleSnapshot::Shape(circle.style.clone())),
        AnyElement::Polygon(polygon) => Some(StyleSnapshot::Polygon {
            style: polygon.style.clone(),
            sides: polygon.sides(),
            inner_radius: polygon.inner_radius(),
            kind: polygon.kind(),
        }),
        AnyElement::Line(line) => Some(StyleSnapshot::Line {
            style: line.style.clone(),
            start_marker: line.start_marker,
            end_marker: line.end_marker,
        }),
        AnyElement::Path(path) => Some(StyleSnapshot::Shape(path.style.clone())),
        AnyElement::Text(text) => Some(StyleSnapshot::Text(TextPaint {
            font_size: text.font_size,
            color: text.color,
            opacity: text.opacity,
            stroke: text.stroke,
            stroke_width: text.stroke_width,
            align: text.align,
            font_weight: text.font_weight,
            shadow: text.shadow,
            blend_mode: text.blend_mode,
            blur_radius: crate::element::blur::sanitize_blur(text.blur_radius),
        })),
        AnyElement::Image(image) => Some(StyleSnapshot::Image {
            alpha: image.alpha,
            shadow: image.shadow,
            blend_mode: image.blend_mode,
            blur_radius: crate::element::blur::sanitize_blur(image.blur_radius),
        }),
        AnyElement::Frame(frame) => Some(StyleSnapshot::Shape(frame.style.clone())),
        AnyElement::Group(_) => None,
    }
}

/// 写回样式快照，并 [`Scene::mark_dirty`]。
///
/// 文本的字号、颜色、描边、对齐或字重变化会丢掉光栅纹理。不透明度只改 tint，不重新光栅化。
pub fn apply_style_snapshot(scene: &mut Scene, id: ElementId, snapshot: &StyleSnapshot) {
    let applied = match scene.get_mut(id) {
        Some(AnyElement::Rectangle(rect)) => assign_rectangle(rect, snapshot),
        Some(AnyElement::Circle(circle)) => assign_shape(&mut circle.style, snapshot),
        Some(AnyElement::Polygon(polygon)) => assign_polygon(polygon, snapshot),
        Some(AnyElement::Line(line)) => assign_line(line, snapshot),
        Some(AnyElement::Path(path)) => assign_shape(&mut path.style, snapshot),
        Some(AnyElement::Text(text)) => {
            if let StyleSnapshot::Text(paint) = snapshot {
                text.blend_mode = paint.blend_mode;
                text.set_paint(
                    paint.font_size,
                    paint.color,
                    paint.opacity,
                    paint.stroke,
                    paint.stroke_width,
                );
                text.set_align_and_weight(paint.align, paint.font_weight);
                // 投影和图层模糊都不进字形纹理，只标脏让顶点重写。
                text.shadow = paint.shadow;
                text.blur_radius = crate::element::blur::sanitize_blur(paint.blur_radius);
                true
            } else {
                false
            }
        }
        Some(AnyElement::Image(image)) => {
            if let StyleSnapshot::Image {
                alpha,
                shadow,
                blend_mode,
                blur_radius,
            } = snapshot
            {
                image.blend_mode = *blend_mode;
                image.set_alpha(*alpha);
                image.shadow = *shadow;
                image.blur_radius = crate::element::blur::sanitize_blur(*blur_radius);
                true
            } else {
                false
            }
        }
        Some(AnyElement::Frame(frame)) => assign_shape(&mut frame.style, snapshot),
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
    let mut text_aligns = Vec::new();
    let mut font_weights = Vec::new();
    let mut dashes = Vec::new();
    let mut caps = Vec::new();
    let mut joins = Vec::new();
    let mut aligns = Vec::new();
    let mut radii = Vec::new();
    let mut side_counts = Vec::new();
    let mut inner_radii = Vec::new();
    let mut shadow_xs = Vec::new();
    let mut shadow_ys = Vec::new();
    let mut shadow_blurs = Vec::new();
    let mut shadow_colors = Vec::new();
    let mut shadow_opacities = Vec::new();
    let mut fill_kinds = Vec::new();
    let mut fill_stops = Vec::new();
    let mut fill_angles = Vec::new();
    let mut fill_centers = Vec::new();
    let mut fill_radii = Vec::new();
    let mut blends = Vec::new();
    let mut layer_blurs = Vec::new();
    let mut start_markers = Vec::new();
    let mut end_markers = Vec::new();
    let mut has_line = false;
    let mut has_shape = false;
    let mut has_polygon = false;
    let mut has_star = false;
    let mut has_rect = false;
    let mut has_text = false;
    let mut has_image = false;

    for id in style_target_ids(scene, ids) {
        match capture_style(scene, id) {
            Some(StyleSnapshot::Line {
                style,
                start_marker,
                end_marker,
            }) => {
                has_shape = true;
                has_line = true;
                push_shape_paint(
                    &mut fills,
                    &mut strokes,
                    &mut widths,
                    &mut opacities,
                    &style,
                );
                push_gradient(
                    &mut fill_kinds,
                    &mut fill_stops,
                    &mut fill_angles,
                    &mut fill_centers,
                    &mut fill_radii,
                    &style,
                );
                dashes.push(style.stroke_dash);
                caps.push(style.stroke_cap);
                joins.push(style.stroke_join);
                blends.push(style.blend_mode);
                push_shadow(
                    &mut shadow_xs,
                    &mut shadow_ys,
                    &mut shadow_blurs,
                    &mut shadow_colors,
                    &mut shadow_opacities,
                    style.shadow,
                );
                layer_blurs.push(crate::element::blur::sanitize_blur(style.blur_radius));
                start_markers.push(start_marker);
                end_markers.push(end_marker);
            }
            Some(StyleSnapshot::Shape(style)) => {
                has_shape = true;
                push_shape_paint(
                    &mut fills,
                    &mut strokes,
                    &mut widths,
                    &mut opacities,
                    &style,
                );
                push_gradient(
                    &mut fill_kinds,
                    &mut fill_stops,
                    &mut fill_angles,
                    &mut fill_centers,
                    &mut fill_radii,
                    &style,
                );
                dashes.push(style.stroke_dash);
                caps.push(style.stroke_cap);
                joins.push(style.stroke_join);
                aligns.push(style.stroke_align);
                blends.push(style.blend_mode);
                push_shadow(
                    &mut shadow_xs,
                    &mut shadow_ys,
                    &mut shadow_blurs,
                    &mut shadow_colors,
                    &mut shadow_opacities,
                    style.shadow,
                );
                layer_blurs.push(crate::element::blur::sanitize_blur(style.blur_radius));
            }
            Some(StyleSnapshot::Rectangle {
                style,
                corner_radius,
            }) => {
                has_shape = true;
                has_rect = true;
                push_shape_paint(
                    &mut fills,
                    &mut strokes,
                    &mut widths,
                    &mut opacities,
                    &style,
                );
                push_gradient(
                    &mut fill_kinds,
                    &mut fill_stops,
                    &mut fill_angles,
                    &mut fill_centers,
                    &mut fill_radii,
                    &style,
                );
                dashes.push(style.stroke_dash);
                caps.push(style.stroke_cap);
                joins.push(style.stroke_join);
                aligns.push(style.stroke_align);
                blends.push(style.blend_mode);
                radii.push(corner_radius);
                push_shadow(
                    &mut shadow_xs,
                    &mut shadow_ys,
                    &mut shadow_blurs,
                    &mut shadow_colors,
                    &mut shadow_opacities,
                    style.shadow,
                );
                layer_blurs.push(crate::element::blur::sanitize_blur(style.blur_radius));
            }
            Some(StyleSnapshot::Polygon {
                style,
                sides,
                inner_radius,
                kind,
            }) => {
                has_shape = true;
                push_shape_paint(
                    &mut fills,
                    &mut strokes,
                    &mut widths,
                    &mut opacities,
                    &style,
                );
                push_gradient(
                    &mut fill_kinds,
                    &mut fill_stops,
                    &mut fill_angles,
                    &mut fill_centers,
                    &mut fill_radii,
                    &style,
                );
                dashes.push(style.stroke_dash);
                caps.push(style.stroke_cap);
                joins.push(style.stroke_join);
                aligns.push(style.stroke_align);
                blends.push(style.blend_mode);
                side_counts.push(sides as f32);
                if kind == PolygonKind::Star {
                    has_star = true;
                    inner_radii.push(inner_radius);
                } else {
                    has_polygon = true;
                }
                push_shadow(
                    &mut shadow_xs,
                    &mut shadow_ys,
                    &mut shadow_blurs,
                    &mut shadow_colors,
                    &mut shadow_opacities,
                    style.shadow,
                );
                layer_blurs.push(crate::element::blur::sanitize_blur(style.blur_radius));
            }
            Some(StyleSnapshot::Text(text)) => {
                has_text = true;
                strokes.push(text.stroke);
                widths.push(text.stroke_width);
                opacities.push(text.opacity);
                font_sizes.push(text.font_size);
                text_colors.push(text.color);
                text_aligns.push(text.align);
                font_weights.push(text.font_weight);
                blends.push(text.blend_mode);
                push_shadow(
                    &mut shadow_xs,
                    &mut shadow_ys,
                    &mut shadow_blurs,
                    &mut shadow_colors,
                    &mut shadow_opacities,
                    text.shadow,
                );
                layer_blurs.push(text.blur_radius);
            }
            Some(StyleSnapshot::Image {
                alpha,
                shadow,
                blend_mode,
                blur_radius,
            }) => {
                has_image = true;
                opacities.push(alpha);
                blends.push(blend_mode);
                push_shadow(
                    &mut shadow_xs,
                    &mut shadow_ys,
                    &mut shadow_blurs,
                    &mut shadow_colors,
                    &mut shadow_opacities,
                    shadow,
                );
                layer_blurs.push(blur_radius);
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
        text_align: fold(text_aligns),
        font_weight: fold(font_weights),
        stroke_dash: fold(dashes),
        stroke_cap: fold(caps),
        stroke_join: fold(joins),
        stroke_align: fold(aligns),
        corner_radius: fold(radii),
        sides: fold(side_counts),
        inner_radius: fold(inner_radii),
        has_polygon,
        has_star,
        shadow_offset_x: fold(shadow_xs),
        shadow_offset_y: fold(shadow_ys),
        shadow_blur: fold(shadow_blurs),
        shadow_color: fold(shadow_colors),
        shadow_opacity: fold(shadow_opacities),
        shadow_supported: has_shape || has_text || has_image,
        fill_kind: fold(fill_kinds),
        fill_stops: fold(fill_stops),
        fill_angle: fold(fill_angles),
        fill_center: fold(fill_centers),
        fill_radius: fold(fill_radii),
        blur_radius: fold(layer_blurs),
        blur_supported: has_shape || has_text || has_image,
        start_marker: fold(start_markers),
        end_marker: fold(end_markers),
        has_line,
        has_shape,
        has_rect,
        has_text,
        has_image,
        blend_mode: fold(blends),
    }
}

/// 丢掉非有限值。全是 0 时收成空数组，和实线是同一种样式。
fn sanitize_dash(dash: &[f32]) -> Vec<f32> {
    let values: Vec<f32> = dash
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .map(|value| value.max(0.0))
        .collect();
    if values.iter().all(|value| *value <= 1e-6) {
        Vec::new()
    } else {
        values
    }
}

fn push_shape_paint(
    fills: &mut Vec<Option<Color>>,
    strokes: &mut Vec<Option<Color>>,
    widths: &mut Vec<f32>,
    opacities: &mut Vec<f32>,
    style: &ElementStyle,
) {
    fills.push(style.fill);
    strokes.push(style.stroke);
    widths.push(style.stroke_width);
    opacities.push(style.opacity);
}

fn apply_shape_paint(style: &mut ElementStyle, patch: &StylePatch) {
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
    if let Some(dash) = &patch.stroke_dash {
        style.stroke_dash = sanitize_dash(dash);
    }
    if let Some(cap) = patch.stroke_cap {
        style.stroke_cap = cap;
    }
    if let Some(join) = patch.stroke_join {
        style.stroke_join = join;
    }
    if let Some(align) = patch.stroke_align {
        style.stroke_align = align;
    }
    if let Some(kind) = patch.fill_kind {
        style.fill_kind = kind;
    }
    if let Some(stops) = &patch.fill_stops {
        style.fill_stops = GradientStop::normalize(stops);
    }
    if let Some(angle) = patch.fill_angle.filter(|value| value.is_finite()) {
        style.fill_angle = angle;
    }
    if let Some(center) = patch.fill_center {
        if center.x.is_finite() && center.y.is_finite() {
            style.fill_center = center;
        }
    }
    if let Some(radius) = patch.fill_radius.filter(|value| value.is_finite()) {
        style.fill_radius = radius.max(0.0);
    }
    if let Some(mode) = patch.blend_mode {
        style.blend_mode = mode;
    }
    apply_shadow(&mut style.shadow, patch);
    apply_layer_blur(&mut style.blur_radius, patch);
}

fn apply_layer_blur(radius: &mut f32, patch: &StylePatch) {
    if let Some(value) = patch.blur_radius.filter(|value| value.is_finite()) {
        *radius = crate::element::blur::sanitize_blur(value);
    }
}

fn apply_shadow(shadow: &mut DropShadow, patch: &StylePatch) {
    if let Some(x) = patch.shadow_offset_x.filter(|value| value.is_finite()) {
        shadow.offset.x = x;
    }
    if let Some(y) = patch.shadow_offset_y.filter(|value| value.is_finite()) {
        shadow.offset.y = y;
    }
    if let Some(blur) = patch.shadow_blur.filter(|value| value.is_finite()) {
        shadow.blur = blur.max(0.0);
    }
    if let Some(color) = patch.shadow_color {
        let alpha = shadow.color.a;
        shadow.color = Color::new(color.r, color.g, color.b, alpha);
    }
    if let Some(opacity) = patch.shadow_opacity.filter(|value| value.is_finite()) {
        shadow.color.a = opacity.clamp(0.0, 1.0);
    }
    *shadow = shadow.sanitized();
}

fn push_shadow(
    xs: &mut Vec<f32>,
    ys: &mut Vec<f32>,
    blurs: &mut Vec<f32>,
    colors: &mut Vec<Color>,
    opacities: &mut Vec<f32>,
    shadow: DropShadow,
) {
    let shadow = shadow.sanitized();
    xs.push(shadow.offset.x);
    ys.push(shadow.offset.y);
    blurs.push(shadow.blur);
    colors.push(Color::rgb(shadow.color.r, shadow.color.g, shadow.color.b));
    opacities.push(shadow.color.a);
}

fn push_gradient(
    kinds: &mut Vec<FillKind>,
    stops: &mut Vec<Vec<GradientStop>>,
    angles: &mut Vec<f32>,
    centers: &mut Vec<Vec2>,
    radii: &mut Vec<f32>,
    style: &ElementStyle,
) {
    kinds.push(style.fill_kind);
    stops.push(GradientStop::normalize(&style.fill_stops));
    angles.push(style.fill_angle);
    centers.push(style.fill_center);
    radii.push(style.fill_radius);
}

fn assign_rectangle(rect: &mut Rectangle, snapshot: &StyleSnapshot) -> bool {
    match snapshot {
        StyleSnapshot::Rectangle {
            style,
            corner_radius,
        } => {
            rect.style = style.clone();
            rect.set_corner_radius(*corner_radius);
            true
        }
        StyleSnapshot::Shape(style) => {
            rect.style = style.clone();
            true
        }
        _ => false,
    }
}

fn assign_polygon(polygon: &mut Polygon, snapshot: &StyleSnapshot) -> bool {
    match snapshot {
        StyleSnapshot::Polygon {
            style,
            sides,
            inner_radius,
            ..
        } => {
            polygon.style = style.clone();
            polygon.set_sides(*sides);
            polygon.set_inner_radius(*inner_radius);
            true
        }
        StyleSnapshot::Shape(style) => {
            polygon.style = style.clone();
            true
        }
        _ => false,
    }
}

fn assign_line(line: &mut Line, snapshot: &StyleSnapshot) -> bool {
    match snapshot {
        StyleSnapshot::Line {
            style,
            start_marker,
            end_marker,
        } => {
            line.style = style.clone();
            line.start_marker = *start_marker;
            line.end_marker = *end_marker;
            true
        }
        StyleSnapshot::Shape(style) => {
            line.style = style.clone();
            true
        }
        _ => false,
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
    use crate::element::{
        DropShadow, FillKind, Frame, GradientStop, Image, Line, LineMarker, Path, Rectangle,
        StrokeAlign, StrokeCap, StrokeJoin, Text,
    };
    use crate::Element;
    use crate::{
        apply_reparent_into_frame, hit_test_point, sample_paint_at, Circle, Command,
        CommandHistory, HitTestOptions, SampledPaint, SetStyleCommand, VertexCache,
    };
    use infinitecanvas_math::Vec2;

    fn shape_fill(scene: &Scene, id: ElementId) -> Option<Color> {
        shape_style(scene, id).fill
    }

    /// 圆、路径是 [`StyleSnapshot::Shape`]，直线是 [`StyleSnapshot::Line`]，矩形是 [`StyleSnapshot::Rectangle`]。
    fn shape_style(scene: &Scene, id: ElementId) -> ElementStyle {
        match capture_style(scene, id) {
            Some(
                StyleSnapshot::Shape(style)
                | StyleSnapshot::Line { style, .. }
                | StyleSnapshot::Rectangle { style, .. }
                | StyleSnapshot::Polygon { style, .. },
            ) => style,
            other => panic!("shape {id}, got {other:?}"),
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
        assert_eq!(style.fill_kind, StyleField::Uniform(FillKind::Solid));
        assert_eq!(style.fill_stops, StyleField::Uniform(Vec::new()));
        assert_eq!(style.opacity, StyleField::Uniform(1.0));
        assert_eq!(style.stroke_width, StyleField::Uniform(1.0));
        assert!(style.has_shape);
        assert!(style.has_rect);
        assert_eq!(style.corner_radius, StyleField::Uniform(0.0));
        assert!(!style.has_text);
        assert!(!style.has_image);
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
            corner_radius: Some(3.0),
            ..StylePatch::default()
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
                assert_eq!(rect.corner_radius(), 3.0);
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
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.style, ElementStyle::new());
                assert_eq!(rect.corner_radius(), 0.0);
            }
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

    #[test]
    fn image_opacity_follows_other_leaves_and_leaves_the_asset_table_alone() {
        use crate::asset::encode_png_rgba;
        use crate::element::Image;

        let mut scene = Scene::new();
        let png = encode_png_rgba(1, 1, &[255, 0, 0, 255]).unwrap();
        let asset_id = scene.insert_image_bytes(&png, "image/png").unwrap();
        scene.add_image(Image::from_asset(1, Vec2::ZERO, &asset_id, 1, 1));
        scene.add_rect(
            Rectangle::new(2, Vec2::new(20.0, 0.0), Vec2::new(10.0, 10.0))
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        if let Some(AnyElement::Image(image)) = scene.get_mut(1) {
            image.set_alpha(0.8);
        }
        let group_id = scene.group(&[1, 2]).unwrap();

        let image_only = selection_style(&scene, &[1]);
        assert!(image_only.has_image);
        assert!(!image_only.has_shape);
        assert!(!image_only.has_text);
        assert_eq!(image_only.opacity, StyleField::Uniform(0.8));
        assert_eq!(image_only.fill, StyleField::Absent);
        assert_eq!(image_only.fill_kind, StyleField::Absent);
        assert_eq!(image_only.fill_stops, StyleField::Absent);
        assert_eq!(image_only.fill_angle, StyleField::Absent);
        assert_eq!(image_only.stroke_width, StyleField::Absent);
        assert_eq!(image_only.stroke_dash, StyleField::Absent);
        assert_eq!(image_only.stroke_cap, StyleField::Absent);
        assert_eq!(image_only.stroke_join, StyleField::Absent);
        assert_eq!(image_only.corner_radius, StyleField::Absent);
        assert!(!image_only.is_empty());

        let together = selection_style(&scene, &[group_id]);
        assert!(together.has_image && together.has_shape);
        assert_eq!(together.opacity, StyleField::Mixed);
        assert_eq!(together.fill, StyleField::Uniform(Some(Color::RED)));

        let fill_only = StylePatch {
            fill: Some(Some(Color::BLUE)),
            ..StylePatch::default()
        };
        assert!(SetStyleCommand::from_patch(&scene, &[1], &fill_only).is_none());

        let patch = StylePatch {
            opacity: Some(0.3),
            fill: Some(Some(Color::BLUE)),
            ..StylePatch::default()
        };
        let mut history = CommandHistory::new();
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[group_id], &patch).unwrap()),
            &mut scene,
        );
        match scene.get(1).unwrap() {
            AnyElement::Image(image) => {
                assert_eq!(image.alpha, 0.3);
                assert_eq!(image.asset_id, asset_id);
            }
            other => panic!("image, got {other:?}"),
        }
        assert_eq!(scene.asset_refcount(&asset_id), 1);
        assert_eq!(scene.image_asset_count(), 1);
        assert_eq!(shape_fill(&scene, 2), Some(Color::BLUE));

        let mut cache = VertexCache::new();
        let _ = cache.sync(&mut scene);
        let vertices = cache.textured_slice(1).unwrap();
        assert!(vertices
            .iter()
            .all(|vertex| (vertex.color[3] - 0.3).abs() < 1.0e-5));

        assert!(history.undo(&mut scene));
        match scene.get(1).unwrap() {
            AnyElement::Image(image) => {
                assert_eq!(image.alpha, 0.8);
                assert_eq!(image.asset_id, asset_id);
            }
            other => panic!("image, got {other:?}"),
        }
        assert_eq!(scene.asset_refcount(&asset_id), 1);
        assert_eq!(shape_fill(&scene, 2), Some(Color::RED));
    }

    #[test]
    fn stroke_dash_cap_join_round_trip_undo_and_legacy_defaults() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(40.0, 20.0)));
        scene.add_line(Line::from_world(
            2,
            Vec2::ZERO,
            Vec2::new(80.0, 0.0),
            ElementStyle::stroke_only(Color::BLACK, 4.0),
            true,
        ));
        scene.add_path(Path::from_world_points(
            3,
            &[Vec2::ZERO, Vec2::new(30.0, 10.0), Vec2::new(40.0, 0.0)],
            ElementStyle::stroke_only(Color::BLACK, 4.0),
        ));
        scene.add_circle(Circle::new(4, Vec2::new(10.0, 10.0), 12.0));

        let patch = StylePatch {
            fill: Some(Some(Color::RED)),
            stroke_width: Some(6.0),
            opacity: Some(0.5),
            stroke_dash: Some(vec![8.0, 4.0, f32::NAN]),
            stroke_cap: Some(StrokeCap::Round),
            stroke_join: Some(StrokeJoin::Bevel),
            stroke_align: Some(StrokeAlign::Inside),
            ..StylePatch::default()
        };
        let mut history = CommandHistory::new();
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[1, 2, 3, 4], &patch).unwrap()),
            &mut scene,
        );
        assert_eq!(history.undo_count(), 1);
        assert!(scene.has_dirty_elements());

        let json = scene.to_json().unwrap();
        assert!(json.contains("\"stroke_dash\""));
        assert!(json.contains("\"stroke_cap\":\"round\""));
        assert!(json.contains("\"stroke_join\":\"bevel\""));
        assert!(json.contains("\"stroke_align\":\"inside\""));
        let restored = Scene::from_json(&json).unwrap();
        for id in [1, 2, 3, 4] {
            let style = shape_style(&restored, id);
            assert_eq!(style.stroke_dash, vec![8.0, 4.0]);
            assert_eq!(style.stroke_cap, StrokeCap::Round);
            assert_eq!(style.stroke_join, StrokeJoin::Bevel);
            assert_eq!(style.stroke_align, StrokeAlign::Inside);
            assert_eq!(style.stroke_width, 6.0);
            assert!((style.opacity - 0.5).abs() < 1e-5);
        }
        assert_eq!(shape_style(&restored, 1).fill, Some(Color::RED));
        match capture_style(&restored, 1) {
            Some(StyleSnapshot::Rectangle { corner_radius, .. }) => {
                assert_eq!(corner_radius, 0.0);
            }
            other => panic!("rect, got {other:?}"),
        }

        assert!(history.undo(&mut scene));
        let line = shape_style(&scene, 2);
        assert!(line.stroke_dash.is_empty());
        assert_eq!(line.stroke_cap, StrokeCap::Butt);
        assert_eq!(line.stroke_join, StrokeJoin::Miter);
        assert_eq!(line.stroke_width, 4.0);
        assert!(history.redo(&mut scene));
        let path = shape_style(&scene, 3);
        assert_eq!(path.stroke_dash, vec![8.0, 4.0]);
        assert_eq!(path.stroke_cap, StrokeCap::Round);
        assert_eq!(path.stroke_join, StrokeJoin::Bevel);

        let mixed = StylePatch {
            stroke_cap: Some(StrokeCap::Square),
            ..StylePatch::default()
        };
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[1], &mixed).unwrap()),
            &mut scene,
        );
        let summary = selection_style(&scene, &[1, 2]);
        assert_eq!(summary.stroke_cap, StyleField::Mixed);
        assert_eq!(summary.stroke_dash, StyleField::Uniform(vec![8.0, 4.0]));
        assert_eq!(summary.stroke_join, StyleField::Uniform(StrokeJoin::Bevel));
        assert_eq!(summary.opacity, StyleField::Uniform(0.5));
        assert_eq!(summary.corner_radius, StyleField::Uniform(0.0));

        let size = serde_json::to_value(Vec2::new(8.0, 6.0)).unwrap();
        let start = serde_json::to_value(Vec2::ZERO).unwrap();
        let end = serde_json::to_value(Vec2::new(12.0, 0.0)).unwrap();
        let legacy = serde_json::json!({
            "version": 1,
            "elements": [
                { "type": "rectangle", "data": { "props": { "id": 1 }, "size": size } },
                { "type": "line", "data": { "props": { "id": 2 }, "start": start, "end": end } },
                { "type": "path", "data": { "props": { "id": 3 } } },
                { "type": "circle", "data": { "props": { "id": 4 }, "radius": 5.0 } }
            ]
        });
        let legacy_scene = Scene::from_json(&legacy.to_string()).unwrap();
        for id in [1, 2, 3, 4] {
            let style = shape_style(&legacy_scene, id);
            assert!(style.stroke_dash.is_empty(), "id {id} dash");
            assert_eq!(style.stroke_cap, StrokeCap::Butt);
            assert_eq!(style.stroke_join, StrokeJoin::Miter);
        }
        match capture_style(&legacy_scene, 1) {
            Some(StyleSnapshot::Rectangle { corner_radius, .. }) => {
                assert_eq!(corner_radius, 0.0);
            }
            other => panic!("legacy rect, got {other:?}"),
        }
    }

    #[test]
    fn corner_radius_is_one_undo_entry_and_mixed_only_among_rects() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(40.0, 20.0)));
        scene.add_rect(
            Rectangle::new(2, Vec2::new(80.0, 0.0), Vec2::new(40.0, 10.0)).with_corner_radius(2.0),
        );
        scene.add_circle(Circle::new(3, Vec2::new(140.0, 0.0), 8.0));

        let rects = selection_style(&scene, &[1, 2]);
        assert!(rects.has_rect);
        assert_eq!(rects.corner_radius, StyleField::Mixed);
        let circle = selection_style(&scene, &[3]);
        assert!(!circle.has_rect);
        assert_eq!(circle.corner_radius, StyleField::Absent);
        assert!(SetStyleCommand::from_patch(
            &scene,
            &[3],
            &StylePatch {
                corner_radius: Some(4.0),
                ..StylePatch::default()
            },
        )
        .is_none());

        let mut history = CommandHistory::new();
        history.execute(
            Box::new(
                SetStyleCommand::from_patch(
                    &scene,
                    &[1, 3],
                    &StylePatch {
                        corner_radius: Some(100.0),
                        ..StylePatch::default()
                    },
                )
                .unwrap(),
            ),
            &mut scene,
        );
        assert_eq!(history.undo_count(), 1);
        assert!(scene.has_dirty_elements());
        match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => assert_eq!(rect.corner_radius(), 10.0),
            other => panic!("rect, got {other:?}"),
        }
        match scene.get(3).unwrap() {
            AnyElement::Circle(circle) => assert_eq!(circle.radius, 8.0),
            other => panic!("circle, got {other:?}"),
        }

        let mut cache = VertexCache::new();
        let _ = cache.sync(&mut scene);
        assert!(cache.fill_slice(1).unwrap().len() > 6);
        assert!(!scene.has_dirty_elements());

        assert!(history.undo(&mut scene));
        assert!(scene.has_dirty_elements());
        match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.corner_radius(), 0.0);
                assert_eq!(rect.generate_fill_vertices().len(), 6);
            }
            other => panic!("rect, got {other:?}"),
        }
        let _ = cache.sync(&mut scene);
        assert!(!scene.has_dirty_elements());

        assert!(history.redo(&mut scene));
        assert!(scene.has_dirty_elements());
        match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.corner_radius(), 10.0);
                assert!(rect.generate_fill_vertices().len() > 6);
            }
            other => panic!("rect, got {other:?}"),
        }
        let together = selection_style(&scene, &[1, 2]);
        assert_eq!(together.corner_radius, StyleField::Mixed);
    }

    fn red_blue() -> Vec<GradientStop> {
        vec![
            GradientStop {
                offset: 0.0,
                color: Color::RED,
            },
            GradientStop {
                offset: 1.0,
                color: Color::BLUE,
            },
        ]
    }

    #[test]
    fn gradient_is_one_undo_step_and_old_json_stays_solid() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::new(100.0, 40.0), Vec2::new(20.0, 10.0))
                .with_style(ElementStyle::fill_only(Color::BLACK)),
        );
        scene.add_circle(
            Circle::new(2, Vec2::new(40.0, 0.0), 10.0)
                .with_style(ElementStyle::fill_only(Color::BLACK)),
        );
        scene.add_rect(
            Rectangle::new(3, Vec2::new(80.0, 0.0), Vec2::new(20.0, 10.0))
                .with_style(ElementStyle::stroke_only(Color::GREEN, 4.0)),
        );

        assert!(SetStyleCommand::from_patch(
            &scene,
            &[],
            &StylePatch {
                fill_kind: Some(FillKind::Linear),
                fill_stops: Some(red_blue()),
                ..StylePatch::default()
            }
        )
        .is_none());
        let same = StylePatch {
            fill_kind: Some(FillKind::Solid),
            ..StylePatch::default()
        };
        assert!(SetStyleCommand::from_patch(&scene, &[1], &same).is_none());

        let patch = StylePatch {
            fill_kind: Some(FillKind::Linear),
            fill_stops: Some(red_blue()),
            fill_angle: Some(0.0),
            opacity: Some(0.5),
            ..StylePatch::default()
        };
        let mut history = CommandHistory::new();
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[1, 2], &patch).unwrap()),
            &mut scene,
        );
        assert_eq!(history.undo_count(), 1);
        assert!(scene.has_dirty_elements());
        assert_eq!(shape_style(&scene, 3).fill_kind, FillKind::Solid);

        let rect_vertices = match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => rect.generate_fill_vertices(),
            other => panic!("rect, got {other:?}"),
        };
        let left = rect_vertices
            .iter()
            .min_by(|a, b| a.position[0].partial_cmp(&b.position[0]).unwrap())
            .unwrap();
        let right = rect_vertices
            .iter()
            .max_by(|a, b| a.position[0].partial_cmp(&b.position[0]).unwrap())
            .unwrap();
        assert!(left.position[0] < 95.0);
        assert!(right.position[0] > 105.0);
        assert!(left.color[0] > 0.9 && left.color[2] < 0.1);
        assert!(right.color[2] > 0.9 && right.color[0] < 0.1);
        assert!((left.color[3] - 0.5).abs() < 1.0e-4);

        let circle_vertices = match scene.get(2).unwrap() {
            AnyElement::Circle(circle) => circle.generate_fill_vertices(),
            other => panic!("circle, got {other:?}"),
        };
        assert!(circle_vertices.iter().any(|vertex| vertex.color[0] > 0.9));
        assert!(circle_vertices.iter().any(|vertex| vertex.color[2] > 0.9));

        let stroke = match scene.get(3).unwrap() {
            AnyElement::Rectangle(rect) => rect.generate_stroke_vertices(),
            other => panic!("rect, got {other:?}"),
        };
        assert!(stroke.iter().all(|vertex| vertex.color[1] > 0.9));

        let mut cache = VertexCache::new();
        let _ = cache.sync(&mut scene);
        assert!(!scene.has_dirty_elements());
        assert!(cache.fill_slice(1).unwrap().len() >= 6);

        let json = scene.to_json().unwrap();
        assert!(json.contains("\"fill_kind\":\"linear\""));
        assert!(json.contains("\"fill_stops\""));
        let restored = Scene::from_json(&json).unwrap();
        let restored_style = shape_style(&restored, 1);
        assert_eq!(restored_style.fill_kind, FillKind::Linear);
        assert_eq!(restored_style.fill_stops, red_blue());
        assert!((restored_style.opacity - 0.5).abs() < 1.0e-5);
        assert_eq!(shape_style(&restored, 2).fill_kind, FillKind::Linear);

        assert!(history.undo(&mut scene));
        assert_eq!(shape_style(&scene, 1).fill_kind, FillKind::Solid);
        assert!(shape_style(&scene, 1).fill_stops.is_empty());
        assert_eq!(shape_fill(&scene, 1), Some(Color::BLACK));
        assert!(history.redo(&mut scene));
        assert_eq!(shape_style(&scene, 1).fill_kind, FillKind::Linear);

        let radial = StylePatch {
            fill_kind: Some(FillKind::Radial),
            fill_stops: Some(red_blue()),
            fill_center: Some(Vec2::new(1.0, -2.0)),
            fill_radius: Some(8.0),
            ..StylePatch::default()
        };
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[1], &radial).unwrap()),
            &mut scene,
        );
        assert_eq!(history.undo_count(), 2);
        let summary = selection_style(&scene, &[1, 2]);
        assert_eq!(summary.fill_kind, StyleField::Mixed);
        assert_eq!(summary.fill_stops, StyleField::Uniform(red_blue()));
        assert!(summary.has_shape);
        match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.style.fill_center, Vec2::new(1.0, -2.0));
                assert_eq!(rect.style.fill_radius, 8.0);
                let vertices = rect.generate_fill_vertices();
                assert!(vertices.iter().any(|vertex| {
                    let dx = vertex.position[0] - 101.0;
                    let dy = vertex.position[1] - 38.0;
                    dx * dx + dy * dy < 1.0 && vertex.color[0] > 0.9
                }));
            }
            other => panic!("rect, got {other:?}"),
        }

        scene.set_visible(1, false);
        let mut hidden_cache = VertexCache::new();
        let _ = hidden_cache.sync(&mut scene);
        assert_eq!(hidden_cache.fill_slice(1).map(|slice| slice.len()), Some(0));

        let legacy = serde_json::json!({
            "version": 1,
            "elements": [{
                "type": "rectangle",
                "data": {
                    "props": { "id": 9 },
                    "size": [4.0, 4.0]
                }
            }]
        });
        let legacy_scene = Scene::from_json(&legacy.to_string()).unwrap();
        let legacy_style = shape_style(&legacy_scene, 9);
        assert_eq!(legacy_style.fill_kind, FillKind::Solid);
        assert!(legacy_style.fill_stops.is_empty());
        assert_eq!(legacy_style.fill_angle, 0.0);
        assert_eq!(legacy_style.fill_center, Vec2::ZERO);
        assert_eq!(legacy_style.fill_radius, 0.0);
        assert_eq!(legacy_style, ElementStyle::new());
    }

    #[test]
    fn drop_shadow_is_one_style_patch_and_stays_out_of_hit_testing() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(40.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let before = match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => rect.world_bounds(),
            other => panic!("rect, got {other:?}"),
        };
        assert!(SetStyleCommand::from_patch(
            &scene,
            &[],
            &StylePatch {
                shadow_offset_x: Some(8.0),
                shadow_opacity: Some(0.4),
                ..StylePatch::default()
            },
        )
        .is_none());
        assert!(SetStyleCommand::from_patch(
            &scene,
            &[1],
            &StylePatch {
                shadow_blur: Some(0.0),
                ..StylePatch::default()
            },
        )
        .is_none());

        let patch = StylePatch {
            shadow_offset_x: Some(8.0),
            shadow_offset_y: Some(6.0),
            shadow_blur: Some(12.0),
            shadow_color: Some(Color::rgb(0.2, 0.0, 0.0)),
            shadow_opacity: Some(0.4),
            ..StylePatch::default()
        };
        let mut history = CommandHistory::new();
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[1], &patch).unwrap()),
            &mut scene,
        );
        assert_eq!(history.undo_count(), 1);
        assert!(scene.has_dirty_elements());
        match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.style.shadow.offset, Vec2::new(8.0, 6.0));
                assert_eq!(rect.style.shadow.blur, 12.0);
                assert!((rect.style.shadow.color.r - 0.2).abs() < 1.0e-5);
                assert!((rect.style.shadow.color.a - 0.4).abs() < 1.0e-5);
                assert_eq!(rect.world_bounds(), before);
                assert!(rect.contains_point(Vec2::ZERO));
                assert!(!rect.contains_point(Vec2::new(8.0, 6.0)));
                let fill = rect.generate_fill_vertices();
                let shadow = rect.shadow_vertices();
                assert!(!shadow.is_empty());
                assert!(shadow.len() > fill.len());
            }
            other => panic!("rect, got {other:?}"),
        }
        assert!(hit_test_point(&scene, Vec2::new(8.0, 6.0), &HitTestOptions::default()).is_empty());
        assert!(sample_paint_at(&scene, Vec2::new(8.0, 6.0)).is_none());

        let summary = selection_style(&scene, &[1, 2]);
        assert_eq!(summary.shadow_offset_x, StyleField::Mixed);
        assert_eq!(summary.shadow_blur, StyleField::Mixed);
        assert!(summary.shadow_supported);

        let mut cache = VertexCache::new();
        let synced = cache.sync(&mut scene);
        assert!(synced.regenerated >= 1);
        let painted = cache.fill_slice(1).unwrap();
        assert!(painted[0].color[3] < 0.5);
        assert!(painted.iter().any(|vertex| vertex.color[3] > 0.9));
        assert!(cache.sync(&mut scene).geometry_unchanged);

        let stored = match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => serde_json::to_value(rect).unwrap(),
            other => panic!("rect, got {other:?}"),
        };
        assert!(stored["style"]["shadow"].is_object());
        let restored: Rectangle = serde_json::from_value(stored).unwrap();
        assert_eq!(restored.style.shadow.blur, 12.0);
        let legacy = serde_json::json!({
            "props": { "id": 9 },
            "size": serde_json::to_value(Vec2::new(10.0, 10.0)).unwrap(),
        });
        let old: Rectangle = serde_json::from_value(legacy).unwrap();
        assert!(old.style.shadow.is_unset());
        assert!(serde_json::to_value(&old).unwrap().get("shadow").is_none());

        let sample = SampledPaint {
            id: 2,
            fill: Some(Color::BLUE),
            stroke: None,
        };
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[1], &sample.to_style_patch()).unwrap()),
            &mut scene,
        );
        match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.style.fill, Some(Color::BLUE));
                assert_eq!(rect.style.shadow.offset, Vec2::new(8.0, 6.0));
            }
            other => panic!("rect, got {other:?}"),
        }

        assert!(history.undo(&mut scene));
        assert!(history.undo(&mut scene));
        match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => assert!(rect.style.shadow.is_off()),
            other => panic!("rect, got {other:?}"),
        }

        let mut text = Text::new(3, Vec2::new(0.0, 30.0), "Hi");
        text.texture_id = Some("glyph".to_string());
        text.raster_revision = text.revision;
        scene.add_text(text);
        scene.add_image(Image::new(
            4,
            Vec2::new(80.0, 0.0),
            Vec2::new(12.0, 12.0),
            "photo".to_string(),
        ));
        let mut text_cmd = SetStyleCommand::from_patch(
            &scene,
            &[3, 4],
            &StylePatch {
                shadow_offset_x: Some(4.0),
                shadow_opacity: Some(0.5),
                ..StylePatch::default()
            },
        )
        .unwrap();
        text_cmd.execute(&mut scene);
        match scene.get(3).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.texture_id.as_deref(), Some("glyph"));
                assert!(!text.shadow.is_off());
                assert!(!text.shadow_vertices().is_empty());
            }
            other => panic!("text, got {other:?}"),
        }
        match scene.get(4).unwrap() {
            AnyElement::Image(image) => assert!(!image.shadow_vertices().is_empty()),
            other => panic!("image, got {other:?}"),
        }
        let mixed = selection_style(&scene, &[3, 4]);
        assert_eq!(mixed.shadow_offset_x, StyleField::Uniform(4.0));
        assert!(mixed.shadow_supported);

        let group_id = scene.group(&[1, 2]).unwrap();
        assert!(scene.set_visible(group_id, false));
        let mut hidden = VertexCache::new();
        let _ = hidden.sync(&mut scene);
        assert!(hidden.fill_slice(1).is_none_or(|slice| slice.is_empty()));
        let _ = DropShadow::default();
    }

    fn leaf_blend(scene: &Scene, id: ElementId) -> BlendMode {
        scene.get(id).unwrap().blend_mode()
    }

    #[test]
    fn blend_mode_is_one_undoable_set_style_and_noop_when_unchanged() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        scene.add_text(Text::new(2, Vec2::new(20.0, 0.0), "Hi"));
        scene.add_image(Image::new(
            3,
            Vec2::new(40.0, 0.0),
            Vec2::new(8.0, 8.0),
            "tex".to_string(),
        ));
        let group_id = scene.group(&[1, 2]).unwrap();

        let patch = StylePatch {
            blend_mode: Some(BlendMode::Multiply),
            ..StylePatch::default()
        };
        assert!(SetStyleCommand::from_patch(&scene, &[], &patch).is_none());
        assert!(SetStyleCommand::from_patch(
            &scene,
            &[1],
            &StylePatch {
                blend_mode: Some(BlendMode::Normal),
                ..StylePatch::default()
            },
        )
        .is_none());

        let mut locked = Scene::new();
        let mut rect = Rectangle::new(1, Vec2::ZERO, Vec2::new(4.0, 4.0));
        rect.props.locked = true;
        locked.add_rect(rect);
        assert!(SetStyleCommand::from_patch(&locked, &[1], &patch).is_none());

        let mut history = CommandHistory::new();
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[group_id, 2, 3], &patch).unwrap()),
            &mut scene,
        );
        assert_eq!(history.undo_count(), 1);
        assert_eq!(leaf_blend(&scene, 1), BlendMode::Multiply);
        assert_eq!(leaf_blend(&scene, 2), BlendMode::Multiply);
        assert_eq!(leaf_blend(&scene, 3), BlendMode::Multiply);
        assert!(scene.has_dirty_elements());
        let reported = selection_style(&scene, &[1, 2, 3]);
        assert_eq!(
            reported.blend_mode,
            StyleField::Uniform(BlendMode::Multiply)
        );

        assert!(history.undo(&mut scene));
        assert_eq!(leaf_blend(&scene, 1), BlendMode::Normal);
        assert_eq!(leaf_blend(&scene, 2), BlendMode::Normal);
        assert_eq!(leaf_blend(&scene, 3), BlendMode::Normal);
        assert!(history.redo(&mut scene));
        assert_eq!(leaf_blend(&scene, 3), BlendMode::Multiply);
    }

    #[test]
    fn mixed_blend_modes_and_legacy_json_default_to_normal() {
        let mut scene = Scene::new();
        let mut multiply = Rectangle::new(1, Vec2::ZERO, Vec2::new(4.0, 4.0));
        multiply.style.blend_mode = BlendMode::Multiply;
        scene.add_rect(multiply);
        let mut screen = Rectangle::new(2, Vec2::new(8.0, 0.0), Vec2::new(4.0, 4.0));
        screen.style.blend_mode = BlendMode::Screen;
        scene.add_rect(screen);

        let mixed = selection_style(&scene, &[1, 2]);
        assert_eq!(mixed.blend_mode, StyleField::Mixed);
        let mut history = CommandHistory::new();
        history.execute(
            Box::new(
                SetStyleCommand::from_patch(
                    &scene,
                    &[1, 2],
                    &StylePatch {
                        blend_mode: Some(BlendMode::Overlay),
                        ..StylePatch::default()
                    },
                )
                .unwrap(),
            ),
            &mut scene,
        );
        assert_eq!(history.undo_count(), 1);
        assert_eq!(leaf_blend(&scene, 1), BlendMode::Overlay);
        assert_eq!(leaf_blend(&scene, 2), BlendMode::Overlay);

        let mut cache = VertexCache::new();
        let sync = cache.sync(&mut scene);
        assert!(sync.full_upload || !sync.geometry_unchanged);
        let spans = cache.paint_spans();
        assert!(spans.iter().all(|span| span.blend == BlendMode::Overlay));

        let json = scene.to_json().unwrap();
        assert!(json.contains("\"blend_mode\":\"overlay\""));
        let restored = Scene::from_json(&json).unwrap();
        assert_eq!(leaf_blend(&restored, 1), BlendMode::Overlay);

        let size = serde_json::to_value(Vec2::new(2.0, 2.0)).unwrap();
        let legacy = serde_json::json!({
            "version": 1,
            "elements": [
                { "type": "rectangle", "data": { "props": { "id": 1 }, "size": size } },
                { "type": "text", "data": { "props": { "id": 2 }, "content": "Old" } },
                { "type": "image", "data": { "props": { "id": 3 }, "size": size, "texture_id": "t" } }
            ]
        });
        let old = Scene::from_json(&legacy.to_string()).unwrap();
        assert_eq!(leaf_blend(&old, 1), BlendMode::Normal);
        assert_eq!(leaf_blend(&old, 2), BlendMode::Normal);
        assert_eq!(leaf_blend(&old, 3), BlendMode::Normal);
        assert!(!old.to_json().unwrap().contains("blend_mode"));
    }

    #[test]
    fn layer_blur_is_one_undoable_patch_and_stays_out_of_hit_testing() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::ZERO, Vec2::new(80.0, 80.0))
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        scene.add_rect(
            Rectangle::new(2, Vec2::new(200.0, 0.0), Vec2::new(10.0, 10.0))
                .with_style(ElementStyle::fill_only(Color::BLUE)),
        );
        let before = match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => rect.world_bounds(),
            other => panic!("rect, got {other:?}"),
        };
        let blur_on = StylePatch {
            blur_radius: Some(4.0),
            ..StylePatch::default()
        };
        assert!(SetStyleCommand::from_patch(&scene, &[], &blur_on).is_none());
        assert!(scene.set_locked(1, true));
        assert!(SetStyleCommand::from_patch(&scene, &[1], &blur_on).is_none());
        assert!(scene.set_locked(1, false));
        assert!(SetStyleCommand::from_patch(
            &scene,
            &[1],
            &StylePatch {
                blur_radius: Some(0.0),
                ..StylePatch::default()
            },
        )
        .is_none());
        assert!(SetStyleCommand::from_patch(
            &scene,
            &[1],
            &StylePatch {
                blur_radius: Some(-2.0),
                ..StylePatch::default()
            },
        )
        .is_none());
        assert!(!selection_style(&scene, &[]).blur_supported);

        let mut history = CommandHistory::new();
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[1], &blur_on).unwrap()),
            &mut scene,
        );
        assert_eq!(history.undo_count(), 1);
        assert!(scene.has_dirty_elements());
        match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.style.blur_radius, 4.0);
                assert_eq!(rect.world_bounds(), before);
                assert!(rect.contains_point(Vec2::ZERO));
                assert!(!rect.contains_point(Vec2::new(44.0, 0.0)));
                assert!(rect.generate_fill_vertices().len() > 6);
            }
            other => panic!("rect, got {other:?}"),
        }
        assert!(
            hit_test_point(&scene, Vec2::new(44.0, 0.0), &HitTestOptions::default()).is_empty()
        );
        assert!(sample_paint_at(&scene, Vec2::new(44.0, 0.0)).is_none());

        let mut gradient = SetStyleCommand::from_patch(
            &scene,
            &[1],
            &StylePatch {
                fill_kind: Some(FillKind::Linear),
                fill_stops: Some(vec![
                    GradientStop {
                        offset: 0.0,
                        color: Color::RED,
                    },
                    GradientStop {
                        offset: 1.0,
                        color: Color::BLUE,
                    },
                ]),
                ..StylePatch::default()
            },
        )
        .unwrap();
        gradient.execute(&mut scene);
        assert_eq!(shape_style(&scene, 1).fill_kind, FillKind::Linear);
        assert!(shape_style(&scene, 1).uses_gradient());
        assert_eq!(shape_style(&scene, 1).blur_radius, 4.0);
        gradient.undo(&mut scene);
        assert_eq!(shape_style(&scene, 1).fill_kind, FillKind::Solid);
        assert_eq!(shape_style(&scene, 1).blur_radius, 4.0);

        let mixed = selection_style(&scene, &[1, 2]);
        assert_eq!(mixed.blur_radius, StyleField::Mixed);
        assert!(mixed.blur_supported);

        let stored = match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => serde_json::to_value(rect).unwrap(),
            other => panic!("rect, got {other:?}"),
        };
        assert_eq!(stored["style"]["blur_radius"], 4.0);
        let restored: Rectangle = serde_json::from_value(stored).unwrap();
        assert_eq!(restored.style.blur_radius, 4.0);
        let size = serde_json::to_value(Vec2::new(10.0, 10.0)).unwrap();
        let legacy = serde_json::json!({
            "props": { "id": 9 },
            "size": size,
        });
        let old: Rectangle = serde_json::from_value(legacy).unwrap();
        assert_eq!(old.style.blur_radius, 0.0);
        assert!(serde_json::to_value(&old).unwrap()["style"]
            .get("blur_radius")
            .is_none());
        let negative = serde_json::json!({
            "version": 1,
            "elements": [{
                "type": "rectangle",
                "data": {
                    "props": { "id": 9 },
                    "size": size,
                    "style": { "blur_radius": -4.0 }
                }
            }]
        });
        let loaded = Scene::from_json(&negative.to_string()).unwrap();
        assert_eq!(shape_style(&loaded, 9).blur_radius, 0.0);
        assert!(!loaded.to_json().unwrap().contains("blur_radius"));

        let mut cache = VertexCache::new();
        let synced = cache.sync(&mut scene);
        assert!(synced.regenerated >= 1);
        let painted = cache.fill_slice(1).unwrap();
        assert!(painted.len() > 6);
        assert!(painted.iter().any(|vertex| vertex.color[3] < 0.2));
        assert!(painted.iter().any(|vertex| vertex.color[3] > 0.9));
        assert!(cache.sync(&mut scene).geometry_unchanged);
        assert!(!scene.has_dirty_elements());

        let sample = sample_paint_at(&scene, Vec2::new(200.0, 0.0)).unwrap();
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[1], &sample.to_style_patch()).unwrap()),
            &mut scene,
        );
        match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.style.fill, Some(Color::BLUE));
                assert_eq!(rect.style.blur_radius, 4.0);
            }
            other => panic!("rect, got {other:?}"),
        }
        assert!(history.undo(&mut scene));
        assert!(history.undo(&mut scene));
        assert!(scene.has_dirty_elements());
        match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => {
                assert_eq!(rect.style.fill, Some(Color::RED));
                assert_eq!(rect.style.blur_radius, 0.0);
                assert_eq!(rect.generate_fill_vertices().len(), 6);
            }
            other => panic!("rect, got {other:?}"),
        }
        let _ = cache.sync(&mut scene);
        assert!(!scene.has_dirty_elements());

        let group_id = scene.group(&[1, 2]).unwrap();
        let mut grouped = SetStyleCommand::from_patch(
            &scene,
            &[group_id],
            &StylePatch {
                blur_radius: Some(6.0),
                ..StylePatch::default()
            },
        )
        .unwrap();
        grouped.execute(&mut scene);
        assert_eq!(shape_style(&scene, 1).blur_radius, 6.0);
        assert_eq!(shape_style(&scene, 2).blur_radius, 6.0);
        assert!(matches!(scene.get(group_id), Some(AnyElement::Group(_))));
        assert_eq!(
            selection_style(&scene, &[group_id]).blur_radius,
            StyleField::Uniform(6.0)
        );

        scene.add_frame(Frame::new(10, Vec2::new(400.0, 0.0), Vec2::new(80.0, 80.0)));
        scene.add_rect(Rectangle::new(
            11,
            Vec2::new(400.0, 0.0),
            Vec2::new(20.0, 20.0),
        ));
        let mut frame_history = CommandHistory::new();
        assert!(apply_reparent_into_frame(
            &mut frame_history,
            &mut scene,
            &[11],
            10
        ));
        let mut frame_blur = SetStyleCommand::from_patch(
            &scene,
            &[10],
            &StylePatch {
                blur_radius: Some(5.0),
                ..StylePatch::default()
            },
        )
        .unwrap();
        frame_blur.execute(&mut scene);
        assert_eq!(shape_style(&scene, 10).blur_radius, 5.0);
        assert_eq!(shape_style(&scene, 11).blur_radius, 0.0);
        assert!(selection_style(&scene, &[10]).blur_supported);

        let mut text = Text::new(3, Vec2::new(0.0, 300.0), "Hi");
        text.texture_id = Some("glyph".to_string());
        let revision = text.revision;
        text.raster_revision = revision;
        scene.add_text(text);
        let mut text_blur = SetStyleCommand::from_patch(
            &scene,
            &[3],
            &StylePatch {
                blur_radius: Some(3.0),
                ..StylePatch::default()
            },
        )
        .unwrap();
        text_blur.execute(&mut scene);
        match scene.get(3).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.blur_radius, 3.0);
                assert_eq!(text.texture_id.as_deref(), Some("glyph"));
                assert_eq!(text.raster_revision, revision);
            }
            other => panic!("text, got {other:?}"),
        }

        assert!(scene.set_visible(1, false));
        let mut hidden = VertexCache::new();
        let _ = hidden.sync(&mut scene);
        assert!(hidden.fill_slice(1).is_none_or(|slice| slice.is_empty()));
    }

    #[test]
    fn text_align_and_weight_are_one_undoable_set_style() {
        use crate::element::{FontWeight, TextAlign};

        let mut scene = Scene::new();
        scene.add_text(Text::new(1, Vec2::ZERO, "Hello"));
        scene.add_text(
            Text::new(2, Vec2::new(40.0, 0.0), "World")
                .with_align(TextAlign::Center)
                .with_font_weight(FontWeight::Bold),
        );
        scene.add_rect(Rectangle::new(
            3,
            Vec2::new(80.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));

        let mixed = selection_style(&scene, &[1, 2]);
        assert_eq!(mixed.text_align, StyleField::Mixed);
        assert_eq!(mixed.font_weight, StyleField::Mixed);
        assert!(mixed.has_text);
        let shape_only = selection_style(&scene, &[3]);
        assert_eq!(shape_only.text_align, StyleField::Absent);
        assert_eq!(shape_only.font_weight, StyleField::Absent);

        assert!(SetStyleCommand::from_patch(
            &scene,
            &[],
            &StylePatch {
                text_align: Some(TextAlign::Right),
                ..StylePatch::default()
            },
        )
        .is_none());
        assert!(SetStyleCommand::from_patch(
            &scene,
            &[3],
            &StylePatch {
                text_align: Some(TextAlign::Right),
                font_weight: Some(FontWeight::Bold),
                ..StylePatch::default()
            },
        )
        .is_none());
        assert!(SetStyleCommand::from_patch(
            &scene,
            &[1],
            &StylePatch {
                text_align: Some(TextAlign::Left),
                font_weight: Some(FontWeight::Regular),
                ..StylePatch::default()
            },
        )
        .is_none());

        let mut locked = Scene::new();
        let mut text = Text::new(1, Vec2::ZERO, "Lock");
        text.props.locked = true;
        locked.add_text(text);
        assert!(SetStyleCommand::from_patch(
            &locked,
            &[1],
            &StylePatch {
                text_align: Some(TextAlign::Center),
                ..StylePatch::default()
            },
        )
        .is_none());

        let mut history = CommandHistory::new();
        let mut textured = Text::new(4, Vec2::new(0.0, 40.0), "Raster");
        textured.texture_id = Some("glyph".to_string());
        textured.raster_revision = textured.revision;
        scene.add_text(textured);

        history.execute(
            Box::new(
                SetStyleCommand::from_patch(
                    &scene,
                    &[1, 4],
                    &StylePatch {
                        text_align: Some(TextAlign::Right),
                        font_weight: Some(FontWeight::Bold),
                        ..StylePatch::default()
                    },
                )
                .unwrap(),
            ),
            &mut scene,
        );
        assert_eq!(history.undo_count(), 1);
        assert!(scene.has_dirty_elements());
        match scene.get(1).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.align, TextAlign::Right);
                assert_eq!(text.font_weight, FontWeight::Bold);
                assert!(text.texture_id.is_none());
            }
            other => panic!("text, got {other:?}"),
        }
        match scene.get(4).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.align, TextAlign::Right);
                assert_eq!(text.font_weight, FontWeight::Bold);
                assert!(text.texture_id.is_none());
            }
            other => panic!("text, got {other:?}"),
        }
        let uniform = selection_style(&scene, &[1, 4]);
        assert_eq!(uniform.text_align, StyleField::Uniform(TextAlign::Right));
        assert_eq!(uniform.font_weight, StyleField::Uniform(FontWeight::Bold));

        assert!(history.undo(&mut scene));
        match scene.get(1).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.align, TextAlign::Left);
                assert_eq!(text.font_weight, FontWeight::Regular);
            }
            other => panic!("text, got {other:?}"),
        }
        assert!(history.redo(&mut scene));
        match scene.get(1).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.align, TextAlign::Right);
                assert_eq!(text.font_weight, FontWeight::Bold);
            }
            other => panic!("text, got {other:?}"),
        }

        let json = scene.to_json().unwrap();
        assert!(json.contains("\"align\":\"right\""));
        assert!(json.contains("\"font_weight\":\"bold\""));
        let restored = Scene::from_json(&json).unwrap();
        match restored.get(1).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.align, TextAlign::Right);
                assert_eq!(text.font_weight, FontWeight::Bold);
            }
            other => panic!("text, got {other:?}"),
        }

        let legacy = serde_json::json!({
            "version": 1,
            "elements": [{
                "type": "text",
                "data": { "props": { "id": 9 }, "content": "Old" }
            }]
        });
        let old = Scene::from_json(&legacy.to_string()).unwrap();
        match old.get(9).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.align, TextAlign::Left);
                assert_eq!(text.font_weight, FontWeight::Regular);
            }
            other => panic!("text, got {other:?}"),
        }
    }
    #[test]
    fn line_markers_are_one_undoable_set_style_and_noop_on_non_lines() {
        let mut scene = Scene::new();
        scene.add_line(Line::from_world(
            1,
            Vec2::ZERO,
            Vec2::new(80.0, 0.0),
            ElementStyle::stroke_only(Color::BLACK, 4.0),
            false,
        ));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(0.0, 20.0),
            Vec2::new(10.0, 10.0),
        ));

        let empty = StylePatch {
            end_marker: Some(LineMarker::Arrow),
            ..StylePatch::default()
        };
        assert!(SetStyleCommand::from_patch(&scene, &[], &empty).is_none());
        assert!(SetStyleCommand::from_patch(&scene, &[2], &empty).is_none());

        let summary = selection_style(&scene, &[1]);
        assert!(summary.has_line);
        assert_eq!(summary.end_marker, StyleField::Uniform(LineMarker::None));
        assert_eq!(summary.start_marker, StyleField::Uniform(LineMarker::None));

        let mut history = CommandHistory::new();
        history.execute(
            Box::new(
                SetStyleCommand::from_patch(
                    &scene,
                    &[1],
                    &StylePatch {
                        end_marker: Some(LineMarker::Arrow),
                        start_marker: Some(LineMarker::Triangle),
                        ..StylePatch::default()
                    },
                )
                .unwrap(),
            ),
            &mut scene,
        );
        assert_eq!(history.undo_count(), 1);
        assert!(scene.has_dirty_elements());
        match scene.get(1).unwrap() {
            AnyElement::Line(line) => {
                assert_eq!(line.end_marker(), LineMarker::Arrow);
                assert_eq!(line.start_marker(), LineMarker::Triangle);
                assert!(line.arrow());
                assert_eq!(line.generate_vertices().len(), 12);
            }
            other => panic!("line, got {other:?}"),
        }

        let json = scene.to_json().unwrap();
        assert!(json.contains("\"end_marker\":\"arrow\""));
        assert!(json.contains("\"start_marker\":\"triangle\""));
        assert!(json.contains("\"arrow\":true"));
        let restored = Scene::from_json(&json).unwrap();
        match restored.get(1).unwrap() {
            AnyElement::Line(line) => {
                assert_eq!(line.end_marker(), LineMarker::Arrow);
                assert_eq!(line.start_marker(), LineMarker::Triangle);
            }
            other => panic!("line, got {other:?}"),
        }

        let mixed = selection_style(&scene, &[1, 2]);
        assert!(mixed.has_line);
        assert_eq!(mixed.end_marker, StyleField::Uniform(LineMarker::Arrow));

        assert!(history.undo(&mut scene));
        match scene.get(1).unwrap() {
            AnyElement::Line(line) => {
                assert_eq!(line.end_marker(), LineMarker::None);
                assert_eq!(line.start_marker(), LineMarker::None);
                assert!(!line.arrow());
            }
            other => panic!("line, got {other:?}"),
        }
        assert!(history.redo(&mut scene));
        match scene.get(1).unwrap() {
            AnyElement::Line(line) => assert_eq!(line.end_marker(), LineMarker::Arrow),
            other => panic!("line, got {other:?}"),
        }
    }

    #[test]
    fn stroke_align_defaults_center_and_undoes() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::ZERO, Vec2::new(40.0, 20.0))
                .with_style(ElementStyle::stroke_only(Color::BLACK, 8.0)),
        );
        assert_eq!(shape_style(&scene, 1).stroke_align, StrokeAlign::Center);
        let old = scene.to_json().unwrap();
        assert!(!old.contains("stroke_align"));
        let restored = Scene::from_json(&old).unwrap();
        assert_eq!(shape_style(&restored, 1).stroke_align, StrokeAlign::Center);

        let patch = StylePatch {
            stroke_align: Some(StrokeAlign::Outside),
            ..StylePatch::default()
        };
        let mut history = CommandHistory::new();
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[1], &patch).unwrap()),
            &mut scene,
        );
        assert_eq!(shape_style(&scene, 1).stroke_align, StrokeAlign::Outside);
        assert!(scene.has_dirty_elements());
        history.undo(&mut scene);
        assert_eq!(shape_style(&scene, 1).stroke_align, StrokeAlign::Center);

        let summary = selection_style(&scene, &[1]);
        assert_eq!(
            summary.stroke_align,
            StyleField::Uniform(StrokeAlign::Center)
        );
        assert!(SetStyleCommand::from_patch(&scene, &[], &patch).is_none());
        scene.set_locked(1, true);
        assert!(SetStyleCommand::from_patch(&scene, &[1], &patch).is_none());
    }
}
