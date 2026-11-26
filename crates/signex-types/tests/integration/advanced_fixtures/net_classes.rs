//! `net-class-directives` (exit criterion 3): one sheet where U1 and J1 share
//! five named nets, a board with no parts yet, and the directives that give
//! the nets their classes:
//!
//! - `VIN`: a Net Blanket with net class `POWER` (and a track width);
//! - `CLK`: a Net Parameters directive with net class `HS`, a 0.2 mm width
//!   and a 0.15 mm clearance, which Update PCB turns into rules;
//! - `USB_P` / `USB_N`: a Net Parameters directive on `USB_P` naming the
//!   diff-pair class `USB`;
//! - `SIG`: no directive, no class.
//!
//! The project's net-class table lists `POWER` and `HS` with no member nets
//! and no patterns, so a class a net has came from a directive.

use signex_types::format::SnxPcb;
use signex_types::pcb::PcbBoard;
use signex_types::schematic::{BlanketPour, LabelType, NetRules, SchDirective, SchematicSheet};

use super::builder::{
    Ids, PartSpec, empty_sheet, label, place, project_bytes, pt, sheet_bytes, sheet_entry, wire,
};

pub(super) const DIR: &str = "net-class-directives";

/// The nets, by the pin pair (U1.i, J1.i) they join, pin 1 first.
pub(super) const NETS: [&str; 5] = ["VIN", "CLK", "USB_P", "USB_N", "SIG"];

pub(super) fn files() -> Vec<(String, Vec<u8>)> {
    let mut ids = Ids::new(0xA5C1);
    vec![
        (format!("{DIR}/main.snxsch"), sheet_bytes(main(&mut ids))),
        (format!("{DIR}/{DIR}.snxpcb"), board(&mut ids)),
        (format!("{DIR}/{DIR}.snxprj"), project()),
    ]
}

fn main(ids: &mut Ids) -> SchematicSheet {
    let mut s = empty_sheet(ids);
    let u1 = PartSpec {
        reference: "U1",
        value: "PHY",
        footprint: "Fixture:SOIC-6",
        lib_id: "Fixture:U6",
        pins: 6,
    };
    let j1 = PartSpec {
        reference: "J1",
        value: "Header",
        footprint: "Fixture:PinHeader_1x06",
        lib_id: "Fixture:J6",
        pins: 6,
    };
    let (_, u) = place(&mut s, ids, &u1, pt(10.0, 10.0));
    let (_, j) = place(&mut s, ids, &j1, pt(80.0, 10.0));
    for (i, name) in NETS.iter().enumerate() {
        s.wires.push(wire(ids, u[i], j[i]));
        s.labels
            .push(label(ids, name, pt(20.0, u[i].y), LabelType::Net));
    }
    // Around the VIN wire only: the CLK wire is 2.54 mm lower.
    s.directives.push(SchDirective::Blanket {
        uuid: ids.next(),
        start: pt(5.0, u[0].y - 1.0),
        end: pt(85.0, u[0].y + 1.0),
        net_class: Some("POWER".to_string()),
        erc_exclusions: Vec::new(),
        track_width: Some(0.5),
        pour: BlanketPour::Normal,
        include_edge_nets: false,
    });
    s.directives.push(SchDirective::NetParameters {
        uuid: ids.next(),
        position: pt(50.0, u[1].y),
        net_class: Some("HS".to_string()),
        diff_pair_class: None,
        rules: NetRules {
            width_mm: Some(0.2),
            clearance_mm: Some(0.15),
            ..NetRules::default()
        },
        show_on_flag: true,
    });
    s.directives.push(SchDirective::NetParameters {
        uuid: ids.next(),
        position: pt(50.0, u[2].y),
        net_class: None,
        diff_pair_class: Some("USB".to_string()),
        rules: NetRules::default(),
        show_on_flag: false,
    });
    s
}

/// A 60 x 40 mm outline, no parts: Update PCB places them.
fn board(ids: &mut Ids) -> Vec<u8> {
    let board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": ids.next(),
        "outline": [
            {"x": 0.0, "y": 0.0}, {"x": 60.0, "y": 0.0},
            {"x": 60.0, "y": 40.0}, {"x": 0.0, "y": 40.0}
        ]
    }))
    .expect("an empty board");
    SnxPcb::new(board)
        .write_string()
        .expect("the fixture board writes")
        .into_bytes()
}

fn project() -> Vec<u8> {
    project_bytes(serde_json::json!({
        "name": DIR,
        "dir": "",
        "schematic_root": "main.snxsch",
        "pcb_file": format!("{DIR}.snxpcb"),
        "sheets": [sheet_entry("main.snxsch")],
        "net_classes": [{"name": "POWER"}, {"name": "HS"}],
        "schematic_design": {
            "diff_pair_classes": [
                {"name": "USB", "impedance_ohm": 90.0, "width_mm": 0.2, "gap_mm": 0.15}
            ]
        },
    }))
}
