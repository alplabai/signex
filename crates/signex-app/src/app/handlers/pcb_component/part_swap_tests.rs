//! #2410 — pin and gate swap on a board, driven through `Signex::update` (the
//! two pads clicked, the command run as the board menu runs it, Undo) and
//! through the headless path an agent and the CLI take. The board's pads
//! trade nets and the linked part's sheet follows in the same undo step.

use std::path::{Path, PathBuf};

use pcb_geom::ClickMode;
use signex_types::part_swap::{GATE_SWAP_GROUPS_FIELD, PIN_SWAP_GROUPS_FIELD};
use signex_types::pcb::{PcbBoard, SymbolLink};
use signex_types::schematic::{
    InstancePath, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet,
    Symbol, Wire, pin_tips,
};
use uuid::Uuid;

use super::part_swap::SwapRequest;
use crate::app::command::{CommandArgs, CommandOutcome, translator};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::pcb_canvas::PcbCanvasMsg;

const FP: Uuid = Uuid::from_u128(0xf0);
const UNIT_A: Uuid = Uuid::from_u128(0xa);
const UNIT_B: Uuid = Uuid::from_u128(0xb);
const SHEET: &str = "top.snxsch";

fn lib_pin(unit: u32, number: &str, x: f64) -> LibPin {
    LibPin {
        unit,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: String::new(),
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

/// A dual gate: gate 1 has pins 1 and 2, gate 2 pins 3 and 4, each drawn at
/// x = -5 and x = 5.
fn dual() -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": "DUAL" })).unwrap();
    lib.pins = vec![
        lib_pin(1, "1", -5.0),
        lib_pin(1, "2", 5.0),
        lib_pin(2, "3", -5.0),
        lib_pin(2, "4", 5.0),
    ];
    lib
}

fn unit(uuid: Uuid, unit: u32, y: f64) -> Symbol {
    let mut symbol = Symbol {
        uuid,
        lib_id: "DUAL".into(),
        reference: "U1".into(),
        position: Point::new(0.0, y),
        unit,
        ..Default::default()
    };
    symbol
        .fields
        .insert(PIN_SWAP_GROUPS_FIELD.to_string(), "1,2;3,4".to_string());
    symbol
        .fields
        .insert(GATE_SWAP_GROUPS_FIELD.to_string(), "1,2".to_string());
    symbol
}

/// The world tip of `pin` of `symbol` on `sheet`.
fn tip(sheet: &SchematicSheet, symbol: Uuid, pin: &str) -> Point {
    let placed = sheet.symbols.iter().find(|s| s.uuid == symbol).unwrap();
    let lib = sheet.resolve_lib_symbol(placed).unwrap();
    pin_tips(&lib, placed)
        .find(|(lp, _)| lp.pin.number == pin)
        .map(|(_, at)| at)
        .unwrap()
}

/// Gate 1 of U1 at the origin, gate 2 at y = 20, a wire running up 10 mm
/// from each of pins 1 and 2.
fn sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet.lib_symbols.insert("DUAL".into(), dual());
    sheet.symbols = vec![unit(UNIT_A, 1, 0.0), unit(UNIT_B, 2, 20.0)];
    let stub = |n: u128, from: Point| Wire {
        uuid: Uuid::from_u128(n),
        start: from,
        end: Point::new(from.x, from.y - 10.0),
        ..Default::default()
    };
    let (one, two) = (tip(&sheet, UNIT_A, "1"), tip(&sheet, UNIT_A, "2"));
    sheet.wires = vec![stub(0x101, one), stub(0x102, two)];
    sheet
}

