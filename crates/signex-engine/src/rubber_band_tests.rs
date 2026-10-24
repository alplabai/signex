//! Rubber-band move (#963): wires follow the pins of moved symbols.
//!
//! Fixture: resistor `R1` at (50, 50) with pin 1 at (45, 50) and pin 2 at
//! (55, 50); a wire into each pin from a fixed far end.

use crate::test_support::test_sheet;
use crate::{Command, Engine};
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, SelectedItem,
    SelectedKind, Symbol, Wire,
};
use uuid::Uuid;

fn lib_pin(number: &str, x: f64) -> LibPin {
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
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

fn add_resistor(sheet: &mut SchematicSheet, reference: &str, at: Point) -> Uuid {
    sheet
        .lib_symbols
        .entry("R".to_string())
        .or_insert_with(|| LibSymbol {
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
            pins: vec![lib_pin("1", -5.0), lib_pin("2", 5.0)],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        });
    let uuid = Uuid::new_v4();
    sheet.symbols.push(Symbol {
        uuid,
        lib_id: "R".to_string(),
        reference: reference.to_string(),
        position: at,
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

fn symbol_item(uuid: Uuid) -> SelectedItem {
    SelectedItem::new(uuid, SelectedKind::Symbol)
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

type Segment = ((f64, f64), (f64, f64));

fn segments(engine: &Engine) -> Vec<Segment> {
    engine
        .document()
        .wires
        .iter()
        .map(|w| ((w.start.x, w.start.y), (w.end.x, w.end.y)))
        .collect()
}

/// Net identity independent of ids: the sorted terminal sets of each net.
fn net_shape(sheet: &SchematicSheet) -> Vec<Vec<(String, String)>> {
    let mut nets: Vec<Vec<(String, String)>> = signex_net::build_netlist(sheet)
        .nets
        .iter()
        .map(|net| {
            let mut t: Vec<_> = net
                .terminals
                .iter()
                .map(|t| (t.reference.clone(), t.pin.clone()))
                .collect();
            t.sort();
            t
        })
        .collect();
    nets.sort();
    nets
}

/// Two resistors in series, R1 pin 2 wired to R2 pin 1 and R1 pin 1 to a
/// far-left stub; R2 is far enough away that R1 can move freely.
fn series_fixture() -> (Engine, Uuid, Uuid) {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    let r2 = add_resistor(&mut sheet, "R2", Point::new(80.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (55.0, 50.0), (75.0, 50.0));
    add_wire(&mut sheet, (85.0, 50.0), (100.0, 50.0));
    (Engine::new(sheet).expect("engine"), r1, r2)
}

fn orthogonal(seg: &Segment) -> bool {
    seg.0.0 == seg.1.0 || seg.0.1 == seg.1.1
}

#[test]
fn one_end_on_a_pin_follows_it_along_the_wire() {
    let (mut engine, r1, _) = series_fixture();
    mv(&mut engine, vec![symbol_item(r1)], -5.0, 0.0, true);
    let segs = segments(&engine);
    assert_eq!(segs.len(), 3, "a collinear follow adds no corner: {segs:?}");
    assert_eq!(segs[0], ((30.0, 50.0), (40.0, 50.0)));
    assert_eq!(segs[1], ((50.0, 50.0), (75.0, 50.0)));
    assert_eq!(segs[2], ((85.0, 50.0), (100.0, 50.0)), "far wire untouched");
}

#[test]
fn off_axis_move_inserts_an_orthogonal_bend_keeping_the_fixed_end() {
    let (mut engine, r1, _) = series_fixture();
    mv(&mut engine, vec![symbol_item(r1)], 0.0, 10.0, true);
    let segs = segments(&engine);
    assert_eq!(segs.len(), 5, "one corner per attached wire: {segs:?}");
    assert!(segs.iter().all(orthogonal), "diagonal wire: {segs:?}");
    // Horizontal wires keep the fixed end's row; the tail turns at the
    // pin's new column and runs to the pin's new tip.
    assert_eq!(segs[0], ((30.0, 50.0), (45.0, 50.0)));
    assert_eq!(segs[3], ((45.0, 50.0), (45.0, 60.0)));
    assert_eq!(segs[1], ((55.0, 50.0), (75.0, 50.0)));
    assert_eq!(segs[4], ((55.0, 60.0), (55.0, 50.0)));
}

#[test]
fn vertical_wire_keeps_its_column_and_turns_at_the_pins_new_row() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (45.0, 30.0), (45.0, 50.0));
    let mut engine = Engine::new(sheet).expect("engine");
    mv(&mut engine, vec![symbol_item(r1)], 10.0, 5.0, true);
    let segs = segments(&engine);
    assert_eq!(
        segs,
        vec![((45.0, 30.0), (45.0, 55.0)), ((45.0, 55.0), (55.0, 55.0)),]
    );
}

#[test]
fn wire_between_two_moved_symbols_translates_whole() {
    let (mut engine, r1, r2) = series_fixture();
    mv(
        &mut engine,
        vec![symbol_item(r1), symbol_item(r2)],
        0.0,
        10.0,
        true,
    );
    let segs = segments(&engine);
    assert_eq!(segs[1], ((55.0, 60.0), (75.0, 60.0)));
    assert_eq!(segs.len(), 5, "only the two outer wires gain a corner");
    assert!(segs.iter().all(orthogonal), "{segs:?}");
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
    assert_eq!(segs[0], ((30.0, 60.0), (45.0, 60.0)));
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
    let before = segments(&engine);
    let ids: Vec<Uuid> = engine.document().wires.iter().map(|w| w.uuid).collect();
    mv(&mut engine, vec![symbol_item(r1)], 7.0, 10.0, true);
    assert_ne!(segments(&engine), before);

    engine.undo().expect("undo");
    assert_eq!(segments(&engine), before);
    let after_ids: Vec<Uuid> = engine.document().wires.iter().map(|w| w.uuid).collect();
    assert_eq!(after_ids, ids);
    assert_eq!(
        engine.document().symbols[0].position,
        Point::new(50.0, 50.0)
    );

    engine.redo().expect("redo");
    assert_eq!(engine.document().wires.len(), 5);
}

#[test]
fn the_netlist_is_unchanged_by_a_rubber_band_move() {
    let (mut engine, r1, r2) = series_fixture();
    let before = net_shape(engine.document());
    assert_eq!(before.len(), 3, "fixture has three nets: {before:?}");
    mv(&mut engine, vec![symbol_item(r1)], 7.5, -12.0, true);
    assert_eq!(net_shape(engine.document()), before);
    mv(
        &mut engine,
        vec![symbol_item(r1), symbol_item(r2)],
        -3.0,
        4.0,
        true,
    );
    assert_eq!(net_shape(engine.document()), before);
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
