//! A note's text typed in the Properties panel of an undocked window, without
//! Ctrl+Enter, reaches its sheet when that window goes away: closed, or its
//! tab put back in the main window. The main window's tab switch is the
//! control.

use iced::Task;
use iced::widget::text_editor::{Action, Edit, Motion};

use super::note_draft_persist_tests::{
    note_item, note_text, switch_to, two_sheets, type_into_note,
};
use crate::app::{Message, Signex, WindowMsg};
use crate::dock::DockMessage;
use crate::panels::{PanelMsg, PlacedEditMsg};

/// `a` in an undocked window, `b` active in the main window, and "!" typed
/// at the end of the note's box in the window's Properties.
fn typed_in_undocked_window() -> (
    tempfile::TempDir,
    Signex,
    std::path::PathBuf,
    iced::window::Id,
) {
    let (dir, mut app, a, _b) = two_sheets();
    switch_to(&mut app, 1);
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        crate::app::state::WindowKind::UndockedTab {
            path: a.clone(),
            title: "a".into(),
        },
    );
    let mut slot = crate::canvas::CanvasSlot::new();
    let item = note_item();
    slot.selected = vec![item];
    app.interaction_state.canvases.insert(window, slot);
    drop(app.run_in_window(window, true, |app| {
        app.update_selection_info();
        Task::none()
    }));
    let _ = app.update(Message::Ui(crate::app::UiMsg::Noop));
    for action in [
        Action::Move(Motion::DocumentEnd),
        Action::Edit(Edit::Insert('!')),
    ] {
        let msg = Message::Dock(DockMessage::Panel(PanelMsg::PlacedEdit(
            PlacedEditMsg::NoteActed { item, action },
        )));
        let msg = app.scope_dock_message(window, msg);
        let _ = app.update(msg);
    }
    let draft = app.document_state.window_panel_ctx[&window]
        .placed
        .note
        .as_ref();
    assert!(
        draft.is_some_and(|d| d.edited && d.content.text().starts_with("note!")),
        "the window's box holds the typed text"
    );
    assert_eq!(note_text(&app, &a), "note", "still a draft");
    (dir, app, a, window)
}

#[test]
fn closing_an_undocked_window_applies_its_typed_note() {
    let (_dir, mut app, a, window) = typed_in_undocked_window();
    let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(window)));
    assert_eq!(note_text(&app, &a), "note!");
    assert!(app.document_state.is_document_dirty(&a));
}

#[test]
fn reattaching_an_undocked_tab_applies_its_typed_note() {
    let (_dir, mut app, a, window) = typed_in_undocked_window();
    let _ = app.update(Message::Window(WindowMsg::ReattachTab(window)));
    assert_eq!(note_text(&app, &a), "note!");
    assert!(app.document_state.is_document_dirty(&a));
}

/// Close Document run inside the window, then the OS close it asks for.
/// Already true before #2649 (the close-tab path commits the note itself);
/// this pins it rather than testing the fix.
#[test]
fn closing_the_document_from_its_undocked_window_applies_its_typed_note() {
    let (_dir, mut app, a, window) = typed_in_undocked_window();
    let index = app.document_state.tabs.iter().position(|t| t.path == a);
    let index = index.expect("a has a tab");
    drop(app.run_in_window(window, true, |app| app.close_tab_parking_dirty(index)));
    let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(window)));
    assert_eq!(note_text(&app, &a), "note!");
    assert!(app.document_state.is_document_dirty(&a));
}

/// A handler that ends the window itself: its context is not put back, so
/// the typed box must still reach the sheet.
#[test]
fn a_window_ended_inside_run_in_window_applies_its_typed_note() {
    let (_dir, mut app, a, window) = typed_in_undocked_window();
    drop(app.run_in_window(window, true, |app| {
        app.ui_state.windows.remove(&window);
        Task::none()
    }));
    let _ = app.update(Message::Ui(crate::app::UiMsg::Noop));
    assert_eq!(note_text(&app, &a), "note!");
    assert!(app.document_state.is_document_dirty(&a));
}

/// Control: the same typing in the main window reaches the sheet on a tab
/// switch.
#[test]
fn a_main_window_tab_switch_applies_the_typed_note() {
    let (_dir, mut app, a, _b) = two_sheets();
    type_into_note(&mut app, '!');
    assert_eq!(note_text(&app, &a), "note", "still a draft");
    switch_to(&mut app, 1);
    assert_eq!(note_text(&app, &a), "note!");
    assert!(app.document_state.is_document_dirty(&a));
}
