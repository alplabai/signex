//! Annotate and Reset Duplicate Designators without file IO in `update`
//! (#859): what the three steps (read, number, write) guarantee, and what a
//! stale or failed step leaves behind.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_engine::AnnotateMode::ResetAndRenumber;
use signex_types::format::SnxSchematic;
use signex_types::schematic::Symbol;

use super::annotate_tests::{
    child_path, child_refs, logged_since, notice_detail, project, root_refs, sheet_of, unit,
};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::path_ops::settle_task;
use crate::app::{AnnotateMsg, Message, Signex};
use crate::persist::run_task::outputs;

/// The one message a worker's task delivers: the step's completion.
fn completion(task: Task<Message>) -> Message {
    let mut messages = outputs(task);
    assert_eq!(messages.len(), 1, "one completion per worker");
    messages.remove(0)
}

fn write_child(dir: &Path, name: &str, symbols: Vec<Symbol>) {
    let text = SnxSchematic::new(sheet_of(&[], symbols))
        .write_string()
        .expect("encode");
    std::fs::write(dir.join(name), text).expect("write child");
}

fn root_path(dir: &Path) -> PathBuf {
    dir.join("a_root.snxsch")
}

/// The root is open; `m_child` and `z_child` are unopened children on disk.
fn project_with_two_children(
    root: Vec<Symbol>,
    m: Vec<Symbol>,
    z: Vec<Symbol>,
) -> (Signex, PathBuf) {
    let dir = std::env::temp_dir().join(format!("signex-annotate-job-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    write_child(&dir, "m_child.snxsch", m);
    write_child(&dir, "z_child.snxsch", z);
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    open_with(
        &mut app.document_state,
        &root_path(&dir),
        sheet_of(&["m_child.snxsch", "z_child.snxsch"], root),
    );
    app.document_state.active_path = Some(root_path(&dir));
    (app, dir)
}

/// Put a directory where the sheet file was: the worker reads nothing wrong
/// with it any more, but cannot replace it.
fn make_unwritable(path: &Path) {
    std::fs::remove_file(path).expect("remove sheet");
    std::fs::create_dir(path).expect("block the path");
}

fn numbered(refs: &[String]) -> bool {
    refs.iter().all(|r| !r.ends_with('?'))
}

fn unique(refs: impl IntoIterator<Item = String>) -> bool {
    let all: Vec<String> = refs.into_iter().collect();
    let distinct: std::collections::HashSet<&String> = all.iter().collect();
    distinct.len() == all.len()
}

fn pending_nothing(app: &Signex, paths: &[&Path]) -> bool {
    !app.ui_state.annotate_runs.is_running()
        && app.document_state.pending_path_ops.is_empty()
        && paths
            .iter()
            .all(|p| !app.document_state.document_saves.is_saving(p))
}

// ---- (a) nothing blocks update.

#[test]
fn update_reads_and_writes_no_unopened_sheet_until_its_task_runs() {
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![unit("R?", 1, 0.0)]);
    let read = app.handle_annotate(ResetAndRenumber);
    assert!(app.ui_state.annotate_runs.is_running());
    // Replace the child before the worker runs: a read inside `update` would
    // not have seen two symbols.
    write_child(
        &dir,
        "z_child.snxsch",
        vec![unit("R?", 1, 0.0), unit("R?", 1, 10.0)],
    );
    let before = std::fs::read(child_path(&dir)).expect("child");

    let write = app.update(completion(read));

    assert_eq!(
        std::fs::read(child_path(&dir)).expect("child"),
        before,
        "numbering must not write the unopened sheet"
    );
    assert!(
        app.document_state
            .document_saves
            .is_saving(&child_path(&dir))
    );
    settle_task(&mut app, write);
    let (root, child) = (root_refs(&app, &dir), child_refs(&dir));
    let idle = pending_nothing(&app, &[&child_path(&dir)]);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(child.len(), 2, "the worker read the replaced file");
    assert!(numbered(&child) && numbered(&root));
    assert!(idle);
}

#[test]
fn reset_duplicates_writes_nothing_in_update_either() {
    let (mut app, dir) = project(vec![unit("R1", 1, 0.0)], vec![unit("R1", 1, 0.0)]);
    let read = app.handle_reset_duplicate_designators();
    let before = std::fs::read(child_path(&dir)).expect("child");

    let write = app.update(completion(read));

    let untouched = std::fs::read(child_path(&dir)).expect("child") == before;
    settle_task(&mut app, write);
    let child = child_refs(&dir);
    std::fs::remove_dir_all(&dir).ok();
    assert!(untouched, "the reset must not write in update");
    assert_eq!(child, ["R?"]);
}

// ---- (c) a sheet that changed while the worker read.

#[test]
fn an_unopened_sheet_opened_while_the_sheets_are_read_is_not_overwritten() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![unit("R?", 1, 0.0)]);
    let read = app.handle_annotate(ResetAndRenumber);
    let message = completion(read);
    // The user opens the child and edits it before the result arrives.
    let child = child_path(&dir);
    let mut edited = sheet_of(&[], vec![unit("R42", 1, 0.0)]);
    edited.symbols[0].reference = "R42".to_string();
    let engine = signex_engine::Engine::new_with_path(edited, Some(child.clone())).expect("engine");
    app.document_state.insert_engine(child.clone(), engine);
    let on_disk = std::fs::read(&child).expect("child");
    let since = crate::diagnostics::newest_entry_id();

    let task = app.update(message);
    settle_task(&mut app, task);

    let open_refs: Vec<String> = app.document_state.engines[&child]
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect();
    let (root, notice) = (root_refs(&app, &dir), notice_detail(&app));
    let (same, idle) = (
        std::fs::read(&child).expect("child") == on_disk,
        pending_nothing(&app, &[&child]),
    );
    std::fs::remove_dir_all(&dir).ok();
    assert!(same, "the file the user opened was rewritten");
    assert_eq!(open_refs, ["R42"], "the user's edit was overwritten");
    assert_eq!(root, ["R?"], "all or nothing: the open sheet was numbered");
    assert!(notice.contains("Nothing was changed"), "{notice}");
    assert!(logged_since(since, "Annotate was not run"));
    assert!(idle);
}

