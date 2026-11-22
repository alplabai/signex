//! #2401 / #2407: the layer stack rows, per-layer sizes and track and via
//! locks survive a save and load; a board without them writes none.

use super::tests::empty_board;
use super::*;
use crate::pcb::{LayerSize, LayerSizes, LayerStack, PcbBoard};
use crate::pcb_layer::CopperLayer;
use crate::pcb_stack::{StackLayer, StackLayerKind};
use uuid::Uuid;

fn board() -> PcbBoard {
    let mut board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [{"number": 1, "name": "GND"}],
        "segments": [
            {"uuid": Uuid::from_u128(2), "start": {"x": 0.0, "y": 0.0},
             "end": {"x": 5.0, "y": 0.0}, "width": 0.25, "layer": "TopCopper",
             "net": 1, "locked": true},
            {"uuid": Uuid::from_u128(3), "start": {"x": 0.0, "y": 1.0},
             "end": {"x": 5.0, "y": 1.0}, "width": 0.25, "layer": "TopCopper",
             "net": 1}
        ],
        "vias": [
            {"uuid": Uuid::from_u128(4), "position": {"x": 1.0, "y": 1.0},
             "diameter": 0.6, "drill": 0.3,
             "layers": ["TopCopper", "Inner2Copper"], "via_type": "blind",
             "net": 1, "locked": true,
             "layer_sizes": {"mode": "top_middle_bottom",
                             "top": 0.75, "middle": 0.5, "bottom": 0.75}},
            {"uuid": Uuid::from_u128(5), "position": {"x": 3.0, "y": 1.0},
             "diameter": 0.6, "drill": 0.3,
             "layers": ["TopCopper", "BottomCopper"], "net": 1}
        ]
    }))
    .unwrap();
    let mut stack = LayerStack::derived(&[], 1.6);
    stack.layers.insert(
        4,
        StackLayer {
            name: "Core".into(),
            ..StackLayer::new(StackLayerKind::Dielectric)
        },
    );
    board.stackup = Some(stack);
    board.layers = empty_board().layers;
    board
}

fn round_trip(board: &PcbBoard) -> (String, PcbBoard) {
    let text = SnxPcb::new(board.clone()).write_string().unwrap();
    let back = SnxPcb::parse(&text).unwrap().board;
    (text, back)
}

#[test]
fn the_stack_rows_round_trip() {
    let board = board();
    let (text, back) = round_trip(&board);
    assert!(text.contains("dielectric"), "the rows are written: {text}");
    assert_eq!(back.stackup, board.stackup);
}

#[test]
fn locks_and_per_layer_sizes_round_trip() {
    let board = board();
    let (_, back) = round_trip(&board);
    assert_eq!(back.segments, board.segments);
    assert_eq!(back.vias, board.vias);
    assert!(back.segments[0].locked && !back.segments[1].locked);
    assert!(back.vias[0].locked);
    assert_eq!(
        back.vias[0].layer_sizes,
        Some(LayerSizes::TopMiddleBottom {
            top: 0.75,
            middle: 0.5,
            bottom: 0.75
        })
    );
}

#[test]
fn a_board_without_locks_sizes_or_stack_writes_none_of_them() {
    let mut board = board();
    board.stackup = None;
    for s in &mut board.segments {
        s.locked = false;
    }
    for v in &mut board.vias {
        v.locked = false;
        v.layer_sizes = None;
    }
    let (text, _) = round_trip(&board);
    assert!(!text.contains("locked"), "{text}");
    assert!(!text.contains("layer_sizes"), "{text}");
    assert!(!text.contains("dielectric"), "{text}");
}

