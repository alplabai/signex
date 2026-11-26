//! A sheet a project-wide run parked is an open document (#2760): an outside
//! change is never overwritten, an earlier recovery copy survives autosave,
//! undo reaches every sheet until you save and is refused whole after, and it
//! follows a rename.

use std::path::{Path, PathBuf};

use signex_engine::AnnotateMode::ResetAndRenumber;

use super::annotate_tests::{
    child_path, child_refs, logged_since, notice_detail, project, root_refs, unit,
};
use crate::annotate_undo::AnnotateUndo;
use crate::app::path_ops::settle_task;
use crate::app::state::NoticeAction;
use crate::app::{AnnotateMsg, EditMsg, FileMsg, Message, Signex, TabInfo, TabKind};
use crate::autosave::AutosaveMsg;
use crate::menu_bar::MenuMessage;
use crate::persist::ContentHash;

/// An open root (R5) and an unopened child (R7), one-step undo.
fn fixture() -> (Signex, PathBuf, PathBuf) {
    let (mut app, dir) = project(vec![unit("R5", 1, 0.0)], vec![unit("R7", 1, 0.0)]);
    app.ui_state.annotate_undo = AnnotateUndo::Project;
    app.document_state.tabs.push(TabInfo {
        title: "a_root.snxsch".into(),
        path: dir.join("a_root.snxsch"),
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let child = child_path(&dir);
    (app, dir, child)
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    settle_task(app, task);
}

fn annotate(app: &mut Signex) {
    send(app, Message::Annotate(AnnotateMsg::Run(ResetAndRenumber)));
}

fn save_all(app: &mut Signex) {
    send(app, Message::Menu(MenuMessage::SaveAll));
}

fn undo(app: &mut Signex) {
    send(app, Message::Edit(EditMsg::Undo));
}

fn held_refs(app: &Signex, path: &Path) -> Vec<String> {
    (app.document_state.engines[path].document().symbols.iter())
        .map(|symbol| symbol.reference.clone())
        .collect()
}

fn card_names(app: &Signex, path: &Path) -> bool {
    let action = (app.document_state.error_notice.as_ref()).and_then(|n| n.action.as_ref());
    matches!(action, Some(NoticeAction::DiskChanged { path: card, .. }) if card == path)
}

// ---- An outside change.

#[test]
fn every_parked_sheet_records_the_bytes_the_run_read() {
    let (mut app, dir, child) = fixture();
    let read = ContentHash::of(&std::fs::read(&child).expect("child"));
    annotate(&mut app);
    let baseline = app.document_state.document_saves.baseline(&child);
    let carried = app.document_state.engines[&child]
        .document()
        .unknown
        .source_hash
        .clone();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(baseline, Some(read));
    assert_eq!(carried, None, "undo snapshots do not carry the fingerprint");
}

#[test]
fn reset_duplicates_parks_with_the_same_baseline() {
    let (mut app, dir) = project(vec![unit("R1", 1, 0.0)], vec![unit("R1", 1, 0.0)]);
    let child = child_path(&dir);
    let read = ContentHash::of(&std::fs::read(&child).expect("child"));
    let task = app.handle_reset_duplicate_designators();
    settle_task(&mut app, task);
    let baseline = app.document_state.document_saves.baseline(&child);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(baseline, Some(read));
}

#[test]
fn save_all_does_not_overwrite_a_parked_sheet_changed_on_disk() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, dir, child) = fixture();
    annotate(&mut app);
    std::fs::write(&child, b"written by another program").expect("outside change");
    let since = crate::diagnostics::newest_entry_id();

    save_all(&mut app);

    let on_disk = std::fs::read(&child).expect("child");
    let dirty = app.document_state.is_document_dirty(&child);
    let said = logged_since(since, "changed on disk");
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(on_disk, b"written by another program");
    assert!(dirty, "the numbering is still unsaved");
    assert!(said, "the user is told");
}

#[test]
fn the_idle_poll_reports_a_parked_sheet_changed_on_disk() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, dir, child) = fixture();
    annotate(&mut app);
    std::fs::write(&child, b"written by another program").expect("outside change");

    let poll = app.start_document_disk_poll();
    settle_task(&mut app, poll);

    let told = card_names(&app, &child);
    std::fs::remove_dir_all(&dir).ok();
    assert!(told, "the changed-on-disk card names the parked sheet");
}

// ---- An earlier run's recovery copy.

