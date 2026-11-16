use super::*;
use crate::tool_fixture::{MM, clearance, empty_board, pad_part, track, via};

/// A net-1 via at the origin with a track out to (5, 0), and a net-1 pad at
/// (10, 0) with a track out to (14, 0).
fn board() -> PcbBoard {
    let mut board = empty_board();
    board.vias.push(via(1, (0.0, 0.0), 1.0, 1));
    board.segments.push(track(2, (0.0, 0.0), (5.0, 0.0), 1));
    board.footprints.push(pad_part(10, (10.0, 0.0), 1.2, 1));
    board.segments.push(track(3, (14.0, 0.0), (10.0, 0.0), 1));
    board
}

#[test]
fn a_track_into_a_via_or_pad_of_its_net_gets_one_teardrop() {
    let plan = teardrops(&board(), None, clearance);
    assert_eq!(plan.skipped, 0);
    assert_eq!(plan.drops.len(), 2, "{:?}", plan.drops);
    let at_via = &plan.drops[0];
    assert_eq!(at_via.track, Uuid::from_u128(2));
    assert_eq!((at_via.net, at_via.layer.as_str()), (1, "TopCopper"));
    // Base at 0.9 of the via radius, tip one radius past the via edge at
    // the track's half width.
    assert_eq!(
        at_via.outline,
        [
            PointNm::new(0, 450_000),
            PointNm::new(MM, 125_000),
            PointNm::new(MM, -125_000),
            PointNm::new(0, -450_000),
        ]
    );
    let at_pad = &plan.drops[1];
    assert_eq!(at_pad.track, Uuid::from_u128(3));
    // Towards +x from the pad at 10 mm: the tip is at 10 + 1.2 mm.
    assert!(
        at_pad
            .outline
            .iter()
            .all(|p| p.x >= 10 * MM && p.x <= 11_200_000)
    );
}

#[test]
fn only_the_nets_asked_for_get_teardrops() {
    let other: BTreeSet<u32> = [2].into();
    assert!(
        teardrops(&board(), Some(&other), clearance)
            .drops
            .is_empty()
    );
    let own: BTreeSet<u32> = [1].into();
    assert_eq!(teardrops(&board(), Some(&own), clearance).drops.len(), 2);
}

#[test]
fn a_teardrop_that_would_crowd_another_net_is_skipped() {
    let mut board = board();
    // A net-2 via about 0.16 mm off the teardrop's upper edge: under the
    // 0.2 mm clearance, though clear of the track itself.
    board.vias.push(via(4, (0.5, 0.55), 0.2, 2));
    let plan = teardrops(&board, None, clearance);
    assert_eq!(plan.skipped, 1);
    assert_eq!(plan.drops.len(), 1);
    assert_eq!(plan.drops[0].track, Uuid::from_u128(3));
}

#[test]
fn a_track_as_wide_as_the_via_gets_none() {
    let mut board = board();
    board.segments[0].width = 1.0;
    let plan = teardrops(&board, None, clearance);
    assert!(plan.drops.iter().all(|d| d.track != Uuid::from_u128(2)));
}