/// U1 at (10, 10): pads 1 to 4 on nets A to D, both swapping switches on,
/// linked to the two units on the sheet.
fn board() -> PcbBoard {
    let pad = |n: u128, number: &str, x: f64, net: u32, name: &str| {
        serde_json::json!({
            "uuid": Uuid::from_u128(n), "number": number, "pad_type": "smd", "shape": "rect",
            "position": {"x": x, "y": 0.0}, "size": {"x": 1.0, "y": 1.0}, "drill": null,
            "layers": ["TopCopper"], "net": {"number": net, "name": name}
        })
    };
    let mut board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [
            {"number": 1, "name": "A"}, {"number": 2, "name": "B"},
            {"number": 3, "name": "C"}, {"number": 4, "name": "D"}
        ],
        "footprints": [{
            "uuid": FP, "reference": "U1", "value": "DUAL", "footprint_id": "SO8",
            "position": {"x": 10.0, "y": 10.0}, "rotation": 0.0, "layer": "TopCopper",
            "component": {"pin_swapping": true, "part_swapping": true},
            "pads": [
                pad(0x11, "1", -3.0, 1, "A"), pad(0x12, "2", -1.0, 2, "B"),
                pad(0x13, "3", 1.0, 3, "C"), pad(0x14, "4", 3.0, 4, "D")
            ]
        }]
    }))
    .unwrap();
    board.footprints[0].schematic_link = Some(SymbolLink::new(
        InstancePath::from_chain(Vec::new()),
        [UNIT_A, UNIT_B],
    ));
    board
}

pub(crate) fn app_with_part(dir: &Path) -> (Signex, PathBuf, PathBuf) {
    app_with(dir, board())
}

fn app_with(dir: &Path, board: PcbBoard) -> (Signex, PathBuf, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[SHEET]);
    app.ui_state.first_run_tour_open = false;
    let sheet_path = dir.join(SHEET);
    open_with(&mut app.document_state, &sheet_path, sheet());
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let board_path = dir.join("board.snxpcb");
    app.open_pcb_tab(board_path.clone(), "board".into(), board);
    (app, board_path, sheet_path)
}

fn pad_nets(app: &Signex, board: &Path) -> Vec<(String, String)> {
    app.document_state
        .pcb_board(board)
        .expect("board")
        .footprints[0]
        .pads
        .iter()
        .map(|p| {
            let net = p.net.as_ref().map_or(String::new(), |n| n.name.clone());
            (p.number.clone(), net)
        })
        .collect()
}

fn nets(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(p, n)| ((*p).to_string(), (*n).to_string()))
        .collect()
}

fn live_sheet(app: &Signex, sheet: &Path) -> SchematicSheet {
    app.document_state.engines[sheet].document().clone()
}

/// Where wire `n` ends on the pin side.
fn wire_start(app: &Signex, sheet: &Path, n: u128) -> Point {
    let doc = live_sheet(app, sheet);
    doc.wires
        .iter()
        .find(|w| w.uuid == Uuid::from_u128(n))
        .unwrap()
        .start
}

fn click(app: &mut Signex, x: f64, y: f64, mode: ClickMode) {
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
        at: signex_types::pcb::Point::new(x, y),
        tolerance_mm: 0.05,
        mode,
    }));
}

fn run(app: &mut Signex, id: &str) {
    let _ = app.update(Message::Menu(MenuMessage::command(id)));
}

fn headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

#[test]
fn swap_pins_on_two_clicked_pads_trades_their_nets_and_rewires_the_sheet_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board, sheet) = app_with_part(dir.path());
    let before = live_sheet(&app, &sheet);
    let (one, two) = (tip(&before, UNIT_A, "1"), tip(&before, UNIT_A, "2"));

    click(&mut app, 7.0, 10.0, ClickMode::Replace);
    click(&mut app, 9.0, 10.0, ClickMode::Add);
    run(&mut app, "swap_pins");

    assert_eq!(
        pad_nets(&app, &board),
        nets(&[("1", "B"), ("2", "A"), ("3", "C"), ("4", "D")]),
        "pads 1 and 2 traded nets"
    );
    assert_eq!(
        wire_start(&app, &sheet, 0x101),
        two,
        "pin 1's wire is on pin 2"
    );
    assert_eq!(
        wire_start(&app, &sheet, 0x102),
        one,
        "pin 2's wire is on pin 1"
    );

    run(&mut app, "undo");
    assert_eq!(
        pad_nets(&app, &board),
        nets(&[("1", "A"), ("2", "B"), ("3", "C"), ("4", "D")]),
        "one Undo puts the board back"
    );
    assert_eq!(
        live_sheet(&app, &sheet).wires,
        before.wires,
        "and the sheet"
    );
}

