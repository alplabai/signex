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

    let task = save_all(&mut app, &[lower.clone()]);
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

    let task = save_all(&mut app, &[lower.clone()]);
    feed(&mut app, outputs(task));

    assert_eq!(*sink.0.lock().unwrap(), vec![lower.clone()]);
    assert!(!app.document_state.dirty_paths.contains(&upper));
    assert!(!app.document_state.dirty_paths.contains(&lower));
    assert!(app.document_state.error_notice.is_none());
}
