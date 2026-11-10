use std::collections::BTreeSet;

use signex_types::pcb::{
    Footprint, NetDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point, Segment, SymbolLink, Via,
    ViaType,
};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::*;

const FP_A: Uuid = Uuid::from_u128(0xa);
const FP_B: Uuid = Uuid::from_u128(0xb);
const FP_LOOSE: Uuid = Uuid::from_u128(0xc);
const UNIT_A1: Uuid = Uuid::from_u128(0x1);
const UNIT_A2: Uuid = Uuid::from_u128(0x2);
const UNIT_B: Uuid = Uuid::from_u128(0x3);
const PAD_A: Uuid = Uuid::from_u128(0x100);
const PAD_B: Uuid = Uuid::from_u128(0x101);

fn child() -> InstancePath {
    InstancePath::from_chain(vec![Uuid::from_u128(0x99)])
}

fn pad(uuid: Uuid, net: Option<u32>) -> Pad {
    Pad {
        uuid,
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: Vec::new(),
        net: net.map(|number| PadNet {
            number,
            name: String::new(),
        }),
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn footprint(uuid: Uuid, link: Option<SymbolLink>, pads: Vec<Pad>) -> Footprint {
    Footprint {
        uuid,
        reference: String::new(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        layer: String::new(),
        locked: false,
        pads,
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: link,
        library_ref: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// A two-unit part A on the root sheet, a part B on a child placement, an
/// unlinked footprint, and net 1 (`VCC`) on A's pad, a track and a via.
fn board() -> PcbBoard {
    PcbBoard {
        sketch: None,
        uuid: Uuid::nil(),
        version: 0,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: vec![
            NetDef {
                number: 0,
                name: String::new(),
            },
            NetDef {
                number: 1,
                name: "VCC".into(),
            },
            NetDef {
                number: 2,
                name: "GND".into(),
            },
        ],
        footprints: vec![
            footprint(
                FP_A,
                Some(SymbolLink::new(InstancePath::root(), [UNIT_A1, UNIT_A2])),
                vec![pad(PAD_A, Some(1))],
            ),
            footprint(
                FP_B,
                Some(SymbolLink::new(child(), [UNIT_B])),
                vec![pad(PAD_B, Some(2))],
            ),
            footprint(FP_LOOSE, None, vec![pad(Uuid::from_u128(0x102), Some(1))]),
        ],
        segments: vec![Segment {
            uuid: Uuid::from_u128(0x200),
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
            width: 0.2,
            layer: String::new(),
            net: 1,
            unknown_columns: Default::default(),
        }],
        vias: vec![Via {
            uuid: Uuid::from_u128(0x300),
            position: Point::new(0.0, 0.0),
            diameter: 0.6,
            drill: 0.3,
            layers: Vec::new(),
            net: 2,
            via_type: ViaType::Through,
            unknown_columns: Default::default(),
        }],
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

fn part(sheet: InstancePath, symbol: Uuid) -> PartOccurrence {
    PartOccurrence { sheet, symbol }
}

#[test]
fn any_unit_of_a_part_finds_its_footprint() {
    let board = board();
    for unit in [UNIT_A1, UNIT_A2] {
        assert_eq!(
            footprints_for_parts(&board, &[part(InstancePath::root(), unit)]),
            vec![FP_A]
        );
    }
}

#[test]
fn the_sheet_occurrence_must_match_not_only_the_unit() {
    let board = board();
    assert!(footprints_for_parts(&board, &[part(InstancePath::root(), UNIT_B)]).is_empty());
    assert_eq!(
        footprints_for_parts(&board, &[part(child(), UNIT_B)]),
        vec![FP_B]
    );
}

#[test]
fn a_footprint_gives_every_unit_of_its_part() {
    let board = board();
    assert_eq!(
        parts_for_footprints(&board, [FP_A, FP_LOOSE]),
        vec![
            part(InstancePath::root(), UNIT_A1),
            part(InstancePath::root(), UNIT_A2)
        ]
    );
    assert!(parts_for_footprints(&board, [FP_LOOSE]).is_empty());
}

#[test]
fn nets_match_by_name_and_list_their_board_members() {
    let board = board();
    let nets = nets_named(&board, &["VCC".to_string(), "MISSING".to_string()]);
    assert_eq!(nets, BTreeSet::from([1]));
    let members = board_net_members(&board, &nets);
    assert_eq!(
        members.pads,
        vec![(FP_A, PAD_A), (FP_LOOSE, Uuid::from_u128(0x102))]
    );
    assert_eq!(members.tracks, vec![Uuid::from_u128(0x200)]);
    assert!(members.vias.is_empty());
    assert_eq!(net_names(&board, &BTreeSet::from([2, 7])), vec!["GND"]);
}

#[test]
fn net_zero_and_blank_names_match_nothing() {
    let board = board();
    assert!(nets_named(&board, &[String::new()]).is_empty());
    assert!(board_net_members(&board, &BTreeSet::new()).is_empty());
    assert!(net_names(&board, &BTreeSet::from([0])).is_empty());
}

#[test]
fn a_pad_names_its_net() {
    let board = board();
    assert_eq!(pad_net(&board, FP_A, PAD_A), Some(1));
    assert_eq!(pad_net(&board, FP_B, PAD_A), None);
}
