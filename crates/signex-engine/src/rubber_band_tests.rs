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

pub(crate) fn lib_pin(number: &str, x: f64) -> LibPin {
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
pub(crate) fn segments(engine: &Engine) -> Vec<Segment> {
    engine
        .document()
        .wires
        .iter()
        .map(|w| norm(((w.start.x, w.start.y), (w.end.x, w.end.y))))
        .collect()
}

pub(crate) fn has(segs: &[Segment], a: (f64, f64), b: (f64, f64)) -> bool {
    segs.contains(&norm((a, b)))
}

pub(crate) fn mv(
    engine: &mut Engine,
    items: Vec<SelectedItem>,
    dx: f64,
    dy: f64,
    keep_wires: bool,
) {
    engine
        .execute(Command::MoveSelection {
            items,
            dx,
            dy,
            keep_wires,
        })
        .expect("move");
}

pub(crate) fn symbol_item(uuid: Uuid) -> SelectedItem {
    SelectedItem::new(uuid, SelectedKind::Symbol)
}

pub(crate) fn add_resistor(sheet: &mut SchematicSheet, reference: &str, at: Point) -> Uuid {
    sheet
        .lib_symbols
        .entry("R".to_string())
        .or_insert_with(|| lib_symbol("R", vec![lib_pin("1", -5.0), lib_pin("2", 5.0)]));
    place(sheet, reference, "R", at)
}

pub(crate) fn lib_symbol(id: &str, pins: Vec<LibPin>) -> LibSymbol {
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

pub(crate) fn place(sheet: &mut SchematicSheet, reference: &str, lib_id: &str, at: Point) -> Uuid {
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

pub(crate) fn add_wire(sheet: &mut SchematicSheet, a: (f64, f64), b: (f64, f64)) -> Uuid {
    let uuid = Uuid::new_v4();
    sheet.wires.push(Wire {
        uuid,
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        ..Default::default()
    });
    uuid
}

pub(crate) fn add_label(sheet: &mut SchematicSheet, text: &str, at: (f64, f64)) {
    sheet.labels.push(Label {
        unknown_columns: Default::default(),
        uuid: Uuid::new_v4(),
        text: text.to_string(),
        position: Point::new(at.0, at.1),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked: false,
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
pub(crate) type NetShape = Vec<(String, Vec<(Uuid, String)>)>;

pub(crate) fn net_shape(sheet: &SchematicSheet) -> NetShape {
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

pub(crate) fn orthogonal(seg: &Segment) -> bool {
    seg.0.0 == seg.1.0 || seg.0.1 == seg.1.1
}

pub(crate) fn assert_clean(engine: &mut Engine, before: &NetShape) {
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
pub(crate) fn series_fixture() -> (Engine, Uuid, Uuid) {
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
    add_resistor(&mut sheet, "R1", Point::new(70.0, 50.0));
    add_wire(&mut sheet, (50.0, 50.0), (65.0, 50.0));
    let mut engine = Engine::new(sheet).expect("engine");
    let before = net_shape(engine.document());
    assert!(before.iter().any(|(name, _)| name == "GND"), "{before:?}");
    mv(&mut engine, vec![symbol_item(gnd)], 0.0, 10.0, true);
    assert_clean(&mut engine, &before);
}

pub(crate) fn child_fixture() -> (Engine, Uuid, Uuid) {
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

pub(crate) struct Lcg(pub(crate) u64);

impl Lcg {
    pub(crate) fn next(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % bound
    }
}

/// Geometry dump for failure messages.
pub(crate) fn describe(sheet: &SchematicSheet) -> String {
    let wires: Vec<_> = sheet
        .wires
        .iter()
        .map(|w| format!("({},{})-({},{})", w.start.x, w.start.y, w.end.x, w.end.y))
        .collect();
    let tips: Vec<_> = sheet
        .symbols
        .iter()
        .map(|s| {
            let n = sheet.lib_symbols[&s.lib_id].pins.len();
            let tips: Vec<String> = (0..n)
                .map(|i| {
                    let p = pin_tip(sheet, s.uuid, i);
                    format!("{},{}", p.x, p.y)
                })
                .collect();
            let value = if s.is_power {
                format!("={}", s.value)
            } else {
                String::new()
            };
            format!("{}{value}[{}]", s.reference, tips.join(" | "))
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

pub(crate) struct Case {
    pub(crate) sheet: SchematicSheet,
    pub(crate) moves: Vec<(SelectedItem, f64, f64)>,
    /// A wire is selected: it moves plainly, so nets may change by design.
    pub(crate) selects_wire: bool,
}

pub(crate) fn add_labelled(sheet: &mut SchematicSheet, rng: &mut Lcg, at: (f64, f64)) {
    let text = ["A", "B", "GND"][rng.next(3) as usize];
    let label_type = [
        LabelType::Net,
        LabelType::Global,
        LabelType::Power,
        LabelType::Hierarchical,
    ][rng.next(4) as usize];
    sheet.labels.push(Label {
        unknown_columns: Default::default(),
        uuid: Uuid::new_v4(),
        text: text.to_string(),
        position: Point::new(at.0, at.1),
        rotation: 0.0,
        label_type,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked: false,
    });
}

/// A random small schematic and a random move, with nothing pruned: pins
/// land on foreign wire ends, interiors, pins and labels, T-junctions often
/// have no dot, labels share names across kinds, no-connects sit on pins,
/// symbols are multi-unit or power ports, a child sheet has pins, and
/// sometimes a wire is selected or each item moves by its own delta.
pub(crate) fn random_case(rng: &mut Lcg) -> Option<Case> {
    let mut sheet = test_sheet();
    sheet.lib_symbols.insert(
        "U".to_string(),
        lib_symbol("U", vec![unit_pin("1", -5.0, 1), unit_pin("2", 5.0, 2)]),
    );
    sheet.lib_symbols.insert(
        "GND".to_string(),
        lib_symbol("GND", vec![lib_pin("1", 0.0)]),
    );
    let mut symbols = Vec::new();
    for col in 0..6u64 {
        for row in 0..3u64 {
            if rng.next(100) >= 55 {
                continue;
            }
            let at = Point::new(20.0 + 10.0 * col as f64, 20.0 + 15.0 * row as f64);
            let reference = format!("R{}", symbols.len() + 1);
            let id = match rng.next(100) {
                0..=69 => add_resistor(&mut sheet, &reference, at),
                70..=84 => {
                    let id = place(&mut sheet, &reference, "U", at);
                    sheet.symbols.last_mut().unwrap().unit = 1 + rng.next(2) as u32;
                    id
                }
                _ => {
                    let id = place(&mut sheet, &format!("#PWR{}", symbols.len()), "GND", at);
                    let sym = sheet.symbols.last_mut().unwrap();
                    sym.is_power = true;
                    sym.value = "GND".to_string();
                    id
                }
            };
            let sym = sheet.symbols.last_mut().unwrap();
            sym.mirror_y = rng.next(100) < 25;
            sym.mirror_x = rng.next(100) < 15;
            sym.rotation = 90.0 * rng.next(4) as f64;
            symbols.push(id);
        }
    }
    if symbols.is_empty() {
        return None;
    }
    let mut child = None;
    if rng.next(100) < 25 {
        let pins: Vec<SheetPin> = (0..2)
            .map(|i| SheetPin {
                uuid: Uuid::new_v4(),
                name: format!("P{i}"),
                direction: "input".to_string(),
                position: Point::new(100.0, 65.0 + 5.0 * i as f64),
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            })
            .collect();
        let sheet_pin = pins[0].uuid;
        let id = Uuid::new_v4();
        sheet.child_sheets.push(ChildSheet {
            uuid: id,
            name: "Child".to_string(),
            filename: "child.snxsch".to_string(),
            position: Point::new(100.0, 60.0),
            size: (30.0, 20.0),
            stroke_width: 0.12,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins,
            instances: Vec::new(),
        });
        child = Some((id, sheet_pin));
    }

    let mut points: Vec<(f64, f64)> = Vec::new();
    for sym in &sheet.symbols {
        let lib = &sheet.lib_symbols[&sym.lib_id];
        for index in 0..lib.pins.len() {
            let tip = pin_tip(&sheet, sym.uuid, index);
            points.push((tip.x, tip.y));
        }
    }
    for pin in sheet.child_sheets.iter().flat_map(|c| c.pins.iter()) {
        points.push((pin.position.x, pin.position.y));
    }
    for _ in 0..8 {
        points.push((
            15.0 + 5.0 * rng.next(16) as f64,
            15.0 + 5.0 * rng.next(14) as f64,
        ));
    }
    for _ in 0..(6 + rng.next(8)) {
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
    // Some T-junctions get their dot, some do not.
    let ends: Vec<(f64, f64)> = sheet
        .wires
        .iter()
        .flat_map(|w| [(w.start.x, w.start.y), (w.end.x, w.end.y)])
        .collect();
    for end in ends {
        let k = signex_net::pt_key(&Point::new(end.0, end.1));
        let inside = sheet.wires.iter().any(|w| {
            let (a, b) = (signex_net::pt_key(&w.start), signex_net::pt_key(&w.end));
            k != a && k != b && signex_net::point_on_segment(k, a, b)
        });
        let has_dot = sheet
            .junctions
            .iter()
            .any(|j| signex_net::pt_key(&j.position) == k);
        if inside && !has_dot && rng.next(100) < 50 {
            add_dot(&mut sheet, end, rng.next(2) == 0);
        }
    }
    for _ in 0..3 {
        let at = points[rng.next(points.len() as u64) as usize];
        add_labelled(&mut sheet, rng, at);
    }
    if rng.next(100) < 30 {
        let at = points[rng.next(points.len() as u64) as usize];
        sheet.no_connects.push(signex_types::schematic::NoConnect {
            uuid: Uuid::new_v4(),
            position: Point::new(at.0, at.1),
        });
    }

    let mut moves = Vec::new();
    let picks = 1 + rng.next(2) as usize;
    let mut chosen: Vec<Uuid> = (0..picks)
        .map(|_| symbols[rng.next(symbols.len() as u64) as usize])
        .collect();
    chosen.sort();
    chosen.dedup();
    let uniform = (
        2.5 * (rng.next(9) as f64 - 4.0),
        2.5 * (rng.next(9) as f64 - 4.0),
    );
    let each = rng.next(100) < 25;
    for id in chosen {
        let delta = if each {
            (
                2.5 * (rng.next(9) as f64 - 4.0),
                2.5 * (rng.next(9) as f64 - 4.0),
            )
        } else {
            uniform
        };
        moves.push((symbol_item(id), delta.0, delta.1));
    }
    if let Some((child_id, pin_id)) = child {
        match rng.next(10) {
            0 | 1 => moves.push((
                SelectedItem::new(child_id, SelectedKind::ChildSheet),
                uniform.0,
                uniform.1,
            )),
            2 => moves.push((SelectedItem::new(pin_id, SelectedKind::SheetPin), 0.0, 2.5)),
            _ => {}
        }
    }
    // Co-selected labels, dots and no-connects leave their unselected wires:
    // the user's own edit, which changes nets the pass must not be blamed for.
    let mut co_selected = false;
    if rng.next(100) < 25 {
        if !sheet.labels.is_empty() {
            let label = sheet.labels[rng.next(sheet.labels.len() as u64) as usize].uuid;
            moves.push((
                SelectedItem::new(label, SelectedKind::Label),
                uniform.0,
                uniform.1,
            ));
            co_selected = true;
        }
        if !sheet.junctions.is_empty() {
            let dot = sheet.junctions[rng.next(sheet.junctions.len() as u64) as usize].uuid;
            moves.push((
                SelectedItem::new(dot, SelectedKind::Junction),
                uniform.0,
                uniform.1,
            ));
            co_selected = true;
        }
        if !sheet.no_connects.is_empty() {
            let mark = sheet.no_connects[0].uuid;
            moves.push((
                SelectedItem::new(mark, SelectedKind::NoConnect),
                uniform.0,
                uniform.1,
            ));
            co_selected = true;
        }
    }
    let selects_wire = rng.next(100) < 10 && !sheet.wires.is_empty();
    if selects_wire {
        let wire = sheet.wires[rng.next(sheet.wires.len() as u64) as usize].uuid;
        moves.push((
            SelectedItem::new(wire, SelectedKind::Wire),
            uniform.0,
            uniform.1,
        ));
    }
    if moves.iter().all(|(_, dx, dy)| *dx == 0.0 && *dy == 0.0) {
        return None;
    }
    Some(Case {
        sheet,
        moves,
        selects_wire: selects_wire || co_selected,
    })
}

fn overlap_pairs(sheet: &SchematicSheet) -> usize {
    let mut count = 0;
    for (i, a) in sheet.wires.iter().enumerate() {
        for b in &sheet.wires[i + 1..] {
            count += usize::from(crate::transform::rubber_band::wires_overlap(a, b));
        }
    }
    count
}

/// The sheet without the power ports among `skip_terminals`: a power port
/// that moved plainly took its name-merges with it.
fn without_skipped_power(
    sheet: &SchematicSheet,
    skip_terminals: &std::collections::BTreeSet<(Uuid, String)>,
) -> SchematicSheet {
    let mut copy = sheet.clone();
    copy.symbols
        .retain(|s| !(s.is_power && skip_terminals.iter().any(|(id, _)| *id == s.uuid)));
    copy
}

/// The nets with the pins in `skip` taken out (see [`Guard`](crate::Guard)).
fn projected(
    sheet: &SchematicSheet,
    skip_terminals: &std::collections::BTreeSet<(Uuid, String)>,
    skip_pins: &std::collections::BTreeSet<Uuid>,
) -> std::collections::BTreeSet<signex_net::NetSignature> {
    signex_net::net_signatures(&without_skipped_power(sheet, skip_terminals))
        .iter()
        .filter_map(|net| net.without(skip_terminals, skip_pins))
        .collect()
}

/// What a net can hold, for relations that do not care about ids or names.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub(crate) enum Item {
    Pin(Uuid, String),
    Label(Uuid),
    SheetPin(Uuid),
    /// A wire none of the three sheets edited: follows geometry through bare
    /// runs of wire that no terminal or label names.
    Wire(Uuid),
}

type Nets = Vec<std::collections::BTreeSet<Item>>;

fn items_per_net(sheet: &SchematicSheet, stable: &std::collections::HashSet<Uuid>) -> Nets {
    signex_net::net_members(sheet)
        .into_iter()
        .map(|(net, wires)| {
            net.terminals
                .into_iter()
                .map(|(s, p)| Item::Pin(s, p))
                .chain(net.labels.into_iter().map(Item::Label))
                .chain(net.sheet_pins.into_iter().map(Item::SheetPin))
                .chain(
                    wires
                        .into_iter()
                        .filter(|w| stable.contains(w))
                        .map(Item::Wire),
                )
                .collect()
        })
        .filter(|net: &std::collections::BTreeSet<Item>| !net.is_empty())
        .collect()
}

/// Wires present with identical geometry in every sheet.
fn unedited_wires(sheets: &[&SchematicSheet]) -> std::collections::HashSet<Uuid> {
    let geometry = |sheet: &SchematicSheet| -> std::collections::HashMap<Uuid, _> {
        sheet
            .wires
            .iter()
            .map(|w| {
                (
                    w.uuid,
                    (signex_net::pt_key(&w.start), signex_net::pt_key(&w.end)),
                )
            })
            .collect()
    };
    let maps: Vec<_> = sheets.iter().map(|s| geometry(s)).collect();
    maps[0]
        .iter()
        .filter(|(uuid, g)| maps[1..].iter().all(|m| m.get(*uuid) == Some(*g)))
        .map(|(uuid, _)| *uuid)
        .collect()
}

fn net_index(nets: &Nets) -> std::collections::HashMap<Item, usize> {
    nets.iter()
        .enumerate()
        .flat_map(|(i, net)| net.iter().map(move |item| (item.clone(), i)))
        .collect()
}

/// Union-find over nodes: everything that shares a net in any of `all`.
fn closure<K: Clone + Eq + std::hash::Hash>(all: &[&[Vec<K>]]) -> std::collections::HashMap<K, K> {
    fn find<K: Clone + Eq + std::hash::Hash>(
        parent: &mut std::collections::HashMap<K, K>,
        item: &K,
    ) -> K {
        let mut root = item.clone();
        while let Some(next) = parent.get(&root) {
            if *next == root {
                break;
            }
            root = next.clone();
        }
        root
    }
    let mut parent = std::collections::HashMap::new();
    for nets in all {
        for net in nets.iter() {
            let mut members = net.iter();
            let Some(first) = members.next() else {
                continue;
            };
            parent.entry(first.clone()).or_insert_with(|| first.clone());
            for other in members {
                parent.entry(other.clone()).or_insert_with(|| other.clone());
                let (a, b) = (find(&mut parent, first), find(&mut parent, other));
                parent.insert(a, b);
            }
        }
    }
    let items: Vec<K> = parent.keys().cloned().collect();
    for item in items {
        let root = find(&mut parent, &item);
        parent.insert(item, root);
    }
    parent
}

/// The guard-independent oracle: relative to the nets before the move and the
/// nets a plain move leaves, the rubber-band result may not join anything the
/// two never joined (a short) nor part anything both kept together (a split),
/// except a warned pin, which must then be exactly where the plain move put it.
/// A moved pin is judged on either side but bridges neither: it may stay on
/// its old net or land where the plain move puts it, not join the two.
pub(crate) fn check_closure_oracle(
    before: &SchematicSheet,
    plain: &SchematicSheet,
    after: &SchematicSheet,
    warned: &std::collections::BTreeSet<Item>,
    moved_pins: &std::collections::BTreeSet<Item>,
) -> Result<(), String> {
    // Wires the plain move leaves alone bridge the before and plain nets; the
    // rubber-banded result is judged on identified items only.
    let stable = unedited_wires(&[before, plain]);
    let (b, p, r) = (
        items_per_net(before, &stable),
        items_per_net(plain, &stable),
        items_per_net(after, &stable),
    );
    // A node is an item, plus whether it is a moved pin's plain-move side.
    let nodes = |nets: &Nets, plain_side: bool| -> Vec<Vec<(Item, bool)>> {
        nets.iter()
            .map(|net| {
                net.iter()
                    .map(|i| (i.clone(), plain_side && moved_pins.contains(i)))
                    .collect()
            })
            .collect()
    };
    let joined = closure(&[&nodes(&b, false), &nodes(&p, true)]);
    let roots = |item: &Item| -> std::collections::HashSet<&(Item, bool)> {
        [(item.clone(), false), (item.clone(), true)]
            .iter()
            .filter_map(|node| joined.get(node))
            .collect()
    };
    for net in &r {
        // Wires only bridge: a bare run of wire joining a pin changes nothing
        // electrical, two identified items joining does.
        let mut members = net
            .iter()
            .filter(|i| !warned.contains(i) && !matches!(i, Item::Wire(_)));
        let Some(first) = members.next() else {
            continue;
        };
        let mut shared = roots(first);
        for other in members {
            let theirs = roots(other);
            shared.retain(|root| theirs.contains(root));
            if shared.is_empty() {
                return Err(format!("SHORT: {first:?} joined {other:?}"));
            }
        }
    }
    let (bi, pi, ri) = (net_index(&b), net_index(&p), net_index(&r));
    for net in &b {
        let live: Vec<&Item> = net
            .iter()
            .filter(|i| !warned.contains(i) && !matches!(i, Item::Wire(_)))
            .collect();
        for (n, first) in live.iter().enumerate() {
            for other in &live[n + 1..] {
                let together = |m: &std::collections::HashMap<Item, usize>| {
                    m.get(*first).is_some() && m.get(*first) == m.get(*other)
                };
                if together(&bi) && together(&pi) && !together(&ri) {
                    return Err(format!("SPLIT: {first:?} parted from {other:?}"));
                }
            }
        }
    }
    for pin in warned {
        let members = |nets: &Nets,
                       index: &std::collections::HashMap<Item, usize>|
         -> std::collections::BTreeSet<Item> {
            index
                .get(pin)
                .map(|i| {
                    nets[*i]
                        .iter()
                        .filter(|m| !moved_pins.contains(m))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        };
        if members(&r, &ri) != members(&p, &pi) {
            return Err(format!("WARNED {pin:?} is not where the plain move put it"));
        }
    }
    Ok(())
}

pub(crate) fn moved_pin_items(tips: &[crate::PinTip]) -> std::collections::BTreeSet<Item> {
    tips.iter()
        .map(|tip| match tip.ident() {
            crate::PinIdent::Terminal(s, p) => Item::Pin(*s, p.clone()),
            crate::PinIdent::SheetPin(u) => Item::SheetPin(*u),
        })
        .collect()
}

pub(crate) fn warned_items(
    sheet: &SchematicSheet,
    unkept: &[String],
) -> std::collections::BTreeSet<Item> {
    let (terminals, pins) = warned_skips(sheet, unkept);
    terminals
        .into_iter()
        .map(|(s, p)| Item::Pin(s, p))
        .chain(pins.into_iter().map(Item::SheetPin))
        .collect()
}

/// Cases, and cases that fell back to a plain move with a notice.
#[derive(Debug, Default, Clone, Copy)]
struct Share {
    cases: usize,
    warned: usize,
}

/// One seeded run's counts. Symbol-only cases and cases with a co-selected
/// wire, label, dot or no-connect are counted apart: the co-selected ones fall
/// back more often, and one shared cap would let them hide a regression in
/// the symbol-only ones.
#[derive(Debug, Default)]
struct RunCounts {
    symbols_only: Share,
    co_selected: Share,
    /// Cases that edited wires.
    rerouted: usize,
}

impl RunCounts {
    fn add(&mut self, other: &Self) {
        for (mine, theirs) in [
            (&mut self.symbols_only, other.symbols_only),
            (&mut self.co_selected, other.co_selected),
        ] {
            mine.cases += theirs.cases;
            mine.warned += theirs.warned;
        }
        self.rerouted += other.rerouted;
    }
}

#[test]
fn random_rubber_band_moves_never_change_the_netlist() {
    let mut total = RunCounts::default();
    for seed in [0x5EED_0963, 0xC0FFEE, 0xBADC_AB1E, 0x1234_5678] {
        total.add(&property_run(seed, 2000));
    }
    let exercised = total.symbols_only.cases + total.co_selected.cases;
    assert!(exercised > 4000, "only {exercised} cases");
    assert!(
        total.rerouted * 3 > exercised,
        "only {} cases rerouted wires",
        total.rerouted
    );
    // Measured at 22.6 % and 26.9 % (#1177); the caps leave about two points.
    for (kind, share, cap) in [
        ("symbol-only", total.symbols_only, 25),
        ("co-selected", total.co_selected, 29),
    ] {
        assert!(
            share.warned * 100 < share.cases * cap,
            "{} of {} {kind} cases fell back to a plain move",
            share.warned,
            share.cases
        );
    }
}

/// One seeded run. Every case is held to the closure oracle, the overlap and
/// zero-length checks, and (unless user edits are co-selected) strict net
/// equality.
fn property_run(seed: u64, cases: usize) -> RunCounts {
    let mut rng = Lcg(seed);
    let mut counts = RunCounts::default();
    for index in 0..cases {
        let Some(Case {
            sheet,
            moves,
            selects_wire,
        }) = random_case(&mut rng)
        else {
            continue;
        };
        let described = describe(&sheet);
        let overlaps_before = overlap_pairs(&sheet);
        let wires_before: Vec<_> = sheet.wires.iter().map(|w| (w.start, w.end)).collect();

        // Pins the netlist did not see before are the user's to place.
        let mut skip_terminals = std::collections::BTreeSet::new();
        let mut skip_pins = std::collections::BTreeSet::new();
        let items: Vec<SelectedItem> = moves.iter().map(|(item, _, _)| *item).collect();
        let tips = crate::moved_pin_tips(&sheet, &items);
        let seen = crate::Guard::new(&sheet, &sheet, &tips, None);
        for tip in tips.iter().filter(|t| !seen.was_connected(t)) {
            match tip.ident() {
                crate::PinIdent::Terminal(symbol, pin) => {
                    skip_terminals.insert((*symbol, pin.clone()));
                }
                crate::PinIdent::SheetPin(uuid) => {
                    skip_pins.insert(*uuid);
                }
            }
        }

        let run = |keep_wires: bool| {
            let mut engine = Engine::new(sheet.clone()).expect("engine");
            engine
                .execute(Command::MoveEach {
                    moves: moves.clone(),
                    keep_wires,
                })
                .expect("move");
            engine
        };
        let plain = run(false);
        let mut engine = run(true);
        let warnings = engine.take_warnings();
        let unkept = engine.take_unkept_pins();
        let share = if selects_wire {
            &mut counts.co_selected
        } else {
            &mut counts.symbols_only
        };
        share.cases += 1;
        share.warned += usize::from(!warnings.is_empty());
        let after = engine.document();
        counts.rerouted += usize::from(
            after
                .wires
                .iter()
                .map(|w| (w.start, w.end))
                .collect::<Vec<_>>()
                != wires_before,
        );

        for w in &after.wires {
            assert_ne!(
                signex_net::pt_key(&w.start),
                signex_net::pt_key(&w.end),
                "seed {seed:#x} case {index}: zero-length wire\n{described}"
            );
        }
        assert!(
            overlap_pairs(after) <= overlaps_before.max(overlap_pairs(plain.document())),
            "seed {seed:#x} case {index}: new overlapping wires\nBEFORE {described}\nAFTER {}",
            describe(after)
        );
        if let Err(why) = check_closure_oracle(
            &sheet,
            plain.document(),
            after,
            &warned_items(&sheet, &unkept),
            &moved_pin_items(&tips),
        ) {
            panic!(
                "seed {seed:#x} case {index}: {why} (warnings {warnings:?}, moves {moves:?})\nBEFORE {described}\nAFTER {}",
                describe(after)
            );
        }
        let moves_power = sheet
            .symbols
            .iter()
            .any(|s| s.is_power && moves.iter().any(|(item, _, _)| item.uuid == s.uuid));
        if selects_wire || moves_power {
            // Co-selected items move plainly, wherever the user puts them, and
            // a power port landing on a net merges it by name: the closure
            // oracle above is the check for these cases.
            continue;
        }
        // A warned pin keeps the old plain behaviour; every other net stays.
        let (warned_terminals, warned_pins) = warned_skips(&sheet, &unkept);
        skip_terminals.extend(warned_terminals);
        skip_pins.extend(warned_pins);
        let want: std::collections::BTreeSet<_> =
            signex_net::net_signatures(&without_skipped_power(&sheet, &skip_terminals))
                .iter()
                .filter_map(|n| n.without(&skip_terminals, &skip_pins))
                .collect();
        let got = projected(after, &skip_terminals, &skip_pins);
        assert_eq!(
            got,
            want,
            "seed {seed:#x} case {index}: nets changed (warnings {warnings:?}, moves {moves:?})\nBEFORE {described}\nAFTER {}",
            describe(after)
        );
    }
    counts
}

/// No exact duplicates and no collinear overlaps among the document's wires.
fn assert_no_overlaps(e: &Engine) {
    let wires = &e.document().wires;
    for (i, a) in wires.iter().enumerate() {
        for b in &wires[i + 1..] {
            assert!(
                !crate::transform::rubber_band::wires_overlap(a, b),
                "overlapping wires {:?}-{:?} and {:?}-{:?}",
                a.start,
                a.end,
                b.start,
                b.end
            );
        }
    }
}

/// Pins named by `warnings` ("R1 pin 2 ...") and child-sheet pins.
fn warned_skips(
    sheet: &SchematicSheet,
    unkept: &[String],
) -> (
    std::collections::BTreeSet<(Uuid, String)>,
    std::collections::BTreeSet<Uuid>,
) {
    let mut terminals = std::collections::BTreeSet::new();
    let mut pins = std::collections::BTreeSet::new();
    for sym in &sheet.symbols {
        for lp in &sheet.lib_symbols[&sym.lib_id].pins {
            let id = if lp.pin.number.is_empty() {
                &lp.pin.name
            } else {
                &lp.pin.number
            };
            if unkept.contains(&format!("{} pin {id}", sym.reference)) {
                terminals.insert((sym.uuid, id.clone()));
            }
        }
    }
    for child in &sheet.child_sheets {
        for pin in &child.pins {
            if unkept.contains(&format!("{} pin {}", child.name, pin.name)) {
                pins.insert(pin.uuid);
            }
        }
    }
    (terminals, pins)
}

/// Every net survives, except for pins the engine warned about and pins that
/// were on no net before and are among the `moved` symbols (landing a free
/// pin on a wire is the user's call; a stationary pin must not be dragged in).
fn assert_nets_hold(e: &mut Engine, before: &SchematicSheet, moved: &[Uuid]) {
    let warnings = e.take_warnings();
    let unkept = e.take_unkept_pins();
    let (mut terminals, pins) = warned_skips(before, &unkept);
    let was: std::collections::BTreeSet<_> = signex_net::net_signatures(before)
        .iter()
        .flat_map(|n| n.terminals.iter().cloned())
        .collect();
    for net in signex_net::net_signatures(e.document()) {
        for t in net.terminals {
            if !was.contains(&t) && moved.contains(&t.0) {
                terminals.insert(t);
            }
        }
    }
    let want: std::collections::BTreeSet<_> =
        signex_net::net_signatures(&without_skipped_power(before, &terminals))
            .iter()
            .filter_map(|n| n.without(&terminals, &pins))
            .collect();
    let got: std::collections::BTreeSet<_> =
        signex_net::net_signatures(&without_skipped_power(e.document(), &terminals))
            .iter()
            .filter_map(|n| n.without(&terminals, &pins))
            .collect();
    assert_eq!(got, want, "nets changed (warnings {warnings:?})");
    assert_no_overlaps(e);
}

// ---- review regressions: nets must survive every landing and overlap ----

#[test]
fn moving_a_part_by_its_own_pitch_does_not_short_its_pins() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0)); // 45/55
    let _r3 = add_resistor(&mut sheet, "R3", Point::new(25.0, 50.0)); // 20/30
    let _r2 = add_resistor(&mut sheet, "R2", Point::new(60.0, 70.0)); // 55/65 @ y70
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (55.0, 50.0), (55.0, 70.0));
    let before_sheet = sheet.clone();
    let mut e = Engine::new(sheet).unwrap();
    mv(&mut e, vec![symbol_item(r1)], 10.0, 0.0, true);
    assert_nets_hold(&mut e, &before_sheet, &[r1]);
}

#[test]
fn a_pin_landing_on_a_foreign_wire_interior_does_not_merge_nets() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0)); // 45/55
    let _r2 = add_resistor(&mut sheet, "R2", Point::new(25.0, 50.0)); // 20/30
    let _r5 = add_resistor(&mut sheet, "R5", Point::new(50.0, 90.0)); // 45/55 @ y90
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (45.0, 60.0), (45.0, 90.0)); // foreign net, R5.1
    let before_sheet = sheet.clone();
    let mut e = Engine::new(sheet).unwrap();
    mv(&mut e, vec![symbol_item(r1)], 0.0, 15.0, true);
    assert_nets_hold(&mut e, &before_sheet, &[r1]);
}

#[test]
fn a_pin_between_two_collinear_wires_writes_no_duplicate() {
    let mut sheet = test_sheet();
    // vertical part: pins at (50,45) and (50,55) via rotation 90
    let tp = add_resistor(&mut sheet, "TP1", Point::new(50.0, 50.0));
    sheet.symbols.last_mut().unwrap().rotation = 90.0;
    let a = pin_tip(&sheet, tp, 0);
    let b = pin_tip(&sheet, tp, 1);
    let top = if a.y < b.y { a } else { b };
    let _l = add_resistor(&mut sheet, "R1", Point::new(top.x - 25.0, top.y)); // pin2 at x-20
    let _r = add_resistor(&mut sheet, "R2", Point::new(top.x + 25.0, top.y)); // pin1 at x+20
    add_wire(&mut sheet, (top.x - 20.0, top.y), (top.x, top.y));
    add_wire(&mut sheet, (top.x, top.y), (top.x + 20.0, top.y));
    let before = net_shape(&sheet);
    let mut e = Engine::new(sheet).unwrap();
    mv(&mut e, vec![symbol_item(tp)], 0.0, -5.0, true);
    let segs = segments(&e);
    assert_eq!(net_shape(e.document()), before, "T3 netlist changed");
    let mut s2 = segs.clone();
    s2.sort_by(|x, y| x.partial_cmp(y).unwrap());
    s2.dedup();
    assert_eq!(s2.len(), segs.len(), "T3 duplicate wires {segs:?}");
}

#[test]
fn a_pin_landing_on_a_foreign_wire_end_does_not_merge_nets() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    let _r2 = add_resistor(&mut sheet, "R2", Point::new(25.0, 50.0));
    let _r5 = add_resistor(&mut sheet, "R5", Point::new(50.0, 90.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (45.0, 65.0), (45.0, 90.0));
    let before_sheet = sheet.clone();
    let mut e = Engine::new(sheet).unwrap();
    mv(&mut e, vec![symbol_item(r1)], 0.0, 15.0, true);
    assert_nets_hold(&mut e, &before_sheet, &[r1]);
}

#[test]
fn a_pin_landing_on_an_unwired_stationary_pin_keeps_the_nets() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    let _r2 = add_resistor(&mut sheet, "R2", Point::new(25.0, 50.0));
    let _r5 = add_resistor(&mut sheet, "R5", Point::new(50.0, 65.0)); // pin1 at (45,65), unwired
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    let before_sheet = sheet.clone();
    let mut e = Engine::new(sheet).unwrap();
    mv(&mut e, vec![symbol_item(r1)], 0.0, 15.0, true);
    assert_nets_hold(&mut e, &before_sheet, &[r1]);
}

#[test]
fn a_pin_landing_on_a_dangling_label_does_not_rename_a_net() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    let _r2 = add_resistor(&mut sheet, "R2", Point::new(25.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_label(&mut sheet, "SDA", (45.0, 65.0));
    let before_sheet = sheet.clone();
    let mut e = Engine::new(sheet).unwrap();
    mv(&mut e, vec![symbol_item(r1)], 0.0, 15.0, true);
    assert_nets_hold(&mut e, &before_sheet, &[r1]);
}

#[test]
fn a_junctionless_t_at_a_reused_fixed_end_gets_no_dot() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    let _r2 = add_resistor(&mut sheet, "R2", Point::new(25.0, 30.0)); // 20/30 @ y30
    let _r9 = add_resistor(&mut sheet, "R9", Point::new(25.0, 70.0)); // 20/30 @ y70
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0)); // stub to R1.1
    add_wire(&mut sheet, (30.0, 30.0), (30.0, 70.0)); // foreign trunk, (30,50) on interior, no dot
    let before = net_shape(&sheet);
    let mut e = Engine::new(sheet).unwrap();
    mv(&mut e, vec![symbol_item(r1)], 0.0, 5.0, true);
    assert_eq!(net_shape(e.document()), before, "T7 netlist changed");
}

#[test]
fn move_each_with_opposite_deltas_keeps_the_shared_wire_connected() {
    let (mut e, r1, r2) = series_fixture();
    let before = net_shape(e.document());
    e.execute(Command::MoveEach {
        moves: vec![(symbol_item(r1), 0.0, 10.0), (symbol_item(r2), 0.0, -10.0)],
        keep_wires: true,
    })
    .unwrap();
    assert_eq!(net_shape(e.document()), before, "T8 netlist changed");
    e.undo().unwrap();
}

#[test]
fn moving_past_the_fixed_end_keeps_the_nets() {
    // move beyond the fixed end of a wire: wire (30,50)-(45,50), move left by 25 -> new (20,50)
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    let _r2 = add_resistor(&mut sheet, "R2", Point::new(25.0, 40.0)); // pin2 (30,40)
    add_wire(&mut sheet, (30.0, 40.0), (30.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    let before_sheet = sheet.clone();
    let mut e = Engine::new(sheet).unwrap();
    mv(&mut e, vec![symbol_item(r1)], -25.0, 0.0, true);
    assert_nets_hold(&mut e, &before_sheet, &[r1]);
}

#[test]
fn a_selected_wire_overlapping_a_follower_is_merged() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    let _r2 = add_resistor(&mut sheet, "R2", Point::new(25.0, 50.0));
    let s = add_wire(&mut sheet, (45.0, 40.0), (45.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    let mut e = Engine::new(sheet).unwrap();
    let before = net_shape(e.document());
    mv(
        &mut e,
        vec![symbol_item(r1), SelectedItem::new(s, SelectedKind::Wire)],
        0.0,
        10.0,
        true,
    );
    assert_no_overlaps(&e);
    let _ = before;
}

#[test]
fn a_four_way_dot_moves_with_the_tip() {
    let mut sheet = test_sheet();
    let tp = add_resistor(&mut sheet, "TP", Point::new(55.0, 50.0)); // pin1 at (50,50)
    for (a, b) in [
        ((30.0, 50.0), (50.0, 50.0)),
        ((50.0, 50.0), (70.0, 50.0)),
        ((50.0, 30.0), (50.0, 50.0)),
        ((50.0, 50.0), (50.0, 70.0)),
    ] {
        add_wire(&mut sheet, a, b);
    }
    add_dot(&mut sheet, (50.0, 50.0), true);
    let before = net_shape(&sheet);
    let mut e = Engine::new(sheet).unwrap();
    mv(&mut e, vec![symbol_item(tp)], 5.0, 5.0, true);
    assert_eq!(net_shape(e.document()), before, "T11 netlist changed");
    assert_no_overlaps(&e);
    assert!(
        e.document().junctions.len() <= 2,
        "no dot beyond the old meeting and one new T: {}",
        describe(e.document())
    );
}

#[test]
fn a_no_connect_on_a_moved_pin_tip_moves_with_it() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0)); // pins 45, 55
    let mark = Uuid::new_v4();
    sheet.no_connects.push(signex_types::schematic::NoConnect {
        uuid: mark,
        position: Point::new(55.0, 50.0),
    });
    let mut engine = Engine::new(sheet).expect("engine");
    mv(&mut engine, vec![symbol_item(r1)], 0.0, 10.0, true);
    let moved = engine
        .document()
        .no_connects
        .iter()
        .find(|n| n.uuid == mark)
        .expect("mark kept");
    assert_eq!(moved.position, Point::new(55.0, 60.0));
}

/// Rubber-banding a symbol on a big sheet stays cheap, guard included.
#[test]
fn the_guard_is_cheap_on_a_500_symbol_sheet() {
    let mut sheet = test_sheet();
    let mut first = None;
    for row in 0..20 {
        for col in 0..25 {
            let id = add_resistor(
                &mut sheet,
                &format!("R{}", row * 25 + col),
                Point::new(20.0 + 20.0 * col as f64, 20.0 + 12.0 * row as f64),
            );
            first = first.or(Some(id));
            let y = 20.0 + 12.0 * row as f64;
            let x = 20.0 + 20.0 * col as f64;
            // Wire pin 2 to the next symbol's pin 1 along the row.
            if col < 24 {
                add_wire(&mut sheet, (x + 5.0, y), (x + 15.0, y));
            }
        }
    }
    let victim = sheet.symbols[12 * 25 / 2].uuid;
    let mut engine = Engine::new(sheet).expect("engine");
    let started = std::time::Instant::now();
    mv(&mut engine, vec![symbol_item(victim)], 2.5, 6.0, true);
    let elapsed = started.elapsed();
    eprintln!("500-symbol rubber-band move incl. guard: {elapsed:?}");
    assert!(engine.take_warnings().is_empty());
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "took {elapsed:?}"
    );
}

// ---- the user's own co-selected edits are not the pass's failures ----

fn pair_with_far_net_label() -> (SchematicSheet, Uuid, Uuid) {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_resistor(&mut sheet, "R2", Point::new(80.0, 50.0));
    add_wire(&mut sheet, (85.0, 50.0), (100.0, 50.0));
    add_label(&mut sheet, "A", (100.0, 50.0));
    let label = sheet.labels[0].uuid;
    (sheet, r1, label)
}

fn assert_bent_without_warning(e: &mut Engine) {
    let warnings = e.take_warnings();
    assert!(warnings.is_empty(), "false warning: {warnings:?}");
    let segs = segments(e);
    assert!(has(&segs, (30.0, 50.0), (45.0, 50.0)), "{segs:?}");
    assert!(has(&segs, (45.0, 50.0), (45.0, 60.0)), "{segs:?}");
}

#[test]
fn a_co_selected_label_leaving_its_wire_does_not_make_the_symbols_wire_stay_behind() {
    let (sheet, r1, label) = pair_with_far_net_label();
    let mut e = Engine::new(sheet).unwrap();
    mv(
        &mut e,
        vec![
            symbol_item(r1),
            SelectedItem::new(label, SelectedKind::Label),
        ],
        0.0,
        10.0,
        true,
    );
    assert_bent_without_warning(&mut e);
}

#[test]
fn a_co_selected_junction_leaving_its_trunk_is_the_users_edit() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_resistor(&mut sheet, "R2", Point::new(95.0, 40.0)); // pins 90, 100
    add_wire(&mut sheet, (100.0, 40.0), (120.0, 40.0));
    add_wire(&mut sheet, (110.0, 40.0), (110.0, 60.0));
    add_resistor(&mut sheet, "R3", Point::new(115.0, 60.0)); // pins 110, 120
    add_dot(&mut sheet, (110.0, 40.0), false);
    let dot = sheet.junctions[0].uuid;
    let mut e = Engine::new(sheet).unwrap();
    mv(
        &mut e,
        vec![
            symbol_item(r1),
            SelectedItem::new(dot, SelectedKind::Junction),
        ],
        0.0,
        10.0,
        true,
    );
    assert_bent_without_warning(&mut e);
}

#[test]
fn a_co_selected_wire_leaving_an_unselected_wire_is_the_users_edit() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_resistor(&mut sheet, "R2", Point::new(95.0, 50.0)); // pins 90, 100
    add_wire(&mut sheet, (100.0, 50.0), (110.0, 50.0));
    add_resistor(&mut sheet, "R3", Point::new(115.0, 70.0)); // pins 110, 120
    let leaving = add_wire(&mut sheet, (110.0, 50.0), (110.0, 70.0));
    let mut e = Engine::new(sheet).unwrap();
    mv(
        &mut e,
        vec![
            symbol_item(r1),
            SelectedItem::new(leaving, SelectedKind::Wire),
        ],
        0.0,
        10.0,
        true,
    );
    assert_bent_without_warning(&mut e);
}

