//! #859 — symbol / footprint container saves run off the UI thread.

use std::path::{Path, PathBuf};

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex, SymbolDocument, TabInfo, TabKind};
use crate::persist::run_task::outputs;

/// An app with a dirty symbol editor (and its tab) keyed at `dir/Lib.snxsym`
/// whose project has git on.
fn app_with_dirty_symbol(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.document_state.projects[0].data.enable_git = true;
    let path = dir.join("Lib.snxsym");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Sym1"));
    let mut editor = SymbolDocument::new(path.clone(), file);
    editor.mark_dirty();
    app.document_state
        .symbol_editors
        .insert(path.clone(), editor);
    app.document_state.mark_document_dirty(&path);
    app.document_state.tabs.push(TabInfo {
        title: "Lib".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    (app, path)
}

fn land(app: &mut Signex, task: iced::Task<Message>) {
    for message in outputs(task) {
        let _ = app.update(message);
    }
}

#[test]
fn the_write_happens_when_the_task_runs_not_in_update() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());

    let task = app.save_primitive_tab_at(&path).expect("save starts");
    assert!(!path.exists(), "update itself wrote nothing");
    assert!(
        app.document_state.is_document_dirty(&path),
        "nothing is clean before the write completes"
    );

    land(&mut app, task);
    assert!(path.exists(), "the task wrote the container");
    assert!(!app.document_state.is_document_dirty(&path));
    assert!(
        !app.document_state
            .is_document_dirty(&app.document_state.tabs[0].path)
    );
    assert!(!app.document_state.symbol_editors[&path].is_dirty());
}

#[test]
fn a_saved_primitive_still_commits_to_the_project_git_repo() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    let task = app.save_primitive_tab_at(&path).expect("save starts");
    assert!(app.document_state.inflight_git_commits.is_empty());
    land(&mut app, task);
    assert_eq!(app.document_state.inflight_git_commits.len(), 1);
}

#[test]
fn a_failed_write_reaches_the_messages_panel_and_keeps_the_draft_dirty() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _) = app_with_dirty_symbol(dir.path());
    // A file where the container's directory belongs makes the write fail.
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"x").expect("blocker");
    let path = blocker.join("Lib.snxsym");
    let editor = app
        .document_state
        .symbol_editors
        .remove(&dir.path().join("Lib.snxsym"))
        .expect("editor");
    app.document_state
        .symbol_editors
        .insert(path.clone(), editor);
    app.document_state.mark_document_dirty(&path);

    let task = app.save_primitive_tab_at(&path).expect("save starts");
    land(&mut app, task);

    assert!(app.document_state.symbol_editors[&path].is_dirty());
    assert!(app.document_state.is_document_dirty(&path));
    assert!(app.document_state.error_notice.is_some());
    assert!(app.document_state.inflight_git_commits.is_empty());
    let path_text = path.display().to_string();
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|entry| entry.message.contains(&path_text)),
        "the failure must reach the Messages panel"
    );
}

#[test]
fn an_edit_during_the_write_keeps_the_draft_dirty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    let task = app.save_primitive_tab_at(&path).expect("save starts");
    // Every canvas edit sets this flag.
    app.document_state
        .symbol_editors
        .get_mut(&path)
        .expect("editor")
        .set_dirty(true);

    land(&mut app, task);

    assert!(path.exists(), "the snapshot was written");
    assert!(app.document_state.symbol_editors[&path].is_dirty());
    assert!(app.document_state.is_document_dirty(&path));
    assert!(
        app.document_state
            .is_document_dirty(&app.document_state.tabs[0].path)
    );
}

#[test]
fn quit_waits_for_a_running_container_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    let task = app.save_primitive_tab_at(&path).expect("save starts");
    let barrier = app.document_state.document_saves.barrier();
    let (tx, rx) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        barrier.wait();
        tx.send(()).expect("send");
    });
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "the barrier holds while the write has not run"
    );
    let _ = outputs(task);
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .expect("the barrier opens once the write finished");
    waiter.join().expect("waiter");
    assert!(path.exists());
}

