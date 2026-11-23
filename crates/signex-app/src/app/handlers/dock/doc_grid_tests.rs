//! Mockup row 40: typing a Visible Grid or Snap Grid pitch.

use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::panels::{DocGridField, DocGridMsg, PanelMsg};
use signex_types::coord::Unit;

fn app() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app
}

fn send(app: &mut Signex, msg: DocGridMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::DocGrid(msg))));
}

fn type_pitch(app: &mut Signex, field: DocGridField, text: &str) {
    send(app, DocGridMsg::DraftChanged(field, text.into()));
}

#[test]
fn a_typed_pitch_applies_only_on_enter() {
    let mut app = app();
    let before = app.ui_state.visible_grid_mm;
    type_pitch(&mut app, DocGridField::Visible, "3");
    assert_eq!(app.ui_state.visible_grid_mm, before, "not per keystroke");
    send(&mut app, DocGridMsg::DraftSubmitted(DocGridField::Visible));
    assert_eq!(app.ui_state.visible_grid_mm, 3.0);
    assert_eq!(app.document_state.panel_ctx.visible_grid_draft, None);
}

#[test]
fn refused_text_stays_and_the_pitch_does_not_change() {
    let mut app = app();
    let before = app.ui_state.grid_size_mm;
    type_pitch(&mut app, DocGridField::Snap, "abc");
    send(&mut app, DocGridMsg::DraftSubmitted(DocGridField::Snap));
    assert_eq!(app.ui_state.grid_size_mm, before);
    assert_eq!(
        app.document_state.panel_ctx.snap_grid_draft.as_deref(),
        Some("abc")
    );
}

#[test]
fn picking_a_row_drops_the_typed_text() {
    let mut app = app();
    type_pitch(&mut app, DocGridField::Snap, "abc");
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::SetGridSize(
        2.54,
    ))));
    assert_eq!(app.document_state.panel_ctx.snap_grid_draft, None);
}

/// Text typed in one unit is never read in another.
#[test]
fn changing_the_unit_drops_the_typed_text() {
    let mut app = app();
    type_pitch(&mut app, DocGridField::Visible, "50");
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::SetUnit(
        Unit::Mil,
    ))));
    assert_eq!(app.document_state.panel_ctx.visible_grid_draft, None);
}
