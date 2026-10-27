//! #1185 — autosave and recovery for PCB tabs.
//!
//! Real files, the real sidecar writer and the real recovery prompt: a board
//! autosaved by one run is offered, restored or discarded by the next.

use std::path::{Path, PathBuf};

use signex_app::app::{FileMsg, Message, Signex, TabDocument};
use signex_app::autosave::{AutosaveMsg, RecoveryChoice, sidecar};
use signex_types::pcb::PcbBoard;

use super::complete;

const ORIGINAL_THICKNESS: f64 = 1.6;
const EDITED_THICKNESS: f64 = 2.4;

fn board(thickness: f64) -> PcbBoard {
    PcbBoard {
        unknown: Default::default(),
        uuid: uuid::Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    }
}

struct Fixture {
    app: Signex,
    doc: PathBuf,
    recovery: PathBuf,
    _tmp: tempfile::TempDir,
}

fn write_original(doc: &Path) {
    let text = signex_types::format::SnxPcb::new(board(ORIGINAL_THICKNESS))
        .write_string()
        .unwrap();
    std::fs::write(doc, text).unwrap();
}

fn fresh_run(recovery: &Path, doc: &Path) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.document_state
        .autosave
        .set_dir(Some(recovery.to_path_buf()));
    app.ui_state.autosave_enabled = true;
    let task = app.update(Message::File(FileMsg::Opened(Some(doc.to_path_buf()))));
    complete(&mut app, task);
    app
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let doc = tmp.path().join("board.snxpcb");
    let recovery = tmp.path().join("recovery");
    write_original(&doc);
    let app = fresh_run(&recovery, &doc);
    assert_eq!(thickness_in_tab(&app, &doc), ORIGINAL_THICKNESS);
    Fixture {
        app,
        doc,
        recovery,
        _tmp: tmp,
    }
}

fn thickness_in_tab(app: &Signex, doc: &Path) -> f64 {
    app.document_state
        .tabs
        .iter()
        .find(|tab| tab.path == doc)
        .and_then(|tab| tab.cached_document.as_ref())
        .and_then(TabDocument::as_pcb)
        .expect("an open PCB tab")
        .thickness
}

fn is_dirty(app: &Signex, doc: &Path) -> bool {
    app.document_state
        .tabs
        .iter()
        .find(|tab| tab.path == doc)
        .is_some_and(|tab| tab.dirty)
}

/// Change the open board and mark its tab dirty, as an edit would.
fn edit(app: &mut Signex, doc: &Path) {
    let tab = app
        .document_state
        .tabs
        .iter_mut()
        .find(|tab| tab.path == doc)
        .unwrap();
    tab.cached_document = Some(TabDocument::Pcb(board(EDITED_THICKNESS)));
    tab.dirty = true;
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
    sidecar::sidecar_path(&f.recovery, &f.doc)
}

/// The first run edited and autosaved; a second run then opens the file.
fn next_run() -> (Fixture, Signex, Vec<u8>) {
    let mut f = fixture();
    edit(&mut f.app, &f.doc.clone());
    tick(&mut f.app);
    assert!(sidecar_of(&f).exists(), "the tick wrote a recovery copy");
    let on_disk = std::fs::read(&f.doc).unwrap();
    let next = fresh_run(&f.recovery, &f.doc);
    (f, next, on_disk)
}

#[test]
fn a_dirty_pcb_gets_a_sidecar_and_the_real_file_is_untouched() {
    let mut f = fixture();
    let before = std::fs::read(&f.doc).unwrap();
    edit(&mut f.app, &f.doc.clone());
    tick(&mut f.app);
    assert!(sidecar_of(&f).exists());
    assert_eq!(std::fs::read(&f.doc).unwrap(), before);
    assert!(is_dirty(&f.app, &f.doc), "a copy is not a save");
}

#[test]
fn a_clean_pcb_gets_no_sidecar() {
    let mut f = fixture();
    tick(&mut f.app);
    assert!(!sidecar_of(&f).exists());
}

#[test]
fn autosave_off_writes_no_pcb_sidecar() {
    let mut f = fixture();
    f.app.ui_state.autosave_enabled = false;
    edit(&mut f.app, &f.doc.clone());
    tick(&mut f.app);
    assert!(!sidecar_of(&f).exists());
}