#[test]
fn an_edit_to_an_open_sheet_while_the_sheets_are_read_is_numbered_not_lost() {
    // `z_child` is open and sorts after the unopened `m_child`, so `m_child`
    // is numbered before `z_child` is reached.
    let (mut app, dir) = project_with_two_children(
        vec![unit("R?", 1, 0.0)],
        vec![unit("R?", 1, 0.0)],
        vec![unit("R?", 1, 0.0)],
    );
    let z = dir.join("z_child.snxsch");
    open_with(
        &mut app.document_state,
        &z,
        sheet_of(&[], vec![unit("R?", 1, 0.0)]),
    );
    let read = app.handle_annotate(signex_engine::AnnotateMode::Incremental);
    let message = completion(read);
    // R1 typed into the open `z_child` after the worker started: the counter
    // must know it, or `m_child` would take R1 as well.
    let engine = app.document_state.engines.get_mut(&z).expect("z_child");
    let mut document = engine.document().clone();
    document.symbols.push(unit("R1", 1, 20.0));
    engine
        .execute(signex_engine::Command::ReplaceDocument { document })
        .expect("edit");

    let task = app.update(message);
    settle_task(&mut app, task);

    let z_refs: Vec<String> = app.document_state.engines[&z]
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect();
    let m_refs = SnxSchematic::parse(
        &std::fs::read_to_string(dir.join("m_child.snxsch")).expect("m on disk"),
    )
    .expect("parse")
    .sheet
    .symbols
    .iter()
    .map(|s| s.reference.clone())
    .collect::<Vec<_>>();
    let root = root_refs(&app, &dir);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(z_refs.len(), 2, "the edit is still there");
    assert!(z_refs.contains(&"R1".to_string()), "{z_refs:?}");
    assert!(numbered(&z_refs) && numbered(&m_refs) && numbered(&root));
    assert!(
        unique(root.into_iter().chain(m_refs).chain(z_refs)),
        "the late R1 must not be handed out again"
    );
}