#[test]
fn a_co_selected_no_connect_moves_without_blaming_the_pass() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    let mark = Uuid::new_v4();
    sheet.no_connects.push(signex_types::schematic::NoConnect {
        uuid: mark,
        position: Point::new(200.0, 200.0),
    });
    let mut e = Engine::new(sheet).unwrap();
    mv(
        &mut e,
        vec![
            symbol_item(r1),
            SelectedItem::new(mark, SelectedKind::NoConnect),
        ],
        0.0,
        10.0,
        true,
    );
    assert_bent_without_warning(&mut e);
}

#[test]
fn a_bus_and_bus_entry_on_the_new_path_are_harmless() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    sheet.buses.push(signex_types::schematic::Bus {
        uuid: Uuid::new_v4(),
        start: Point::new(45.0, 52.0),
        end: Point::new(45.0, 58.0),
    });
    sheet.bus_entries.push(signex_types::schematic::BusEntry {
        uuid: Uuid::new_v4(),
        position: Point::new(45.0, 55.0),
        size: (2.54, 2.54),
    });
    let mut e = Engine::new(sheet).unwrap();
    let before = net_shape(e.document());
    mv(&mut e, vec![symbol_item(r1)], 0.0, 10.0, true);
    assert_clean(&mut e, &before);
}

