//! The core half of the edit tails on its own, without the projector
//! (#2326 slice 2, PR 2.2, test block B): the dirty mark, the events and
//! their order. The projected behaviour is pinned by `edit_tail_tests.rs`
//! and `edit_tail_seam_tests/`.

use std::path::{Path, PathBuf};

use signex_engine::{Command, DocumentPatch};
use signex_i18n_ids::ids;
use signex_types::schematic::{SelectedItem, SelectedKind};

use crate::app::Signex;
use crate::app::core::event::{ChangeCause, CoreEvent, DocumentChanged, NoticeLevel};
use crate::app::core::mutation_outcome::MutationOutcome;
use crate::app::handlers::editing_commands::net_color_driver as colours;

fn paper(size: &str) -> Command {
    Command::SetPaperSize {
        paper_size: size.to_string(),
    }
}

fn dirty(app: &Signex, path: &Path) -> bool {
    app.document_state.is_document_dirty(path)
}

fn changes(events: &[CoreEvent]) -> Vec<&DocumentChanged> {
    events
        .iter()
        .filter_map(|event| match event {
            CoreEvent::DocumentChanged(changed) => Some(changed),
            CoreEvent::Notice(_) => None,
        })
        .collect()
}

/// The levels of the notices in `events`, in order.
fn notice_levels(events: &[CoreEvent]) -> Vec<NoticeLevel> {
    events
        .iter()
        .filter_map(|event| match event {
            CoreEvent::Notice(notice) => Some(notice.level),
            CoreEvent::DocumentChanged(_) => None,
        })
        .collect()
}

/// A clean active sheet whose symbol is locked, and the symbol's and the
/// wire's selection items.
fn with_locked_symbol(name: &str) -> (Signex, PathBuf, SelectedItem, SelectedItem) {
    let (mut app, path) = colours::app_with_sheet(name);
    let engine = app.document_state.engines.get_mut(&path).unwrap();
    let mut doc = engine.document().clone();
    doc.symbols[0].locked = true;
    let (symbol, wire) = (doc.symbols[0].uuid, doc.wires[0].uuid);
    *engine = signex_engine::Engine::new(doc).unwrap();
    (
        app,
        path,
        SelectedItem::new(symbol, SelectedKind::Symbol),
        SelectedItem::new(wire, SelectedKind::Wire),
    )
}

fn move_items(items: Vec<SelectedItem>) -> Command {
    Command::MoveSelection {
        items,
        dx: 5.0,
        dy: 0.0,
        keep_wires: false,
        wiring: signex_types::corner::DragWiringMode::Auto,
    }
}

#[test]
fn the_core_half_alone_leaves_an_edited_sheet_dirty() {
    let (mut app, path) = colours::app_with_sheet("core-half-dirty.snxsch");
    assert!(!dirty(&app, &path));
    let edit =
        app.document_state
            .execute_on_edit_path(paper("A3"), true, ids::NOTICE_EDIT_COMMAND_FAILED);
    assert_eq!(edit.outcome, MutationOutcome::Changed);
    // Nothing projected: the mark is the core half's alone.
    assert!(dirty(&app, &path), "close-confirm must ask for this sheet");
}

#[test]
fn an_edit_returns_one_document_changed_with_its_patch_and_edit_cause() {
    let (mut app, path) = colours::app_with_sheet("core-half-event.snxsch");
    let edit =
        app.document_state
            .execute_on_edit_path(paper("A3"), true, ids::NOTICE_EDIT_COMMAND_FAILED);
    assert_eq!(edit.events.len(), 1, "{:?}", edit.events);
    let changed = changes(&edit.events);
    assert_eq!(changed[0].path, path);
    assert_eq!(changed[0].cause, ChangeCause::Edit);
    assert!(changed[0].patch.intersects(DocumentPatch::PAPER));
}

#[test]
fn an_undo_returns_a_step_cause() {
    let (mut app, path) = colours::app_with_sheet("core-half-step.snxsch");
    // The loaded engine's position becomes the save point.
    app.document_state.mark_document_clean(&path);
    let _edit =
        app.document_state
            .execute_on_edit_path(paper("A3"), true, ids::NOTICE_EDIT_COMMAND_FAILED);
    assert!(dirty(&app, &path));
    let step = app
        .document_state
        .step_active_history(true)
        .expect("an active engine");
    assert!(step.stepped);
    let changed = changes(&step.events);
    assert_eq!(changed.len(), 1, "{:?}", step.events);
    assert_eq!(changed[0].path, path);
    assert_eq!(changed[0].cause, ChangeCause::Step);
    assert!(!dirty(&app, &path), "back at the save point");
}

