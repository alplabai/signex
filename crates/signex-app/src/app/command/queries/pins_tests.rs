//! #2199 — `query_pins` lists a part's pins with the tips the canvas hit-test
//! uses, per placed unit, and refuses what it cannot name.

use serde_json::Value;
use signex_types::schematic::{
    BodyStyle, InstancePath, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point,
    SchematicSheet, Symbol, SymbolInstance, snap_to_hotspot,
};
use uuid::Uuid;

use super::tests::{answer, app_with, path};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::editing_commands::net_color_driver as fixture;
use crate::keymap::AppCommandId;

fn lib_pin(number: &str, unit: u32, x: f64, visible: bool) -> LibPin {
    LibPin {
        unit,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: format!("N{number}"),
            number: number.into(),
            visible,
            name_visible: true,
            number_visible: true,
        },
    }
}

fn in_body(mut pin: LibPin, body_style: u32) -> LibPin {
    pin.body_style = body_style;
    pin
}

fn lib(id: &str, pins: Vec<LibPin>) -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": id })).unwrap();
    lib.pins = pins;
    lib
}

fn instance(path: &InstancePath, reference: &str) -> SymbolInstance {
    SymbolInstance {
        project: "p".into(),
        path: path.to_string(),
        reference: reference.into(),
        unit: 1,
    }
}

fn part(reference: &str, lib_id: &str, unit: u32, at: (f64, f64)) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: lib_id.into(),
        reference: reference.into(),
        position: Point::new(at.0, at.1),
        unit,
        ..Symbol::default()
    }
}

/// R1 turned 90 degrees, R2 mirrored in Y, a three-unit U1 with units 1 and 3
/// placed (pin 8 is hidden and shared by every unit), a second part also
/// called U2, an unannotated R?, U4 with a shared pin and a pin per body, a
/// lower-case u1 beside U1, R5 renamed R7 at the root by an instance entry,
/// and Q1 whose pin has a name and no number.
pub(in crate::app) fn pin_sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    let mut unnumbered = lib_pin("", 0, 0.0, true);
    unnumbered.pin.name = "CLK".into();
    for (id, pins) in [
        ("Q", vec![unnumbered]),
        (
            "DM",
            vec![
                in_body(lib_pin("A", 0, 1.0, true), 0),
                in_body(lib_pin("N", 0, 2.0, true), 1),
                in_body(lib_pin("D", 0, 3.0, true), 2),
                in_body(lib_pin("Y", 2, 4.0, true), 0),
            ],
        ),
        (
            "R",
            vec![lib_pin("1", 0, -2.54, true), lib_pin("2", 0, 2.54, true)],
        ),
        (
            "OPAMP",
            vec![
                lib_pin("1", 1, 3.0, true),
                lib_pin("2", 2, 3.0, true),
                lib_pin("3", 3, 3.0, true),
                lib_pin("8", 0, 0.0, false),
            ],
        ),
    ] {
        sheet.lib_symbols.insert(id.into(), lib(id, pins));
    }
    let mut r1 = part("R1", "R", 1, (50.0, 50.0));
    r1.rotation = 90.0;
    let mut r2 = part("R2", "R", 1, (10.0, 10.0));
    r2.mirror_y = true;
    let mut r5 = part("R5", "R", 1, (60.0, 60.0));
    r5.instances = vec![instance(&InstancePath::root(), "R7")];
    let mut u4_alternate = part("U4", "DM", 2, (210.0, 0.0));
    u4_alternate.body_style = BodyStyle::Alternate;
    let mut port = part("#PWR01", "R", 1, (500.0, 0.0));
    port.is_power = true;
    sheet.symbols = vec![
        port,
        r1,
        r2,
        r5,
        part("u1", "R", 1, (300.0, 0.0)),
        part("U4", "DM", 1, (200.0, 0.0)),
        u4_alternate,
        part("Q1", "Q", 1, (400.0, 0.0)),
        part("U1", "OPAMP", 1, (100.0, 100.0)),
        part("U1", "OPAMP", 3, (120.0, 100.0)),
        part("U2", "R", 1, (0.0, 0.0)),
        part("U2", "OPAMP", 2, (5.0, 5.0)),
        part("U3", "R", 1, (0.0, 40.0)),
        part("U3", "R", 1, (0.0, 45.0)),
        part("R?", "R", 1, (0.0, 20.0)),
    ];
    sheet
}

fn app() -> Signex {
    let mut app = app_with(&[]);
    fixture::add_sheet_with(&mut app, &path("pins.snxsch"), pin_sheet());
    app.document_state.mark_document_clean(&path("pins.snxsch"));
    app
}

