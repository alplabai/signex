//! #111 — an idle cursor move only refreshes the cached pointer position;
//! an active layout drag still runs the full drag handling.

#![allow(clippy::let_underscore_must_use)]

use signex_app::app::{DragTarget, Message, Signex, UiMsg};

#[test]
fn idle_cursor_move_only_records_the_position() {
    let (mut app, _t) = Signex::new();
    assert!(!app.is_layout_drag_active());
    let _ = app.update(Message::Ui(UiMsg::DragMove(12.0, 34.0)));
    assert_eq!(app.interaction_state.last_mouse_pos, (12.0, 34.0));
}

#[test]
fn active_panel_drag_still_resizes() {
    let (mut app, _t) = Signex::new();
    let start = app.ui_state.left_width;
    let _ = app.update(Message::Ui(UiMsg::DragStart(DragTarget::LeftPanel)));
    assert!(app.is_layout_drag_active());
    let _ = app.update(Message::Ui(UiMsg::DragMove(100.0, 0.0)));
    let _ = app.update(Message::Ui(UiMsg::DragMove(160.0, 0.0)));
    assert_eq!(app.ui_state.left_width, (start + 60.0).clamp(100.0, 500.0));
    let _ = app.update(Message::Ui(UiMsg::DragEnd));
    assert!(!app.is_layout_drag_active());
}
