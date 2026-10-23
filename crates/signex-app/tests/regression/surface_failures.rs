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
