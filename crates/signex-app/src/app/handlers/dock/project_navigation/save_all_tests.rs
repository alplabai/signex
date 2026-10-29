//! #859 — Save All writes off the UI thread and only then closes / quits.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::app::contracts::{SaveAllNext, SaveAllPlan};
use crate::app::state::project_case::CaseProbeOutcome;
use crate::app::state::scope::CaseFold;
use crate::app::{FileMsg, Message, Signex, SymbolDocument, TabInfo, TabKind};
use crate::persist::run_task::outputs;

/// An app with a dirty symbol draft at `dir/Lib.snxsym` and a dirty
/// schematic at `dir/Sheet.snxsch`.
fn app_with_two_dirty_documents(dir: &Path) -> (Signex, PathBuf, PathBuf) {
    let (mut app, _task) = Signex::new();
    let symbol_path = dir.join("Lib.snxsym");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Sym1"));
    let mut editor = SymbolDocument::new(symbol_path.clone(), file);
    editor.mark_dirty();
    app.document_state
        .symbol_editors
        .insert(symbol_path.clone(), editor);
    app.document_state.mark_document_dirty(&symbol_path);

    let sheet_path = dir.join("Sheet.snxsch");
    let mut engine = signex_engine::Engine::new(signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    })
    .expect("engine");
    engine.set_path(Some(sheet_path.clone()));
    app.document_state
        .engines
        .insert(sheet_path.clone(), engine);
    app.document_state.mark_document_dirty(&sheet_path);
    app.document_state.tabs.push(TabInfo {
        title: "Sheet".into(),
        path: sheet_path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    (app, symbol_path, sheet_path)
}

/// A Save All whose directory probes were answered as `probes` says.
fn save_all_probed(
    app: &mut Signex,
    paths: &[PathBuf],
    probes: HashMap<PathBuf, bool>,
) -> iced::Task<Message> {
    app.begin_save_all_planned(SaveAllPlan {
        context: "Save All".into(),
        paths: paths.to_vec(),
        rows: Vec::new(),
        next: SaveAllNext::Quit,
        probes,
        undecided: Vec::new(),
    })
}

/// A Save All probed against the real filesystem, synchronously.
pub(super) fn save_all(app: &mut Signex, paths: &[PathBuf]) -> iced::Task<Message> {
    let (probes, undecided) = super::save_all::probe_dirs(app.loose_parent_dirs(paths));
    app.begin_save_all_planned(SaveAllPlan {
        context: "Save All".into(),
        paths: paths.to_vec(),
        rows: Vec::new(),
        next: SaveAllNext::Quit,
        probes,
        undecided,
    })
}

pub(super) fn feed(app: &mut Signex, messages: Vec<Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

fn ends_with_settle(messages: &[Message]) -> bool {
    matches!(
        messages.last(),
        Some(Message::File(FileMsg::SaveAllSettled(_)))
    )
}

#[test]
fn save_all_writes_when_the_task_runs_and_settles_after_every_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, sheet) = app_with_two_dirty_documents(dir.path());

    let task = save_all(&mut app, &[symbol.clone(), sheet.clone()]);
    assert!(!symbol.exists() && !sheet.exists(), "update wrote nothing");

    let messages = outputs(task);
    assert!(symbol.exists() && sheet.exists());
    assert!(
        ends_with_settle(&messages),
        "the settle message follows the completions"
    );
    feed(&mut app, messages);
    assert!(!app.document_state.has_dirty_documents());
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn a_path_nothing_backs_is_reported_with_a_reason() {
    let (mut app, _task) = Signex::new();
    let Err(err) =
        app.begin_dirty_path_save_with(Path::new("/nonexistent/Ghost.snxsym"), CaseFold::FOLD)
    else {
        panic!("nothing backs this path");
    };
    assert!(
        format!("{err:#}").contains("can't be saved automatically"),
        "unexpected error: {err:#}"
    );
}

#[test]
fn a_failed_write_keeps_signex_open_and_names_the_file_and_the_reason() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, sheet) = app_with_two_dirty_documents(dir.path());
    // A file where the sheet's directory belongs makes only that write fail.
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"x").expect("blocker");
    let blocked = blocker.join("Sheet.snxsch");
    let engine = app.document_state.engines.remove(&sheet).expect("engine");
    app.document_state.engines.insert(blocked.clone(), engine);
    app.document_state.mark_document_clean(&sheet);
    app.document_state.mark_document_dirty(&blocked);

    let task = save_all(&mut app, &[symbol.clone(), blocked.clone()]);
    feed(&mut app, outputs(task));

    assert!(symbol.exists(), "the other file still saved");
    assert!(app.document_state.is_document_dirty(&blocked));
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(format!("{notice:?}").contains("stayed open"), "{notice:?}");
    assert!(format!("{notice:?}").contains("Sheet.snxsch"), "{notice:?}");
    let path_text = blocked.display().to_string();
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|entry| entry.message.contains(&path_text)),
        "the failure must reach the Messages panel"
    );
}