fn pins_of(app: &mut Signex, designator: &str) -> Vec<Value> {
    let document = format!("document={}", path("pins.snxsch").display());
    let designator = format!("designator={designator}");
    let answered = answer(app, "query_pins", &[&document, &designator]);
    answered["pins"].as_array().expect("pins").clone()
}

fn refusal(app: &mut Signex, operands: &[&str]) -> String {
    let command = AppCommandId::new("query_pins").unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    match app.run_query(&command, &args).expect("a query").0 {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("was not refused: {other:?}"),
    }
}

fn tip(pin: &Value) -> (f64, f64) {
    let at = pin["position"].as_array().expect("position");
    (at[0].as_f64().unwrap(), at[1].as_f64().unwrap())
}

/// Every answered tip is where a wire snaps on the open sheet.
fn assert_snaps(app: &Signex, pins: &[Value]) {
    let sheet = app.document_state.engines[&path("pins.snxsch")].document();
    for pin in pins {
        let (x, y) = tip(pin);
        let snapped = snap_to_hotspot(sheet, Point::new(x, y), 0.1);
        assert_eq!(snapped, Some(Point::new(x, y)), "{pin}");
    }
}

fn assert_tip(pin: &Value, x: f64, y: f64) {
    let (got_x, got_y) = tip(pin);
    assert!(
        (got_x - x).abs() < 1e-9 && (got_y - y).abs() < 1e-9,
        "expected ({x}, {y}), got ({got_x}, {got_y}) for {pin}"
    );
}

#[test]
fn a_rotated_resistor_lists_its_tips_where_the_canvas_hit_test_finds_them() {
    let mut app = app();
    let pins = pins_of(&mut app, "R1");
    assert_eq!(pins.len(), 2);
    assert_eq!(pins[0]["number"], "1");
    assert_eq!(pins[0]["name"], "N1");
    assert_eq!(pins[0]["electrical_type"], "passive");
    assert_eq!(pins[0]["hidden"], false);
    assert_eq!(pins[0]["unit"], 1);
    assert_eq!(pins[0]["shared"], false, "a one-unit part shares nothing");
    assert_tip(&pins[0], 50.0, 52.54);
    assert_tip(&pins[1], 50.0, 47.46);
    assert_snaps(&app, &pins);
}

#[test]
fn a_mirrored_resistor_lists_mirrored_tips() {
    let mut app = app();
    let pins = pins_of(&mut app, "R2");
    assert_tip(&pins[0], 12.54, 10.0);
    assert_tip(&pins[1], 7.46, 10.0);
    assert_snaps(&app, &pins);
}

/// (number, unit, shared) of each answered pin.
fn listed(pins: &[Value]) -> Vec<(String, u64, bool)> {
    pins.iter()
        .map(|p| {
            (
                p["number"].as_str().unwrap().to_string(),
                p["unit"].as_u64().unwrap(),
                p["shared"].as_bool().unwrap(),
            )
        })
        .collect()
}

fn entry(number: &str, unit: u64, shared: bool) -> (String, u64, bool) {
    (number.into(), unit, shared)
}

#[test]
fn a_multi_unit_part_lists_each_placed_unit_with_its_shared_pin_at_every_one() {
    let mut app = app();
    let pins = pins_of(&mut app, "U1");
    // Unit 2 is not placed: its pin is not listed. Pin 8 is at both tips.
    assert_eq!(
        listed(&pins),
        [
            entry("1", 1, false),
            entry("8", 1, true),
            entry("3", 3, false),
            entry("8", 3, true),
        ]
    );
    assert_eq!(pins[1]["hidden"], true);
    assert_tip(&pins[1], 100.0, 100.0);
    assert_tip(&pins[3], 120.0, 100.0);
    assert_tip(&pins[2], 123.0, 100.0);
    assert_snaps(&app, &pins);
}

#[test]
fn a_shared_pin_is_listed_for_a_unit_on_the_other_body() {
    let mut app = app();
    let pins = pins_of(&mut app, "U4");
    // Unit 1 shows the normal body, unit 2 the alternate one; the pin common
    // to both bodies is at each tip.
    assert_eq!(
        listed(&pins),
        [
            entry("A", 1, true),
            entry("N", 1, true),
            entry("A", 2, true),
            entry("D", 2, true),
            entry("Y", 2, false),
        ]
    );
    assert_snaps(&app, &pins);
}

