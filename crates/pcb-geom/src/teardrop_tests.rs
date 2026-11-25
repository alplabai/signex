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

#[test]
fn a_track_entering_off_centre_gets_a_teardrop_along_the_track() {
    let mut board = empty_board();
    board.vias.push(via(1, (0.0, 0.0), 1.0, 1));
    // Along y = 0.2 mm, 0.2 mm off the via's centre.
    board.segments.push(track(2, (0.0, 0.2), (5.0, 0.2), 1));
    let plan = teardrops(&board, None, clearance);
    assert_eq!(plan.drops.len(), 1, "{plan:?}");
    let outline = &plan.drops[0].outline;
    let line = 200_000;
    // Base across the track at x = 0, inside the via.
    assert_eq!((outline[0].x, outline[3].x), (0, 0));
    for corner in [outline[0], outline[3]] {
        assert!(corner.distance_sq(PointNm::ORIGIN) < 500_000i128 * 500_000);
    }
    // Tip on the track's edges, centred on its line, not on the via's axis.
    assert_eq!(outline[1].x, outline[2].x);
    assert_eq!(
        (outline[1].y, outline[2].y),
        (line + 125_000, line - 125_000)
    );
    // Base centred on the track's line too.
    assert_eq!(outline[0].y - line, line - outline[3].y);
}

/// The pad of [`board`]'s part (`pad_part(10, ..)` names it 11).
const PAD: Uuid = Uuid::from_u128(11);

#[test]
fn the_rule_turns_teardrops_off_at_vias_or_at_pads() {
    let mut no_vias = board();
    no_vias.options.teardrops.vias = false;
    let plan = teardrops(&no_vias, None, clearance);
    assert_eq!(plan.drops.len(), 1, "{plan:?}");
    assert_eq!(plan.drops[0].track, Uuid::from_u128(3), "only the pad's");

    let mut no_pads = board();
    no_pads.options.teardrops.pads = false;
    let plan = teardrops(&no_pads, None, clearance);
    assert_eq!(plan.drops.len(), 1, "{plan:?}");
    assert_eq!(plan.drops[0].track, Uuid::from_u128(2), "only the via's");
}

#[test]
fn a_pad_override_wins_over_the_rule_both_ways() {
    let mut board = board();
    board.options.teardrops.pads = false;
    board.options.teardrops.set_pad_override(PAD, Some(true));
    let plan = teardrops(&board, None, clearance);
    assert!(plan.drops.iter().any(|d| d.track == Uuid::from_u128(3)));

    board.options.teardrops.pads = true;
    board.options.teardrops.set_pad_override(PAD, Some(false));
    let plan = teardrops(&board, None, clearance);
    assert!(plan.drops.iter().all(|d| d.track != Uuid::from_u128(3)));
    assert_eq!(plan.skipped, 0, "a pad the rule leaves out is not a skip");
}

#[test]
fn the_rule_sets_how_wide_and_how_long_a_teardrop_is() {
    let mut board = board();
    board.options.teardrops.width_share = 0.5;
    board.options.teardrops.length_share = 2.0;
    let plan = teardrops(&board, None, clearance);
    // Base at half the via radius; tip two radii past the via edge.
    assert_eq!(
        plan.drops[0].outline,
        [
            PointNm::new(0, 250_000),
            PointNm::new(1_500_000, 125_000),
            PointNm::new(1_500_000, -125_000),
            PointNm::new(0, -250_000),
        ]
    );
}
