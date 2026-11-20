//! Mockup row 166 — workspace layouts. Applying one rebuilds the dock and the dock's
//! own persistence writes it once; Save Layout names the arrangement on
//! screen (and replaces one of yours of the same name, never a built-in);
//! Manage Layouts deletes with Undo, reorders, renames and duplicates. Both
//! stores are recording doubles, so no `prefs.json` is touched.

use std::sync::{Arc, Mutex};

use futures::StreamExt;
use iced::Task;
use signex_app::app::{Message, Signex};
use signex_app::dock::{
    DockLayoutMsg, DockLayoutSnapshot, DockLayoutStore, DockLayoutSync, PanelPosition,
};
use signex_app::panels::PanelKind;
use signex_app::workspace_layouts::{
    BuiltinLayout, LayoutId, ManageLayoutsMsg, SaveLayoutMsg, WorkspaceLayoutMsg,
    WorkspaceLayoutsStore,
};

#[derive(Default)]
struct DockRecorder {
    writes: Mutex<Vec<DockLayoutSnapshot>>,
}

impl DockLayoutStore for DockRecorder {
    fn write(&self, layout: &DockLayoutSnapshot) -> Result<(), String> {
        self.writes.lock().unwrap().push(layout.clone());
        Ok(())
    }
}

#[derive(Default)]
struct LayoutsRecorder {
    writes: Mutex<Vec<serde_json::Value>>,
}

impl WorkspaceLayoutsStore for LayoutsRecorder {
    fn write(&self, value: &serde_json::Value) -> Result<(), String> {
        self.writes.lock().unwrap().push(value.clone());
        Ok(())
    }
}

struct Fixture {
    app: Signex,
    dock_writes: Arc<DockRecorder>,
    layout_writes: Arc<LayoutsRecorder>,
}

fn fixture() -> Fixture {
    let (mut app, _boot) = Signex::new();
    let dock_writes = Arc::new(DockRecorder::default());
    app.document_state.dock_layout_sync = DockLayoutSync::new(
        dock_writes.clone(),
        DockLayoutSnapshot::capture(&app.document_state.dock),
    );
    let layout_writes = Arc::new(LayoutsRecorder::default());
    // A fresh list: the test user's own prefs never leak in.
    app.ui_state.workspace_layouts.layouts = Default::default();
    let store: Arc<dyn WorkspaceLayoutsStore> = layout_writes.clone();
    app.ui_state.workspace_layouts.store = store;
    Fixture {
        app,
        dock_writes,
        layout_writes,
    }
}

