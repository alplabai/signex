//! #1423 — a Save All that is refused over two case-variant spellings of one
//! file must keep every spelling dirty, name it, and not let quit close.

use std::path::{Path, PathBuf};

use super::save_all_tests::{
    CountingSink, app_with_two_engine_variants, count_writes, feed, feed_then_settle, save_all,
};
use crate::app::Signex;
use crate::persist::run_task::outputs;

/// `Lib` is clean, `lib` is the only edited spelling.
fn only_lower_dirty(dir: &Path, differ: bool) -> (Signex, PathBuf, PathBuf) {
    let (mut app, upper, lower) = app_with_two_engine_variants(dir, differ);
    app.document_state.dirty_paths.remove(&upper);
    (app, upper, lower)
}

fn assert_quit_held_open(app: &Signex, after: &iced::Task<crate::app::Message>, name: &str) {
    assert_eq!(after.units(), 0, "Signex must not close over unsaved edits");
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(format!("{notice:?}").contains(name), "{notice:?}");
}

#[test]
fn a_refused_save_keeps_the_dirty_spelling_dirty_and_blocks_quit() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = only_lower_dirty(dir.path(), true);
    std::fs::write(&upper, b"on-disk").expect("seed");

    let task = save_all(&mut app, std::slice::from_ref(&lower));
    let after = feed_then_settle(&mut app, outputs(task));

    assert!(app.document_state.dirty_paths.contains(&lower));
    assert_eq!(std::fs::read(&upper).expect("read"), b"on-disk");
    assert_quit_held_open(&app, &after, "lib.snxsch");
}

#[test]
fn both_spellings_dirty_and_differing_are_refused_and_both_stay_dirty() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = app_with_two_engine_variants(dir.path(), true);

    let task = save_all(&mut app, &[upper.clone(), lower.clone()]);
    let after = feed_then_settle(&mut app, outputs(task));

    assert!(app.document_state.dirty_paths.contains(&upper));
    assert!(app.document_state.dirty_paths.contains(&lower));
    assert_quit_held_open(&app, &after, "snxsch");
}

#[test]
fn the_only_dirty_spelling_is_the_one_written_and_both_markers_clear() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = only_lower_dirty(dir.path(), false);
    let sink: std::sync::Arc<CountingSink> = count_writes(&mut app);

    let task = save_all(&mut app, std::slice::from_ref(&lower));
    feed(&mut app, outputs(task));

    assert_eq!(*sink.0.lock().unwrap(), vec![lower.clone()]);
    assert!(!app.document_state.dirty_paths.contains(&upper));
    assert!(!app.document_state.dirty_paths.contains(&lower));
    assert!(app.document_state.error_notice.is_none());
}

/// Replace the sheet held under `path`, as an edit does, and mark it dirty.
fn edit_engine(app: &mut Signex, path: &Path) {
    let document = signex_types::schematic::SchematicSheet {
        paper_size: "A2".to_string(),
        ..Default::default()
    };
    app.document_state
        .engines
        .get_mut(path)
        .expect("engine")
        .execute(signex_engine::Command::ReplaceDocument { document })
        .expect("edit");
    app.document_state.dirty_paths.insert(path.to_path_buf());
}

#[test]
fn a_spelling_edited_during_the_write_stays_dirty_and_blocks_quit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = only_lower_dirty(dir.path(), false);

    let task = save_all(&mut app, std::slice::from_ref(&lower));
    edit_engine(&mut app, &upper);
    let after = feed_then_settle(&mut app, outputs(task));

    assert!(!app.document_state.dirty_paths.contains(&lower));
    assert!(app.document_state.dirty_paths.contains(&upper));
    assert_eq!(after.units(), 0, "Signex must not close over the edit");
    let modal = app.ui_state.app_quit_confirm.as_ref().expect("quit gate");
    assert_eq!(modal.dirty_paths, vec![upper]);
}

/// A sink whose every write fails.
struct FailingSink;

impl crate::persist::DocumentSink for FailingSink {
    fn write(&self, _path: &Path, _bytes: &[u8]) -> std::io::Result<()> {
        Err(std::io::Error::other("disk full"))
    }
}

#[test]
fn three_spellings_with_two_dirty_stay_dirty_when_the_write_fails() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = app_with_two_engine_variants(dir.path(), false);
    let all_caps = dir.path().join("LIB.snxsch");
    let sheet = signex_types::schematic::SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    engine.set_path(Some(all_caps.clone()));
    app.document_state.engines.insert(all_caps.clone(), engine);
    // `LIB` sorts first and is clean; `Lib` is the first dirty one.
    app.document_state.document_saves =
        crate::persist::DocumentSaves::new(std::sync::Arc::new(FailingSink));

    let task = save_all(&mut app, &[upper.clone(), lower.clone()]);
    let after = feed_then_settle(&mut app, outputs(task));

    assert!(
        app.document_state.document_saves.failure(&upper).is_some(),
        "the first dirty spelling was really written, and that write failed"
    );
    assert!(app.document_state.dirty_paths.contains(&upper));
    assert!(app.document_state.dirty_paths.contains(&lower));
    assert_eq!(after.units(), 0, "Signex must not close over unsaved edits");
}

#[test]
fn a_net_colour_edit_during_the_write_keeps_the_other_spelling_dirty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = only_lower_dirty(dir.path(), false);

    let task = save_all(&mut app, std::slice::from_ref(&lower));
    app.mark_net_colors_dirty(&upper);
    let after = feed_then_settle(&mut app, outputs(task));

    assert!(app.document_state.dirty_paths.contains(&upper));
    assert_eq!(after.units(), 0, "Signex must not close over the colour");
    let modal = app.ui_state.app_quit_confirm.as_ref().expect("quit gate");
    assert_eq!(modal.dirty_paths, vec![upper]);
}

#[test]
fn an_undecided_directory_is_reported_by_the_shared_builder_from_update() {
    use crate::app::contracts::{SaveAllNext, SaveAllPlan};
    use crate::app::state::project_case::{CaseProbeOutcome, undecided_message};
    use crate::app::state::scope::CaseFold;

    let dir = Path::new("/undecided-1423-probe-dir");
    let outcome = CaseProbeOutcome::StatFailed("permission denied".into());
    let line = undecided_message(&outcome, dir, CaseFold::host_default());
    assert!(line.len() <= 157, "{} bytes: {line}", line.len());
    assert!(line.ends_with(&format!("{}", dir.display())), "{line}");

    let before = crate::diagnostics::logged_count(&line);
    let (mut app, _task) = Signex::new();
    let _ = app.begin_save_all_planned(SaveAllPlan {
        context: "Save All".into(),
        paths: Vec::new(),
        rows: Vec::new(),
        next: SaveAllNext::Quit,
        probes: Default::default(),
        undecided: vec![(dir.to_path_buf(), outcome)],
    });
    assert_eq!(crate::diagnostics::logged_count(&line), before + 1);
}

#[test]
fn probing_logs_nothing_and_returns_the_undecided_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("gone-1423");
    let before = crate::diagnostics::logged_count("gone-1423");
    let (decided, undecided) = super::save_all::probe_dirs(vec![missing.clone()]);
    assert_eq!(crate::diagnostics::logged_count("gone-1423"), before);
    assert!(!decided.contains_key(&missing) || undecided.is_empty());
}
