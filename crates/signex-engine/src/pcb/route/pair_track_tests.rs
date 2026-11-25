//! #2585 — the tracks of a differential pair that run side by side: the
//! partner of a pair track, the gap between them, and what an edit of the
//! pair reaches.

use serde_json::json;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::tests::track;
use super::*;

/// USB_P and USB_N side by side on Top, centres 0.4 mm apart (0.25 mm
/// tracks, so a 0.15 mm gap), a far USB_N track 3 mm away, a USB_N track on
/// Bottom under USB_P, a crossing USB_N track, and GND.
fn board() -> PcbBoard {
    let mut bottom = track(15, (0.0, 0.0), (10.0, 0.0), 2);
    bottom["layer"] = json!("BottomCopper");
    serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "nets": [
            {"number": 1, "name": "USB_P"}, {"number": 2, "name": "USB_N"},
            {"number": 3, "name": "GND"}
        ],
        "segments": [
            track(11, (0.0, 0.0), (10.0, 0.0), 1),
            track(12, (2.0, 0.4), (12.0, 0.4), 2),
            track(13, (0.0, 3.0), (10.0, 3.0), 2),
            track(14, (5.0, -5.0), (5.0, 5.0), 2),
            bottom,
            track(16, (0.0, 0.8), (10.0, 0.8), 3)
        ]
    }))
    .unwrap()
}

const PAIR: PairNets = PairNets {
    positive: 1,
    negative: 2,
};

fn segment(board: &PcbBoard, n: u128) -> &signex_types::pcb::Segment {
    board
        .segments
        .iter()
        .find(|s| s.uuid == Uuid::from_u128(n))
        .expect("track")
}

#[test]
fn the_partner_is_the_nearest_parallel_track_of_the_other_net_on_the_layer() {
    let board = board();
    let (partner, gap) = coupled_partner(&board, PAIR, segment(&board, 11)).expect("coupled");
    assert_eq!(partner.uuid, Uuid::from_u128(12));
    assert!((gap - 0.15).abs() < 1.0e-9, "{gap}");
    // And back again from the other net.
    let (back, _) = coupled_partner(&board, PAIR, segment(&board, 12)).expect("coupled");
    assert_eq!(back.uuid, Uuid::from_u128(11));
}

#[test]
fn a_track_off_the_pair_or_with_nothing_beside_it_has_no_partner() {
    let board = board();
    assert!(
        coupled_partner(&board, PAIR, segment(&board, 16)).is_none(),
        "GND"
    );
    // The crossing track runs across USB_P, not beside it.
    let crossing = segment(&board, 14);
    let lone = PcbBoard {
        segments: vec![segment(&board, 11).clone(), crossing.clone()],
        ..board.clone()
    };
    assert!(coupled_partner(&lone, PAIR, segment(&lone, 11)).is_none());
}

#[test]
fn an_edit_of_one_track_reaches_its_partner_once() {
    let board = board();
    let one = [Uuid::from_u128(11)];
    assert_eq!(
        with_partners(&board, PAIR, &one),
        [Uuid::from_u128(11), Uuid::from_u128(12)]
    );
    let both = [Uuid::from_u128(12), Uuid::from_u128(11)];
    assert_eq!(
        with_partners(&board, PAIR, &both),
        [Uuid::from_u128(11), Uuid::from_u128(12)],
        "board order, each once"
    );
}

#[test]
fn the_shared_gap_reads_only_when_every_track_has_one() {
    let board = board();
    let gap = shared_gap_mm(&board, PAIR, &[Uuid::from_u128(11), Uuid::from_u128(12)]);
    assert!((gap.expect("shared") - 0.15).abs() < 1.0e-9);
    let alone = PcbBoard {
        segments: vec![segment(&board, 11).clone()],
        ..board.clone()
    };
    assert_eq!(shared_gap_mm(&alone, PAIR, &[Uuid::from_u128(11)]), None);
}
