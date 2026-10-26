//! #1112 — the write-ordering ledger keys a document by its volume's fold
//! key, so two spellings of one file on a case-insensitive volume are one
//! file to the ledger. Unit tests because `CaseFold` is crate-private, so an
//! integration test cannot build a folding project.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures::StreamExt;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::scope::CaseFold;
use crate::app::{Message, Signex};
use crate::persist::{DocumentSaves, DocumentSink};

/// Records every write the sink is asked to perform, in order.
#[derive(Default)]
struct Recorder(Mutex<Vec<(PathBuf, Vec<u8>)>>);

impl DocumentSink for Recorder {
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.0
            .lock()
            .expect("recorder")
            .push((path.to_path_buf(), bytes.to_vec()));
        Ok(())
    }

    /// The simulated volume ignores case: the spellings are one file.
    fn same_file(&self, _a: &Path, _b: &Path) -> crate::persist::SameFile {
        crate::persist::SameFile::Same
    }
}

fn engine_at(path: &Path, paper: &str) -> signex_engine::Engine {
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: paper.to_string(),
        ..Default::default()
    };
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    engine.set_path(Some(path.to_path_buf()));
    engine
}

/// A project on a volume the probe says folds case (on every host), holding
/// one sheet under two spellings, each dirty with different content.
fn folding_app_with_two_spellings() -> (Signex, Arc<Recorder>, tempfile::TempDir, PathBuf, PathBuf)
{
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_str().expect("utf8 tempdir").to_string();
    let mut app = app_workspace(&dir, &["Lib.snxsch"]);
    app.document_state.projects[0].case_fold = CaseFold::from_probe(true);
    let recorder = Arc::new(Recorder::default());
    app.document_state.document_saves = DocumentSaves::new(recorder.clone());
    let upper = tmp.path().join("Lib.snxsch");
    let lower = tmp.path().join("lib.snxsch");
    for (path, paper) in [(&upper, "A4"), (&lower, "A3")] {
        app.document_state
            .engines
            .insert(path.clone(), engine_at(path, paper));
        app.document_state.dirty_paths.insert(path.clone());
    }
    (app, recorder, tmp, upper, lower)
}

fn run(task: iced::Task<Message>) -> Vec<Message> {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(async {
            let mut out = Vec::new();
            while let Some(action) = stream.next().await {
                if let iced_runtime::Action::Output(message) = action {
                    out.push(message);
                }
            }
            out
        })
}

fn bytes_of(app: &Signex, path: &Path) -> Vec<u8> {
    app.document_state.engines[path]
        .serialize_with_net_colors(&Default::default())
        .expect("serialize")
        .into_bytes()
}

#[test]
fn an_older_write_under_one_spelling_never_lands_over_a_newer_one_under_another() {
    let (mut app, recorder, _tmp, upper, lower) = folding_app_with_two_spellings();
    let newer_bytes = bytes_of(&app, &lower);
    // Ctrl+S under `Lib`, then a newer Ctrl+S under `lib` while it is owed.
    let older = app.begin_schematic_save(upper.clone(), None).expect("A");
    let newer = app.begin_schematic_save(lower.clone(), None).expect("B");
    // The scheduler runs them in the bad order: newer first, older last.
    let newer_done = run(newer);
    let older_done = run(older);
    let writes = recorder.0.lock().expect("recorder");
    assert_eq!(
        writes.len(),
        1,
        "the older snapshot must not be written after the newer one"
    );
    assert!(writes[0].1 == newer_bytes, "disk holds the newer snapshot");
    drop(writes);
    for message in older_done.into_iter().chain(newer_done) {
        let _ = app.update(message);
    }
    assert!(!app.document_state.dirty_paths.contains(&lower));
    assert!(
        app.document_state.dirty_paths.contains(&upper),
        "the spelling whose bytes never landed stays dirty"
    );
}

#[test]
fn a_sibling_that_wins_on_disk_is_cleaned_and_the_older_spelling_stays_dirty() {
    let (mut app, recorder, _tmp, upper, lower) = folding_app_with_two_spellings();
    let sibling_bytes = bytes_of(&app, &lower);
    // Save All writes `Lib` (older); the sibling's Ctrl+S on `lib` is newer
    // and its write lands first, but its completion is enqueued last.
    let save_all = app.begin_schematic_save(upper.clone(), None).expect("A");
    let sibling = app.begin_schematic_save(lower.clone(), None).expect("B");
    let sibling_done = run(sibling);
    let save_all_done = run(save_all);
    assert!(
        recorder
            .0
            .lock()
            .expect("recorder")
            .last()
            .is_some_and(|write| write.1 == sibling_bytes),
        "disk must end holding the newest snapshot"
    );
    for message in save_all_done.into_iter().chain(sibling_done) {
        let _ = app.update(message);
    }
    assert!(!app.document_state.dirty_paths.contains(&lower));
    assert!(
        app.document_state.dirty_paths.contains(&upper),
        "the older spelling's bytes never landed, so it stays dirty"
    );
    assert!(!app.document_state.document_saves.is_saving(&upper));
    assert!(!app.document_state.document_saves.is_saving(&lower));
}

#[test]
fn a_project_fold_change_between_two_saves_of_one_spelling_keeps_one_ordering() {
    let (mut app, recorder, _tmp, upper, _lower) = folding_app_with_two_spellings();
    let first = app.begin_schematic_save(upper.clone(), None).expect("1");
    // The probe reports again (or the sheet gains a project owner) while the
    // first write is still owed: the project's fold differs now.
    app.document_state.projects[0].case_fold = CaseFold::from_probe(false);
    let second = app.begin_schematic_save(upper.clone(), None).expect("2");
    let second_done = run(second);
    let first_done = run(first);
    assert_eq!(
        recorder.0.lock().expect("recorder").len(),
        1,
        "the older save must be superseded, not written last"
    );
    for message in first_done.into_iter().chain(second_done) {
        let _ = app.update(message);
    }
    assert!(!app.document_state.document_saves.is_saving(&upper));
}

#[test]
fn two_spellings_of_a_loose_document_share_one_ordering() {
    let (mut app, recorder, tmp, _upper, _lower) = folding_app_with_two_spellings();
    let elsewhere = tmp.path().join("loose");
    let (a, b) = (
        elsewhere.join("Child.snxsch"),
        elsewhere.join("child.snxsch"),
    );
    for (path, paper) in [(&a, "A4"), (&b, "A3")] {
        app.document_state
            .engines
            .insert(path.clone(), engine_at(path, paper));
    }
    let older = app.begin_schematic_save(a, None).expect("A");
    let newer = app.begin_schematic_save(b, None).expect("B");
    let _ = run(newer);
    let _ = run(older);
    assert_eq!(recorder.0.lock().expect("recorder").len(), 1);
}