#[test]
fn an_agent_swaps_two_gates_and_reads_what_changed() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board, sheet) = app_with_part(dir.path());
    let document = format!("document={}", board.display());

    let outcome = headless(
        &mut app,
        "swap_gates",
        &[&document, "designator=U1", "units=1,2"],
    );

    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["kind"], "gates");
    assert_eq!(answer["swapped"], serde_json::json!(["1", "2"]));
    assert_eq!(answer["pads"].as_array().map(Vec::len), Some(4));
    assert_eq!(
        pad_nets(&app, &board),
        nets(&[("1", "C"), ("2", "D"), ("3", "A"), ("4", "B")])
    );
    let units: Vec<(Uuid, u32)> = live_sheet(&app, &sheet)
        .symbols
        .iter()
        .map(|s| (s.uuid, s.unit))
        .collect();
    assert_eq!(units, [(UNIT_A, 2), (UNIT_B, 1)], "the placed gates traded");
}

#[test]
fn pins_outside_one_swap_group_are_refused_and_nothing_changes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board, sheet) = app_with_part(dir.path());
    let document = format!("document={}", board.display());
    let before = live_sheet(&app, &sheet);

    let outcome = headless(
        &mut app,
        "swap_pins",
        &[&document, "designator=U1", "pins=1,3"],
    );

    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if signex_i18n::RenderTranslatable::render_en_us(reason).contains("pin swap group")),
        "{outcome:?}"
    );
    assert_eq!(
        pad_nets(&app, &board),
        nets(&[("1", "A"), ("2", "B"), ("3", "C"), ("4", "D")])
    );
    assert_eq!(live_sheet(&app, &sheet).wires, before.wires);
}

#[test]
fn a_component_with_pin_swapping_off_is_refused_with_the_switch_named() {
    let dir = tempfile::tempdir().unwrap();
    let mut off = board();
    off.footprints[0].component.pin_swapping = false;
    let (mut app, board, _) = app_with(dir.path(), off);
    let document = format!("document={}", board.display());

    let outcome = headless(
        &mut app,
        "swap_pins",
        &[&document, "designator=U1", "pins=1,2"],
    );

    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if signex_i18n::RenderTranslatable::render_en_us(reason).contains("Pin Swapping")),
        "{outcome:?}"
    );
}

/// Swap Gates from the board menu: the two clicked pads name the gates they
/// belong to; pads of one gate are refused with nothing changed.
#[test]
fn swap_gates_on_pads_of_two_gates_trades_the_gates_and_one_gate_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board, sheet) = app_with_part(dir.path());

    let refused = app.swap_part(
        &board,
        FP,
        &SwapRequest::GatesOfPins("1".into(), "2".into()),
    );
    assert!(
        refused.is_err_and(|why| why.contains("one gate")),
        "pads 1 and 2 are both on gate 1"
    );
    assert_eq!(
        pad_nets(&app, &board),
        nets(&[("1", "A"), ("2", "B"), ("3", "C"), ("4", "D")])
    );

    click(&mut app, 7.0, 10.0, ClickMode::Replace);
    click(&mut app, 11.0, 10.0, ClickMode::Add);
    run(&mut app, "swap_gates");

    assert_eq!(
        pad_nets(&app, &board),
        nets(&[("1", "C"), ("2", "D"), ("3", "A"), ("4", "B")])
    );
    let units: Vec<(Uuid, u32)> = live_sheet(&app, &sheet)
        .symbols
        .iter()
        .map(|s| (s.uuid, s.unit))
        .collect();
    assert_eq!(units, [(UNIT_A, 2), (UNIT_B, 1)]);
}