/// Review case 5625: R6's own wire moves with it and lands as a T on R2's old
/// wire end. The plain move keeps R6.1 on net "B"; the pass must too, and may
/// not leave R6.1 alone because R2's end followed R2 away.
#[test]
fn a_pin_whose_wire_moves_with_it_stays_on_the_net_the_wire_lands_on() {
    let mut sheet = test_sheet();
    let r6 = add_resistor(&mut sheet, "R6", Point::new(50.0, 20.0)); // pins 45, 55
    let r2 = add_resistor(&mut sheet, "R2", Point::new(25.0, 30.0)); // pins 20, 30
    let carried = add_wire(&mut sheet, (45.0, 20.0), (25.0, 20.0));
    add_wire(&mut sheet, (20.0, 30.0), (20.0, 40.0));
    add_label(&mut sheet, "B", (20.0, 40.0));
    let moves = vec![
        (symbol_item(r6), -7.5, 10.0),
        (symbol_item(r2), -7.5, 10.0),
        (SelectedItem::new(carried, SelectedKind::Wire), -7.5, 10.0),
    ];
    let run = |keep_wires: bool| {
        let mut e = Engine::new(sheet.clone()).unwrap();
        e.execute(Command::MoveEach {
            moves: moves.clone(),
            keep_wires,
        })
        .unwrap();
        e
    };
    let plain = run(false);
    let mut e = run(true);
    let unkept = e.take_unkept_pins();
    let r6_on_b = |sheet: &SchematicSheet| {
        signex_net::net_signatures(sheet)
            .iter()
            .any(|n| n.name == "B" && n.terminals.contains(&(r6, "1".to_string())))
    };
    assert!(
        r6_on_b(plain.document()),
        "fixture: the plain move keeps R6.1 on B"
    );
    assert!(
        r6_on_b(e.document()),
        "R6.1 was parted from B (unkept {unkept:?}): {}",
        describe(e.document())
    );
    let warned = warned_items(&sheet, &unkept);
    let tips = crate::moved_pin_tips(
        &sheet,
        &moves.iter().map(|(item, _, _)| *item).collect::<Vec<_>>(),
    );
    check_closure_oracle(
        &sheet,
        plain.document(),
        e.document(),
        &warned,
        &moved_pin_items(&tips),
    )
    .unwrap();
}

