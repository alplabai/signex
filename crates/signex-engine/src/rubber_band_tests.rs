//! Rubber-band move (#963): wires follow the pins of moved symbols.
//!
//! Fixture: resistor `R1` at (50, 50) with pin 1 at (45, 50) and pin 2 at
//! (55, 50); a wire into each pin from a fixed far end.

use crate::test_support::test_sheet;
use crate::{Command, Engine};
use signex_types::schematic::{
    ChildSheet, FillType, HAlign, Junction, Label, LabelType, LibPin, LibSymbol, Pin, PinDirection,
    PinShapeStyle, Point, SchematicSheet, SelectedItem, SelectedKind, SheetPin, Symbol,
    SymbolTransform, VAlign, Wire,
};
use uuid::Uuid;

fn lib_pin(number: &str, x: f64) -> LibPin {
    unit_pin(number, x, 0)
}

fn unit_pin(number: &str, x: f64, unit: u32) -> LibPin {
    LibPin {
        unit,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 0.0,
            name: String::new(),
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

type Segment = ((f64, f64), (f64, f64));

fn norm(seg: Segment) -> Segment {
    if (seg.0.0, seg.0.1) <= (seg.1.0, seg.1.1) {
        seg
    } else {
        (seg.1, seg.0)
    }
}

/// Wires as direction-free segments.
fn segments(engine: &Engine) -> Vec<Segment> {
    engine
        .document()
        .wires
        .iter()
        .map(|w| norm(((w.start.x, w.start.y), (w.end.x, w.end.y))))
        .collect()
}

fn has(segs: &[Segment], a: (f64, f64), b: (f64, f64)) -> bool {
    segs.contains(&norm((a, b)))
}

fn mv(engine: &mut Engine, items: Vec<SelectedItem>, dx: f64, dy: f64, keep_wires: bool) {
    engine
        .execute(Command::MoveSelection {
            items,
            dx,
            dy,
            keep_wires,
        })
        .expect("move");
}

fn symbol_item(uuid: Uuid) -> SelectedItem {
    SelectedItem::new(uuid, SelectedKind::Symbol)
}

fn add_resistor(sheet: &mut SchematicSheet, reference: &str, at: Point) -> Uuid {
    sheet
        .lib_symbols
        .entry("R".to_string())
        .or_insert_with(|| lib_symbol("R", vec![lib_pin("1", -5.0), lib_pin("2", 5.0)]));
    place(sheet, reference, "R", at)
}

fn lib_symbol(id: &str, pins: Vec<LibPin>) -> LibSymbol {
    LibSymbol {
        id: id.to_string(),
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
        pins,
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn place(sheet: &mut SchematicSheet, reference: &str, lib_id: &str, at: Point) -> Uuid {
    let uuid = Uuid::new_v4();
    sheet.symbols.push(Symbol {
        uuid,
        lib_id: lib_id.to_string(),
        reference: reference.to_string(),
        position: at,
        unit: 1,
        ..Default::default()
    });
    uuid
}

fn add_wire(sheet: &mut SchematicSheet, a: (f64, f64), b: (f64, f64)) -> Uuid {
    let uuid = Uuid::new_v4();
    sheet.wires.push(Wire {
        uuid,
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        ..Default::default()
    });
    uuid
}

fn add_label(sheet: &mut SchematicSheet, text: &str, at: (f64, f64)) {
    sheet.labels.push(Label {
        unknown_columns: Default::default(),
        uuid: Uuid::new_v4(),
        text: text.to_string(),
        position: Point::new(at.0, at.1),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
    });
}

fn add_dot(sheet: &mut SchematicSheet, at: (f64, f64), minted: bool) {
    sheet.junctions.push(Junction {
        uuid: Uuid::new_v4(),
        position: Point::new(at.0, at.1),
        minted,
        ..Default::default()
    });
}

/// Net identity independent of ids: each net's name and terminals.
type NetShape = Vec<(String, Vec<(Uuid, String)>)>;

fn net_shape(sheet: &SchematicSheet) -> NetShape {
    let mut nets: NetShape = signex_net::build_netlist(sheet)
        .nets
        .iter()
        .map(|net| {
            let mut t: Vec<_> = net
                .terminals
                .iter()
                .map(|t| (t.symbol, t.pin.clone()))
                .collect();
            t.sort();
            // Unlabelled nets are numbered by sort order (`N$3`); only a
            // real name is identity.
            let name = if net.name.starts_with("N$") {
                String::new()
            } else {
                net.name.clone()
            };
            (name, t)
        })
        .collect();
    nets.sort();
    nets
}

fn orthogonal(seg: &Segment) -> bool {
    seg.0.0 == seg.1.0 || seg.0.1 == seg.1.1
}

fn assert_clean(engine: &mut Engine, before: &NetShape) {
    assert!(engine.take_warnings().is_empty());
    assert_eq!(&net_shape(engine.document()), before, "netlist changed");
    let segs = segments(engine);
    assert!(segs.iter().all(orthogonal), "diagonal wire: {segs:?}");
    assert!(
        segs.iter().all(|s| s.0 != s.1),
        "zero-length wire: {segs:?}"
    );
}

/// Two resistors in series: R1 (50,50) pins 45/55, R2 (80,50) pins 75/85,
/// wires (30,50)-(45,50), (55,50)-(75,50), (85,50)-(100,50).
fn series_fixture() -> (Engine, Uuid, Uuid) {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    let r2 = add_resistor(&mut sheet, "R2", Point::new(80.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (55.0, 50.0), (75.0, 50.0));
    add_wire(&mut sheet, (85.0, 50.0), (100.0, 50.0));
    (Engine::new(sheet).expect("engine"), r1, r2)
}

#[test]
fn one_end_on_a_pin_follows_it_along_the_wire() {
    let (mut engine, r1, _) = series_fixture();
    mv(&mut engine, vec![symbol_item(r1)], -5.0, 0.0, true);
    let segs = segments(&engine);
    assert_eq!(segs.len(), 3, "a collinear follow adds no corner: {segs:?}");
    assert!(has(&segs, (30.0, 50.0), (40.0, 50.0)));
    assert!(has(&segs, (50.0, 50.0), (75.0, 50.0)));
    assert!(
        has(&segs, (85.0, 50.0), (100.0, 50.0)),
        "far wire untouched"
    );
}

#[test]
fn off_axis_move_inserts_an_orthogonal_bend_keeping_the_fixed_end() {
    let (mut engine, r1, _) = series_fixture();
    let before = net_shape(engine.document());
    mv(&mut engine, vec![symbol_item(r1)], 0.0, 10.0, true);
    let segs = segments(&engine);
    assert_eq!(segs.len(), 5, "one corner per attached wire: {segs:?}");
    // Horizontal wires keep the fixed end's row; the tail turns at the
    // pin's new column and runs to the pin's new tip.
    assert!(has(&segs, (30.0, 50.0), (45.0, 50.0)));
    assert!(has(&segs, (45.0, 50.0), (45.0, 60.0)));
    assert!(has(&segs, (55.0, 50.0), (75.0, 50.0)));
    assert!(has(&segs, (55.0, 50.0), (55.0, 60.0)));
    assert_clean(&mut engine, &before);
}

#[test]
fn vertical_wire_keeps_its_column_and_turns_at_the_pins_new_row() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (45.0, 30.0), (45.0, 50.0));
    let mut engine = Engine::new(sheet).expect("engine");
    mv(&mut engine, vec![symbol_item(r1)], 10.0, 5.0, true);
    let segs = segments(&engine);
    assert_eq!(segs.len(), 2, "{segs:?}");
    assert!(has(&segs, (45.0, 30.0), (45.0, 55.0)));
    assert!(has(&segs, (45.0, 55.0), (55.0, 55.0)));
}

#[test]
fn wire_between_two_moved_symbols_translates_whole() {
    let (mut engine, r1, r2) = series_fixture();
    let before = net_shape(engine.document());
    mv(
        &mut engine,
        vec![symbol_item(r1), symbol_item(r2)],
        0.0,
        10.0,
        true,
    );
    let segs = segments(&engine);
    assert!(has(&segs, (55.0, 60.0), (75.0, 60.0)), "{segs:?}");
    assert_eq!(segs.len(), 5, "only the two outer wires gain a corner");
    assert_clean(&mut engine, &before);
}

#[test]
fn selected_wires_move_as_before_and_are_not_rerouted() {
    let (mut engine, r1, _) = series_fixture();
    let w0 = engine.document().wires[0].uuid;
    mv(
        &mut engine,
        vec![symbol_item(r1), SelectedItem::new(w0, SelectedKind::Wire)],
        0.0,
        10.0,
        true,
    );
    let segs = segments(&engine);
    assert!(has(&segs, (30.0, 60.0), (45.0, 60.0)));
    assert_eq!(
        segs.len(),
        4,
        "only the unselected right wire bends: {segs:?}"
    );
}

#[test]
fn keep_wires_off_is_a_plain_translate() {
    let (mut engine, r1, _) = series_fixture();
    let before = segments(&engine);
    mv(&mut engine, vec![symbol_item(r1)], 0.0, 10.0, false);
    assert_eq!(segments(&engine), before);
}

#[test]
fn move_and_wire_edits_are_one_undo_step_that_restores_exactly() {
    let (mut engine, r1, _) = series_fixture();
    let before: Vec<(Uuid, Point, Point)> = engine
        .document()
        .wires
        .iter()
        .map(|w| (w.uuid, w.start, w.end))
        .collect();
    mv(&mut engine, vec![symbol_item(r1)], 7.0, 10.0, true);
    let moved = segments(&engine);
    assert_eq!(moved.len(), 5);

    engine.undo().expect("undo");
    let after: Vec<(Uuid, Point, Point)> = engine
        .document()
        .wires
        .iter()
        .map(|w| (w.uuid, w.start, w.end))
        .collect();
    assert_eq!(after, before, "one undo restores every wire exactly");
    assert_eq!(
        engine.document().symbols[0].position,
        Point::new(50.0, 50.0)
    );

    engine.redo().expect("redo");
    assert_eq!(segments(&engine), moved, "redo reapplies the same edit");
}

#[test]
fn the_netlist_is_unchanged_by_rubber_band_moves() {
    let (mut engine, r1, r2) = series_fixture();
    let before = net_shape(engine.document());
    assert_eq!(before.len(), 3, "fixture has three nets: {before:?}");
    mv(&mut engine, vec![symbol_item(r1)], 7.5, -12.5, true);
    assert_clean(&mut engine, &before);
    mv(
        &mut engine,
        vec![symbol_item(r1), symbol_item(r2)],
        -2.5,
        5.0,
        true,
    );
    assert_clean(&mut engine, &before);
}

#[test]
fn the_plain_move_breaks_the_netlist_which_is_what_rubber_banding_fixes() {
    let (mut engine, r1, _) = series_fixture();
    let before = net_shape(engine.document());
    mv(&mut engine, vec![symbol_item(r1)], 0.0, 10.0, false);
    assert_ne!(net_shape(engine.document()), before);
}

#[test]
fn the_patch_reports_the_edited_wires() {
    let (mut engine, r1, _) = series_fixture();
    let result = engine
        .execute(Command::MoveSelection {
            items: vec![symbol_item(r1)],
            dx: 0.0,
            dy: 10.0,
            keep_wires: true,
        })
        .expect("move");
    assert!(
        result
            .patch_pair
            .expect("patch")
            .document
            .contains(crate::DocumentPatch::WIRES)
    );
}

/// Review repro (a): another symbol's pin shares the moved pin's tip.
#[test]
fn a_stationary_pin_sharing_the_tip_stays_wired() {
    let mut sheet = test_sheet();
    let a = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_resistor(&mut sheet, "R2", Point::new(40.0, 50.0)); // pin 2 at (45,50)
    add_wire(&mut sheet, (45.0, 50.0), (45.0, 30.0));
    let mut engine = Engine::new(sheet).expect("engine");
    let before = net_shape(engine.document());
    assert!(before.iter().any(|(_, t)| t.len() == 2), "{before:?}");

    mv(&mut engine, vec![symbol_item(a)], 0.0, 10.0, true);
    assert_clean(&mut engine, &before);
    assert!(
        has(&segments(&engine), (45.0, 30.0), (45.0, 50.0)),
        "the stationary pin's wire must not be dragged away"
    );
}

/// Review repro (b): the tip sits on a trunk with a minted junction.
#[test]
fn a_tip_on_a_trunk_junction_keeps_the_dot_and_the_stub() {
    let mut sheet = test_sheet();
    let a = add_resistor(&mut sheet, "R1", Point::new(15.0, 0.0)); // pin 1 at (10,0)
    add_wire(&mut sheet, (0.0, 0.0), (20.0, 0.0));
    add_wire(&mut sheet, (10.0, -10.0), (10.0, 0.0));
    add_dot(&mut sheet, (10.0, 0.0), true);
    let mut engine = Engine::new(sheet).expect("engine");
    let before = net_shape(engine.document());

    mv(&mut engine, vec![symbol_item(a)], 0.0, 5.0, true);
    assert_clean(&mut engine, &before);
    assert_eq!(
        engine.document().junctions.len(),
        1,
        "{:?}",
        engine.document().junctions
    );
    assert_eq!(
        engine.document().junctions[0].position,
        Point::new(10.0, 0.0)
    );
}

#[test]
fn a_net_label_on_the_dragged_end_keeps_its_net_name() {
    let (mut engine, r1, _) = {
        let mut sheet = test_sheet();
        let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
        add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
        add_label(&mut sheet, "VCC", (45.0, 50.0));
        (Engine::new(sheet).expect("engine"), r1, ())
    };
    let before = net_shape(engine.document());
    assert!(before.iter().any(|(name, _)| name == "VCC"), "{before:?}");
    mv(&mut engine, vec![symbol_item(r1)], 0.0, 10.0, true);
    assert_clean(&mut engine, &before);
}

/// The label stays put, so a wire that shrinks away from it would orphan it.
#[test]
fn a_label_on_the_dragged_end_survives_a_shrinking_move() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_label(&mut sheet, "VCC", (45.0, 50.0));
    let mut engine = Engine::new(sheet).expect("engine");
    let before = net_shape(engine.document());
    mv(&mut engine, vec![symbol_item(r1)], -7.5, 0.0, true);
    assert_clean(&mut engine, &before);
}

/// A joint that a moving pin still has to leave must not slide away.
#[test]
fn an_elbow_on_another_moved_pin_does_not_slide() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(40.0, 50.0)); // pins 35, 45
    add_wire(&mut sheet, (35.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (45.0, 45.0), (45.0, 50.0));
    let mut engine = Engine::new(sheet).expect("engine");
    let before = net_shape(engine.document());
    mv(&mut engine, vec![symbol_item(r1)], 10.0, -10.0, true);
    assert_clean(&mut engine, &before);
}

/// A new path's corner can sit on another moved pin's old tip; wires placed
/// for the first pin must not be dragged along with the second.
#[test]
fn a_new_paths_corner_on_another_moved_tip_is_not_dragged_away() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(60.0, 50.0)); // pins 55, 65
    add_label(&mut sheet, "SIG", (55.0, 50.0));
    let mut engine = Engine::new(sheet).expect("engine");
    let before = net_shape(engine.document());
    mv(&mut engine, vec![symbol_item(r1)], 10.0, 7.5, true);
    assert_clean(&mut engine, &before);
}

