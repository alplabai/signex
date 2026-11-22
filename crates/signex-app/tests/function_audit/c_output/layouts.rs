//! Row m166: Workspace Layouts. Every task is dropped: applying and saving
//! only queue writes of the dock and of `prefs.json`, which a test must not
//! reach.

use signex_app::app::{Message, Signex};
use signex_app::dock::{DockLayoutSnapshot, PanelPosition};
use signex_app::panels::PanelKind;
use signex_app::workspace_layouts::{
    BuiltinLayout, LayoutId, ManageLayoutsMsg, SaveLayoutMsg, WorkspaceLayoutMsg,
};

use super::support::fresh_app;

fn app() -> Signex {
    let mut app = fresh_app();
    // A fresh list: the test user's own prefs never leak in.
    app.ui_state.workspace_layouts.layouts = Default::default();
    app
}

fn send(app: &mut Signex, msg: impl Into<Message>) {
    let _ = app.update(msg.into());
}

fn names(app: &Signex) -> Vec<String> {
    app.ui_state
        .workspace_layouts
        .layouts
        .user()
        .iter()
        .map(|layout| layout.name.clone())
        .collect()
}

fn save_as(app: &mut Signex, name: &str) {
    send(app, SaveLayoutMsg::Requested);
    send(app, SaveLayoutMsg::NameEdited(name.to_string()));
    send(app, SaveLayoutMsg::Confirmed);
}

fn user_id(app: &Signex, index: usize) -> LayoutId {
    app.ui_state.workspace_layouts.layouts.user()[index]
        .id
        .clone()
}

fn apply(app: &mut Signex, builtin: BuiltinLayout) {
    send(
        app,
        WorkspaceLayoutMsg::ApplyRequested(LayoutId::Builtin(builtin)),
    );
}

fn dock(app: &Signex) -> DockLayoutSnapshot {
    DockLayoutSnapshot::capture(&app.document_state.dock)
}

#[test]
fn each_built_in_arranges_the_panels_the_decision_lists() {
    let mut app = app();
    apply(&mut app, BuiltinLayout::Schematic);
    let d = &app.document_state.dock;
    assert_eq!(
        d.panel_kinds(PanelPosition::Left),
        [PanelKind::Projects, PanelKind::Components]
    );
    assert_eq!(d.panel_kinds(PanelPosition::Right), [PanelKind::Properties]);
    assert_eq!(d.panel_kinds(PanelPosition::Bottom), [PanelKind::Messages]);

    apply(&mut app, BuiltinLayout::Layout);
    let d = &app.document_state.dock;
    assert_eq!(d.panel_kinds(PanelPosition::Left), [PanelKind::Projects]);
    assert_eq!(
        d.panel_kinds(PanelPosition::Right),
        [PanelKind::Properties, PanelKind::PcbNets]
    );
    assert_eq!(d.panel_kinds(PanelPosition::Bottom), [PanelKind::Layers]);

    apply(&mut app, BuiltinLayout::Review);
    let d = &app.document_state.dock;
    assert_eq!(d.panel_kinds(PanelPosition::Right), [PanelKind::History]);
    assert_eq!(
        d.panel_kinds(PanelPosition::Bottom),
        [PanelKind::Messages, PanelKind::CommandLog]
    );

    apply(&mut app, BuiltinLayout::Notebook);
    let d = &app.document_state.dock;
    assert_eq!(d.panel_kinds(PanelPosition::Left), [PanelKind::Projects]);
    assert_eq!(
        d.panel_kinds(PanelPosition::Right),
        [PanelKind::Annotations, PanelKind::Properties]
    );
}

#[test]
fn applying_a_layout_marks_it_active_and_leaves_the_open_documents() {
    let mut app = app();
    let tabs = app.document_state.tabs.len();
    let active = app.document_state.active_tab;
    apply(&mut app, BuiltinLayout::Review);
    assert_eq!(
        app.ui_state.workspace_layouts.layouts.active(),
        Some(&LayoutId::Builtin(BuiltinLayout::Review))
    );
    assert_eq!(app.document_state.tabs.len(), tabs);
    assert_eq!(app.document_state.active_tab, active);
}