/// #1177: R1 pin 1 is on net A, and a +10 y move lands it on net C. A result
/// that keeps the old wire on the pin and also touches C joins A and C, which
/// neither the pre-move nor the plain-moved sheet does; the oracle must catch
/// that on its own, not only the guard.
#[test]
fn the_oracle_reports_a_kept_pin_shorted_to_its_plain_landing_net() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0)); // pins 45, 55
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_label(&mut sheet, "A", (30.0, 50.0));
    add_wire(&mut sheet, (45.0, 60.0), (40.0, 60.0));
    add_label(&mut sheet, "C", (40.0, 60.0));
    let items = vec![symbol_item(r1)];
    let mut plain = Engine::new(sheet.clone()).unwrap();
    mv(&mut plain, items.clone(), 0.0, 10.0, false);
    let mut shorted = plain.document().clone();
    add_wire(&mut shorted, (45.0, 50.0), (45.0, 60.0));
    let tips = crate::moved_pin_tips(&sheet, &items);
    let verdict = check_closure_oracle(
        &sheet,
        plain.document(),
        &shorted,
        &std::collections::BTreeSet::new(),
        &moved_pin_items(&tips),
    );
    assert!(
        matches!(&verdict, Err(why) if why.starts_with("SHORT")),
        "{verdict:?}"
    );
}

/// One notice for the lot, not a line per pin.
#[test]
fn many_pins_that_cannot_be_kept_make_one_notice() {
    let mut sheet = test_sheet();
    let mut moved = Vec::new();
    for n in 0..5 {
        let y = 20.0 + 20.0 * f64::from(n);
        moved.push(add_resistor(
            &mut sheet,
            &format!("R{}", n + 1),
            Point::new(50.0, y),
        ));
        add_wire(&mut sheet, (30.0, y), (45.0, y));
        // A named foreign wire ends where pin 1 lands after a move of +10 in y.
        add_wire(&mut sheet, (45.0, y + 10.0), (40.0, y + 10.0));
        add_label(&mut sheet, &format!("F{n}"), (40.0, y + 10.0));
    }
    let mut e = Engine::new(sheet).unwrap();
    mv(
        &mut e,
        moved.iter().map(|id| symbol_item(*id)).collect(),
        0.0,
        10.0,
        true,
    );
    let notices = e.take_warnings();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(
        notices[0].contains("could not keep 5 pins") && notices[0].contains("and 2 more"),
        "{notices:?}"
    );
    assert_eq!(e.take_unkept_pins().len(), 5);
}