#[test]
fn a_pin_with_no_number_is_given_by_its_name_as_the_netlist_does() {
    let mut app = app();
    let pins = pins_of(&mut app, "Q1");
    assert_eq!(pins[0]["number"], "CLK");
    assert_eq!(pins[0]["name"], "CLK");
}

#[test]
fn designators_match_exactly_so_u1_and_lower_case_u1_are_two_parts() {
    let mut app = app();
    assert_eq!(pins_of(&mut app, "U1").len(), 4);
    let lower = pins_of(&mut app, "u1");
    assert_eq!(lower.len(), 2, "{lower:?}");
    assert_tip(&lower[0], 297.46, 0.0);
    assert_eq!(listed(&lower)[0].1, 1);
}

#[test]
fn the_designator_is_the_one_an_instance_entry_gives_at_the_root() {
    let mut app = app();
    let pins = pins_of(&mut app, "R7");
    assert_eq!(pins.len(), 2);
    assert_tip(&pins[0], 57.46, 60.0);
    let reason = refusal(
        &mut app,
        &[
            &format!("document={}", path("pins.snxsch").display()),
            "designator=R5",
        ],
    );
    assert!(reason.contains("no part `R5`"), "{reason}");
}

/// `top` places `child` twice; the child's one part is R1 at the first
/// placement and R101 at the second. `child` is not open: the query reads it
/// for the answer, as a headless run does.
#[test]
fn a_project_sheet_placed_twice_answers_each_placements_designator() {
    use crate::app::handlers::menu::export::tests::current_placement::placed_twice_listing;

    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_listing(&dir, &["top.snxsch", "child.snxsch"]);
    let command = AppCommandId::new("query_pins").unwrap();
    let operands = [
        format!("document={}", child.display()),
        "designator=R1".into(),
    ];
    let args = CommandArgs::parse_operands(&command, &[&operands[0], &operands[1]]).unwrap();
    // Before the project's hierarchy is read the designators are unknown:
    // refused, not guessed from the root path.
    match app.run_query_headless(&command, &args).expect("a query").0 {
        CommandOutcome::Disabled { reason } => assert!(reason.contains("hierarchy"), "{reason}"),
        other => panic!("was not refused: {other:?}"),
    }
    crate::app::runtime::sheet_tree_tests::settle(&mut app, true);
    let mut ask = |designator: &str| {
        let operands = [
            format!("document={}", child.display()),
            format!("designator={designator}"),
        ];
        let args = CommandArgs::parse_operands(&command, &[&operands[0], &operands[1]]).unwrap();
        app.run_query_headless(&command, &args).expect("a query").0
    };
    let answered = |outcome| match outcome {
        CommandOutcome::Answered(value) => value["pins"].clone(),
        other => panic!("was not answered: {other:?}"),
    };
    let (first, second) = (answered(ask("R1")), answered(ask("R101")));
    assert_eq!(first.as_array().unwrap().len(), 1, "{first}");
    assert_eq!(first, second, "one sheet, one set of tips");
    match ask("R201") {
        CommandOutcome::Disabled { reason } => assert!(reason.contains("no part"), "{reason}"),
        other => panic!("was not refused: {other:?}"),
    }
}

#[test]
fn a_reused_sheet_answers_each_placements_designator_with_the_same_tips() {
    let (first, second) = (
        InstancePath::from_chain(vec![Uuid::from_u128(1)]),
        InstancePath::from_chain(vec![Uuid::from_u128(2)]),
    );
    let mut sheet = pin_sheet();
    sheet.symbols = vec![Symbol {
        instances: vec![instance(&first, "R101"), instance(&second, "R201")],
        ..part("R1", "R", 1, (5.0, 5.0))
    }];
    let paths = [first, second];
    let one = super::pins::pins_of(&sheet, "R101", &paths).unwrap();
    let two = super::pins::pins_of(&sheet, "R201", &paths).unwrap();
    assert_eq!(one.len(), 2);
    assert_eq!(one, two);
    let bare = super::pins::pins_of(&sheet, "R1", &paths).unwrap_err();
    assert!(bare.contains("no part `R1`"), "{bare}");
}

#[test]
fn what_cannot_be_named_is_refused_with_the_reason() {
    let mut app = app();
    let document = format!("document={}", path("pins.snxsch").display());
    let on = |designator: &str| [document.clone(), format!("designator={designator}")];
    for (designator, fragment) in [
        ("C9", "no part `C9`"),
        ("U2", "ambiguous"),
        ("U3", "ambiguous"),
        ("R?", "not annotated"),
        ("#PWR01", "power port"),
    ] {
        let operands = on(designator);
        let reason = refusal(&mut app, &[&operands[0], &operands[1]]);
        assert!(reason.contains(fragment), "{designator}: {reason}");
    }
    let missing = format!("document={}", path("nope.snxsch").display());
    let reason = refusal(&mut app, &[&missing, "designator=R1"]);
    assert!(reason.contains("not an open or referenced"), "{reason}");
}

