//! Replay on recorded targets (Caner 2026-11-22, mockup row 161).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Map;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind, Symbol};

use super::replay::{ReplayScope, replay_scope};
use crate::app::command::CommandArgs;
use crate::app::{Signex, TabInfo, TabKind};
use crate::keymap::AppCommandId;
use crate::panels::command_log::{CommandLogMsg, LogOutcome, LogRow, LogSource};

fn part(reference: &str, id: u128) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::from_u128(id),
        reference: reference.to_string(),
        ..Symbol::default()
    }
}

fn item(id: u128) -> SelectedItem {
    SelectedItem::new(uuid::Uuid::from_u128(id), SelectedKind::Symbol)
}

fn add_sheet(app: &mut Signex, path: &Path, ids: &[(&str, u128)]) {
    let sheet = SchematicSheet {
        symbols: ids.iter().map(|(r, id)| part(r, *id)).collect(),
        ..SchematicSheet::default()
    };
    let engine =
        signex_engine::Engine::new_with_path(sheet, Some(path.to_path_buf())).expect("an engine");
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: path.display().to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
}

/// One open sheet in view holding R1 (id 1) and R2 (id 2).
fn app_with_parts() -> (Signex, PathBuf) {
    let (mut app, _boot) = Signex::new();
    let path = PathBuf::from("replay-a.snxsch");
    add_sheet(&mut app, &path, &[("R1", 1), ("R2", 2)]);
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.sync_active_tab();
    (app, path)
}

fn select(app: &mut Signex, ids: &[u128]) {
    app.interaction_state.active_canvas_mut().selected = ids.iter().copied().map(item).collect();
}

fn selected(app: &Signex) -> Vec<SelectedItem> {
    app.interaction_state.active_canvas().selected.clone()
}

fn has(app: &Signex, path: &Path, id: u128) -> bool {
    app.document_state.engines[path]
        .document()
        .symbols
        .iter()
        .any(|symbol| symbol.uuid == uuid::Uuid::from_u128(id))
}

fn run(app: &mut Signex, id: &str) {
    let command = AppCommandId::new(id).expect("a catalog id");
    let _ = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
}

fn replay(app: &mut Signex, seq: u64) {
    let _ = app.handle_command_log_message(CommandLogMsg::ReplayRequested(seq));
}

fn newest(app: &Signex) -> LogRow {
    app.ui_state
        .command_log
        .log
        .rows()
        .next_back()
        .expect("a row")
        .clone()
}

fn undo(app: &mut Signex, path: &Path) {
    app.document_state
        .engines
        .get_mut(path)
        .expect("the sheet")
        .undo()
        .expect("an undo step");
}

