//! #111 — an idle cursor move only refreshes the cached pointer position
//! (plus the cheap git-drain / diagnostics safety net); an active layout
//! drag still runs the full drag handling. Handlers that queue a commit or
//! log an error must not depend on a later idle move to flush them.
//!
//! Note: the guard tests below (`only_records_the_position`, `active_panel_drag_still_resizes`,
//! the drain / diagnostics tests) pin behaviour that must keep holding; the
//! mirror and history-dirty tests are the ones that reproduce the review bug.

use signex_app::app::{DragTarget, FootprintEditorState, LoadedProject, Message, Signex, UiMsg};
use signex_app::diagnostics;
use signex_app::library::messages::{LibraryMessage, PrimitiveEdit};
use signex_app::menu_bar::MenuMessage;
use signex_library::{Footprint, FootprintFile};
use signex_types::project::ProjectData;
use std::path::Path;
use tempfile::TempDir;

fn idle_move(app: &mut Signex) {
    let _ = app.update(Message::Ui(UiMsg::DragMove(12.0, 34.0)));
}

fn has_diagnostic(app: &Signex, needle: &str) -> bool {
    app.document_state
        .panel_ctx
        .diagnostics
        .iter()
        .any(|e| e.message.contains(needle))
}

#[test]
fn idle_cursor_move_only_records_the_position() {
    let (mut app, _t) = Signex::new();
    assert!(!app.is_layout_drag_active());
    idle_move(&mut app);
    assert_eq!(app.interaction_state.last_mouse_pos, (12.0, 34.0));
}

#[test]
fn idle_cursor_move_repairs_panel_ctx_mirrors_left_stale_by_other_dispatchers() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Menu(MenuMessage::ToggleGrid));
    idle_move(&mut app);
    assert_eq!(
        app.document_state.panel_ctx.grid_visible,
        app.ui_state.grid_visible
    );
}

#[test]
fn idle_cursor_move_refreshes_the_history_dirty_bit() {
    let (mut app, _t) = Signex::new();
    let p = std::path::PathBuf::from("history-dirty.snxsch");
    app.document_state.history.active_path = Some(p.clone());
    app.document_state.dirty_paths.insert(p);
    idle_move(&mut app);
    assert!(app.document_state.panel_ctx.history.dirty);
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

#[test]
fn idle_cursor_move_drains_queued_git_commits() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _t) = Signex::new();
    push_git_project(&mut app, tmp.path());
    app.commit_save_to_project_git(&tmp.path().join("a.snxfpt"), "m");
    assert!(!app.document_state.pending_git_commits.is_empty());

    idle_move(&mut app);

    assert!(app.document_state.pending_git_commits.is_empty());
}

#[test]
fn idle_cursor_move_republishes_diagnostics() {
    let _ = diagnostics::init_logging();
    let (mut app, _t) = Signex::new();
    let marker = "idle_cursor_move marker 91d3e0";
    diagnostics::log_error("idle_cursor_move", &anyhow::anyhow!("{marker}"));

    idle_move(&mut app);

    assert!(has_diagnostic(&app, marker));
}

fn open_footprint_editor(app: &mut Signex, path: &Path) {
    let file = FootprintFile::from_footprint(Footprint::empty("fp"));
    app.document_state.footprint_editors.insert(
        path.to_path_buf(),
        FootprintEditorState::new(path.to_path_buf(), file),
    );
}

fn push_git_project(app: &mut Signex, dir: &Path) {
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("p.snxprj"),
        data: ProjectData {
            name: "p".into(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: true,
            unknown: Default::default(),
        },
        pending_libraries: std::collections::HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
    });
}

/// Save, run the off-thread write, and hand its completion to `update` -
/// the completion message is what queues the commit and logs the error.
fn save_primitive(app: &mut Signex, path: &Path) {
    let task = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Save,
    }));
    for message in crate::library_symbol_drift::run_task(task) {
        let _ = app.update(message);
    }
}

#[test]
fn primitive_save_queues_and_drains_its_commit_without_a_later_message() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("a.snxfpt");
    std::fs::write(&path, b"").unwrap();
    let (mut app, _t) = Signex::new();
    push_git_project(&mut app, tmp.path());
    open_footprint_editor(&mut app, &path);

    save_primitive(&mut app, &path);

    // Queued (the in-flight pill shows) and already handed to the task
    // pool by the save message itself, not by a following idle move.
    assert!(!app.document_state.inflight_git_commits.is_empty());
    assert!(app.document_state.pending_git_commits.is_empty());
    idle_move(&mut app);
    assert!(app.document_state.pending_git_commits.is_empty());
}

#[test]
fn primitive_save_failure_reaches_the_messages_panel_immediately() {
    let _ = diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    // A directory exists() but cannot be written as a file: forced failure.
    let path = tmp.path().join("unique_save_fail_7c2b.snxfpt");
    std::fs::create_dir(&path).unwrap();
    let (mut app, _t) = Signex::new();
    open_footprint_editor(&mut app, &path);

    save_primitive(&mut app, &path);

    assert!(has_diagnostic(&app, "unique_save_fail_7c2b"));
    idle_move(&mut app);
    assert!(has_diagnostic(&app, "unique_save_fail_7c2b"));
}
