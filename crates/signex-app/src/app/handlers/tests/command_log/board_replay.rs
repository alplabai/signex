//! DECISIONS 2026-11-22 (#2840): a recorded board edit is refused once its
//! target changed, and nothing runs.

use std::path::{Path, PathBuf};

use std::time::Duration;

use signex_engine::pcb::{BoardCommand, BoardItem};
use signex_types::pcb::{PadShape, PcbBoard, Point, Segment};
use signex_types::pcb_stack::LayerStack;

use super::super::operands_of;
use super::super::replay::{SAFE_COMMANDS, replay_proof};
use super::fixtures::*;
use crate::app::Signex;
use crate::app::command::CommandArgs;
use crate::keymap::AppCommandId;
use crate::panels::command_log::{LogOutcome, LogSource};

/// Four copper layers and a track on Inner 1.
fn four_layer_board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::from_u128(1),
        "thickness": 1.6,
        "layers": [
            {"id": 0, "name": "TopCopper", "layer_type": "signal"},
            {"id": 1, "name": "Inner1Copper", "layer_type": "signal"},
            {"id": 2, "name": "Inner2Copper", "layer_type": "signal"},
            {"id": 31, "name": "BottomCopper", "layer_type": "signal"},
        ],
        "segments": [{
            "uuid": uuid::Uuid::from_u128(FIRST), "start": {"x": 0.0, "y": 0.0},
            "end": {"x": 5.0, "y": 0.0}, "width": 0.25, "layer": "Inner1Copper"
        }],
    }))
    .expect("a board")
}

fn app_with_four_layers(dir: &Path) -> (Signex, PathBuf) {
    let mut app =
        crate::app::handlers::menu::export::tests::app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), four_layer_board());
    (app, path)
}

/// The operands of `set_layer_stack` with the two inner copper rows swapped.
fn swapped_stack_args(app: &Signex, path: &Path) -> CommandArgs {
    let board = app.document_state.pcb_board(path).expect("the board");
    let mut stack = LayerStack::derived(&board.layers, 1.6);
    let copper: Vec<usize> = (stack.layers.iter().enumerate())
        .filter(|(_, row)| row.kind.is_copper())
        .map(|(at, _)| at)
        .collect();
    stack.layers.swap(copper[1], copper[2]);
    let document = format!("document={}", path.display());
    let rows = format!("stack={}", serde_json::to_string(&stack).expect("json"));
    let command = AppCommandId::new("set_layer_stack").expect("an id");
    CommandArgs::parse_operands(&command, &[document.as_str(), rows.as_str()]).expect("operands")
}

/// Run `id` from `args` as a logged command: its row's number.
fn run_logged(app: &mut Signex, id: &str, args: &CommandArgs) -> u64 {
    let command = AppCommandId::new(id).expect("an id");
    let _ = app.logged_command(&command, operands_of(args), None, |app| {
        app.run_operands(&command, args)
    });
    let row = newest(app);
    assert!(!row.outcome.is_failed(), "{:?}", row.outcome);
    row.seq
}

fn stack_of(app: &Signex, path: &Path) -> Option<LayerStack> {
    (app.document_state.pcb_board(path))
        .expect("the board")
        .stackup
        .clone()
}

/// The re-review's case: a rerun would swap the copper back under rows that
/// stay swapped.
#[test]
fn a_layer_stack_replay_on_its_own_result_changes_nothing() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_four_layers(dir.path());
    let args = swapped_stack_args(&app, &path);
    let seq = run_logged(&mut app, "set_layer_stack", &args);
    assert_eq!(track(&app, &path, FIRST).layer, "Inner2Copper");
    let stack = stack_of(&app, &path);

    replay(&mut app, seq);

    assert_eq!(track(&app, &path, FIRST).layer, "Inner2Copper");
    assert_eq!(stack_of(&app, &path), stack);
    assert!(failure(&newest(&app)).contains("changed after"));
}

