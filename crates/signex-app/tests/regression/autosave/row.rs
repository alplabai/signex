//! #2141 — autosave and recovery for a Component Preview row.
//!
//! A real `.snxlib` and the real recovery prompt: a row edited by one run is
//! offered, restored or discarded by the next, with the first run's editor
//! gone as after a crash.

use std::path::{Path, PathBuf};

use signex_app::app::{Message, Signex};
use signex_app::autosave::{AutosaveMsg, RecoveryChoice, sidecar};
use signex_app::library::messages::LibraryMessage;
use signex_app::library::state::EditorAddress;
use signex_library::{InternalPn, RowId};

use super::complete;
use crate::library_row_base_local::{TABLE, library_with_a_measurement_row, save_preview};

const EDITED: &str = "SNX-EDITED";

struct Fixture {
    app: Signex,
    snxlib: PathBuf,
    row_id: RowId,
    address: EditorAddress,
    recovery: PathBuf,
    _tmp: tempfile::TempDir,
}

fn fresh_run(snxlib: &Path, recovery: &Path, row_id: RowId) -> (Signex, EditorAddress) {
    let (mut app, _boot) = Signex::new();
    app.document_state
        .autosave
        .set_dir(Some(recovery.to_path_buf()));
    app.ui_state.autosave_enabled = true;
    app.library
        .open_library(snxlib.to_path_buf())
        .expect("mount library");
    let task = app.update(Message::Library(LibraryMessage::OpenComponentRow {
        library_path: snxlib.to_path_buf(),
        table: TABLE.to_string(),
        row_id,
    }));
    complete(&mut app, task);
    let address = EditorAddress::new(snxlib.to_path_buf(), TABLE.to_string(), row_id);
    assert!(app.library.editors.contains_key(&address), "row open");
    (app, address)
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let (snxlib, row_id) = library_with_a_measurement_row(tmp.path());
    let recovery = tmp.path().join("recovery");
    let (app, address) = fresh_run(&snxlib, &recovery, row_id);
    Fixture {
        app,
        snxlib,
        row_id,
        address,
        recovery,
        _tmp: tmp,
    }
}

fn pn_in_editor(app: &Signex, address: &EditorAddress) -> String {
    app.library.editors[address].row.internal_pn.to_string()
}

/// Change the open row and mark it dirty, as an inline edit would.
fn edit(app: &mut Signex, address: &EditorAddress) {
    let state = app.library.editors.get_mut(address).unwrap();
    state.row.internal_pn = InternalPn::new(EDITED);
    state.dirty = true;
}

fn tick(app: &mut Signex) {
    let task = app.update(Message::Autosave(AutosaveMsg::Ticked));
    complete(app, task);
}

fn answer(app: &mut Signex, choice: RecoveryChoice) {
    let task = app.update(Message::Autosave(AutosaveMsg::Recovery(choice)));
    complete(app, task);
}

fn sidecar_of(f: &Fixture) -> PathBuf {
    sidecar::sidecar_path(&f.recovery, &f.address.synthetic_tab_path())
}

/// The first run edited and autosaved; a second run then opens the row.
fn next_run() -> (Fixture, Signex) {
    let mut f = fixture();
    let address = f.address.clone();
    edit(&mut f.app, &address);
    tick(&mut f.app);
    assert!(sidecar_of(&f).exists(), "the tick wrote a recovery copy");
    let (next, _) = fresh_run(&f.snxlib, &f.recovery, f.row_id);
    (f, next)
}

#[test]
fn a_dirty_row_gets_a_sidecar_and_the_library_is_untouched() {
    let mut f = fixture();
    let address = f.address.clone();
    edit(&mut f.app, &address);
    tick(&mut f.app);
    assert!(sidecar_of(&f).exists());
    let (fresh, _) = fresh_run(&f.snxlib, &f.recovery.join("other"), f.row_id);
    let stored = fresh.library.editors[&f.address]
        .row
        .internal_pn
        .to_string();
    assert_ne!(stored, EDITED, "a copy is not a save");
}

#[test]
fn a_clean_row_gets_no_sidecar() {
    let mut f = fixture();
    tick(&mut f.app);
    assert!(!sidecar_of(&f).exists());
}

#[test]
fn autosave_off_writes_no_row_sidecar() {
    let mut f = fixture();
    f.app.ui_state.autosave_enabled = false;
    let address = f.address.clone();
    edit(&mut f.app, &address);
    tick(&mut f.app);
    assert!(!sidecar_of(&f).exists());
}

#[test]
fn opening_a_row_with_an_earlier_copy_offers_it() {
    let (f, next) = next_run();
    let offer = next.document_state.autosave.current_offer().expect("offer");
    assert_eq!(offer.path, f.address.synthetic_tab_path());
    assert!(sidecar_of(&f).exists(), "offering deletes nothing");
    assert!(!next.library.editors[&f.address].dirty);
}

#[test]
fn restore_brings_back_the_row_draft_as_unsaved() {
    let (f, mut next) = next_run();
    assert_ne!(pn_in_editor(&next, &f.address), EDITED);
    answer(&mut next, RecoveryChoice::Restore);

    assert_eq!(pn_in_editor(&next, &f.address), EDITED);
    assert!(next.library.editors[&f.address].dirty);
    assert!(sidecar_of(&f).exists(), "the copy stays until a save");
}

#[test]
fn saving_a_restored_row_removes_the_sidecar() {
    let (f, mut next) = next_run();
    answer(&mut next, RecoveryChoice::Restore);
    next.ui_state.autosave_enabled = false;
    save_preview(&mut next, &f.address);

    assert!(!next.library.editors[&f.address].dirty);
    assert!(!sidecar_of(&f).exists(), "a save drops the recovery copy");
}

#[test]
fn discard_deletes_the_copy_and_leaves_the_row_clean() {
    let (f, mut next) = next_run();
    answer(&mut next, RecoveryChoice::Discard);
    assert!(!sidecar_of(&f).exists());
    assert!(!next.library.editors[&f.address].dirty);
    assert_ne!(pn_in_editor(&next, &f.address), EDITED);
}

#[test]
fn a_copy_equal_to_the_stored_row_is_dropped_not_offered() {
    let f = fixture();
    // A copy of the row as it is stored (the draft was reverted by hand):
    // not news, so the next run neither offers nor keeps it.
    let bytes = serde_json::to_vec_pretty(&f.app.library.editors[&f.address].row).unwrap();
    sidecar::write(&f.recovery, &f.address.synthetic_tab_path(), &bytes).unwrap();

    let (next, _) = fresh_run(&f.snxlib, &f.recovery, f.row_id);
    assert!(next.document_state.autosave.current_offer().is_none());
    assert!(!sidecar_of(&f).exists());
}
