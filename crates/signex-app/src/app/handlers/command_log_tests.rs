//! Replay on recorded targets (Caner 2026-11-22, mockup row 161).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Map;
use signex_engine::Command;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind, Symbol};

use super::replay::{ContinuesAs, ReplayScope, replay_scope};
use crate::app::command::CommandArgs;
use crate::app::{Signex, TabInfo, TabKind};
use crate::keymap::AppCommandId;
use crate::library::editor::symbol::state::SymbolSelection;
use crate::library::messages::{PrimitiveEdit, SymbolEditorMsg};
use crate::panels::command_log::{CommandLogMsg, LogOutcome, LogRow, LogSource};

fn part(reference: &str, id: u128) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::from_u128(id),
        lib_id: "lib:r".to_string(),
        reference: reference.to_string(),
        unit: 1,
        in_bom: true,
        on_board: true,
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
fn each_command_family_has_its_replay_scope() {
    let continues = |takes| ReplayScope::Continues(takes);
    assert_eq!(replay_scope("move_selection"), continues(ContinuesAs::Move));
    assert_eq!(replay_scope("create_union"), continues(ContinuesAs::Union));
    assert_eq!(
        replay_scope("change_footprint"),
        continues(ContinuesAs::Other)
    );
    assert_eq!(replay_scope("duplicate"), ReplayScope::OnTargets);
    assert_eq!(replay_scope("delete_selection"), ReplayScope::OnTargets);
    assert_eq!(replay_scope("select_connection"), ReplayScope::SelectsFrom);
    assert_eq!(replay_scope("select_all"), ReplayScope::SetsSelection);
    assert_eq!(replay_scope("place_wire"), ReplayScope::NoTarget);
    assert_eq!(
        replay_scope("remove_last_vertex"),
        ReplayScope::NothingToRepeat
    );
}

fn position(app: &Signex, path: &Path, id: u128) -> (f64, f64) {
    let symbol = app.document_state.engines[path]
        .document()
        .symbols
        .iter()
        .find(|symbol| symbol.uuid == uuid::Uuid::from_u128(id))
        .expect("the part");
    (symbol.position.x, symbol.position.y)
}

/// What a finished drag or Move runs through the gateway.
fn move_part(app: &mut Signex, id: u128, dx: f64, dy: f64) {
    let moved = app.apply_engine_command(
        Command::MoveSelection {
            items: vec![item(id)],
            dx,
            dy,
            keep_wires: false,
            wiring: signex_types::corner::DragWiringMode::Auto,
        },
        true,
        true,
    );
    assert!(moved, "the move landed");
}

#[test]
fn a_move_replays_its_offset_on_its_item_while_another_is_selected() {
    let (mut app, path) = app_with_parts();
    select(&mut app, &[1]);
    move_part(&mut app, 1, 2.54, 5.08);
    let row = newest(&app);
    assert_eq!(row.command, "move_selection");
    let (x, y) = position(&app, &path, 1);
    let other = position(&app, &path, 2);
    select(&mut app, &[2]);

    replay(&mut app, row.seq);

    assert_eq!(position(&app, &path, 1), (x + 2.54, y + 5.08));
    assert_eq!(
        position(&app, &path, 2),
        other,
        "the selection did not move"
    );
    assert_eq!(selected(&app), vec![item(2)]);
    assert_eq!(newest(&app).outcome, LogOutcome::Ok(None));
}

#[test]
fn a_finished_move_tool_is_one_row_with_its_input() {
    let (mut app, path) = app_with_parts();
    select(&mut app, &[1]);
    run(&mut app, "move_selection");
    let started = newest(&app).seq;
    move_part(&mut app, 1, 2.54, 0.0);
    assert_eq!(newest(&app).seq, started, "the drop adds no second row");
    let (x, y) = position(&app, &path, 1);
    replay(&mut app, started);
    assert_eq!(position(&app, &path, 1), (x + 2.54, y));
}

#[test]
fn a_cancelled_move_tool_has_nothing_to_replay() {
    let (mut app, path) = app_with_parts();
    select(&mut app, &[1]);
    run(&mut app, "move_selection");
    let started = newest(&app).seq;
    app.cancel_move_pick();
    let before = position(&app, &path, 1);
    replay(&mut app, started);
    assert!(failure(&newest(&app)).contains("never finished"));
    assert_eq!(position(&app, &path, 1), before);
}