#[test]
fn a_label_on_a_wire_between_moved_symbols_keeps_its_net_name() {
    let (mut engine, r1, r2) = {
        let (engine, r1, r2) = series_fixture();
        let mut doc = engine.document().clone();
        add_label(&mut doc, "MID", (65.0, 50.0));
        (Engine::new(doc).expect("engine"), r1, r2)
    };
    let before = net_shape(engine.document());
    mv(
        &mut engine,
        vec![symbol_item(r1), symbol_item(r2)],
        0.0,
        10.0,
        true,
    );
    assert_clean(&mut engine, &before);
}

#[test]
fn a_foreign_wire_at_the_preferred_corner_picks_the_other_corner() {
    let (mut engine, r1, _) = {
        let mut sheet = test_sheet();
        let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
        add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
        // A foreign net's wire ends on the preferred tail (45,50)-(45,60).
        add_wire(&mut sheet, (45.0, 55.0), (45.0, 58.0));
        (Engine::new(sheet).expect("engine"), r1, ())
    };
    let before = net_shape(engine.document());
    mv(&mut engine, vec![symbol_item(r1)], 0.0, 10.0, true);
    assert_clean(&mut engine, &before);
    let segs = segments(&engine);
    assert!(has(&segs, (30.0, 50.0), (30.0, 60.0)), "{segs:?}");
    assert!(has(&segs, (30.0, 60.0), (45.0, 60.0)), "{segs:?}");
}

