//! #2389 — nets routed side by side: a pair's centre line walks around
//! other nets' copper wide enough for both tracks, each member follows it
//! at its offset from its own start, and what lands keeps every clearance
//! as one undo step.

use pcb_geom::CopperItem;
use signex_drc::{DrcRules, ViolationKind};
use signex_types::corner::CornerRule;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::tests::{board, pad, track};
use super::*;
use crate::pcb::BoardEngine;

/// Net A's stub ends at (1, 0.5), net B's at (1, -0.5).
fn stubs(footprints: Vec<serde_json::Value>) -> PcbBoard {
    board(
        footprints,
        vec![
            track(21, (0.0, 0.5), (1.0, 0.5), 1),
            track(22, (0.0, -0.5), (1.0, -0.5), 2),
        ],
    )
}

fn members() -> Vec<BusMember> {
    vec![
        BusMember {
            net: 2,
            anchor: Point::new(1.0, -0.5),
        },
        BusMember {
            net: 1,
            anchor: Point::new(1.0, 0.5),
        },
    ]
}

const STYLE: BusStyle = BusStyle {
    layer: CopperLayer::Top,
    width_mm: 0.25,
    pitch_mm: 0.5,
};

/// Land `tracks` as a route draft: it must fit, keep every clearance and
/// be one undo step.
fn land(board: &PcbBoard, tracks: Vec<signex_types::pcb::Segment>, rules: &DrcRules) {
    let draft = RouteDraft {
        tracks,
        ..RouteDraft::default()
    };
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    assert_eq!(
        draft_fits(board, &draft, &clearance, rules.widest_clearance()),
        Ok(())
    );
    let before = serde_json::to_string(board).unwrap();
    let mut engine = BoardEngine::new(board.clone(), 1);
    engine.execute_batch(draft.commands()).expect("lands");
    let report = signex_drc::run(engine.board(), rules);
    let broken = report.count(ViolationKind::Clearance) + report.count(ViolationKind::Short);
    assert_eq!(broken, 0, "{:?}", report.violations);
    engine.undo().unwrap();
    assert_eq!(serde_json::to_string(engine.board()).unwrap(), before);
}

#[test]
fn a_pair_follows_its_centre_line_each_member_on_its_own_side() {
    let board = stubs(Vec::new());
    let start = bus_start(&members()).unwrap();
    assert_eq!(start, Point::new(1.0, 0.0));
    let centre = [start, Point::new(10.0, 0.0)];
    let mut n = 100;
    let tracks = bus_tracks(&board, &members(), &centre, &STYLE, || {
        n += 1;
        Uuid::from_u128(n)
    })
    .expect("tracks");
    // Each member: a join from its start, then its offset run.
    assert_eq!(tracks.len(), 4);
    let of = |net: u32| -> Vec<_> { tracks.iter().filter(|t| t.net == net).collect() };
    let (a, b) = (of(1), of(2));
    assert_eq!(a[0].start, Point::new(1.0, 0.5), "A starts at its stub");
    // Half a pitch off the centre line, plus half the few nm of rounding
    // room the pitch carries.
    assert!((a[1].start.y - 0.25).abs() < 1e-5, "A on the left: {a:?}");
    assert!((b[1].start.y + 0.25).abs() < 1e-5, "B on the right: {b:?}");
    assert!(
        a[1].start.y >= 0.25 && b[1].start.y <= -0.25,
        "never closer"
    );
    assert!((a[1].end.x - 10.0).abs() < 1e-9);
    assert!(
        tracks
            .iter()
            .all(|t| t.width == 0.25 && t.layer == "TopCopper")
    );
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    land(&board, tracks, &rules);
}

