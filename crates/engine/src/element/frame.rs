//! 画板（Frame / Artboard）
//!
//! 轴对齐的命名矩形容器。子元素走和 [`super::Group`] 相同的 `parent` / `children`。
//! 画板自己的尺寸不随子元素变化。
//!
//! # 裁剪
//! 交互画布这一版**不裁剪**子元素：超出画板的部分仍画在画布上。
//! PNG / SVG 导出画板时，输出区域就是画板的世界包围盒，超出部分被裁掉。
//! 空画板仍按矩形尺寸导出。
//!
//! # 命中
//! 子元素优先。点在子元素上选中子元素；点在画板空白处选中画板。
//! 按住 Alt 时，光标下最上层的画板优先于它上面的子元素。

use super::{Element, ElementId, ElementProps, ElementStyle, Rectangle};
use infinitecanvas_math::{Color, Rect, Vec2};
use serde::{Deserialize, Serialize};

fn default_frame_name() -> String {
    "Frame".to_string()
}

fn default_frame_size() -> Vec2 {
    Vec2::new(100.0, 100.0)
}

/// 浅底加细描边，空画板在画布上仍能看见边界。
pub fn default_frame_style() -> ElementStyle {
    ElementStyle {
        fill: Some(Color::rgb(0.957, 0.957, 0.961)),
        stroke: Some(Color::rgb(0.631, 0.631, 0.667)),
        stroke_width: 1.0,
        ..ElementStyle::new()
    }
}

/// 命名画板。
///
/// `transform.translation` 是中心。`size` 是局部宽高，不随子元素重算。
/// JSON 里 `name`、`size`、`style`、`children` 都可省略。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::element::Frame;
/// use infinitecanvas_engine::Element;
/// use infinitecanvas_math::Vec2;
///
/// let frame = Frame::new(1, Vec2::new(40.0, 30.0), Vec2::new(80.0, 60.0)).with_name("封面");
/// assert_eq!(frame.name, "封面");
/// assert_eq!(frame.type_name(), "Frame");
/// assert!(frame.contains_point(Vec2::new(40.0, 30.0)));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// 元素属性。`transform` 是画板在世界中的位姿。
    pub props: ElementProps,
    /// 显示名称。缺省为 `"Frame"`。
    #[serde(default = "default_frame_name")]
    pub name: String,
    /// 局部宽高。缺省 100×100。中心是变换原点。
    #[serde(default = "default_frame_size")]
    pub size: Vec2,
    /// 填充和描边。缺省为浅底加细描边。
    #[serde(default = "default_frame_style")]
    pub style: ElementStyle,
    /// 直接子元素，顺序即大纲顺序。缺省为空。
    #[serde(default)]
    pub children: Vec<ElementId>,
}

impl Frame {
    /// 创建画板。
    ///
    /// # 参数
    /// - `id`: 元素 ID
    /// - `position`: 中心（世界坐标，旋转为 0、缩放为 1 时）
    /// - `size`: 宽高
    pub fn new(id: ElementId, position: Vec2, size: Vec2) -> Self {
        let mut props = ElementProps::new(id);
        props.transform.translation = position;
        Self {
            props,
            name: default_frame_name(),
            size,
            style: default_frame_style(),
            children: Vec::new(),
        }
    }

    /// 设置名称。
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 替换样式。
    pub fn with_style(mut self, style: ElementStyle) -> Self {
        self.style = style;
        self
    }

    /// 直接子元素 ID。
    pub fn child_ids(&self) -> &[ElementId] {
        &self.children
    }

    /// 用同样的变换和样式生成一个矩形，供顶点和导出复用矩形管线。
    pub fn shape_proxy(&self) -> Rectangle {
        let mut rect =
            Rectangle::new(self.props.id, Vec2::ZERO, self.size).with_style(self.style.clone());
        rect.props = self.props.clone();
        rect
    }
}

impl Element for Frame {
    fn props(&self) -> &ElementProps {
        &self.props
    }

    fn props_mut(&mut self) -> &mut ElementProps {
        &mut self.props
    }

    fn local_bounds(&self) -> Rect {
        Rect::from_center_size(Vec2::ZERO, self.size)
    }

    fn type_name(&self) -> &'static str {
        "Frame"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{
        apply_rename_frame, apply_reparent_into_frame, CommandHistory, TransformElementsCommand,
    };
    use crate::hit_test::click_target;
    use crate::{
        export_frame_png, export_frame_svg, export_selection_png, export_svg, Rectangle, Scene,
    };

