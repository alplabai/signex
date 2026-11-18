//! Design reuse blocks (#2410): a routed channel is cut out as a block and
//! laid onto another channel of the same circuit around an anchor, its
//! counterparts found through their schematic units and its copper put on
//! the nets their pads reach, as one batch.

use signex_types::pcb::{PcbBoard, Point, SymbolLink};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::*;

const CH_A: Uuid = Uuid::from_u128(0xca);
const CH_B: Uuid = Uuid::from_u128(0xcb);
const UNIT_U: Uuid = Uuid::from_u128(0x100);
const UNIT_R: Uuid = Uuid::from_u128(0x200);
const U1: Uuid = Uuid::from_u128(0x11);
const R1: Uuid = Uuid::from_u128(0x12);
const U2: Uuid = Uuid::from_u128(0x21);
const R2: Uuid = Uuid::from_u128(0x22);
const TRACK: Uuid = Uuid::from_u128(0x31);
const VIA: Uuid = Uuid::from_u128(0x32);

fn pad(number: &str, x: f64, net: (u32, &str)) -> serde_json::Value {
    serde_json::json!({
        "uuid": Uuid::now_v7(), "number": number, "pad_type": "smd", "shape": "rect",
        "position": {"x": x, "y": 0.0}, "size": {"x": 0.6, "y": 0.6}, "drill": null,
        "layers": ["TopCopper"], "net": {"number": net.0, "name": net.1}
    })
}

fn footprint(
    uuid: Uuid,
    reference: &str,
    at: (f64, f64),
    nets: [(u32, &str); 2],
) -> serde_json::Value {
    serde_json::json!({
        "uuid": uuid, "reference": reference, "value": "x", "footprint_id": "P2",
        "position": {"x": at.0, "y": at.1}, "layer": "TopCopper",
        "pads": [pad("1", -1.0, nets[0]), pad("2", 1.0, nets[1])]
    })
}

/// Channel A (U1, R1) routed with a track on A1 and a GND via; channel B
/// (U2, R2) placed apart and not routed. Each part links to its channel's
/// sheet placement with the units the two placements share.
fn board() -> PcbBoard {
    let mut board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [
            {"number": 1, "name": "GND"}, {"number": 2, "name": "A1"},
            {"number": 3, "name": "OUT1"}, {"number": 4, "name": "A2"},
            {"number": 5, "name": "OUT2"}
        ],
        "footprints": [
            footprint(U1, "U1", (10.0, 10.0), [(2, "A1"), (1, "GND")]),
            footprint(R1, "R1", (14.0, 10.0), [(2, "A1"), (3, "OUT1")]),
            footprint(U2, "U2", (50.0, 50.0), [(4, "A2"), (1, "GND")]),
            footprint(R2, "R2", (70.0, 70.0), [(4, "A2"), (5, "OUT2")])
        ],
        "segments": [{
            "uuid": TRACK, "start": {"x": 11.0, "y": 10.0}, "end": {"x": 13.0, "y": 10.0},
            "width": 0.25, "layer": "TopCopper", "net": 2
        }],
        "vias": [{
            "uuid": VIA, "position": {"x": 12.0, "y": 12.0}, "diameter": 0.6, "drill": 0.3,
            "layers": ["TopCopper", "BottomCopper"], "net": 1
        }]
    }))
    .unwrap();
    for fp in &mut board.footprints {
        let (sheet, unit) = match fp.reference.as_str() {
            "U1" => (CH_A, UNIT_U),
            "R1" => (CH_A, UNIT_R),
            "U2" => (CH_B, UNIT_U),
            _ => (CH_B, UNIT_R),
        };
        fp.schematic_link = Some(SymbolLink::new(
            InstancePath::from_chain(vec![sheet]),
            [unit],
        ));
    }
    board
}

fn channel_a() -> Vec<BoardItemId> {
    vec![
        BoardItemId::Footprint(U1),
        BoardItemId::Footprint(R1),
        BoardItemId::Segment(TRACK),
        BoardItemId::Via(VIA),
    ]
}

fn ids() -> impl FnMut() -> Uuid {
    let mut next = 0x1000_u128;
    move || {
        next += 1;
        Uuid::from_u128(next)
    }
}

fn at(board: &PcbBoard, id: Uuid) -> (f64, f64, f64) {
    let fp = board.footprints.iter().find(|f| f.uuid == id).unwrap();
    (fp.position.x, fp.position.y, fp.rotation)
}

fn net_name(board: &PcbBoard, number: u32) -> &str {
    board
        .nets
        .iter()
        .find(|n| n.number == number)
        .map_or("", |n| n.name.as_str())
}