/// Run a task to completion the way the iced runtime would, returning the
/// messages it produced.
fn run(task: Task<Message>) -> Vec<Message> {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

fn send(app: &mut Signex, msg: impl Into<Message>) -> Task<Message> {
    app.update(msg.into())
}

fn user_names(app: &Signex) -> Vec<String> {
    app.ui_state
        .workspace_layouts
        .layouts
        .user()
        .iter()
        .map(|l| l.name.clone())
        .collect()
}

fn save_as(app: &mut Signex, name: &str) {
    let _ = send(app, SaveLayoutMsg::Requested);
    let _ = send(app, SaveLayoutMsg::NameEdited(name.to_string()));
    let _ = send(app, SaveLayoutMsg::Confirmed);
}

#[test]
fn applying_a_built_in_rebuilds_the_dock_and_writes_it_once() {
    let Fixture {
        mut app,
        dock_writes,
        layout_writes,
    } = fixture();

    let task = send(
        &mut app,
        WorkspaceLayoutMsg::ApplyRequested(LayoutId::Builtin(BuiltinLayout::Schematic)),
    );

    let dock = &app.document_state.dock;
    assert_eq!(
        dock.panel_kinds(PanelPosition::Left),
        [PanelKind::Projects, PanelKind::Components]
    );
    assert_eq!(
        dock.panel_kinds(PanelPosition::Right),
        [PanelKind::Properties]
    );
    assert_eq!(
        dock.panel_kinds(PanelPosition::Bottom),
        [PanelKind::Messages]
    );
    assert_eq!(
        app.ui_state.workspace_layouts.layouts.active(),
        Some(&LayoutId::Builtin(BuiltinLayout::Schematic))
    );
    assert_eq!(
        dock_writes.writes.lock().unwrap().len(),
        0,
        "update itself writes nothing"
    );

    // The dock's debounce timer fires once; its message writes the dock.
    let produced = run(task);
    let mut writes = Vec::new();
    for message in produced {
        if let Message::DockLayout(DockLayoutMsg::SaveDebounceElapsed(_)) = message {
            writes.push(app.update(message));
        }
    }
    for task in writes {
        run(task);
    }
    let written = dock_writes.writes.lock().unwrap().clone();
    assert_eq!(written.len(), 1, "one dock write for the apply");
    assert_eq!(
        written[0],
        DockLayoutSnapshot::capture(&app.document_state.dock)
    );
    // The list was written too, with the active layout.
    let lists = layout_writes.writes.lock().unwrap().clone();
    assert_eq!(
        lists.last().map(|v| v["active"].clone()),
        Some(serde_json::json!("schematic"))
    );
}

#[test]
fn applying_keeps_the_open_documents() {
    let Fixture { mut app, .. } = fixture();
    let tabs = app.document_state.tabs.len();
    let active = app.document_state.active_tab;
    let _ = send(
        &mut app,
        WorkspaceLayoutMsg::ApplyRequested(LayoutId::Builtin(BuiltinLayout::Layout)),
    );
    assert_eq!(app.document_state.tabs.len(), tabs);
    assert_eq!(app.document_state.active_tab, active);
    assert_eq!(
        app.document_state.dock.panel_kinds(PanelPosition::Bottom),
        [PanelKind::Layers]
    );
}

#[test]
fn save_layout_names_the_arrangement_and_replaces_only_your_own() {
    let Fixture { mut app, .. } = fixture();

    let _ = send(&mut app, SaveLayoutMsg::Requested);
    let dialog = app.ui_state.workspace_layouts.save.clone().expect("open");
    assert_eq!(dialog.name, "Layout 1");
    assert!(dialog.store_panels && dialog.store_split);
    let _ = send(&mut app, SaveLayoutMsg::NameEdited("Bench".into()));
    let _ = send(&mut app, SaveLayoutMsg::Confirmed);
    assert!(app.ui_state.workspace_layouts.save.is_none(), "Save closes");
    assert_eq!(user_names(&app), ["Bench"]);
    let bench = app.ui_state.workspace_layouts.layouts.user()[0].clone();
    assert_eq!(
        bench.dock,
        Some(DockLayoutSnapshot::capture(&app.document_state.dock))
    );
    assert!(bench.from_current);
    assert_eq!(
        app.ui_state.workspace_layouts.layouts.active(),
        Some(&bench.id),
        "the arrangement on screen is the saved layout"
    );

    // The same name in another case replaces it.
    save_as(&mut app, "bench");
    assert_eq!(user_names(&app), ["bench"]);

    // A built-in's name is refused and the dialog stays open.
    save_as(&mut app, "Notebook");
    assert!(app.ui_state.workspace_layouts.save.is_some());
    assert_eq!(user_names(&app), ["bench"]);
    let _ = send(&mut app, SaveLayoutMsg::Cancelled);
    assert!(app.ui_state.workspace_layouts.save.is_none());
}

#[test]
fn a_layout_saved_without_panels_or_a_split_is_not_saved() {
    let Fixture { mut app, .. } = fixture();
    let _ = send(&mut app, SaveLayoutMsg::Requested);
    let _ = send(&mut app, SaveLayoutMsg::PanelsToggled);
    // No document area split exists to store.
    let _ = send(&mut app, SaveLayoutMsg::Confirmed);
    assert!(app.ui_state.workspace_layouts.save.is_some());
    assert!(user_names(&app).is_empty());
}

#[test]
fn manage_deletes_with_undo_and_reorders_renames_and_duplicates() {
    let Fixture { mut app, .. } = fixture();
    save_as(&mut app, "A");
    save_as(&mut app, "B");
    let ids: Vec<LayoutId> = app
        .ui_state
        .workspace_layouts
        .layouts
        .user()
        .iter()
        .map(|l| l.id.clone())
        .collect();

    let _ = send(&mut app, ManageLayoutsMsg::Requested);
    let dialog = app.ui_state.workspace_layouts.manage.clone().expect("open");
    assert_eq!(dialog.selected, ids[1], "the active layout is selected");

    // Delete is at once, with Undo.
    let _ = send(&mut app, ManageLayoutsMsg::RowSelected(ids[0].clone()));
    let _ = send(&mut app, ManageLayoutsMsg::DeleteRequested);
    assert_eq!(user_names(&app), ["B"]);
    let toast = app
        .ui_state
        .workspace_layouts
        .manage
        .as_ref()
        .and_then(|d| d.toast.clone())
        .expect("the toast shows");
    assert_eq!(toast.deleted.layout.name, "A");
    let _ = send(&mut app, ManageLayoutsMsg::UndoDeleteRequested);
    assert_eq!(user_names(&app), ["A", "B"]);
    let manage = app
        .ui_state
        .workspace_layouts
        .manage
        .as_ref()
        .expect("open");
    assert!(manage.toast.is_none());
    assert_eq!(manage.selected, ids[0]);

    // An old toast's timer does not close a newer one.
    let _ = send(&mut app, ManageLayoutsMsg::DeleteRequested);
    let generation = app
        .ui_state
        .workspace_layouts
        .manage
        .as_ref()
        .and_then(|d| d.toast.as_ref())
        .map(|t| t.generation)
        .expect("a toast");
    let _ = send(&mut app, ManageLayoutsMsg::ToastExpired(generation - 1));
    assert!(
        app.ui_state
            .workspace_layouts
            .manage
            .as_ref()
            .is_some_and(|d| d.toast.is_some())
    );
    let _ = send(&mut app, ManageLayoutsMsg::ToastExpired(generation));
    assert!(
        app.ui_state
            .workspace_layouts
            .manage
            .as_ref()
            .is_some_and(|d| d.toast.is_none())
    );
    assert_eq!(user_names(&app), ["B"]);
    save_as(&mut app, "A");
    assert_eq!(user_names(&app), ["B", "A"]);

    // Drag B by its grip below A.
    let _ = send(&mut app, ManageLayoutsMsg::GripPressed(0));
    let _ = send(&mut app, ManageLayoutsMsg::RowEntered(1));
    let _ = send(&mut app, ManageLayoutsMsg::GripReleased);
    assert_eq!(user_names(&app), ["A", "B"]);

    // Rename inline; a taken name keeps the field open.
    let a = app.ui_state.workspace_layouts.layouts.user()[0].id.clone();
    let _ = send(&mut app, ManageLayoutsMsg::RowSelected(a.clone()));
    let _ = send(&mut app, ManageLayoutsMsg::RenameStarted);
    let _ = send(&mut app, ManageLayoutsMsg::RenameEdited("b".into()));
    let _ = send(&mut app, ManageLayoutsMsg::RenameCommitted);
    assert!(
        app.ui_state
            .workspace_layouts
            .manage
            .as_ref()
            .is_some_and(|d| d.rename.is_some())
    );
    let _ = send(&mut app, ManageLayoutsMsg::RenameEdited("Bench".into()));
    let _ = send(&mut app, ManageLayoutsMsg::RenameCommitted);
    assert_eq!(user_names(&app), ["Bench", "B"]);

    let _ = send(&mut app, ManageLayoutsMsg::DuplicateRequested);
    assert_eq!(user_names(&app), ["Bench", "Bench copy", "B"]);

    let _ = send(&mut app, ManageLayoutsMsg::Closed);
    assert!(app.ui_state.workspace_layouts.manage.is_none());
}

#[test]
fn a_built_in_cannot_be_renamed_or_deleted_from_manage() {
    let Fixture { mut app, .. } = fixture();
    let _ = send(&mut app, ManageLayoutsMsg::Requested);
    let review = LayoutId::Builtin(BuiltinLayout::Review);
    let _ = send(&mut app, ManageLayoutsMsg::RowSelected(review));
    let _ = send(&mut app, ManageLayoutsMsg::RenameStarted);
    let _ = send(&mut app, ManageLayoutsMsg::DeleteRequested);
    let manage = app
        .ui_state
        .workspace_layouts
        .manage
        .as_ref()
        .expect("open");
    assert!(manage.rename.is_none());
    assert!(manage.toast.is_none());
}

#[test]
fn update_from_current_edits_a_built_in_and_reset_brings_it_back() {
    let Fixture { mut app, .. } = fixture();
    let schematic = LayoutId::Builtin(BuiltinLayout::Schematic);
    let _ = send(
        &mut app,
        WorkspaceLayoutMsg::ApplyRequested(schematic.clone()),
    );
    // Close Messages, then store that as the Schematic layout.
    app.document_state.dock.close_panel(PanelKind::Messages);
    let _ = send(&mut app, ManageLayoutsMsg::Requested);
    let _ = send(&mut app, ManageLayoutsMsg::RowSelected(schematic.clone()));
    let _ = send(&mut app, ManageLayoutsMsg::UpdateFromCurrentRequested);
    let layouts = &app.ui_state.workspace_layouts.layouts;
    assert!(layouts.is_edited(BuiltinLayout::Schematic));
    assert_eq!(layouts.resettable(), Some(BuiltinLayout::Schematic));

    let _ = send(&mut app, WorkspaceLayoutMsg::ResetActiveRequested);
    assert!(
        !app.ui_state
            .workspace_layouts
            .layouts
            .is_edited(BuiltinLayout::Schematic)
    );
    assert_eq!(
        app.document_state.dock.panel_kinds(PanelPosition::Bottom),
        [PanelKind::Messages],
        "Reset applies the built-in as shipped"
    );
}