/// Rename the editor's symbol, the way a canvas edit changes it.
fn edit_symbol(app: &mut Signex, path: &Path, name: &str) {
    let editor = app
        .document_state
        .symbol_editors
        .get_mut(path)
        .expect("editor");
    editor.primitive_mut().name = name.to_string();
    editor.mark_dirty();
}

/// The completion a container save task emits.
fn completion(task: iced::Task<Message>) -> crate::persist::PrimitiveSaveDone {
    match outputs(task).pop() {
        Some(Message::File(crate::app::FileMsg::PrimitiveSaved(done))) => *done,
        other => panic!("expected a container save completion, got {other:?}"),
    }
}

fn names_on_disk(path: &Path) -> Vec<String> {
    let bytes = std::fs::read(path).expect("read container");
    signex_library::SymbolFile::from_bytes(&bytes)
        .expect("parse container")
        .symbols
        .iter()
        .map(|symbol| symbol.name.clone())
        .collect()
}

#[test]
fn a_second_save_of_a_path_still_writing_queues_one_follow_up() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    let running = app.save_primitive_tab_at(&path).expect("save starts");
    edit_symbol(&mut app, &path, "Edited");

    let queued = app
        .save_primitive_tab_at(&path)
        .expect("queued, not refused");
    let repeated = app.save_primitive_tab_at(&path).expect("coalesced");
    assert_eq!(queued.units() + repeated.units(), 0, "nothing starts yet");
    assert!(
        app.document_state.error_notice.is_none(),
        "a queued save is not a failure"
    );

    let follow_up = app.handle_primitive_saved(completion(running));
    assert_eq!(follow_up.units(), 1, "one follow-up save starts");
    assert!(app.document_state.document_saves.is_saving(&path));
    let done = completion(follow_up);
    assert_eq!(
        app.handle_primitive_saved(done).units(),
        0,
        "repeats were coalesced into that one"
    );

    assert_eq!(names_on_disk(&path), vec!["Edited".to_string()]);
    assert!(!app.document_state.is_document_dirty(&path));
    assert!(!app.document_state.symbol_editors[&path].is_dirty());
}

/// Replace the editor at `path` with a fresh one holding `name`, as a reopen
/// does, with its own dirty marker set.
fn replace_editor(app: &mut Signex, path: &Path, name: &str) {
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty(name));
    app.document_state.symbol_editors.insert(
        path.to_path_buf(),
        SymbolDocument::new(path.to_path_buf(), file),
    );
    app.document_state.mark_document_dirty(path);
}

fn assert_replaced_editor_untouched(app: &Signex, path: &Path, name: &str) {
    let editor = &app.document_state.symbol_editors[path];
    assert_eq!(editor.baseline.symbols[0].name, name, "baseline untouched");
    assert!(
        app.document_state.is_document_dirty(path),
        "its dirty marker stays"
    );
    assert!(
        app.document_state.is_document_dirty(path),
        "its tab stays dirty"
    );
}

#[test]
fn a_late_completion_leaves_a_discarded_and_reopened_draft_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    let task = app.save_primitive_tab_at(&path).expect("save starts");
    // Discard: close the tab dropping the draft, then reopen the file.
    let _ = app.close_tab_now(0);
    assert!(!app.document_state.symbol_editors.contains_key(&path));
    app.document_state.tabs.push(TabInfo {
        title: "Lib".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    replace_editor(&mut app, &path, "Reopened");

    land(&mut app, task);

    assert_replaced_editor_untouched(&app, &path, "Reopened");
}

#[test]
fn a_late_completion_leaves_a_reopened_draft_dirty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    let task = app.save_primitive_tab_at(&path).expect("save starts");
    replace_editor(&mut app, &path, "Restored");

    land(&mut app, task);

    assert_replaced_editor_untouched(&app, &path, "Restored");
}