#[test]
fn a_document_saved_or_moved_while_the_sheets_are_read_cancels_the_run() {
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![unit("R?", 1, 0.0)]);
    let read = app.handle_annotate(ResetAndRenumber);
    let message = completion(read);
    // What a save completion or the start of a rename does.
    app.document_state.note_documents_changed();
    let on_disk = std::fs::read(child_path(&dir)).expect("child");

    let task = app.update(message);
    settle_task(&mut app, task);

    let (root, same) = (
        root_refs(&app, &dir),
        std::fs::read(child_path(&dir)).expect("child") == on_disk,
    );
    let idle = pending_nothing(&app, &[&child_path(&dir)]);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(root, ["R?"], "nothing was numbered");
    assert!(same && idle);
}

#[test]
fn a_sheet_symbol_placed_while_the_sheets_are_read_cancels_the_run() {
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![unit("R?", 1, 0.0)]);
    let read = app.handle_annotate(ResetAndRenumber);
    let message = completion(read);
    let root = root_path(&dir);
    let engine = app.document_state.engines.get_mut(&root).expect("root");
    let document = sheet_of(
        &["z_child.snxsch", "extra.snxsch"],
        engine.document().symbols.clone(),
    );
    engine
        .execute(signex_engine::Command::ReplaceDocument { document })
        .expect("edit");
    let on_disk = std::fs::read(child_path(&dir)).expect("child");

    let task = app.update(message);
    settle_task(&mut app, task);

    let (root_refs, same) = (
        root_refs(&app, &dir),
        std::fs::read(child_path(&dir)).expect("child") == on_disk,
    );
    let idle = pending_nothing(&app, &[&child_path(&dir)]);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(root_refs, ["R?"], "nothing was numbered");
    assert!(same && idle);
}

// ---- (d) the sheets are held for the whole run.

#[test]
fn a_rename_or_delete_of_the_runs_sheets_is_refused_until_it_ends() {
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![unit("R?", 1, 0.0)]);
    let (root, child) = (root_path(&dir), child_path(&dir));
    let read = app.handle_annotate(ResetAndRenumber);
    let while_reading = (
        app.refuse_start_while_path_op(&root, "renamed").is_err(),
        app.refuse_while_path_op(&root, "saved").is_err(),
    );

    let write = app.update(completion(read));
    let while_writing = (
        app.refuse_start_while_path_op(&child, "renamed").is_err(),
        app.refuse_start_while_path_op(&child, "deleted").is_err(),
        app.refuse_while_path_op(&child, "opened").is_err(),
        app.document_state.document_saves.is_saving(&child),
    );

    settle_task(&mut app, write);
    let after = (
        app.refuse_start_while_path_op(&child, "renamed").is_ok(),
        app.refuse_start_while_path_op(&root, "renamed").is_ok(),
        pending_nothing(&app, &[&child]),
    );
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(while_reading, (true, true));
    assert_eq!(while_writing, (true, true, true, true));
    assert_eq!(after, (true, true, true));
}

#[test]
fn a_second_run_while_one_is_going_is_refused_and_the_first_completes() {
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![unit("R?", 1, 0.0)]);
    let first = app.handle_annotate(ResetAndRenumber);

    let second = app.handle_annotate(ResetAndRenumber);
    let reset = app.handle_reset_duplicate_designators();
    let refused = notice_detail(&app);
    settle_task(&mut app, second);
    settle_task(&mut app, reset);
    app.document_state.error_notice = None;
    settle_task(&mut app, first);

    let (root, child) = (root_refs(&app, &dir), child_refs(&dir));
    let idle = pending_nothing(&app, &[&child_path(&dir)]);
    std::fs::remove_dir_all(&dir).ok();
    assert!(refused.contains("still going"), "{refused}");
    assert!(numbered(&root) && numbered(&child) && idle);
}