#[test]
fn a_draft_edited_during_save_all_is_reported_unsaved() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, _sheet) = app_with_two_dirty_documents(dir.path());
    let task = save_all(&mut app, std::slice::from_ref(&symbol));
    app.document_state
        .symbol_editors
        .get_mut(&symbol)
        .expect("editor")
        .set_dirty(true);

    feed(&mut app, outputs(task));

    assert!(app.document_state.is_document_dirty(&symbol));
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(format!("{notice:?}").contains("Lib.snxsym"), "{notice:?}");
}

#[test]
fn quit_waits_for_the_writes_of_a_running_save_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, sheet) = app_with_two_dirty_documents(dir.path());
    let task = save_all(&mut app, &[symbol.clone(), sheet.clone()]);
    let barrier = app.document_state.document_saves.barrier();
    let (tx, rx) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        barrier.wait();
        tx.send(()).expect("send");
    });
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "the barrier holds while the writes have not run"
    );
    let _ = outputs(task);
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .expect("the barrier opens once both writes finished");
    waiter.join().expect("waiter");
    assert!(symbol.exists() && sheet.exists());
}

/// Feed every completion in `messages` and hand back what the settle at
/// their end returns: a task with no units means nothing closed.
pub(super) fn feed_then_settle(app: &mut Signex, messages: Vec<Message>) -> iced::Task<Message> {
    let mut settled = None;
    for message in messages {
        match message {
            Message::File(FileMsg::SaveAllSettled(settle)) => {
                settled = Some(app.handle_save_all_settled(*settle));
            }
            other => {
                let _ = app.update(other);
            }
        }
    }
    settled.expect("the Save All ends with its settle")
}

/// Mark the schematic at `path` edited, the way an edit site does.
fn edit_sheet(app: &mut Signex, path: &Path) {
    app.document_state.mark_document_dirty(path);
}

#[test]
fn a_clean_sheet_edited_while_quit_save_all_runs_is_prompted_not_dropped() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, sheet) = app_with_two_dirty_documents(dir.path());
    // The sheet starts clean: the quit prompt only listed the symbol.
    app.document_state.mark_document_clean(&sheet);
    {
        let path = app.document_state.tabs[0].path.clone();
        app.document_state.mark_document_clean(&path);
    }

    let task = save_all(&mut app, std::slice::from_ref(&symbol));
    edit_sheet(&mut app, &sheet);
    let after = feed_then_settle(&mut app, outputs(task));

    assert_eq!(
        after.units(),
        0,
        "Signex must not close over an unsaved sheet"
    );
    let modal = app
        .ui_state
        .app_quit_confirm
        .as_ref()
        .expect("the quit gate asks again");
    assert_eq!(modal.dirty_paths, vec![sheet.clone()]);
    assert!(app.document_state.is_document_dirty(&sheet));
}