#[test]
fn when_every_route_collides_the_pin_moves_alone_and_a_warning_names_it() {
    let (mut engine, r1, _) = {
        let mut sheet = test_sheet();
        let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
        add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
        add_wire(&mut sheet, (45.0, 55.0), (45.0, 58.0));
        add_wire(&mut sheet, (30.0, 55.0), (30.0, 58.0));
        (Engine::new(sheet).expect("engine"), r1, ())
    };
    let before = segments(&engine);
    mv(&mut engine, vec![symbol_item(r1)], 0.0, 10.0, true);
    assert_eq!(
        segments(&engine),
        before,
        "wires untouched, as a plain move"
    );
    let warnings = engine.take_warnings();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("R1 pin 1"), "{warnings:?}");
}

#[test]
fn twenty_alternating_moves_keep_the_wire_count_bounded() {
    let (mut engine, r1, _) = series_fixture();
    let before = net_shape(engine.document());
    let steps = [(0.0, 5.0), (5.0, 0.0), (0.0, -5.0), (-5.0, 0.0)];
    for round in 0..20 {
        let (dx, dy) = steps[round % 4];
        mv(&mut engine, vec![symbol_item(r1)], dx, dy, true);
        let n = engine.document().wires.len();
        assert!(n <= 6, "round {round}: {n} wires {:?}", segments(&engine));
        assert_clean(&mut engine, &before);
    }
    assert_eq!(engine.document().wires.len(), 3, "back home: no leftovers");
}