#[test]
fn an_autosave_tick_does_not_replace_an_earlier_runs_recovery_copy() {
    let (mut app, dir, child) = fixture();
    let recovery = dir.join("recovery");
    app.document_state.autosave.set_dir(Some(recovery.clone()));
    app.ui_state.autosave_enabled = true;
    crate::autosave::sidecar::write(&recovery, &child, b"edits from an earlier run")
        .expect("sidecar");
    let sidecar = crate::autosave::sidecar::sidecar_path(&recovery, &child);
    let before = std::fs::read(&sidecar).expect("sidecar");

    // The tick lands while the recovery check is still running.
    let read = app.handle_annotate(ResetAndRenumber);
    let numbered = crate::persist::run_task::outputs(read).remove(0);
    let checks = app.update(numbered);
    send(&mut app, Message::Autosave(AutosaveMsg::Ticked));
    let during = std::fs::read(&sidecar).expect("sidecar");
    settle_task(&mut app, checks);
    // And once it has reported.
    send(&mut app, Message::Autosave(AutosaveMsg::Ticked));
    let after = std::fs::read(&sidecar).expect("sidecar");
    let offered = (app.document_state.autosave.current_offer()).map(|offer| offer.path.clone());
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(during, before, "not replaced before the check reported");
    assert_eq!(after, before, "not replaced while the offer is up");
    assert_eq!(offered, Some(child), "the copy is offered");
}

// ---- Undo after a save.

#[test]
fn annotate_undo_before_saving_restores_every_sheet() {
    let (mut app, dir, child) = fixture();
    let on_disk = std::fs::read(&child).expect("child");
    annotate(&mut app);

    undo(&mut app);

    let root = root_refs(&app, &dir);
    let released = !app.document_state.engines.contains_key(&child);
    let unchanged = std::fs::read(&child).expect("child") == on_disk;
    let refs = child_refs(&dir);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(root, ["R5"]);
    assert_eq!(refs, ["R7"]);
    assert!(released && unchanged, "the child is back at its file");
}

#[test]
fn annotate_save_all_then_undo_is_refused_whole_and_names_the_sheet() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, dir, child) = fixture();
    let root_path = dir.join("a_root.snxsch");
    annotate(&mut app);
    save_all(&mut app);
    let released = !app.document_state.engines.contains_key(&child);
    let saved = (
        std::fs::read(&root_path).expect("root"),
        std::fs::read(&child).expect("child"),
    );
    let since = crate::diagnostics::newest_entry_id();

    undo(&mut app);

    let root = root_refs(&app, &dir);
    let root_dirty = app.document_state.is_document_dirty(&root_path);
    let child_dirty = app.document_state.is_document_dirty(&child);
    let after = (
        std::fs::read(&root_path).expect("root"),
        std::fs::read(&child).expect("child"),
    );
    let refs = child_refs(&dir);
    let named = logged_since(
        since,
        "Cannot undo the project-wide annotate: z_child.snxsch was saved and closed",
    );
    std::fs::remove_dir_all(&dir).ok();
    assert!(released, "a clean save releases the tabless engine");
    assert_eq!(root, ["R1"], "the open sheet did not move");
    assert_eq!(refs, ["R2"]);
    assert!(!root_dirty && !child_dirty, "nothing became unsaved");
    assert!(after == saved, "no file changed");
    assert!(named, "one Messages line names the sheet");
}

/// The unsaved child is still in memory, so the undo reaches both: the saved
/// root is then unsaved, and the child is back at its untouched file.
#[test]
fn annotate_save_only_the_open_sheet_then_undo_restores_both() {
    let (mut app, dir, child) = fixture();
    let root_path = dir.join("a_root.snxsch");
    let child_bytes = std::fs::read(&child).expect("child");
    annotate(&mut app);
    send(&mut app, Message::File(FileMsg::Save));
    let root_saved = std::fs::read(&root_path).expect("root");

    undo(&mut app);

    let root = root_refs(&app, &dir);
    let root_dirty = app.document_state.is_document_dirty(&root_path);
    let child_released = !app.document_state.engines.contains_key(&child);
    let child_dirty = app.document_state.is_document_dirty(&child);
    let files = (
        std::fs::read(&root_path).expect("root") == root_saved,
        std::fs::read(&child).expect("child") == child_bytes,
    );
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(root, ["R5"]);
    assert!(root_dirty, "the undone root differs from its saved file");
    assert!(
        child_released && !child_dirty,
        "the child is back at its file"
    );
    assert_eq!(files, (true, true), "the undo writes nothing");
}