#[test]
fn quit_save_all_with_nothing_edited_meanwhile_closes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, sheet) = app_with_two_dirty_documents(dir.path());
    let task = save_all(&mut app, &[symbol, sheet]);
    let after = feed_then_settle(&mut app, outputs(task));
    assert!(after.units() > 0, "the close goes ahead");
    assert!(app.ui_state.app_quit_confirm.is_none());
}

#[test]
fn a_save_all_write_superseded_by_a_later_ctrl_s_does_not_quit_before_that_save_lands() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, sheet) = app_with_two_dirty_documents(dir.path());
    let save_all_task = save_all(&mut app, &[symbol, sheet.clone()]);
    // Ctrl+S on the sheet while Save All runs: the newer snapshot.
    let ctrl_s = app
        .begin_schematic_save(sheet.clone(), None)
        .expect("ctrl+s starts");
    // The Ctrl+S write lands first; its completion is not handled yet.
    let ctrl_s_done = outputs(ctrl_s);
    let save_all_messages = outputs(save_all_task);
    assert!(
        save_all_messages.iter().any(|m| matches!(
            m,
            Message::File(FileMsg::DocumentSaved(done))
                if done.path == sheet
                    && matches!(done.result, Ok(crate::persist::SaveOutcome::Superseded))
        )),
        "the older Save All snapshot is superseded"
    );

    let after = feed_then_settle(&mut app, save_all_messages);
    assert_eq!(
        after.units(),
        0,
        "no quit while the sheet's save is unreported"
    );
    assert!(app.document_state.is_document_dirty(&sheet));
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(format!("{notice:?}").contains("Sheet.snxsch"), "{notice:?}");

    feed(&mut app, ctrl_s_done);
    assert!(!app.document_state.is_document_dirty(&sheet));
}

#[test]
fn save_all_over_a_running_primitive_ctrl_s_keeps_signex_open_until_it_lands() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, sheet) = app_with_two_dirty_documents(dir.path());
    let ctrl_s = app.save_primitive_tab_at(&symbol).expect("ctrl+s starts");
    let save_all_task = save_all(&mut app, &[symbol.clone(), sheet]);

    let after = feed_then_settle(&mut app, outputs(save_all_task));
    assert_eq!(after.units(), 0, "the symbol has not reported back yet");
    assert!(app.document_state.is_document_dirty(&symbol));
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(format!("{notice:?}").contains("Lib.snxsym"), "{notice:?}");

    feed(&mut app, outputs(ctrl_s));
    assert!(symbol.exists());
    assert!(!app.document_state.is_document_dirty(&symbol));
    assert!(!app.document_state.document_saves.is_saving(&symbol));
}

#[test]
fn kept_paths_compare_without_regard_to_order() {
    let (a, b) = (PathBuf::from("a.snxsch"), PathBuf::from("b.snxsch"));
    assert!(super::save_all::same_paths(
        &[a.clone(), b.clone()],
        &[b.clone(), a.clone()]
    ));
    assert!(!super::save_all::same_paths(std::slice::from_ref(&a), &[b]));
    assert!(!super::save_all::same_paths(&[a], &[]));
}

/// Two engines for one file, held as `Lib`/`lib`; the second carries a
/// different edit (paper size) when `edit_second` is set.
pub(super) fn app_with_two_engine_variants(
    dir: &Path,
    edit_second: bool,
) -> (Signex, PathBuf, PathBuf) {
    let upper = dir.join("Lib.snxsch");
    let lower = dir.join("lib.snxsch");
    // A project probed as case-folding pins the volume rule on every host.
    // Without one the fold is the host default, which does not fold on macOS:
    // the twins then key apart, both are written, and the ledger may
    // supersede either depending on blocking-pool scheduling (#1216).
    let mut app = crate::app::handlers::menu::export::tests::app_workspace(
        dir.to_str().expect("utf8 tempdir"),
        &["Lib.snxsch"],
    );
    app.document_state.projects[0].case_fold = CaseFold::from_probe(true);
    app.document_state.projects[0].case_decided = true;
    let second_paper = if edit_second { "A3" } else { "A4" };
    for (path, paper) in [(&upper, "A4"), (&lower, second_paper)] {
        let sheet = signex_types::schematic::SchematicSheet {
            paper_size: paper.to_string(),
            ..Default::default()
        };
        let mut engine = signex_engine::Engine::new(sheet).expect("engine");
        engine.set_path(Some(path.clone()));
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.mark_document_dirty(&path);
    }
    (app, upper, lower)
}