fn pin_tip(sheet: &SchematicSheet, symbol: Uuid, index: usize) -> Point {
    let sym = sheet.symbols.iter().find(|s| s.uuid == symbol).unwrap();
    let lib = &sheet.lib_symbols[&sym.lib_id];
    SymbolTransform::from_symbol(sym).apply(lib.pins[index].pin.position)
}

#[test]
fn rotated_and_mirrored_symbols_keep_their_wires() {
    for (rotation, mirror_x, mirror_y) in [
        (90.0, false, false),
        (270.0, true, false),
        (180.0, false, true),
    ] {
        let mut sheet = test_sheet();
        let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
        {
            let sym = sheet.symbols.iter_mut().find(|s| s.uuid == r1).unwrap();
            sym.rotation = rotation;
            sym.mirror_x = mirror_x;
            sym.mirror_y = mirror_y;
        }
        for index in 0..2 {
            let tip = pin_tip(&sheet, r1, index);
            let far = (
                tip.x
                    + if tip.x > 50.5 {
                        20.0
                    } else if tip.x < 49.5 {
                        -20.0
                    } else {
                        0.0
                    },
                tip.y
                    + if (tip.x - 50.0).abs() < 0.5 {
                        if tip.y > 50.0 { 20.0 } else { -20.0 }
                    } else {
                        0.0
                    },
            );
            add_wire(&mut sheet, (far.0, far.1), (tip.x, tip.y));
        }
        let mut engine = Engine::new(sheet).expect("engine");
        let before = net_shape(engine.document());
        assert_eq!(before.len(), 2, "rotation {rotation}: {before:?}");
        mv(&mut engine, vec![symbol_item(r1)], 7.5, -5.0, true);
        assert_clean(&mut engine, &before);
    }
}

