//! #359 — reconcile never moves an existing sheet pin, so the wires attached
//! to it keep their nets.

use std::collections::HashMap;

use signex_net::{ProjectGraph, ProjectRoot, SheetKey, build_project_netlist};
use signex_types::schematic::{
    ChildSheet, FillType, HAlign, Junction, Label, LabelType, LibPin, LibSymbol, NoConnect, Pin,
    PinDirection, PinShapeStyle, Point, SchematicSheet, SheetPin, Symbol, VAlign, Wire,
};

use super::{SheetPinContacts, reconcile_child_sheet_pins as reconcile_with};
use crate::command::SheetPort;
use crate::test_support::test_sheet;

/// Reconcile with nothing on the parent sheet to avoid.
fn reconcile_child_sheet_pins(child: &mut ChildSheet, ports: &[SheetPort]) -> bool {
    reconcile_with(child, ports, &SheetPinContacts::default())
}

fn port(name: &str, direction: &str) -> SheetPort {
    SheetPort {
        name: name.to_string(),
        direction: direction.to_string(),
    }
}

fn symbol_with(pins: Vec<SheetPin>) -> ChildSheet {
    ChildSheet {
        uuid: uuid::Uuid::now_v7(),
        name: "Child".to_string(),
        filename: "child.sch".to_string(),
        position: Point::new(100.0, 100.0),
        size: (30.0, 30.0),
        stroke_width: 0.12,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins,
        instances: Vec::new(),
    }
}

fn hand_pin(name: &str, at: (f64, f64)) -> SheetPin {
    SheetPin {
        uuid: uuid::Uuid::now_v7(),
        name: name.to_string(),
        direction: "input".to_string(),
        position: Point::new(at.0, at.1),
        rotation: 0.0,
        auto_generated: false,
        user_moved: false,
    }
}

fn pin_at<'a>(symbol: &'a ChildSheet, name: &str) -> &'a SheetPin {
    symbol
        .pins
        .iter()
        .find(|pin| pin.name == name)
        .unwrap_or_else(|| panic!("pin {name} missing"))
}

#[test]
fn adding_a_label_that_sorts_first_leaves_existing_pins_where_they_are() {
    let mut symbol = symbol_with(Vec::new());
    reconcile_child_sheet_pins(&mut symbol, &[port("B", ""), port("C", "")]);
    let b = pin_at(&symbol, "B").clone();
    let c = pin_at(&symbol, "C").clone();

    let changed =
        reconcile_child_sheet_pins(&mut symbol, &[port("A", ""), port("B", ""), port("C", "")]);

    assert!(changed);
    assert_eq!(pin_at(&symbol, "B").position, b.position);
    assert_eq!(pin_at(&symbol, "B").uuid, b.uuid);
    assert_eq!(pin_at(&symbol, "C").position, c.position);
    let a = pin_at(&symbol, "A");
    assert!(a.auto_generated);
    assert!(
        a.position != b.position && a.position != c.position,
        "the new pin takes a free slot"
    );
}

#[test]
fn a_hand_added_pin_matching_a_label_keeps_position_and_flag() {
    let manual = hand_pin("CLK", (100.0, 120.0));
    let mut symbol = symbol_with(vec![manual.clone()]);

    let changed = reconcile_child_sheet_pins(&mut symbol, &[port("CLK", "output")]);

    assert!(!changed);
    let pin = pin_at(&symbol, "CLK");
    assert_eq!(pin.uuid, manual.uuid);
    assert_eq!(pin.position, manual.position);
    assert!(!pin.auto_generated, "stays hand-added");
    assert_eq!(pin.direction, "input", "direction is the pin's own");
}

#[test]
fn removing_a_label_drops_only_its_auto_pin() {
    let mut symbol = symbol_with(Vec::new());
    reconcile_child_sheet_pins(&mut symbol, &[port("A", ""), port("B", "")]);
    symbol.pins.push(hand_pin("MANUAL", (100.0, 120.0)));
    let b = pin_at(&symbol, "B").clone();

    let changed = reconcile_child_sheet_pins(&mut symbol, &[port("B", "")]);

    assert!(changed);
    let mut names: Vec<_> = symbol.pins.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["B", "MANUAL"]);
    assert_eq!(pin_at(&symbol, "B").position, b.position);
}

