//! Orphan pads, duplicate footprint uuids, `layer_types` length, and the
//! strict version-number grammar (#104).

use super::tests::empty_board;
use super::*;
use crate::pcb::{Footprint, Pad, PadShape, PadType, Point as PcbPoint};
use uuid::Uuid;

const SCH_ID: &str = "schematic_id = \"00000000-0000-0000-0000-000000000000\"\n";
const PCB_ID: &str = "pcb_id = \"00000000-0000-0000-0000-000000000000\"\n";
const PAD_HEADER: &str = "uuid footprint_ref pin pos_x pos_y size_x size_y pad_type shape layers drill net_number net_name roundrect_ratio";

fn pad_line(uuid_tail: u32, footprint_ref: &str, pin: u32) -> String {
    format!(
        "0192a8c0-0000-7000-8000-{uuid_tail:012} {footprint_ref} {pin} 0 0 1000 1000 smd rect TopCopper 0 0 \"\" 0"
    )
}

fn pad(uuid: &str) -> Pad {
    Pad {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::parse_str(uuid).unwrap(),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: PcbPoint { x: 0.0, y: 0.0 },
        size: PcbPoint { x: 1.0, y: 1.0 },
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
    }
}

fn footprint(uuid: &str, reference: &str, pad_uuid: &str) -> Footprint {
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
        pads: vec![pad(pad_uuid)],
        graphics: Vec::new(),
        properties: Vec::new(),
    }
}

#[test]
fn version_numbers_are_plain_ascii_digits() {
    for bad in [
        "snxsch/+1",
        "snxsch/01",
        "snxsch/1.+2",
        "snxsch/1.02",
        "snxsch/1.",
    ] {
        assert!(
            SnxSchematic::parse(&format!("format = \"{bad}\"\n{SCH_ID}")).is_err(),
            "{bad}"
        );
    }
    assert!(SnxSchematic::parse(&format!("format = \"snxsch/1.0\"\n{SCH_ID}")).is_ok());
}

#[test]
fn two_orphan_pad_groups_survive_save_and_reload() {
    let text = format!(
        "format = \"snxpcb/1\"\n{PCB_ID}\n[pads]\ncontent = \"\"\"\n{PAD_HEADER}\n{}\n{}\n{}\n\"\"\"\n",
        pad_line(2001, "R9", 1),
        pad_line(2002, "C9", 1),
        pad_line(2003, "R9", 2),
    );
    let board = SnxPcb::parse(&text).unwrap().board;
    assert_eq!(board.footprints.len(), 2);
    let saved = SnxPcb::new(board).write_string().unwrap();
    let back = SnxPcb::parse(&saved).unwrap().board;
    let mut counts: Vec<usize> = back.footprints.iter().map(|f| f.pads.len()).collect();
    counts.sort();
    assert_eq!(counts, vec![1, 2]);
}

#[test]
fn duplicate_footprint_uuids_are_refused_on_write_and_on_read() {
    let u = "0192a8c0-0010-7000-8000-000000000001";
    let mut board = empty_board();
    board
        .footprints
        .push(footprint(u, "R1", "0192a8c0-0011-7000-8000-000000000001"));
    board
        .footprints
        .push(footprint(u, "C1", "0192a8c0-0011-7000-8000-000000000002"));
    let err = SnxPcb::new(board).write_string().unwrap_err().to_string();
    assert!(err.contains("R1") && err.contains("C1"), "{err}");

    let text = format!(
        "format = \"snxpcb/1.1\"\n{PCB_ID}\n[footprints]\ncontent = \"\"\"\n\
uuid ref library pos_x pos_y rotation layer value\n\
{u} R1 l 0 0 0 TopCopper v\n\
{u} C1 l 0 0 0 TopCopper v\n\"\"\"\n\n[pads]\ncontent = \"\"\"\n{PAD_HEADER}\n{}\n\"\"\"\n",
        pad_line(2001, "C1", 1),
    );
    let err = SnxPcb::parse(&text).unwrap_err().to_string();
    assert!(err.contains("R1") && err.contains("C1"), "{err}");
}