#[test]
fn a_layer_stack_replays_once_after_an_undo_and_not_twice() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_four_layers(dir.path());
    let args = swapped_stack_args(&app, &path);
    let seq = run_logged(&mut app, "set_layer_stack", &args);
    assert!(app.step_board_history(&path, true));
    assert_eq!(track(&app, &path, FIRST).layer, "Inner1Copper");

    replay(&mut app, seq);

    assert_eq!(track(&app, &path, FIRST).layer, "Inner2Copper");
    let replayed = newest(&app);
    assert!(!replayed.outcome.is_failed(), "{:?}", replayed.outcome);

    replay(&mut app, replayed.seq);

    assert_eq!(track(&app, &path, FIRST).layer, "Inner2Copper");
    assert!(failure(&newest(&app)).contains("changed after"));
}

fn pad_shape_of(app: &Signex, path: &Path) -> PadShape {
    let board = app.document_state.pcb_board(path).expect("the board");
    board.footprints[0].pads[0].shape
}

fn pad_shape_args(path: &Path, shape: &str) -> CommandArgs {
    let command = AppCommandId::new("set_pad_shape").expect("an id");
    let document = format!("document={}", path.display());
    let ids = format!("ids={}", uuid::Uuid::from_u128(101));
    let shape = format!("shape={shape}");
    CommandArgs::parse_operands(&command, &[document.as_str(), ids.as_str(), shape.as_str()])
        .expect("operands")
}

#[test]
fn a_value_operand_is_harmless_on_its_result_and_refused_once_changed() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    let seq = run_logged(&mut app, "set_pad_shape", &pad_shape_args(&path, "circle"));

    replay(&mut app, seq);
    assert_eq!(pad_shape_of(&app, &path), PadShape::Circle);

    run_logged(&mut app, "set_pad_shape", &pad_shape_args(&path, "oval"));
    replay(&mut app, seq);

    assert_eq!(pad_shape_of(&app, &path), PadShape::Oval, "the later edit");
    assert!(failure(&newest(&app)).contains("changed after"));
}

/// An agent's row records no board state; its refusal must not turn into a
/// failed row that then runs.
#[test]
fn an_operand_row_with_no_recorded_state_stays_refused() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_four_layers(dir.path());
    let args = swapped_stack_args(&app, &path);
    let seq = app
        .record_command(
            "set_layer_stack",
            operands_of(&args),
            LogOutcome::Ok(None),
            Duration::ZERO,
            LogSource::Gui { window: None },
            Some(path.clone()),
        )
        .expect("logged");

    replay(&mut app, seq);
    let refused = newest(&app);
    assert!(failure(&refused).contains("recorded no state"));
    replay(&mut app, refused.seq);

    assert_eq!(track(&app, &path, FIRST).layer, "Inner1Copper");
    assert!(failure(&newest(&app)).contains("recorded no state"));
}

/// A row with these operands on the board at `path`.
fn logged(app: &mut Signex, path: &Path, id: &str, operands: &[&str]) -> u64 {
    let document = format!("document={}", path.display());
    let mut all = vec![document.as_str()];
    all.extend(operands);
    let command = AppCommandId::new(id).expect("an id");
    let args = CommandArgs::parse_operands(&command, &all).expect("operands");
    run_logged(app, id, &args)
}

const COLOUR: &str = "color=#112233";
const OTHER_COLOUR: &str = "color=#445566";
const TOP: &str = "layer=TopCopper";
const COLUMNS: &str = "field=columns";