// ---- (e) a write that fails.

#[test]
fn one_sheet_that_cannot_be_written_is_reported_and_the_rest_stand() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, dir) = project_with_two_children(
        vec![unit("R?", 1, 0.0)],
        vec![unit("R?", 1, 0.0)],
        vec![unit("R?", 1, 0.0)],
    );
    let (m, z) = (dir.join("m_child.snxsch"), dir.join("z_child.snxsch"));
    let read = app.handle_annotate(ResetAndRenumber);
    let write = app.update(completion(read));
    make_unwritable(&z);
    let since = crate::diagnostics::newest_entry_id();

    settle_task(&mut app, write);

    let m_refs = SnxSchematic::parse(&std::fs::read_to_string(&m).expect("m on disk"))
        .expect("parse")
        .sheet
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect::<Vec<_>>();
    let ds = &app.document_state;
    let state = (
        z.is_dir(),
        ds.is_document_dirty(&m) || ds.is_document_dirty(&z),
        numbered(&root_refs(&app, &dir)),
    );
    let notice = notice_detail(&app);
    let idle = pending_nothing(&app, &[&m, &z]);
    std::fs::remove_dir_all(&dir).ok();
    assert!(numbered(&m_refs), "the sheet that could be written was");
    assert_eq!(
        state,
        (true, false, true),
        "failed sheet untouched, no stray dirty marks, open sheet numbered"
    );
    assert!(notice.contains("could not write 1 unopened"), "{notice}");
    assert!(notice.contains("z_child.snxsch"), "{notice}");
    assert!(logged_since(since, "Annotate could not write"));
    assert!(idle);
}

#[test]
fn a_reset_unopened_sheet_is_clean_only_when_its_write_landed() {
    for fail in [false, true] {
        let (mut app, dir) = project(vec![unit("R1", 1, 0.0)], vec![unit("R1", 1, 0.0)]);
        let child = child_path(&dir);
        app.document_state.mark_document_dirty(&child);
        let read = app.handle_reset_duplicate_designators();
        let write = app.update(completion(read));
        if fail {
            make_unwritable(&child);
        }

        settle_task(&mut app, write);

        let dirty = app.document_state.is_document_dirty(&child);
        let root = root_refs(&app, &dir);
        let on_disk = (!fail).then(|| child_refs(&dir));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(dirty, fail, "clean exactly when the bytes landed");
        assert_eq!(root, ["R?"], "the open sheet was reset either way");
        assert_eq!(on_disk, (!fail).then(|| vec!["R?".to_string()]));
    }
}

#[test]
fn a_worker_that_died_is_reported_and_releases_every_sheet() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![unit("R?", 1, 0.0)]);
    let child = child_path(&dir);
    let before = std::fs::read(&child).expect("child");
    // The read worker died.
    let _unrun = app.handle_annotate(ResetAndRenumber);
    let generation = app.ui_state.annotate_runs.live_generation().expect("run");
    let dead_read = Message::Annotate(AnnotateMsg::PassSheetsRead {
        generation,
        result: Arc::new(Err("the background task failed".to_string())),
    });
    let task = app.update(dead_read);
    settle_task(&mut app, task);
    let after_read = (notice_detail(&app), pending_nothing(&app, &[&child]));
    // The write worker died.
    let read = app.handle_annotate(ResetAndRenumber);
    let unrun_write = app.update(completion(read));
    let generation = app.ui_state.annotate_runs.live_generation().expect("run");
    drop(unrun_write);
    let dead_write = Message::Annotate(AnnotateMsg::PassSheetsWritten {
        generation,
        result: Arc::new(Err("the background task failed".to_string())),
        hold: app.document_state.autosave.hold_for_save(),
    });
    let task = app.update(dead_write);
    settle_task(&mut app, task);
    let notice = notice_detail(&app);
    let (idle, unchanged) = (
        pending_nothing(&app, &[&child]),
        std::fs::read(&child).expect("child") == before,
    );
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        after_read.0.contains("could not be read"),
        "{}",
        after_read.0
    );
    assert!(after_read.1, "the guard outlived a dead read worker");
    assert!(notice.contains("z_child.snxsch"), "{notice}");
    assert!(idle && unchanged);
}