#[test]
fn save_all_finds_a_document_held_under_a_case_variant_spelling() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, held, _sheet) = app_with_two_dirty_documents(dir.path());
    let variant = dir.path().join("lib.SNXSYM");

    let task = app
        .begin_dirty_path_save_with(&variant, CaseFold::FOLD)
        .expect("a held spelling backs the variant")
        .expect("an off-thread write");
    feed(&mut app, outputs(task));

    assert!(held.exists(), "the held spelling must be the one written");
    assert!(!app.document_state.is_document_dirty(&held));
}

#[test]
fn save_all_refuses_differing_case_variants_and_writes_nothing() {
    // On a folding host the two spellings are ONE file; writing both
    // would let the second silently replace the first's edits.
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = app_with_two_engine_variants(dir.path(), true);
    std::fs::write(&upper, b"on-disk").expect("seed");

    let Err(err) = app.begin_dirty_path_save_with(&upper, CaseFold::FOLD) else {
        panic!("differing variants must be refused");
    };

    let text = format!("{err:#}");
    assert!(
        text.contains("Lib.snxsch") && text.contains("lib.snxsch"),
        "{text}"
    );
    assert_eq!(std::fs::read(&upper).expect("read"), b"on-disk");
    assert!(app.document_state.is_document_dirty(&upper));
    assert!(app.document_state.is_document_dirty(&lower));
}

#[test]
fn save_all_writes_identical_case_variants_once_and_clears_every_marker() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = app_with_two_engine_variants(dir.path(), false);

    let task = save_all(&mut app, &[upper.clone(), lower.clone()]);
    feed(&mut app, outputs(task));

    assert!(
        !app.document_state.has_dirty_documents(),
        "{:?}",
        app.document_state.dirty_document_paths()
    );
    assert!(app.document_state.error_notice.is_none());
}

/// A recovery copy of a document that is no longer open is removed when a
/// Save All settles, so it cannot outlive the quit that follows.
#[test]
fn quit_save_all_removes_recovery_copies_of_saved_work() {
    use crate::autosave::{DocStamp, sidecar};

    let dir = tempfile::tempdir().expect("tempdir");
    let recovery = tempfile::tempdir().expect("recovery dir");
    let (mut app, symbol, sheet) = app_with_two_dirty_documents(dir.path());
    let gone = dir.path().join("Closed.snxsch");
    app.document_state
        .autosave
        .set_dir(Some(recovery.path().to_path_buf()));
    sidecar::write(recovery.path(), &gone, b"stale").expect("write sidecar");
    let stamp = DocStamp {
        revision: 1,
        net_color_revision: 0,
    };
    app.document_state.autosave.begin(&gone);
    app.document_state.autosave.finish(&gone, stamp, true);
    let copy = sidecar::sidecar_path(recovery.path(), &gone);
    assert!(copy.exists());

    let task = save_all(&mut app, &[symbol, sheet]);
    let after = feed_then_settle(&mut app, outputs(task));
    feed(&mut app, outputs(after));

    assert!(!copy.exists(), "the Save All settle sweeps stale copies");
}

/// Two engines for one file with no project open, held as `Lib`/`lib` and
/// encoding to identical bytes.
fn loose_identical_variants(dir: &Path) -> (Signex, PathBuf, PathBuf) {
    let (mut app, _task) = Signex::new();
    let upper = dir.join("Lib.snxsch");
    let lower = dir.join("lib.snxsch");
    for path in [&upper, &lower] {
        let mut engine = signex_engine::Engine::new(Default::default()).expect("engine");
        engine.set_path(Some(path.clone()));
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.mark_document_dirty(&path);
    }
    (app, upper, lower)
}

