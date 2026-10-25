//! #859 — a cancelled History restore keeps its claim until its worker
//! reports, so saves of the file stay refused while the worker can still
//! write it. These tests cancel while the worker is still pending: the task
//! is held unrun until after the "meanwhile" actions.

use std::path::Path;

use super::restore_commit_tests::{
    collect_messages, edit_description, editor_name, finish_restore, footprint_fixture, logged,
    marker_editor,
};
use crate::app::{Message, Signex, TabInfo, TabKind};

const REFUSED: &str = "A History restore of part.snxfpt is still running, so the save was refused";

fn reopen(app: &mut Signex, path: &Path) {
    marker_editor(app, path, "reopened");
    app.document_state.tabs.push(TabInfo {
        title: "part".into(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::FootprintEditor(path.to_path_buf()),
    });
    app.document_state.active_tab = 0;
    app.document_state.dirty_paths.insert(path.to_path_buf());
}

/// Every route that could write `path` is refused, and the file is untouched.
fn assert_saves_refused_and_file_untouched(app: &mut Signex, path: &Path, before: &str) {
    for err in [
        app.try_save_dirty_path(path).unwrap_err(),
        app.save_primitive_tab_at(path).unwrap_err(),
    ] {
        assert_eq!(err.to_string(), REFUSED);
        assert!(err.to_string().len() < 157, "Messages-panel budget");
    }
    assert_eq!(std::fs::read_to_string(path).expect("read"), before);
}

fn assert_cancelled_restore_landed_on_disk_only(app: &Signex, path: &Path, sha: &str) {
    assert!(
        !app.document_state.pending_restores.contains(path),
        "claim released"
    );
    assert!(app.refuse_save_while_restoring(path).is_ok());
    assert!(
        std::fs::read_to_string(path)
            .expect("read")
            .contains("name = \"old\""),
        "the worker's write is the final content"
    );
    assert!(app.document_state.pending_git_commits.is_empty());
    assert!(logged(&format!(
        "A cancelled restore of part.snxfpt to {sha} finished; its result was dropped"
    )));
}

#[test]
fn closing_the_tab_mid_restore_keeps_saves_refused_until_the_worker_reports() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, mut app) = footprint_fixture();
    let task = app.handle_history_restore_clicked(&sha);
    let before = std::fs::read_to_string(&path).expect("read");

    let _ = app.close_tab_now(0);
    assert!(
        app.document_state.pending_restores.contains(&path),
        "claim kept"
    );
    reopen(&mut app, &path);
    assert_saves_refused_and_file_untouched(&mut app, &path, &before);

    finish_restore(&mut app, collect_messages(task));
    assert_cancelled_restore_landed_on_disk_only(&app, &path, &sha);
    assert_eq!(editor_name(&app, &path), "reopened", "result dropped");
}

#[test]
fn close_all_mid_restore_keeps_saves_refused_until_the_worker_reports() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, mut app) = footprint_fixture();
    let task = app.handle_history_restore_clicked(&sha);
    let before = std::fs::read_to_string(&path).expect("read");

    let _ = app.update(Message::ContextMenu(crate::app::ContextMenuMsg::TabAction(
        crate::app::TabContextAction::CloseAll,
    )));
    assert!(
        app.document_state.pending_restores.contains(&path),
        "claim kept"
    );
    reopen(&mut app, &path);
    assert_saves_refused_and_file_untouched(&mut app, &path, &before);

    finish_restore(&mut app, collect_messages(task));
    assert_cancelled_restore_landed_on_disk_only(&app, &path, &sha);
}

#[test]
fn a_second_restore_is_refused_while_a_cancelled_one_still_runs() {
    let (_dir, path, sha, mut app) = footprint_fixture();
    let task = app.handle_history_restore_clicked(&sha);
    let _ = app.close_tab_now(0);
    reopen(&mut app, &path);
    app.document_state.dirty_paths.remove(&path);
    assert_eq!(
        app.handle_history_restore_clicked(&sha).units(),
        0,
        "no second worker next to the cancelled one"
    );
    finish_restore(&mut app, collect_messages(task));
    assert!(!app.document_state.pending_restores.contains(&path));
}

#[test]
fn saving_unchanged_content_is_not_an_edit_but_a_real_edit_is() {
    let (_dir, path, _sha, mut app) = footprint_fixture();
    marker_editor(&mut app, &path, "mine");
    // A dirty document, as a restore found it when it started.
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .dirty = true;
    app.document_state.tabs[0].dirty = true;
    let stamp = app.edit_stamp(&path);
    // What a completed save of the same content does.
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .dirty = false;
    app.document_state.tabs[0].dirty = false;
    assert_eq!(app.edit_stamp(&path), stamp);
    edit_description(&mut app, "edited");
    assert_ne!(app.edit_stamp(&path), stamp);
}