#[test]
fn a_failed_save_as_keeps_the_draft_when_the_original_was_reopened() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, from) = app_with_dirty_symbol(dir.path());
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"x").expect("blocker");
    let to = blocker.join("Other.snxsym");
    let task = app.handle_save_primitive_as(&from, &to).expect("starts");
    // The user reopens and edits the original file while the write runs.
    app.document_state.tabs.push(TabInfo {
        title: "Lib".into(),
        path: from.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(from.clone()),
    });
    replace_editor(&mut app, &from, "Reopened");
    app.document_state
        .symbol_editors
        .get_mut(&from)
        .expect("reopened")
        .set_dirty(true);

    land(&mut app, task);

    let draft = &app.document_state.symbol_editors[&to];
    assert_eq!(draft.primitive().name, "Sym1", "the Save As draft survives");
    assert!(draft.is_dirty());
    assert!(app.document_state.is_document_dirty(&to));
    assert_eq!(
        app.document_state.symbol_editors[&from].primitive().name,
        "Reopened",
        "the reopened editor is not overwritten"
    );
    assert!(app.document_state.is_document_dirty(&from));
    for path in [&from, &to] {
        let tabs = app.document_state.tabs.iter().filter(|t| t.path == *path);
        assert_eq!(tabs.count(), 1, "one tab on {}", path.display());
    }
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(
        format!("{notice:?}").contains("stays unsaved"),
        "{notice:?}"
    );
    assert!(app.document_state.document_saves.failure(&to).is_some());
}

#[test]
fn discarding_a_draft_forgets_its_save_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    app.document_state
        .document_saves
        .record_failure(path.clone(), "disk full".into());

    let _ = app.close_tab_now(0);

    assert!(app.document_state.document_saves.failure(&path).is_none());
}

#[test]
fn a_failed_save_as_puts_the_tab_back_where_it_was() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, from) = app_with_dirty_symbol(dir.path());
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"x").expect("blocker");
    let to = blocker.join("Other.snxsym");

    let task = app.handle_save_primitive_as(&from, &to).expect("starts");
    assert_eq!(
        app.document_state.tabs[0].path, to,
        "re-keyed while writing"
    );
    land(&mut app, task);

    assert_eq!(app.document_state.tabs[0].path, from);
    assert!(app.document_state.symbol_editors.contains_key(&from));
    assert!(app.document_state.symbol_editors[&from].is_dirty());
    assert!(app.document_state.is_document_dirty(&from));
    assert!(app.document_state.error_notice.is_some());
}

#[test]
fn a_follow_up_queued_by_a_replaced_editor_is_dropped() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    let running = app.save_primitive_tab_at(&path).expect("save starts");
    let queued = app.save_primitive_tab_at(&path).expect("queued");
    assert_eq!(queued.units(), 0);
    replace_editor(&mut app, &path, "Restored");
    app.document_state
        .symbol_editors
        .get_mut(&path)
        .expect("restored")
        .set_dirty(true);

    let follow_up = app.handle_primitive_saved(completion(running));

    assert_eq!(follow_up.units(), 0, "no save of a draft nobody asked for");
    assert!(!app.document_state.document_saves.is_saving(&path));
}

#[test]
fn a_late_completion_still_commits_the_landed_bytes_to_project_git() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    let task = app.save_primitive_tab_at(&path).expect("save starts");
    replace_editor(&mut app, &path, "Restored");

    land(&mut app, task);

    assert_eq!(app.document_state.inflight_git_commits.len(), 1);
    assert_replaced_editor_untouched(&app, &path, "Restored");
}

#[test]
fn a_late_completion_keeps_a_pending_restore_commit_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    let task = app.save_primitive_tab_at(&path).expect("save starts");
    // A History restore of this file is queued behind another commit that
    // holds the project root.
    let root = dir.path().to_path_buf();
    app.document_state.active_git_repos.insert(root.clone());
    app.commit_save_to_project_git(&path, "Restore Lib.snxsym");
    replace_editor(&mut app, &path, "Restored");

    land(&mut app, task);

    let pending = &app.document_state.pending_git_commits;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].message, "Restore Lib.snxsym");
}

