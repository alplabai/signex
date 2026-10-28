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

/// Whether the copy of `document` is offered when it opens in a fresh run:
/// the startup pass finishes the journalled moves, then the document opens.
fn offered_after_restart(recovery_dir: &Path, document: &Path) -> bool {
    crate::autosave::pending_move::finish_all(recovery_dir);
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
    assert_eq!(done.failed.len(), 1);
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

/// Fixture with the sheet's copy written and the key of `Bar`'s sheet blocked
/// so the first move fails. Returns (recovery dir, blocked new copy).
fn failing_first_move(
    fx: &mut super::project_rename_tests::Fixture,
    root: &Path,
) -> (PathBuf, PathBuf) {
    let recovery_dir = root.join("recovery");
    fx.app
        .document_state
        .autosave
        .set_dir(Some(recovery_dir.clone()));
    sidecar::write(&recovery_dir, &fx.sheet, b"unsaved").expect("sidecar");
    let blocked = sidecar::sidecar_path(&recovery_dir, &root.join("Bar").join("main.snxsch"));
    std::fs::create_dir_all(blocked.join("blocker")).expect("block");
    (recovery_dir, blocked)
}

#[test]
fn renaming_back_after_a_failed_move_does_not_replay_the_stale_move() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let (recovery_dir, blocked) = failing_first_move(&mut fx, root.path());

    let task = submit_rename(&mut fx.app, &fx.folder.join("Foo.snxprj"), "Bar");
    run_task(&mut fx.app, task);
    assert!(root.path().join("Bar").is_dir(), "the folder moved");
    // The user renames it back; the owed move is dropped.
    let back = submit_rename(
        &mut fx.app,
        &root.path().join("Bar").join("Bar.snxprj"),
        "Foo",
    );
    run_task(&mut fx.app, back);
    let flush = fx.app.autosave_flush_moves();
    run_task(&mut fx.app, flush);
    assert!(fx.sheet.is_file(), "back at the original path");

    // Quit; the blocker is gone by the next start.
    drop(fx);
    std::fs::remove_dir_all(&blocked).expect("unblock");
    assert!(offered_after_restart(
        &recovery_dir,
        &root.path().join("Foo").join("main.snxsch")
    ));
    assert!(
        !blocked.exists(),
        "the live copy was not moved to the old name's key"
    );
}

#[test]
fn renaming_twice_while_the_move_fails_leaves_the_copy_at_the_last_name() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let (recovery_dir, blocked) = failing_first_move(&mut fx, root.path());

    let task = submit_rename(&mut fx.app, &fx.folder.join("Foo.snxprj"), "Bar");
    run_task(&mut fx.app, task);
    let again = submit_rename(
        &mut fx.app,
        &root.path().join("Bar").join("Bar.snxprj"),
        "Baz",
    );
    run_task(&mut fx.app, again);
    let flush = fx.app.autosave_flush_moves();
    run_task(&mut fx.app, flush);

    drop(fx);
    std::fs::remove_dir_all(&blocked).expect("unblock");
    let last = root.path().join("Baz").join("main.snxsch");
    assert!(offered_after_restart(&recovery_dir, &last));
    assert!(!blocked.exists(), "the stale first move was not replayed");
}

#[test]
fn a_copy_under_the_pre_1510_key_is_offered_and_carried() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    // A build before #1510 keyed the copy by the spelling, dots and all.
    // Built component by component, the way the pre-#1510 carry built `old`
    // (`old_dir.join(rel)`), so the raw text matches on every platform.
    let spelled = root
        .path()
        .join("x")
        .join("..")
        .join("Foo")
        .join("sub")
        .join("b.snxsch");
    std::fs::create_dir_all(&m.recovery).expect("recovery");
    let legacy = m.recovery.join(format!(
        "b.snxsch-{:016x}.recovery",
        sidecar::bytes_hash(spelled.to_string_lossy().as_bytes())
    ));
    std::fs::write(&legacy, b"crash edits").expect("legacy copy");

    // Opened under that spelling it is found and moved to the current key.
    // The document does not exist, so the copy cannot be judged (`Err`), but
    // the adoption comes first.
    assert!(recovery::reconcile(&m.recovery, &spelled, SystemTime::now()).is_err());
    assert!(sidecar::sidecar_path(&m.recovery, &spelled).is_file());
    assert!(!legacy.exists(), "adopted to the current key");

    // And a folder rename, told the old folder by that spelling, carries a
    // legacy-keyed copy of a closed sheet.
    std::fs::write(&legacy, b"crash edits").expect("legacy copy again");
    let _ = std::fs::remove_file(sidecar::sidecar_path(&m.recovery, &spelled));
    let dotted = Moved {
        old_dir: root.path().join("x").join("..").join("Foo"),
        ..m
    };
    carry(&dotted, CaseFold::PRESERVE);
    assert!(offered_after_restart(&dotted.recovery, &dotted.new_doc));
}

