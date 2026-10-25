//! Format-version policy and the `.snxpcb` 1.1 pad-key / layer-type
//! round-trips (#104). Policy: see `version.rs`.

use super::tests::empty_board;
use super::*;
use crate::pcb::{Footprint, LayerDef, Pad, PadShape, PadType, Point as PcbPoint};
use uuid::Uuid;

const SCH_ID: &str = "schematic_id = \"00000000-0000-0000-0000-000000000000\"\n";
const PCB_ID: &str = "pcb_id = \"00000000-0000-0000-0000-000000000000\"\n";

#[test]
fn snxsch_accepts_current_and_newer_minor_rejects_other_major() {
    let cur = SnxSchematic::parse(&format!("format = \"snxsch/1\"\n{SCH_ID}")).unwrap();
    assert!(!cur.is_from_newer_minor());

    let newer = SnxSchematic::parse(&format!("format = \"snxsch/1.7\"\n{SCH_ID}")).unwrap();
    assert!(newer.is_from_newer_minor());
    // Saving would drop content this build cannot represent: refused.
    let err = newer.write_string().unwrap_err();
    assert!(err.to_string().contains("snxsch/1.7"), "{err}");
    // Even re-wrapped (as the engine does), the source is remembered.
    assert!(
        SnxSchematic::new(newer.sheet.clone())
            .write_string()
            .is_err()
    );

    for bad in ["snxsch/2", "snxsch/0.9", "snxpcb/1", "snxsch/x"] {
        let err = SnxSchematic::parse(&format!("format = \"{bad}\"\n{SCH_ID}")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(bad) && msg.contains(SNXSCH_FORMAT_V1), "{msg}");
    }
}

#[test]
fn snxpcb_older_minor_saves_as_current_newer_minor_is_read_only() {
    let old = SnxPcb::parse(&format!("format = \"snxpcb/1\"\n{PCB_ID}")).unwrap();
    assert!(!old.is_from_newer_minor());
    let out = old.write_string().unwrap();
    assert!(
        out.starts_with(&format!("format = \"{SNXPCB_FORMAT_V1}\"")),
        "{out}"
    );

    let newer = SnxPcb::parse(&format!("format = \"snxpcb/1.9\"\n{PCB_ID}")).unwrap();
    assert!(newer.is_from_newer_minor());
    let err = newer.write_string().unwrap_err();
    assert!(err.to_string().contains("snxpcb/1.9"), "{err}");
    assert!(SnxPcb::new(newer.board.clone()).write_string().is_err());

    assert!(SnxPcb::parse(&format!("format = \"snxpcb/2.0\"\n{PCB_ID}")).is_err());
}

fn pad(uuid: &str, number: &str) -> Pad {
    Pad {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::parse_str(uuid).unwrap(),
        number: number.into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: PcbPoint { x: 1.0, y: 2.0 },
        size: PcbPoint { x: 1.0, y: 0.6 },
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
    }
}

fn footprint(uuid: &str, reference: &str, pad: Pad) -> Footprint {
    Footprint {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::parse_str(uuid).unwrap(),
        reference: reference.into(),
        value: String::new(),
        footprint_id: "x.snxfpt".into(),
        position: PcbPoint { x: 0.0, y: 0.0 },
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
    }
}

#[test]
fn snxpcb_shared_designator_keeps_pads_apart_and_layer_types_round_trip() {
    let mut board = empty_board();
    board.footprints.push(footprint(
        "0192a8c0-0010-7000-8000-000000000001",
        "R1",
        pad("0192a8c0-0011-7000-8000-000000000001", "1"),
    ));
    board.footprints.push(footprint(
        "0192a8c0-0010-7000-8000-000000000002",
        "R1",
        pad("0192a8c0-0011-7000-8000-000000000002", "7"),
    ));
    board.layers = vec![
        LayerDef {
            id: 0,
            name: "Top".into(),
            layer_type: "signal".into(),
        },
        LayerDef {
            id: 1,
            name: "Core".into(),
            layer_type: "dielectric".into(),
        },
    ];
    let text = SnxPcb::new(board).write_string().unwrap();
    let back = SnxPcb::parse(&text).unwrap().board;
    assert_eq!(back.footprints.len(), 2);
    assert_eq!(back.footprints[0].pads.len(), 1);
    assert_eq!(back.footprints[0].pads[0].number, "1");
    assert_eq!(back.footprints[1].pads.len(), 1);
    assert_eq!(back.footprints[1].pads[0].number, "7");
    assert_eq!(back.layers[0].layer_type, "signal");
    assert_eq!(back.layers[1].layer_type, "dielectric");
}

#[test]
fn snxpcb_1_0_designator_keyed_pads_still_read() {
    let text = format!(
        "format = \"snxpcb/1\"\n{PCB_ID}\n\
[footprints]\ncontent = \"\"\"\n\
uuid ref library pos_x pos_y rotation layer value\n\
0192a8c0-0000-7000-8000-000000001001 R1 l 0 0 0 TopCopper v\n\"\"\"\n\n\
[pads]\ncontent = \"\"\"\n\
uuid footprint_ref pin pos_x pos_y size_x size_y pad_type shape layers drill net_number net_name roundrect_ratio\n\
0192a8c0-0000-7000-8000-000000002001 R1 1 0 0 1000 1000 smd rect TopCopper 0 0 \"\" 0\n\"\"\"\n"
    );
    let back = SnxPcb::parse(&text).unwrap().board;
    assert_eq!(back.footprints.len(), 1, "no synthetic orphan footprint");
    assert_eq!(back.footprints[0].pads.len(), 1);
}
