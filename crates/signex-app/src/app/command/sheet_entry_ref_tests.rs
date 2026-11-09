//! #2199 — a sheet-entry reference, `<sheet name>.<entry name>`, as a point
//! operand: the sheet pin's connection point, judged like a named part pin.

use std::path::Path;

use signex_types::format::SnxSchematic;
use signex_types::schematic::{
    ChildSheet, FillType, Label, LabelType, Point, SchematicSheet, SheetPin, Wire,
};
use uuid::Uuid;

use super::wire_run_tests::{part, resistor};
use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::keymap::AppCommandId;

/// U1 at (12.7, 12.7): its pin 2 tip is (16.51, 12.7), pin 1 (8.89, 12.7).
const PIN_2: (f64, f64) = (16.51, 12.7);
/// The entry `SDA` of the sheet `Charger`.
const SDA: (f64, f64) = (30.48, 12.7);

fn sheet_symbol(name: &str, entries: &[(&str, (f64, f64))]) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::now_v7(),
        name: name.into(),
        filename: "charger.snxsch".into(),
        position: Point::new(30.48, 8.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: entries
            .iter()
            .map(|(pin, at)| SheetPin {
                uuid: Uuid::now_v7(),
                name: (*pin).into(),
                direction: "input".into(),
                position: Point::new(at.0, at.1),
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            })
            .collect(),
        instances: Vec::new(),
    }
}

/// The parent: part U1 and the sheet `Charger` with the entries `SDA` and `SCL`.
fn top() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.lib_symbols.insert("R".into(), resistor());
    sheet.symbols.push(part("U1", Point::new(12.7, 12.7)));
    sheet.child_sheets.push(sheet_symbol(
        "Charger",
        &[("SDA", SDA), ("SCL", (30.48, 20.32))],
    ));
    sheet
}

/// The child names its net `SDA`.
fn child() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.wires.push(Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.16, 0.0),
        ..Default::default()
    });
    sheet.labels.push(Label::new(
        "SDA",
        Point::new(0.0, 0.0),
        LabelType::Hierarchical,
    ));
    sheet
}

fn write(path: &Path, sheet: SchematicSheet) {
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(path, text).expect("write sheet");
}

fn app_of(dir: &tempfile::TempDir, top: SchematicSheet) -> Signex {
    let _ = crate::diagnostics::init_logging();
    write(&dir.path().join("top.snxsch"), top);
    write(&dir.path().join("charger.snxsch"), child());
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["top.snxsch"]);
    app.ui_state.snap_enabled = false;
    app.open_or_focus_sheet_path(dir.path().join("top.snxsch"));
    app.refresh_panel_ctx();
    // Designators are known once the project's hierarchy is read.
    crate::app::runtime::sheet_tree_tests::settle(&mut app, true);
    app
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&id, operands).expect("operands parse");
    app.run_operands(&id, &args).0
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn a_wire_from_a_part_pin_to_a_sheet_entry_lands_on_the_sheet_pin_and_joins_the_child_net() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_of(&dir, top());
    let outcome = run(&mut app, "place_wire", &["start=U1.2", "end=Charger.SDA"]);
    let CommandOutcome::Answered(report) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    // The answer carries the references as given, and where they resolved.
    assert_eq!(report["ends"][0]["ref"], "U1.2");
    assert_eq!(report["ends"][1]["ref"], "Charger.SDA");
    assert_eq!(report["ends"][1]["at"], serde_json::json!([SDA.0, SDA.1]));
    // The project netlist puts U1.2 on the child's `SDA` net.
    let top_path = dir.path().join("top.snxsch");
    let sheets = [
        (
            top_path.clone(),
            app.document_state.engines[&top_path].document().clone(),
        ),
        (dir.path().join("charger.snxsch"), child()),
    ]
    .into_iter()
    .collect();
    let graph =
        crate::app::project_sheets::project_graph(&sheets, Some(dir.path()), Some(&top_path));
    let roots = [signex_net::ProjectRoot {
        key: crate::app::project_sheets::sheet_key(&top_path, Some(dir.path())),
    }];
    let project = signex_net::ProjectGraph {
        sheets: &graph.sheets,
        resolved: &graph.resolved,
        roots: &roots,
    };
    let netlist = signex_net::build_project_netlist(&project).netlist;
    let net = netlist
        .nets
        .iter()
        .find(|net| {
            net.terminals
                .iter()
                .any(|t| t.reference == "U1" && t.pin == "2")
        })
        .expect("a net with U1.2");
    assert!(net.name.contains("SDA"), "{}", net.name);
    assert!(net.name.contains("Charger"), "{}", net.name);
}

#[test]
fn a_sheet_entry_reference_is_refused_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_of(&dir, top());
    for (reference, reason) in [
        ("Nope.SDA", "no part `Nope`"),
        ("Charger.XYZ", "sheet `Charger` has no entry `XYZ`"),
    ] {
        let said = refusal(run(
            &mut app,
            "place_wire",
            &["start=U1.2", &format!("end={reference}")],
        ));
        assert!(said.contains(reason), "{said}");
    }
    // A bus never takes one.
    let said = refusal(run(
        &mut app,
        "place_bus",
        &["start=Charger.SDA", "end=40.64,12.7"],
    ));
    assert!(said.contains("a bus does not connect to pins"), "{said}");

    // Two sheet symbols with one name, two entries with one name, and a name
    // that a part and a sheet both carry.
    let mut twice = top();
    twice
        .child_sheets
        .push(sheet_symbol("Charger", &[("SDA", (30.48, 40.64))]));
    let mut doubled = top();
    doubled.child_sheets[0]
        .pins
        .push(sheet_symbol("x", &[("SDA", (30.48, 30.48))]).pins.remove(0));
    let mut both = top();
    both.child_sheets
        .push(sheet_symbol("U1", &[("SDA", (30.48, 40.64))]));
    for (sheet, reason) in [
        (twice, "2 sheet symbols on this sheet are named `Charger`"),
        (doubled, "sheet `Charger` has 2 entries named `SDA`"),
        (both, "`U1` names both a part and a sheet"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_of(&dir, sheet);
        let reference = if reason.contains("`U1`") {
            "U1.SDA"
        } else {
            "Charger.SDA"
        };
        let said = refusal(run(
            &mut app,
            "place_wire",
            &["start=U1.2", &format!("end={reference}")],
        ));
        assert!(said.contains(reason), "{said}");
    }
}

#[test]
fn a_sheet_entry_end_names_only_that_pin_and_a_net_label_on_it_is_judged_across_the_project() {
    // U1.2's tip sits on the entry's point: a wire from the entry would also
    // pull that part pin in, which a reference to the entry did not name.
    let mut butted = top();
    butted.child_sheets[0].pins[0].position = Point::new(PIN_2.0, PIN_2.1);
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_of(&dir, butted);
    let said = refusal(run(
        &mut app,
        "place_wire",
        &["start=Charger.SDA", "end=16.51,25.4"],
    ));
    assert!(said.contains("pin U1.2"), "{said}");

    // A local label that names the child's net differently is a second name
    // for one signal: refused unless allow_merge.
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_of(&dir, top());
    run(&mut app, "place_wire", &["start=U1.2", "end=Charger.SDA"]);
    let said = refusal(run(
        &mut app,
        "place_net_label",
        &["at=Charger.SDA", "text=OTHER"],
    ));
    assert!(said.contains("reach it from other sheets"), "{said}");
    let outcome = run(
        &mut app,
        "place_net_label",
        &["at=Charger.SDA", "text=OTHER", "allow_merge=true"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
}