/// A gap set exactly at the rule clearance stays legal on a 45° leg, where
/// the members' corners are rounded to whole nm.
#[test]
fn a_gap_at_the_clearance_stays_legal_on_a_diagonal_leg() {
    let board = stubs(Vec::new());
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    // 0.25 mm tracks 0.2 mm apart: the board's clearance exactly.
    let tight = BusStyle {
        pitch_mm: 0.45,
        ..STYLE
    };
    let start = bus_start(&members()).unwrap();
    let centre = [start, Point::new(5.0, 4.0), Point::new(9.0, 4.0)];
    let tracks = bus_tracks(&board, &members(), &centre, &tight, Uuid::now_v7).expect("tracks");
    assert_eq!(tracks.len(), 6, "a join and two legs each");
    land(&board, tracks, &rules);
}

#[test]
fn a_pair_walks_around_an_other_net_pad_and_passes_the_rule_check() {
    let board = stubs(vec![pad(1, 6.0, 0.0, (3, "C"))]);
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let route = RouteRules {
        clearance: &clearance,
        reach: rules.widest_clearance(),
        corner: CornerRule::default(),
        mode: RouteMode::Walkaround,
        grid_mm: 0.5,
        budget: SEARCH_BUDGET,
    };
    let from = Point::new(1.0, 0.0);
    let to = Point::new(11.0, 0.0);
    let leg = route_bus_leg(&board, &members(), from, to, &STYLE, &route).expect("a way round");
    assert!(leg.len() >= 2, "not straight through the pad: {leg:?}");
    assert_eq!(leg.last(), Some(&to));
    let mut centre = vec![from];
    centre.extend(leg);
    let tracks = bus_tracks(&board, &members(), &centre, &STYLE, Uuid::now_v7).expect("tracks");
    land(&board, tracks, &rules);
}

/// #2492: a member whose offset run lies along a track of its own net
/// lays only the stretches that track does not cover, joined to its ends.
#[test]
fn a_bus_member_along_its_own_track_lays_only_the_rest() {
    let mut board = stubs(Vec::new());
    let own: signex_types::pcb::Segment =
        serde_json::from_value(track(23, (3.0, 0.25), (6.0, 0.25), 1)).unwrap();
    board.segments.push(own.clone());
    let centre = [bus_start(&members()).unwrap(), Point::new(10.0, 0.0)];
    let tracks = bus_tracks(&board, &members(), &centre, &STYLE, Uuid::now_v7).expect("tracks");
    let a: Vec<_> = tracks.iter().filter(|t| t.net == 1).collect();
    assert_eq!(a.len(), 3, "the join and the run either side: {a:?}");
    assert_eq!(a[1].end, own.start, "the run ends on the track");
    assert_eq!(a[2].start, own.end, "and starts again at its far end");
    assert!((a[2].end.x - 10.0).abs() < 1e-9);
    assert_eq!(
        tracks.iter().filter(|t| t.net == 2).count(),
        2,
        "B is whole"
    );
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    land(&board, tracks, &rules);
}

#[test]
fn a_pitch_with_no_room_for_the_width_is_refused() {
    let board = stubs(Vec::new());
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let route = RouteRules {
        clearance: &clearance,
        reach: rules.widest_clearance(),
        corner: CornerRule::default(),
        mode: RouteMode::Walkaround,
        grid_mm: 0.5,
        budget: SEARCH_BUDGET,
    };
    let tight = BusStyle {
        pitch_mm: 0.2,
        ..STYLE
    };
    let refused = route_bus_leg(
        &board,
        &members(),
        Point::new(1.0, 0.0),
        Point::new(5.0, 0.0),
        &tight,
        &route,
    );
    assert_eq!(refused, Err(RouteRefusal::Unrepresentable));
}

#[test]
fn the_nearest_point_of_a_net_is_a_track_end_or_a_pad_centre() {
    let board = stubs(vec![pad(1, 5.0, 3.0, (1, "A"))]);
    let near = |at| nearest_net_point(&board, 1, CopperLayer::Top, at);
    assert_eq!(near(Point::new(1.2, 0.2)), Some(Point::new(1.0, 0.5)));
    assert_eq!(near(Point::new(5.0, 2.5)), Some(Point::new(5.0, 3.0)));
    assert_eq!(
        nearest_net_point(&board, 1, CopperLayer::Bottom, Point::new(0.0, 0.0)),
        None,
        "no copper of the net on that layer"
    );
}
