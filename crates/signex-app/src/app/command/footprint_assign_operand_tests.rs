//! #2937: a part's footprint assigned, a board footprint swapped and every
//! part's footprint read back, from operands, against the bundled Stock
//! library: the resistor row offers Chip-0805 (its default), Chip-0603 and
//! other chip sizes; Fiducial-1mm has no pad 1 or 2.

use std::path::{Path, PathBuf};

use signex_types::pcb::{PcbBoard, Point};
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::path_ops::settle;
use crate::app::{AssignFootprintsMsg, EditMsg, Message, Signex, TabInfo, TabKind};
use crate::keymap::AppCommandId;

/// The bundled Stock library's id (`assets/stock-library/Stock.snxlib`).
const STOCK: &str = "019df45d-af5e-7a00-8c3b-dd3bf89b40ae";
const R1: Uuid = Uuid::from_u128(7);

fn stock_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib")
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

fn answered(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => {
            signex_i18n::RenderTranslatable::render_en_us(&reason)
        }
        other => panic!("not refused: {other:?}"),
    }
}

/// A project whose sheet `a.snxsch` is open and in view with the Stock
/// resistor placed on it as the Components panel places it (R1), and the
/// Stock library mounted.
fn app_with_resistor(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch"]);
    app.ui_state.first_run_tour_open = false;
    let stock = stock_path();
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock.clone())
        .expect("mount stock");
    let project = app.document_state.projects[0].id;
    app.document_state.active_project = Some(project);
    let sheet = dir.join("a.snxsch");
    let blank = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let engine = signex_engine::Engine::new_with_path(blank, Some(sheet.clone())).expect("engine");
    app.document_state.engines.insert(sheet.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "a".into(),
        path: sheet.clone(),
        project_id: Some(project),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(sheet.clone());
    let library = format!("library={}", stock.display());
    let placed = run(
        &mut app,
        "place_component",
        &[&library, "part=STOCK-R", "at=254,254"],
    );
    assert!(matches!(placed, CommandOutcome::Answered(_)), "{placed:?}");
    (app, sheet)
}

fn footprint_of(app: &Signex, sheet: &Path) -> String {
    app.document_state.engines[sheet].document().symbols[0]
        .footprint
        .clone()
}

fn assign(app: &mut Signex, sheet: &Path, footprint: &str) -> CommandOutcome {
    let document = format!("document={}", sheet.display());
    let footprint = format!("footprint={STOCK}:{footprint}");
    run(
        app,
        "assign_footprint",
        &[&document, "designator=R1", &footprint],
    )
}

#[test]
fn an_assignment_is_what_assign_footprints_applies_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, sheet) = app_with_resistor(dir.path());
    let before = footprint_of(&app, &sheet);
    let answer = answered(assign(&mut app, &sheet, "Chip-0603"));
    assert_eq!(answer["changed"], true, "{answer}");
    assert_eq!(answer["parts"][0]["designator"], "R1");
    let by_command = footprint_of(&app, &sheet);
    assert_eq!(answer["parts"][0]["footprint"], by_command.as_str());
    assert_ne!(by_command, before);

    // The dialog's Apply on a second project gives the part the same text.
    let other = tempfile::tempdir().unwrap();
    let (mut gui, gui_sheet) = app_with_resistor(other.path());
    let menu_row = Message::Menu(crate::menu_bar::MenuMessage::Command(
        AppCommandId::new("assign_footprints").expect("a command id"),
    ));
    settle(&mut gui, menu_row);
    let binding = format!("{STOCK}:01a19e33-2798-728e-8be0-44de7d2aebed");
    settle(
        &mut gui,
        Message::from(AssignFootprintsMsg::FootprintPicked { row: 0, binding }),
    );
    settle(&mut gui, Message::from(AssignFootprintsMsg::ApplyRequested));
    assert_eq!(footprint_of(&gui, &gui_sheet), by_command);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(footprint_of(&app, &sheet), before, "one undo step");
}

#[test]
fn an_unknown_footprint_or_one_without_the_pins_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, sheet) = app_with_resistor(dir.path());
    let before = footprint_of(&app, &sheet);
    let unknown = refusal(assign(&mut app, &sheet, "NoSuchFootprint"));
    assert!(unknown.contains("no footprint NoSuch"), "{unknown}");
    let mismatch = refusal(assign(&mut app, &sheet, "Fiducial-1mm"));
    assert!(mismatch.contains("R1 pins 1, 2"), "{mismatch}");
    assert_eq!(footprint_of(&app, &sheet), before, "nothing changed");
}

#[test]
fn the_assignment_query_lists_each_part_and_whether_it_resolves() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, sheet) = app_with_resistor(dir.path());
    let document = format!("document={}", sheet.display());
    let answer = answered(run(&mut app, "query_footprint_assignments", &[&document]));
    let parts = answer["parts"].as_array().expect("parts");
    assert_eq!(parts.len(), 1, "{answer}");
    assert_eq!(parts[0]["designator"], "R1");
    assert!(parts[0]["value"].is_string(), "{answer}");
    assert_eq!(parts[0]["footprint"], footprint_of(&app, &sheet).as_str());
    assert_eq!(parts[0]["resolves"], true, "{answer}");
}