/// Run the listed command `id`, then change what it wrote: its row. A listed
/// command with no arm here fails the test below.
fn run_then_change_its_target(app: &mut Signex, path: &Path, id: &str) -> u64 {
    let track_id = format!("ids={}", uuid::Uuid::from_u128(FIRST));
    let ids = track_id.as_str();
    match id {
        "edit_object_properties" => {
            let seq = widen(app, path, &[FIRST]);
            move_track(app, path, FIRST);
            seq
        }
        "lock_board_items" => {
            let seq = logged(app, path, id, &[ids]);
            logged(app, path, "unlock_board_items", &[ids]);
            move_track(app, path, FIRST);
            seq
        }
        "unlock_board_items" => {
            logged(app, path, "lock_board_items", &[ids]);
            let seq = logged(app, path, id, &[ids]);
            move_track(app, path, FIRST);
            seq
        }
        "set_pad_shape" => {
            let seq = run_logged(app, id, &pad_shape_args(path, "circle"));
            run_logged(app, id, &pad_shape_args(path, "oval"));
            seq
        }
        "panelize_board" => {
            let seq = logged(app, path, id, &[]);
            logged(app, path, "set_board_panel", &[COLUMNS, "value=3"]);
            seq
        }
        "remove_board_panel" => {
            logged(app, path, "panelize_board", &[]);
            let seq = logged(app, path, id, &[]);
            logged(app, path, "panelize_board", &[]);
            logged(app, path, "set_board_panel", &[COLUMNS, "value=3"]);
            seq
        }
        "set_board_panel" => {
            logged(app, path, "panelize_board", &[]);
            let seq = logged(app, path, id, &[COLUMNS, "value=3"]);
            logged(app, path, id, &[COLUMNS, "value=4"]);
            seq
        }
        "set_layer_color" => {
            let seq = logged(app, path, id, &[TOP, COLOUR]);
            logged(app, path, id, &[TOP, OTHER_COLOUR]);
            seq
        }
        "reset_layer_colors" => {
            logged(app, path, "set_layer_color", &[TOP, COLOUR]);
            let seq = logged(app, path, id, &[]);
            logged(app, path, "set_layer_color", &[TOP, OTHER_COLOUR]);
            seq
        }
        "set_layer_stack" => {
            let args = swapped_stack_args(app, path);
            let seq = run_logged(app, id, &args);
            let again = swapped_stack_args(app, path);
            run_logged(app, id, &again);
            seq
        }
        other => panic!("{other} is on the safe list with no changed-target test"),
    }
}

/// Move, Delete or Duplicate of part 1 on a sheet, then a later move of it.
fn sheet_row_then_change_its_target(app: &mut Signex, path: &Path, id: &str) -> u64 {
    select(app, &[1]);
    let seq = match id {
        "move_selection" => {
            move_part(app, 1, 2.54, 0.0);
            newest(app).seq
        }
        "delete_selection" => {
            run(app, id);
            let seq = newest(app).seq;
            undo(app, path);
            seq
        }
        "duplicate" => {
            run(app, id);
            newest(app).seq
        }
        other => panic!("{other} has no sheet form here"),
    };
    move_part(app, 1, 2.54, 0.0);
    seq
}

fn refused_on_a_sheet_once_changed(id: &str) {
    let (mut app, path) = app_with_parts();
    let seq = sheet_row_then_change_its_target(&mut app, &path, id);
    let at = position(&app, &path, 1);
    let parts = app.document_state.engines[&path].document().symbols.len();
    let revision = app.document_state.engines[&path].revision();

    replay(&mut app, seq);

    assert!(failure(&newest(&app)).contains("changed after"), "{id}");
    assert_eq!(position(&app, &path, 1), at, "{id} moved the part");
    let engine = &app.document_state.engines[&path];
    assert_eq!(engine.document().symbols.len(), parts, "{id}");
    assert_eq!(engine.revision(), revision, "{id}");
}

const PART: u128 = 1;

/// The offset drop of a board move, as the canvas runs it.
fn drop_part(app: &mut Signex, path: &Path, dx: f64) {
    let moved = std::collections::BTreeSet::from([uuid::Uuid::from_u128(PART)]);
    let delta = Point::new(dx, 0.0);
    let corner = pcb_geom::stretch_rule(Default::default());
    let built = signex_engine::pcb::placement::move_commands(
        app.document_state.pcb_board(path).expect("the board"),
        &moved,
        delta,
        corner,
        &mut signex_types::object_id::UuidV7Ids,
    );
    app.ui_state
        .command_replay
        .note_board_drop(moved, delta, false, corner);
    assert!(app.run_placement("Move", built));
}

