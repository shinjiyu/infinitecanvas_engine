//! 两个本地连接通过内存 provider 同步后，场景收敛。

use infinitecanvas_collab::{CollabError, CollabSession, MemoryProvider, MAX_PEER_SLOT};
use infinitecanvas_engine::math::{Color, Transform, Vec2};
use infinitecanvas_engine::{AnyElement, Circle, ElementStyle, Line, Rectangle, Scene, Text};

fn rect(id: u64, width: f32) -> AnyElement {
    let mut rect = Rectangle::new(id, Vec2::new(10.0, 20.0), Vec2::new(width, 40.0));
    rect.props.set_rotation(0.5);
    rect.props.z_index = 4;
    rect.props.selected = true;
    rect.style = ElementStyle::fill_only(Color::RED);
    AnyElement::Rectangle(rect)
}

fn paint(element: &mut AnyElement, color: Color) {
    match element {
        AnyElement::Rectangle(shape) => shape.style.fill = Some(color),
        AnyElement::Circle(shape) => shape.style.fill = Some(color),
        _ => panic!("expected a filled shape"),
    }
}

#[test]
fn two_peers_converge_after_add_and_delete() -> Result<(), CollabError> {
    let mut a = CollabSession::with_client_id(1, 1);
    let mut b = CollabSession::with_client_id(2, 2);

    let rect_id = a.allocate_id();
    a.insert(rect(rect_id, 30.0))?;
    assert_eq!(a.scene().count(), 1);
    assert_eq!(b.scene().count(), 0);
    assert!(
        !a.scene()
            .get(rect_id)
            .unwrap()
            .as_element()
            .props()
            .selected
    );

    let circle_id = b.allocate_id();
    b.insert(AnyElement::Circle(
        Circle::new(circle_id, Vec2::new(-3.0, 4.0), 5.5)
            .with_style(ElementStyle::fill_only(Color::GREEN)),
    ))?;

    MemoryProvider::sync(&mut a, &mut b)?;
    assert_eq!(a.scene(), b.scene());
    assert_eq!(a.scene().count(), 2);
    assert!(a.scene().contains(rect_id) && a.scene().contains(circle_id));

    assert!(a.delete(rect_id)?);
    assert!(!a.scene().contains(rect_id));
    MemoryProvider::sync(&mut a, &mut b)?;
    assert_eq!(a.scene(), b.scene());
    assert!(!b.scene().contains(rect_id));
    assert!(b.scene().contains(circle_id));

    let line_id = a.allocate_id();
    a.insert(AnyElement::Line(Line::from_world(
        line_id,
        Vec2::ZERO,
        Vec2::new(30.0, 0.0),
        ElementStyle::stroke_only(Color::BLACK, 2.0),
        true,
    )))?;
    let mut circle = b.scene().get(circle_id).unwrap().clone();
    paint(&mut circle, Color::BLACK);
    b.update(circle)?;
    MemoryProvider::sync(&mut a, &mut b)?;
    assert_eq!(a.scene(), b.scene());
    assert_eq!(a.scene().count(), 2);
    assert!(a.scene().contains(line_id));

    let mut replay = CollabSession::with_client_id(3, 3);
    replay.apply_update(&a.encode_state_v1())?;
    assert_eq!(replay.scene(), a.scene());
    replay.apply_update(&a.encode_state_v1())?;
    assert_eq!(replay.scene(), b.scene());
    Ok(())
}