/// Counts the document writes a Save All issues, performing each for real.
#[derive(Default)]
pub(super) struct CountingSink(pub(super) std::sync::Mutex<Vec<PathBuf>>);

impl crate::persist::DocumentSink for CountingSink {
    fn write(&self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        self.0.lock().unwrap().push(path.to_path_buf());
        std::fs::write(path, bytes)
    }
}

/// Records the writes a Save All issues without touching the disk, so a
/// test about which spellings are written does not depend on whether the
/// host's temp directory folds case (two writes to one real file race).
#[derive(Default)]
struct RecordingSink(std::sync::Mutex<Vec<PathBuf>>);

impl crate::persist::DocumentSink for RecordingSink {
    fn write(&self, path: &Path, _bytes: &[u8]) -> std::io::Result<()> {
        self.0.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

fn record_writes(app: &mut Signex) -> std::sync::Arc<RecordingSink> {
    let sink = std::sync::Arc::new(RecordingSink::default());
    app.document_state.document_saves = crate::persist::DocumentSaves::new(sink.clone());
    sink
}

pub(super) fn count_writes(app: &mut Signex) -> std::sync::Arc<CountingSink> {
    let sink = std::sync::Arc::new(CountingSink::default());
    app.document_state.document_saves = crate::persist::DocumentSaves::new(sink.clone());
    sink
}

/// #1216 — with no project, the directory's probe decides whether two
/// spellings are one file. A positive probe collapses them on any host, so
/// reverting `fold_for_save` to the host default turns this red on Linux.
#[test]
fn loose_identical_variants_collapse_when_the_directory_probe_says_so() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = loose_identical_variants(dir.path());
    let sink = count_writes(&mut app);
    let probes = HashMap::from([(dir.path().to_path_buf(), true)]);

    let task = save_all_probed(&mut app, &[upper.clone(), lower.clone()], probes);
    feed(&mut app, outputs(task));

    // One write, to the spelling that sorts first, on every host.
    assert_eq!(*sink.0.lock().unwrap(), vec![upper.clone()]);

    assert!(
        !app.document_state.has_dirty_documents(),
        "{:?}",
        app.document_state.dirty_document_paths()
    );
    assert!(app.document_state.error_notice.is_none());
}

/// The mirror image: a directory that probes case-sensitive keeps the two
/// spellings apart, so a host that folds by default (Windows) goes red if
/// Save All reverts to the host default.
///
/// The probe result is fed in, but the writes still hit the real volume: on a
/// case-insensitive one (APFS on the macOS runners, #1576) `Lib.snxsch` and
/// `lib.snxsch` really are one file, so the premise only holds where the temp
/// directory is genuinely case-sensitive.
#[test]
fn loose_identical_variants_stay_apart_when_the_directory_probe_says_so() {
    let dir = tempfile::tempdir().expect("tempdir");
    if crate::app::state::volume_case::directory_ignores_case(dir.path()) {
        return;
    }
    let (mut app, upper, lower) = loose_identical_variants(dir.path());
    let sink = count_writes(&mut app);
    let probes = HashMap::from([(dir.path().to_path_buf(), false)]);

    let task = save_all_probed(&mut app, &[upper.clone(), lower.clone()], probes);
    feed(&mut app, outputs(task));

    let mut written = sink.0.lock().unwrap().clone();
    written.sort();
    assert_eq!(written, vec![upper, lower]);
}

/// The whole product path, probe task included, on the real volume.
#[test]
fn begin_save_all_probes_the_loose_directory_then_saves_both_spellings() {
    let dir = tempfile::tempdir().expect("tempdir");
    // A lettered entry of its own, so the probe answers from this directory
    // instead of borrowing a sibling in the shared temp dir that a parallel
    // test may delete mid-probe.
    std::fs::write(dir.path().join("Probe.txt"), b"").expect("probe entry");
    let (mut app, upper, lower) = loose_identical_variants(dir.path());
    let folds = crate::app::state::volume_case::directory_ignores_case(dir.path());
    let sink = count_writes(&mut app);

    let task = app.begin_save_all("Save All", &[upper.clone(), lower], &[], SaveAllNext::Quit);
    let probed = outputs(task);
    assert!(
        matches!(
            probed.as_slice(),
            [Message::File(FileMsg::SaveAllProbed(_))]
        ),
        "the probe runs as a task before any write"
    );
    let mut writes = Vec::new();
    for message in probed {
        writes.extend(outputs(app.update(message)));
    }
    feed(&mut app, writes);

    assert!(!app.document_state.has_dirty_documents());
    assert!(app.document_state.error_notice.is_none());
    let written = sink.0.lock().unwrap().clone();
    if folds {
        assert_eq!(written, vec![upper.clone()], "one file, one write");
    } else {
        assert_eq!(written.len(), 2, "two files, two writes: {written:?}");
    }
}

/// #1505 / #1447 — an undecided directory is one file for Save All, open and
/// Ctrl+S alike, so Save All writes the first dirty spelling once. The fold is
/// only ASSUMED, though: on a case-sensitive volume the other spelling is a
/// second file that was never written, so it stays dirty, is named, and Quit
/// stays open.
#[test]
fn an_undecided_directory_writes_once_and_keeps_the_other_spelling_dirty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = loose_identical_variants(dir.path());
    let sink = record_writes(&mut app);

