//! #2326 slice 6.P2b — the drawing order and Delete from their operands
//! (`document`, `ids`), as the CLI and an agent run them, and the menu's
//! Delete through the translator, which acts as the direct engine path did.

use std::path::{Path, PathBuf};

use pcb_geom::ItemRef;
use signex_i18n::RenderTranslatable;
use signex_types::schematic::{
    Point, SchDrawing, SchematicSheet, SelectedItem, SelectedKind, Symbol, Wire,
};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome, translator};
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;

const WIRE: u128 = 10;
const LOCKED_PART: u128 = 20;
const PART: u128 = 21;
/// A designator no other test logs, to count its lock notice.
const LOCKED_REF: &str = "R2326LOCK";

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn line(n: u128) -> SchDrawing {
    SchDrawing::Line {
        uuid: id(n),
        start: Point::new(n as f64, 0.0),
        end: Point::new(n as f64, 5.0),
        width: 0.25,
        stroke_color: None,
        locked: false,
    }
}

/// Sheet `n` (its document id, as a load gives every sheet one): drawings
/// 1..=3, a wire, a locked part and a free one.
fn sheet(n: u128) -> SchematicSheet {
    let part = |n: u128, reference: &str, locked: bool| Symbol {
        uuid: id(n),
        lib_id: "Device:R".into(),
        reference: reference.into(),
        position: Point::new(50.0 + n as f64, 50.0),
        locked,
        ..Default::default()
    };
    SchematicSheet {
        uuid: id(n),
        drawings: (1..=3).map(line).collect(),
        wires: vec![Wire {
            uuid: id(WIRE),
            start: Point::new(0.0, 20.0),
            end: Point::new(10.0, 20.0),
            stroke_width: 0.25,
            ..Default::default()
        }],
        symbols: vec![part(LOCKED_PART, LOCKED_REF, true), part(PART, "R2", false)],
        ..SchematicSheet::default()
    }
}

/// Sheets `a` and `b` open, `b` in view.
fn two_sheets(dir: &Path) -> (Signex, PathBuf, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let (a, b) = (dir.join("a.snxsch"), dir.join("b.snxsch"));
    add_sheet_with(&mut app, &a, sheet(0xa00));
    add_sheet_with(&mut app, &b, sheet(0xb00));
    (app, a, b)
}

fn by_operands(app: &mut Signex, command: &str, document: &Path, ids: &[u128]) -> CommandOutcome {
    let command = AppCommandId::new(command).expect("a catalog id");
    let document = format!("document={}", document.display());
    let ids: Vec<String> = ids.iter().map(|n| id(*n).to_string()).collect();
    let ids = format!("ids={}", ids.join(","));
    let args = CommandArgs::parse_operands(&command, &[document.as_str(), ids.as_str()])
        .expect("operands parse");
    app.run_operands(&command, &args).0
}

fn sheet_at<'a>(app: &'a Signex, path: &Path) -> &'a SchematicSheet {
    app.document_state.engines[path].document()
}

fn order(app: &Signex, path: &Path) -> Vec<u128> {
    let drawings = &sheet_at(app, path).drawings;
    drawings.iter().map(|d| d.uuid().as_u128()).collect()
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn an_agent_orders_the_drawings_of_a_sheet_not_in_view() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, a, b) = two_sheets(dir.path());
    let outcome = by_operands(&mut app, "bring_drawings_to_front", &a, &[1]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    assert_eq!(order(&app, &a), [2, 3, 1]);
    assert_eq!(order(&app, &b), [1, 2, 3], "the sheet in view is untouched");
    assert_eq!(
        app.document_state.engines[&a].undo_position(),
        1,
        "one step"
    );
}

#[test]
fn a_uuid_that_is_no_drawing_of_the_sheet_refuses_the_order() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, a, _) = two_sheets(dir.path());
    let why = refusal(by_operands(
        &mut app,
        "send_drawings_to_back",
        &a,
        &[3, WIRE],
    ));
    assert!(why.contains("names nothing on the sheet"), "{why}");
    assert_eq!(order(&app, &a), [1, 2, 3]);
}

#[test]
fn an_agent_deletes_sheet_objects_by_uuid_on_a_sheet_not_in_view() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, a, b) = two_sheets(dir.path());
    let outcome = by_operands(&mut app, "delete_selection", &a, &[WIRE, PART, 2]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    let left = sheet_at(&app, &a);
    assert!(left.wires.is_empty());
    assert_eq!(left.symbols.len(), 1, "only the free part goes");
    assert_eq!(order(&app, &a), [1, 3]);
    assert_eq!(
        sheet_at(&app, &b).wires.len(),
        1,
        "the sheet in view keeps its own"
    );
    assert_eq!(
        app.document_state.engines[&a].undo_position(),
        1,
        "one step"
    );
}