    #[test]
    fn frame_hits_its_rectangle_and_round_trips() {
        let frame = Frame::new(3, Vec2::new(10.0, 20.0), Vec2::new(40.0, 20.0));
        assert!(frame.contains_point(Vec2::new(10.0, 20.0)));
        assert!(!frame.contains_point(Vec2::new(100.0, 100.0)));
        let json = serde_json::to_string(&frame).unwrap();
        let back: Frame = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "Frame");
        assert_eq!(back.size, Vec2::new(40.0, 20.0));
        assert!(back.children.is_empty());
    }

    #[test]
    fn omitted_frame_fields_use_defaults() {
        let frame: Frame = serde_json::from_str(r#"{"props":{"id":4}}"#).unwrap();
        assert_eq!(frame.name, "Frame");
        assert_eq!(frame.size, Vec2::new(100.0, 100.0));
        assert!(frame.style.fill.is_some());
        assert!(frame.children.is_empty());
        assert!(frame.props.visible);
    }

    #[test]
    fn old_document_without_frame_still_loads() {
        let json = r#"{"version":1,"next_id":2,"elements":[{"type":"rectangle","data":{"props":{"id":1},"size":[8,6]}}]}"#;
        let scene = Scene::from_json(json).unwrap();
        assert_eq!(scene.count(), 1);
        assert!(scene.get(1).unwrap().as_group().is_none());
    }

    #[test]
    fn frame_json_round_trip_keeps_name_children_and_parent() {
        let mut scene = Scene::new();
        let frame_id = scene
            .add_frame(Frame::new(1, Vec2::new(0.0, 0.0), Vec2::new(80.0, 40.0)).with_name("看板"));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(4.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let mut history = CommandHistory::new();
        assert!(apply_reparent_into_frame(
            &mut history,
            &mut scene,
            &[2],
            frame_id
        ));
        let json = scene.to_json().unwrap();
        let restored = Scene::from_json(&json).unwrap();
        let Some(crate::AnyElement::Frame(frame)) = restored.get(frame_id) else {
            panic!("frame missing");
        };
        assert_eq!(frame.name, "看板");
        assert_eq!(frame.children, vec![2]);
        assert_eq!(
            restored.get(2).unwrap().as_element().props().parent,
            Some(frame_id)
        );
    }

    #[test]
    fn child_is_hit_first_and_alt_selects_the_frame() {
        let mut scene = Scene::new();
        let frame_id = scene.add_frame(Frame::new(1, Vec2::ZERO, Vec2::new(100.0, 80.0)));
        scene.add_rect(Rectangle::new(2, Vec2::ZERO, Vec2::new(20.0, 20.0)));
        let mut history = CommandHistory::new();
        assert!(apply_reparent_into_frame(
            &mut history,
            &mut scene,
            &[2],
            frame_id
        ));
        assert_eq!(scene.find_at(Vec2::ZERO), Some(2));
        assert_eq!(click_target(&scene, Vec2::ZERO, false), Some(2));
        assert_eq!(click_target(&scene, Vec2::ZERO, true), Some(frame_id));
        assert_eq!(
            click_target(&scene, Vec2::new(40.0, 0.0), false),
            Some(frame_id)
        );
    }

    #[test]
    fn create_rename_reparent_and_resize_are_undoable() {
        let mut scene = Scene::new();
        let mut history = CommandHistory::new();
        let frame = Frame::new(1, Vec2::new(0.0, 0.0), Vec2::new(50.0, 40.0)).with_name("A");
        history.execute(
            Box::new(crate::AddElementCommand::new(crate::AnyElement::Frame(
                frame,
            ))),
            &mut scene,
        );
        assert!(scene.contains(1));
        assert!(!apply_rename_frame(&mut history, &mut scene, 1, "A"));
        assert!(history.can_undo());
        assert!(apply_rename_frame(&mut history, &mut scene, 1, "B"));
        scene.add_rect(Rectangle::new(2, Vec2::new(4.0, 0.0), Vec2::new(8.0, 8.0)));
        assert!(!apply_reparent_into_frame(&mut history, &mut scene, &[], 1));
        assert!(apply_reparent_into_frame(&mut history, &mut scene, &[2], 1));
        assert_eq!(scene.parent_of(2), Some(1));
        let before = scene.get(1).unwrap().as_element().props().transform;
        let mut scaled = before;
        scaled.scale = Vec2::new(2.0, 2.0);
        scene.set_world_transform(1, scaled).unwrap();
        history.push_executed(Box::new(TransformElementsCommand::new(
            vec![(1, before)],
            vec![(1, scaled)],
        )));
        let grown = scene.get(1).unwrap().as_element().world_bounds();
        assert!((grown.width - 100.0).abs() < 1e-2);
        assert!(history.undo(&mut scene));
        let restored = scene.get(1).unwrap().as_element().world_bounds();
        assert!((restored.width - 50.0).abs() < 1e-2);
        assert!(history.undo(&mut scene));
        assert_eq!(scene.parent_of(2), None);
        assert!(history.undo(&mut scene));
        match scene.get(1) {
            Some(crate::AnyElement::Frame(frame)) => assert_eq!(frame.name, "A"),
            other => panic!("expected frame, got {other:?}"),
        }
        assert!(history.undo(&mut scene));
        assert!(!scene.contains(1));
    }

    #[test]
    fn moving_a_frame_moves_its_children_without_resizing_the_board() {
        let mut scene = Scene::new();
        let frame_id = scene.add_frame(Frame::new(1, Vec2::ZERO, Vec2::new(40.0, 20.0)));
        scene.add_rect(Rectangle::new(2, Vec2::new(4.0, 0.0), Vec2::new(6.0, 6.0)));
        let mut history = CommandHistory::new();
        apply_reparent_into_frame(&mut history, &mut scene, &[2], frame_id);
        scene
            .translate_element(frame_id, Vec2::new(10.0, 0.0))
            .unwrap();
        let child = scene
            .get(2)
            .unwrap()
            .as_element()
            .props()
            .transform
            .translation;
        assert_eq!(child, Vec2::new(14.0, 0.0));
        let Some(crate::AnyElement::Frame(frame)) = scene.get(frame_id) else {
            panic!("frame");
        };
        assert_eq!(frame.size, Vec2::new(40.0, 20.0));
    }

    #[test]
    fn export_crops_to_frame_bounds_and_empty_frame_keeps_its_size() {
        let mut scene = Scene::new();
        let frame_id = scene.add_frame(
            Frame::new(1, Vec2::new(50.0, 40.0), Vec2::new(100.0, 80.0))
                .with_style(ElementStyle::fill_only(Color::rgb(0.9, 0.9, 0.9))),
        );
        scene.add_rect(
            Rectangle::new(2, Vec2::new(30.0, 40.0), Vec2::new(10.0, 10.0))
                .with_style(ElementStyle::fill_only(Color::BLUE)),
        );
        scene.add_rect(
            Rectangle::new(3, Vec2::new(20.0, 40.0), Vec2::new(10.0, 10.0))
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        let mut history = CommandHistory::new();
        apply_reparent_into_frame(&mut history, &mut scene, &[2], frame_id);
        scene
            .get_mut(frame_id)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .selected = true;

        let png = export_frame_png(&scene, frame_id, 1.0).unwrap();
        let (width, height, pixels) = decode_png(&png);
        assert_eq!((width, height), (100, 80));
        let selected = export_selection_png(&scene, 1.0).unwrap();
        let (sw, sh, _) = decode_png(&selected);
        assert_eq!((sw, sh), (100, 80));

        let blue = pixel_at(&pixels, width, 30, 40);
        assert!(
            blue[2] > blue[0] && blue[2] > 200,
            "child inside frame: {blue:?}"
        );
        let overlapped = pixel_at(&pixels, width, 20, 40);
        assert!(
            overlapped[2] > 180,
            "non-child overlapping the board is omitted: {overlapped:?}"
        );

        let svg = export_frame_svg(&scene, frame_id).unwrap();
        assert!(svg.contains("overflow=\"hidden\""));
        assert!(svg.contains("viewBox=\"0 0 100 80\""));
        assert!(svg.contains("<rect"));
        let selected_svg = export_svg(&scene, true);
        assert!(selected_svg.contains("viewBox=\"0 0 100 80\""));
        assert!(selected_svg.contains("overflow=\"hidden\""));

        let mut empty = Scene::new();
        empty.add_frame(Frame::new(9, Vec2::new(40.0, 30.0), Vec2::new(80.0, 60.0)));
        let png = export_frame_png(&empty, 9, 1.0).unwrap();
        let (width, height, _) = decode_png(&png);
        assert_eq!((width, height), (80, 60));
    }

    fn decode_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        buf.truncate(info.buffer_size());
        (info.width, info.height, buf)
    }

    fn pixel_at(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let index = ((y as usize) * (width as usize) + (x as usize)) * 4;
        [
            pixels[index],
            pixels[index + 1],
            pixels[index + 2],
            pixels[index + 3],
        ]
    }
}
