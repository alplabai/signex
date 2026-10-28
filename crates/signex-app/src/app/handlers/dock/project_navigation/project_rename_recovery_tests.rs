//! #1510 — a recovery copy whose move after a project rename keeps failing
//! is finished by the next run, and a copy keyed by another spelling of the
//! old path (dot segments, case on a case-insensitive volume) is still
//! carried.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::project_rename_fs::carry_closed_copies;
use super::project_rename_tests::{fixture, run_task, submit_rename};
use crate::app::state::CaseFold;
use crate::autosave::{recovery, sidecar};

/// Whether the copy of `document` is offered when it opens in a fresh run.
fn offered_after_restart(recovery_dir: &Path, document: &Path) -> bool {
    matches!(
        recovery::reconcile(recovery_dir, document, SystemTime::now()),
        Ok(Some(_))
    )
}

/// A closed folder `Foo/sub/b.snxsch` moved to `Bar/sub/b.snxsch` on disk.
struct Moved {
    recovery: PathBuf,
    old_doc: PathBuf,
    new_doc: PathBuf,
    old_dir: PathBuf,
    new_dir: PathBuf,
}

fn moved(root: &Path) -> Moved {
    let (old_dir, new_dir) = (root.join("Foo"), root.join("Bar"));
    let new_doc = new_dir.join("sub").join("b.snxsch");
    std::fs::create_dir_all(new_doc.parent().expect("sub")).expect("sub");
    std::fs::write(&new_doc, b"saved sheet").expect("sheet");
    Moved {
        recovery: root.join("recovery"),
        old_doc: old_dir.join("sub").join("b.snxsch"),
        new_doc,
        old_dir,
        new_dir,
    }
}

fn carry(m: &Moved, fold: CaseFold) {
    carry_closed_copies(
        &m.recovery,
        &m.old_dir,
        &m.new_dir,
        &Default::default(),
        fold,
    );
}

/// The move `carry_closed_copies` makes for a copy, failing: a directory sits
/// on the new key, which a rename onto it cannot replace.
fn fail_move(m: &Moved) {
    sidecar::rename(&m.recovery, &m.old_doc, &m.new_doc).expect_err("the move fails");
}

#[test]
fn a_copy_whose_move_failed_is_offered_after_a_restart() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    sidecar::write(&m.recovery, &m.old_doc, b"crash edits").expect("copy");
    let blocked = sidecar::sidecar_path(&m.recovery, &m.new_doc);
    std::fs::create_dir_all(blocked.join("blocker")).expect("block");

    fail_move(&m);
    assert!(sidecar::sidecar_path(&m.recovery, &m.old_doc).is_file());

    // The app quits; the blocker goes away; the next run opens the sheet.
    std::fs::remove_dir_all(&blocked).expect("unblock");
    assert!(offered_after_restart(&m.recovery, &m.new_doc));
    assert_eq!(std::fs::read(&blocked).expect("moved"), b"crash edits");
    assert!(!sidecar::sidecar_path(&m.recovery, &m.old_doc).exists());
}

#[test]
fn the_startup_sweep_finishes_a_move_before_any_document_opens() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    sidecar::write(&m.recovery, &m.old_doc, b"crash edits").expect("copy");
    let blocked = sidecar::sidecar_path(&m.recovery, &m.new_doc);
    std::fs::create_dir_all(blocked.join("blocker")).expect("block");
    fail_move(&m);
    std::fs::remove_dir_all(&blocked).expect("unblock");

    let done = crate::autosave::pending_move::finish_all(&m.recovery);
    assert_eq!(done.moved, 1);
    assert_eq!(std::fs::read(&blocked).expect("moved"), b"crash edits");
    assert_eq!(
        crate::autosave::pending_move::finish_all(&m.recovery),
        Default::default(),
        "the entry is cleared"
    );
}

#[test]
fn a_move_that_keeps_failing_never_deletes_the_copy() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    sidecar::write(&m.recovery, &m.old_doc, b"crash edits").expect("copy");
    let blocked = sidecar::sidecar_path(&m.recovery, &m.new_doc);
    std::fs::create_dir_all(blocked.join("blocker")).expect("block");
    fail_move(&m);

    let done = crate::autosave::pending_move::finish_all(&m.recovery);
    assert_eq!(done.failed, 1);
    assert_eq!(
        std::fs::read(sidecar::sidecar_path(&m.recovery, &m.old_doc)).expect("kept"),
        b"crash edits"
    );
}

#[test]
fn a_failed_open_document_move_is_offered_after_a_restart() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let recovery_dir = root.path().join("recovery");
    fx.app
        .document_state
        .autosave
        .set_dir(Some(recovery_dir.clone()));
    let new_sheet = root.path().join("Bar").join("main.snxsch");
    sidecar::write(&recovery_dir, &fx.sheet, b"unsaved").expect("sidecar");
    let new_copy = sidecar::sidecar_path(&recovery_dir, &new_sheet);
    std::fs::create_dir_all(new_copy.join("blocker")).expect("block");

    let file = fx.folder.join("Foo.snxprj");
    let task = submit_rename(&mut fx.app, &file, "Bar");
    run_task(&mut fx.app, task);
    assert!(new_sheet.is_file(), "the folder moved");

    // Quit with the move still failing, then restart after it clears.
    drop(fx);
    std::fs::remove_dir_all(&new_copy).expect("unblock");
    assert!(offered_after_restart(&recovery_dir, &new_sheet));
}

#[test]
fn a_copy_keyed_by_a_dot_segment_spelling_is_carried() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    let spelled = root.path().join("x").join("..").join("Foo/sub/b.snxsch");
    sidecar::write(&m.recovery, &spelled, b"crash edits").expect("copy");

    carry(&m, CaseFold::PRESERVE);
    assert!(offered_after_restart(&m.recovery, &m.new_doc));
}

#[test]
fn a_copy_keyed_by_another_case_spelling_is_carried_on_a_case_insensitive_volume() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    // Written when the path was spelled in lower case: one file on a
    // case-insensitive volume, two keys.
    let lower = PathBuf::from(m.old_doc.to_string_lossy().to_lowercase());
    assert_ne!(lower, m.old_doc);
    sidecar::write(&m.recovery, &lower, b"crash edits").expect("copy");

    carry(&m, CaseFold::FOLD);
    assert!(offered_after_restart(&m.recovery, &m.new_doc));
}

#[test]
fn another_case_spelling_is_not_taken_for_this_file_on_a_case_sensitive_volume() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    let lower = PathBuf::from(m.old_doc.to_string_lossy().to_lowercase());
    sidecar::write(&m.recovery, &lower, b"another file's edits").expect("copy");

    carry(&m, CaseFold::PRESERVE);
    assert!(!offered_after_restart(&m.recovery, &m.new_doc));
    assert!(sidecar::sidecar_path(&m.recovery, &lower).is_file());
}
