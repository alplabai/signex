//! #110 — failures reach the user, and destructive actions do not run
//! silently.
//!
//! Covers: a failed Ctrl+S and a failed Component Preview row save raise the
//! Save Failed card; closing a dirty primitive / Component Preview tab parks
//! the draft instead of discarding it; History ▸ Restore over unsaved edits
//! asks first; a refresh keeps a Custom theme's tokens.

use signex_app::app::{
    ComponentEditorTab, ContextMenuMsg, FileMsg, Message, Signex, TabContextAction, TabInfo,
    TabKind,
};
use signex_app::dock::DockMessage;
use signex_app::library::messages::{EditorMsg, LibraryMessage};
use signex_app::panels::PanelMsg;
use signex_types::theme::{CustomThemeFile, ThemeId, canvas_colors, theme_tokens};
use tempfile::TempDir;

use crate::primitive_dirty_quit::{
    app_with_preview_edit, edit_description_via_panel, open_footprint_editor,
};

fn close_tab(app: &mut Signex, idx: usize) {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(idx),
    )));
}

#[test]
fn ctrl_s_on_a_primitive_that_cannot_be_written_shows_the_save_failed_card() {
    // The "file" is a directory: it exists, so Save does not divert to
    // Save As, and the atomic write into it fails.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("blocked.snxfpt");
    std::fs::create_dir(&path).unwrap();
    let (mut app, _t) = Signex::new();
    open_footprint_editor(&mut app, &path);
    edit_description_via_panel(&mut app);

    let _ = app.update(Message::File(FileMsg::Save));

    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("a failed save must raise the error card");
    assert_eq!(notice.title, "Save Failed");
    assert!(
        app.document_state.dirty_paths.contains(&path),
        "still dirty"
    );
}

#[test]
fn saving_a_component_row_that_cannot_be_written_shows_the_save_failed_card() {
    // The row's library is not mounted, so `update_row` cannot run.
    let mut app = app_with_preview_edit();
    let address = app.library.editors.keys().next().cloned().unwrap();

    let _ = app.update(Message::Library(LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg: EditorMsg::SaveDraft,
    }));

    let notice = app.document_state.error_notice.as_ref().expect("a card");
    assert_eq!(notice.title, "Save Failed");
    assert!(app.library.editors[&address].dirty, "the draft stays dirty");
}

#[test]
fn closing_a_dirty_footprint_tab_parks_the_draft_and_a_clean_one_drops_it() {
    let tmp = TempDir::new().unwrap();
    let dirty = tmp.path().join("dirty.snxfpt");
    let clean = tmp.path().join("clean.snxfpt");
    let (mut app, _t) = Signex::new();
    open_footprint_editor(&mut app, &clean);
    open_footprint_editor(&mut app, &dirty);
    edit_description_via_panel(&mut app);
    assert!(app.document_state.dirty_paths.contains(&dirty));

    close_tab(&mut app, 1);
    close_tab(&mut app, 0);

    assert!(app.document_state.tabs.is_empty());
    assert!(
        app.document_state.footprint_editors.contains_key(&dirty),
        "the unsaved draft must survive the tab close"
    );
    assert!(app.document_state.dirty_paths.contains(&dirty));
    assert!(
        !app.document_state.footprint_editors.contains_key(&clean),
        "a clean editor is dropped as before"
    );
}

#[test]
fn closing_a_dirty_component_preview_tab_keeps_the_draft() {
    let mut app = app_with_preview_edit();
    let address = app.library.editors.keys().next().cloned().unwrap();
    app.document_state.tabs.push(TabInfo {
        title: "row".into(),
        path: address.synthetic_tab_path(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::ComponentEditor(ComponentEditorTab {
            library_path: address.library_path.clone(),
            table: address.table.clone(),
            row_id: address.row_id,
        }),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;

    let last = app.document_state.tabs.len() - 1;
    close_tab(&mut app, last);

    assert!(
        app.library.editors[&address].dirty,
        "draft parked, not lost"
    );
}

fn restore_clicked(app: &mut Signex) {
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::HistoryRestoreClicked {
            sha: "abc123".into(),
        },
    )));
}