#[test]
fn saving_the_pcb_removes_the_sidecar() {
    let mut f = fixture();
    edit(&mut f.app, &f.doc.clone());
    tick(&mut f.app);
    assert!(sidecar_of(&f).exists());

    let task = f.app.update(Message::File(FileMsg::Save));
    complete(&mut f.app, task);

    assert!(!is_dirty(&f.app, &f.doc));
    assert!(!sidecar_of(&f).exists(), "a save drops the recovery copy");
}

#[test]
fn opening_a_pcb_with_an_earlier_copy_offers_it() {
    let (f, next, _) = next_run();
    let offer = next.document_state.autosave.current_offer().expect("offer");
    assert_eq!(offer.path, f.doc);
    assert!(sidecar_of(&f).exists(), "offering deletes nothing");
}

#[test]
fn restore_loads_the_copy_as_unsaved_edits_and_never_writes_the_file() {
    let (f, mut next, on_disk) = next_run();
    assert_eq!(thickness_in_tab(&next, &f.doc), ORIGINAL_THICKNESS);
    answer(&mut next, RecoveryChoice::Restore);

    assert_eq!(thickness_in_tab(&next, &f.doc), EDITED_THICKNESS);
    assert!(is_dirty(&next, &f.doc), "restored content is unsaved");
    assert!(next.document_state.dirty_paths.contains(&f.doc));
    assert_eq!(std::fs::read(&f.doc).unwrap(), on_disk);
    assert!(sidecar_of(&f).exists(), "the copy stays until a save");

    let task = next.update(Message::File(FileMsg::Save));
    complete(&mut next, task);
    assert!(!sidecar_of(&f).exists());
    assert_ne!(std::fs::read(&f.doc).unwrap(), on_disk);
}

#[test]
fn discard_deletes_the_copy_and_leaves_the_pcb_clean() {
    let (f, mut next, on_disk) = next_run();
    answer(&mut next, RecoveryChoice::Discard);
    assert!(!sidecar_of(&f).exists());
    assert!(!is_dirty(&next, &f.doc));
    assert_eq!(thickness_in_tab(&next, &f.doc), ORIGINAL_THICKNESS);
    assert_eq!(std::fs::read(&f.doc).unwrap(), on_disk);
}

#[test]
fn decide_later_keeps_the_copy_and_autosave_does_not_overwrite_it() {
    let (f, mut next, _) = next_run();
    let copy = std::fs::read(sidecar_of(&f)).unwrap();
    answer(&mut next, RecoveryChoice::Later);
    let tab = next
        .document_state
        .tabs
        .iter_mut()
        .find(|tab| tab.path == f.doc)
        .unwrap();
    tab.cached_document = Some(TabDocument::Pcb(board(3.2)));
    tab.dirty = true;
    tick(&mut next);
    assert_eq!(std::fs::read(sidecar_of(&f)).unwrap(), copy);
}

#[test]
fn a_copy_that_is_not_a_readable_pcb_is_refused_and_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = tmp.path().join("board.snxpcb");
    let recovery = tmp.path().join("recovery");
    write_original(&doc);
    sidecar::write(&recovery, &doc, b"not a board").unwrap();

    let mut app = fresh_run(&recovery, &doc);
    assert!(app.document_state.autosave.current_offer().is_some());
    answer(&mut app, RecoveryChoice::Restore);
    assert!(app.document_state.error_notice.is_some());
    assert!(sidecar::sidecar_path(&recovery, &doc).exists());
    assert_eq!(thickness_in_tab(&app, &doc), ORIGINAL_THICKNESS);
}

#[test]
fn a_tick_during_an_in_flight_save_that_then_fails_keeps_the_sidecar() {
    let mut f = fixture();
    edit(&mut f.app, &f.doc.clone());
    tick(&mut f.app);
    assert!(sidecar_of(&f).exists());

    let save = f.app.update(Message::File(FileMsg::Save));
    tick(&mut f.app);
    assert!(sidecar_of(&f).exists(), "a save in flight is not a save");

    // The write fails: a directory sits where the file belongs.
    std::fs::remove_file(&f.doc).unwrap();
    std::fs::create_dir(&f.doc).unwrap();
    for message in super::run(save) {
        let task = f.app.update(message);
        complete(&mut f.app, task);
    }

    assert!(is_dirty(&f.app, &f.doc), "the failed save leaves it dirty");
    tick(&mut f.app);
    assert!(sidecar_of(&f).exists(), "the recovery copy survives");
}
