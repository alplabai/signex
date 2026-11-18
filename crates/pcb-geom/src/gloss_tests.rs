use signex_types::pcb::Point;

use super::*;
use crate::tool_fixture::{clearance, empty_board, track, via};

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// A net-1 route bent at (5, 0): (0, 0) to (5, 0), then up to (5, 5).
fn board() -> PcbBoard {
    let mut board = empty_board();
    board.segments.push(track(1, (0.0, 0.0), (5.0, 0.0), 1));
    board.segments.push(track(2, (5.0, 0.0), (5.0, 5.0), 1));
    board
}

fn both() -> BTreeSet<Uuid> {
    [id(1), id(2)].into()
}

#[test]
fn a_free_corner_is_cut_into_one_straight_track() {
    let glossed = gloss(&board(), &both(), clearance);
    assert_eq!(glossed.removed, [id(2)]);
    assert_eq!(glossed.replaced.len(), 1);
    let kept = &glossed.replaced[0];
    assert_eq!(kept.uuid, id(1));
    assert_eq!(
        (kept.start, kept.end),
        (Point::new(0.0, 0.0), Point::new(5.0, 5.0))
    );
    assert_eq!((kept.net, kept.width), (1, 0.25));
}

#[test]
fn a_cut_that_would_crowd_another_net_is_not_made() {
    let mut board = board();
    board.vias.push(via(9, (2.5, 2.5), 0.6, 2));
    assert!(gloss(&board, &both(), clearance).is_empty());
}

#[test]
fn a_corner_on_a_via_or_where_a_third_track_meets_stays() {
    let mut on_via = board();
    on_via.vias.push(via(9, (5.0, 0.0), 0.6, 1));
    assert!(gloss(&on_via, &both(), clearance).is_empty());

    let mut tee = board();
    tee.segments.push(track(3, (5.0, 0.0), (9.0, 0.0), 1));
    assert!(gloss(&tee, &both(), clearance).is_empty());
}

#[test]
fn only_selected_tracks_are_glossed() {
    let one: BTreeSet<Uuid> = [id(1)].into();
    assert!(gloss(&board(), &one, clearance).is_empty());
}

#[test]
fn a_three_leg_run_glosses_down_to_one_track() {
    let mut board = board();
    board.segments.push(track(3, (5.0, 5.0), (10.0, 5.0), 1));
    let all: BTreeSet<Uuid> = [id(1), id(2), id(3)].into();
    let glossed = gloss(&board, &all, clearance);
    assert_eq!(glossed.removed.len(), 2);
    assert_eq!(glossed.replaced.len(), 1);
    let kept = &glossed.replaced[0];
    let ends = [kept.start, kept.end];
    assert!(ends.contains(&Point::new(0.0, 0.0)) && ends.contains(&Point::new(10.0, 5.0)));
}

#[test]
fn a_cut_that_would_strand_a_branch_or_via_on_the_route_is_not_made() {
    // A net-1 branch ending in the middle of the first leg.
    let mut branch = board();
    branch.segments.push(track(3, (2.0, 0.0), (2.0, -4.0), 1));
    assert!(gloss(&branch, &both(), clearance).is_empty());

    // A net-1 via sitting on the second leg.
    let mut on_leg = board();
    on_leg.vias.push(via(9, (5.0, 3.0), 0.6, 1));
    assert!(gloss(&on_leg, &both(), clearance).is_empty());

    // A branch at the far end stays joined: the cut is made.
    let mut far = board();
    far.segments.push(track(3, (5.0, 5.0), (9.0, 5.0), 1));
    assert_eq!(gloss(&far, &both(), clearance).removed, [id(2)]);
}