fn select_part(app: &mut Signex) {
    let part = pcb_geom::ItemRef::Footprint(uuid::Uuid::from_u128(PART));
    app.select_pcb_items(&[part]);
}

fn footprints_on(app: &Signex, path: &Path) -> usize {
    let board = app.document_state.pcb_board(path).expect("the board");
    board.footprints.len()
}

/// Move, Delete or Duplicate of a part on a board, then a later move of it.
fn board_row_then_change_its_target(app: &mut Signex, path: &Path, id: &str) -> u64 {
    let seq = match id {
        "move_selection" => {
            drop_part(app, path, 2.0);
            newest(app).seq
        }
        "delete_selection" => {
            select_part(app);
            run(app, id);
            let seq = newest(app).seq;
            assert!(app.step_board_history(path, true));
            seq
        }
        "duplicate" => {
            select_part(app);
            run(app, id);
            newest(app).seq
        }
        other => panic!("{other} has no board form here"),
    };
    drop_part(app, path, 3.0);
    seq
}

fn refused_on_a_board_once_changed(id: &str) {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    let seq = board_row_then_change_its_target(&mut app, &path, id);
    let x = footprint_x(&app, &path, PART);
    let count = footprints_on(&app, &path);
    let revision = |app: &Signex| {
        let engine = app.document_state.board_engine(&path);
        engine.expect("the board").revision()
    };
    let before = revision(&app);

    replay(&mut app, seq);

    assert!(failure(&newest(&app)).contains("changed after"), "{id}");
    assert_eq!(footprint_x(&app, &path, PART), x, "{id} moved the part");
    assert_eq!(footprints_on(&app, &path), count, "{id}");
    assert_eq!(revision(&app), before, "{id}");
}

fn footprint_places(app: &Signex, path: &Path) -> Vec<(f64, f64)> {
    let board = app.document_state.pcb_board(path).expect("the board");
    (board.footprints.iter())
        .map(|fp| (fp.position.x, fp.position.y))
        .collect()
}

/// The review's case: a second copy would stack exactly on the first.
#[test]
fn a_board_duplicate_replay_onto_its_own_copy_is_refused() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    select_part(&mut app);
    run(&mut app, "duplicate");
    let seq = newest(&app).seq;
    let before = footprint_places(&app, &path);

    replay(&mut app, seq);

    assert!(failure(&newest(&app)).contains("already holds a copy"));
    assert_eq!(footprint_places(&app, &path), before, "nothing was placed");
}

/// The stack removes Inner 2; a via given a size there since keeps it.
#[test]
fn a_layer_stack_replay_says_so_when_it_would_drop_layer_sizes() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_four_layers(dir.path());
    let board = app.document_state.pcb_board(&path).expect("the board");
    let mut stack = LayerStack::derived(&board.layers, 1.6);
    let copper: Vec<usize> = (stack.layers.iter().enumerate())
        .filter(|(_, row)| row.kind.is_copper())
        .map(|(at, _)| at)
        .collect();
    stack.layers.remove(copper[2]);
    let rows = format!("stack={}", serde_json::to_string(&stack).expect("json"));
    let seq = logged(&mut app, &path, "set_layer_stack", &[&rows]);
    assert!(app.step_board_history(&path, true));
    let via = serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::from_u128(77), "position": {"x": 1.0, "y": 1.0},
        "diameter": 0.6, "drill": 0.2, "layers": ["TopCopper", "BottomCopper"],
        "layer_sizes": {"mode": "full", "layers": [
            {"layer": "TopCopper", "size": 0.6},
            {"layer": "Inner1Copper", "size": 0.55},
            {"layer": "Inner2Copper", "size": 0.5},
            {"layer": "BottomCopper", "size": 0.6},
        ]}
    }))
    .expect("a via");
    let add = BoardCommand::AddItems {
        items: vec![BoardItem::Via(via)],
    };
    app.execute_board_commands(&path, vec![add], None)
        .expect("the via lands");

    replay(&mut app, seq);

    assert!(failure(&newest(&app)).contains("still have sizes"));
    let board = app.document_state.pcb_board(&path).expect("the board");
    assert!(board.vias[0].layer_sizes.is_some(), "the sizes stay");
}