#[test]
fn an_unreadable_journal_entry_is_left_and_does_not_stop_the_pass() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    sidecar::write(&m.recovery, &m.old_doc, b"crash edits").expect("copy");
    let blocked = sidecar::sidecar_path(&m.recovery, &m.new_doc);
    std::fs::create_dir_all(blocked.join("blocker")).expect("block");
    fail_move(&m);
    std::fs::remove_dir_all(&blocked).expect("unblock");
    let junk = m.recovery.join("junk-0000000000000000.recovery.move");
    std::fs::write(&junk, b"not json").expect("junk");

    assert_eq!(
        crate::autosave::pending_move::finish_all(&m.recovery).moved,
        1
    );
    assert!(junk.is_file(), "left in place, not deleted");
}

/// A closed sheet whose copy sits at the key of `old_doc`, with the move to
/// `new_doc` journalled and failed, as a locked copy leaves it.
fn journalled_failed_move(m: &Moved) {
    sidecar::write(&m.recovery, &m.old_doc, b"crash edits").expect("copy");
    crate::autosave::pending_move::record(&m.recovery, &m.old_doc, &m.new_doc).expect("entry");
}

#[test]
fn a_closed_copy_is_not_moved_by_a_stale_entry_after_the_folder_came_back() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    journalled_failed_move(&m);
    // Bar goes back to Foo: the sheet is at its old path again.
    std::fs::create_dir_all(m.old_doc.parent().expect("sub")).expect("sub");
    std::fs::rename(&m.new_doc, &m.old_doc).expect("back");
    let (back_from, back_to) = (m.new_dir.clone(), m.old_dir.clone());
    carry_closed_copies(
        &m.recovery,
        &back_from,
        &back_to,
        &Default::default(),
        CaseFold::PRESERVE,
    );

    assert!(offered_after_restart(&m.recovery, &m.old_doc));
    assert!(!sidecar::sidecar_path(&m.recovery, &m.new_doc).exists());
}

#[test]
fn a_stale_entry_is_dropped_when_no_document_is_at_the_new_path() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    journalled_failed_move(&m);
    // Never carried back: the new path has no document (its folder is gone).
    std::fs::remove_file(&m.new_doc).expect("gone");
    std::fs::create_dir_all(m.old_doc.parent().expect("sub")).expect("sub");
    std::fs::write(&m.old_doc, b"saved sheet").expect("sheet at the old path");

    assert!(offered_after_restart(&m.recovery, &m.old_doc));
    assert!(!sidecar::sidecar_path(&m.recovery, &m.new_doc).exists());
}

#[test]
fn a_second_folder_rename_carries_a_copy_whose_first_move_is_still_owed() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    journalled_failed_move(&m);
    // Bar goes on to Baz.
    let baz = root.path().join("Baz");
    std::fs::rename(&m.new_dir, &baz).expect("Bar to Baz");
    let baz_doc = baz.join("sub").join("b.snxsch");
    carry_closed_copies(
        &m.recovery,
        &m.new_dir,
        &baz,
        &Default::default(),
        CaseFold::PRESERVE,
    );

    assert!(offered_after_restart(&m.recovery, &baz_doc));
    assert!(!sidecar::sidecar_path(&m.recovery, &m.new_doc).exists());
}

#[test]
fn the_folder_rename_journals_closed_copies_before_the_move() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    // Before the move the sheet is still under Foo.
    std::fs::create_dir_all(m.old_doc.parent().expect("sub")).expect("sub");
    std::fs::rename(&m.new_doc, &m.old_doc).expect("sheet back under Foo");
    sidecar::write(&m.recovery, &m.old_doc, b"crash edits").expect("copy");

    let journalled = super::project_rename_fs::journal_closed_copies(
        &m.recovery,
        &m.old_dir,
        &m.new_dir,
        &Default::default(),
        CaseFold::PRESERVE,
    );
    assert_eq!(journalled.len(), 1);
    // Crash after the folder moved but before the carry: startup finishes it.
    std::fs::remove_dir_all(&m.new_dir).expect("empty target");
    std::fs::rename(&m.old_dir, &m.new_dir).expect("folder moved");
    assert!(offered_after_restart(&m.recovery, &m.new_doc));
}

