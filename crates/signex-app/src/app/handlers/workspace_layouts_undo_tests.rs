//! Manage Layouts' Undo next to Apply (mockup row 166, #2930): each change
//! made in the dialog is a step, taken back newest first; Apply's check mark
//! stays with the arrangement on screen. The store is a recording double, so
//! no `prefs.json` is touched.

use std::sync::{Arc, Mutex};

use crate::app::{Message, Signex};
use crate::workspace_layouts::{
    BuiltinLayout, LayoutId, ManageLayoutsMsg, SaveLayoutMsg, WorkspaceLayoutMsg,
    WorkspaceLayoutsStore,
};

#[derive(Default)]
struct Recorder {
    writes: Mutex<Vec<serde_json::Value>>,
}

impl WorkspaceLayoutsStore for Recorder {
    fn write(&self, value: &serde_json::Value) -> Result<(), String> {
        self.writes.lock().unwrap().push(value.clone());
        Ok(())
    }
}

fn app() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.workspace_layouts.layouts = Default::default();
    app.ui_state.workspace_layouts.kept = Default::default();
    let store: Arc<dyn WorkspaceLayoutsStore> = Arc::new(Recorder::default());
    app.ui_state.workspace_layouts.store = store;
    app
}

fn send(app: &mut Signex, msg: impl Into<Message>) {
    let _ = app.update(msg.into());
}

fn save_as(app: &mut Signex, name: &str) {
    send(app, SaveLayoutMsg::Requested);
    send(app, SaveLayoutMsg::NameEdited(name.to_string()));
    send(app, SaveLayoutMsg::Confirmed);
}

fn user_names(app: &Signex) -> Vec<String> {
    let layouts = &app.ui_state.workspace_layouts.layouts;
    layouts.user().iter().map(|l| l.name.clone()).collect()
}

fn can_undo(app: &Signex) -> bool {
    let manage = app.ui_state.workspace_layouts.manage.as_ref();
    manage.expect("Manage Layouts is open").can_undo()
}

/// A fresh dialog has nothing to undo; a rename and a duplicate come back
/// one Undo at a time, newest first, and Undo is greyed once both are back.
#[test]
fn undo_takes_back_the_dialogs_changes_newest_first() {
    let mut app = app();
    save_as(&mut app, "Bench");
    send(&mut app, ManageLayoutsMsg::Requested);
    assert!(!can_undo(&app), "nothing changed yet");

    send(&mut app, ManageLayoutsMsg::RenameStarted);
    send(
        &mut app,
        ManageLayoutsMsg::RenameEdited("Review desk".into()),
    );
    send(&mut app, ManageLayoutsMsg::RenameCommitted);
    send(&mut app, ManageLayoutsMsg::DuplicateRequested);
    assert_eq!(user_names(&app), ["Review desk", "Review desk copy"]);
    assert!(can_undo(&app));

    send(&mut app, ManageLayoutsMsg::UndoRequested);
    assert_eq!(user_names(&app), ["Review desk"]);
    let selected = &app
        .ui_state
        .workspace_layouts
        .manage
        .as_ref()
        .unwrap()
        .selected;
    assert_eq!(
        selected,
        &app.ui_state.workspace_layouts.layouts.user()[0].id,
        "the copy is gone, so its source is selected"
    );
    send(&mut app, ManageLayoutsMsg::UndoRequested);
    assert_eq!(user_names(&app), ["Bench"]);
    assert!(!can_undo(&app));

    // A third Undo has nothing left and changes nothing.
    send(&mut app, ManageLayoutsMsg::UndoRequested);
    assert_eq!(user_names(&app), ["Bench"]);
}

/// Undo of a delete brings the layout back and closes the toast, whose own
/// Undo would otherwise put it back twice. A refused rename is no step.
#[test]
fn undo_of_a_delete_closes_the_toast_and_a_refused_rename_is_no_step() {
    let mut app = app();
    save_as(&mut app, "A");
    save_as(&mut app, "B");
    send(&mut app, ManageLayoutsMsg::Requested);
    send(&mut app, ManageLayoutsMsg::RenameStarted);
    send(&mut app, ManageLayoutsMsg::RenameEdited("a".into()));
    send(&mut app, ManageLayoutsMsg::RenameCommitted);
    assert!(!can_undo(&app), "a name in use changes nothing");

    send(&mut app, ManageLayoutsMsg::DeleteRequested);
    assert_eq!(user_names(&app), ["A"]);
    send(&mut app, ManageLayoutsMsg::UndoRequested);
    assert_eq!(user_names(&app), ["A", "B"]);
    let manage = app.ui_state.workspace_layouts.manage.as_ref().unwrap();
    assert!(manage.toast.is_none(), "the toast went with the delete");
    assert_eq!(
        app.ui_state.workspace_layouts.layouts.active(),
        Some(&app.ui_state.workspace_layouts.layouts.user()[1].id),
        "B was on screen before the delete and is again"
    );
}

/// Apply is not a step: after a delete and an Apply, Undo brings the layout
/// back and the applied built-in keeps the check mark.
#[test]
fn undo_keeps_the_applied_layout_checked() {
    let mut app = app();
    save_as(&mut app, "A");
    send(&mut app, ManageLayoutsMsg::Requested);
    send(&mut app, ManageLayoutsMsg::DeleteRequested);
    let review = LayoutId::Builtin(BuiltinLayout::Review);
    send(&mut app, ManageLayoutsMsg::RowSelected(review.clone()));
    send(&mut app, ManageLayoutsMsg::ApplyRequested);
    send(&mut app, ManageLayoutsMsg::UndoRequested);
    assert_eq!(user_names(&app), ["A"]);
    assert_eq!(
        app.ui_state.workspace_layouts.layouts.active(),
        Some(&review)
    );
    assert!(!can_undo(&app));
}

/// Closing the dialog drops its steps: a reopened dialog has nothing to undo.
#[test]
fn closing_manage_layouts_drops_its_undo_steps() {
    let mut app = app();
    save_as(&mut app, "A");
    send(&mut app, ManageLayoutsMsg::Requested);
    send(&mut app, ManageLayoutsMsg::DuplicateRequested);
    send(
        &mut app,
        Message::from(WorkspaceLayoutMsg::Manage(ManageLayoutsMsg::Closed)),
    );
    send(&mut app, ManageLayoutsMsg::Requested);
    assert!(!can_undo(&app));
    assert_eq!(user_names(&app), ["A", "A copy"]);
}