#[test]
fn a_board_delete_replays_on_its_part_after_an_undo() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    select_part(&mut app);
    run(&mut app, "delete_selection");
    let seq = newest(&app).seq;
    let after = footprints_on(&app, &path);
    assert!(app.step_board_history(&path, true));

    replay(&mut app, seq);

    assert_eq!(footprints_on(&app, &path), after);
    assert!(!newest(&app).outcome.is_failed());
}

#[test]
fn a_board_delete_whose_target_is_gone_says_so() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    select_part(&mut app);
    run(&mut app, "delete_selection");
    let seq = newest(&app).seq;
    let after = footprints_on(&app, &path);

    replay(&mut app, seq);

    assert!(failure(&newest(&app)).contains("no longer in"));
    assert_eq!(footprints_on(&app, &path), after);
}

#[test]
fn a_board_duplicate_replays_as_one_more_copy() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    select_part(&mut app);
    run(&mut app, "duplicate");
    let seq = newest(&app).seq;
    let with_copy = footprint_places(&app, &path);
    // The copy lands at a fixed offset: its place must be free to replay.
    assert!(app.step_board_history(&path, true));
    let copies = footprints_on(&app, &path);

    replay(&mut app, seq);

    assert_eq!(footprints_on(&app, &path), copies + 1);
    assert!(!newest(&app).outcome.is_failed());
    assert_eq!(
        footprint_places(&app, &path),
        with_copy,
        "the copy is back at the same place"
    );
}

#[test]
fn every_listed_command_is_refused_once_its_target_changed() {
    for (id, _) in SAFE_COMMANDS {
        if matches!(*id, "move_selection" | "delete_selection" | "duplicate") {
            refused_on_a_sheet_once_changed(id);
            refused_on_a_board_once_changed(id);
            continue;
        }
        let dir = tempfile::tempdir().expect("a folder");
        let (mut app, path) = if *id == "set_layer_stack" {
            app_with_four_layers(dir.path())
        } else {
            app_with_tracks(dir.path())
        };
        let seq = run_then_change_its_target(&mut app, &path, id);
        let revision = |app: &Signex| {
            let engine = app.document_state.board_engine(&path);
            engine.expect("the board").revision()
        };
        let before = revision(&app);

        replay(&mut app, seq);

        assert_eq!(revision(&app), before, "{id} wrote the board");
        let row = newest(&app);
        assert!(failure(&row).contains("changed after"), "{id}");
    }
}

#[test]
fn every_listed_command_is_in_the_catalog() {
    for (id, _) in SAFE_COMMANDS {
        assert!(crate::keymap::metadata_for_str(id).is_some(), "{id}");
    }
}

#[test]
fn an_unlisted_command_is_refused_and_says_why() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    let before = track(&app, &path, FIRST);
    let mut operands = serde_json::Map::new();
    operands.insert(
        "document".to_string(),
        serde_json::Value::String(path.display().to_string()),
    );
    let seq = app
        .record_command(
            "arrange_in_box",
            operands,
            LogOutcome::Ok(None),
            Duration::ZERO,
            LogSource::Gui { window: None },
            Some(path.clone()),
        )
        .expect("logged");

    replay(&mut app, seq);

    assert_not_replayable(&app);
    assert!(!failure(&newest(&app)).contains("changed after"));
    assert_eq!(track(&app, &path, FIRST), before);
}

/// The review named these: each writes over later edits, resolves through
/// live state, or has no guard.
#[test]
fn the_commands_with_no_proof_are_off_the_list() {
    for id in [
        "board_shape_from_selection",
        "arrange_in_box",
        "set_board_graphic",
        "set_footprint_text",
        "delete_net_class",
        "set_net_class_rules",
        "assign_board_net_class",
        "undo",
        "redo",
        "swap_pins",
        "swap_gates",
        "import_dxf",
        "import_dxf_outline",
        "update_pcb_from_schematic",
        "tune_net_length",
        "tune_diff_pair",
        "rules_set",
        "project_add_variant",
        "set_via_span",
        "set_layer_sizes",
        "reset_board_origin",
        "waive_drc_violation",
    ] {
        assert_eq!(replay_proof(id), None, "{id}");
    }
}