#[test]
fn concurrent_edits_converge_independent_of_apply_order() -> Result<(), CollabError> {
    let mut a = CollabSession::with_client_id(1, 1);
    let mut b = CollabSession::with_client_id(2, 2);
    let id = 42;
    a.insert(rect(id, 10.0))?;
    MemoryProvider::sync(&mut a, &mut b)?;

    let mut red = a.scene().get(id).unwrap().clone();
    paint(&mut red, Color::RED);
    red.as_element_mut().props_mut().z_index = 1;
    let mut green = b.scene().get(id).unwrap().clone();
    paint(&mut green, Color::GREEN);
    green.as_element_mut().props_mut().z_index = 9;
    a.update(red)?;
    b.update(green)?;

    let update_a = a.encode_state_v1();
    let update_b = b.encode_state_v1();
    let mut a_then_b = CollabSession::with_client_id(3, 3);
    let mut b_then_a = CollabSession::with_client_id(4, 4);
    a_then_b.apply_update(&update_a)?;
    a_then_b.apply_update(&update_b)?;
    b_then_a.apply_update(&update_b)?;
    b_then_a.apply_update(&update_a)?;
    assert_eq!(a_then_b.scene(), b_then_a.scene());

    MemoryProvider::sync(&mut a, &mut b)?;
    assert_eq!(a.scene(), b.scene());
    assert_eq!(a.scene(), a_then_b.scene());
    assert_eq!(a.scene().count(), 1);

    a.delete(id)?;
    let mut resized = b.scene().get(id).unwrap().clone();
    match &mut resized {
        AnyElement::Rectangle(shape) => shape.size = Vec2::new(80.0, 10.0),
        _ => panic!("expected rectangle"),
    }
    b.update(resized)?;
    let delete_update = a.encode_diff_v1(&b.state_vector_v1())?;
    let edit_update = b.encode_diff_v1(&a.state_vector_v1())?;
    let mut delete_first = CollabSession::with_client_id(5, 5);
    delete_first.apply_update(&a.encode_state_v1())?;
    delete_first.apply_update(&edit_update)?;
    let mut edit_first = CollabSession::with_client_id(6, 6);
    edit_first.apply_update(&b.encode_state_v1())?;
    edit_first.apply_update(&delete_update)?;
    assert_eq!(delete_first.scene(), edit_first.scene());

    MemoryProvider::sync(&mut a, &mut b)?;
    assert_eq!(a.scene(), b.scene());
    assert_eq!(a.scene(), delete_first.scene());
    Ok(())
}

#[test]
fn same_id_inserts_and_local_guards_converge() -> Result<(), CollabError> {
    let mut a = CollabSession::with_client_id(1, 1);
    let mut b = CollabSession::with_client_id(2, 2);
    a.insert(rect(7, 4.0))?;
    b.insert(rect(7, 12.0))?;
    MemoryProvider::sync(&mut a, &mut b)?;
    assert_eq!(a.scene(), b.scene());
    assert_eq!(a.scene().count(), 1);

    let id = a.allocate_id();
    let element = rect(id, 1.0);
    a.insert(element.clone())?;
    assert!(matches!(a.insert(element), Err(CollabError::Duplicate(dup)) if dup == id));
    assert!(matches!(
        a.update(rect(99, 1.0)),
        Err(CollabError::Missing(99))
    ));
    assert!(!a.delete(99)?);

    let before = b.encode_state_v1();
    let circle_id = b.allocate_id();
    b.insert(AnyElement::Circle(Circle::new(circle_id, Vec2::ZERO, 1.0)))?;
    assert_ne!(before, b.encode_state_v1());
    Ok(())
}

#[test]
fn text_syncs_content_style_and_drops_texture_id() -> Result<(), CollabError> {
    let mut a = CollabSession::with_client_id(1, 1);
    let mut b = CollabSession::with_client_id(2, 2);
    let id = a.allocate_id();
    let color = Color::new(0.2, 0.4, 0.6, 1.0);
    let mut text = Text::new(id, Vec2::new(12.0, 34.0), "Hello 你好")
        .with_font_size(24.0)
        .with_color(color);
    text.texture_id = Some("text-local-r1-l0".to_string());
    a.insert(AnyElement::Text(text))?;

    MemoryProvider::sync(&mut a, &mut b)?;
    assert_eq!(a.scene(), b.scene());

    match b.scene().get(id) {
        Some(AnyElement::Text(remote)) => {
            assert_eq!(remote.content, "Hello 你好");
            assert_eq!(remote.font_size, 24.0);
            assert_eq!(remote.color, color);
            assert!(remote.texture_id.is_none());
        }
        other => panic!("peer B did not materialize text {id}, got {other:?}"),
    }
    Ok(())
}

fn world_transform(scene: &Scene, id: u64) -> Transform {
    scene.get(id).unwrap().as_element().props().transform
}

