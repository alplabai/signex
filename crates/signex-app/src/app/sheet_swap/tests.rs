//! #1261 — pin and gate swap started on the schematic, through the headless
//! path an agent and the CLI take and through `Signex::update` (the Part
//! Actions row, the palette or a key on the clicked pin or the selected
//! units). Only the sheet changes; the board waits for Update PCB.

use std::path::{Path, PathBuf};

use serde_json::json;
use signex_i18n::RenderTranslatable;
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

use super::{SheetSwapMsg, SwapReady};
use crate::app::command::{CommandArgs, CommandOutcome, translator};
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::handlers::pcb_component::part_swap_tests::{
    SHEET, UNIT_A, UNIT_B, app_with_part, pad_nets, sheet, tip,
};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

fn headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

fn answered(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn live(app: &Signex, path: &Path) -> SchematicSheet {
    app.document_state.engines[path].document().clone()
}

fn wire_start(app: &Signex, path: &Path, n: u128) -> Point {
    live(app, path)
        .wires
        .iter()
        .find(|w| w.uuid == Uuid::from_u128(n))
        .unwrap()
        .start
}

fn units(app: &Signex, path: &Path) -> Vec<(Uuid, u32)> {
    live(app, path)
        .symbols
        .iter()
        .map(|s| (s.uuid, s.unit))
        .collect()
}

/// The part's sheet alone, open in the main window.
fn sheet_app(dir: &Path) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let path = dir.join(SHEET);
    add_sheet_with(&mut app, &path, sheet());
    (app, path)
}

fn select(app: &mut Signex, symbols: &[Uuid]) {
    app.interaction_state.active_canvas_mut().selected = symbols
        .iter()
        .map(|id| SelectedItem::new(*id, SelectedKind::Symbol))
        .collect();
}

fn click_pin(app: &mut Signex, path: &Path, symbol: Uuid, pin: &str) {
    select(app, &[symbol]);
    app.interaction_state.clicked_pin = Some(crate::app::state::ClickedPin {
        sheet: path.to_path_buf(),
        placement: None,
        symbol,
        pin: pin.to_string(),
    });
}

fn run(app: &mut Signex, id: &str) {
    let _ = app.update(Message::Menu(MenuMessage::command(id)));
}

#[test]
fn an_agent_swaps_two_pins_on_the_sheet_and_the_board_waits_for_update_pcb() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board, path) = app_with_part(dir.path());
    let before = live(&app, &path);
    let pads = pad_nets(&app, &board);
    let document = format!("document={}", path.display());

    let answer = answered(headless(
        &mut app,
        "swap_schematic_pins",
        &[&document, "designator=U1", "pins=1,2"],
    ));

    assert_eq!(answer["designator"], "U1");
    assert_eq!(answer["kind"], "pins");
    assert_eq!(answer["swapped"], json!(["1", "2"]));
    assert_eq!(answer["pairs"], json!([["1", "2"]]));
    assert_eq!(answer["changed"], true);
    assert_eq!(
        wire_start(&app, &path, 0x101),
        tip(&before, UNIT_A, "2"),
        "pin 1's wire is on pin 2"
    );
    assert_eq!(
        wire_start(&app, &path, 0x102),
        tip(&before, UNIT_A, "1"),
        "pin 2's wire is on pin 1"
    );
    assert_eq!(
        pad_nets(&app, &board),
        pads,
        "the board waits for Update PCB"
    );

    assert_eq!(app.step_sheet_on(&path, true), Ok(true));
    assert_eq!(live(&app, &path).wires, before.wires, "one undo step");
}

#[test]
fn an_agent_swaps_two_gates_of_the_unit_it_names() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board, path) = app_with_part(dir.path());
    let pads = pad_nets(&app, &board);
    let document = format!("document={}", path.display());
    let ids = format!("ids={UNIT_B}");

    let answer = answered(headless(
        &mut app,
        "swap_schematic_gates",
        &[&document, &ids, "units=1,2"],
    ));

    assert_eq!(answer["kind"], "gates");
    assert_eq!(answer["pairs"], json!([["1", "3"], ["2", "4"]]));
    assert_eq!(units(&app, &path), [(UNIT_A, 2), (UNIT_B, 1)]);
    assert_eq!(pad_nets(&app, &board), pads);
}