#[test]
fn an_edit_inside_an_agent_run_owes_its_patch_emits_no_document_event_and_keeps_its_notices() {
    let (mut app, path, symbol, wire) = with_locked_symbol("core-half-owed.snxsch");
    assert!(!dirty(&app, &path));
    app.document_state.agent_edits.target = Some(path.clone());
    let edit = app.document_state.execute_on_edit_path(
        move_items(vec![symbol, wire]),
        true,
        ids::NOTICE_EDIT_COMMAND_FAILED,
    );
    assert_eq!(edit.outcome, MutationOutcome::Changed);
    assert!(changes(&edit.events).is_empty(), "{:?}", edit.events);
    assert_eq!(notice_levels(&edit.events), [NoticeLevel::Warning]);
    assert_ne!(app.document_state.agent_edits.owed, DocumentPatch::NONE);
    assert!(!dirty(&app, &path), "the run marks it dirty at its end");
}

#[test]
fn the_active_tail_with_a_none_patch_neither_syncs_dirty_nor_emits() {
    let (mut app, path) = colours::app_with_sheet("core-half-none.snxsch");
    // Held dirty outside the history: a sync at the save point would clear it.
    app.document_state.keep_document_dirty_until_saved(&path);
    let edit = app
        .document_state
        .settle_active_sheet(DocumentPatch::NONE, ChangeCause::Edit);
    assert_eq!(edit.outcome, MutationOutcome::Unchanged);
    assert!(edit.events.is_empty(), "{:?}", edit.events);
    assert!(dirty(&app, &path));
}

#[test]
fn the_by_path_tail_with_a_none_patch_still_emits() {
    let (mut app, path) = colours::app_with_sheet("core-half-by-path-none.snxsch");
    let events = app
        .document_state
        .settle_sheet_at(&path, DocumentPatch::NONE, ChangeCause::Edit);
    let changed = changes(&events);
    assert_eq!(changed.len(), 1, "{events:?}");
    assert_eq!(changed[0].patch, DocumentPatch::NONE);
}

#[test]
fn notices_come_back_in_raised_order() {
    let (mut app, path, symbol, wire) = with_locked_symbol("core-half-order.snxsch");
    // A junction the sheet already holds: the engine refuses the batch.
    let junction = app.document_state.engines[&path].document().junctions[0].clone();
    let edit = app.document_state.execute_batch_on_edit_path(vec![
        move_items(vec![symbol, wire]),
        Command::PlaceJunction { junction },
    ]);
    assert_eq!(edit.outcome, MutationOutcome::Failed);
    // The skip is raised before the engine runs, the refusal after.
    assert_eq!(
        notice_levels(&edit.events),
        [NoticeLevel::Warning, NoticeLevel::Error]
    );
    assert!(changes(&edit.events).is_empty());
}

#[test]
fn an_undo_with_no_active_engine_still_sets_the_library_drift_check() {
    let (mut app, _task) = Signex::new();
    app.library.drift.check_pending = false;
    assert!(!app.apply_engine_undo(true));
    assert!(app.library.drift.check_pending);
}

/// A step whose refresh the core half does not show (here owed to an agent's
/// run) still asks for the drift check: the history moved all the same.
#[test]
fn an_undo_the_active_tail_does_not_show_still_sets_the_library_drift_check() {
    let (mut app, path) = colours::app_with_sheet("core-half-owed-step.snxsch");
    assert!(app.apply_engine_command(paper("A3"), true, true));
    app.document_state.agent_edits.target = Some(path);
    app.library.drift.check_pending = false;
    assert!(app.apply_engine_undo(true));
    assert!(app.library.drift.check_pending);
}

/// `path`'s engine with one paper edit on top, recorded under a new group.
fn grouped_top_step(app: &mut Signex, path: &Path) -> signex_engine::UndoGroupId {
    let group = app.mint_undo_group();
    let engine = app.document_state.engines.get_mut(path).unwrap();
    engine.begin_transaction().unwrap();
    engine.execute(paper("A3")).unwrap();
    engine.commit_transaction(Some(group)).unwrap();
    group
}

#[test]
fn a_step_by_path_onto_a_project_wide_run_is_refused_with_its_reason() {
    let (mut app, path) = colours::app_with_sheet("core-half-run-step.snxsch");
    grouped_top_step(&mut app, &path);
    let before = app.document_state.engines[&path].undo_position();
    assert_eq!(
        app.step_sheet_on(&path, true),
        Err(
            "the next step belongs to a project-wide run (annotate, replace or re-prefix) \
             and moves other sheets too; undo it from the sheet it was run on"
                .to_string()
        )
    );
    assert_eq!(app.document_state.engines[&path].undo_position(), before);
}

#[test]
fn a_step_by_path_onto_an_agents_step_is_refused_with_its_reason() {
    let (mut app, path) = colours::app_with_sheet("core-half-agent-step.snxsch");
    let group = grouped_top_step(&mut app, &path);
    let origin = app.agent.origin.clone();
    app.document_state
        .agent_edits
        .record_step(group, "Agent: test".to_string(), &origin);
    let before = app.document_state.engines[&path].undo_position();
    assert_eq!(
        app.step_sheet_on(&path, true),
        Err("the next step is an agent's edit step; take it back with `revert`".to_string())
    );
    assert_eq!(app.document_state.engines[&path].undo_position(), before);
}
