//! 两个本地连接通过内存 provider 同步后，场景收敛。

use infinitecanvas_collab::{CollabError, CollabSession, MemoryProvider};
use infinitecanvas_engine::math::{Color, Vec2};
use infinitecanvas_engine::{AnyElement, Circle, ElementStyle, Line, Rectangle};

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
