//! The optional `net_locked` pad column and the optional
//! `[stackup] layer_display_names` list: written only when used, so a board
//! that uses neither re-saves byte-identical and older files load unchanged.

use super::tests::empty_board;
use super::*;
use crate::pcb::{Footprint, LayerDef, Pad, PadShape, PadType, Point as PcbPoint};
use uuid::Uuid;

fn pad(n: u128, locked: bool) -> Pad {
    Pad {
        uuid: Uuid::from_u128(0x100 + n),
        number: n.to_string(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: PcbPoint { x: 0.0, y: 0.0 },
        size: PcbPoint { x: 1.0, y: 1.0 },
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        net_locked: locked,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn board_with_pads(locked: &[bool]) -> PcbBoard {
    let mut board = empty_board();
    board.footprints.push(Footprint {
        uuid: Uuid::from_u128(1),
        reference: "R1".into(),
        value: String::new(),
        footprint_id: "x.snxfpt".into(),
        position: PcbPoint { x: 0.0, y: 0.0 },
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: locked
            .iter()
            .enumerate()
            .map(|(i, l)| pad(i as u128 + 1, *l))
            .collect(),
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    });
    board
}

#[test]
fn unlocked_pads_write_no_net_locked_column() {
    let text = SnxPcb::new(board_with_pads(&[false, false]))
        .write_string()
        .unwrap();
    assert!(!text.contains("net_locked"), "{text}");
    let again = SnxPcb::parse(&text).unwrap();
    assert!(again.board.footprints[0].pads.iter().all(|p| !p.net_locked));
    assert_eq!(again.write_string().unwrap(), text);
}

#[test]
fn a_locked_pad_round_trips_and_its_neighbour_stays_unlocked() {
    let text = SnxPcb::new(board_with_pads(&[false, true]))
        .write_string()
        .unwrap();
    assert!(text.contains("net_locked"), "{text}");
    let parsed = SnxPcb::parse(&text).unwrap();
    let pads = &parsed.board.footprints[0].pads;
    assert!(!pads[0].net_locked);
    assert!(pads[1].net_locked);
    assert!(
        pads.iter().all(|p| p.unknown_columns.is_empty()),
        "the lock column is read, not kept as a foreign cell"
    );
    assert_eq!(parsed.write_string().unwrap(), text);
}

#[test]
fn an_unreadable_net_locked_cell_stays_a_foreign_cell() {
    let text = SnxPcb::new(board_with_pads(&[true]))
        .write_string()
        .unwrap()
        .replace("true", "maybe");
    let parsed = SnxPcb::parse(&text).unwrap();
    let pad = &parsed.board.footprints[0].pads[0];
    assert!(!pad.net_locked);
    assert_eq!(pad.unknown_columns.get("net_locked"), Some("maybe"));
    assert_eq!(parsed.write_string().unwrap(), text);
}

fn layer(id: u8, name: &str, display: &str) -> LayerDef {
    LayerDef {
        id,
        name: name.into(),
        layer_type: String::new(),
        display_name: display.into(),
    }
}

#[test]
fn mechanical_display_names_round_trip() {
    let mut board = empty_board();
    board.layers = vec![
        layer(0, "TopCopper", ""),
        layer(1, "Mechanical1", "Board outline 2"),
        layer(2, "Mechanical2", ""),
    ];
    let text = SnxPcb::new(board).write_string().unwrap();
    assert!(text.contains("layer_display_names"), "{text}");
    let parsed = SnxPcb::parse(&text).unwrap();
    let layers = &parsed.board.layers;
    assert_eq!(layers[1].display_name, "Board outline 2");
    assert_eq!(layers[1].display_label(), "Board outline 2");
    assert_eq!(layers[2].display_name, "");
    assert_eq!(layers[2].display_label(), "Mechanical2");
    assert!(parsed.board.unknown.stackup.is_empty());
    assert_eq!(parsed.write_string().unwrap(), text);
}

#[test]
fn layers_without_display_names_write_no_key() {
    let mut board = empty_board();
    board.layers = vec![layer(0, "TopCopper", ""), layer(1, "Mechanical1", "")];
    let text = SnxPcb::new(board).write_string().unwrap();
    assert!(!text.contains("layer_display_names"), "{text}");
    let parsed = SnxPcb::parse(&text).unwrap();
    assert!(
        parsed
            .board
            .layers
            .iter()
            .all(|l| l.display_name.is_empty())
    );
    assert_eq!(parsed.write_string().unwrap(), text);
}

#[test]
fn more_display_names_than_layers_is_refused() {
    let mut board = empty_board();
    board.layers = vec![layer(0, "Mechanical1", "A")];
    let text = SnxPcb::new(board)
        .write_string()
        .unwrap()
        .replace("[\"A\"]", "[\"A\", \"B\"]");
    assert!(text.contains("\"B\""), "{text}");
    let err = SnxPcb::parse(&text).unwrap_err();
    assert!(
        matches!(err, FormatError::LayerDisplayNamesExceedLayers { .. }),
        "got {err:?}"
    );
}