#[test]
fn save_layout_offers_a_free_name_and_stores_the_arrangement() {
    let mut app = app();
    apply(&mut app, BuiltinLayout::Review);
    send(&mut app, SaveLayoutMsg::Requested);
    let dialog = app.ui_state.workspace_layouts.save.clone().expect("open");
    assert_eq!(dialog.name, "Layout 1");
    send(&mut app, SaveLayoutMsg::NameEdited("Bench".into()));
    send(&mut app, SaveLayoutMsg::Confirmed);
    assert!(app.ui_state.workspace_layouts.save.is_none(), "Save closes");
    assert_eq!(names(&app), ["Bench"]);
    let saved = app.ui_state.workspace_layouts.layouts.user()[0].clone();
    assert_eq!(saved.dock, Some(dock(&app)));
    assert_eq!(
        app.ui_state.workspace_layouts.layouts.active(),
        Some(&saved.id)
    );
}

#[test]
fn a_saved_layout_brings_its_arrangement_back() {
    let mut app = app();
    apply(&mut app, BuiltinLayout::Review);
    let review = dock(&app);
    save_as(&mut app, "Bench");
    let id = user_id(&app, 0);
    apply(&mut app, BuiltinLayout::Schematic);
    assert_ne!(dock(&app), review);
    send(&mut app, WorkspaceLayoutMsg::ApplyRequested(id));
    assert_eq!(
        app.document_state.dock.panel_kinds(PanelPosition::Bottom),
        [PanelKind::Messages, PanelKind::CommandLog]
    );
}

#[test]
fn saving_under_one_of_your_names_replaces_that_layout() {
    let mut app = app();
    save_as(&mut app, "Bench");
    save_as(&mut app, "bench");
    assert_eq!(names(&app), ["bench"]);
}

#[test]
fn cancel_in_save_layout_stores_nothing() {
    let mut app = app();
    send(&mut app, SaveLayoutMsg::Requested);
    send(&mut app, SaveLayoutMsg::NameEdited("Bench".into()));
    send(&mut app, SaveLayoutMsg::Cancelled);
    assert!(app.ui_state.workspace_layouts.save.is_none());
    assert!(names(&app).is_empty());
}

#[test]
fn a_built_ins_name_is_refused_and_the_dialog_keeps_the_text() {
    let mut app = app();
    save_as(&mut app, "Notebook");
    let dialog = app
        .ui_state
        .workspace_layouts
        .save
        .clone()
        .expect("a refused name keeps the dialog open");
    assert_eq!(dialog.name, "Notebook");
    assert!(names(&app).is_empty());
}

#[test]
fn an_empty_name_is_refused_and_the_dialog_stays() {
    let mut app = app();
    save_as(&mut app, "   ");
    let dialog = app.ui_state.workspace_layouts.save.clone().expect("open");
    assert_eq!(dialog.name, "   ");
    assert!(names(&app).is_empty());
}

#[test]
fn a_layout_storing_neither_panels_nor_a_split_is_not_saved() {
    let mut app = app();
    send(&mut app, SaveLayoutMsg::Requested);
    send(&mut app, SaveLayoutMsg::PanelsToggled);
    send(&mut app, SaveLayoutMsg::Confirmed);
    assert!(app.ui_state.workspace_layouts.save.is_some());
    assert!(names(&app).is_empty());
}

#[test]
fn manage_renames_a_layout_and_a_taken_name_keeps_the_field_open() {
    let mut app = app();
    save_as(&mut app, "A");
    save_as(&mut app, "B");
    let a = user_id(&app, 0);
    send(&mut app, ManageLayoutsMsg::Requested);
    send(&mut app, ManageLayoutsMsg::RowSelected(a));
    send(&mut app, ManageLayoutsMsg::RenameStarted);
    send(&mut app, ManageLayoutsMsg::RenameEdited("b".into()));
    send(&mut app, ManageLayoutsMsg::RenameCommitted);
    let manage = app
        .ui_state
        .workspace_layouts
        .manage
        .as_ref()
        .expect("open");
    assert_eq!(
        manage.rename.as_deref(),
        Some("b"),
        "the typed text is kept"
    );
    assert_eq!(names(&app), ["A", "B"]);

    send(&mut app, ManageLayoutsMsg::RenameEdited("Bench".into()));
    send(&mut app, ManageLayoutsMsg::RenameCommitted);
    assert_eq!(names(&app), ["Bench", "B"]);
}

#[test]
fn cancelling_a_rename_keeps_the_name() {
    let mut app = app();
    save_as(&mut app, "A");
    send(&mut app, ManageLayoutsMsg::Requested);
    send(&mut app, ManageLayoutsMsg::RenameStarted);
    send(&mut app, ManageLayoutsMsg::RenameEdited("Other".into()));
    send(&mut app, ManageLayoutsMsg::RenameCancelled);
    assert_eq!(names(&app), ["A"]);
}