/// R1 with pads 1 and 2, at (10, 10) turned 90 degrees on the bottom, on a
/// board open beside the sheet.
fn open_board(app: &mut Signex, dir: &Path) -> PathBuf {
    let path = dir.join("board.snxpcb");
    let pad = |n: u128, number: &str, x: f64| {
        serde_json::json!({
            "uuid": Uuid::from_u128(n), "number": number, "pad_type": "smd", "shape": "rect",
            "position": {"x": x, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
            "drill": null, "layers": ["BottomCopper"]
        })
    };
    let board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "footprints": [{
            "uuid": R1, "reference": "R1", "position": {"x": 10.0, "y": 10.0},
            "rotation": 90.0, "layer": "BottomCopper", "locked": false,
            "pads": [pad(70, "1", -1.0), pad(71, "2", 1.0)]
        }]
    }))
    .unwrap();
    app.open_pcb_tab(path.clone(), "board".into(), board);
    path
}

fn swap(app: &mut Signex, board: &Path, footprint: &str) -> CommandOutcome {
    let document = format!("document={}", board.display());
    let id = format!("id={R1}");
    let footprint = format!("footprint={STOCK}:{footprint}");
    run(app, "change_footprint", &[&document, &id, &footprint])
}

#[test]
fn a_board_swap_keeps_the_place_turn_and_side_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _sheet) = app_with_resistor(dir.path());
    let board = open_board(&mut app, dir.path());
    let answer = answered(swap(&mut app, &board, "Chip-0603"));
    assert_eq!(answer["changed"], true, "{answer}");
    let fp = &answer["footprints"][0];
    assert_eq!(fp["designator"], "R1");
    assert_eq!(fp["rotation"], 90.0);
    assert_eq!(fp["side"], "bottom");
    let swapped = app.document_state.pcb_board(&board).expect("board").clone();
    let r1 = swapped.footprints.iter().find(|f| f.uuid == R1).unwrap();
    assert_eq!(r1.position, Point::new(10.0, 10.0));
    assert!(r1.library_ref.is_some(), "the new footprint is in");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let back = app.document_state.pcb_board(&board).expect("board");
    let r1 = back.footprints.iter().find(|f| f.uuid == R1).unwrap();
    assert!(r1.library_ref.is_none(), "one undo step");
}

#[test]
fn a_board_swap_to_a_footprint_without_the_pads_is_refused_with_them() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _sheet) = app_with_resistor(dir.path());
    let board = open_board(&mut app, dir.path());
    let why = refusal(swap(&mut app, &board, "Fiducial-1mm"));
    assert!(why.contains("R1.1, R1.2"), "{why}");
    let unknown = refusal(swap(&mut app, &board, "NoSuchFootprint"));
    assert!(unknown.contains("has no footprint"), "{unknown}");
    let kept = app.document_state.pcb_board(&board).expect("board");
    assert!(kept.footprints[0].library_ref.is_none(), "nothing changed");
}

/// `unmatched=keep` swaps anyway and answers the pads that went, as the
/// footprint picker swaps.
#[test]
fn a_board_swap_that_keeps_unmatched_pads_swaps_and_names_them() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _sheet) = app_with_resistor(dir.path());
    let board = open_board(&mut app, dir.path());
    let document = format!("document={}", board.display());
    let id = format!("id={R1}");
    let footprint = format!("footprint={STOCK}:Fiducial-1mm");
    let answer = answered(run(
        &mut app,
        "change_footprint",
        &[&document, &id, &footprint, "unmatched=keep"],
    ));
    assert_eq!(answer["changed"], true, "{answer}");
    assert_eq!(
        answer["pads_dropped"],
        serde_json::json!(["R1.1", "R1.2"]),
        "{answer}"
    );
    let bad = refusal(run(
        &mut app,
        "change_footprint",
        &[&document, &id, &footprint, "unmatched=maybe"],
    ));
    assert!(bad.contains("`refuse` or `keep`"), "{bad}");
}

/// The picker's Swap runs `change_footprint` through the GUI translator,
/// with the picked footprint as read and `unmatched=keep`: the pads with no
/// match go, as the picker always swapped.
#[test]
fn the_pickers_swap_runs_change_footprint_through_the_translator() {
    use crate::app::command::translator;
    use crate::app::{FootprintSwapMsg, PcbMsg};

    let dir = tempfile::tempdir().unwrap();
    let (mut app, _sheet) = app_with_resistor(dir.path());
    let board = open_board(&mut app, dir.path());
    let library = app
        .library
        .open_libraries
        .iter()
        .find(|lib| lib.library_id.to_string() == STOCK)
        .expect("the Stock library");
    let fiducial = library
        .cached_footprints
        .iter()
        .find(|s| s.name == "Fiducial-1mm")
        .expect("Fiducial-1mm")
        .uuid;
    let address = signex_library::PrimitiveRef::new(library.library_id, fiducial);
    let roots = app.open_library_roots();
    let template = crate::library::place::resolve_library_footprint(&address, &roots)
        .expect("the footprint reads");
    let _ = app.update(Message::Pcb(PcbMsg::FootprintSwap(
        FootprintSwapMsg::Resolved {
            board: board.clone(),
            footprints: vec![R1],
            result: Ok(Box::new(template)),
        },
    )));
    assert!(translator::ran("change_footprint"), "the picker translates");
    let swapped = app.document_state.pcb_board(&board).expect("board");
    let r1 = swapped.footprints.iter().find(|f| f.uuid == R1).unwrap();
    assert!(
        r1.library_ref.is_some(),
        "the unmatched pads went and the swap was made"
    );
}