#[test]
fn a_failed_folder_move_restores_the_entry_it_replaced() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    // The copy of S (a document at the old Foo path) is owed to Bar.
    let s_doc = m.old_doc.clone();
    sidecar::write(&m.recovery, &s_doc, b"crash edits").expect("copy");
    crate::autosave::pending_move::record(&m.recovery, &s_doc, &m.new_doc).expect("entry");
    // Bar then starts moving to Baz: S's entry is rewritten to Baz.
    let baz = root.path().join("Baz");
    let journalled = super::project_rename_fs::journal_closed_copies(
        &m.recovery,
        &m.new_dir,
        &baz,
        &Default::default(),
        CaseFold::PRESERVE,
    );
    assert_eq!(journalled.len(), 1);
    assert_eq!(
        crate::autosave::pending_move::owed(&m.recovery),
        vec![(s_doc.clone(), baz.join("sub").join("b.snxsch"))]
    );

    // The folder move fails: S is owed to Bar again.
    super::project_rename_fs::unjournal_closed_copies(&m.recovery, &journalled);
    assert_eq!(
        crate::autosave::pending_move::owed(&m.recovery),
        vec![(s_doc, m.new_doc.clone())]
    );
}

#[test]
fn a_journal_clear_for_an_older_target_keeps_a_newer_entry() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    let newer = root.path().join("Baz").join("sub").join("b.snxsch");
    crate::autosave::pending_move::record(&m.recovery, &m.old_doc, &newer).expect("entry");
    crate::autosave::pending_move::clear_if(
        &m.recovery,
        &m.old_doc,
        std::slice::from_ref(&m.new_doc),
    );
    assert_eq!(crate::autosave::pending_move::owed(&m.recovery).len(), 1);
    crate::autosave::pending_move::clear_if(&m.recovery, &m.old_doc, std::slice::from_ref(&newer));
    assert!(crate::autosave::pending_move::owed(&m.recovery).is_empty());
}

#[test]
fn a_move_still_failing_at_startup_leaves_the_crash_copy_safe_while_the_new_document_is_edited() {
    let root = tempfile::tempdir().expect("tempdir");
    let m = moved(root.path());
    sidecar::write(&m.recovery, &m.old_doc, b"crash edits").expect("copy");
    let new_key = sidecar::sidecar_path(&m.recovery, &m.new_doc);
    std::fs::create_dir_all(new_key.join("blocker")).expect("block");
    crate::autosave::pending_move::record(&m.recovery, &m.old_doc, &m.new_doc).expect("entry");

    let (mut app, _boot) = crate::app::Signex::new();
    app.document_state
        .autosave
        .set_dir(Some(m.recovery.clone()));
    app.autosave_finish_pending_moves();
    assert!(
        !app.document_state.autosave.has_owed_moves(),
        "the live state does not take the move: it would land under a key the session writes"
    );

    // The lock clears; the user opens the document at the new path (nothing
    // to offer yet), edits it, and autosave writes its own copy.
    std::fs::remove_dir_all(&new_key).expect("unblock");
    let old_key = sidecar::sidecar_path(&m.recovery, &m.old_doc);
    assert!(!offered_without_startup(&m.recovery, &m.new_doc));
    let tick = app.autosave_flush_moves();
    run_task(&mut app, tick);
    sidecar::write(&m.recovery, &m.new_doc, b"new edits").expect("autosave write");
    assert_eq!(std::fs::read(&old_key).expect("untouched"), b"crash edits");

    // The edit is saved (its copy goes) and the app restarts: the move is
    // finished and the crash copy is offered.
    sidecar::remove(&m.recovery, &m.new_doc).expect("saved");
    assert!(offered_after_restart(&m.recovery, &m.new_doc));
    assert_eq!(std::fs::read(&new_key).expect("moved"), b"crash edits");
}

/// `reconcile` alone, as a document opens mid-session.
fn offered_without_startup(recovery_dir: &Path, document: &Path) -> bool {
    matches!(
        recovery::reconcile(recovery_dir, document, SystemTime::now()),
        Ok(Some(_))
    )
}

#[test]
fn a_host_where_backslash_separates_also_tries_the_forward_slash_sheet_spelling() {
    // A tab path is `dir.join("sub/b.snxsch")`: on Windows that text mixes
    // `\` and `/`, and a pre-#1510 build keyed the copy by it.
    let dir = Path::new("proj");
    let rel = Path::new("sub").join("b.snxsch");
    let alt = super::project_rename_fs::forward_slash_spelling(dir, &rel, true);
    assert_eq!(alt, dir.join("sub/b.snxsch"));
    assert_eq!(
        super::project_rename_fs::forward_slash_spelling(dir, Path::new("sub\\b.snxsch"), true),
        dir.join("sub/b.snxsch")
    );
}