#[test]
fn a_uuid_that_names_nothing_refuses_the_whole_delete() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, a, _) = two_sheets(dir.path());
    let why = refusal(by_operands(&mut app, "delete_selection", &a, &[WIRE, 999]));
    assert!(why.contains("names nothing on the sheet"), "{why}");
    assert_eq!(sheet_at(&app, &a).wires.len(), 1);
}

/// The menu's Delete on a wire, a locked part's designator and a free
/// part's value deletes what the direct engine path deleted (the wire),
/// names the lock in Messages as it did, clears the selection and adds one
/// undo step: the old path, still Cut's, is the reference.
#[test]
fn the_menu_delete_on_a_sheet_acts_as_the_direct_engine_path() {
    let selection = vec![
        SelectedItem::new(id(WIRE), SelectedKind::Wire),
        SelectedItem::new(id(LOCKED_PART), SelectedKind::SymbolRefField),
        SelectedItem::new(id(PART), SelectedKind::SymbolValField),
    ];
    let run = |translated: bool| {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _, b) = two_sheets(dir.path());
        app.interaction_state.active_canvas_mut().selected = selection.clone();
        let named = crate::diagnostics::logged_count(LOCKED_REF);
        if translated {
            let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
            assert!(translator::ran("delete_selection"));
        } else {
            app.handle_selection_delete_requested();
        }
        let said = crate::diagnostics::logged_count(LOCKED_REF) - named;
        let left = serde_json::to_string(sheet_at(&app, &b)).unwrap();
        let selected = app.interaction_state.active_canvas().selected.clone();
        let steps = app.document_state.engines[&b].undo_position();
        (left, selected, steps, said)
    };
    let menu = run(true);
    assert_eq!(menu, run(false));
    assert!(menu.1.is_empty() && menu.2 == 1 && menu.3 == 1, "{menu:?}");
}

/// A board designator no other test logs.
const LOCKED_FOOTPRINT: &str = "U2326LOCKED";

/// U1 free at (10, 10), the locked part at (30, 10), one pad each.
fn board() -> signex_types::pcb::PcbBoard {
    let footprint = |n: u128, x: f64, locked: bool| {
        let reference = if locked { LOCKED_FOOTPRINT } else { "U1" };
        serde_json::json!({
            "uuid": id(n), "reference": reference, "position": {"x": x, "y": 10.0},
            "rotation": 0.0, "layer": "TopCopper", "locked": locked,
            "pads": [{"uuid": id(n * 16), "number": "1", "pad_type": "smd", "shape": "rect",
                      "position": {"x": 1.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
                      "drill": null, "layers": ["TopCopper"]}]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": id(100),
        "footprints": [footprint(1, 10.0, false), footprint(2, 30.0, true)]
    }))
    .unwrap()
}

fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

fn live(app: &Signex, path: &Path) -> String {
    serde_json::to_string(app.document_state.pcb_board(path).expect("board")).unwrap()
}

#[test]
fn the_menu_delete_on_a_board_matches_its_operand_form_and_clears_the_selection() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    app.select_pcb_items(&[ItemRef::Footprint(id(1))]);
    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    assert!(translator::ran("delete_selection"));
    let selected = app.with_pcb_selection(|sel, _| sel.selection().items().count());
    assert_eq!(selected, Some(0), "the selection clears");

    let other = tempfile::tempdir().unwrap();
    let (mut agent, agent_path) = app_with_board(other.path());
    let outcome = by_operands(&mut agent, "delete_selection", &agent_path, &[1]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    assert_eq!(live(&app, &path), live(&agent, &agent_path));
}

/// The locked part is refused in Messages in the menu's own words, as
/// before the translator.
#[test]
fn the_menu_delete_of_a_locked_part_says_why_once() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    app.select_pcb_items(&[ItemRef::Footprint(id(2))]);
    let said = crate::diagnostics::logged_count(LOCKED_FOOTPRINT);
    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    assert_eq!(crate::diagnostics::logged_count(LOCKED_FOOTPRINT), said + 1);
    let line = format!("Delete: {LOCKED_FOOTPRINT}");
    assert_eq!(
        crate::diagnostics::logged_count(&line),
        1,
        "the menu's words"
    );
    assert_eq!(live(&app, &path), before);
}

#[test]
fn a_board_uuid_that_is_no_item_refuses_the_delete() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    // A pad's uuid: a pad goes with its footprint, it is no board item.
    let why = refusal(by_operands(&mut app, "delete_selection", &path, &[1, 16]));
    assert!(why.contains("names nothing on the board"), "{why}");
    assert_eq!(live(&app, &path), before);
}