#[test]
fn only_the_placed_units_pins_drag_wires() {
    let mut sheet = test_sheet();
    sheet.lib_symbols.insert(
        "U".to_string(),
        lib_symbol("U", vec![unit_pin("1", -5.0, 1), unit_pin("9", 5.0, 2)]),
    );
    let u = place(&mut sheet, "U1", "U", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0)); // unit 1 pin
    add_wire(&mut sheet, (55.0, 50.0), (75.0, 50.0)); // unit 2 pin: not placed
    let mut engine = Engine::new(sheet).expect("engine");
    mv(&mut engine, vec![symbol_item(u)], 0.0, 10.0, true);
    let segs = segments(&engine);
    assert!(has(&segs, (55.0, 50.0), (75.0, 50.0)), "{segs:?}");
    assert!(has(&segs, (45.0, 50.0), (45.0, 60.0)), "{segs:?}");
    assert!(
        !segs
            .iter()
            .any(|s| s.0 == (55.0, 60.0) || s.1 == (55.0, 60.0)),
        "the unplaced unit's pin must not be dragged: {segs:?}"
    );
}

#[test]
fn a_power_port_keeps_its_wire_and_its_net_name() {
    let mut sheet = test_sheet();
    sheet.lib_symbols.insert(
        "GND".to_string(),
        lib_symbol("GND", vec![lib_pin("1", 0.0)]),
    );
    let gnd = place(&mut sheet, "#PWR1", "GND", Point::new(50.0, 50.0));
    {
        let sym = sheet.symbols.iter_mut().find(|s| s.uuid == gnd).unwrap();
        sym.is_power = true;
        sym.value = "GND".to_string();
    }
    let r1 = add_resistor(&mut sheet, "R1", Point::new(70.0, 50.0));
    add_wire(&mut sheet, (50.0, 50.0), (65.0, 50.0));
    let mut engine = Engine::new(sheet).expect("engine");
    let before = net_shape(engine.document());
    assert!(before.iter().any(|(name, _)| name == "GND"), "{before:?}");
    mv(&mut engine, vec![symbol_item(gnd)], 0.0, 10.0, true);
    assert_clean(&mut engine, &before);
    let _ = r1;
}

