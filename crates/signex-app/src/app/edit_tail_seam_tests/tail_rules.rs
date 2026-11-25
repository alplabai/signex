//! #2645: the four rows where the active-sheet tail (A1,
//! `finish_schematic_mutation`) and the by-path tail (A2,
//! `after_sheet_change`) followed different rules until both became one
//! `settle` (decided 2026-11-25, docs/DECISIONS.md). One test per row and
//! tail, each on the decided rule.

use std::path::{Path, PathBuf};

use signex_engine::DocumentPatch;

use super::refresh::{begin_agent_run, content_clears};
use super::{dirty, paper};
use crate::app::Signex;
use crate::app::command::queries::tests::{app_with, path};
use crate::app::core::event::ChangeCause;
use crate::app::mutation_gateway::MutationOutcome;
use crate::app::projector::RefreshScope;

const ACTIVE: &str = "tail-rules-a.snxsch";
const OTHER: &str = "tail-rules-b.snxsch";

/// Two clean sheets open; `ACTIVE` is the main window's active one.
fn two_sheets() -> (Signex, PathBuf, PathBuf) {
    let app = app_with(&[(OTHER, 200), (ACTIVE, 100)]);
    (app, path(ACTIVE), path(OTHER))
}

/// Change the paper of the sheet at `path` on its engine alone, as a
/// multi-sheet finisher does before it calls a tail; the patch to settle.
fn paper_on_engine(app: &mut Signex, path: &Path) -> DocumentPatch {
    let engine = app.document_state.engines.get_mut(path).unwrap();
    let result = engine.execute(paper("A3")).unwrap();
    result.patch_pair.expect("a changed paper").document
}

/// Forget when the panel context was last built, so a rebuild shows.
fn reset_panel_build(app: &mut Signex) {
    app.document_state.times.now = None;
}

fn panel_rebuilt(app: &Signex) -> bool {
    app.document_state.times.now.is_some()
}

fn drawn_page(app: &Signex) -> (f32, f32) {
    let canvas = app.interaction_state.active_canvas();
    (canvas.paper_width_mm, canvas.paper_height_mm)
}

// ---- Row 1: an empty patch.

#[test]
fn row_1_the_active_tail_returns_on_an_empty_patch_before_the_dirty_sync() {
    for patch in [None, Some(DocumentPatch::NONE)] {
        let (mut app, a, _) = two_sheets();
        // Held dirty at the save point: a sync from the history would clear it.
        app.document_state.keep_document_dirty_until_saved(&a);
        reset_panel_build(&mut app);
        assert!(!app.finish_schematic_mutation(patch, None, true, true));
        assert!(dirty(&app, &a), "{patch:?}: no dirty sync");
        assert!(!panel_rebuilt(&app), "{patch:?}: no panel refresh");
    }
}

#[test]
fn row_1_the_by_path_tail_returns_on_an_empty_patch_before_the_dirty_sync() {
    let (mut app, a, _) = two_sheets();
    app.document_state.keep_document_dirty_until_saved(&a);
    reset_panel_build(&mut app);
    app.after_sheet_change(&a, DocumentPatch::NONE);
    assert!(dirty(&app, &a), "no dirty sync");
    assert!(!panel_rebuilt(&app), "no panel refresh");
}

// ---- Row 2: an agent's run on another sheet.

/// The wrapper's #2645 `debug_assert` refuses this state, so the test runs
/// its core half and projection as the wrapper does.
#[test]
fn row_2_the_active_tail_settles_the_active_sheet_an_agent_run_on_another_sheet_does_not_target() {
    let (mut app, a, b) = two_sheets();
    let patch = paper_on_engine(&mut app, &a);
    begin_agent_run(&mut app, &b);
    let clears = content_clears(&app);
    let edit = app
        .document_state
        .settle_active_sheet(patch, ChangeCause::Edit);
    assert_eq!(edit.outcome, MutationOutcome::Changed);
    let scope = RefreshScope::ActiveCanvas {
        clear_overlay: true,
        update_selection_info: true,
    };
    app.project_events(edit.events, scope);
    assert_eq!(app.document_state.agent_edits.owed, DocumentPatch::NONE);
    assert!(dirty(&app, &a));
    assert!(content_clears(&app) > clears, "the active canvas repainted");
}