#[test]
fn a_swap_the_part_does_not_allow_or_names_no_part_is_refused_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _board, path) = app_with_part(dir.path());
    let before = live(&app, &path);
    let document = format!("document={}", path.display());
    let ids = format!("ids={UNIT_A}");

    for (operands, says) in [
        (vec!["designator=U1", "pins=1,3"], "pin swap group"),
        (vec!["designator=U1", "units=1,1"], "gate swap group"),
        (vec!["designator=U9", "pins=1,2"], "no part U9"),
        (vec![ids.as_str(), "designator=U1", "pins=1,2"], "not both"),
        (vec!["pins=1,2"], "name the part"),
        (vec!["designator=U1", "pins=1"], "two members"),
    ] {
        let id = if operands.iter().any(|o| o.starts_with("units")) {
            "swap_schematic_gates"
        } else {
            "swap_schematic_pins"
        };
        let mut texts = vec![document.as_str()];
        texts.extend(operands.iter().copied());
        let why = refusal(headless(&mut app, id, &texts));
        assert!(why.contains(says), "{operands:?}: {why}");
    }
    assert_eq!(live(&app, &path).wires, before.wires);
    assert_eq!(units(&app, &path), [(UNIT_A, 1), (UNIT_B, 2)]);
}

#[test]
fn a_swap_named_at_a_board_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board, _) = app_with_part(dir.path());
    let document = format!("document={}", board.display());
    let why = refusal(headless(
        &mut app,
        "swap_schematic_pins",
        &[&document, "designator=U1", "pins=1,2"],
    ));
    assert!(why.contains("acts on a sheet"), "{why}");
}

/// The Part Actions row on the clicked pin runs the operand handler and
/// leaves the sheet as the operands do.
#[test]
fn swap_pins_on_the_clicked_pin_is_its_operand_handler() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = sheet_app(dir.path());
    let before = live(&app, &path);
    click_pin(&mut app, &path, UNIT_A, "1");

    run(&mut app, "swap_schematic_pins");

    assert!(translator::ran("swap_schematic_pins"));
    assert_eq!(wire_start(&app, &path, 0x101), tip(&before, UNIT_A, "2"));
    assert_eq!(wire_start(&app, &path, 0x102), tip(&before, UNIT_A, "1"));
}

#[test]
fn swap_gates_on_the_two_selected_units_is_its_operand_handler() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = sheet_app(dir.path());
    select(&mut app, &[UNIT_A, UNIT_B]);

    run(&mut app, "swap_schematic_gates");

    assert!(translator::ran("swap_schematic_gates"));
    assert_eq!(units(&app, &path), [(UNIT_A, 2), (UNIT_B, 1)]);
}

#[test]
fn a_swap_with_nothing_to_swap_says_why_in_messages() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = sheet_app(dir.path());
    let before = live(&app, &path);
    let mark = crate::diagnostics::newest_entry_id();

    let _ = app.update(Message::from(SheetSwapMsg::PinsRequested));

    let said = crate::diagnostics::entries_since(mark).entries;
    assert!(
        said.iter()
            .any(|e| e.message.contains("did nothing") && e.message.contains("click the pin")),
        "{said:?}"
    );
    assert_eq!(live(&app, &path).wires, before.wires);
}

/// The canvas menu's rows are live by what the clicked pin and the
/// selection name.
#[test]
fn the_rows_are_ready_by_the_clicked_pin_and_the_selected_units() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = sheet_app(dir.path());
    let ready = |app: &Signex| {
        let doc = app.document_state.engines[&path].document();
        app.sheet_swap_ready(doc, app.interaction_state.active_canvas())
    };
    assert_eq!(ready(&app), SwapReady::default(), "nothing selected");

    click_pin(&mut app, &path, UNIT_A, "1");
    assert_eq!(
        ready(&app),
        SwapReady {
            pins: true,
            gates: true
        },
        "pin 1 trades with pin 2; gate 1 with the placed gate 2"
    );

    select(&mut app, &[UNIT_B]);
    assert!(
        !ready(&app).pins,
        "the clicked pin's part is no longer selected"
    );
}