fn failure(row: &LogRow) -> &str {
    match &row.outcome {
        LogOutcome::Failed(reason) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_delete_records_its_targets_in_the_row() {
    let (mut app, path) = app_with_parts();
    select(&mut app, &[1]);
    run(&mut app, "delete_selection");
    let row = newest(&app);
    assert_eq!(row.document.as_ref(), Some(&path));
    let json = row.arguments_json().expect("the detail shows them");
    assert!(
        json.contains(&uuid::Uuid::from_u128(1).to_string()),
        "{json}"
    );
    assert!(!row.has_given_operands());
}

#[test]
fn replay_acts_on_the_targets_and_puts_the_selection_back_as_one_undo_step() {
    let (mut app, path) = app_with_parts();
    select(&mut app, &[1]);
    run(&mut app, "delete_selection");
    let seq = newest(&app).seq;
    undo(&mut app, &path);
    select(&mut app, &[2]);
    let depth = app.document_state.engines[&path].undo_depth();

    replay(&mut app, seq);

    assert!(!has(&app, &path, 1), "the recorded target was deleted");
    assert!(has(&app, &path, 2), "the current selection was not");
    assert_eq!(selected(&app), vec![item(2)], "and it is selected still");
    assert_eq!(app.document_state.engines[&path].undo_depth(), depth + 1);
    let row = newest(&app);
    assert!(row.seq > seq);
    assert_eq!(row.source, LogSource::Gui { window: None });
    assert_eq!(row.outcome, LogOutcome::Ok(None));
}

#[test]
fn a_target_that_is_gone_refuses_the_whole_replay() {
    let (mut app, path) = app_with_parts();
    select(&mut app, &[1, 2]);
    run(&mut app, "delete_selection");
    let seq = newest(&app).seq;
    undo(&mut app, &path);
    select(&mut app, &[1]);
    run(&mut app, "delete_selection");
    select(&mut app, &[2]);
    let revision = app.document_state.engines[&path].revision();

    replay(&mut app, seq);

    assert!(has(&app, &path, 2), "the survivor is not acted on");
    assert_eq!(app.document_state.engines[&path].revision(), revision);
    assert_eq!(selected(&app), vec![item(2)]);
    let row = newest(&app);
    let reason = failure(&row);
    assert!(
        reason.contains(&uuid::Uuid::from_u128(1).to_string()),
        "{reason}"
    );
    assert!(reason.contains("replay-a.snxsch"), "{reason}");
}

#[test]
fn a_closed_document_refuses_the_replay() {
    let (mut app, path) = app_with_parts();
    select(&mut app, &[1]);
    run(&mut app, "delete_selection");
    let seq = newest(&app).seq;
    app.document_state.tabs.retain(|tab| tab.path != path);
    app.document_state.active_tab = 0;

    replay(&mut app, seq);

    let row = newest(&app);
    let reason = failure(&row);
    assert!(reason.contains("is not open"), "{reason}");
    assert!(reason.contains("replay-a.snxsch"), "{reason}");
}

#[test]
fn replay_reaches_a_sheet_behind_the_one_in_view_and_comes_back() {
    let (mut app, first) = app_with_parts();
    select(&mut app, &[1]);
    run(&mut app, "delete_selection");
    let seq = newest(&app).seq;
    undo(&mut app, &first);
    let second = PathBuf::from("replay-b.snxsch");
    add_sheet(&mut app, &second, &[("R9", 9)]);
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.sync_active_tab();
    select(&mut app, &[9]);

    replay(&mut app, seq);

    assert!(!has(&app, &first, 1), "it ran on its own sheet");
    assert!(has(&app, &second, 9));
    assert_eq!(app.active_tab_path(), Some(second));
    assert_eq!(selected(&app), vec![item(9)]);
}

#[test]
fn a_command_that_keeps_working_on_the_selection_is_refused() {
    assert_eq!(replay_scope("move_selection"), ReplayScope::LiveSelection);
    assert_eq!(replay_scope("duplicate"), ReplayScope::LiveSelection);
    assert_eq!(replay_scope("delete_selection"), ReplayScope::OnTargets);
    assert_eq!(replay_scope("select_all"), ReplayScope::SetsSelection);
    assert_eq!(replay_scope("place_wire"), ReplayScope::NoTarget);

    let (mut app, path) = app_with_parts();
    let seq = app
        .record_command(
            "move_selection",
            Map::new(),
            LogOutcome::Ok(None),
            Duration::ZERO,
            LogSource::Gui { window: None },
            Some(path),
        )
        .expect("logged");
    select(&mut app, &[2]);
    replay(&mut app, seq);
    assert!(failure(&newest(&app)).contains("recorded targets"));
    assert_eq!(selected(&app), vec![item(2)]);
}

#[test]
fn a_targeted_row_with_no_recorded_targets_is_refused() {
    let (mut app, path) = app_with_parts();
    let seq = app
        .record_command(
            "delete_selection",
            Map::new(),
            LogOutcome::Ok(None),
            Duration::ZERO,
            LogSource::Gui { window: None },
            Some(path.clone()),
        )
        .expect("logged");
    select(&mut app, &[2]);
    replay(&mut app, seq);
    assert!(has(&app, &path, 2), "the current selection is not taken");
    assert!(failure(&newest(&app)).contains("recorded no targets"));
}

/// `cycle_selection_mode` has no target and needs no document: it runs again.
#[test]
fn a_command_with_no_target_runs_again_as_it_is() {
    let (mut app, _boot) = Signex::new();
    let mode = app.ui_state.selection_mode;
    let seq = app
        .record_command(
            "cycle_selection_mode",
            Map::new(),
            LogOutcome::Ok(None),
            Duration::ZERO,
            LogSource::Gui { window: None },
            None,
        )
        .expect("logged");

    let _ = app.handle_command_log_message(CommandLogMsg::ReplayRequested(seq));

    assert_ne!(app.ui_state.selection_mode, mode, "the command ran");
    let log = &app.ui_state.command_log.log;
    assert_eq!(log.rows().count(), 2, "the replay is a new row");
    let again = log.rows().next_back().expect("the new row");
    assert_eq!(again.command, "cycle_selection_mode");
    assert_eq!(again.source, LogSource::Gui { window: None });
    assert_eq!(again.outcome, LogOutcome::Ok(None));
}
