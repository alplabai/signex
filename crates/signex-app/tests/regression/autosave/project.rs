//! #2141 — autosave and recovery for a loaded project (`.snxprj`).
//!
//! Real files, the real sidecar writer and the real recovery prompt: a
//! project edited by one run is offered, restored or discarded by the next.

use std::path::{Path, PathBuf};

use signex_app::app::{FileMsg, Message, Signex};
use signex_app::autosave::{AutosaveMsg, RecoveryChoice, sidecar};
use signex_types::project::{ProjectData, write_project};

use super::complete;

const ORIGINAL: &str = "the saved description";
const EDITED: &str = "typed, never saved";

fn project() -> ProjectData {
    ProjectData {
        document_id: None,
        description: Some(ORIGINAL.to_string()),
        name: "P".to_string(),
        dir: String::new(),
        schematic_root: None,
        pcb_file: None,
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

struct Fixture {
    app: Signex,
    /// The project path as the loaded project holds it.
    prj: PathBuf,
    recovery: PathBuf,
    _tmp: tempfile::TempDir,
}

fn fresh_run(recovery: &Path, prj: &Path) -> (Signex, PathBuf) {
    let (mut app, _boot) = Signex::new();
    app.document_state
        .autosave
        .set_dir(Some(recovery.to_path_buf()));
    app.ui_state.autosave_enabled = true;
    let task = app.update(Message::File(FileMsg::Opened(Some(prj.to_path_buf()))));
    complete(&mut app, task);
    let loaded = app
        .document_state
        .projects
        .first()
        .expect("the project loaded")
        .path
        .clone();
    (app, loaded)
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("P.snxprj");
    write_project(&file, &project()).unwrap();
    let recovery = tmp.path().join("recovery");
    let (app, prj) = fresh_run(&recovery, &file);
    Fixture {
        app,
        prj,
        recovery,
        _tmp: tmp,
    }
}

fn description(app: &Signex) -> Option<String> {
    app.document_state.projects[0].data.description.clone()
}

/// Change the loaded project and mark it modified, as an edit would.
fn edit(app: &mut Signex, prj: &Path) {
    app.document_state.projects[0].data.description = Some(EDITED.to_string());
    app.document_state.mark_document_dirty(prj);
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
    sidecar::sidecar_path(&f.recovery, &f.prj)
}

/// The first run edited and autosaved; a second run then opens the project.
fn next_run() -> (Fixture, Signex, Vec<u8>) {
    let mut f = fixture();
    let prj = f.prj.clone();
    edit(&mut f.app, &prj);
    tick(&mut f.app);
    assert!(sidecar_of(&f).exists(), "the tick wrote a recovery copy");
    let on_disk = std::fs::read(&f.prj).unwrap();
    let (next, _) = fresh_run(&f.recovery, &f.prj);
    (f, next, on_disk)
}

#[test]
fn a_dirty_project_gets_a_sidecar_and_the_real_file_is_untouched() {
    let mut f = fixture();
    let before = std::fs::read(&f.prj).unwrap();
    let prj = f.prj.clone();
    edit(&mut f.app, &prj);
    tick(&mut f.app);
    assert!(sidecar_of(&f).exists());
    assert_eq!(std::fs::read(&f.prj).unwrap(), before);
    assert!(f.app.document_state.is_document_dirty(&f.prj));
}

#[test]
fn a_clean_project_gets_no_sidecar() {
    let mut f = fixture();
    tick(&mut f.app);
    assert!(!sidecar_of(&f).exists());
}

#[test]
fn opening_a_project_with_an_earlier_copy_offers_it() {
    let (f, next, _) = next_run();
    let offer = next.document_state.autosave.current_offer().expect("offer");
    assert_eq!(offer.path, f.prj);
    assert!(sidecar_of(&f).exists(), "offering deletes nothing");
}

#[test]
fn restore_brings_back_the_edits_as_unsaved_and_never_writes_the_file() {
    let (f, mut next, on_disk) = next_run();
    assert_eq!(description(&next).as_deref(), Some(ORIGINAL));
    answer(&mut next, RecoveryChoice::Restore);

    assert_eq!(description(&next).as_deref(), Some(EDITED));
    assert!(next.document_state.is_document_dirty(&f.prj));
    assert_eq!(std::fs::read(&f.prj).unwrap(), on_disk);
    assert!(sidecar_of(&f).exists(), "the copy stays until a save");

    let task = next.update(Message::File(FileMsg::Save));
    complete(&mut next, task);
    assert!(!next.document_state.is_document_dirty(&f.prj));
    assert!(!sidecar_of(&f).exists(), "a save drops the copy");
    assert_ne!(std::fs::read(&f.prj).unwrap(), on_disk);
}

#[test]
fn discard_deletes_the_copy_and_leaves_the_project_clean() {
    let (f, mut next, on_disk) = next_run();
    answer(&mut next, RecoveryChoice::Discard);
    assert!(!sidecar_of(&f).exists());
    assert!(!next.document_state.is_document_dirty(&f.prj));
    assert_eq!(description(&next).as_deref(), Some(ORIGINAL));
    assert_eq!(std::fs::read(&f.prj).unwrap(), on_disk);
}

#[test]
fn saving_the_project_removes_the_sidecar_with_autosave_off() {
    let mut f = fixture();
    let prj = f.prj.clone();
    edit(&mut f.app, &prj);
    tick(&mut f.app);
    assert!(sidecar_of(&f).exists());

    f.app.ui_state.autosave_enabled = false;
    let task = f.app.update(Message::File(FileMsg::Save));
    complete(&mut f.app, task);

    assert!(!sidecar_of(&f).exists());
}

#[test]
fn autosave_off_writes_no_project_sidecar() {
    let mut f = fixture();
    f.app.ui_state.autosave_enabled = false;
    let prj = f.prj.clone();
    edit(&mut f.app, &prj);
    tick(&mut f.app);
    assert!(!sidecar_of(&f).exists());
}

/// #2172: a restore brings back the copy's content, not its identity.
#[test]
fn restore_keeps_the_id_of_the_open_project() {
    let (_f, mut next, _) = next_run();
    let live = signex_types::document_id::DocumentId::new();
    next.document_state.projects[0].data.document_id = Some(live);
    answer(&mut next, RecoveryChoice::Restore);
    assert_eq!(description(&next).as_deref(), Some(EDITED));
    assert_eq!(next.document_state.projects[0].data.document_id, Some(live));
}