/// `lock` writes only the items not yet locked: one already locked has no
/// recorded state, and a rerun would lock it again after a later unlock.
#[test]
fn a_lock_row_naming_an_item_it_did_not_write_is_refused() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    let first = uuid::Uuid::from_u128(FIRST);
    let second = uuid::Uuid::from_u128(SECOND);
    logged(
        &mut app,
        &path,
        "lock_board_items",
        &[&format!("ids={first}")],
    );
    let both = format!("ids={first},{second}");
    let seq = logged(&mut app, &path, "lock_board_items", &[&both]);
    logged(&mut app, &path, "unlock_board_items", &[&both]);

    replay(&mut app, seq);

    assert!(failure(&newest(&app)).contains("recorded no state"));
    assert!(!track(&app, &path, FIRST).locked);
    assert!(!track(&app, &path, SECOND).locked);
}

const FIRST: u128 = 11;
const SECOND: u128 = 12;

fn new_track(id: u128, y: f64) -> Segment {
    serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::from_u128(id),
        "start": {"x": 0.0, "y": y},
        "end": {"x": 5.0, "y": y},
        "width": 0.25,
        "layer": "TopCopper",
    }))
    .expect("a track")
}

fn app_with_tracks(dir: &Path) -> (Signex, PathBuf) {
    let mut app =
        crate::app::handlers::menu::export::tests::app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    let mut board = board_with_two_parts();
    board.segments = vec![new_track(FIRST, 0.0), new_track(SECOND, 5.0)];
    board.outline = [(0.0, 0.0), (50.0, 0.0), (50.0, 40.0), (0.0, 40.0)]
        .map(|(x, y)| Point::new(x, y))
        .to_vec();
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path)
}

fn track(app: &Signex, path: &Path, id: u128) -> Segment {
    (app.document_state.pcb_board(path).expect("the board"))
        .segments
        .iter()
        .find(|s| s.uuid == uuid::Uuid::from_u128(id))
        .expect("the track")
        .clone()
}

fn replace(app: &mut Signex, path: &Path, tracks: Vec<Segment>) {
    let items = tracks.into_iter().map(BoardItem::Segment).collect();
    let command = BoardCommand::ReplaceItems { items };
    app.execute_board_commands(path, vec![command], None)
        .expect("the edit runs");
}

/// A Properties panel width edit of `ids`: one row.
fn widen(app: &mut Signex, path: &Path, ids: &[u128]) -> u64 {
    let tracks = ids
        .iter()
        .map(|id| Segment {
            width: 0.5,
            ..track(app, path, *id)
        })
        .collect();
    app.ui_state.command_replay.mark_board_properties_edit();
    replace(app, path, tracks);
    let row = newest(app);
    assert_eq!(row.command, "edit_object_properties");
    row.seq
}

fn move_track(app: &mut Signex, path: &Path, id: u128) -> Segment {
    let mut moved = track(app, path, id);
    moved.start.x += 2.0;
    moved.end.x += 2.0;
    replace(app, path, vec![moved.clone()]);
    moved
}

#[test]
fn a_track_width_replay_is_refused_after_a_later_move() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    let seq = widen(&mut app, &path, &[FIRST]);
    let moved = move_track(&mut app, &path, FIRST);

    replay(&mut app, seq);

    assert_eq!(track(&app, &path, FIRST), moved, "the move stays");
    assert!(failure(&newest(&app)).contains("changed after"));
}

#[test]
fn a_track_width_replays_after_its_edit_was_undone() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    let seq = widen(&mut app, &path, &[FIRST]);
    assert!(app.step_board_history(&path, true));
    assert_eq!(track(&app, &path, FIRST).width, 0.25);

    replay(&mut app, seq);

    assert_eq!(track(&app, &path, FIRST).width, 0.5);
}