#[test]
fn group_lock_and_ungroup_sync_hierarchy_and_world_transforms() -> Result<(), CollabError> {
    let mut a = CollabSession::with_client_id(1, 1);
    let mut b = CollabSession::with_client_id(2, 2);

    let rect_id = a.allocate_id();
    a.insert(AnyElement::Rectangle(Rectangle::new(
        rect_id,
        Vec2::new(0.0, 0.0),
        Vec2::new(20.0, 10.0),
    )))?;
    let text_id = a.allocate_id();
    a.insert(AnyElement::Text(Text::new(
        text_id,
        Vec2::new(80.0, 10.0),
        "Grouped",
    )))?;

    let group_id = a.group(&[rect_id, text_id])?;
    assert_eq!(group_id >> 32, 1);
    assert_ne!(group_id, rect_id);
    assert_ne!(group_id, text_id);

    let frame = world_transform(a.scene(), group_id);
    let mut moved = frame;
    moved.translation += Vec2::new(12.0, -4.0);
    a.set_world_transform(group_id, moved)?;
    a.set_locked(group_id, true)?;

    let rect_world = world_transform(a.scene(), rect_id);
    let text_world = world_transform(a.scene(), text_id);
    let group_world = world_transform(a.scene(), group_id);
    assert_ne!(rect_world.translation, Vec2::new(0.0, 0.0));
    assert_ne!(text_world.translation, Vec2::new(80.0, 10.0));

    MemoryProvider::sync(&mut a, &mut b)?;
    assert_eq!(a.scene(), b.scene());
    assert_eq!(a.scene().scene_tree(), b.scene().scene_tree());

    assert_eq!(b.scene().parent_of(rect_id), Some(group_id));
    assert_eq!(b.scene().parent_of(text_id), Some(group_id));
    assert!(b.scene().parent_of(group_id).is_none());
    assert_eq!(b.scene().children_of(group_id), vec![rect_id, text_id]);
    assert!(b.scene().get(group_id).unwrap().as_element().props().locked);
    assert_eq!(
        b.scene()
            .get(group_id)
            .unwrap()
            .as_group()
            .unwrap()
            .children,
        vec![rect_id, text_id]
    );
    assert_eq!(world_transform(b.scene(), rect_id), rect_world);
    assert_eq!(world_transform(b.scene(), text_id), text_world);
    assert_eq!(world_transform(b.scene(), group_id), group_world);

    let tree = b.scene().scene_tree();
    assert_eq!(tree.roots.len(), 1);
    assert_eq!(tree.roots[0].id, group_id);
    assert_eq!(tree.roots[0].element_type, "Group");
    assert!(tree.roots[0].parent_id.is_none());
    assert!(tree.roots[0].locked);
    assert_eq!(tree.roots[0].children.len(), 2);
    assert_eq!(tree.roots[0].children[0].id, rect_id);
    assert_eq!(tree.roots[0].children[0].parent_id, Some(group_id));
    assert_eq!(tree.roots[0].children[0].element_type, "Rectangle");
    assert_eq!(tree.roots[0].children[1].id, text_id);
    assert_eq!(tree.roots[0].children[1].parent_id, Some(group_id));
    assert_eq!(tree.roots[0].children[1].element_type, "Text");
    match b.scene().get(text_id) {
        Some(AnyElement::Text(text)) => assert_eq!(text.content, "Grouped"),
        other => panic!("peer B did not materialize grouped text {text_id}, got {other:?}"),
    }

    assert!(a.ungroup(group_id).is_err());
    assert!(a.scene().contains(group_id));
    assert_eq!(a.scene().parent_of(rect_id), Some(group_id));

    a.set_locked(group_id, false)?;
    let children = a.ungroup(group_id)?;
    assert_eq!(children, vec![rect_id, text_id]);
    assert_eq!(world_transform(a.scene(), rect_id), rect_world);
    assert_eq!(world_transform(a.scene(), text_id), text_world);
    assert!(a.scene().parent_of(rect_id).is_none());
    assert!(a.scene().parent_of(text_id).is_none());
    assert!(!a.scene().contains(group_id));

    MemoryProvider::sync(&mut a, &mut b)?;
    assert_eq!(a.scene(), b.scene());
    assert_eq!(a.scene().scene_tree(), b.scene().scene_tree());
    assert!(!b.scene().contains(group_id));
    assert!(b.scene().parent_of(rect_id).is_none());
    assert!(b.scene().parent_of(text_id).is_none());
    assert_eq!(world_transform(b.scene(), rect_id), rect_world);
    assert_eq!(world_transform(b.scene(), text_id), text_world);
    assert_eq!(b.scene().scene_tree().roots.len(), 2);
    assert!(b
        .scene()
        .scene_tree()
        .roots
        .iter()
        .all(|node| node.parent_id.is_none() && node.children.is_empty()));
    Ok(())
}

#[test]
fn allocated_id_stays_within_js_safe_integer() {
    let mut session = CollabSession::with_client_id(9, MAX_PEER_SLOT);
    let id = session.allocate_id();
    assert_eq!(id, (u64::from(MAX_PEER_SLOT) << 32) | 1);
    assert!(id <= (1u64 << 53) - 1);
    let again = session.allocate_id();
    assert_eq!(again, id + 1);
    assert!(again <= (1u64 << 53) - 1);
}

#[test]
#[should_panic(expected = "21 bits")]
fn peer_slot_above_21_bits_is_rejected() {
    let _session = CollabSession::with_client_id(9, 1u32 << 21);
}
