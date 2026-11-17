//! A track's and via's lock (#2407) and mask settings (#2378) survive a save and a
//! load, and a board that sets none re-saves without their columns.

use super::tests::empty_board;
use super::*;
use crate::pcb::{
    MaskExpansion, Point as PcbPoint, Segment, TrackAttributes, Via, ViaAttributes, ViaMaskSide,
    ViaType,
};
use uuid::Uuid;

fn track(n: u128, attributes: TrackAttributes) -> Segment {
    Segment {
        uuid: Uuid::from_u128(0x10 + n),
        start: PcbPoint {
            x: 0.0,
            y: n as f64,
        },
        end: PcbPoint {
            x: 5.0,
            y: n as f64,
        },
        width: 0.25,
        layer: "TopCopper".into(),
        net: 0,
        locked: false,
        attributes,
        unknown_columns: Default::default(),
    }
}

fn via(n: u128, attributes: ViaAttributes) -> Via {
    Via {
        uuid: Uuid::from_u128(0x20 + n),
        position: PcbPoint {
            x: 1.0,
            y: n as f64,
        },
        diameter: 0.6,
        drill: 0.3,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: 0,
        via_type: ViaType::Through,
        layer_sizes: None,
        locked: false,
        attributes,
        unknown_columns: Default::default(),
    }
}

#[test]
fn default_settings_write_no_new_column() {
    let mut board = empty_board();
    board.segments.push(track(1, TrackAttributes::default()));
    board.vias.push(via(1, ViaAttributes::default()));
    let text = SnxPcb::new(board).write_string().unwrap();
    for column in ["locked", "paste_mask", "solder_mask", "tented_top"] {
        assert!(!text.contains(column), "{column} written: {text}");
    }
    assert_eq!(SnxPcb::parse(&text).unwrap().write_string().unwrap(), text);
}

#[test]
fn track_and_via_settings_round_trip_beside_default_neighbours() {
    let locked_track = TrackAttributes {
        paste_mask: MaskExpansion::Manual { mm: 0.05 },
        solder_mask: MaskExpansion::Manual { mm: -0.1 },
    };
    let tented_via = ViaAttributes {
        top: ViaMaskSide {
            expansion: MaskExpansion::Manual { mm: 0.075 },
            tented: false,
        },
        bottom: ViaMaskSide {
            expansion: MaskExpansion::Rule,
            tented: true,
        },
        from_hole_edge: true,
    };
    let mut board = empty_board();
    board.segments.push(track(1, TrackAttributes::default()));
    board.segments.push(Segment {
        locked: true,
        ..track(2, locked_track)
    });
    board.vias.push(Via {
        locked: true,
        ..via(1, tented_via)
    });
    board.vias.push(via(2, ViaAttributes::default()));
    let text = SnxPcb::new(board).write_string().unwrap();
    let parsed = SnxPcb::parse(&text).unwrap();
    let b = &parsed.board;
    assert_eq!(b.segments[0].attributes, TrackAttributes::default());
    assert_eq!(b.segments[1].attributes, locked_track);
    assert_eq!(b.vias[0].attributes, tented_via);
    assert_eq!(b.vias[1].attributes, ViaAttributes::default());
    assert!(!b.segments[0].locked && b.segments[1].locked && b.vias[0].locked);
    assert!(
        b.segments.iter().all(|s| s.unknown_columns.is_empty())
            && b.vias.iter().all(|v| v.unknown_columns.is_empty()),
        "the settings are read, not kept as foreign cells"
    );
    assert_eq!(parsed.write_string().unwrap(), text);
}
