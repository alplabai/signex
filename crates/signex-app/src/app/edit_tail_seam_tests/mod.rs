//! Epic #2326 slice 2, PR 2.0: characterisation of the schematic edit tails
//! (`finish_schematic_mutation`, `after_sheet_change`, the grouped step and
//! the multi-sheet finishers) before they move behind the core seam. Each
//! test pins what trunk does today, not a wish; see `edit_tail_tests.rs` for
//! the #2445 patch decisions these build on.

mod dirty_matrix;
mod groups;
mod members;
mod messages;
mod refresh;

use std::path::{Path, PathBuf};

use signex_engine::Command;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::current_placement::placed_twice_on_disk;
use crate::find_replace::{FindReplaceMsg, FindScope};

fn dirty(app: &Signex, path: &Path) -> bool {
    app.document_state.is_document_dirty(path)
}

fn paper(size: &str) -> Command {
    Command::SetPaperSize {
        paper_size: size.to_string(),
    }
}

/// A sheet that is always unsaved, so the quit prompt opens on every call
/// and a clean sheet shows as missing from its list rather than as no prompt.
const SENTINEL: &str = "edit-tail-seam-sentinel.snxsch";

/// Whether the real quit prompt lists `path` as unsaved. The prompt is
/// dismissed again, so the test can go on.
fn quit_prompt_lists(app: &mut Signex, path: &Path) -> bool {
    app.document_state
        .mark_document_dirty(&std::env::temp_dir().join(SENTINEL));
    // The prompt is state; its Task (the window effect) is not run here.
    let _quit = app.handle_app_quit_requested();
    app.ui_state
        .app_quit_confirm
        .take()
        .expect("the quit prompt opens while the sentinel is unsaved")
        .dirty_paths
        .contains(&path.to_path_buf())
}

/// A project-scope Replace All of `query` by `replacement`, run to the end.
fn replace_in_project(app: &mut Signex, query: &str, replacement: &str) {
    app.ui_state.find_replace.options = signex_engine::MatchOptions::SUBSTRING;
    for msg in [
        FindReplaceMsg::ScopeChanged(FindScope::Project),
        FindReplaceMsg::QueryChanged(query.into()),
    ] {
        let _ = app.update(crate::app::Message::FindReplaceMsg(msg));
    }
    app.run_project_scan_blocking();
    let _ = app.update(crate::app::Message::FindReplaceMsg(
        FindReplaceMsg::ReplacementChanged(replacement.into()),
    ));
    let matches = app.ui_state.find_replace.matches.clone();
    app.run_project_replace_blocking(matches);
}

/// `placed_twice_on_disk` after a project Replace All of `NET` by `X`: `top`
/// open and active, `child` (closed before) now a parked tabless engine.
fn after_replace(dir: &tempfile::TempDir) -> (Signex, PathBuf, PathBuf) {
    let (mut app, top, child) = placed_twice_on_disk(dir);
    replace_in_project(&mut app, "net", "X");
    assert!(dirty(&app, &child), "the replace parked the child dirty");
    (app, top, child)
}

/// `placed_twice_on_disk` with `child` also loaded as a saved, cached engine.
fn with_saved_cached_child(dir: &tempfile::TempDir) -> (Signex, PathBuf, PathBuf) {
    let (mut app, top, child) = placed_twice_on_disk(dir);
    let text = std::fs::read_to_string(&child).unwrap();
    let sheet = signex_types::format::SnxSchematic::parse(&text)
        .unwrap()
        .sheet;
    let mut engine = signex_engine::Engine::new_with_path(sheet, Some(child.clone())).unwrap();
    engine.mark_saved();
    app.document_state.insert_engine(child.clone(), engine);
    (app, top, child)
}

/// The text of the first label as `slot`'s render cache draws it.
fn drawn_label(slot: &crate::canvas::CanvasSlot) -> String {
    slot.active_snapshot().expect("a render cache").labels[0]
        .text
        .clone()
}

/// The first label of the sheet at `path` as its engine holds it.
fn engine_label(app: &Signex, path: &Path) -> String {
    app.document_state.engines[path].document().labels[0]
        .text
        .clone()
}

/// A render cache of what the engine at `path` holds now.
fn fresh_cache(app: &Signex, path: &Path) -> crate::schematic_runtime::SchematicRenderCache {
    let engine = &app.document_state.engines[path];
    crate::schematic_runtime::SchematicRenderCache::from_sheet_at(
        engine.document(),
        engine.revision(),
    )
}
