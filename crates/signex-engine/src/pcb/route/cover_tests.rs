//! #2492: a routed track keeps only what same-net tracks of its layer do
//! not already cover, so a route never lays duplicate copper.

use signex_types::corner::CornerRule;
use pcb_geom::CopperItem;
use signex_drc::DrcRules;
use signex_types::pcb::Segment;
use uuid::Uuid;

use super::cover::uncovered;
use super::tests::{board, request, track};
use super::*;

fn segment(n: u128, a: (f64, f64), b: (f64, f64), net: u32) -> Segment {
    serde_json::from_value(track(n, a, b, net)).unwrap()
}

fn ends(tracks: &[Segment]) -> Vec<((f64, f64), (f64, f64))> {
    tracks
        .iter()
        .map(|t| ((t.start.x, t.start.y), (t.end.x, t.end.y)))
        .collect()
}

fn trimmed(laid: &[Segment]) -> Vec<Segment> {
    let mut next = 100;
    let mut ids = || {
        next += 1;
        Uuid::from_u128(next)
    };
    uncovered(segment(1, (0.0, 0.0), (10.0, 0.0), 1), laid, &mut ids)
}

#[test]
fn a_cover_in_the_middle_leaves_the_two_ends_joined_to_it() {
    let pieces = trimmed(&[segment(2, (6.0, 0.0), (3.0, 0.0), 1)]);
    assert_eq!(ends(&pieces), [((0.0, 0.0), (3.0, 0.0)), ((6.0, 0.0), (10.0, 0.0))]);
    assert_eq!(pieces[0].uuid, Uuid::from_u128(1), "the first keeps the id");
    assert_eq!(pieces[1].uuid, Uuid::from_u128(101));
}

#[test]
fn covers_past_either_end_and_overlapping_each_other_merge() {
    let pieces = trimmed(&[
        segment(2, (-2.0, 0.0), (2.0, 0.0), 1),
        segment(3, (1.0, 0.0), (4.0, 0.0), 1),
        segment(4, (8.0, 0.0), (12.0, 0.0), 1),
    ]);
    assert_eq!(ends(&pieces), [((4.0, 0.0), (8.0, 0.0))]);
}

#[test]
fn a_track_covered_whole_is_dropped() {
    assert!(trimmed(&[segment(2, (10.0, 0.0), (0.0, 0.0), 1)]).is_empty());
}

#[test]
fn what_does_not_share_a_stretch_of_its_net_and_layer_leaves_it_whole() {
    let mut bottom = segment(6, (0.0, 0.0), (10.0, 0.0), 1);
    bottom.layer = "BottomCopper".into();
    let laid = [
        segment(2, (10.0, 0.0), (14.0, 0.0), 1),
        segment(3, (0.0, 0.5), (10.0, 0.5), 1),
        segment(4, (0.0, 0.0), (10.0, 0.0), 2),
        segment(5, (5.0, -3.0), (5.0, 3.0), 1),
        bottom,
    ];
    assert_eq!(trimmed(&laid), [segment(1, (0.0, 0.0), (10.0, 0.0), 1)]);
}

fn walk_rules<'a>(clearance: &'a ClearanceRule<'a>, rules: &DrcRules) -> RouteRules<'a> {
    RouteRules {
        clearance,
        reach: rules.widest_clearance(),
        corner: CornerRule::default(),
        mode: RouteMode::Walkaround,
        grid_mm: 0.5,
        budget: SEARCH_BUDGET,
    }
}

/// The soak's case: a leg that runs along a same-net track already on the
/// board laid a second track over it; now it lays only the rest.
#[test]
fn a_leg_along_a_same_net_track_lays_only_the_rest() {
    let board = board(Vec::new(), vec![track(11, (0.0, 0.0), (3.0, 0.0), 1)]);
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let route = walk_rules(&clearance, &rules);
    let leg = route_leg(
        &board,
        &RouteDraft::default(),
        request((0.0, 0.0), (5.0, 0.0)),
        &route,
        Uuid::now_v7,
    )
    .expect("a way");
    assert_eq!(ends(&leg.tracks), [((3.0, 0.0), (5.0, 0.0))]);
}

/// A second leg that turns back along the first lays nothing over it.
#[test]
fn a_leg_back_along_the_draft_lays_nothing_over_it() {
    let board = board(Vec::new(), Vec::new());
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let route = walk_rules(&clearance, &rules);
    let mut draft = RouteDraft::default();
    let first = route_leg(
        &board,
        &draft,
        request((0.0, 0.0), (6.0, 0.0)),
        &route,
        Uuid::now_v7,
    )
    .expect("the first leg");
    draft.add(first);
    let back = route_leg(
        &board,
        &draft,
        request((6.0, 0.0), (2.0, 0.0)),
        &route,
        Uuid::now_v7,
    )
    .expect("the second leg");
    assert!(back.tracks.is_empty(), "{:?}", back.tracks);
}