/// The gateway settles on the sheet the edit acted on: a person's edit run
/// through `on_edit_target` on a sheet that is not the active one marks that
/// sheet dirty, never the active one.
#[test]
fn row_2_a_gateway_edit_on_the_edit_target_marks_the_target_dirty() {
    let (mut app, a, b) = two_sheets();
    let changed = app.on_edit_target(&b, |app| app.apply_engine_command(paper("A3"), true, true));
    assert!(changed);
    assert_eq!(app.document_state.engines[&b].document().paper_size, "A3");
    assert!(dirty(&app, &b), "the target is marked dirty");
    assert!(!dirty(&app, &a), "the active sheet is untouched");
    assert_eq!(app.document_state.agent_edits.owed, DocumentPatch::NONE);
}

#[test]
fn row_2_the_by_path_tail_settles_a_sheet_an_agent_run_on_another_sheet_does_not_target() {
    let (mut app, a, b) = two_sheets();
    let patch = paper_on_engine(&mut app, &a);
    begin_agent_run(&mut app, &b);
    let clears = content_clears(&app);
    app.after_sheet_change(&a, patch);
    assert_eq!(app.document_state.agent_edits.owed, DocumentPatch::NONE);
    assert!(dirty(&app, &a));
    assert!(content_clears(&app) > clears, "the active canvas repainted");
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "#2645")]
fn row_2_the_active_tail_asserts_that_an_agent_run_owes_only_its_own_target() {
    let (mut app, a, b) = two_sheets();
    let patch = paper_on_engine(&mut app, &a);
    begin_agent_run(&mut app, &b);
    app.finish_schematic_mutation(Some(patch), None, true, true);
}

// ---- Row 3: no engine behind the sheet.

/// An app whose active path has no engine loaded.
fn active_without_engine() -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let ghost = path("tail-rules-no-engine.snxsch");
    app.document_state.active_path = Some(ghost.clone());
    (app, ghost)
}

#[test]
fn row_3_the_active_tail_with_no_engine_stops_after_the_erc_epoch() {
    let (mut app, _ghost) = active_without_engine();
    let epoch = app.document_state.sheet_edit_epoch;
    reset_panel_build(&mut app);
    assert!(!app.finish_schematic_mutation(Some(DocumentPatch::PAPER), None, true, true));
    assert_eq!(app.document_state.sheet_edit_epoch, epoch + 1);
    assert!(!panel_rebuilt(&app), "no panel refresh");
}

#[test]
fn row_3_the_by_path_tail_with_no_engine_stops_after_the_erc_epoch() {
    let (mut app, ghost) = active_without_engine();
    let epoch = app.document_state.sheet_edit_epoch;
    reset_panel_build(&mut app);
    app.after_sheet_change(&ghost, DocumentPatch::PAPER);
    assert_eq!(app.document_state.sheet_edit_epoch, epoch + 1);
    assert!(!panel_rebuilt(&app), "no panel refresh");
}

// ---- Row 4: a paper change of the active sheet.

#[test]
fn row_4_the_active_tail_moves_the_panel_and_the_drawn_page() {
    let (mut app, a, _) = two_sheets();
    assert_eq!(drawn_page(&app), crate::panels::paper_dimensions("A4"));
    let patch = paper_on_engine(&mut app, &a);
    assert!(app.finish_schematic_mutation(Some(patch), None, true, true));
    assert_eq!(app.document_state.panel_ctx.paper_size, "A3");
    assert_eq!(drawn_page(&app), crate::panels::paper_dimensions("A3"));
}

#[test]
fn row_4_the_by_path_tail_moves_the_panel_and_the_drawn_page() {
    let (mut app, a, _) = two_sheets();
    assert_eq!(drawn_page(&app), crate::panels::paper_dimensions("A4"));
    let patch = paper_on_engine(&mut app, &a);
    app.after_sheet_change(&a, patch);
    assert_eq!(app.document_state.panel_ctx.paper_size, "A3");
    assert_eq!(drawn_page(&app), crate::panels::paper_dimensions("A3"));
}

/// The named change of the decision: an undo or redo by path of a paper
/// change on the active sheet redraws the page frame at the size it takes.
#[test]
fn row_4_an_undo_and_redo_by_path_of_a_paper_change_redraw_the_drawn_page() {
    let (mut app, a, _) = two_sheets();
    assert_eq!(app.execute_on_sheet(&a, paper("A3")), Ok(true));
    assert_eq!(drawn_page(&app), crate::panels::paper_dimensions("A3"));
    assert_eq!(app.step_sheet_on(&a, true), Ok(true));
    assert_eq!(app.document_state.panel_ctx.paper_size, "A4");
    assert_eq!(drawn_page(&app), crate::panels::paper_dimensions("A4"));
    assert_eq!(app.step_sheet_on(&a, false), Ok(true));
    assert_eq!(drawn_page(&app), crate::panels::paper_dimensions("A3"));
}
