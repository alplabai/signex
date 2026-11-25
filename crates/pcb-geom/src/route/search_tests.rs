//! #2389 — walkaround: the rule's legs when they are clear, a way round an
//! other-net pad at the rule clearance when they are not, in the corner
//! mode's directions; a drawing on copper is in the way too (#2883).

use signex_types::corner::{CornerMode, CornerRule};
use signex_types::pcb::{BoardGraphic, Point};
use uuid::Uuid;

use super::test_board::{at, board, nm, pad_at};
use super::{RouteWorld, Walk, walkaround};
use crate::copper::CopperItem;
use crate::units::PointNm;

fn rule(_: &CopperItem, _: &CopperItem) -> i64 {
    200_000
}

fn walk(mode: CornerMode) -> Walk {
    Walk {
        net: 1,
        layers: 1,
        radius: nm(0.125),
        rule: CornerRule {
            mode,
            is_flipped: false,
        },
        step: nm(0.5),
        budget: 20_000,
    }
}

fn legs_are_clear(world: &RouteWorld, walk: &Walk, path: &[PointNm]) -> bool {
    path.windows(2).all(|leg| {
        world.is_clear(&super::NewCopper::track(
            leg[0],
            leg[1],
            walk.radius,
            walk.net,
            walk.layers,
        ))
    })
}

#[test]
fn a_clear_run_takes_the_rules_legs() {
    let board = board(Vec::new(), Vec::new());
    let world = RouteWorld::new(&board, &rule);
    let path = walkaround(
        &world,
        at(0.0, 0.0),
        at(5.0, 4.0),
        &walk(CornerMode::Ortho90),
    );
    assert_eq!(
        path,
        Some(vec![at(0.0, 0.0), at(5.0, 0.0), at(5.0, 4.0)]),
        "horizontal first, as the rule draws it"
    );
}

#[test]
fn a_run_goes_around_an_other_net_pad_at_the_rule_clearance() {
    let board = board(vec![pad_at(1, 5.0, 0.0, 1.0, 2)], Vec::new());
    let world = RouteWorld::new(&board, &rule);
    for mode in [
        CornerMode::Ortho90,
        CornerMode::Angle45Start,
        CornerMode::FreeAngle,
    ] {
        let walk = walk(mode);
        let path = walkaround(&world, at(0.0, 0.0), at(10.0, 0.0), &walk)
            .unwrap_or_else(|| panic!("{mode:?}: a way round"));
        assert_eq!(path.first(), Some(&at(0.0, 0.0)));
        assert_eq!(path.last(), Some(&at(10.0, 0.0)));
        assert!(path.len() > 2, "{mode:?}: not straight through the pad");
        assert!(legs_are_clear(&world, &walk, &path), "{mode:?}: {path:?}");
        if mode == CornerMode::FreeAngle {
            continue;
        }
        for leg in path.windows(2) {
            let (dx, dy) = ((leg[1].x - leg[0].x).abs(), (leg[1].y - leg[0].y).abs());
            let diagonal = mode != CornerMode::Ortho90 && dx == dy;
            assert!(dx == 0 || dy == 0 || diagonal, "{mode:?}: leg {leg:?}");
        }
    }
}

/// #2883: a circle drawn on copper is copper of no net; a route of a net
/// goes round its stroke at the rule clearance, never across it.
#[test]
fn a_run_goes_around_a_copper_circle() {
    let mut board = board(Vec::new(), Vec::new());
    board.graphics.push(BoardGraphic {
        uuid: Uuid::from_u128(77),
        graphic_type: "circle".into(),
        layer: "TopCopper".into(),
        width: 0.2,
        start: None,
        end: Some(Point::new(6.0, 0.0)),
        center: Some(Point::new(5.0, 0.0)),
        radius: 1.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
    });
    let world = RouteWorld::new(&board, &rule);
    let walk = walk(CornerMode::Ortho90);
    let straight = super::NewCopper::track(at(0.0, 0.0), at(10.0, 0.0), walk.radius, 1, 1);
    assert!(
        !world.is_clear(&straight),
        "the circle's stroke is in the way"
    );
    let path = walkaround(&world, at(0.0, 0.0), at(10.0, 0.0), &walk).expect("a way round");
    assert!(path.len() > 2, "not straight across the circle");
    assert!(legs_are_clear(&world, &walk, &path), "{path:?}");
}

#[test]
fn a_start_inside_another_nets_clearance_finds_no_way() {
    let board = board(vec![pad_at(1, 0.0, 0.0, 1.0, 2)], Vec::new());
    let world = RouteWorld::new(&board, &rule);
    let path = walkaround(
        &world,
        at(0.6, 0.0),
        at(6.0, 0.0),
        &walk(CornerMode::Ortho90),
    );
    assert_eq!(path, None);
}

#[test]
fn a_way_round_in_a_45_degree_mode_turns_45_degrees_at_most() {
    let board = board(vec![pad_at(1, 5.0, 0.0, 1.0, 2)], Vec::new());
    let world = RouteWorld::new(&board, &rule);
    for mode in [CornerMode::Angle45Start, CornerMode::Angle45End] {
        let walk = walk(mode);
        let path = walkaround(&world, at(0.0, 0.0), at(10.0, 0.0), &walk)
            .unwrap_or_else(|| panic!("{mode:?}: a way round"));
        assert!(path.len() > 2, "{mode:?}: not straight through the pad");
        let directions: Vec<(i64, i64)> = path
            .windows(2)
            .map(|leg| {
                (
                    (leg[1].x - leg[0].x).signum(),
                    (leg[1].y - leg[0].y).signum(),
                )
            })
            .collect();
        for turn in directions.windows(2) {
            let (a, b) = (turn[0], turn[1]);
            assert!(
                a.0 * b.0 + a.1 * b.1 > 0,
                "{mode:?}: a corner sharper than 45° in {path:?}"
            );
        }
    }
}