fn units_on(app: &Signex, sheet: &Path) -> Vec<(Uuid, u32)> {
    live_sheet(app, sheet)
        .symbols
        .iter()
        .map(|s| (s.uuid, s.unit))
        .collect()
}

/// `command` from the board menu on the pads at x = `a` and `b` leaves the
/// board and the sheet as its operand handler does with `operands`, and
/// goes through that handler (#2326 slice 6 translator).
fn menu_swap_matches_operands(command: &str, a: f64, b: f64, operands: &[&str]) {
    let dir = tempfile::tempdir().unwrap();
    let (mut menu, board, sheet) = app_with_part(dir.path());
    click(&mut menu, a, 10.0, ClickMode::Replace);
    click(&mut menu, b, 10.0, ClickMode::Add);
    run(&mut menu, command);
    assert!(
        translator::ran(command),
        "{command} ran its operand handler"
    );

    let dir = tempfile::tempdir().unwrap();
    let (mut app, board_b, sheet_b) = app_with_part(dir.path());
    let document = format!("document={}", board_b.display());
    let mut texts = vec![document.as_str(), "designator=U1"];
    texts.extend_from_slice(operands);
    let outcome = headless(&mut app, command, &texts);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    assert_eq!(pad_nets(&menu, &board), pad_nets(&app, &board_b));
    assert_ne!(
        pad_nets(&menu, &board),
        nets(&[("1", "A"), ("2", "B"), ("3", "C"), ("4", "D")]),
        "{command} changed the board"
    );
    assert_eq!(
        live_sheet(&menu, &sheet).wires,
        live_sheet(&app, &sheet_b).wires
    );
    assert_eq!(units_on(&menu, &sheet), units_on(&app, &sheet_b));
}

#[test]
fn swap_pins_from_the_menu_is_its_operand_handler_on_the_two_pads() {
    menu_swap_matches_operands("swap_pins", 7.0, 9.0, &["pins=1,2"]);
}

#[test]
fn swap_gates_from_the_menu_is_its_operand_handler_on_the_gates_of_the_two_pads() {
    menu_swap_matches_operands("swap_gates", 7.0, 11.0, &["units=1,2"]);
}

/// Pads of one gate are refused from the menu in the words they always
/// were, before the handler is asked.
#[test]
fn swap_gates_from_the_menu_on_one_gate_is_refused_as_before() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board, _) = app_with_part(dir.path());
    click(&mut app, 7.0, 10.0, ClickMode::Replace);
    click(&mut app, 9.0, 10.0, ClickMode::Add);
    let mark = crate::diagnostics::newest_entry_id();
    run(&mut app, "swap_gates");
    let said = crate::diagnostics::entries_since(mark).entries;
    assert!(
        said.iter().any(|e| e.message.contains("are on one gate")),
        "{said:?}"
    );
    assert_eq!(
        pad_nets(&app, &board),
        nets(&[("1", "A"), ("2", "B"), ("3", "C"), ("4", "D")])
    );
}

/// U1 and a second, unlinked component also named U1 at y = 40.
fn board_with_a_twin() -> PcbBoard {
    let mut twin = board();
    let mut other = twin.footprints[0].clone();
    other.uuid = Uuid::from_u128(0xf1);
    other.position.y = 40.0;
    other.schematic_link = None;
    for (n, pad) in other.pads.iter_mut().enumerate() {
        pad.uuid = Uuid::from_u128(0x21 + n as u128);
    }
    twin.footprints.push(other);
    twin
}

/// The menu names the clicked component by uuid (#2326 slice 6), so a
/// designator another component shares, or one stored as "U1 ", does not
/// stop the swap: it acts on the clicked component, as before the translator.
fn menu_swap_acts_on_the_clicked_component(board: PcbBoard) {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path, sheet) = app_with(dir.path(), board);
    let one = tip(&live_sheet(&app, &sheet), UNIT_A, "1");
    click(&mut app, 7.0, 10.0, ClickMode::Replace);
    click(&mut app, 9.0, 10.0, ClickMode::Add);
    run(&mut app, "swap_pins");
    assert!(translator::ran("swap_pins"));
    assert_eq!(
        pad_nets(&app, &path),
        nets(&[("1", "B"), ("2", "A"), ("3", "C"), ("4", "D")]),
        "the clicked component's pads traded nets"
    );
    assert_eq!(
        wire_start(&app, &sheet, 0x102),
        one,
        "and its sheet followed"
    );
}