#[test]
fn a_padded_existing_pin_is_matched_not_duplicated() {
    let padded = hand_pin(" CLK ", (100.0, 120.0));
    let mut symbol = symbol_with(vec![padded.clone()]);

    let changed = reconcile_child_sheet_pins(&mut symbol, &[port("CLK", "")]);

    assert!(!changed);
    assert_eq!(symbol.pins.len(), 1);
    assert_eq!(symbol.pins[0].uuid, padded.uuid);
}

#[test]
fn a_new_pin_skips_slots_taken_by_hand_added_pins() {
    // A hand-added pin sits on the first left-edge slot.
    let taken = hand_pin("MANUAL", (100.0, 102.54));
    let mut symbol = symbol_with(vec![taken.clone()]);

    reconcile_child_sheet_pins(&mut symbol, &[port("NEW", "")]);

    assert_ne!(pin_at(&symbol, "NEW").position, taken.position);
}

// ---- netlist: wires keep their nets across a sync -------------------------

fn lib_resistor() -> LibSymbol {
    LibSymbol {
        id: "R".to_string(),
        reference: String::new(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn place(sheet: &mut SchematicSheet, reference: &str, at: Point) {
    sheet.symbols.push(Symbol {
        lib_id: "R".to_string(),
        reference: reference.to_string(),
        position: at,
        ..Default::default()
    });
}

fn hierarchical_label(text: &str, at: Point) -> Label {
    Label {
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::now_v7(),
        text: text.to_string(),
        position: at,
        rotation: 0.0,
        label_type: LabelType::Hierarchical,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        font_family: None,
    }
}

/// True when the project netlist puts resistors `a` and `b` on one net.
fn share_a_net(parent: &SchematicSheet, child: &SchematicSheet, a: &str, b: &str) -> bool {
    let mut sheets = HashMap::new();
    sheets.insert(SheetKey::new("root"), parent.clone());
    sheets.insert(SheetKey::new("child.sch"), child.clone());
    let mut resolved = HashMap::new();
    resolved.insert(
        SheetKey::new("root"),
        HashMap::from([("child.sch".to_string(), SheetKey::new("child.sch"))]),
    );
    resolved.insert(SheetKey::new("child.sch"), HashMap::new());
    let roots = [ProjectRoot {
        key: SheetKey::new("root"),
    }];
    let nets = build_project_netlist(&ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    })
    .netlist
    .nets;
    nets.iter().any(|net| {
        net.terminals.iter().any(|t| t.reference == a)
            && net.terminals.iter().any(|t| t.reference == b)
    })
}

#[test]
fn wired_pins_keep_their_nets_when_a_label_is_added_and_synced() {
    let mut child = test_sheet();
    child.lib_symbols.insert("R".to_string(), lib_resistor());
    // Child: labels B and C, each wired to its own resistor.
    for (name, y) in [("B", 0.0), ("C", 20.0)] {
        child.wires.push(Wire {
            start: Point::new(0.0, y),
            end: Point::new(10.0, y),
            ..Default::default()
        });
        child
            .labels
            .push(hierarchical_label(name, Point::new(0.0, y)));
        place(&mut child, &format!("RC{name}"), Point::new(10.0, y));
    }

    let mut symbol = symbol_with(Vec::new());
    reconcile_child_sheet_pins(&mut symbol, &[port("B", ""), port("C", "")]);

    // Parent: a resistor wired to each pin where the sync put it.
    let mut parent = test_sheet();
    parent.lib_symbols.insert("R".to_string(), lib_resistor());
    for name in ["B", "C"] {
        let at = pin_at(&symbol, name).position;
        parent.wires.push(Wire {
            start: at,
            end: Point::new(at.x - 5.0, at.y),
            ..Default::default()
        });
        place(
            &mut parent,
            &format!("RP{name}"),
            Point::new(at.x - 5.0, at.y),
        );
    }
    parent.child_sheets.push(symbol);
    assert!(share_a_net(&parent, &child, "RPB", "RCB"), "precondition");
    assert!(share_a_net(&parent, &child, "RPC", "RCC"), "precondition");

    // The child gains label A, which sorts before B and C; sync again.
    child
        .labels
        .push(hierarchical_label("A", Point::new(0.0, 40.0)));
    reconcile_child_sheet_pins(
        &mut parent.child_sheets[0],
        &[port("A", ""), port("B", ""), port("C", "")],
    );

    assert!(share_a_net(&parent, &child, "RPB", "RCB"), "B kept its net");
    assert!(share_a_net(&parent, &child, "RPC", "RCC"), "C kept its net");
    assert!(!share_a_net(&parent, &child, "RPB", "RCC"));
}

// ---- a freed or full slot must not capture a neighbour's net ----------------

/// Reconcile the symbol at `parent.child_sheets[0]` as the app does: blockers
/// come from the parent sheet's own wires, labels and junctions.
fn sync(parent: &mut SchematicSheet, ports: &[SheetPort]) {
    let blockers = SheetPinContacts::of_sheet(parent);
    reconcile_with(&mut parent.child_sheets[0], ports, &blockers);
}

/// A parent sheet whose symbol was synced to `{A, B}` and whose pin A has a
/// wire ending on it.
fn parent_with_wired_a() -> (SchematicSheet, Point) {
    let mut parent = test_sheet();
    let mut symbol = symbol_with(Vec::new());
    reconcile_child_sheet_pins(&mut symbol, &[port("A", ""), port("B", "")]);
    let a = pin_at(&symbol, "A").position;
    parent.wires.push(Wire {
        start: a,
        end: Point::new(a.x - 5.0, a.y),
        ..Default::default()
    });
    parent.child_sheets.push(symbol);
    (parent, a)
}

#[test]
fn a_pin_added_as_another_is_dropped_avoids_the_dropped_pins_wire() {
    let (mut parent, a) = parent_with_wired_a();

    sync(&mut parent, &[port("B", ""), port("Z", "")]);

    assert_ne!(pin_at(&parent.child_sheets[0], "Z").position, a);
}

#[test]
fn a_pin_added_after_a_drop_in_an_earlier_sync_avoids_the_dangling_wire() {
    let (mut parent, a) = parent_with_wired_a();
    sync(&mut parent, &[port("B", "")]);
    assert!(parent.child_sheets[0].pins.iter().all(|p| p.name != "A"));

    sync(&mut parent, &[port("B", ""), port("Z", "")]);

    assert_ne!(pin_at(&parent.child_sheets[0], "Z").position, a);
}

#[test]
fn a_dropped_pins_wire_does_not_join_the_new_pins_net() {
    let mut child = test_sheet();
    child.lib_symbols.insert("R".to_string(), lib_resistor());
    child.wires.push(Wire {
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        ..Default::default()
    });
    child
        .labels
        .push(hierarchical_label("Z", Point::new(0.0, 0.0)));
    place(&mut child, "RCZ", Point::new(10.0, 0.0));

    let (mut parent, a) = parent_with_wired_a();
    parent.lib_symbols.insert("R".to_string(), lib_resistor());
    place(&mut parent, "RPA", Point::new(a.x - 5.0, a.y));
    assert!(!share_a_net(&parent, &child, "RPA", "RCZ"), "precondition");

    // The child lost A and gained Z.
    sync(&mut parent, &[port("Z", "")]);

    assert!(!share_a_net(&parent, &child, "RPA", "RCZ"));
}

#[test]
fn a_slot_under_a_wire_interior_or_label_is_not_free() {
    let mut parent = test_sheet();
    parent.child_sheets.push(symbol_with(Vec::new()));
    // A wire passing through slot 0 and a label anchored on slot 1.
    parent.wires.push(Wire {
        start: Point::new(95.0, 102.54),
        end: Point::new(105.0, 102.54),
        ..Default::default()
    });
    parent
        .labels
        .push(hierarchical_label("X", Point::new(100.0, 105.08)));

    sync(&mut parent, &[port("Z", "")]);

    let z = pin_at(&parent.child_sheets[0], "Z").position;
    assert!((z.y - 107.62).abs() < 1e-6, "Z lands on slot 2, got {z:?}");
}

#[test]
fn a_full_edge_places_no_pin_instead_of_stacking() {
    // 6 mm tall: only two distinct slots (102.54 and the clamped 103.46).
    let mut symbol = symbol_with(Vec::new());
    symbol.size = (30.0, 6.0);

    reconcile_child_sheet_pins(&mut symbol, &[port("A", ""), port("B", ""), port("C", "")]);

    let mut names: Vec<_> = symbol.pins.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["A", "B"], "C has no free slot and is left unplaced");
    assert_ne!(pin_at(&symbol, "A").position, pin_at(&symbol, "B").position);
}

#[test]
fn a_slot_under_a_junction_or_no_connect_is_not_free() {
    let mut parent = test_sheet();
    parent.child_sheets.push(symbol_with(Vec::new()));
    parent.junctions.push(Junction {
        uuid: uuid::Uuid::now_v7(),
        position: Point::new(100.0, 102.54),
        diameter: 0.0,
        ..Default::default()
    });
    parent.no_connects.push(NoConnect {
        uuid: uuid::Uuid::now_v7(),
        position: Point::new(100.0, 105.08),
    });

    sync(&mut parent, &[port("Z", "")]);

    let z = pin_at(&parent.child_sheets[0], "Z").position;
    assert!((z.y - 107.62).abs() < 1e-6, "Z lands on slot 2, got {z:?}");
}

#[test]
fn a_pin_never_lands_on_another_sheet_symbols_pin() {
    let mut parent = test_sheet();
    // Left symbol's right edge is the right symbol's left edge (x = 100).
    let mut left = symbol_with(Vec::new());
    left.position = Point::new(70.0, 100.0);
    reconcile_child_sheet_pins(&mut left, &[port("O", "output")]);
    let o = pin_at(&left, "O").position;
    parent.child_sheets.push(left);
    parent.child_sheets.push(symbol_with(Vec::new()));

    let contacts = SheetPinContacts::of_sheet(&parent);
    reconcile_with(
        &mut parent.child_sheets[1],
        &[port("Z", "input")],
        &contacts,
    );

    assert_ne!(pin_at(&parent.child_sheets[1], "Z").position, o);
}

#[test]
fn a_power_port_on_a_dropped_pin_does_not_capture_the_new_pins_net() {
    let mut child = test_sheet();
    child.lib_symbols.insert("R".to_string(), lib_resistor());
    child.wires.push(Wire {
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        ..Default::default()
    });
    child
        .labels
        .push(hierarchical_label("Z", Point::new(0.0, 0.0)));
    place(&mut child, "RCZ", Point::new(10.0, 0.0));

    let (mut parent, a) = parent_with_wired_a();
    parent.wires.clear();
    parent.lib_symbols.insert("R".to_string(), lib_resistor());
    // A GND port sitting directly on pin A (no wire), and another GND port
    // with a resistor on it elsewhere.
    for (at, reference) in [(a, "#PWR1"), (Point::new(50.0, 50.0), "#PWR2")] {
        parent.symbols.push(Symbol {
            lib_id: "R".to_string(),
            reference: reference.to_string(),
            value: "GND".to_string(),
            is_power: true,
            position: at,
            ..Default::default()
        });
    }
    place(&mut parent, "RPG", Point::new(50.0, 50.0));

    sync(&mut parent, &[port("B", "")]);
    sync(&mut parent, &[port("B", ""), port("Z", "")]);

    assert!(!share_a_net(&parent, &child, "RPG", "RCZ"), "Z joined GND");
    assert_ne!(pin_at(&parent.child_sheets[0], "Z").position, a);
}
