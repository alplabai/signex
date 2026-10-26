//! #104 — opt-in autosave of dirty schematics to a recovery sidecar.
//!
//! Off by default and silent while off; on, a tick writes a sidecar in the
//! recovery directory and never the real file; a normal save (or a close)
//! removes the sidecar, including one whose write was still running.

use std::path::{Path, PathBuf};

use futures::StreamExt;
use iced::Task;
use signex_app::app::{FileMsg, Message, Signex, TabInfo, TabKind};
use signex_app::autosave::{AutosaveMsg, sidecar};
use signex_types::schematic::SchematicSheet;

const ORIGINAL: &str = "original bytes the user saved\n";

struct Fixture {
    app: Signex,
    doc: PathBuf,
    recovery: PathBuf,
    _tmp: tempfile::TempDir,
}

fn fixture(autosave_on: bool) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let doc = tmp.path().join("autosave.snxsch");
    let recovery = tmp.path().join("recovery");
    std::fs::write(&doc, ORIGINAL).unwrap();
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let (mut app, _boot) = Signex::new();
    app.document_state
        .engines
        .insert(doc.clone(), signex_engine::Engine::new(sheet).unwrap());
    app.document_state.tabs.push(TabInfo {
        title: "autosave".to_string(),
        path: doc.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(doc.clone());
    app.document_state.dirty_paths.insert(doc.clone());
    app.document_state.autosave.set_dir(Some(recovery.clone()));
    app.ui_state.autosave_enabled = autosave_on;
    Fixture {
        app,
        doc,
        recovery,
        _tmp: tmp,
    }
}

fn run(task: Task<Message>) -> Vec<Message> {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

/// Run a task and feed every completion back into `update`, to a fixpoint.
fn complete(app: &mut Signex, task: Task<Message>) {
    let mut pending = run(task);
    while let Some(message) = pending.pop() {
        pending.extend(run(app.update(message)));
    }
}

fn sidecar_of(f: &Fixture) -> PathBuf {
    sidecar::sidecar_path(&f.recovery, &f.doc)
}

fn tick(f: &mut Fixture) -> Task<Message> {
    f.app.update(Message::Autosave(AutosaveMsg::Ticked))
}

fn recovery_dir_is_empty(dir: &Path) -> bool {
    !dir.exists() || std::fs::read_dir(dir).unwrap().next().is_none()
}

#[test]
fn autosave_is_off_by_default() {
    let (app, _boot) = Signex::new();
    assert!(!app.ui_state.autosave_enabled);
}

#[test]
fn autosave_off_writes_nothing() {
    let mut f = fixture(false);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(recovery_dir_is_empty(&f.recovery));
    assert_eq!(std::fs::read_to_string(&f.doc).unwrap(), ORIGINAL);
}

#[test]
fn autosave_on_writes_the_sidecar_and_never_the_real_file() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    assert!(
        recovery_dir_is_empty(&f.recovery),
        "update must not write inline"
    );
    complete(&mut f.app, task);

    let engine = f.app.document_state.engines.get(&f.doc).unwrap();
    let expected = engine
        .serialize_with_net_colors(&f.app.ui_state.wire_color_overrides)
        .unwrap();
    assert_eq!(std::fs::read_to_string(sidecar_of(&f)).unwrap(), expected);
    assert_eq!(std::fs::read_to_string(&f.doc).unwrap(), ORIGINAL);
    assert!(
        f.app.document_state.dirty_paths.contains(&f.doc),
        "a recovery copy is not a save"
    );
}

#[test]
fn a_clean_document_gets_no_sidecar() {
    let mut f = fixture(true);
    f.app.document_state.dirty_paths.clear();
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(recovery_dir_is_empty(&f.recovery));
}

#[test]
fn normal_save_removes_the_sidecar() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(sidecar_of(&f).exists());

    let task = f.app.update(Message::File(FileMsg::Save));
    complete(&mut f.app, task);

    assert!(!sidecar_of(&f).exists(), "save must drop the recovery copy");
    assert_ne!(std::fs::read_to_string(&f.doc).unwrap(), ORIGINAL);
}

#[test]
fn a_save_that_lands_while_the_sidecar_write_runs_leaves_no_sidecar() {
    let mut f = fixture(true);
    let write = tick(&mut f);
    let save = f.app.update(Message::File(FileMsg::Save));
    complete(&mut f.app, save);
    // The autosave write finishes after the save.
    complete(&mut f.app, write);
    assert!(
        !sidecar_of(&f).exists(),
        "a late write must not resurrect a stale copy"
    );
}

#[test]
fn closing_the_document_removes_the_sidecar_on_the_next_tick() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(sidecar_of(&f).exists());

    f.app.document_state.engines.remove(&f.doc);
    f.app.document_state.dirty_paths.remove(&f.doc);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(!sidecar_of(&f).exists());
}

#[test]
fn an_unchanged_document_is_not_rewritten() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    std::fs::remove_file(sidecar_of(&f)).unwrap();
    // Same revision: nothing is owed, so no new write.
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(!sidecar_of(&f).exists());
}

#[test]
fn a_failing_write_frees_the_slot_so_a_later_tick_can_write() {
    let mut f = fixture(true);
    // A file where the recovery directory should be makes every write fail.
    std::fs::write(&f.recovery, b"not a directory").unwrap();
    let task = tick(&mut f);
    complete(&mut f.app, task);
    std::fs::remove_file(&f.recovery).unwrap();
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(sidecar_of(&f).exists());
}
