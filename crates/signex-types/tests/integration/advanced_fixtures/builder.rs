//! The pieces the Advanced Schematic fixtures are built from: ids that are
//! the same on every run, and parts whose pin `i` tip is at
//! `(o.x, o.y + 2.54 i)`, as in signex-net's harness fixture.

use signex_types::format::SnxSchematic;
use signex_types::project::{ProjectData, encode_project};
use signex_types::schematic::{
    Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet,
    Symbol, Wire,
};
use uuid::Uuid;

pub(super) const PITCH: f64 = 2.54;

pub(super) fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Ids for one fixture: `base` names the fixture, a counter the object, so
/// a rebuild writes the same file.
pub(super) struct Ids {
    base: u128,
    next: u128,
}

impl Ids {
    pub(super) fn new(base: u16) -> Self {
        Self {
            base: u128::from(base) << 64,
            next: 0,
        }
    }

    pub(super) fn next(&mut self) -> Uuid {
        self.next += 1;
        Uuid::from_u128(self.base | self.next)
    }
}

/// An A4 sheet with nothing on it.
pub(super) fn empty_sheet(ids: &mut Ids) -> SchematicSheet {
    SchematicSheet {
        uuid: ids.next(),
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

/// The library part `lib_id` with `pins` passive pins numbered 1.., pin `i`
/// at local `(0, -2.54 i)`, embedded under its versioned body ref as a
/// load would store it; returns that ref.
fn add_lib(sheet: &mut SchematicSheet, lib_id: &str, pins: usize) -> String {
    let pins = (1..=pins)
        .map(|i| LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: pt(0.0, -PITCH * i as f64),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: i.to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        })
        .collect();
    sheet
        .embed_body(LibSymbol {
            id: lib_id.to_string(),
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
        })
        .expect("one body per library part")
}

/// What a placed part is: its designator, value, footprint, library part
/// and pin count.
pub(super) struct PartSpec<'a> {
    pub reference: &'a str,
    pub value: &'a str,
    pub footprint: &'a str,
    pub lib_id: &'a str,
    pub pins: usize,
}

/// Place `spec` at `o`; returns the placed symbol's index and pin `i`'s tip
/// for each `i` in `1..=pins`.
pub(super) fn place(
    sheet: &mut SchematicSheet,
    ids: &mut Ids,
    spec: &PartSpec<'_>,
    o: Point,
) -> (usize, Vec<Point>) {
    let body_ref = add_lib(sheet, spec.lib_id, spec.pins);
    sheet.symbols.push(Symbol {
        uuid: ids.next(),
        lib_id: spec.lib_id.to_string(),
        body_ref,
        reference: spec.reference.to_string(),
        value: spec.value.to_string(),
        footprint: spec.footprint.to_string(),
        position: o,
        ..Default::default()
    });
    let tips = (1..=spec.pins)
        .map(|i| pt(o.x, o.y + PITCH * i as f64))
        .collect();
    (sheet.symbols.len() - 1, tips)
}

pub(super) fn wire(ids: &mut Ids, a: Point, b: Point) -> Wire {
    Wire {
        uuid: ids.next(),
        start: a,
        end: b,
        ..Default::default()
    }
}

pub(super) fn label(ids: &mut Ids, text: &str, at: Point, kind: LabelType) -> Label {
    Label {
        uuid: ids.next(),
        ..Label::new(text, at, kind)
    }
}

/// A wire from `tip` to `tip + (dx, 0)` with a `kind` label `name` at its
/// far end.
pub(super) fn named_stub(
    sheet: &mut SchematicSheet,
    ids: &mut Ids,
    tip: Point,
    dx: f64,
    name: &str,
    kind: LabelType,
) {
    let end = pt(tip.x + dx, tip.y);
    sheet.wires.push(wire(ids, tip, end));
    sheet.labels.push(label(ids, name, end, kind));
}

/// The sheet as the writer saves it.
pub(super) fn sheet_bytes(sheet: SchematicSheet) -> Vec<u8> {
    SnxSchematic::new(sheet)
        .write_string()
        .expect("the fixture sheet writes")
        .into_bytes()
}

/// A project file from its JSON keys, as the writer saves it. `dir` is the
/// corpus placeholder, empty.
pub(super) fn project_bytes(json: serde_json::Value) -> Vec<u8> {
    let data: ProjectData = serde_json::from_value(json).expect("the fixture project reads");
    encode_project(&data).expect("the fixture project encodes")
}

/// A sheet list entry of a project file.
pub(super) fn sheet_entry(file: &str) -> serde_json::Value {
    serde_json::json!({
        "name": file,
        "filename": file,
        "symbols_count": 0,
        "wires_count": 0,
        "labels_count": 0,
    })
}
