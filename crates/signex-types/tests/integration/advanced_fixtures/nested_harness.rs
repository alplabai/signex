//! `nested-harness` and `nested-harness-carry` (exit criterion 2): the
//! signal harness project of `docs/SIGNAL_HARNESSES.md`, "Test fixture", as
//! files. `mcu.snxsch` runs `SPI_BUS` to `sensor.snxsch` and `DEBUG`, with
//! `UART` nested in its `UART` entry, to `connectors.snxsch`. The carry
//! project routes `DEBUG` through `bridge.snxsch`, a sheet with no connector.
//! Both are drawn as signex-net's in-memory harness fixture is, so their
//! netlists are its golden netlist.

use signex_types::schematic::{
    ChildSheet, FillType, HarnessConnector, HarnessEntry, HarnessWire, LabelType, Point,
    SchematicSheet, SheetPin,
};

use super::builder::{
    Ids, PartSpec, empty_sheet, label, place, project_bytes, pt, sheet_bytes, sheet_entry, wire,
};

pub(super) const DIR: &str = "nested-harness";
pub(super) const CARRY_DIR: &str = "nested-harness-carry";

pub(super) fn files() -> Vec<(String, Vec<u8>)> {
    let mut ids = Ids::new(0xA5B1);
    vec![
        (
            format!("{DIR}/mcu.snxsch"),
            sheet_bytes(mcu(&mut ids, "connectors", "connectors.snxsch")),
        ),
        (
            format!("{DIR}/sensor.snxsch"),
            sheet_bytes(sensor(&mut ids)),
        ),
        (
            format!("{DIR}/connectors.snxsch"),
            sheet_bytes(connectors(&mut ids)),
        ),
        (
            format!("{DIR}/{DIR}.snxprj"),
            project(DIR, &["mcu.snxsch", "sensor.snxsch", "connectors.snxsch"]),
        ),
    ]
}

pub(super) fn carry_files() -> Vec<(String, Vec<u8>)> {
    let mut ids = Ids::new(0xA5B2);
    let d = CARRY_DIR;
    vec![
        (
            format!("{d}/mcu.snxsch"),
            sheet_bytes(mcu(&mut ids, "bridge", "bridge.snxsch")),
        ),
        (format!("{d}/sensor.snxsch"), sheet_bytes(sensor(&mut ids))),
        (format!("{d}/bridge.snxsch"), sheet_bytes(bridge(&mut ids))),
        (
            format!("{d}/connectors.snxsch"),
            sheet_bytes(connectors(&mut ids)),
        ),
        (
            format!("{d}/{d}.snxprj"),
            project(
                d,
                &[
                    "mcu.snxsch",
                    "sensor.snxsch",
                    "bridge.snxsch",
                    "connectors.snxsch",
                ],
            ),
        ),
    ]
}

fn connector(ids: &mut Ids, harness_type: &str, at: Point, names: &[&str]) -> HarnessConnector {
    HarnessConnector {
        uuid: ids.next(),
        harness_type: harness_type.to_string(),
        position: at,
        width: 10.16,
        entry_pitch: super::builder::PITCH,
        rotation: 0.0,
        entries: names
            .iter()
            .map(|n| HarnessEntry {
                uuid: ids.next(),
                name: (*n).to_string(),
            })
            .collect(),
        locked: false,
    }
}

fn harness_wire(ids: &mut Ids, a: Point, b: Point) -> HarnessWire {
    HarnessWire {
        uuid: ids.next(),
        start: a,
        end: b,
        locked: false,
    }
}

/// Wire each of `tips` to the entry of the same index on `c`.
fn wire_entries(s: &mut SchematicSheet, ids: &mut Ids, c: &HarnessConnector, tips: &[Point]) {
    for (i, tip) in tips.iter().enumerate() {
        s.wires.push(wire(ids, *tip, c.entry_point(i)));
    }
}

