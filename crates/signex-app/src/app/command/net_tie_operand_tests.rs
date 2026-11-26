//! #1277 (Caner, 2026-11-25): `set_net_tie` from operands (the CLI and agent
//! path; no Properties row yet) and from the palette, which has none, and
//! `query_net_ties` on a sheet and on a board.

use std::path::PathBuf;

use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_types::pcb::{ComponentType, PcbBoard};
use signex_types::schematic::{
    Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet,
    Symbol, Wire,
};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::command::queries::tests::answer;
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::keymap::AppCommandId;

const TIE: u128 = 0x1277_0001;
const PORT: u128 = 0x1277_0002;
const LABEL: u128 = 0x1277_0003;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn sheet_path() -> PathBuf {
    std::env::temp_dir().join("snx-1277").join("tie.snxsch")
}

fn pin(number: &str, x: f64) -> LibPin {
    LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 0.0,
            name: String::new(),
            number: number.into(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

fn wire(a: (f64, f64), b: (f64, f64)) -> Wire {
    Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        ..Default::default()
    }
}

/// NT1 at (10, 10), not yet a tie: pin 1 wired to a `GND` label, pin 2 to
/// an `AGND` label; a power port and a label to be refused.
fn sheet() -> SchematicSheet {
    let mut body: LibSymbol = serde_json::from_value(serde_json::json!({ "id": "NT" })).unwrap();
    body.pins = vec![pin("1", 0.0), pin("2", 5.0)];
    let mut sheet = SchematicSheet {
        paper_size: "A4".into(),
        ..Default::default()
    };
    sheet.lib_symbols.insert("NT".into(), body);
    sheet.symbols.push(Symbol {
        uuid: id(TIE),
        lib_id: "NT".into(),
        reference: "NT1".into(),
        position: Point::new(10.0, 10.0),
        ..Default::default()
    });
    sheet.symbols.push(Symbol {
        uuid: id(PORT),
        lib_id: "power:GND".into(),
        reference: "#PWR1".into(),
        value: "GND".into(),
        position: Point::new(60.0, 10.0),
        is_power: true,
        ..Default::default()
    });
    sheet.wires.push(wire((10.0, 10.0), (10.0, 20.0)));
    sheet.wires.push(wire((15.0, 10.0), (15.0, 20.0)));
    let mut gnd = Label::new("GND", Point::new(10.0, 20.0), LabelType::Net);
    gnd.uuid = id(LABEL);
    sheet.labels.push(gnd);
    sheet
        .labels
        .push(Label::new("AGND", Point::new(15.0, 20.0), LabelType::Net));
    sheet
}

fn app() -> Signex {
    let (mut app, _task) = Signex::new();
    add_sheet_with(&mut app, &sheet_path(), sheet());
    app
}

fn run(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new("set_net_tie").expect("a catalog id");
    let args = CommandArgs::parse_operands(&id, operands).expect("operands");
    app.run_operands_settled(&id, &args).0
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn is_tie(app: &Signex) -> bool {
    let engine = app.document_state.active_engine().expect("engine");
    engine
        .document()
        .symbols
        .iter()
        .any(|s| s.uuid == id(TIE) && s.net_tie)
}

fn undo(app: &mut Signex) {
    let undo = AppCommandId::new("undo").expect("id");
    let _ = app.dispatch_command_reporting(&undo, CommandArgs::none(), None);
}

fn ties_of(app: &mut Signex, path: &std::path::Path) -> Value {
    let document = format!("document={}", path.display());
    answer(app, "query_net_ties", &[&document])
}

#[test]
fn the_command_makes_a_part_a_tie_and_back_one_undo_step_each() {
    let mut app = app();
    let tie = format!("id={}", id(TIE));
    let CommandOutcome::Answered(answer) = run(&mut app, &[&tie, "on=true"]) else {
        panic!("not answered");
    };
    assert_eq!(answer["net_tie"], true);
    assert_eq!(answer["kind"], "symbol");
    assert!(is_tie(&app));
    assert!(matches!(
        run(&mut app, &[&tie, "on=false"]),
        CommandOutcome::Answered(_)
    ));
    assert!(!is_tie(&app));
    undo(&mut app);
    assert!(is_tie(&app));
    undo(&mut app);
    assert!(!is_tie(&app));
}

#[test]
fn a_power_port_another_object_or_an_unknown_id_is_refused() {
    let mut app = app();
    for (target, needle) in [
        (id(PORT), "power port"),
        (id(LABEL), "only a placed part"),
        (Uuid::from_u128(99), "no placed object"),
    ] {
        let target = format!("id={target}");
        let reason = refusal(run(&mut app, &[&target, "on=true"]));
        assert!(reason.contains(needle), "{target}: {reason}");
    }
    assert!(!is_tie(&app));
}

/// The palette and a key give no operands: the command says what it needs.
#[test]
fn without_operands_the_command_says_what_it_needs() {
    let mut app = app();
    let command = AppCommandId::new("set_net_tie").expect("id");
    let (outcome, _task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
    let reason = refusal(outcome);
    assert!(reason.contains("`id` and `on`"), "{reason}");
    assert!(!is_tie(&app));
}

#[test]
fn the_query_lists_a_sheets_ties_with_the_nets_each_joins() {
    let mut app = app();
    assert_eq!(ties_of(&mut app, &sheet_path())["count"], 0);
    run(&mut app, &[&format!("id={}", id(TIE)), "on=true"]);
    let answer = ties_of(&mut app, &sheet_path());
    assert_eq!(answer["kind"], "schematic");
    assert_eq!(answer["count"], 1);
    let tie = &answer["ties"][0];
    assert_eq!(tie["id"], id(TIE).to_string());
    assert_eq!(tie["designator"], "NT1");
    assert_eq!(tie["nets"], serde_json::json!(["AGND", "GND"]));
    assert_eq!(tie["status"], "joins");
    assert_eq!(
        tie["pins"],
        serde_json::json!([{"pin": "1", "net": "GND"}, {"pin": "2", "net": "AGND"}])
    );
}

/// The headless path (the CLI): a project sheet that is not open is read
/// for the answer, as `query_pins` reads one, and dropped after it.
#[test]
fn the_headless_query_reads_a_closed_project_sheet() {
    use crate::app::handlers::menu::export::tests::current_placement::placed_twice_listing;

    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_listing(&dir, &["top.snxsch", "child.snxsch"]);
    assert!(!app.document_state.engines.contains_key(&child));
    let command = AppCommandId::new("query_net_ties").unwrap();
    let document = format!("document={}", child.display());
    let args = CommandArgs::parse_operands(&command, &[&document]).unwrap();
    let CommandOutcome::Answered(answer) = app.run_query_headless(&command, &args).unwrap().0
    else {
        panic!("not answered");
    };
    assert_eq!(answer["kind"], "schematic");
    assert_eq!(answer["count"], 0, "the child sheet holds no tie");
    assert!(!app.document_state.engines.contains_key(&child), "dropped");
}

#[test]
fn the_query_lists_a_boards_net_tie_footprints_with_the_nets_they_bridge() {
    let mut app = app();
    let pad = |n: u128, number: &str, net: u32, name: &str| {
        serde_json::json!({
            "uuid": Uuid::from_u128(n), "number": number, "pad_type": "smd",
            "shape": "rect", "position": {"x": 0.0, "y": 0.0},
            "size": {"x": 0.6, "y": 1.0}, "drill": null, "layers": ["TopCopper"],
            "net": {"number": net, "name": name}
        })
    };
    let mut board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::nil(),
        "footprints": [{
            "uuid": Uuid::from_u128(0x10), "reference": "NT1", "value": "",
            "footprint_id": "NetTie_2", "position": {"x": 0.0, "y": 0.0},
            "rotation": 0.0, "layer": "TopCopper", "properties": [],
            "pads": [pad(0x11, "1", 1, "GND"), pad(0x12, "2", 2, "AGND")]
        }]
    }))
    .unwrap();
    board.footprints[0].component.component_type = ComponentType::NetTie;
    let path = std::env::temp_dir().join("snx-1277").join("tie.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);

    let answer = ties_of(&mut app, &path);
    assert_eq!(answer["kind"], "pcb");
    assert_eq!(answer["count"], 1);
    let tie = &answer["ties"][0];
    assert_eq!(tie["designator"], "NT1");
    assert_eq!(tie["component_type"], "net_tie");
    assert_eq!(tie["nets"], serde_json::json!(["AGND", "GND"]));
    assert_eq!(tie["status"], "joins");
    assert_eq!(
        tie["pads"][0],
        serde_json::json!({"pad": "1", "net": "GND"})
    );
}