fn child_fixture() -> (Engine, Uuid, Uuid) {
    let mut sheet = test_sheet();
    let pin = SheetPin {
        uuid: Uuid::new_v4(),
        name: "IN".to_string(),
        direction: "input".to_string(),
        position: Point::new(100.0, 110.0),
        rotation: 0.0,
        auto_generated: false,
        user_moved: false,
    };
    let pin_id = pin.uuid;
    let child = ChildSheet {
        uuid: Uuid::new_v4(),
        name: "Child".to_string(),
        filename: "child.snxsch".to_string(),
        position: Point::new(100.0, 100.0),
        size: (30.0, 30.0),
        stroke_width: 0.12,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![pin],
        instances: Vec::new(),
    };
    let child_id = child.uuid;
    sheet.child_sheets.push(child);
    add_wire(&mut sheet, (80.0, 110.0), (100.0, 110.0));
    (Engine::new(sheet).expect("engine"), child_id, pin_id)
}

#[test]
fn moving_a_child_sheet_drags_the_wire_on_its_pin() {
    let (mut engine, child, _) = child_fixture();
    mv(
        &mut engine,
        vec![SelectedItem::new(child, SelectedKind::ChildSheet)],
        10.0,
        5.0,
        true,
    );
    let pin = engine.document().child_sheets[0].pins[0].position;
    assert_eq!(pin, Point::new(110.0, 115.0));
    let segs = segments(&engine);
    assert!(segs.iter().all(orthogonal), "{segs:?}");
    assert!(
        segs.iter()
            .any(|s| s.0 == (pin.x, pin.y) || s.1 == (pin.x, pin.y)),
        "a wire must reach the moved pin: {segs:?}"
    );
}

#[test]
fn sliding_a_sheet_pin_along_its_edge_drags_its_wire() {
    let (mut engine, _, pin_id) = child_fixture();
    mv(
        &mut engine,
        vec![SelectedItem::new(pin_id, SelectedKind::SheetPin)],
        0.0,
        8.0,
        true,
    );
    let pin = engine.document().child_sheets[0].pins[0].position;
    assert_ne!(pin, Point::new(100.0, 110.0), "the pin slid");
    let segs = segments(&engine);
    assert!(segs.iter().all(orthogonal), "{segs:?}");
    assert!(
        segs.iter()
            .any(|s| s.0 == (pin.x, pin.y) || s.1 == (pin.x, pin.y)),
        "a wire must reach the slid pin {pin:?}: {segs:?}"
    );
}

#[test]
fn move_each_in_one_step_translates_a_wire_between_alike_moved_symbols() {
    let (mut engine, r1, r2) = series_fixture();
    let before = net_shape(engine.document());
    engine
        .execute(Command::MoveEach {
            moves: vec![(symbol_item(r1), 0.0, 10.0), (symbol_item(r2), 0.0, 10.0)],
            keep_wires: true,
        })
        .expect("move each");
    let segs = segments(&engine);
    assert!(has(&segs, (55.0, 60.0), (75.0, 60.0)), "{segs:?}");
    assert_clean(&mut engine, &before);
    engine.undo().expect("undo");
    assert_eq!(engine.document().wires.len(), 3, "one undo step");
}

#[test]
fn move_each_with_different_deltas_bends_and_keeps_the_nets() {
    let (mut engine, r1, r2) = series_fixture();
    let before = net_shape(engine.document());
    engine
        .execute(Command::MoveEach {
            moves: vec![(symbol_item(r1), 0.0, 10.0), (symbol_item(r2), 0.0, 20.0)],
            keep_wires: true,
        })
        .expect("move each");
    assert_clean(&mut engine, &before);
}

