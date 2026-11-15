//! #2389 — push and shove: an other-net track in a leg's way moves square
//! to itself just far enough, carrying the tracks joined at its ends; a
//! track held by a pad, or one the leg crosses, does not move.

use uuid::Uuid;

use super::test_board::{at, board, nm, pad_at, track_at};
use super::{NewCopper, Pushed, RouteWorld, shove};
use crate::copper::CopperItem;
use crate::units::PointNm;

fn rule(_: &CopperItem, _: &CopperItem) -> i64 {
    200_000
}

/// A net-1 leg along y = 0 from x = 0 to x = 10, 0.25 mm wide.
fn leg() -> NewCopper {
    NewCopper::track(at(0.0, 0.0), at(10.0, 0.0), nm(0.125), 1, 1)
}

#[test]
fn a_track_in_the_way_moves_just_clear_and_its_neighbours_follow() {
    // Net 2: a run 0.3 mm above the leg, its ends joined to two risers.
    let board = board(
        Vec::new(),
        vec![
            track_at(1, (2.0, 0.3), (8.0, 0.3), 2),
            track_at(2, (2.0, 0.3), (2.0, 5.0), 2),
            track_at(3, (8.0, 5.0), (8.0, 0.3), 2),
        ],
    );
    let world = RouteWorld::new(&board, &rule);
    assert!(!world.is_clear(&leg()), "the run is in the way");
    let pushed = shove(&world, &[leg()]).expect("the run can move");
    // Track edges 0.125 mm from each centre line, 0.2 mm apart: the run's
    // centre line must reach y = 0.45 mm, one nm past for the rounding.
    let lifted = at(0.0, 0.3).y + 150_001;
    assert_eq!(
        pushed,
        vec![
            Pushed {
                track: Uuid::from_u128(1),
                start: PointNm::new(nm(2.0), lifted),
                end: PointNm::new(nm(8.0), lifted),
            },
            Pushed {
                track: Uuid::from_u128(2),
                start: PointNm::new(nm(2.0), lifted),
                end: at(2.0, 5.0),
            },
            Pushed {
                track: Uuid::from_u128(3),
                start: at(8.0, 5.0),
                end: PointNm::new(nm(8.0), lifted),
            },
        ]
    );
}

#[test]
fn nothing_in_the_way_moves_nothing() {
    let board = board(Vec::new(), vec![track_at(1, (2.0, 3.0), (8.0, 3.0), 2)]);
    let world = RouteWorld::new(&board, &rule);
    assert_eq!(shove(&world, &[leg()]), Some(Vec::new()));
}

#[test]
fn a_track_held_by_a_pad_or_crossed_by_the_leg_does_not_move() {
    // The pad is past the leg's end, clear of it; the track it holds is not.
    let held = board(
        vec![pad_at(1, 12.0, 0.3, 0.5, 2)],
        vec![track_at(1, (5.0, 0.3), (12.0, 0.3), 2)],
    );
    let world = RouteWorld::new(&held, &rule);
    assert_eq!(shove(&world, &[leg()]), None, "held by its pad");

    let crossed = board(Vec::new(), vec![track_at(1, (5.0, -3.0), (5.0, 3.0), 2)]);
    let world = RouteWorld::new(&crossed, &rule);
    assert_eq!(shove(&world, &[leg()]), None, "the leg crosses it");
}