/// Reopening gives a fresh history, so the run cannot reach that sheet.
#[test]
fn a_sheet_reopened_after_the_save_refuses_the_undo_whole() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, dir, child) = fixture();
    let root_path = dir.join("a_root.snxsch");
    annotate(&mut app);
    save_all(&mut app);
    assert!(app.open_or_focus_sheet_path(child.clone()));
    assert!(app.activate_tab_for_path(&root_path));
    let since = crate::diagnostics::newest_entry_id();

    undo(&mut app);

    let (root, reopened) = (root_refs(&app, &dir), held_refs(&app, &child));
    let named = logged_since(since, "z_child.snxsch was saved and closed");
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(root, ["R1"]);
    assert_eq!(reopened, ["R2"]);
    assert!(named);
}

/// A sheet with an open tab keeps its engine through the save, so its undo.
#[test]
fn sheets_with_open_tabs_undo_together_after_a_save() {
    let (mut app, dir, child) = fixture();
    let root_path = dir.join("a_root.snxsch");
    assert!(app.open_or_focus_sheet_path(child.clone()));
    assert!(app.activate_tab_for_path(&root_path));
    annotate(&mut app);
    save_all(&mut app);

    undo(&mut app);

    let (root, held) = (root_refs(&app, &dir), held_refs(&app, &child));
    let dirty = app.document_state.is_document_dirty(&child);
    let on_disk = child_refs(&dir);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        (root, held),
        (vec!["R5".to_string()], vec!["R7".to_string()])
    );
    assert!(dirty, "the undone sheet differs from its saved file");
    assert_eq!(on_disk, ["R2"], "the undo writes nothing");
}

#[test]
fn a_save_landing_after_an_undo_leaves_the_sheet_unsaved_through_redo_and_undo() {
    let (mut app, dir, child) = fixture();
    annotate(&mut app);
    let write = app.update(Message::Menu(MenuMessage::SaveAll));
    undo(&mut app);
    settle_task(&mut app, write);
    send(&mut app, Message::Edit(EditMsg::Redo));

    undo(&mut app);

    let dirty = app.document_state.is_document_dirty(&child);
    let kept = app.document_state.engines.contains_key(&child);
    let on_disk = child_refs(&dir);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(on_disk, ["R2"], "the write landed");
    assert!(kept, "the undone sheet is not dropped as if clean");
    assert!(dirty, "the file holds the annotated bytes");
}

// ---- Reset Duplicates is refused whole.

#[test]
fn reset_duplicates_with_an_unreadable_sheet_changes_nothing() {
    let (mut app, dir) = project(
        vec![unit("R1", 1, 0.0), unit("R1", 1, 10.0)],
        vec![unit("R7", 1, 0.0)],
    );
    let child = child_path(&dir);
    std::fs::write(&child, "not a schematic").expect("corrupt the child");

    let task = app.handle_reset_duplicate_designators();
    settle_task(&mut app, task);

    let root = root_refs(&app, &dir);
    let notice = notice_detail(&app);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(root, ["R1", "R1"], "no partial reset");
    assert!(notice.contains("z_child.snxsch"), "{notice}");
    assert!(notice.contains("Nothing was changed"), "{notice}");
}

#[test]
fn reset_duplicates_with_a_blocked_sheet_changes_nothing() {
    let (mut app, dir) = project(vec![unit("R1", 1, 0.0)], vec![unit("R1", 1, 0.0)]);
    let child = child_path(&dir);
    let task = app.handle_reset_duplicate_designators();
    // The read of the child starts after the run's own sheets were read.
    app.document_state.pending_opens.begin(child.clone());
    settle_task(&mut app, task);

    let (root, parked) = (
        root_refs(&app, &dir),
        app.document_state.engines.contains_key(&child),
    );
    let notice = notice_detail(&app);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(root, ["R1"], "no partial reset");
    assert!(!parked);
    assert!(notice.contains("z_child.snxsch"), "{notice}");
    assert!(notice.contains("Nothing was changed"), "{notice}");
}

// ---- Saves outside the async path, closed tabs and refused steps.

