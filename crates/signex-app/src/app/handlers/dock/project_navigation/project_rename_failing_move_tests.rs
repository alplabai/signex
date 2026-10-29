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

/// Make a move onto `new_copy` fail for good while the key stays writable:
/// a copy already sits there and every name a displaced copy could be set
/// aside under is taken, so the newest-wins resolution has nowhere to put the
/// loser. (A lock on the source does the same on Windows; this works anywhere.)
fn jam_the_move_onto(new_copy: &Path) {
    std::fs::create_dir_all(new_copy.parent().unwrap()).expect("recovery dir");
    std::fs::write(new_copy, b"decoy").expect("copy at the new key");
    let mut base = new_copy.to_path_buf().into_os_string();
    base.push(".older");
    for n in 0..100u32 {
        let mut name = base.clone();
        if n > 0 {
            name.push(format!(".{n}"));
        }
        std::fs::write(PathBuf::from(name), b"taken").expect("claim an aside name");
    }
}

fn unjam(new_copy: &Path) {
    let dir = new_copy.parent().unwrap();
    let stem = new_copy.file_name().unwrap().to_string_lossy().into_owned();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        if entry.file_name().to_string_lossy().starts_with(&stem) {
            std::fs::remove_file(entry.path()).unwrap();
        }
    }
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
    std::fs::remove_dir_all(&new_copy).expect("no blocker");
    jam_the_move_onto(&new_copy);

    rename_foo_to_bar(&mut fx);
    assert!(old_copy.is_file(), "the first attempt failed");
    let retry = fx.app.autosave_flush_moves();
    run_task(&mut fx.app, retry);
    assert!(old_copy.is_file(), "the second attempt failed");

    // The hold is given up: the tick retries the move (it fails again) and
    // writes the current content to the new key, which stays writable.
    fx.app.ui_state.autosave_enabled = true;
    let tick = fx.app.update(Message::Autosave(AutosaveMsg::Ticked));
    run_task(&mut fx.app, tick);
    let fresh = std::fs::read(&new_copy).expect("a fresh copy at the new key");
    assert!(
        fresh != b"decoy" && fresh != b"stale edits",
        "the copy at the new key is current"
    );

    // Crash: a new run finishes the journal and opens the sheet.
    drop(fx);
    pending_move::finish_all(&recovery);
    let offered = recovery::reconcile(&recovery, &new_sheet, SystemTime::now()).expect("judged");
    assert!(offered.is_some(), "the current copy is offered");
    assert_eq!(std::fs::read(&new_copy).unwrap(), fresh, "the newest wins");
}

fn released_after_two_failures(from: &Path, to: &Path) -> crate::autosave::AutosaveState {
    let mut state = crate::autosave::AutosaveState::default();
    state.begin(from);
    state.finish(from, DocStamp::of_bytes(b"x"), true);
    state.rekey(from, to);
    for _ in 0..2 {
        let (mut start, _) = state.take_moves();
        assert!(state.move_failed(start.remove(0)).is_none());
    }
    state
}

#[test]
fn a_released_move_survives_a_second_rename_and_keeps_its_copy() {
    let (a, b, c) = (Path::new("a"), Path::new("b"), Path::new("c"));
    // No edit since the first rename: no fresh copy exists, so the old copy
    // is the only one and must follow the document to c, not be removed.
    let mut state = released_after_two_failures(a, b);
    state.rekey(b, c);
    let (start, obsolete) = state.take_moves();
    assert!(obsolete.is_empty(), "the only copy is not removed");
    let pairs: Vec<_> = start
        .iter()
        .map(|m| (m.old.as_path(), m.new.as_path()))
        .collect();
    assert!(
        pairs.contains(&(a, c)),
        "the old copy follows to c: {pairs:?}"
    );
}

#[test]
fn a_released_move_landing_after_the_next_rename_is_carried_on() {
    let (a, b, c) = (Path::new("a"), Path::new("b"), Path::new("c"));
    let mut state = released_after_two_failures(a, b);
    let (mut running, _) = state.take_moves();
    let running = running.remove(0);
    state.rekey(b, c);
    let (start, _) = state.take_moves();
    assert!(
        start.iter().all(|m| m.old != b),
        "b -> c waits for a -> b to end"
    );
    state.move_landed(&running);
    let (start, _) = state.take_moves();
    assert!(
        start.iter().any(|m| m.old == b && m.new == c),
        "then it carries the copy on"
    );
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
fn a_move_that_still_fails_when_the_document_opens_neither_pauses_autosave_nor_loses_the_copy() {
    let _ = crate::diagnostics::init_logging();
    let root = tempfile::tempdir().expect("tempdir");
    let recovery = root.path().join("recovery");
    let (old_doc, new_doc) = (root.path().join("a.snxsch"), root.path().join("b.snxsch"));
    std::fs::write(&new_doc, b"saved").expect("document");
    let new_key = sidecar::sidecar_path(&recovery, &new_doc);
    jam_the_move_onto(&new_key);
    sidecar::write(&recovery, &old_doc, b"crash edits").expect("copy");
    pending_move::record(&recovery, &old_doc, &new_doc).expect("journal");

    // Not an error: an error would pause autosave for the whole session. The
    // copy at the new key is judged as usual and the crash copy stays put.
    let judged = recovery::reconcile(&recovery, &new_doc, SystemTime::now());
    assert!(matches!(judged, Ok(Some(_))), "{judged:?}");
    let old_key = sidecar::sidecar_path(&recovery, &old_doc);
    assert_eq!(std::fs::read(&old_key).unwrap(), b"crash edits");

    // The edits made meanwhile are saved and the copy goes; the jam clears and
    // the next start finishes the move: the crash copy is offered.
    std::fs::remove_file(&new_key).unwrap();
    unjam(&new_key);
    assert!(
        pending_move::finish_all(&recovery).failed.is_empty(),
        "the move can finish now"
    );
    let offered = recovery::reconcile(&recovery, &new_doc, SystemTime::now()).expect("judged");
    assert!(offered.is_some());
    assert_eq!(std::fs::read(&new_key).unwrap(), b"crash edits");
}

#[test]
fn an_open_does_not_finish_a_move_the_live_state_owns() {
    let _ = crate::diagnostics::init_logging();
    let root = tempfile::tempdir().expect("tempdir");
    let recovery = root.path().join("recovery");
    let (old_doc, new_doc) = (root.path().join("a.snxsch"), root.path().join("b.snxsch"));
    std::fs::write(&new_doc, b"saved").expect("document");
    sidecar::write(&recovery, &old_doc, b"unsaved edits").expect("copy");
    pending_move::record(&recovery, &old_doc, &new_doc).expect("journal");

    let judged = recovery::reconcile_with(&recovery, &new_doc, SystemTime::now(), false);
    assert!(matches!(judged, Ok(None)), "{judged:?}");
    assert!(sidecar::sidecar_path(&recovery, &old_doc).is_file());
    assert_eq!(pending_move::owed(&recovery).len(), 1, "the entry stays");
}