#[test]
fn a_junction_on_the_tip_moves_with_it() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (45.0, 30.0), (45.0, 50.0));
    add_dot(&mut sheet, (45.0, 50.0), false);
    let mut engine = Engine::new(sheet).expect("engine");
    let before = net_shape(engine.document());
    mv(&mut engine, vec![symbol_item(r1)], 0.0, 10.0, true);
    assert_clean(&mut engine, &before);
    assert!(
        engine
            .document()
            .junctions
            .iter()
            .any(|j| j.position == Point::new(45.0, 60.0)),
        "{:?}",
        engine.document().junctions
    );
}

// --- property test -------------------------------------------------------

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % bound
    }
}

/// Geometry dump for failure messages.
fn describe(sheet: &SchematicSheet) -> String {
    let wires: Vec<_> = sheet
        .wires
        .iter()
        .map(|w| format!("({},{})-({},{})", w.start.x, w.start.y, w.end.x, w.end.y))
        .collect();
    let tips: Vec<_> = sheet
        .symbols
        .iter()
        .map(|s| {
            let a = pin_tip(sheet, s.uuid, 0);
            let b = pin_tip(sheet, s.uuid, 1);
            format!("{}[{},{} | {},{}]", s.reference, a.x, a.y, b.x, b.y)
        })
        .collect();
    let labels: Vec<_> = sheet
        .labels
        .iter()
        .map(|l| format!("{}@({},{})", l.text, l.position.x, l.position.y))
        .collect();
    let dots: Vec<_> = sheet
        .junctions
        .iter()
        .map(|j| {
            format!(
                "({},{}){}",
                j.position.x,
                j.position.y,
                if j.minted { "m" } else { "u" }
            )
        })
        .collect();
    format!("wires {wires:?} symbols {tips:?} labels {labels:?} dots {dots:?}")
}

struct Case {
    sheet: SchematicSheet,
    moved: Vec<Uuid>,
    dx: f64,
    dy: f64,
}