#[test]
fn full_stack_pad_sizes_round_trip() {
    let mut board = board();
    let pad_sizes = LayerSizes::Full {
        layers: vec![
            LayerSize::new(CopperLayer::Top, crate::pcb::Point::new(1.0, 0.5)),
            LayerSize::new(CopperLayer::Bottom, crate::pcb::Point::new(0.75, 0.5)),
        ],
    };
    let footprint: crate::pcb::Footprint = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(10),
        "position": {"x": 0.0, "y": 0.0},
        "layer": "TopCopper",
        "pads": [{"uuid": Uuid::from_u128(11), "number": "1", "pad_type": "thru",
                  "shape": "circle", "position": {"x": 0.0, "y": 0.0},
                  "size": {"x": 1.0, "y": 1.0},
                  "drill": {"diameter": 0.5}, "layers": [], "net": null}]
    }))
    .unwrap();
    board.footprints.push(footprint);
    board.footprints[0].pads[0].layer_sizes = Some(pad_sizes.clone());
    let (text, back) = round_trip(&board);
    assert!(text.contains("TopCopper=1000000x500000"), "{text}");
    assert_eq!(back.footprints[0].pads[0].layer_sizes, Some(pad_sizes));
}

#[test]
fn a_malformed_layer_sizes_cell_is_a_load_error() {
    let board = board();
    let (text, _) = round_trip(&board);
    let broken = text.replace("top=750000;middle=500000;bottom=750000", "top=abc");
    assert_ne!(broken, text, "the cell is in the file: {text}");
    assert!(SnxPcb::parse(&broken).is_err());
}

/// Decision 2026-11-22 (mockup row 105): the name is saved under its token.
#[test]
fn a_named_stackup_round_trips_under_its_token() {
    let mut named = board();
    named.stackup.as_mut().unwrap().name = Some("sensor-node 4L".into());
    let (text, back) = round_trip(&named);
    assert!(
        text.starts_with(&format!("format = \"{SNXPCB_FORMAT_STACKUP_NAME}\"")),
        "{text}"
    );
    assert!(text.contains("stack_name = \"sensor-node 4L\""), "{text}");
    assert_eq!(back.stackup, named.stackup);
    assert!(back.unknown.stackup.is_empty(), "the key is not foreign");
    let parsed = SnxPcb::parse(&text).unwrap();
    assert!(!parsed.is_from_newer_minor(), "it opens for editing");
    assert_eq!(parsed.write_string().unwrap(), text, "stable on a resave");
}

/// Decision 2026-11-22: no name, no key and no higher token; a name taken
/// away gives the same bytes back.
#[test]
fn a_board_without_a_stackup_name_keeps_its_token_and_bytes() {
    let plain = board();
    let (text, back) = round_trip(&plain);
    assert!(
        text.starts_with(&format!("format = \"{SNXPCB_FORMAT_V1}\"")),
        "{text}"
    );
    assert!(!text.contains("stack_name"), "{text}");
    assert_eq!(SnxPcb::new(back).write_string().unwrap(), text);

    let mut renamed = plain;
    renamed.stackup.as_mut().unwrap().name = Some("4L".into());
    renamed.stackup.as_mut().unwrap().name = None;
    assert_eq!(SnxPcb::new(renamed).write_string().unwrap(), text);
}

/// A name on a file without rows is kept, on the stack the board derives; a
/// blank one is no name.
#[test]
fn a_name_without_rows_names_the_derived_stack() {
    let base = SnxPcb::new(empty_board()).write_string().unwrap();
    let text = format!("{base}\n[stackup]\nstack_name = \"Named\"\n");
    let read = SnxPcb::parse(&text).unwrap().board;
    let stack = read.stackup.as_ref().expect("the name is kept");
    assert_eq!(stack.name.as_deref(), Some("Named"));
    assert_eq!(
        stack.layers,
        LayerStack::derived(&read.layers, read.thickness).layers
    );
    assert!(read.unknown.stackup.is_empty());

    let blank = format!("{base}\n[stackup]\nstack_name = \"  \"\n");
    assert!(SnxPcb::parse(&blank).unwrap().board.stackup.is_none());
}

#[test]
fn a_size_that_does_not_save_is_refused_before_writing() {
    let mut board = board();
    board.vias[0].layer_sizes = Some(LayerSizes::TopMiddleBottom {
        top: 0.6,
        middle: -1.0,
        bottom: 0.6,
    });
    assert!(SnxPcb::new(board).write_string().is_err());
}
