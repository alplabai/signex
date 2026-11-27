//! #2470 — `reannotate_board document=..` renames as the dialog's Execute
//! does, as one labelled undo step, and answers with every rename.

use std::path::{Path, PathBuf};

use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::BoardReannotateMsg;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// Left to right: R1 is in order, R3 should be R2, R2 should be R3, and the
/// locked R9 keeps its designator.
fn board() -> PcbBoard {
    let parts = [
        (1, "R1", 0.0, false),
        (2, "R3", 10.0, false),
        (3, "R2", 20.0, false),
        (4, "R9", 30.0, true),
    ];
    let footprints: Vec<_> = parts
        .iter()
        .map(|&(n, reference, x, locked)| {
            serde_json::json!({
                "uuid": id(n), "reference": reference, "value": "v",
                "position": {"x": x, "y": 0.0}, "layer": "TopCopper", "locked": locked,
                "schematic_link": {"sheet": [], "units": [id(n + 0x1000)]},
            })
        })
        .collect();
    serde_json::from_value(serde_json::json!({ "uuid": id(999), "footprints": footprints }))
        .expect("board")
}

fn board_app(name: &str) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let path = std::env::temp_dir().join(name);
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

fn references(app: &Signex, path: &Path) -> Vec<String> {
    let engine = app.document_state.board_engine(path).expect("the board");
    (engine.board().footprints.iter())
        .map(|fp| fp.reference.clone())
        .collect()
}

fn undo_steps(app: &Signex, path: &Path) -> (usize, Option<String>) {
    let engine = app.document_state.board_engine(path).expect("the board");
    (engine.undo_len(), engine.undo_label().map(str::to_owned))
}

fn run(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new("reannotate_board").expect("an id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

fn renames(outcome: &CommandOutcome) -> Vec<(String, String)> {
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("Re-Annotate answers with its renames: {outcome:?}");
    };
    let text = |value: &serde_json::Value| value.as_str().expect("text").to_string();
    (answer["renames"].as_array().expect("renames").iter())
        .map(|rename| (text(&rename["old"]), text(&rename["new"])))
        .collect()
}

#[test]
fn the_operand_form_renames_as_the_dialogs_execute() {
    let (mut gui, gui_path) = board_app("reannotate-gui.snxpcb");
    let _ = gui.update(Message::from(BoardReannotateMsg::Requested));
    let _ = gui.update(Message::from(BoardReannotateMsg::ExecuteRequested));

    let (mut cli, cli_path) = board_app("reannotate-cli.snxpcb");
    let document = format!("document={}", cli_path.display());
    let outcome = run(&mut cli, &[&document]);

    assert_eq!(references(&gui, &gui_path), ["R1", "R2", "R3", "R9"]);
    assert_eq!(references(&cli, &cli_path), references(&gui, &gui_path));
    assert_eq!(undo_steps(&cli, &cli_path), undo_steps(&gui, &gui_path));
    assert_eq!(undo_steps(&cli, &cli_path).0, 1, "one undo step");
    assert_eq!(
        renames(&outcome),
        [
            ("R3".to_string(), "R2".to_string()),
            ("R2".to_string(), "R3".to_string()),
        ]
    );
    let CommandOutcome::Answered(answer) = &outcome else {
        unreachable!()
    };
    assert_eq!(answer["applied"], true);
    assert_eq!(answer["unchanged"], 1);
    assert_eq!(answer["kept"][0]["designator"], "R9");
    assert_eq!(answer["kept"][0]["reason"], "locked");
}

#[test]
fn a_dry_run_lists_the_renames_and_changes_nothing() {
    let (mut app, path) = board_app("reannotate-dry.snxpcb");
    let document = format!("document={}", path.display());
    let outcome = run(&mut app, &[&document, "dry_run=true", "order=y_then_x"]);
    assert_eq!(renames(&outcome).len(), 2, "{outcome:?}");
    let CommandOutcome::Answered(answer) = &outcome else {
        unreachable!()
    };
    assert_eq!(answer["applied"], false);
    assert_eq!(references(&app, &path), ["R1", "R3", "R2", "R9"]);
    assert_eq!(undo_steps(&app, &path).0, 0);
}

#[test]
fn an_unknown_order_is_refused_before_anything_changes() {
    let (mut app, path) = board_app("reannotate-order.snxpcb");
    let document = format!("document={}", path.display());
    let outcome = run(&mut app, &[&document, "order=diagonal"]);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if signex_i18n::RenderTranslatable::render_en_us(reason).contains("x_then_y")),
        "{outcome:?}"
    );
    assert_eq!(references(&app, &path), ["R1", "R3", "R2", "R9"]);
}

#[test]
fn without_operands_a_headless_run_is_told_to_name_the_board() {
    let (mut app, path) = board_app("reannotate-bare.snxpcb");
    let command = AppCommandId::new("reannotate_board").expect("an id");
    let (outcome, _) = app.run_headless_command_in_test(&command, CommandArgs::none());
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if signex_i18n::RenderTranslatable::render_en_us(reason).contains("`document`")),
        "{outcome:?}"
    );
    assert!(app.ui_state.board_reannotate.is_none(), "no dialog");
    assert_eq!(references(&app, &path), ["R1", "R3", "R2", "R9"]);
}
