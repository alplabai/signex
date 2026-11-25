//! #2389 — push and shove: an other-net track in a leg's way moves square
//! to itself just far enough, carrying the tracks joined at its ends; a
//! track held by a pad, or one the leg crosses, does not move.

use uuid::Uuid;

use super::test_board::{at, board, nm, pad_at, track_at};
use super::{NewCopper, Pushed, RouteWorld, shove, shove_run};
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

#[test]
fn a_track_with_a_branch_off_its_side_does_not_move() {
    // Net 2: a run over the leg, risers joined at both ends, and a branch
    // whose end sits on the run's side (a T-junction), clear of the leg.
    let run = |branch: bool| {
        let mut tracks = vec![
            track_at(1, (2.0, 0.1), (14.0, 0.1), 2),
            track_at(2, (2.0, 0.1), (2.0, 5.0), 2),
            track_at(3, (14.0, 5.0), (14.0, 0.1), 2),
        ];
        if branch {
            tracks.push(track_at(4, (13.0, 0.1), (13.0, 5.0), 2));
        }
        board(Vec::new(), tracks)
    };
    let free = run(false);
    let world = RouteWorld::new(&free, &rule);
    let pushed = shove(&world, &[leg()]).expect("without the branch the run moves");
    assert_eq!(pushed.len(), 3, "the run and its two risers: {pushed:?}");

    let branched = run(true);
    let world = RouteWorld::new(&branched, &rule);
    assert_eq!(
        shove(&world, &[leg()]),
        None,
        "moving the run would leave the branch behind"
    );
}

/// #2451 review: a locked track is not pushed, nor stretched when it joins
/// a pushed one, so the route walks around it instead of offering a move
/// the board then refuses.
#[test]
fn a_locked_track_is_not_pushed_or_stretched() {
    let mut run = track_at(1, (2.0, 0.3), (8.0, 0.3), 2);
    run.locked = true;
    let locked_run = board(Vec::new(), vec![run]);
    let world = RouteWorld::new(&locked_run, &rule);
    assert_eq!(shove(&world, &[leg()]), None, "the locked run stays");

    let mut riser = track_at(2, (2.0, 0.3), (2.0, 5.0), 2);
    riser.locked = true;
    let locked_riser = board(
        Vec::new(),
        vec![track_at(1, (2.0, 0.3), (8.0, 0.3), 2), riser],
    );
    let world = RouteWorld::new(&locked_riser, &rule);
    assert_eq!(shove(&world, &[leg()]), None, "its locked riser stays");
}

/// #2483: a shove reports the rounds of pushing it ran: none when nothing
/// is in the way, one per track pushed in turn, and the round it gave up in.
#[test]
fn a_shove_reports_the_rounds_it_took() {
    let clear = board(Vec::new(), vec![track_at(1, (2.0, 3.0), (8.0, 3.0), 2)]);
    let world = RouteWorld::new(&clear, &rule);
    let run = shove_run(&world, &[leg()]);
    let answer = (run.pushed, run.rounds, run.settled);
    assert_eq!(answer, (Some(Vec::new()), 0, true));

    // Net 2 over the leg, net 3 over net 2: net 2 pushes net 3 in turn.
    let stacked = board(
        Vec::new(),
        vec![
            track_at(1, (2.0, 0.3), (8.0, 0.3), 2),
            track_at(2, (2.0, 0.7), (8.0, 0.7), 3),
        ],
    );
    let world = RouteWorld::new(&stacked, &rule);
    let run = shove_run(&world, &[leg()]);
    assert_eq!(run.rounds, 2, "{run:?}");
    assert_eq!(run.pushed.as_ref().map(Vec::len), Some(2));
    assert_eq!(run.pushed, shove(&world, &[leg()]), "the same answer");

    let crossed = board(Vec::new(), vec![track_at(1, (5.0, -3.0), (5.0, 3.0), 2)]);
    let world = RouteWorld::new(&crossed, &rule);
    let run = shove_run(&world, &[leg()]);
    assert_eq!(
        (run.pushed, run.rounds, run.settled),
        (None, 1, true),
        "gave up in round one: a push that cannot be made is an answer"
    );

    // Six nets stacked over the leg, each crowding the next: more pushes in
    // turn than the rounds allow, so the pushing never settles.
    let tower = board(
        Vec::new(),
        (0..6)
            .map(|k| {
                let y = 0.3 + 0.4 * k as f64;
                track_at(k as u128 + 1, (2.0, y), (8.0, y), k + 2)
            })
            .collect(),
    );
    let world = RouteWorld::new(&tower, &rule);
    let run = shove_run(&world, &[leg()]);
    assert_eq!((run.pushed.as_ref(), run.settled), (None, false), "{run:?}");
    assert_eq!(run.rounds, super::shove::ROUNDS + 1);
}

/// #2515: a track of no net in the way is an obstacle the push leaves
/// where it is.
#[test]
fn a_track_of_no_net_in_the_way_blocks_and_does_not_move() {
    let board = board(Vec::new(), vec![track_at(1, (2.0, 0.3), (8.0, 0.3), 0)]);
    let world = RouteWorld::new(&board, &rule);
    assert!(!world.is_clear(&leg()), "the no-net track is in the way");
    assert_eq!(shove(&world, &[leg()]), None, "not moved");
}
