//! #2199 — a wire or bus run from operands: pin references, the intent check
//! and the geometry rules, the escapes, the dry run, and the click it equals.

use serde_json::{Value, json};
use signex_types::schematic::{
    Bus, BusEntry, LabelType, LibGraphic, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle,
    Point, SchematicSheet, Symbol,
};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::handlers::canvas::placement_tests::{app_with_tool, click, grid, sheet_shape};
use crate::app::{DrawMode, Signex, Tool};
use crate::keymap::AppCommandId;

/// A resistor with a 2.54 x 1.016 body and pins 1 and 2 at x = -3.81 and +3.81.
pub(super) fn resistor() -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(json!({ "id": "R" })).unwrap();
    lib.graphics = vec![LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: signex_types::schematic::Graphic::Rectangle {
            start: Point::new(-1.27, -0.508),
            end: Point::new(1.27, 0.508),
            width: 0.0,
            fill: signex_types::schematic::FillType::None,
        },
    }];
    let pin = |number: &str, x: f64| LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: format!("N{number}"),
            number: number.into(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    };
    lib.pins = vec![pin("1", -3.81), pin("2", 3.81)];
    lib
}

pub(super) fn part(reference: &str, at: Point) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "R".into(),
        reference: reference.into(),
        position: at,
        unit: 1,
        ..Symbol::default()
    }
}

fn wire(a: Point, b: Point) -> signex_types::schematic::Wire {
    signex_types::schematic::Wire {
        start: a,
        end: b,
        ..Default::default()
    }
}

fn label(text: &str, at: Point) -> signex_types::schematic::Label {
    signex_types::schematic::Label::new(text, at, LabelType::Net)
}

/// R1 at (12.7, 12.7) with tips (8.89, 12.7) and (16.51, 12.7); C1 at
/// (38.1, 12.7) with tips (34.29, 12.7) and (41.91, 12.7): all on the grid.
fn two_parts() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.lib_symbols.insert("R".into(), resistor());
    sheet.symbols = vec![part("R1", grid(10.0, 10.0)), part("C1", grid(30.0, 10.0))];
    sheet
}

/// `sheet` as the active sheet of an app with the select tool.
fn app_with(sheet: SchematicSheet) -> Signex {
    let mut app = app_with_tool(Tool::Select);
    let path = app.document_state.active_path.clone().expect("a sheet");
    app.document_state
        .engines
        .insert(path, signex_engine::Engine::new(sheet).expect("engine"));
    app
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&id, operands).expect("operands parse");
    app.run_operands(&id, &args).0
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("not refused: {other:?}"),
    }
}

fn document(app: &Signex) -> SchematicSheet {
    app.document_state
        .edit_engine()
        .expect("a sheet")
        .document()
        .clone()
}

/// A refused run changes nothing.
fn refused_with(app: &mut Signex, id: &str, operands: &[&str], fragment: &str) {
    let before = serde_json::to_vec(&document(app)).unwrap();
    let reason = refusal(run(app, id, operands));
    assert!(reason.contains(fragment), "{id} {operands:?}: {reason}");
    assert_eq!(
        serde_json::to_vec(&document(app)).unwrap(),
        before,
        "{reason}"
    );
}

/// [`sheet_shape`] with every number rounded to a nanometre: a pin tip is the
/// part's origin plus an offset, the grid point a click lands on is a product,
/// and the two can differ in the last bit.
fn nm_shape(app: &Signex) -> Value {
    fn round(value: &mut Value) {
        match value {
            Value::Number(n) if n.is_f64() => {
                let nm = (n.as_f64().unwrap_or_default() * 1e6).round() / 1e6;
                *value = json!(nm);
            }
            Value::Object(map) => map.values_mut().for_each(round),
            Value::Array(items) => items.iter_mut().for_each(round),
            _ => {}
        }
    }
    let mut shape = sheet_shape(app);
    round(&mut shape);
    shape
}

#[test]
fn a_wire_between_two_pin_references_equals_two_clicks_on_the_tips() {
    for id in ["place_wire", "draw_wire"] {
        let mut app = app_with(two_parts());
        let empty = sheet_shape(&app);
        let report = answered(run(&mut app, id, &["start=R1.2", "end=C1.1"]));
        assert_ne!(sheet_shape(&app), empty, "{id} placed nothing");

        // Two clicks on the same tips, which a wire click pulls onto.
        let mut clicked = app_with(two_parts());
        clicked.ui_state.snap_hotspots = true;
        let (a, b) = (
            clicked.pin_ref_tip("R1", "2").unwrap(),
            clicked.pin_ref_tip("C1", "1").unwrap(),
        );
        let _ = clicked.dispatch_command_reporting(
            &AppCommandId::new(id).unwrap(),
            CommandArgs::none(),
            None,
        );
        click(&mut clicked, (a.x, a.y));
        click(&mut clicked, (b.x, b.y));
        assert_eq!(nm_shape(&app), nm_shape(&clicked), "{id}");

        assert_eq!(report["dry_run"], false);
        assert_eq!(report["net"]["pins"], json!(["C1.1", "R1.2"]));
        assert_eq!(report["merged"], json!([]));
        // Both ends carry the reference and the point it resolved to.
        assert_eq!(report["ends"][0]["ref"], "R1.2");
        assert_eq!(report["ends"][1]["ref"], "C1.1");
        assert_eq!(report["ends"][0]["at"], json!([a.x, a.y]));
        assert_eq!(report["ends"][1]["at"], json!([b.x, b.y]));
        let segment = report["segments"][0].as_array().unwrap();
        let wanted = [a.x, a.y, b.x, b.y];
        assert!(
            segment
                .iter()
                .zip(wanted)
                .all(|(got, want)| (got.as_f64().unwrap() - want).abs() < 1e-9),
            "{segment:?}"
        );
    }
}