#[test]
fn a_block_is_its_items_about_the_first_designator_with_their_nets() {
    let block = capture(&board(), &channel_a()).unwrap();
    let names: Vec<&str> = block
        .footprints
        .iter()
        .map(|f| f.reference.as_str())
        .collect();
    assert_eq!(names, ["U1", "R1"]);
    assert_eq!(at(&block, R1), (0.0, 0.0, 0.0), "R1 is the origin");
    assert_eq!(at(&block, U1), (-4.0, 0.0, 0.0));
    let track = &block.segments[0];
    assert_eq!(
        (track.start, track.end),
        (Point::new(-3.0, 0.0), Point::new(-1.0, 0.0))
    );
    assert_eq!(block.vias[0].position, Point::new(-2.0, 2.0));
    let mut nets: Vec<&str> = block.nets.iter().map(|n| n.name.as_str()).collect();
    nets.sort_unstable();
    assert_eq!(nets, ["A1", "GND", "OUT1"]);
}

#[test]
fn a_block_needs_footprints_with_parts_that_are_on_the_board() {
    let mut unlinked = board();
    unlinked.footprints[1].schematic_link = None;
    assert_eq!(
        capture(&unlinked, &channel_a()).unwrap_err(),
        ReuseRefusal::Unlinked("R1".into())
    );
    assert_eq!(
        capture(&board(), &[BoardItemId::Segment(TRACK)]).unwrap_err(),
        ReuseRefusal::NoFootprint
    );
    assert_eq!(
        capture(&board(), &[BoardItemId::Footprint(Uuid::from_u128(0xdead))]).unwrap_err(),
        ReuseRefusal::Missing
    );
}

#[test]
fn placing_lays_the_channel_out_around_the_anchor_on_its_own_nets() {
    let mut target = board();
    let block = capture(&target, &channel_a()).unwrap();
    let placed = place(&block, &target, U2, 0.0, &mut ids()).unwrap();
    assert_eq!(
        placed.footprints,
        [
            ("U1".to_string(), "U2".to_string()),
            ("R1".to_string(), "R2".to_string())
        ]
    );
    assert_eq!((placed.tracks, placed.vias), (1, 1));
    for command in placed.commands {
        command.apply(&mut target).unwrap();
    }
    assert_eq!(at(&target, U2), (50.0, 50.0, 0.0), "the anchor stays");
    assert_eq!(at(&target, R2), (54.0, 50.0, 0.0));
    let track = target.segments.last().unwrap();
    assert_ne!(track.uuid, TRACK, "the copper has ids of its own");
    assert_eq!(
        (track.start, track.end),
        (Point::new(51.0, 50.0), Point::new(53.0, 50.0))
    );
    assert_eq!(net_name(&target, track.net), "A2", "A1 follows the pads");
    let via = target.vias.last().unwrap();
    assert_eq!(via.position, Point::new(52.0, 52.0));
    assert_eq!(net_name(&target, via.net), "GND");
}

#[test]
fn a_turn_turns_the_block_about_the_anchor() {
    let mut target = board();
    let block = capture(&target, &channel_a()).unwrap();
    let placed = place(&block, &target, U2, 90.0, &mut ids()).unwrap();
    for command in placed.commands {
        command.apply(&mut target).unwrap();
    }
    assert_eq!(at(&target, U2), (50.0, 50.0, 90.0));
    assert_eq!(at(&target, R2), (50.0, 54.0, 90.0));
}

#[test]
fn an_anchor_outside_the_block_and_a_part_without_counterpart_are_refused() {
    let target = board();
    let block = capture(&target, &channel_a()).unwrap();
    let mut stranger = target.clone();
    stranger.footprints[2].schematic_link = Some(SymbolLink::new(
        InstancePath::from_chain(vec![CH_B]),
        [Uuid::from_u128(0x999)],
    ));
    assert_eq!(
        place(&block, &stranger, U2, 0.0, &mut ids()),
        Err(ReuseRefusal::AnchorNotInBlock("U2".into()))
    );
    let mut lonely = target.clone();
    lonely.footprints[3].schematic_link = None;
    assert_eq!(
        place(&block, &lonely, U2, 0.0, &mut ids()),
        Err(ReuseRefusal::NoMatch {
            anchor: "U2".into(),
            block: "R1".into()
        })
    );
    let mut locked = target.clone();
    locked.footprints[3].locked = true;
    assert_eq!(
        place(&block, &locked, U2, 0.0, &mut ids()),
        Err(ReuseRefusal::Locked("R2".into()))
    );
    assert_eq!(
        place(&block, &target, U2, f64::NAN, &mut ids()),
        Err(ReuseRefusal::BadTurn)
    );
}

#[test]
fn a_block_net_that_meets_two_board_nets_is_refused() {
    let mut target = board();
    let block = capture(&target, &channel_a()).unwrap();
    target.nets.push(signex_types::pcb::NetDef {
        number: 6,
        name: "B2".into(),
    });
    let r2_pad = &mut target.footprints[3].pads[0];
    r2_pad.net = Some(signex_types::pcb::PadNet {
        number: 6,
        name: "B2".into(),
    });
    assert_eq!(
        place(&block, &target, U2, 0.0, &mut ids()),
        Err(ReuseRefusal::NetConflict {
            net: "A1".into(),
            first: "A2".into(),
            second: "B2".into()
        })
    );
}