#[test]
fn history_restore_over_unsaved_edits_asks_before_it_acts() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("h.snxfpt");
    let (mut app, _t) = Signex::new();
    open_footprint_editor(&mut app, &path);
    edit_description_via_panel(&mut app);

    restore_clicked(&mut app);
    assert_eq!(
        app.document_state.history.pending_restore.as_deref(),
        Some("abc123"),
        "the first click only arms the confirmation"
    );
    assert!(app.document_state.error_notice.is_none(), "nothing ran yet");

    // Cancel disarms it.
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::HistoryRestoreCancelled,
    )));
    assert!(app.document_state.history.pending_restore.is_none());

    // Confirming runs the restore; the file has no owning project, so the
    // failure is shown rather than only logged.
    restore_clicked(&mut app);
    restore_clicked(&mut app);
    assert!(app.document_state.history.pending_restore.is_none());
    assert!(
        app.document_state.error_notice.is_some(),
        "a failed restore must raise the error card"
    );
}

#[test]
fn panel_refresh_keeps_the_custom_theme_tokens() {
    let (mut app, _t) = Signex::new();
    let custom = theme_tokens(ThemeId::Nord);
    assert_ne!(custom, theme_tokens(ThemeId::Signex));
    app.ui_state.custom_theme = Some(CustomThemeFile {
        name: "mine".into(),
        tokens: custom,
        canvas: canvas_colors(ThemeId::Signex),
    });
    app.ui_state.theme_id = ThemeId::Custom;

    // Any update ends in a panel-context refresh.
    let tmp = TempDir::new().unwrap();
    open_footprint_editor(&mut app, &tmp.path().join("t.snxfpt"));
    edit_description_via_panel(&mut app);

    assert_eq!(app.document_state.panel_ctx.tokens, custom);
}

// ---- parked-draft integrity ------------------------------------------------

use signex_app::app::{EditMsg, ProjectCloseChoice, ProjectMsg, SymbolEditorState, WindowMsg};
use signex_app::library::messages::CloseLibraryChoice;
use signex_library::{Symbol, SymbolFile};
use std::path::Path;

fn write_symbol(path: &Path, name: &str) {
    let text = SymbolFile::from_symbol(Symbol::empty(name))
        .to_toml_string()
        .expect("serialise symbol");
    std::fs::write(path, text).unwrap();
}