    let task = app.begin_save_all_planned(SaveAllPlan {
        context: "Save All".into(),
        paths: vec![upper.clone(), lower.clone()],
        rows: Vec::new(),
        next: SaveAllNext::Quit,
        probes: HashMap::new(),
        undecided: vec![(dir.path().to_path_buf(), CaseProbeOutcome::Undecided)],
    });
    let after = feed_then_settle(&mut app, outputs(task));

    assert!(
        app.identity_fold_for(&lower).fold_case,
        "open and Ctrl+S fold"
    );
    assert_eq!(*sink.0.lock().unwrap(), vec![upper.clone()], "one write");
    assert!(!app.document_state.is_document_dirty(&upper));
    assert!(app.document_state.is_document_dirty(&lower));
    assert_eq!(
        after.units(),
        0,
        "Signex must not quit over the unsaved file"
    );
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(format!("{notice:?}").contains("lib.snxsch"), "{notice:?}");
}

/// A project whose own volume probe did not decide gives no decided rule:
/// the same keep-dirty answer as a loose undecided folder.
#[test]
fn an_undecided_project_keeps_the_other_spelling_dirty_too() {
    use crate::app::handlers::menu::export::tests::app_workspace;

    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_workspace(dir.path().to_str().expect("utf8"), &["X.snxsch"]);
    app.document_state.projects[0].case_probed = true;
    assert!(!app.document_state.projects[0].case_decided);
    let (upper, lower) = (dir.path().join("Lib.snxsch"), dir.path().join("lib.snxsch"));
    for path in [&upper, &lower] {
        let mut engine = signex_engine::Engine::new(Default::default()).expect("engine");
        engine.set_path(Some(path.clone()));
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.mark_document_dirty(&path);
    }
    let sink = record_writes(&mut app);

    let task = app.begin_save_all_planned(SaveAllPlan {
        context: "Save All".into(),
        paths: vec![upper.clone(), lower.clone()],
        rows: Vec::new(),
        next: SaveAllNext::Quit,
        probes: HashMap::new(),
        undecided: Vec::new(),
    });
    let after = feed_then_settle(&mut app, outputs(task));

    assert_eq!(*sink.0.lock().unwrap(), vec![upper.clone()]);
    assert!(app.document_state.is_document_dirty(&lower));
    assert_eq!(
        after.units(),
        0,
        "Signex must not quit over the unsaved file"
    );
}