/// The blocking save of Exclude from Project must end the run's undo for the
/// sheet it wrote, or Ctrl+Z reverts the root alone.
#[test]
fn excluding_a_parked_sheet_with_save_refuses_the_undo_whole() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, dir, child) = fixture();
    let pages = &mut app.document_state.projects[0].data.sheets;
    let mut page = pages[0].clone();
    page.filename = "z_child.snxsch".to_string();
    pages.push(page);
    annotate(&mut app);
    app.ui_state.remove_dialog = Some(crate::app::RemoveDialogState {
        target_path: child.clone(),
        tree_path: vec![0, 1],
        project_id: Some(app.document_state.projects[0].id),
        display_name: "z_child.snxsch".to_string(),
        is_outside_project: false,
        unsaved_prompt: true,
        is_directory_package: false,
        choice: None,
    });
    send(
        &mut app,
        Message::Remove(crate::app::RemoveMsg::ExcludeDirty(
            crate::app::ExcludeDirtyChoice::Save,
        )),
    );
    let released = !app.document_state.engines.contains_key(&child);
    let saved = child_refs(&dir);
    let since = crate::diagnostics::newest_entry_id();

    undo(&mut app);

    let root = root_refs(&app, &dir);
    let after = child_refs(&dir);
    let named = logged_since(since, "z_child.snxsch was saved and closed");
    std::fs::remove_dir_all(&dir).ok();
    assert!(released, "the blocking save releases the tabless engine");
    assert_eq!(saved, ["R2"]);
    assert_eq!(root, ["R1"], "the root did not revert alone");
    assert_eq!(after, ["R2"]);
    assert!(named, "the refusal names the sheet");
}

#[test]
fn a_clean_sheet_whose_tab_closed_after_the_run_is_released_by_the_undo() {
    let (mut app, dir, child) = fixture();
    let root_path = dir.join("a_root.snxsch");
    let on_disk = std::fs::read(&child).expect("child");
    assert!(app.open_or_focus_sheet_path(child.clone()));
    assert!(app.activate_tab_for_path(&root_path));
    annotate(&mut app);
    let idx = (app.document_state.tabs.iter())
        .position(|tab| tab.path == child)
        .expect("child tab");
    let _close = app.close_tab_now(idx);
    assert!(app.activate_tab_for_path(&root_path));
    let parked = app.document_state.engines.contains_key(&child);

    undo(&mut app);

    let root = root_refs(&app, &dir);
    let released = !app.document_state.engines.contains_key(&child);
    let dirty = app.document_state.is_document_dirty(&child);
    let unchanged = std::fs::read(&child).expect("child") == on_disk;
    std::fs::remove_dir_all(&dir).ok();
    assert!(parked, "the unsaved sheet was parked when its tab closed");
    assert_eq!(root, ["R5"]);
    assert!(released && !dirty, "no clean tabless engine is left");
    assert!(unchanged);
}

/// A refused step moves nothing, not even a dead class-pick marker.
#[test]
fn a_refused_undo_leaves_a_dead_pick_marker_where_it_was() {
    let (mut app, dir, child) = fixture();
    let root_path = dir.join("a_root.snxsch");
    assert!(app.open_or_focus_sheet_path(child.clone()));
    assert!(app.activate_tab_for_path(&root_path));
    annotate(&mut app);
    (app.document_state.engines.get_mut(&child))
        .expect("child")
        .record_class_pick(signex_engine::ClassPickMarker {
            project: 1,
            step: 1,
        });
    for group in app.document_state.undo_groups.values_mut() {
        group.live.push(dir.join("gone.snxsch"));
    }

    undo(&mut app);

    let marker = app.document_state.engines[&child].next_class_pick(true);
    let root = root_refs(&app, &dir);
    std::fs::remove_dir_all(&dir).ok();
    assert!(marker.is_some(), "the marker was not passed");
    assert_eq!(root, ["R1"], "nothing moved");
}

// ---- A rename while parked.

#[test]
fn a_parked_sheet_renamed_on_disk_is_still_restored_by_one_undo() {
    let (mut app, dir, child) = fixture();
    annotate(&mut app);
    let renamed = dir.join("z_renamed.snxsch");
    std::fs::rename(&child, &renamed).expect("rename");
    app.rekey_document(&child, &renamed);
    let baseline_moved = app
        .document_state
        .document_saves
        .baseline(&renamed)
        .is_some();
    let original = std::fs::read(&renamed).expect("renamed");

    undo(&mut app);

    let root = root_refs(&app, &dir);
    let dropped = !app.document_state.engines.contains_key(&renamed);
    let clean = !app.document_state.is_document_dirty(&renamed);
    let on_disk = std::fs::read(&renamed).expect("renamed");
    std::fs::remove_dir_all(&dir).ok();
    assert!(baseline_moved, "the disk baseline follows the rename");
    assert_eq!(root, ["R5"]);
    assert!(dropped && clean, "the undo reached the renamed sheet");
    assert_eq!(on_disk, original);
}
