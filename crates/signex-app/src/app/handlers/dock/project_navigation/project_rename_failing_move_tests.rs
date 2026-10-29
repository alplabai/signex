//! #1433 — a recovery copy move that keeps failing neither freezes recovery
//! nor strands an obsolete copy: autosave writes the current content to the
//! new key, a restart finds it, and a save or quit settles the owed move.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::project_rename_tests::{Fixture, fixture, run_task, submit_rename};
use crate::app::Message;
use crate::autosave::{AutosaveMsg, DocStamp, pending_move, recovery, sidecar};

fn rename_foo_to_bar(fx: &mut Fixture) {
    let file = fx.folder.join("Foo.snxprj");
    let task = submit_rename(&mut fx.app, &file, "Bar");
    run_task(&mut fx.app, task);
}

/// An old copy written by this run, and a directory on the new key so the
/// rename `Foo` -> `Bar` cannot move it. Returns (recovery dir, old copy,
/// new copy, new sheet).
fn owed_move(fx: &mut Fixture, root: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let recovery = root.join("recovery");
    let autosave = &mut fx.app.document_state.autosave;
    autosave.set_dir(Some(recovery.clone()));
    let new_sheet = root.join("Bar").join("main.snxsch");
    sidecar::write(&recovery, &fx.sheet, b"stale edits").expect("old copy");
    autosave.begin(&fx.sheet);
    autosave.finish(&fx.sheet, DocStamp::of_bytes(b"stale edits"), true);
    let new_copy = sidecar::sidecar_path(&recovery, &new_sheet);
    std::fs::create_dir_all(new_copy.join("blocker")).expect("blocker");
    let old_copy = sidecar::sidecar_path(&recovery, &fx.sheet);
    (recovery, old_copy, new_copy, new_sheet)
}

#[test]
fn a_persistently_failing_move_still_leaves_a_current_copy_at_the_new_key() {
    let _ = crate::diagnostics::init_logging();
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let (recovery, old_copy, new_copy, new_sheet) = owed_move(&mut fx, root.path());

    rename_foo_to_bar(&mut fx);
    assert!(old_copy.is_file(), "the first attempt failed");
    let retry = fx.app.autosave_flush_moves();
    run_task(&mut fx.app, retry);
    assert!(old_copy.is_file(), "the second attempt failed");

    // The hold is given up: a write to the new key may go ahead. (The lock
    // that failed the renames is lifted for the write only; the owed move is
    // taken out of the tick so it stays owed.)
    std::fs::remove_dir_all(&new_copy).expect("unblock");
    drop(fx.app.document_state.autosave.take_moves());
    fx.app.ui_state.autosave_enabled = true;
    let tick = fx.app.update(Message::Autosave(AutosaveMsg::Ticked));
    run_task(&mut fx.app, tick);
    let fresh = std::fs::read(&new_copy).expect("a fresh copy at the new key");
    assert_ne!(fresh, b"stale edits", "the copy is current");
    assert_eq!(std::fs::read(&old_copy).unwrap(), b"stale edits");

    // Crash: a new run finishes the journal and opens the sheet.
    drop(fx);
    pending_move::finish_all(&recovery);
    let offered = recovery::reconcile(&recovery, &new_sheet, SystemTime::now()).expect("judged");
    assert!(offered.is_some(), "the copy is offered");
    assert_eq!(std::fs::read(&new_copy).unwrap(), fresh, "the newest wins");
}

#[test]
fn saving_while_a_move_is_owed_removes_the_old_copy_with_autosave_off() {
    let _ = crate::diagnostics::init_logging();
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let (recovery, old_copy, new_copy, new_sheet) = owed_move(&mut fx, root.path());
    rename_foo_to_bar(&mut fx);
    assert!(old_copy.is_file() && fx.app.document_state.autosave.has_owed_moves());
    std::fs::remove_dir_all(&new_copy).expect("unblock");

    // The preference is read from a prefs file other tests share.
    fx.app.ui_state.autosave_enabled = false;
    let saved = fx.app.autosave_after_save(&new_sheet);
    run_task(&mut fx.app, saved);
    assert!(!old_copy.exists(), "no obsolete copy outlives the save");
    assert!(!new_copy.exists());
    assert!(!fx.app.document_state.autosave.has_owed_moves());
    assert!(pending_move::owed(&recovery).is_empty(), "journal cleared");
}

#[test]
fn quitting_with_save_while_a_move_is_owed_leaves_no_old_copy() {
    let _ = crate::diagnostics::init_logging();
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let (recovery, old_copy, new_copy, new_sheet) = owed_move(&mut fx, root.path());
    rename_foo_to_bar(&mut fx);
    std::fs::remove_dir_all(&new_copy).expect("unblock");

    let saved = fx.app.autosave_after_save(&new_sheet);
    let quit = fx.app.wait_for_autosave_then(iced::Task::none());
    run_task(&mut fx.app, iced::Task::batch([saved, quit]));
    assert!(!old_copy.exists());
    assert!(pending_move::owed(&recovery).is_empty());
    assert!(
        fx.app
            .document_state
            .autosave
            .barrier()
            .wait_timeout(std::time::Duration::ZERO),
        "no autosave job is left running"
    );
}

/// A copy of `old_doc` is owed a move to `new_doc`, and only the old key holds it.
#[test]
fn reconcile_finds_a_copy_still_owed_a_move_into_the_document() {
    let _ = crate::diagnostics::init_logging();
    let root = tempfile::tempdir().expect("tempdir");
    let recovery = root.path().join("recovery");
    let (old_doc, new_doc) = (root.path().join("a.snxsch"), root.path().join("b.snxsch"));
    std::fs::write(&new_doc, b"saved").expect("document");
    sidecar::write(&recovery, &old_doc, b"unsaved edits").expect("copy");
    pending_move::record(&recovery, &old_doc, &new_doc).expect("journal");

    let offered = recovery::reconcile(&recovery, &new_doc, SystemTime::now()).expect("judged");
    assert!(offered.is_some());
    let key = sidecar::sidecar_path(&recovery, &new_doc);
    assert_eq!(std::fs::read(key).unwrap(), b"unsaved edits");
    assert!(!sidecar::sidecar_path(&recovery, &old_doc).exists());
}

#[test]
fn reconcile_names_the_stranded_copy_when_its_move_still_fails() {
    let _ = crate::diagnostics::init_logging();
    let root = tempfile::tempdir().expect("tempdir");
    let recovery = root.path().join("recovery");
    let (old_doc, new_doc) = (root.path().join("a.snxsch"), root.path().join("b.snxsch"));
    std::fs::write(&new_doc, b"saved").expect("document");
    sidecar::write(&recovery, &old_doc, b"unsaved edits").expect("copy");
    pending_move::record(&recovery, &old_doc, &new_doc).expect("journal");
    let blocked = sidecar::sidecar_path(&recovery, &new_doc);
    std::fs::create_dir_all(blocked.join("blocker")).expect("block");

    let error = recovery::reconcile(&recovery, &new_doc, SystemTime::now()).expect_err("kept");
    assert!(error.contains("a.snxsch"), "{error}");
    assert!(sidecar::sidecar_path(&recovery, &old_doc).is_file());
}