/// Save As into a `.snxlib` path whose editor was replaced while the write
/// ran still reaches the library attach step. (A real manifest is a file,
/// not an ancestor directory of the primitive, so the attach reports that it
/// could not open the library; reaching that report is what is observed.)
#[test]
fn a_late_save_as_completion_still_attempts_the_library_attach() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, from) = app_with_dirty_symbol(dir.path());
    let lib = dir.path().join("Fresh.snxlib");
    let to = lib.join("symbols").join("Moved.snxsym");
    std::fs::create_dir_all(to.parent().expect("dir")).expect("symbols dir");

    let task = app.handle_save_primitive_as(&from, &to).expect("starts");
    replace_editor(&mut app, &to, "Reopened");

    land(&mut app, task);

    assert!(to.exists(), "the bytes landed");
    let lib_text = lib.display().to_string();
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|entry| entry.message.contains("could not open library")
                && entry.message.contains(&lib_text)),
        "the attach step ran for the mounted-library path"
    );
    assert_replaced_editor_untouched(&app, &to, "Reopened");
}

/// A drag tick owes a solve; a save started before it lands must finish it
/// first, or the container is written with the previous solve's pads (#1047).
#[test]
fn a_footprint_save_settles_the_drag_solve_it_owes() {
    use signex_sketch::SketchData;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("Lib.snxfpt");
    let (plane, point) = (PlaneId::new(), SketchEntityId::new());
    let mut fp = signex_library::Footprint::empty("Fp1");
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![Entity::new(
            point,
            plane,
            EntityKind::Point { x: 0.0, y: 0.0 },
        )],
        ..SketchData::default()
    });
    let mut editor = crate::app::FootprintDocument::new(
        path.clone(),
        signex_library::FootprintFile::from_footprint(fp),
    );
    crate::library::editor::footprint::updates::apply_footprint_primitive_edit(
        &mut editor,
        crate::library::messages::FootprintEditorMsg::SketchMovePoint {
            id: point,
            dx: 1.0,
            dy: 0.0,
        },
    );
    assert!(
        editor.state.sketch_solve.is_solve_owed(),
        "the tick owes a solve"
    );
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    let _task = app.save_primitive_tab_at(&path).expect("save starts");
    assert!(
        !app.document_state.footprint_editors[&path]
            .state
            .sketch_solve
            .is_solve_owed(),
        "the save finished the owed solve before snapshotting"
    );
}

#[test]
fn a_blocking_save_settles_the_draft_without_a_task() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());

    app.save_primitive_tab_blocking(&path).expect("save");

    assert!(path.exists());
    assert!(!app.document_state.is_document_dirty(&path));
    assert!(
        !app.document_state
            .is_document_dirty(&app.document_state.tabs[0].path)
    );
    assert!(!app.document_state.document_saves.is_saving(&path));
}

/// The flows that save blocking (Exclude prompt, project-close Save All)
/// chain `autosave_sweep` afterwards; the blocking save must leave the path
/// visible to that sweep so the recovery copy is removed.
#[test]
fn a_blocking_save_leaves_its_recovery_copy_to_the_sweep() {
    use crate::autosave::{DocStamp, sidecar};

    let dir = tempfile::tempdir().expect("tempdir");
    let recovery = tempfile::tempdir().expect("recovery dir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    app.document_state
        .autosave
        .set_dir(Some(recovery.path().to_path_buf()));
    sidecar::write(recovery.path(), &path, b"stale").expect("write sidecar");
    let stamp = DocStamp {
        revision: 1,
        net_color_revision: 0,
    };
    app.document_state.autosave.begin(&path);
    app.document_state.autosave.finish(&path, stamp, true);
    let copy = sidecar::sidecar_path(recovery.path(), &path);
    assert!(copy.exists());

    app.save_primitive_tab_blocking(&path).expect("save");
    let sweep = app.autosave_sweep();
    land(&mut app, sweep);

    assert!(!copy.exists(), "the sweep removes the saved file's sidecar");
}

/// The `no discarded tasks` CI gate bans `drop(<Task>)`; the blocking save
/// must settle through the Task-free helper, not by dropping the
/// completion handler's follow-up.
#[test]
fn the_blocking_save_does_not_discard_a_task() {
    let source = include_str!("primitive_save.rs");
    assert!(
        !source.contains("drop(self.handle_primitive_saved"),
        "settle through settle_primitive_save instead of dropping a Task"
    );
}