#[test]
fn a_duplicate_replays_on_its_target_and_keeps_the_clipboard() {
    let (mut app, path) = app_with_parts();
    select(&mut app, &[1]);
    run(&mut app, "duplicate");
    let seq = newest(&app).seq;
    let parts = app.document_state.engines[&path].document().symbols.len();
    app.interaction_state.clipboard = None;
    app.interaction_state.clipboard_text = Some("kept".to_string());
    select(&mut app, &[2]);

    replay(&mut app, seq);

    let sheet = app.document_state.engines[&path].document();
    assert_eq!(sheet.symbols.len(), parts + 1, "one new copy");
    assert_eq!(selected(&app), vec![item(2)]);
    assert!(app.interaction_state.clipboard.is_none());
    assert_eq!(
        app.interaction_state.clipboard_text.as_deref(),
        Some("kept")
    );
}

#[test]
fn a_properties_edit_is_one_row_per_field_run_and_replays_its_value() {
    let (mut app, path) = app_with_parts();
    select(&mut app, &[1]);
    let dnp = |dnp| Command::SetSymbolDnp {
        symbol_id: uuid::Uuid::from_u128(1),
        dnp,
    };
    assert!(app.apply_engine_command(dnp(true), true, true));
    let row = newest(&app);
    assert_eq!(row.command, "edit_object_properties");
    let rows = app.ui_state.command_log.log.rows().count();
    assert!(app.apply_engine_command(dnp(false), true, true));
    assert!(app.apply_engine_command(dnp(true), true, true));
    assert_eq!(
        app.ui_state.command_log.log.rows().count(),
        rows,
        "one field's run is one row"
    );
    assert!(app.apply_engine_command(dnp(false), true, true));
    undo(&mut app, &path);
    let is_dnp = |app: &Signex| {
        app.document_state.engines[&path]
            .document()
            .symbols
            .iter()
            .find(|s| s.uuid == uuid::Uuid::from_u128(1))
            .map(|s| s.dnp)
    };
    assert_eq!(is_dnp(&app), Some(true));
    select(&mut app, &[2]);

    replay(&mut app, row.seq);

    assert_eq!(is_dnp(&app), Some(false), "the last value set");
    assert_eq!(selected(&app), vec![item(2)]);
}

#[test]
fn a_union_replays_with_a_new_id() {
    let (mut app, path) = app_with_parts();
    select(&mut app, &[1, 2]);
    let id = uuid::Uuid::from_u128(77);
    let created = app.apply_engine_command(
        Command::CreateUnion {
            union: signex_types::schematic::SchUnion {
                uuid: id,
                name: "U".to_string(),
                tags: Vec::new(),
                members: vec![uuid::Uuid::from_u128(1), uuid::Uuid::from_u128(2)],
                locked: false,
            },
        },
        true,
        true,
    );
    assert!(created);
    let row = newest(&app);
    assert_eq!(row.command, "create_union");
    undo(&mut app, &path);

    replay(&mut app, row.seq);

    let unions = &app.document_state.engines[&path].document().unions;
    assert_eq!(unions.len(), 1);
    assert_ne!(unions[0].uuid, id, "a replay makes its own union");
}

#[test]
fn replay_onto_a_sheet_behind_a_running_tool_is_refused() {
    let (mut app, first) = app_with_parts();
    select(&mut app, &[1]);
    move_part(&mut app, 1, 2.54, 0.0);
    let seq = newest(&app).seq;
    let moved = position(&app, &first, 1);
    let second = PathBuf::from("replay-b.snxsch");
    add_sheet(&mut app, &second, &[("R9", 9)]);
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.sync_active_tab();
    app.interaction_state.wire_drawing = true;

    replay(&mut app, seq);

    assert!(failure(&newest(&app)).contains("tool is in progress"));
    assert_eq!(position(&app, &first, 1), moved);
    assert_eq!(app.active_tab_path(), Some(second));
    assert!(app.interaction_state.wire_drawing, "the tool is untouched");
}

/// A symbol tab with pins `1` and `2`.
fn app_with_symbol() -> (Signex, PathBuf) {
    let (mut app, _boot) = Signex::new();
    let path = PathBuf::from("replay.snxsym");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("S"));
    app.document_state.symbol_editors.insert(
        path.clone(),
        crate::app::SymbolDocument::new(path.clone(), file),
    );
    for (at, number) in [(0.0, "1"), (5.08, "2")] {
        symbol_edit(&mut app, &path, SymbolEditorMsg::AddPin { x: at, y: 0.0 });
        let idx = pins(&app, &path).len() - 1;
        symbol_edit(
            &mut app,
            &path,
            SymbolEditorMsg::SetPinNumber {
                idx,
                number: number.to_string(),
            },
        );
    }
    (app, path)
}

