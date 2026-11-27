//! `four-channel-variants`: a 4-channel design with two variants (exit
//! criterion 1). `top.snxsch` holds U1 and places `channel.snxsch` as
//! `Repeat(CH, 1, 4)`; each channel holds R1, C1 and C2. The variants:
//!
//! - `Full`: every part fitted; CH1's R1 is 4k7 instead of 10k (a per-channel
//!   value in the project's variant settings).
//! - `Lite`: C2 is not fitted on any channel (its `Fitted@Lite` field on the
//!   sheet), and R1 is not fitted on CH3 and CH4 (per-channel entries in the
//!   project's variant settings).

use signex_types::schematic::{ChildSheet, FillType, LabelType, SchematicSheet};

use super::builder::{
    Ids, PartSpec, empty_sheet, label, named_stub, place, project_bytes, pt, sheet_bytes,
    sheet_entry, wire,
};

pub(super) const DIR: &str = "four-channel-variants";

pub(super) fn files() -> Vec<(String, Vec<u8>)> {
    let mut ids = Ids::new(0xA5C4);
    vec![
        (format!("{DIR}/top.snxsch"), sheet_bytes(top(&mut ids))),
        (
            format!("{DIR}/channel.snxsch"),
            sheet_bytes(channel(&mut ids)),
        ),
        (format!("{DIR}/{DIR}.snxprj"), project()),
    ]
}

/// U1 with VIN and GND on global labels, and the four channels.
fn top(ids: &mut Ids) -> SchematicSheet {
    let mut s = empty_sheet(ids);
    let u1 = PartSpec {
        reference: "U1",
        value: "MCU",
        footprint: "Fixture:QFN-16",
        lib_id: "Fixture:U4",
        pins: 4,
    };
    let (_, tips) = place(&mut s, ids, &u1, pt(20.0, 20.0));
    named_stub(&mut s, ids, tips[0], 20.0, "VIN", LabelType::Global);
    named_stub(&mut s, ids, tips[1], 20.0, "GND", LabelType::Global);
    s.child_sheets.push(ChildSheet {
        uuid: ids.next(),
        name: "Repeat(CH, 1, 4)".to_string(),
        filename: "channel.snxsch".to_string(),
        position: pt(80.0, 20.0),
        size: (30.0, 20.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    });
    s
}

/// One channel: R1 from VIN to OUT, C1 and C2 from OUT to GND.
fn channel(ids: &mut Ids) -> SchematicSheet {
    let mut s = empty_sheet(ids);
    let r1 = PartSpec {
        reference: "R1",
        value: "10k",
        footprint: "Fixture:R_0603",
        lib_id: "Fixture:R2",
        pins: 2,
    };
    let c1 = PartSpec {
        reference: "C1",
        value: "100n",
        footprint: "Fixture:C_0603",
        lib_id: "Fixture:C2",
        pins: 2,
    };
    let c2 = PartSpec {
        reference: "C2",
        value: "1u",
        footprint: "Fixture:C_0805",
        lib_id: "Fixture:C2",
        pins: 2,
    };
    let (_, r) = place(&mut s, ids, &r1, pt(20.0, 20.0));
    let (_, a) = place(&mut s, ids, &c1, pt(40.0, 20.0));
    let (c2_index, b) = place(&mut s, ids, &c2, pt(60.0, 20.0));
    s.symbols[c2_index]
        .fields
        .insert("Fitted@Lite".to_string(), "0".to_string());

    named_stub(&mut s, ids, r[0], -10.0, "VIN", LabelType::Global);
    // OUT: R1.2 up to the C1.1 / C2.1 rail.
    let bend = pt(30.0, r[1].y);
    let rail = pt(30.0, a[0].y);
    s.wires.push(wire(ids, r[1], bend));
    s.wires.push(wire(ids, bend, rail));
    s.wires.push(wire(ids, rail, a[0]));
    s.wires.push(wire(ids, a[0], b[0]));
    s.labels.push(label(ids, "OUT", rail, LabelType::Net));
    // GND: C1.2 and C2.2.
    s.wires.push(wire(ids, a[1], b[1]));
    s.labels.push(label(ids, "GND", a[1], LabelType::Global));
    s
}

fn project() -> Vec<u8> {
    project_bytes(serde_json::json!({
        "name": DIR,
        "dir": "",
        "schematic_root": "top.snxsch",
        "pcb_file": null,
        "sheets": [sheet_entry("top.snxsch"), sheet_entry("channel.snxsch")],
        "variant_definitions": ["Full", "Lite"],
        "variant_settings": {
            "parts": [
                {"variant": "Full", "designator": "CH1_R1", "parameters": {"Value": "4k7"}},
                {"variant": "Lite", "designator": "CH3_R1", "fitted": false},
                {"variant": "Lite", "designator": "CH4_R1", "fitted": false},
            ]
        },
    }))
}