fn tip_of(sheet: &SchematicSheet, designator: &str, pin: &str) -> Result<Point, String> {
    super::pins::pin_tip_of(sheet, designator, pin, &[InstancePath::root()])
}

fn assert_point(got: Point, x: f64, y: f64) {
    assert!(
        (got.x - x).abs() < 1e-9 && (got.y - y).abs() < 1e-9,
        "expected ({x}, {y}), got {got:?}"
    );
}

#[test]
fn a_pin_reference_is_the_tip_query_pins_lists_by_number_or_by_name() {
    let sheet = pin_sheet();
    assert_point(tip_of(&sheet, "R1", "2").unwrap(), 50.0, 47.46);
    assert_point(tip_of(&sheet, "R1", "N2").unwrap(), 50.0, 47.46);
    // A pin with no number is its name; a pin on a part's other unit is found
    // from the bare designator.
    assert_eq!(tip_of(&sheet, "Q1", "CLK").map(|p| p.y), Ok(0.0));
    assert_point(tip_of(&sheet, "U1", "3").unwrap(), 123.0, 100.0);
}

#[test]
fn the_pin_number_wins_over_a_pin_name_and_an_ambiguous_name_is_refused() {
    let mut sheet = pin_sheet();
    sheet.lib_symbols.get_mut("R").unwrap().pins[0].pin.name = "2".into();
    // `2` is pin 2's number, though pin 1 is also named `2`.
    assert_point(tip_of(&sheet, "R1", "2").unwrap(), 50.0, 47.46);
    for lp in &mut sheet.lib_symbols.get_mut("R").unwrap().pins {
        lp.pin.name = "X".into();
    }
    let reason = tip_of(&sheet, "R1", "X").unwrap_err();
    assert!(reason.contains("matches several pins (1, 2)"), "{reason}");
}

#[test]
fn a_pin_that_is_not_one_tip_on_this_sheet_is_refused_with_the_reason() {
    let sheet = pin_sheet();
    for (designator, pin, fragment) in [
        ("R1", "9", "has no pin `9`"),
        ("U1", "2", "belongs to unit 2, which is not placed"),
        ("U1", "8", "is hidden"),
        ("U4", "A", "has several tips"),
        ("C9", "1", "no part `C9`"),
        ("R?", "1", "not annotated"),
        ("#PWR01", "1", "power port"),
        ("U2", "1", "ambiguous"),
    ] {
        let reason = tip_of(&sheet, designator, pin).unwrap_err();
        assert!(reason.contains(fragment), "{designator}.{pin}: {reason}");
    }
    // The pin common to the normal body but placed once has one tip.
    assert!(tip_of(&sheet, "U4", "N").is_ok());
}

/// A project whose hierarchy is wanted but not walked yet is being read: the
/// designators of its sheets are not final, so neither query nor reference
/// answers from them.
#[test]
fn a_hierarchy_being_read_refuses_pin_references_and_query_pins_keeps_its_refusal() {
    use crate::app::handlers::menu::export::tests::current_placement::placed_twice_listing;

    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_listing(&dir, &["top.snxsch", "child.snxsch"]);
    let command = AppCommandId::new("query_pins").unwrap();
    let operands = [
        format!("document={}", child.display()),
        "designator=R1".to_string(),
    ];
    let args = CommandArgs::parse_operands(&command, &[&operands[0], &operands[1]]).unwrap();
    match app.run_query_headless(&command, &args).expect("a query").0 {
        // `query_pins` keeps its own refusal: only a reference waits for a re-read.
        CommandOutcome::Disabled { reason } => {
            assert!(reason.contains("not read yet"), "{reason}")
        }
        other => panic!("was not refused: {other:?}"),
    }
    let reason = app.pin_ref_tip("R_TOP", "1").unwrap_err();
    assert!(reason.contains("being re-read"), "{reason}");
    crate::app::runtime::sheet_tree_tests::settle(&mut app, true);
    // Read: the gate is open (whatever the part answers is its own reason).
    let after = app.pin_ref_tip("R_TOP", "1");
    assert!(
        after
            .as_ref()
            .err()
            .is_none_or(|e| !e.contains("hierarchy")),
        "{after:?}"
    );
}