/// A random small schematic and a random move of one or two of its symbols.
/// Symbols sit on a 10 mm pitch so neighbours share pin tips; wires are
/// random orthogonal pairs between pin tips and free grid points; labels
/// sit on random points; editor-style junctions are added wherever a wire
/// end lands on another wire. Geometry the move would land a pin on is
/// pruned (that is user placement, not rubber-banding), as is any wire with
/// a pin strictly inside it (the netlist does not connect those).
fn random_case(rng: &mut Lcg) -> Option<Case> {
    use signex_net::{point_on_segment, pt_key};
    let mut sheet = test_sheet();
    let mut symbols = Vec::new();
    for col in 0..6u64 {
        for row in 0..3u64 {
            if rng.next(100) < 55 {
                let at = Point::new(20.0 + 10.0 * col as f64, 20.0 + 15.0 * row as f64);
                let id = add_resistor(&mut sheet, &format!("R{}", symbols.len() + 1), at);
                {
                    let sym = sheet.symbols.last_mut().unwrap();
                    sym.mirror_y = rng.next(100) < 25;
                    sym.rotation = 90.0 * rng.next(4) as f64;
                }
                symbols.push(id);
            }
        }
    }
    if symbols.is_empty() {
        return None;
    }
    let picks = 1 + rng.next(2) as usize;
    let mut moved: Vec<Uuid> = (0..picks)
        .map(|_| symbols[rng.next(symbols.len() as u64) as usize])
        .collect();
    moved.dedup();
    moved.sort();
    moved.dedup();
    let dx = 2.5 * (rng.next(9) as f64 - 4.0);
    let dy = 2.5 * (rng.next(9) as f64 - 4.0);
    if dx == 0.0 && dy == 0.0 {
        return None;
    }

    let mut old_tips = Vec::new();
    let mut new_tips = Vec::new();
    let mut points: Vec<(f64, f64)> = Vec::new();
    for sym in &sheet.symbols {
        for index in 0..2 {
            let tip = pin_tip(&sheet, sym.uuid, index);
            points.push((tip.x, tip.y));
            old_tips.push(pt_key(&tip));
            if moved.contains(&sym.uuid) {
                new_tips.push(pt_key(&Point::new(tip.x + dx, tip.y + dy)));
            }
        }
    }
    let all: Vec<_> = old_tips.iter().chain(new_tips.iter()).copied().collect();
    // A moved pin may not land on a stationary pin or on another moved
    // pin: that is a new connection the user makes, not one to preserve.
    let stationary: Vec<_> = sheet
        .symbols
        .iter()
        .filter(|s| !moved.contains(&s.uuid))
        .flat_map(|s| {
            (0..2)
                .map(|i| pt_key(&pin_tip(&sheet, s.uuid, i)))
                .collect::<Vec<_>>()
        })
        .collect();
    let mut distinct = new_tips.clone();
    distinct.sort();
    distinct.dedup();
    if distinct.len() != new_tips.len() || new_tips.iter().any(|k| stationary.contains(k)) {
        return None;
    }
    for _ in 0..8 {
        points.push((
            15.0 + 5.0 * rng.next(14) as f64,
            15.0 + 5.0 * rng.next(8) as f64,
        ));
    }
    for _ in 0..12 {
        let a = points[rng.next(points.len() as u64) as usize];
        let b = points[rng.next(points.len() as u64) as usize];
        let elbow = if rng.next(2) == 0 {
            (b.0, a.1)
        } else {
            (a.0, b.1)
        };
        for (p, q) in [(a, elbow), (elbow, b)] {
            if p != q {
                add_wire(&mut sheet, p, q);
            }
        }
    }
    let keep_wire = |w: &Wire| {
        let (a, b) = (pt_key(&w.start), pt_key(&w.end));
        !all.iter()
            .any(|k| *k != a && *k != b && point_on_segment(*k, a, b))
            && !new_tips.contains(&a)
            && !new_tips.contains(&b)
            && !new_tips.iter().any(|k| point_on_segment(*k, a, b))
    };
    sheet.wires.retain(keep_wire);
    for n in 0..2 {
        let at = points[rng.next(points.len() as u64) as usize];
        if !new_tips.contains(&pt_key(&Point::new(at.0, at.1))) {
            add_label(&mut sheet, &format!("N{n}"), at);
        }
    }
    let ends: Vec<(f64, f64)> = sheet
        .wires
        .iter()
        .flat_map(|w| [(w.start.x, w.start.y), (w.end.x, w.end.y)])
        .collect();
    for end in ends {
        let k = pt_key(&Point::new(end.0, end.1));
        let inside = sheet.wires.iter().any(|w| {
            let (a, b) = (pt_key(&w.start), pt_key(&w.end));
            k != a && k != b && point_on_segment(k, a, b)
        });
        let has_dot = sheet.junctions.iter().any(|j| pt_key(&j.position) == k);
        if inside && !has_dot {
            add_dot(&mut sheet, end, true);
        }
    }
    Some(Case {
        sheet,
        moved,
        dx,
        dy,
    })
}

#[test]
fn random_rubber_band_moves_never_change_the_netlist() {
    let mut rng = Lcg(0x5EED_0963);
    let mut exercised = 0;
    let mut rerouted = 0;
    for index in 0..4000 {
        let Some(Case {
            sheet,
            moved,
            dx,
            dy,
        }) = random_case(&mut rng)
        else {
            continue;
        };
        let before = net_shape(&sheet);
        let described = describe(&sheet);
        let wires_before: Vec<_> = sheet.wires.iter().map(|w| (w.start, w.end)).collect();
        let mut engine = Engine::new(sheet).expect("engine");
        let items = moved.iter().map(|u| symbol_item(*u)).collect();
        mv(&mut engine, items, dx, dy, true);
        if !engine.take_warnings().is_empty() {
            continue;
        }
        exercised += 1;
        let wires_after: Vec<_> = engine
            .document()
            .wires
            .iter()
            .map(|w| (w.start, w.end))
            .collect();
        rerouted += usize::from(wires_after != wires_before);
        assert_eq!(
            net_shape(engine.document()),
            before,
            "case {index}: moved {moved:?} by ({dx},{dy})\nBEFORE {described}\nAFTER {}",
            describe(engine.document())
        );
        for w in &engine.document().wires {
            assert_ne!(
                signex_net::pt_key(&w.start),
                signex_net::pt_key(&w.end),
                "case {index}: zero-length wire"
            );
        }
    }
    assert!(exercised > 500, "only {exercised} usable cases");
    assert!(rerouted > 200, "only {rerouted} cases rerouted wires");
}