#[test]
fn layer_types_longer_than_layers_is_refused() {
    let longer = format!(
        "format = \"snxpcb/1.1\"\n{PCB_ID}\n[stackup]\nlayers = [\"Top\"]\nlayer_types = [\"signal\", \"core\"]\n"
    );
    let err = SnxPcb::parse(&longer).unwrap_err().to_string();
    assert!(err.contains("layer_types"), "{err}");
    let shorter = format!(
        "format = \"snxpcb/1.1\"\n{PCB_ID}\n[stackup]\nlayers = [\"Top\", \"Bot\"]\nlayer_types = [\"signal\"]\n"
    );
    let board = SnxPcb::parse(&shorter).unwrap().board;
    assert_eq!(board.layers[1].layer_type, "");
}

#[test]
fn snxpcb_1_0_two_designators_each_keep_their_pads() {
    let text = format!(
        "format = \"snxpcb/1\"\n{PCB_ID}\n[footprints]\ncontent = \"\"\"\n\
uuid ref library pos_x pos_y rotation layer value\n\
0192a8c0-0000-7000-8000-000000001001 R1 l 0 0 0 TopCopper v\n\
0192a8c0-0000-7000-8000-000000001002 C1 l 0 0 0 TopCopper v\n\"\"\"\n\n\
[pads]\ncontent = \"\"\"\n{PAD_HEADER}\n{}\n{}\n{}\n\"\"\"\n",
        pad_line(2001, "R1", 1),
        pad_line(2002, "C1", 1),
        pad_line(2003, "C1", 2),
    );
    let back = SnxPcb::parse(&text).unwrap().board;
    assert_eq!(back.footprints.len(), 2);
    assert_eq!(back.footprints[0].pads.len(), 1);
    assert_eq!(back.footprints[1].pads.len(), 2);
}

#[test]
fn non_contiguous_layer_ids_and_types_round_trip() {
    use crate::pcb::LayerDef;
    let mut board = empty_board();
    board.layers = vec![
        LayerDef {
            id: 0,
            name: "Top".into(),
            layer_type: "signal".into(),
        },
        LayerDef {
            id: 31,
            name: "Bot".into(),
            layer_type: "signal".into(),
        },
    ];
    let text = SnxPcb::new(board).write_string().unwrap();
    let back = SnxPcb::parse(&text).unwrap().board;
    assert_eq!(back.layers[0].id, 0);
    assert_eq!(back.layers[1].id, 31);
    assert_eq!(back.layers[1].layer_type, "signal");
}

#[test]
fn layer_ids_of_wrong_length_are_a_load_error() {
    let text = format!(
        "format = \"snxpcb/1.1\"
{PCB_ID}
[stackup]
layers = [\"Top\", \"Bot\"]
layer_ids = [7]
"
    );
    let err = SnxPcb::parse(&text).unwrap_err();
    assert!(
        matches!(
            err,
            FormatError::LayerIdsLengthMismatch { layers: 2, ids: 1 }
        ),
        "got {err:?}"
    );
}

#[test]
fn duplicate_layer_ids_are_a_load_error() {
    let text = format!(
        "format = \"snxpcb/1.1\"
{PCB_ID}
[stackup]
layers = [\"Top\", \"Bot\"]
layer_ids = [0, 0]
"
    );
    let err = SnxPcb::parse(&text).unwrap_err();
    assert!(
        matches!(err, FormatError::DuplicateLayerId { id: 0 }),
        "got {err:?}"
    );
}

#[test]
fn positional_layer_ids_write_no_layer_ids_key() {
    use crate::pcb::LayerDef;
    let mut board = empty_board();
    board.layers = (0..2u8)
        .map(|id| LayerDef {
            id,
            name: format!("L{id}"),
            layer_type: "signal".into(),
        })
        .collect();
    let text = SnxPcb::new(board).write_string().unwrap();
    assert!(!text.contains("layer_ids"), "{text}");
}