#[test]
fn a_pin_reference_resolves_an_off_grid_tip_that_a_coordinate_is_refused_for() {
    let mut sheet = two_parts();
    sheet.symbols.push(part("R3", Point::new(50.1, 30.3)));
    let mut app = app_with(sheet);
    // Tip of R3 pin 1: (46.29, 30.3), off the 1.27 grid.
    refused_with(&mut app, "place_junction", &["at=46.29,30.3"], "grid");
    let outcome = run(&mut app, "place_junction", &["at=R3.1"]);
    assert_eq!(outcome, CommandOutcome::Ran);
    let junction = document(&app).junctions[0].position;
    assert!((junction.x - 46.29).abs() < 1e-9 && (junction.y - 30.3).abs() < 1e-9);
    // The label of any point operand takes one as well.
    let outcome = run(&mut app, "place_net_label", &["at=R3.2", "text=A"]);
    assert_eq!(outcome, CommandOutcome::Ran);
}

#[test]
fn a_label_that_would_rename_the_net_at_its_point_is_refused_unless_allowed() {
    // R1.2 is on the net VCC (a stub with a VCC label).
    for id in ["place_net_label", "place_global_label"] {
        let mut app = app_with(two_named_nets());
        // By pin reference and by coordinate (the stub's interior).
        for at in ["at=R1.2", "at=16.51,10.16"] {
            refused_with(
                &mut app,
                id,
                &[at, "text=GND"],
                // Joined by name to the GND net: a short of VCC with GND.
                "pass allow_merge=true to merge on purpose",
            );
        }
        let before = sheet_shape(&app);
        let outcome = run(&mut app, id, &["at=R1.2", "text=GND", "allow_merge=true"]);
        assert_eq!(outcome, CommandOutcome::Ran, "{id}");
        assert_ne!(sheet_shape(&app), before, "{id}");
    }
    // A hierarchical label is a port, not a name of the net: on a loose sheet
    // it renames nothing (#2199 comment 6761836527 point 5). Through a wired
    // entry the
    // project check sees the short (`project_name_tests`).
    let mut app = app_with(two_named_nets());
    let outcome = run(
        &mut app,
        "place_hierarchical_label",
        &["at=R1.2", "text=GND"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
}

#[test]
fn a_label_with_the_same_name_or_on_an_unnamed_net_or_on_nothing_is_allowed() {
    let mut sheet = two_named_nets();
    // C1.2 has an unnamed stub.
    sheet
        .wires
        .push(wire(Point::new(41.91, 12.7), Point::new(41.91, 7.62)));
    for operands in [
        ["at=R1.2", "text=VCC"],
        ["at=C1.2", "text=SENSE"],
        ["at=41.91,10.16", "text=OTHER"],
        ["at=63.5,63.5", "text=ANY"],
    ] {
        let mut app = app_with(sheet.clone());
        let outcome = run(&mut app, "place_net_label", &operands);
        assert_eq!(outcome, CommandOutcome::Ran, "{operands:?}");
    }
}

#[test]
fn a_power_port_that_would_rename_the_net_is_refused_unless_allowed() {
    let mut app = app_with(two_named_nets());
    let gnd = ["net_name=GND", "lib_id=power:GND"];
    refused_with(
        &mut app,
        "place_power_symbol",
        &["at=R1.2", gnd[0], gnd[1]],
        "would add GND to the net {VCC}",
    );
    let outcome = run(
        &mut app,
        "place_power_symbol",
        &["at=R1.2", gnd[0], gnd[1], "allow_merge=true"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    let mut app = app_with(two_named_nets());
    let outcome = run(
        &mut app,
        "place_power_symbol",
        &["at=R1.2", "net_name=VCC", "lib_id=power:VCC"],
    );
    assert_eq!(outcome, CommandOutcome::Ran, "the same name is no short");
}

#[test]
fn a_wire_passing_under_a_referenced_pin_is_not_joined_silently() {
    // A GND wire runs over R1.2's tip with no end or junction there, so R1.2
    // is on no net; a run to it would join GND.
    let mut sheet = two_parts();
    sheet
        .wires
        .push(wire(Point::new(16.51, 5.08), Point::new(16.51, 20.32)));
    sheet.labels.push(label("GND", Point::new(16.51, 5.08)));
    let mut app = app_with(sheet);
    refused_with(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=C1.1"],
        "also touches",
    );
    // A coordinate on the pin tip is no T either.
    refused_with(
        &mut app,
        "place_wire",
        &["start=16.51,12.7", "end=C1.1"],
        "also touches",
    );
    // A plain coordinate on the wire is the T the caller asked for.
    let report = answered(run(
        &mut app,
        "place_wire",
        &["start=16.51,10.16", "end=34.29,10.16", "allow_open=true"],
    ));
    assert_eq!(report["dry_run"], false);
}

#[test]
fn a_label_on_a_referenced_pin_under_a_passing_wire_is_refused_with_no_escape() {
    // An unnamed wire that carries C1.1 runs vertically through R1.2's tip with
    // no end or junction there: R1.2 is on no net, a label there would join it.
    let mut sheet = two_parts();
    for (a, b) in [
        ((16.51, 5.08), (16.51, 20.32)),
        ((16.51, 20.32), (34.29, 20.32)),
        ((34.29, 20.32), (34.29, 12.7)),
    ] {
        sheet
            .wires
            .push(wire(Point::new(a.0, a.1), Point::new(b.0, b.1)));
    }
    let mut app = app_with(sheet);
    for operands in [
        vec!["at=R1.2", "text=SIG"],
        vec!["at=R1.2", "text=SIG", "allow_merge=true"],
    ] {
        refused_with(
            &mut app,
            "place_net_label",
            &operands,
            "label on R1.2 also touches pin C1.1",
        );
    }
    // A junction by pin reference joins the same way; by coordinate it is the
    // caller's own.
    refused_with(
        &mut app,
        "place_junction",
        &["at=R1.2"],
        "junction on R1.2 also touches pin C1.1",
    );
    // By coordinate the label is the caller's own T onto the wire.
    let outcome = run(&mut app, "place_net_label", &["at=16.51,12.7", "text=SIG"]);
    assert_eq!(outcome, CommandOutcome::Ran);
}

#[test]
fn a_power_flag_on_a_named_net_is_no_short() {
    // A flag marks the net as driven and never names it.
    let mut app = app_with(two_named_nets());
    let outcome = run(
        &mut app,
        "place_power_symbol",
        &["at=R1.2", "net_name=PWR_FLAG", "lib_id=power:PWR_FLAG"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
}

#[test]
fn a_wire_to_a_pin_that_shares_its_tip_with_a_connected_power_port_is_accepted() {
    use signex_types::schematic::Symbol;
    // A stub wire on R1.2 and a GND port on the same tip: they are one net.
    let mut sheet = two_parts();
    sheet
        .wires
        .push(wire(Point::new(16.51, 12.7), Point::new(16.51, 7.62)));
    sheet.symbols.push(Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "power:GND".into(),
        reference: "#PWR01".into(),
        value: "GND".into(),
        is_power: true,
        position: Point::new(16.51, 12.7),
        unit: 1,
        ..Symbol::default()
    });
    let mut by_ref = app_with(sheet.clone());
    let mut by_point = app_with(sheet);
    let report = answered(run(&mut by_ref, "place_wire", &["start=R1.2", "end=C1.1"]));
    assert_eq!(report["dry_run"], false);
    answered(run(
        &mut by_point,
        "place_wire",
        &["start=16.51,12.7", "end=C1.1"],
    ));
}

#[test]
fn a_name_the_net_already_carries_is_no_short() {
    // VCC (net label) and X on one net.
    let mut sheet = two_named_nets();
    sheet.labels.push(label("X", Point::new(16.51, 10.16)));
    for operands in [vec!["at=R1.2", "text=X"], vec!["at=R1.2", "text=VCC"]] {
        let mut app = app_with(sheet.clone());
        assert_eq!(
            run(&mut app, "place_net_label", &operands),
            CommandOutcome::Ran,
            "{operands:?}"
        );
    }
    let mut app = app_with(sheet.clone());
    let outcome = run(
        &mut app,
        "place_power_symbol",
        &["at=R1.2", "net_name=X", "lib_id=power:X"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    // Another name is still a short, and the refusal lists what the net has.
    let mut app = app_with(sheet);
    refused_with(
        &mut app,
        "place_net_label",
        &["at=R1.2", "text=GND"],
        "would join the nets {VCC, X} and {GND}",
    );
}

#[test]
fn a_power_port_on_a_pin_that_only_has_a_no_connect_flag_is_refused() {
    let mut sheet = two_parts();
    sheet.no_connects.push(signex_types::schematic::NoConnect {
        uuid: Uuid::now_v7(),
        position: Point::new(16.51, 12.7),
    });
    let mut app = app_with(sheet);
    refused_with(
        &mut app,
        "place_power_symbol",
        &["at=R1.2", "net_name=GND", "lib_id=power:GND"],
        "would touch the pin without connecting",
    );
}

/// The active sheet `a` (R1 and C1 elsewhere) and a second loaded sheet `b`
/// (`two_parts`), with the document operand naming `b`.
fn a_active_b_loaded() -> (Signex, std::path::PathBuf, String) {
    let mut a_sheet = two_parts();
    for sym in &mut a_sheet.symbols {
        sym.position.y += 50.8;
    }
    let mut app = app_with(a_sheet);
    let b = std::env::temp_dir().join("snx-2199-b.snxsch");
    app.document_state.engines.insert(
        b.clone(),
        signex_engine::Engine::new(two_parts()).expect("engine"),
    );
    let operand = format!("document={}", b.display());
    (app, b, operand)
}

fn wires_of(app: &Signex, path: &std::path::Path) -> usize {
    app.document_state.engines[path].document().wires.len()
}

#[test]
fn a_placement_with_a_document_runs_on_that_sheet_and_never_on_the_active_tab() {
    let (mut app, b, document) = a_active_b_loaded();
    let active = app.document_state.active_path.clone().expect("active");
    let tabs = app.document_state.tabs.len();

    // The pin references resolve on `b`; the checks and the commit go to `b`.
    let report = answered(run(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=C1.1", &document],
    ));
    assert_eq!(report["ends"][0]["ref"], "R1.2");
    assert_eq!(wires_of(&app, &b), 1);
    let a_sheet = app.document_state.engines[&active].document();
    assert!(
        a_sheet.wires.is_empty() && a_sheet.junctions.is_empty() && a_sheet.labels.is_empty(),
        "the active sheet is untouched"
    );
    assert!(app.document_state.is_document_dirty(&b));
    assert!(!app.document_state.is_document_dirty(&active));
    // No tab or focus change, and the target is put back.
    assert_eq!(app.document_state.active_path.as_ref(), Some(&active));
    assert_eq!(app.document_state.tabs.len(), tabs);
    assert!(app.document_state.agent_edits.target.is_none());
    // One undo on `b` takes it back.
    app.document_state
        .engines
        .get_mut(&b)
        .unwrap()
        .undo()
        .expect("an undo")
        .expect("an undo step");
    assert_eq!(wires_of(&app, &b), 0);
}

#[test]
fn a_placement_naming_a_document_it_cannot_use_is_refused_and_the_target_restored() {
    let (mut app, b, document) = a_active_b_loaded();
    let unknown = format!(
        "document={}",
        std::env::temp_dir().join("snx-2199-none.snxsch").display()
    );
    for (operands, fragment) in [
        (vec!["at=R1.2", "text=X", &unknown[..]], ""),
        (
            vec!["start=R1.2", "end=R9.1", &document[..]],
            "no part `R9`",
        ),
    ] {
        let id = if operands[0].starts_with("at") {
            "place_net_label"
        } else {
            "place_wire"
        };
        let reason = refusal(run(&mut app, id, &operands));
        assert!(reason.contains(fragment), "{reason}");
        assert!(app.document_state.agent_edits.target.is_none());
    }
    // A sheet from a newer Signex is read-only.
    let mut newer = two_parts();
    newer.unknown.newer_minor_source = Some("snxsch/9.9".into());
    app.document_state.engines.insert(
        b.clone(),
        signex_engine::Engine::new(newer).expect("engine"),
    );
    let reason = refusal(run(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=C1.1", &document],
    ));
    assert!(reason.contains("read-only"), "{reason}");
    assert!(app.document_state.agent_edits.target.is_none());
    assert_eq!(wires_of(&app, &b), 0);
}

/// Two stubs on R1.2 and C1.1, each with the hierarchical label DATA (the best
/// name of both) and a net label of its own: `p` and `q`.
fn stubs_with_data(p: &str, q: &str) -> SchematicSheet {
    let mut sheet = two_parts();
    for (x, own) in [(16.51, p), (34.29, q)] {
        sheet
            .wires
            .push(wire(Point::new(x, 12.7), Point::new(x, 7.62)));
        sheet.labels.push(signex_types::schematic::Label::new(
            "DATA",
            Point::new(x, 7.62),
            LabelType::Hierarchical,
        ));
        sheet.labels.push(label(own, Point::new(x, 10.16)));
    }
    sheet
}

#[test]
fn a_wire_between_nets_with_different_name_sets_is_a_short_even_with_a_shared_name() {
    let mut app = app_with(stubs_with_data("P", "Q"));
    refused_with(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=C1.1"],
        "would join the nets `DATA`, `P` and `DATA`, `Q`",
    );
    let report = answered(run(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=C1.1", "allow_merge=true"],
    ));
    assert_eq!(report["merged"], json!(["DATA", "P", "Q"]));
    // The same names on both sides are no short.
    let mut same = app_with(stubs_with_data("P", "P"));
    answered(run(&mut same, "place_wire", &["start=R1.2", "end=C1.1"]));
}

/// A VCC wire crossed by a GND wire with no junction at the crossing.
fn crossing_nets(gnd_named: bool) -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet
        .wires
        .push(wire(Point::new(0.0, 25.4), Point::new(25.4, 25.4)));
    sheet
        .wires
        .push(wire(Point::new(12.7, 19.05), Point::new(12.7, 31.75)));
    sheet.labels.push(label("VCC", Point::new(0.0, 25.4)));
    if gnd_named {
        sheet.labels.push(label("GND", Point::new(12.7, 19.05)));
    }
    sheet
}

#[test]
fn a_junction_that_joins_two_named_nets_is_a_short_unless_allowed() {
    let mut app = app_with(crossing_nets(true));
    refused_with(
        &mut app,
        "place_junction",
        &["at=12.7,25.4"],
        "a junction at 12.7,25.4 would join the nets {VCC} and {GND}",
    );
    let outcome = run(
        &mut app,
        "place_junction",
        &["at=12.7,25.4", "allow_merge=true"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    // One named net and one unnamed: what a junction is for.
    let mut app = app_with(crossing_nets(false));
    let outcome = run(&mut app, "place_junction", &["at=12.7,25.4"]);
    assert_eq!(outcome, CommandOutcome::Ran);
}

#[test]
fn a_no_connect_on_a_pin_butted_on_another_pin_is_refused() {
    // C2.1's tip sits on R1.2's: the flag would make the pair one net.
    let mut sheet = two_parts();
    sheet.symbols.push(part("C2", Point::new(20.32, 12.7)));
    let mut app = app_with(sheet);
    refused_with(
        &mut app,
        "place_no_connect",
        &["at=R1.2"],
        "no-connect on R1.2 also touches pin C2.1",
    );
}

/// Bus D[0..7] on y = 38.1 from x 12.7 to 25.4, and A[0..15] from 50.8 to 63.5.
fn two_named_buses(second_named: bool) -> SchematicSheet {
    let mut sheet = two_parts();
    for (a, b) in [(12.7, 25.4), (50.8, 63.5)] {
        sheet.buses.push(Bus {
            uuid: Uuid::now_v7(),
            start: Point::new(a, 38.1),
            end: Point::new(b, 38.1),
        });
    }
    sheet.labels.push(label("D[0..7]", Point::new(12.7, 38.1)));
    if second_named {
        sheet.labels.push(label("A[0..15]", Point::new(63.5, 38.1)));
    }
    sheet
}

#[test]
fn a_bus_between_two_differently_named_buses_is_a_merge_unless_allowed() {
    let ends = ["start=25.4,38.1", "end=50.8,38.1"];
    let mut app = app_with(two_named_buses(true));
    refused_with(
        &mut app,
        "place_bus",
        &ends,
        "would join the buses `D[0..7]` and `A[0..15]`",
    );
    let report = answered(run(
        &mut app,
        "place_bus",
        &[ends[0], ends[1], "allow_merge=true"],
    ));
    assert_eq!(report["merged"], json!(["D[0..7]", "A[0..15]"]));
    // One name and one unnamed run: allowed, and the report names the result.
    let mut app = app_with(two_named_buses(false));
    let report = answered(run(&mut app, "draw_bus", &ends));
    assert_eq!(report["bus"], json!(["D[0..7]"]));
    assert_eq!(report["merged"], json!([]));
    // A coordinate end carries its point only.
    assert_eq!(report["ends"][0], json!({"at": [25.4, 38.1]}));
}

#[test]
fn a_bus_that_touches_another_run_an_entry_or_a_bend_target_is_refused() {
    // Mid-path: a stub run ends on the path.
    let mut sheet = two_named_buses(false);
    sheet.buses.push(Bus {
        uuid: Uuid::now_v7(),
        start: Point::new(38.1, 38.1),
        end: Point::new(38.1, 44.45),
    });
    let mut app = app_with(sheet);
    refused_with(
        &mut app,
        "place_bus",
        &["start=25.4,38.1", "end=50.8,38.1"],
        "the bus also touches an unnamed bus at 38.1,38.1",
    );
    // A bus entry on the path.
    let mut sheet = two_named_buses(false);
    sheet.bus_entries.push(BusEntry {
        uuid: Uuid::now_v7(),
        position: Point::new(38.1, 38.1),
        size: (2.54, 2.54),
    });
    let mut app = app_with(sheet);
    refused_with(
        &mut app,
        "place_bus",
        &["start=25.4,38.1", "end=50.8,38.1"],
        "a bus entry at 38.1,38.1",
    );
    // A bend that lands on another run: the bend of (25.4,38.1)..(50.8,50.8)
    // is (50.8,38.1), the end of the second bus, and the end sits on a third.
    let mut sheet = two_named_buses(false);
    sheet.buses.push(Bus {
        uuid: Uuid::now_v7(),
        start: Point::new(44.45, 50.8),
        end: Point::new(57.15, 50.8),
    });
    sheet.buses.push(Bus {
        uuid: Uuid::now_v7(),
        start: Point::new(38.1, 44.45),
        end: Point::new(38.1, 50.8),
    });
    let mut app = app_with(sheet);
    let outcome = run(
        &mut app,
        "place_bus",
        &["start=25.4,38.1", "end=50.8,50.8", "allow_open=true"],
    );
    let reason = refusal(outcome);
    assert!(reason.contains("the bus also touches"), "{reason}");
}

#[test]
fn a_junction_or_label_that_ties_a_loose_port_to_a_net_is_refused_unless_allowed() {
    // A VCC wire with a GND port on its interior (no junction ties it yet).
    let mut sheet = two_parts();
    sheet
        .wires
        .push(wire(Point::new(16.51, 12.7), Point::new(34.29, 12.7)));
    sheet.labels.push(label("VCC", Point::new(16.51, 12.7)));
    sheet.symbols.push(Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "power:GND".into(),
        reference: "#PWR01".into(),
        value: "GND".into(),
        is_power: true,
        position: Point::new(25.4, 12.7),
        unit: 1,
        ..Symbol::default()
    });
    for (id, operands) in [
        ("place_junction", vec!["at=25.4,12.7"]),
        ("place_net_label", vec!["at=25.4,12.7", "text=VCC"]),
    ] {
        let mut app = app_with(sheet.clone());
        refused_with(&mut app, id, &operands, "pass allow_merge=true");
        let mut allowed = operands.clone();
        allowed.push("allow_merge=true");
        assert_eq!(run(&mut app, id, &allowed), CommandOutcome::Ran, "{id}");
    }
}

#[test]
fn a_sheet_pin_on_a_referenced_tip_is_refused_for_every_pin_reference_placement() {
    use signex_types::schematic::{ChildSheet, FillType, SheetPin};
    let mut sheet = two_parts();
    sheet.child_sheets.push(ChildSheet {
        uuid: Uuid::now_v7(),
        name: "Sub".into(),
        filename: "sub.snxsch".into(),
        position: Point::new(16.51, 40.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: Uuid::now_v7(),
            name: "SDA".into(),
            direction: "input".into(),
            position: Point::new(16.51, 12.7),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    });
    for (id, operands) in [
        ("place_wire", vec!["start=R1.2", "end=C1.1"]),
        ("place_net_label", vec!["at=R1.2", "text=X"]),
        ("place_no_connect", vec!["at=R1.2"]),
        ("place_junction", vec!["at=R1.2"]),
    ] {
        let mut app = app_with(sheet.clone());
        refused_with(&mut app, id, &operands, "sheet pin SDA of Sub");
    }
}

#[test]
fn a_label_that_gives_a_bus_run_another_runs_name_is_refused_unless_allowed() {
    let mut app = app_with(two_named_buses(true));
    refused_with(
        &mut app,
        "place_net_label",
        &["at=57.15,38.1", "text=D[0..7]"],
        "would add D[0..7] to the bus {A[0..15]}",
    );
    let outcome = run(
        &mut app,
        "place_net_label",
        &["at=57.15,38.1", "text=D[0..7]", "allow_merge=true"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    // Its own name is no change.
    let mut app = app_with(two_named_buses(true));
    let outcome = run(
        &mut app,
        "place_net_label",
        &["at=57.15,38.1", "text=A[0..15]"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
}

#[test]
fn a_wire_may_start_on_either_end_of_a_bus_entry() {
    let mut sheet = two_parts();
    sheet.bus_entries.push(BusEntry {
        uuid: Uuid::now_v7(),
        position: Point::new(5.08, 25.4),
        size: (2.54, -2.54),
    });
    let mut app = app_with(sheet);
    // The entry's wire-side end is (7.62, 22.86).
    let report = answered(run(
        &mut app,
        "place_wire",
        &["start=7.62,22.86", "end=R1.1"],
    ));
    assert_eq!(report["dry_run"], false);
}

#[test]
fn a_power_port_on_a_referenced_pin_that_connects_to_nothing_is_refused() {
    let mut app = app_with(two_parts());
    refused_with(
        &mut app,
        "place_power_symbol",
        &["at=R1.2", "net_name=GND", "lib_id=power:GND"],
        "would touch the pin without connecting",
    );
    // A coordinate is unchanged.
    let outcome = run(
        &mut app,
        "place_power_symbol",
        &["at=16.51,12.7", "net_name=GND", "lib_id=power:GND"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
}

#[test]
fn a_pin_reference_that_cannot_be_resolved_is_refused_and_changes_nothing() {
    let mut app = app_with(two_parts());
    for (start, fragment) in [
        ("R9.1", "no part `R9`"),
        ("R1.7", "no pin `7`"),
        ("R?.1", "not annotated"),
        // `R 1.2` is the designator `R 1`, not R1 and not a number.
        ("R 1.2", "no part `R 1`"),
    ] {
        let start = format!("start={start}");
        refused_with(&mut app, "place_wire", &[&start, "end=C1.1"], fragment);
    }
    refused_with(&mut app, "place_wire", &["start=R1.2", "end=C9.1"], "`end`");
}

#[test]
fn a_run_with_the_same_pin_at_both_ends_is_refused() {
    let mut app = app_with(two_parts());
    refused_with(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=R1.2"],
        "same point",
    );
}

#[test]
fn a_path_that_touches_another_pin_is_refused_with_the_item_and_its_place() {
    // R5 stands upright above the run with its pin 1 tip on it, at (25.4, 12.7).
    let mut sheet = two_parts();
    let mut r5 = part("R5", Point::new(25.4, 8.89));
    r5.rotation = 90.0;
    sheet.symbols.push(r5);
    // A pin the run only passes over joins nothing: the netlist needs a wire
    // end or a junction at its tip, so the run is as clean as it was drawn.
    let mut passing = app_with(sheet.clone());
    let report = answered(run(&mut passing, "place_wire", &["start=R1.2", "end=C1.1"]));
    assert_eq!(report["net"]["pins"], json!(["C1.1", "R1.2"]));
    // With a stub ending on the tip the engine joins it with a junction.
    sheet
        .wires
        .push(wire(Point::new(25.4, 12.7), Point::new(25.4, 11.43)));
    let mut app = app_with(sheet);
    let reason = refusal(run(&mut app, "place_wire", &["start=R1.2", "end=C1.1"]));
    assert!(
        reason.contains("the wire also touches pin R5.1 at 25.4,"),
        "{reason}"
    );
}

#[test]
fn a_wire_end_touched_on_the_path_is_named() {
    let mut sheet = two_parts();
    sheet.wires.push(wire(grid(18.0, 10.0), grid(18.0, 8.0)));
    let mut app = app_with(sheet);
    let reason = refusal(run(&mut app, "place_wire", &["start=R1.2", "end=C1.1"]));
    assert!(reason.contains("another wire at 22.86,"), "{reason}");
}

#[test]
fn a_bend_that_lands_on_a_pin_is_refused() {
    // The bend of (R1.2 -> (34.29, 25.4)) is at (34.29, 12.7): C1 pin 1.
    let mut app = app_with(two_parts());
    refused_with(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=34.29,25.4", "allow_open=true"],
        "pin C1.1 at 34.29,",
    );
}

#[test]
fn a_label_touched_on_the_path_is_refused() {
    let mut sheet = two_parts();
    sheet.labels.push(label("SENSE", grid(20.0, 10.0)));
    let mut app = app_with(sheet);
    refused_with(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=C1.1"],
        "label `SENSE`",
    );
}

/// R1.2 and C1.1 on stubs that carry the labels VCC and GND.
fn two_named_nets() -> SchematicSheet {
    let mut sheet = two_parts();
    sheet
        .wires
        .push(wire(Point::new(16.51, 12.7), Point::new(16.51, 7.62)));
    sheet
        .wires
        .push(wire(Point::new(34.29, 12.7), Point::new(34.29, 7.62)));
    sheet.labels.push(label("VCC", Point::new(16.51, 7.62)));
    sheet.labels.push(label("GND", Point::new(34.29, 7.62)));
    sheet
}

#[test]
fn a_run_between_two_named_nets_is_a_short_unless_allowed() {
    let mut app = app_with(two_named_nets());
    refused_with(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=C1.1"],
        "`VCC` and `GND`",
    );
    let before = sheet_shape(&app);
    let report = answered(run(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=C1.1", "allow_merge=true"],
    ));
    assert_ne!(sheet_shape(&app), before);
    assert_eq!(report["merged"], json!(["VCC", "GND"]));
}

#[test]
fn an_end_with_nothing_under_it_is_refused_unless_allowed() {
    let mut app = app_with(two_parts());
    refused_with(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=25.4,12.7"],
        "`end` 25.4,12.7 has nothing to connect to",
    );
    refused_with(
        &mut app,
        "place_wire",
        &["start=25.4,25.4", "end=30.48,25.4"],
        "`start` 25.4,25.4 and `end` 30.48,25.4 have nothing",
    );
    let report = answered(run(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=25.4,12.7", "allow_open=true"],
    ));
    assert_eq!(report["net"]["pins"], json!(["R1.2"]));
}

#[test]
fn a_start_on_a_wire_interior_is_intended_and_joins_that_net() {
    let mut sheet = two_parts();
    sheet
        .wires
        .push(wire(Point::new(16.51, 12.7), Point::new(34.29, 12.7)));
    sheet.symbols.push(part("R4", grid(20.0, 20.0)));
    let mut app = app_with(sheet);
    // R4.2 tip (29.21, 25.4), straight up to the wire's interior.
    let report = answered(run(
        &mut app,
        "place_wire",
        &["start=R4.2", "end=29.21,12.7"],
    ));
    assert_eq!(report["net"]["pins"], json!(["C1.1", "R1.2", "R4.2"]));
}

#[test]
fn only_horizontal_and_vertical_segments_are_placed() {
    // A 45 degree diagonal for Angle45, any slope for FreeAngle.
    for (mode, end) in [
        (DrawMode::Angle45, "end=21.59,17.78"),
        (DrawMode::FreeAngle, "end=31.75,17.78"),
    ] {
        let mut app = app_with(two_parts());
        app.interaction_state.draw_mode = mode;
        refused_with(
            &mut app,
            "place_wire",
            &["start=R1.2", end, "allow_open=true"],
            "only horizontal and vertical",
        );
    }
    // Ortho90 bends the same points into two orthogonal segments.
    let mut app = app_with(two_parts());
    let report = answered(run(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=31.75,17.78", "allow_open=true"],
    ));
    assert_eq!(report["segments"].as_array().map(Vec::len), Some(2));
}

#[test]
fn a_run_through_a_symbol_body_is_refused_even_along_its_own_pins() {
    let mut app = app_with(two_parts());
    refused_with(
        &mut app,
        "place_wire",
        &["start=R1.1", "end=R1.2"],
        "crosses the body of R1",
    );
}

#[test]
fn a_run_along_a_body_edge_does_not_cross_it() {
    let mut app = app_with(two_parts());
    // R1's body spans x 11.43..13.97, y 12.192..13.208: the run is on its
    // left edge, past the pin row.
    let report = answered(run(
        &mut app,
        "place_wire",
        &["start=11.43,7.62", "end=11.43,17.78", "allow_open=true"],
    ));
    assert_eq!(report["segments"].as_array().map(Vec::len), Some(1));
}

fn bytes_and_state(app: &Signex) -> (Vec<u8>, u64, bool, Option<u64>) {
    let path = app
        .document_state
        .edit_path()
        .expect("a sheet")
        .to_path_buf();
    (
        serde_json::to_vec(&document(app)).unwrap(),
        app.document_state.engines[&path].revision(),
        app.document_state.is_document_dirty(&path),
        crate::diagnostics::newest_entry_id(),
    )
}

#[test]
fn a_dry_run_reports_and_leaves_the_document_untouched() {
    for (id, operands) in [
        ("place_wire", vec!["start=R1.2", "end=C1.1", "dry_run=true"]),
        (
            "place_bus",
            vec![
                "start=12.7,25.4",
                "end=25.4,25.4",
                "dry_run=true",
                "allow_open=true",
            ],
        ),
    ] {
        let mut app = app_with(two_parts());
        let before = bytes_and_state(&app);
        let report = answered(run(&mut app, id, &operands));
        assert_eq!(report["dry_run"], true, "{id}");
        assert!(report["segments"].is_array(), "{id}");
        assert_eq!(bytes_and_state(&app), before, "{id}");
        // Nothing to undo: the document is as it was after an undo as well.
        let _ = app.dispatch_command_reporting(
            &AppCommandId::new("undo").unwrap(),
            CommandArgs::none(),
            None,
        );
        assert_eq!(bytes_and_state(&app).0, before.0, "{id}");
    }
}

#[test]
fn a_wire_that_lands_is_one_undo_step_and_marks_the_sheet_dirty() {
    let mut app = app_with(two_parts());
    let before = bytes_and_state(&app);
    let _ = answered(run(&mut app, "place_wire", &["start=R1.2", "end=C1.1"]));
    let after = bytes_and_state(&app);
    assert_ne!(after.0, before.0);
    assert!(after.2, "dirty");
    let _ = app.dispatch_command_reporting(
        &AppCommandId::new("undo").unwrap(),
        CommandArgs::none(),
        None,
    );
    assert_eq!(bytes_and_state(&app).0, before.0);
}

#[test]
fn a_dry_run_refuses_what_the_run_would() {
    let mut app = app_with(two_named_nets());
    refused_with(
        &mut app,
        "place_wire",
        &["start=R1.2", "end=C1.1", "dry_run=true"],
        "`VCC` and `GND`",
    );
}

/// A bus on y = 25.4 from x = 12.7 to 25.4, with a label on its right end.
fn with_bus() -> SchematicSheet {
    let mut sheet = two_parts();
    sheet.buses.push(Bus {
        uuid: Uuid::now_v7(),
        start: Point::new(12.7, 25.4),
        end: Point::new(25.4, 25.4),
    });
    sheet.bus_entries.push(BusEntry {
        uuid: Uuid::now_v7(),
        position: Point::new(12.7, 38.1),
        size: (2.54, 2.54),
    });
    sheet.labels.push(label("D[0..7]", Point::new(50.8, 25.4)));
    sheet
}

#[test]
fn a_bus_connects_to_a_bus_an_entry_or_a_label_or_is_refused_open() {
    let mut app = app_with(with_bus());
    // A bus's interior to a label, and an entry to a bus's end: nothing is open.
    for (id, start, end) in [
        ("place_bus", "19.05,25.4", "50.8,25.4"),
        ("draw_bus", "12.7,38.1", "12.7,25.4"),
    ] {
        let (start, end) = (format!("start={start}"), format!("end={end}"));
        let report = answered(run(&mut app, id, &[&start, &end]));
        assert!(report["segments"].is_array(), "{id}");
        assert!(report.get("net").is_none(), "a bus has no net diff");
    }
    refused_with(
        &mut app,
        "place_bus",
        &["start=63.5,25.4", "end=63.5,38.1"],
        "have nothing to connect to (a bus, bus entry, label or sheet entry)",
    );
    let outcome = run(
        &mut app,
        "place_bus",
        &["start=63.5,25.4", "end=63.5,38.1", "allow_open=true"],
    );
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
}

#[test]
fn a_bus_run_ending_on_a_sheet_entry_is_not_open() {
    // 6761836527 point 7: a sheet entry is something a bus connects to.
    let mut sheet = two_parts();
    sheet.buses.push(Bus {
        uuid: Uuid::now_v7(),
        start: Point::new(12.7, 50.8),
        end: Point::new(25.4, 50.8),
    });
    sheet
        .child_sheets
        .push(signex_types::schematic::ChildSheet {
            uuid: Uuid::now_v7(),
            name: "Mem".into(),
            filename: "mem.snxsch".into(),
            position: Point::new(63.5, 45.72),
            size: (10.16, 10.16),
            stroke_width: 0.0,
            fill: signex_types::schematic::FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: vec![signex_types::schematic::SheetPin {
                uuid: Uuid::now_v7(),
                name: "D[0..7]".into(),
                direction: "input".into(),
                position: Point::new(63.5, 50.8),
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            }],
            instances: Vec::new(),
        });
    let mut app = app_with(sheet);
    let outcome = run(&mut app, "place_bus", &["start=25.4,50.8", "end=63.5,50.8"]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
}

#[test]
fn a_hierarchical_bus_label_on_a_named_run_is_an_alias_not_a_rename() {
    // 6761836527 point 7: a hierarchical bus label is an alias, so it renames
    // nothing; a second own name on the run still does.
    let mut sheet = two_parts();
    sheet.buses.push(Bus {
        uuid: Uuid::now_v7(),
        start: Point::new(12.7, 50.8),
        end: Point::new(25.4, 50.8),
    });
    sheet
        .labels
        .push(label("DATA[0..7]", Point::new(12.7, 50.8)));
    let mut app = app_with(sheet.clone());
    let outcome = run(
        &mut app,
        "place_hierarchical_label",
        &["at=25.4,50.8", "text=D[0..7]"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    let mut app = app_with(sheet);
    refused_with(
        &mut app,
        "place_net_label",
        &["at=25.4,50.8", "text=A[0..15]"],
        "allow_merge",
    );
}

#[test]
fn a_bus_run_joins_runs_by_the_name_rule() {
    // 6761836527 point 7, by points 2-4: a run between a hierarchically
    // labelled run (an alias) and a locally labelled run (an own name) joins
    // freely; between two different own names it is a merge.
    for (left, ty, merge) in [
        ("D[0..7]", LabelType::Hierarchical, false),
        ("A[0..15]", LabelType::Net, true),
    ] {
        let mut sheet = two_parts();
        for (a, b) in [(0.0, 12.7), (25.4, 38.1)] {
            sheet.buses.push(Bus {
                uuid: Uuid::now_v7(),
                start: Point::new(a, 50.8),
                end: Point::new(b, 50.8),
            });
        }
        let named = signex_types::schematic::Label::new(left, Point::new(0.0, 50.8), ty);
        sheet.labels.push(named);
        sheet
            .labels
            .push(label("DATA[0..7]", Point::new(38.1, 50.8)));
        let mut app = app_with(sheet);
        let ends = ["start=12.7,50.8", "end=25.4,50.8"];
        if merge {
            refused_with(&mut app, "place_bus", &ends, "allow_merge");
        } else {
            answered(run(&mut app, "place_bus", &ends));
        }
    }
}

#[test]
fn a_bus_takes_no_pin_reference_and_keeps_the_geometry_rules() {
    let mut app = app_with(with_bus());
    refused_with(
        &mut app,
        "place_bus",
        &["start=R1.2", "end=25.4,25.4"],
        "a bus does not connect to pins",
    );
    refused_with(
        &mut app,
        "draw_bus",
        &["start=12.7,25.4", "end=C1.1", "allow_open=true"],
        "a bus does not connect to pins",
    );
    app.interaction_state.draw_mode = DrawMode::FreeAngle;
    refused_with(
        &mut app,
        "place_bus",
        &["start=12.7,25.4", "end=25.4,38.1", "allow_open=true"],
        "only horizontal and vertical",
    );
    app.interaction_state.draw_mode = DrawMode::Ortho90;
    refused_with(
        &mut app,
        "place_bus",
        &["start=5.08,12.7", "end=20.32,12.7", "allow_open=true"],
        "crosses the body of R1",
    );
}
