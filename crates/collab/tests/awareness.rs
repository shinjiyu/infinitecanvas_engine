//! Awareness 在场：光标 / 选区同步，离开清理，不进场景 JSON。

use infinitecanvas_collab::{CollabError, CollabSession, MemoryProvider, PresenceState};
use infinitecanvas_engine::math::Vec2;
use infinitecanvas_engine::{AnyElement, Rectangle};

#[test]
fn peers_see_remote_cursor_and_selection_via_memory_provider() -> Result<(), CollabError> {
    let mut a = CollabSession::with_client_id(1, 1);
    let mut b = CollabSession::with_client_id(2, 2);

    a.set_local_presence(
        PresenceState::new("Ada", "#4090f2")
            .with_cursor(10.5, 20.25)
            .with_selection([42, 7]),
    )?;
    b.set_local_presence(
        PresenceState::new("Bob", "#36d399")
            .with_cursor(-3.0, 8.0)
            .with_selection([99]),
    )?;

    MemoryProvider::sync(&mut a, &mut b)?;

    let from_a = b.remote_presences();
    assert_eq!(from_a.len(), 1);
    assert_eq!(from_a[0].client_id, 1);
    assert_eq!(from_a[0].state.name, "Ada");
    assert_eq!(from_a[0].state.color, "#4090f2");
    assert_eq!(
        from_a[0].state.cursor.map(|c| (c.x, c.y)),
        Some((10.5, 20.25))
    );
    assert_eq!(from_a[0].state.selection, vec![42, 7]);

    let from_b = a.remote_presences();
    assert_eq!(from_b.len(), 1);
    assert_eq!(from_b[0].client_id, 2);
    assert_eq!(from_b[0].state.name, "Bob");
    assert_eq!(from_b[0].state.selection, vec![99]);

    // 本地用户不出现在 remote_presences 里。
    assert!(a
        .remote_presences()
        .iter()
        .all(|p| p.client_id != a.client_id()));
    assert!(b
        .remote_presences()
        .iter()
        .all(|p| p.client_id != b.client_id()));
    Ok(())
}

#[test]
fn leave_clears_remote_presence_and_scene_json_stays_clean() -> Result<(), CollabError> {
    let mut a = CollabSession::with_client_id(1, 1);
    let mut b = CollabSession::with_client_id(2, 2);

    let id = a.allocate_id();
    a.insert(AnyElement::Rectangle(Rectangle::new(
        id,
        Vec2::new(1.0, 2.0),
        Vec2::new(3.0, 4.0),
    )))?;
    a.set_local_presence(
        PresenceState::new("Ada", "#4090f2")
            .with_cursor(1.0, 1.0)
            .with_selection([id]),
    )?;
    MemoryProvider::sync(&mut a, &mut b)?;
    assert_eq!(b.remote_presences().len(), 1);

    a.clear_local_presence();
    assert!(a.local_presence().is_none());
    MemoryProvider::sync(&mut a, &mut b)?;
    assert!(
        b.remote_presences().is_empty(),
        "cleared awareness must disappear on the peer"
    );

    // 场景仍有元素；awareness 名字/颜色不进场景 JSON，也不进 Yjs 文档更新。
    assert!(b.scene().contains(id));
    let scene_json = b.scene().to_json().expect("scene encodes");
    assert!(!scene_json.contains("Ada"));
    assert!(!scene_json.contains("#4090f2"));
    assert!(!scene_json.contains("cursor"));

    let doc_bytes = b.encode_state_v1();
    let doc_text = String::from_utf8_lossy(&doc_bytes);
    assert!(!doc_text.contains("Ada"));
    assert!(!doc_text.contains("#4090f2"));
    Ok(())
}

#[test]
fn awareness_update_bytes_round_trip_without_touching_scene() -> Result<(), CollabError> {
    let mut a = CollabSession::with_client_id(1, 1);
    let mut b = CollabSession::with_client_id(2, 2);
    assert_eq!(a.scene().count(), 0);
    assert_eq!(b.scene().count(), 0);

    a.set_local_presence(PresenceState::new("Ada", "#f24040").with_cursor(5.0, 6.0))?;
    let bytes = a.encode_awareness_update()?;
    b.apply_awareness_update(&bytes)?;

    assert_eq!(b.scene().count(), 0);
    assert_eq!(a.scene().count(), 0);
    let remotes = b.remote_presences();
    assert_eq!(remotes.len(), 1);
    assert_eq!(remotes[0].state.color, "#f24040");
    Ok(())
}

#[test]
fn selection_changes_propagate_on_resync() -> Result<(), CollabError> {
    let mut a = CollabSession::with_client_id(1, 1);
    let mut b = CollabSession::with_client_id(2, 2);
    a.set_local_presence(PresenceState::new("Ada", "#4090f2").with_selection([1]))?;
    MemoryProvider::sync(&mut a, &mut b)?;
    assert_eq!(b.remote_presences()[0].state.selection, vec![1]);

    a.set_local_presence(
        PresenceState::new("Ada", "#4090f2")
            .with_cursor(9.0, 9.0)
            .with_selection([2, 3]),
    )?;
    MemoryProvider::sync(&mut a, &mut b)?;
    let remote = &b.remote_presences()[0].state;
    assert_eq!(remote.selection, vec![2, 3]);
    assert_eq!(remote.cursor.map(|c| (c.x, c.y)), Some((9.0, 9.0)));
    Ok(())
}