fn symbol_edit(app: &mut Signex, path: &Path, msg: SymbolEditorMsg) {
    let _ = app.handle_primitive_editor_event(path.to_path_buf(), PrimitiveEdit::Symbol(msg));
}

fn pins(app: &Signex, path: &Path) -> Vec<String> {
    app.document_state.symbol_editors[path]
        .primitive()
        .pins
        .iter()
        .map(|pin| pin.number.clone())
        .collect()
}

fn pick_pin(app: &mut Signex, path: &Path, index: usize) {
    let editor = app
        .document_state
        .symbol_editors
        .get_mut(path)
        .expect("the editor");
    editor.selected = Some(SymbolSelection::Pin(index));
}

#[test]
fn a_symbol_editor_delete_replays_on_its_pin_not_the_one_selected() {
    let (mut app, path) = app_with_symbol();
    pick_pin(&mut app, &path, 0);
    symbol_edit(&mut app, &path, SymbolEditorMsg::DeleteSelected);
    assert_eq!(pins(&app, &path), ["2"]);
    let seq = newest(&app).seq;
    symbol_edit(&mut app, &path, SymbolEditorMsg::Undo);
    assert_eq!(pins(&app, &path), ["1", "2"]);
    pick_pin(&mut app, &path, 1);

    replay(&mut app, seq);

    assert_eq!(pins(&app, &path), ["2"], "pin 1, the recorded target, went");
}

#[test]
fn a_symbol_editor_replay_is_refused_when_its_pin_is_gone() {
    let (mut app, path) = app_with_symbol();
    pick_pin(&mut app, &path, 0);
    symbol_edit(&mut app, &path, SymbolEditorMsg::DeleteSelected);
    let seq = newest(&app).seq;

    replay(&mut app, seq);

    assert_eq!(pins(&app, &path), ["2"], "nothing else was deleted");
    assert!(failure(&newest(&app)).contains("Pin 1"));
}

fn board_with_two_parts() -> signex_types::pcb::PcbBoard {
    let footprint = |id: u128, reference: &str, x: f64| {
        serde_json::json!({
            "uuid": uuid::Uuid::from_u128(id), "reference": reference,
            "position": {"x": x, "y": 10.0}, "rotation": 0.0, "layer": "TopCopper",
            "pads": [{"uuid": uuid::Uuid::from_u128(id + 100), "number": "1",
                "pad_type": "smd", "shape": "rect", "position": {"x": 0.0, "y": 0.0},
                "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"],
                "net": null}]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::from_u128(500),
        "setup": {"grid_size": 1.0, "trace_width": 0.3, "via_diameter": 0.8,
                  "via_drill": 0.4, "clearance": 0.2},
        "nets": [],
        "footprints": [footprint(1, "U1", 10.0), footprint(2, "U2", 30.0)]
    }))
    .expect("a board")
}

fn footprint_x(app: &Signex, path: &Path, id: u128) -> f64 {
    app.document_state
        .pcb_board(path)
        .expect("the board")
        .footprints
        .iter()
        .find(|fp| fp.uuid == uuid::Uuid::from_u128(id))
        .expect("the footprint")
        .position
        .x
}

#[test]
fn a_board_move_replays_its_offset_while_another_part_is_selected() {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = crate::app::handlers::menu::export::tests::app_workspace(
        &dir.path().to_string_lossy(),
        &[],
    );
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board_with_two_parts());
    let moved = std::collections::BTreeSet::from([uuid::Uuid::from_u128(1)]);
    let delta = signex_types::pcb::Point::new(2.0, 0.0);
    let built = signex_engine::pcb::placement::move_commands(
        app.document_state.pcb_board(&path).expect("the board"),
        &moved,
        delta,
    );
    app.ui_state
        .command_replay
        .note_board_drop(moved, delta, false);
    assert!(app.run_placement("Move", built));
    let row = newest(&app);
    assert_eq!(row.command, "move_selection");
    assert_eq!(footprint_x(&app, &path, 1), 12.0);
    app.select_pcb_items(&[pcb_geom::ItemRef::Footprint(uuid::Uuid::from_u128(2))]);

    replay(&mut app, row.seq);

    assert_eq!(footprint_x(&app, &path, 1), 14.0, "by the offset again");
    assert_eq!(footprint_x(&app, &path, 2), 30.0);
    let still = app.interaction_state.pcb_selection.selection();
    assert!(still.contains(pcb_geom::ItemRef::Footprint(uuid::Uuid::from_u128(2))));
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
