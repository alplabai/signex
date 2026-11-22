//! m15 autosave recovery prompt.

use std::path::PathBuf;

use signex_app::app::{EditMsg, FileMsg, Message, Signex};
use signex_app::autosave::{AutosaveMsg, DocStamp, RecoveryChoice, sidecar};
use tempfile::TempDir;

use super::support::{app, esc, is_dirty, open, paper, sheet_text};
use crate::settle_support::settle;

struct Crash {
    app: Signex,
    doc: PathBuf,
    recovery: PathBuf,
    saved: String,
    _dir: TempDir,
}

impl Crash {
    fn copy(&self) -> PathBuf {
        sidecar::sidecar_path(&self.recovery, &self.doc)
    }

    fn file(&self) -> String {
        std::fs::read_to_string(&self.doc).expect("read the file")
    }
}

/// A sheet on A4 opened while a recovery copy holding `copy` waits for it.
fn opened_with_copy(copy: &[u8]) -> Crash {
    let dir = TempDir::new().expect("tempdir");
    let doc = dir.path().join("power.snxsch");
    let recovery = dir.path().join("recovery");
    let saved = sheet_text("A4");
    std::fs::write(&doc, &saved).expect("write the sheet");
    sidecar::write(&recovery, &doc, copy).expect("write the recovery copy");
    let mut app = app();
    app.document_state.autosave.set_dir(Some(recovery.clone()));
    open(&mut app, &doc);
    Crash {
        app,
        doc,
        recovery,
        saved,
        _dir: dir,
    }
}

fn opened_with_a3_copy() -> Crash {
    opened_with_copy(sheet_text("A3").as_bytes())
}

fn answer(app: &mut Signex, choice: RecoveryChoice) {
    settle(app, Message::Autosave(AutosaveMsg::Recovery(choice)));
}

#[test]
fn m15_a_copy_that_differs_from_the_file_is_offered_and_nothing_is_loaded() {
    let crash = opened_with_a3_copy();
    let offer = crash
        .app
        .document_state
        .autosave
        .current_offer()
        .expect("the prompt");
    assert_eq!(offer.path, crash.doc);
    assert_eq!(paper(&crash.app, &crash.doc), "A4");
    assert!(!is_dirty(&crash.app, &crash.doc));
    assert!(crash.copy().exists());
}

#[test]
fn m15_restore_loads_the_copy_as_unsaved_edits_and_writes_nothing() {
    let mut crash = opened_with_a3_copy();
    answer(&mut crash.app, RecoveryChoice::Restore);

    assert!(crash.app.document_state.autosave.current_offer().is_none());
    assert_eq!(paper(&crash.app, &crash.doc), "A3");
    assert!(
        is_dirty(&crash.app, &crash.doc),
        "restored edits are unsaved"
    );
    assert_eq!(crash.file(), crash.saved, "nothing written until a save");
}

/// Row m15: "Restoring replaces the open document and cannot be undone with
/// Ctrl+Z."
#[test]
fn m15_undo_does_not_take_a_restore_back() {
    let mut crash = opened_with_a3_copy();
    answer(&mut crash.app, RecoveryChoice::Restore);

    settle(&mut crash.app, Message::Edit(EditMsg::Undo));

    assert_eq!(paper(&crash.app, &crash.doc), "A3");
    assert!(is_dirty(&crash.app, &crash.doc));
}

#[test]
fn m15_saving_a_restored_document_writes_it_and_removes_the_copy() {
    let mut crash = opened_with_a3_copy();
    answer(&mut crash.app, RecoveryChoice::Restore);

    settle(&mut crash.app, Message::File(FileMsg::Save));

    assert_ne!(crash.file(), crash.saved);
    assert!(!is_dirty(&crash.app, &crash.doc));
    assert!(!crash.copy().exists());
}

#[test]
fn m15_discard_deletes_the_copy_and_leaves_the_document_as_saved() {
    let mut crash = opened_with_a3_copy();
    answer(&mut crash.app, RecoveryChoice::Discard);

    assert!(crash.app.document_state.autosave.current_offer().is_none());
    assert!(!crash.copy().exists());
    assert_eq!(paper(&crash.app, &crash.doc), "A4");
    assert!(!is_dirty(&crash.app, &crash.doc));
    assert_eq!(crash.file(), crash.saved);
}

#[test]
fn m15_decide_later_keeps_the_copy_and_pauses_autosave_for_the_document() {
    let mut crash = opened_with_a3_copy();
    answer(&mut crash.app, RecoveryChoice::Later);

    assert!(crash.app.document_state.autosave.current_offer().is_none());
    assert!(crash.copy().exists());
    assert_eq!(paper(&crash.app, &crash.doc), "A4");
    assert!(!is_dirty(&crash.app, &crash.doc));
    assert!(
        !crash
            .app
            .document_state
            .autosave
            .needs_write(&crash.doc, DocStamp { revision: u64::MAX }),
        "autosave may replace the kept copy"
    );
}

#[test]
fn m15_a_copy_kept_for_later_is_offered_at_the_next_open() {
    let mut crash = opened_with_a3_copy();
    answer(&mut crash.app, RecoveryChoice::Later);

    let mut next = app();
    next.document_state
        .autosave
        .set_dir(Some(crash.recovery.clone()));
    open(&mut next, &crash.doc);

    let offer = next.document_state.autosave.current_offer();
    assert_eq!(
        offer.map(|offer| offer.path.clone()),
        Some(crash.doc.clone())
    );
}

#[test]
fn m15_esc_means_decide_later() {
    let mut crash = opened_with_a3_copy();

    esc(&mut crash.app);

    assert!(crash.app.document_state.autosave.current_offer().is_none());
    assert!(crash.copy().exists(), "Esc must not delete the copy");
    assert_eq!(paper(&crash.app, &crash.doc), "A4", "Esc must not load it");
}

#[test]
fn m15_a_copy_that_cannot_be_read_is_refused_and_the_document_stays() {
    let mut crash = opened_with_copy(b"this is not a sheet file");
    assert!(crash.app.document_state.autosave.current_offer().is_some());

    answer(&mut crash.app, RecoveryChoice::Restore);

    assert!(crash.app.document_state.error_notice.is_some(), "no notice");
    assert_eq!(paper(&crash.app, &crash.doc), "A4");
    assert!(!is_dirty(&crash.app, &crash.doc));
    assert_eq!(crash.file(), crash.saved);
}
