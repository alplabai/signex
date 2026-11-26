//! The signal harness fixture project (#1280, mockup row 152); its sheets:
//! `docs/SIGNAL_HARNESSES.md`, "Test fixture".

use std::collections::HashMap;

use signex_types::schematic::{
    ChildSheet, FillType, HarnessConnector, HarnessEntry, HarnessWire, Label, LabelType, LibPin,
    LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, SheetPin, Symbol, Wire,
};
use uuid::Uuid;

use crate::{ProjectGraph, ProjectNetlist, ProjectRoot, SheetKey, build_project_netlist};

pub(crate) const PITCH: f64 = 2.54;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A fresh id for every object of the fixture.
pub(crate) fn id() -> Uuid {
    Uuid::now_v7()
}

/// A library part with `pins` pins numbered 1.., pin `i` at local
/// `(0, -2.54 i)`: placed at `o` its tip is at `(o.x, o.y + 2.54 i)`.
fn add_part_lib(sheet: &mut SchematicSheet, lib_id: &str, pins: usize) {
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
    sheet.lib_symbols.insert(
        lib_id.to_string(),
        LibSymbol {
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
        },
    );
}

/// Place `reference` at `o`; returns pin `i`'s tip for each `i` in `1..=n`.
pub(crate) fn place_part(
    sheet: &mut SchematicSheet,
    reference: &str,
    pins: usize,
    o: Point,
) -> Vec<Point> {
    let lib_id = format!("fixture:{reference}");
    add_part_lib(sheet, &lib_id, pins);
    sheet.symbols.push(Symbol {
        uuid: id(),
        lib_id,
        reference: reference.to_string(),
        position: o,
        ..Default::default()
    });
    (1..=pins)
        .map(|i| pt(o.x, o.y + PITCH * i as f64))
        .collect()
}

pub(crate) fn connector(harness_type: &str, at: Point, names: &[&str]) -> HarnessConnector {
    HarnessConnector {
        uuid: id(),
        harness_type: harness_type.to_string(),
        position: at,
        width: 10.16,
        entry_pitch: PITCH,
        rotation: 0.0,
        entries: names
            .iter()
            .map(|n| HarnessEntry {
                uuid: id(),
                name: (*n).to_string(),
            })
            .collect(),
        locked: false,
    }
}

pub(crate) fn wire(a: Point, b: Point) -> Wire {
    Wire {
        uuid: id(),
        start: a,
        end: b,
        ..Default::default()
    }
}

pub(crate) fn harness_wire(a: Point, b: Point) -> HarnessWire {
    HarnessWire {
        uuid: id(),
        start: a,
        end: b,
        locked: false,
    }
}

/// Wire each of `tips` to the entry of the same index on `c`.
pub(crate) fn wire_entries(sheet: &mut SchematicSheet, c: &HarnessConnector, tips: &[Point]) {
    for (i, tip) in tips.iter().enumerate() {
        sheet.wires.push(wire(*tip, c.entry_point(i)));
    }
}