#[test]
fn a_row_with_one_changed_target_changes_nothing() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    let seq = widen(&mut app, &path, &[FIRST, SECOND]);
    assert!(app.step_board_history(&path, true));
    let moved = move_track(&mut app, &path, SECOND);

    replay(&mut app, seq);

    assert_eq!(track(&app, &path, FIRST).width, 0.25, "the free one too");
    assert_eq!(track(&app, &path, SECOND), moved);
    assert!(failure(&newest(&app)).contains("changed after"));
}

/// Every track of the board as `[start x, start y, end x, end y]`, sorted.
fn track_lines(app: &Signex, path: &Path) -> Vec<[f64; 4]> {
    let board = app.document_state.pcb_board(path).expect("the board");
    let mut lines: Vec<[f64; 4]> = (board.segments.iter())
        .map(|s| [s.start.x, s.start.y, s.end.x, s.end.y])
        .collect();
    lines.sort_by(|a, b| {
        (a.iter().zip(b))
            .map(|(x, y)| x.total_cmp(y))
            .find(|order| order.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    lines
}

/// #2513, Caner (2026-11-25): a part's drop bends the track on its pad at
/// one 45° corner by the corner rule it was dragged with; the row keeps
/// that rule, so its Replay bends the track the same way even though the
/// router's rule has changed since.
#[test]
fn a_board_move_replay_bends_the_tracks_on_its_pads_as_the_drop_did() {
    use signex_types::corner::{CornerMode, CornerRule};
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = crate::app::handlers::menu::export::tests::app_workspace(
        &dir.path().to_string_lossy(),
        &[],
    );
    let path = dir.path().join("board.snxpcb");
    let mut board = board_with_two_parts();
    // From U1's pad at (10, 10) to (15, 10), on no net as the pad.
    let mut on_pad = new_track(FIRST, 10.0);
    (on_pad.start, on_pad.end) = (Point::new(10.0, 10.0), Point::new(15.0, 10.0));
    board.segments = vec![on_pad];
    app.open_pcb_tab(path.clone(), "board".into(), board);
    // 45° first out of the track's fixed end: not the default rule.
    let corner = CornerRule {
        mode: CornerMode::Angle45Start,
        is_flipped: false,
    };
    let moved = std::collections::BTreeSet::from([uuid::Uuid::from_u128(PART)]);
    let delta = Point::new(0.0, 3.0);
    let built = signex_engine::pcb::placement::move_commands(
        app.document_state.pcb_board(&path).expect("the board"),
        &moved,
        delta,
        corner,
        &mut signex_types::object_id::UuidV7Ids,
    );
    app.ui_state
        .command_replay
        .note_board_drop(moved, delta, false, corner);
    assert!(app.run_placement("Move", built));
    let row = newest(&app);
    assert_eq!(row.command, "move_selection");
    // From (15, 10) 45° to (12, 13), then straight to the pad at (10, 13).
    let bent = vec![[10.0, 13.0, 12.0, 13.0], [12.0, 13.0, 15.0, 10.0]];
    assert_eq!(track_lines(&app, &path), bent, "the drop bends the track");
    assert!(app.step_board_history(&path, true), "one undo step");
    assert_eq!(track_lines(&app, &path), [[10.0, 10.0, 15.0, 10.0]]);
    app.interaction_state.corner_rule = CornerRule::default();

    replay(&mut app, row.seq);

    let ran = newest(&app);
    assert!(!ran.outcome.is_failed(), "{:?}", ran.outcome);
    assert_eq!(track_lines(&app, &path), bent, "the replay bends it alike");
    let u1 = app.document_state.pcb_board(&path).expect("the board");
    let u1 = u1
        .footprints
        .iter()
        .find(|fp| fp.uuid == uuid::Uuid::from_u128(PART));
    assert_eq!(u1.expect("U1").position, Point::new(10.0, 13.0));
}