#[test]
fn an_unopened_sheet_with_a_save_running_is_left_alone() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![unit("R5", 1, 0.0)]);
    let child = child_path(&dir);
    let _save = app
        .document_state
        .document_saves
        .begin(child.clone(), b"unused".to_vec());
    let since = crate::diagnostics::newest_entry_id();

    let task = app.handle_annotate(ResetAndRenumber);
    settle_task(&mut app, task);

    let refs = child_refs(&dir);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(refs, ["R5"], "a sheet a save owns is not rewritten");
    assert!(logged_since(
        since,
        "Annotate: z_child.snxsch was left unchanged, a save of it is still running"
    ));
}

// ---- (g) undo of the open part.

#[test]
fn the_open_sheet_of_a_run_is_one_undo_step() {
    let (mut app, dir) = project(
        vec![unit("R?", 1, 0.0), unit("R?", 1, 10.0)],
        vec![unit("R?", 1, 0.0)],
    );
    let task = app.handle_annotate(ResetAndRenumber);
    settle_task(&mut app, task);

    let engine = app
        .document_state
        .engines
        .get_mut(&root_path(&dir))
        .expect("root");
    let first = engine.undo().expect("undo").is_some();
    let after_undo: Vec<String> = engine
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect();
    let second = engine.undo().expect("undo").is_some();
    std::fs::remove_dir_all(&dir).ok();
    assert!(first && !second, "one step for the sheet, no more");
    assert_eq!(after_undo, ["R?", "R?"]);
}

#[test]
fn the_project_undo_group_records_the_open_sheet_and_the_written_one() {
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![unit("R?", 1, 0.0)]);
    app.ui_state.annotate_undo = crate::annotate_undo::AnnotateUndo::Project;
    let task = app.handle_annotate(ResetAndRenumber);
    settle_task(&mut app, task);

    let groups: Vec<_> = app.document_state.undo_groups.values().collect();
    let record = (
        groups.len(),
        groups.first().map(|g| g.live.clone()),
        groups.first().map(|g| g.disk_only.clone()),
    );
    let (root, child) = (root_path(&dir), child_path(&dir));
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        record,
        (1, Some(vec![root]), Some(vec![child])),
        "undo must say the written sheet stays annotated"
    );
}

#[test]
fn an_undo_while_the_unopened_sheets_are_still_being_written_names_them() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![unit("R?", 1, 0.0)]);
    app.ui_state.annotate_undo = crate::annotate_undo::AnnotateUndo::Project;
    let read = app.handle_annotate(ResetAndRenumber);
    let write = app.update(completion(read));
    let since = crate::diagnostics::newest_entry_id();

    // Ctrl+Z after numbering, before the write worker has reported.
    let undo = app.update(Message::Edit(crate::app::EditMsg::Undo));
    settle_task(&mut app, undo);
    let named = logged_since(since, "not open, so still annotated on disk");
    let child = child_path(&dir);
    let mentions_child = crate::diagnostics::recent_entries().iter().any(|e| {
        Some(e.id) > since
            && e.message.contains("still annotated on disk")
            && e.message.contains("z_child.snxsch")
    });
    settle_task(&mut app, write);
    std::fs::remove_dir_all(&dir).ok();
    assert!(named && mentions_child, "undo must warn about {child:?}");
}