/// A sheet symbol `name` with one harness-typed entry `entry` of type `ty`
/// at `at`.
pub(crate) fn typed_sheet_symbol(
    sheet: &mut SchematicSheet,
    name: &str,
    file: &str,
    entry: &str,
    ty: &str,
    at: Point,
) {
    let pin = SheetPin {
        uuid: id(),
        name: entry.to_string(),
        direction: String::new(),
        position: at,
        rotation: 0.0,
        auto_generated: false,
        user_moved: false,
    };
    sheet.harness.entry_types.insert(pin.uuid, ty.to_string());
    sheet.child_sheets.push(ChildSheet {
        uuid: id(),
        name: name.to_string(),
        filename: file.to_string(),
        position: pt(at.x, at.y - 5.0),
        size: (20.0, 20.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![pin],
        instances: Vec::new(),
    });
}

pub(crate) fn port(name: &str, at: Point) -> Label {
    Label {
        uuid: id(),
        ..Label::new(name, at, LabelType::Hierarchical)
    }
}

fn empty() -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

/// The root sheet `mcu.snxsch`.
pub(crate) fn mcu() -> SchematicSheet {
    let mut s = empty();
    let u3 = place_part(&mut s, "U3", 8, pt(10.0, 10.0));
    // SPI: U3 pins 1..4 into the SPI_BUS connector, harness to `sensor`.
    let spi = connector("SPI_BUS", pt(40.0, 10.0), &["SCK", "MOSI", "MISO", "CS"]);
    wire_entries(&mut s, &spi, &u3[0..4]);
    let to_sensor = pt(80.0, 20.0);
    s.harness
        .wires
        .push(harness_wire(spi.tip_point(), to_sensor));
    typed_sheet_symbol(
        &mut s,
        "sensor",
        "sensor.snxsch",
        "SPI_BUS",
        "SPI_BUS",
        to_sensor,
    );
    // DEBUG: SWD on pins 5, 6; UART (pins 7, 8) nested at its UART entry.
    let uart = connector("UART", pt(30.0, 60.0), &["TX", "RX"]);
    wire_entries(&mut s, &uart, &u3[6..8]);
    let debug = connector("DEBUG", pt(50.0, 40.0), &["SWDIO", "SWCLK", "UART"]);
    wire_entries(&mut s, &debug, &u3[4..6]);
    s.harness
        .wires
        .push(harness_wire(uart.tip_point(), debug.entry_point(2)));
    let to_connectors = pt(80.0, 60.0);
    s.harness
        .wires
        .push(harness_wire(debug.tip_point(), to_connectors));
    typed_sheet_symbol(
        &mut s,
        "connectors",
        "connectors.snxsch",
        "DEBUG",
        "DEBUG",
        to_connectors,
    );
    s.harness.connectors.extend([spi, uart, debug]);
    s
}

/// The child `sensor.snxsch`.
pub(crate) fn sensor() -> SchematicSheet {
    let mut s = empty();
    let u1 = place_part(&mut s, "U1", 4, pt(10.0, 10.0));
    let spi = connector("SPI_BUS", pt(30.0, 10.0), &["SCK", "MOSI", "MISO", "CS"]);
    wire_entries(&mut s, &spi, &u1);
    let port_at = pt(70.0, 20.0);
    s.harness.wires.push(harness_wire(spi.tip_point(), port_at));
    s.labels.push(port("SPI_BUS", port_at));
    s.harness.connectors.push(spi);
    s
}

/// The child `connectors.snxsch`.
pub(crate) fn connectors() -> SchematicSheet {
    let mut s = empty();
    let j2 = place_part(&mut s, "J2", 4, pt(10.0, 10.0));
    let debug = connector("DEBUG", pt(40.0, 10.0), &["SWDIO", "SWCLK", "UART"]);
    wire_entries(&mut s, &debug, &j2[0..2]);
    let uart = connector("UART", pt(20.0, 40.0), &["TX", "RX"]);
    wire_entries(&mut s, &uart, &j2[2..4]);
    s.harness
        .wires
        .push(harness_wire(uart.tip_point(), debug.entry_point(2)));
    let port_at = pt(80.0, 20.0);
    s.harness
        .wires
        .push(harness_wire(debug.tip_point(), port_at));
    s.labels.push(port("DEBUG", port_at));
    s.harness.connectors.extend([debug, uart]);
    s
}

/// A child that only passes the `DEBUG` harness on (#2657): a port `DEBUG`
/// on a harness wire to the typed entry `DEBUG` of the sheet symbol `name`
/// (`file`), no connector.
pub(crate) fn bridge(name: &str, file: &str) -> SchematicSheet {
    let mut s = empty();
    let port_at = pt(10.0, 20.0);
    let to_child = pt(60.0, 20.0);
    s.harness.wires.push(harness_wire(port_at, to_child));
    s.labels.push(port("DEBUG", port_at));
    typed_sheet_symbol(&mut s, name, file, "DEBUG", "DEBUG", to_child);
    s
}

/// [`mcu`] with its `DEBUG` harness going to `bridge.snxsch` instead of
/// straight to `connectors.snxsch`.
pub(crate) fn mcu_through_bridge() -> SchematicSheet {
    let mut s = mcu();
    if let Some(cs) = s.child_sheets.iter_mut().find(|cs| cs.name == "connectors") {
        cs.name = "bridge".to_string();
        cs.filename = "bridge.snxsch".to_string();
    }
    s
}

/// The carry fixture project, stitched: `mcu` -> `bridge` -> `connectors`
/// for the nested `DEBUG` harness, `mcu` -> `sensor` for `SPI_BUS`.
pub(crate) fn carry_fixture_netlist() -> ProjectNetlist {
    stitch(
        mcu_through_bridge(),
        vec![
            ("sensor.snxsch", sensor()),
            ("bridge.snxsch", bridge("connectors", "connectors.snxsch")),
            ("connectors.snxsch", connectors()),
        ],
    )
}

/// Stitch `root` with `children`, each child keyed by its own file name.
pub(crate) fn stitch(
    root: SchematicSheet,
    children: Vec<(&str, SchematicSheet)>,
) -> ProjectNetlist {
    let mut sheets: HashMap<SheetKey, SchematicSheet> = children
        .into_iter()
        .map(|(k, v)| (SheetKey::new(k), v))
        .collect();
    sheets.insert(SheetKey::new("mcu.snxsch"), root);
    let resolved: HashMap<SheetKey, HashMap<String, SheetKey>> = sheets
        .iter()
        .map(|(key, sheet)| {
            let submap = sheet
                .child_sheets
                .iter()
                .map(|cs| (cs.filename.clone(), SheetKey::new(cs.filename.clone())))
                .collect();
            (key.clone(), submap)
        })
        .collect();
    let roots = [ProjectRoot {
        key: SheetKey::new("mcu.snxsch"),
    }];
    build_project_netlist(&ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    })
}

/// The whole fixture project, stitched.
pub(crate) fn fixture_netlist() -> ProjectNetlist {
    stitch(
        mcu(),
        vec![
            ("sensor.snxsch", sensor()),
            ("connectors.snxsch", connectors()),
        ],
    )
}

/// A netlist as golden text: one line per net, sorted by name, `name` then
/// its terminals as `reference.pin`.
pub(crate) fn golden_text(netlist: &signex_types::net::Netlist) -> String {
    let mut lines: Vec<String> = netlist
        .nets
        .iter()
        .map(|n| {
            let pins: Vec<String> = n
                .terminals
                .iter()
                .map(|t| format!("{}.{}", t.reference, t.pin))
                .collect();
            format!("{}\t{}", n.name, pins.join(" "))
        })
        .collect();
    lines.sort();
    let mut text = lines.join("\n");
    text.push('\n');
    text
}