#[test]
fn manage_duplicates_and_reorders_your_layouts() {
    let mut app = app();
    save_as(&mut app, "A");
    save_as(&mut app, "B");
    send(&mut app, ManageLayoutsMsg::Requested);
    send(&mut app, ManageLayoutsMsg::GripPressed(0));
    send(&mut app, ManageLayoutsMsg::RowEntered(1));
    send(&mut app, ManageLayoutsMsg::GripReleased);
    assert_eq!(names(&app), ["B", "A"]);

    let b = user_id(&app, 0);
    send(&mut app, ManageLayoutsMsg::RowSelected(b));
    send(&mut app, ManageLayoutsMsg::DuplicateRequested);
    assert_eq!(names(&app), ["B", "B copy", "A"]);
}

#[test]
fn delete_is_at_once_and_its_toast_undoes_it() {
    let mut app = app();
    save_as(&mut app, "A");
    save_as(&mut app, "B");
    let a = user_id(&app, 0);
    send(&mut app, ManageLayoutsMsg::Requested);
    send(&mut app, ManageLayoutsMsg::RowSelected(a.clone()));
    send(&mut app, ManageLayoutsMsg::DeleteRequested);
    assert_eq!(names(&app), ["B"]);
    let manage = app
        .ui_state
        .workspace_layouts
        .manage
        .as_ref()
        .expect("open");
    assert!(manage.toast.is_some(), "the Undo toast shows");

    send(&mut app, ManageLayoutsMsg::UndoDeleteRequested);
    assert_eq!(names(&app), ["A", "B"], "back in its place");
    let manage = app
        .ui_state
        .workspace_layouts
        .manage
        .as_ref()
        .expect("open");
    assert!(manage.toast.is_none());
    assert_eq!(manage.selected, a);
}

#[test]
fn a_built_in_cannot_be_renamed_or_deleted() {
    let mut app = app();
    send(&mut app, ManageLayoutsMsg::Requested);
    send(
        &mut app,
        ManageLayoutsMsg::RowSelected(LayoutId::Builtin(BuiltinLayout::Review)),
    );
    send(&mut app, ManageLayoutsMsg::RenameStarted);
    send(&mut app, ManageLayoutsMsg::DeleteRequested);
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
fn apply_in_manage_arranges_the_selected_layout() {
    let mut app = app();
    send(&mut app, ManageLayoutsMsg::Requested);
    send(
        &mut app,
        ManageLayoutsMsg::RowSelected(LayoutId::Builtin(BuiltinLayout::Layout)),
    );
    send(&mut app, ManageLayoutsMsg::ApplyRequested);
    assert_eq!(
        app.document_state.dock.panel_kinds(PanelPosition::Bottom),
        [PanelKind::Layers]
    );
}

#[test]
fn update_from_current_edits_a_built_in_and_reset_restores_it() {
    let mut app = app();
    let schematic = LayoutId::Builtin(BuiltinLayout::Schematic);
    apply(&mut app, BuiltinLayout::Schematic);
    app.document_state.dock.close_panel(PanelKind::Messages);
    send(&mut app, ManageLayoutsMsg::Requested);
    send(&mut app, ManageLayoutsMsg::RowSelected(schematic));
    send(&mut app, ManageLayoutsMsg::UpdateFromCurrentRequested);
    let layouts = &app.ui_state.workspace_layouts.layouts;
    assert!(layouts.is_edited(BuiltinLayout::Schematic));
    assert_eq!(layouts.resettable(), Some(BuiltinLayout::Schematic));

    send(&mut app, WorkspaceLayoutMsg::ResetActiveRequested);
    assert!(
        !app.ui_state
            .workspace_layouts
            .layouts
            .is_edited(BuiltinLayout::Schematic)
    );
    assert_eq!(
        app.document_state.dock.panel_kinds(PanelPosition::Bottom),
        [PanelKind::Messages]
    );
}

#[test]
fn close_in_manage_ends_the_dialog_and_keeps_the_list() {
    let mut app = app();
    save_as(&mut app, "A");
    send(&mut app, ManageLayoutsMsg::Requested);
    send(&mut app, ManageLayoutsMsg::Closed);
    assert!(app.ui_state.workspace_layouts.manage.is_none());
    assert_eq!(names(&app), ["A"]);
}