/// A sheet symbol `name` (`file`) with one entry `entry` of harness type
/// `ty` at `at`.
fn typed_sheet_symbol(
    s: &mut SchematicSheet,
    ids: &mut Ids,
    (name, file): (&str, &str),
    (entry, ty): (&str, &str),
    at: Point,
) {
    let pin = SheetPin {
        uuid: ids.next(),
        name: entry.to_string(),
        direction: String::new(),
        position: at,
        rotation: 0.0,
        auto_generated: false,
        user_moved: false,
    };
    s.harness.entry_types.insert(pin.uuid, ty.to_string());
    s.child_sheets.push(ChildSheet {
        uuid: ids.next(),
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

fn part(reference: &str, pins: usize) -> PartSpec<'_> {
    PartSpec {
        reference,
        value: reference,
        footprint: "Fixture:Header",
        lib_id: match pins {
            2 => "Fixture:P2",
            4 => "Fixture:P4",
            _ => "Fixture:P8",
        },
        pins,
    }
}

/// The root: U3, `SPI_BUS` to `sensor`, `DEBUG` (SWD, `UART` nested) to the
/// sheet symbol `child` (`file`).
fn mcu(ids: &mut Ids, child: &str, file: &str) -> SchematicSheet {
    let mut s = empty_sheet(ids);
    let (_, u3) = place(&mut s, ids, &part("U3", 8), pt(10.0, 10.0));
    let spi = connector(
        ids,
        "SPI_BUS",
        pt(40.0, 10.0),
        &["SCK", "MOSI", "MISO", "CS"],
    );
    wire_entries(&mut s, ids, &spi, &u3[0..4]);
    let to_sensor = pt(80.0, 20.0);
    s.harness
        .wires
        .push(harness_wire(ids, spi.tip_point(), to_sensor));
    typed_sheet_symbol(
        &mut s,
        ids,
        ("sensor", "sensor.snxsch"),
        ("SPI_BUS", "SPI_BUS"),
        to_sensor,
    );
    let uart = connector(ids, "UART", pt(30.0, 60.0), &["TX", "RX"]);
    wire_entries(&mut s, ids, &uart, &u3[6..8]);
    let debug = connector(ids, "DEBUG", pt(50.0, 40.0), &["SWDIO", "SWCLK", "UART"]);
    wire_entries(&mut s, ids, &debug, &u3[4..6]);
    s.harness
        .wires
        .push(harness_wire(ids, uart.tip_point(), debug.entry_point(2)));
    let to_child = pt(80.0, 60.0);
    s.harness
        .wires
        .push(harness_wire(ids, debug.tip_point(), to_child));
    typed_sheet_symbol(&mut s, ids, (child, file), ("DEBUG", "DEBUG"), to_child);
    s.harness.connectors.extend([spi, uart, debug]);
    s
}

fn sensor(ids: &mut Ids) -> SchematicSheet {
    let mut s = empty_sheet(ids);
    let (_, u1) = place(&mut s, ids, &part("U1", 4), pt(10.0, 10.0));
    let spi = connector(
        ids,
        "SPI_BUS",
        pt(30.0, 10.0),
        &["SCK", "MOSI", "MISO", "CS"],
    );
    wire_entries(&mut s, ids, &spi, &u1);
    let port_at = pt(70.0, 20.0);
    s.harness
        .wires
        .push(harness_wire(ids, spi.tip_point(), port_at));
    s.labels
        .push(label(ids, "SPI_BUS", port_at, LabelType::Hierarchical));
    s.harness.connectors.push(spi);
    s
}

fn connectors(ids: &mut Ids) -> SchematicSheet {
    let mut s = empty_sheet(ids);
    let (_, j2) = place(&mut s, ids, &part("J2", 4), pt(10.0, 10.0));
    let debug = connector(ids, "DEBUG", pt(40.0, 10.0), &["SWDIO", "SWCLK", "UART"]);
    wire_entries(&mut s, ids, &debug, &j2[0..2]);
    let uart = connector(ids, "UART", pt(20.0, 40.0), &["TX", "RX"]);
    wire_entries(&mut s, ids, &uart, &j2[2..4]);
    s.harness
        .wires
        .push(harness_wire(ids, uart.tip_point(), debug.entry_point(2)));
    let port_at = pt(80.0, 20.0);
    s.harness
        .wires
        .push(harness_wire(ids, debug.tip_point(), port_at));
    s.labels
        .push(label(ids, "DEBUG", port_at, LabelType::Hierarchical));
    s.harness.connectors.extend([debug, uart]);
    s
}

/// A sheet that only passes `DEBUG` on: a port `DEBUG` on a harness wire to
/// the typed entry `DEBUG` of the sheet symbol `connectors`.
fn bridge(ids: &mut Ids) -> SchematicSheet {
    let mut s = empty_sheet(ids);
    let port_at = pt(10.0, 20.0);
    let to_child = pt(60.0, 20.0);
    s.harness.wires.push(harness_wire(ids, port_at, to_child));
    s.labels
        .push(label(ids, "DEBUG", port_at, LabelType::Hierarchical));
    typed_sheet_symbol(
        &mut s,
        ids,
        ("connectors", "connectors.snxsch"),
        ("DEBUG", "DEBUG"),
        to_child,
    );
    s
}

/// The project file, with the three harness definitions (`UART` nested in
/// `DEBUG`).
fn project(name: &str, sheets: &[&str]) -> Vec<u8> {
    let entries = |names: &[&str]| -> Vec<serde_json::Value> {
        names
            .iter()
            .map(|n| serde_json::json!({"name": n}))
            .collect()
    };
    project_bytes(serde_json::json!({
        "name": name,
        "dir": "",
        "schematic_root": "mcu.snxsch",
        "pcb_file": null,
        "sheets": sheets.iter().copied().map(sheet_entry).collect::<Vec<_>>(),
        "schematic_design": {
            "harness_definitions": [
                {"name": "SPI_BUS", "entries": entries(&["SCK", "MOSI", "MISO", "CS"])},
                {"name": "UART", "entries": entries(&["TX", "RX"])},
                {"name": "DEBUG", "entries": [
                    {"name": "SWDIO"},
                    {"name": "SWCLK"},
                    {"name": "UART", "harness": "UART"},
                ]},
            ]
        },
    }))
}