/// A symbol tab for `path` with an editor holding the on-disk content.
fn open_symbol_tab(app: &mut Signex, path: &Path) {
    let file = SymbolFile::from_symbol(Symbol::empty("disk"));
    app.document_state.symbol_editors.insert(
        path.to_path_buf(),
        SymbolEditorState::new(path.to_path_buf(), file),
    );
    app.document_state.tabs.push(TabInfo {
        title: "sym".into(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::SymbolEditor(path.to_path_buf()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

fn draft_name(app: &Signex, path: &Path) -> String {
    app.document_state.symbol_editors[path]
        .primitive()
        .name
        .clone()
}

fn reopen(app: &mut Signex, path: &Path) {
    let _ = app.update(Message::File(FileMsg::Opened(Some(path.to_path_buf()))));
}

#[test]
fn reopening_a_closed_dirty_symbol_tab_restores_the_draft_not_the_disk_copy() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("s.snxsym");
    write_symbol(&path, "disk");
    let (mut app, _t) = Signex::new();
    open_symbol_tab(&mut app, &path);
    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    editor.primitive_mut().name = "draft".into();
    editor.dirty = true;

    close_tab(&mut app, 0);
    reopen(&mut app, &path);

    assert_eq!(app.document_state.tabs.len(), 1);
    assert_eq!(draft_name(&app, &path), "draft");
}

#[test]
fn a_parked_draft_is_reachable_even_when_its_file_is_gone() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("gone.snxsym");
    write_symbol(&path, "disk");
    let (mut app, _t) = Signex::new();
    open_symbol_tab(&mut app, &path);
    app.document_state
        .symbol_editors
        .get_mut(&path)
        .unwrap()
        .dirty = true;
    close_tab(&mut app, 0);
    std::fs::remove_file(&path).unwrap();

    reopen(&mut app, &path);

    assert_eq!(app.document_state.tabs.len(), 1, "the draft reopens");
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn a_saved_parked_editor_is_replaced_by_the_changed_disk_file_on_reopen() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("stale.snxsym");
    write_symbol(&path, "disk");
    let (mut app, _t) = Signex::new();
    open_symbol_tab(&mut app, &path);
    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    editor.primitive_mut().name = "draft".into();
    editor.dirty = true;
    close_tab(&mut app, 0);
    // Save All saved it (editor stays, now clean) and another tool then
    // rewrote the file.
    app.document_state
        .symbol_editors
        .get_mut(&path)
        .unwrap()
        .dirty = false;
    app.document_state.dirty_paths.remove(&path);
    write_symbol(&path, "external");

    reopen(&mut app, &path);

    assert_eq!(draft_name(&app, &path), "external");
}

#[test]
fn closing_a_symbol_tab_dirtied_by_undo_after_a_save_parks_the_draft() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("undo.snxsym");
    write_symbol(&path, "disk");
    let (mut app, _t) = Signex::new();
    open_symbol_tab(&mut app, &path);
    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    let snapshot = editor.snapshot();
    editor.undo_snapshots.push(snapshot);
    editor.primitive_mut().name = "edited".into();
    editor.dirty = true;
    app.document_state.dirty_paths.insert(path.clone());
    let _ = app.update(Message::File(FileMsg::Save));
    assert!(!app.document_state.dirty_paths.contains(&path));

    // Undo re-dirties the editor; the panel refresh mirrors it into
    // `dirty_paths` (#760).
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(app.document_state.symbol_editors[&path].dirty);
    assert!(app.document_state.dirty_paths.contains(&path));
    close_tab(&mut app, 0);

    assert!(app.document_state.symbol_editors.contains_key(&path));
    assert!(app.document_state.dirty_paths.contains(&path));
}

#[test]
fn close_library_save_all_with_a_failing_row_keeps_the_library_and_drafts() {
    let mut app = app_with_preview_edit();
    let address = app.library.editors.keys().next().cloned().unwrap();

    let _ = app.update(Message::Library(LibraryMessage::ConfirmCloseLibrary {
        library_path: address.library_path.clone(),
        dirty_editors: vec![address.clone()],
    }));
    let _ = app.update(Message::Library(LibraryMessage::CloseLibraryConfirm(
        CloseLibraryChoice::SaveAll,
    )));

    assert!(app.library.editors[&address].dirty, "the draft survives");
    let notice = app.document_state.error_notice.as_ref().expect("a card");
    assert_eq!(notice.title, "Save Failed");
}

#[test]
fn a_parked_footprint_draft_is_written_by_quit_save_all() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("parked.snxfpt");
    let (mut app, _t) = Signex::new();
    open_footprint_editor(&mut app, &path);
    edit_description_via_panel(&mut app);
    close_tab(&mut app, 0);

    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    let _ = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::SaveAll,
    )));

    let written = std::fs::read_to_string(&path).expect("Save All writes the parked draft");
    assert!(written.contains("edited"));
}

#[test]
fn switching_tabs_disarms_a_pending_history_restore() {
    let tmp = TempDir::new().unwrap();
    let (a, b) = (tmp.path().join("a.snxfpt"), tmp.path().join("b.snxfpt"));
    let (mut app, _t) = Signex::new();
    open_footprint_editor(&mut app, &a);
    open_footprint_editor(&mut app, &b);
    edit_description_via_panel(&mut app);
    restore_clicked(&mut app);
    assert!(app.document_state.history.pending_restore.is_some());

    app.document_state.active_tab = 0;
    edit_description_via_panel(&mut app);

    assert!(app.document_state.history.pending_restore.is_none());
}

#[test]
fn close_all_and_close_all_others_park_dirty_drafts() {
    let tmp = TempDir::new().unwrap();
    let (a, b, c) = (
        tmp.path().join("a.snxfpt"),
        tmp.path().join("b.snxfpt"),
        tmp.path().join("c.snxfpt"),
    );
    let (mut app, _t) = Signex::new();
    open_footprint_editor(&mut app, &a);
    open_footprint_editor(&mut app, &b);
    open_footprint_editor(&mut app, &c);
    for p in [&a, &b, &c] {
        app.document_state
            .footprint_editors
            .get_mut(p)
            .unwrap()
            .dirty = true;
    }

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::CloseAllOthers(0),
    )));
    assert_eq!(app.document_state.tabs.len(), 1);
    assert!(app.document_state.footprint_editors.contains_key(&b));
    assert!(app.document_state.footprint_editors.contains_key(&c));

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::CloseAll,
    )));
    assert!(app.document_state.tabs.is_empty());
    for p in [&a, &b, &c] {
        assert!(app.document_state.footprint_editors.contains_key(p));
    }
}
