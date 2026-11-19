//! Repair Pin Junctions, Reset board origin and Reset All Layer Colours from
//! operands: a CLI run or an agent does what the menu row does, on the
//! document it names, as one undo step.

use std::path::{Path, PathBuf};

use signex_types::pcb::{PcbBoard, Point};
use signex_types::project::SetupProfile;

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::pin_junction_repair::tests::sheet_with_bare_tap;
use crate::keymap::AppCommandId;

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands");
    app.run_operands_settled(&command, &args).0
}

fn answered(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

/// The dots go on the sheet `document` names, not the one in view, in one
/// undo step; a second run finds nothing to add.
#[test]
fn repair_adds_the_dots_on_the_named_sheet() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let bare = dir.path().join("bare.snxsch");
    add_sheet_with(&mut app, &bare, sheet_with_bare_tap());
    add_sheet_with(
        &mut app,
        &dir.path().join("other.snxsch"),
        Default::default(),
    );
    let junctions = |app: &Signex| {
        let engine = app.document_state.engines.get(&bare).expect("open");
        engine.document().junctions.len()
    };
    assert_eq!(junctions(&app), 0);
    let document = format!("document={}", bare.display());
    let answer = answered(run(&mut app, "repair_pin_junctions", &[&document]));
    assert_eq!(answer["added"], 1);
    assert_eq!(junctions(&app), 1);
    let again = answered(run(&mut app, "repair_pin_junctions", &[&document]));
    assert_eq!(again["added"], 0);
    let engine = app.document_state.engines.get_mut(&bare).expect("open");
    engine.undo().expect("undo");
    assert_eq!(junctions(&app), 0, "one undo step");
}

/// An outline from (10, 20) to (60, 50), origin at the model's zero.
fn board() -> PcbBoard {
    let mut board = SetupProfile::Standard.pcb(2).new_board();
    board.options.origin = Default::default();
    board.outline = vec![
        Point::new(10.0, 20.0),
        Point::new(60.0, 20.0),
        Point::new(60.0, 50.0),
        Point::new(10.0, 50.0),
    ];
    board
}

fn open(dir: &Path, board: PcbBoard) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path)
}

#[test]
fn the_board_resets_run_on_the_named_board() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut coloured = board();
    let top = signex_types::pcb_layer::PcbLayer::Copper(signex_types::pcb_layer::CopperLayer::Top);
    coloured
        .options
        .set_layer_color(top, Some(signex_types::theme::Color::new(255, 0, 255, 255)));
    let (mut app, path) = open(dir.path(), coloured);
    let document = format!("document={}", path.display());

    let answer = answered(run(&mut app, "reset_board_origin", &[&document]));
    let origin = app.document_state.pcb_board(&path).expect("board").options.origin.at;
    assert!(origin != Point::ZERO, "the origin moved to the outline");
    assert_eq!(answer["origin"], serde_json::json!([origin.x, origin.y]));

    assert_eq!(
        run(&mut app, "reset_layer_colors", &[&document]),
        CommandOutcome::Ran
    );
    let board = app.document_state.pcb_board(&path).expect("board");
    assert!(board.options.layer_colors.is_empty(), "every colour reset");
}

#[test]
fn the_board_resets_need_their_board() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _path) = open(dir.path(), board());
    for id in ["reset_board_origin", "reset_layer_colors"] {
        let command = AppCommandId::new(id).expect("id");
        let missing = crate::app::command::missing_headless_operand(&command);
        assert!(missing.is_some(), "{id} names its board headless");
    }
    let nope = format!("document={}", dir.path().join("nope.snxpcb").display());
    let outcome = run(&mut app, "reset_board_origin", &[&nope]);
    assert!(matches!(outcome, CommandOutcome::Disabled { .. }), "{outcome:?}");
}