#[test]
fn swap_pins_from_the_menu_on_a_shared_designator_swaps_the_clicked_component() {
    let twin = board_with_a_twin();
    menu_swap_acts_on_the_clicked_component(twin.clone());
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path, _) = app_with(dir.path(), twin);
    let other = app.document_state.pcb_board(&path).unwrap().footprints[1].clone();
    click(&mut app, 7.0, 10.0, ClickMode::Replace);
    click(&mut app, 9.0, 10.0, ClickMode::Add);
    run(&mut app, "swap_pins");
    assert_eq!(
        app.document_state.pcb_board(&path).unwrap().footprints[1],
        other,
        "the other U1 is left as it was"
    );
}

#[test]
fn swap_pins_from_the_menu_on_a_reference_with_spaces_swaps_the_clicked_component() {
    let mut spaced = board();
    spaced.footprints[0].reference = "U1 ".into();
    menu_swap_acts_on_the_clicked_component(spaced);
}

/// An agent names the component by uuid where its designator is shared.
#[test]
fn an_agent_swaps_pins_of_the_component_its_uuid_names() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board, _) = app_with(dir.path(), board_with_a_twin());
    let document = format!("document={}", board.display());
    let ids = format!("ids={FP}");

    let outcome = headless(&mut app, "swap_pins", &[&document, &ids, "pins=1,2"]);

    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    assert_eq!(
        pad_nets(&app, &board),
        nets(&[("1", "B"), ("2", "A"), ("3", "C"), ("4", "D")])
    );
}

/// A refusal of the operands reaches Messages in the menu's words.
#[test]
fn a_menu_swap_refused_by_its_operands_says_why_in_messages() {
    use signex_engine::pcb::pad_swap::SwapKind;
    use signex_i18n::{Translatable, ids};
    let dir = tempfile::tempdir().unwrap();
    let (app, _, _) = app_with_part(dir.path());
    let unknown = Uuid::from_u128(0xdead_2326);
    let refused = CommandOutcome::Disabled {
        reason: Translatable::new(ids::BOARD_TARGET_UNKNOWN).arg("id", unknown.to_string()),
    };
    let said = crate::diagnostics::logged_count(&unknown.to_string());
    app.say_part_swap_refusal(SwapKind::Pins, &refused);
    assert_eq!(
        crate::diagnostics::logged_count(&unknown.to_string()),
        said + 1
    );
}

/// Pad and pin 1 numbered "1 ": the menu sends the pad numbers as they are,
/// so it swaps them as before the translator, where an agent's `pins` is
/// trimmed.
#[test]
fn swap_pins_from_the_menu_on_a_pad_number_with_a_space_swaps_them() {
    let mut spaced = board();
    spaced.footprints[0].pads[0].number = "1 ".into();
    let mut part = sheet();
    if let Some(lib) = part.lib_symbols.get_mut("DUAL") {
        lib.pins[0].pin.number = "1 ".into();
    }
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[SHEET]);
    app.ui_state.first_run_tour_open = false;
    let sheet_path = dir.path().join(SHEET);
    open_with(&mut app.document_state, &sheet_path, part);
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), spaced);

    click(&mut app, 7.0, 10.0, ClickMode::Replace);
    click(&mut app, 9.0, 10.0, ClickMode::Add);
    run(&mut app, "swap_pins");

    assert!(translator::ran("swap_pins"));
    assert_eq!(
        pad_nets(&app, &path),
        nets(&[("1 ", "B"), ("2", "A"), ("3", "C"), ("4", "D")])
    );
}
