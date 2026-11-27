//! #2593: a pad's own thermal relief values ride the optional `thermal` pad
//! column, written only when some pad sets one; a board with none re-saves
//! byte for byte, and a cell this build cannot use fails naming it.

use super::tests::empty_board;
use super::*;
use crate::pad_thermal::PadThermal;
use crate::pcb::{Footprint, Pad, PadShape, PadType, Point as PcbPoint};
use uuid::Uuid;

fn board_with(pads: Vec<Pad>) -> crate::pcb::PcbBoard {
    let mut board = empty_board();
    board.footprints.push(Footprint {
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::from_u128(1),
        reference: "J1".into(),
        value: String::new(),
        footprint_id: "x.snxfpt".into(),
        position: PcbPoint { x: 0.0, y: 0.0 },
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads,
        graphics: Vec::new(),
        properties: Vec::new(),
    });
    board
}

fn pad(n: u128, thermal: PadThermal) -> Pad {
    Pad {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::from_u128(100 + n),
        number: n.to_string(),
        pad_type: PadType::Thru,
        shape: PadShape::Circle,
        position: PcbPoint {
            x: 2.54 * n as f64,
            y: 0.0,
        },
        size: PcbPoint { x: 1.7, y: 1.7 },
        drill: None,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        layer_sizes: None,
        net_locked: false,
        thermal,
    }
}

fn save(board: &crate::pcb::PcbBoard) -> String {
    SnxPcb::new(board.clone()).write_string().unwrap()
}

fn manual() -> PadThermal {
    PadThermal {
        air_gap: Some(0.3),
        spoke_width: None,
        spokes: Some(2),
    }
}

#[test]
fn a_pads_own_values_round_trip_in_whole_nm_with_rule_parts_empty() {
    let board = board_with(vec![pad(1, manual()), pad(2, PadThermal::RULE)]);
    let text = save(&board);
    let pads = text.split("[pads]").nth(1).unwrap();
    assert!(pads.contains("thermal"), "{text}");
    assert!(pads.contains("300000;;2"), "{text}");
    let back = SnxPcb::parse(&text).unwrap();
    assert_eq!(back.board.footprints[0].pads, board.footprints[0].pads);
    assert_eq!(back.write_string().unwrap(), text, "a re-save is identical");
}

/// No pad sets a value: no column, and the bytes are those of a board
/// written before the column existed.
#[test]
fn a_board_with_no_pad_values_writes_no_thermal_column() {
    let mut board = board_with(vec![pad(1, PadThermal::RULE)]);
    let before = save(&board);
    assert!(!before.split("[pads]").nth(1).unwrap().contains("thermal"));
    board.footprints[0].pads[0].thermal = manual();
    assert_ne!(save(&board), before);
    board.footprints[0].pads[0].thermal = PadThermal::RULE;
    assert_eq!(save(&board), before, "the file is what it was");
}

#[test]
fn a_thermal_cell_this_build_cannot_use_fails_naming_it() {
    let text = save(&board_with(vec![pad(1, manual())]));
    for bad in ["0;;2", "300000;;5", "300000;2", "x;;2", "300000;;2;1"] {
        let err = SnxPcb::parse(&text.replace("300000;;2", bad))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("\"thermal\"") && err.contains(bad),
            "{bad}: {err}"
        );
    }
}

#[test]
fn a_value_the_reader_refuses_is_refused_on_save() {
    let bad = PadThermal {
        spoke_width: Some(-0.1),
        ..PadThermal::RULE
    };
    let err = SnxPcb::new(board_with(vec![pad(1, bad)]))
        .write_string()
        .unwrap_err()
        .to_string();
    assert!(err.contains("thermal"), "{err}");
}

/// A free pad has no thermal key: its own values are refused on save
/// rather than dropped.
#[test]
fn a_free_pad_with_its_own_values_is_refused_on_save() {
    let mut board = empty_board();
    board.free.pads.push(pad(1, manual()));
    let err = SnxPcb::new(board).write_string().unwrap_err().to_string();
    assert!(err.contains("thermal"), "{err}");
}

/// A length under half a nm would be written as 0 nm, which the reader
/// refuses: it is no thermal length, so the save refuses it rather than
/// write a board that cannot reopen.
#[test]
fn a_length_that_rounds_to_zero_nm_is_refused_on_save_not_written() {
    let tiny = PadThermal {
        air_gap: Some(4e-7),
        ..PadThermal::RULE
    };
    assert!(!crate::pad_thermal::is_thermal_length(4e-7));
    assert!(tiny.fault().is_some());
    let err = SnxPcb::new(board_with(vec![pad(1, tiny)]))
        .write_string()
        .unwrap_err()
        .to_string();
    assert!(err.contains("thermal"), "{err}");
    // One nm is the smallest length that survives the round trip.
    let one_nm = PadThermal {
        air_gap: Some(1e-6),
        ..PadThermal::RULE
    };
    let text = save(&board_with(vec![pad(1, one_nm)]));
    let back = SnxPcb::parse(&text).expect("the board reopens").board;
    assert_eq!(back.footprints[0].pads[0].thermal, one_nm);
}
