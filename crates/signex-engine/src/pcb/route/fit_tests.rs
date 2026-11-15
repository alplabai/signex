//! #2389 — what a route keeps beyond its own legs: a pushed track stays
//! clear of the via a layer change adds, a written rule scoped to a pair
//! of object kinds holds, and a draft is checked again against a board
//! that changed under it before it lands.

use pcb_geom::CopperItem;
use serde_json::json;
use signex_drc::{CopperObject, DrcRules, Limit, NetScope, UserRule};
use signex_types::corner::CornerRule;
use signex_types::pcb::{Point, Via};
use signex_types::pcb_layer::CopperLayer;
use signex_types::violation::Severity;
use uuid::Uuid;

use super::tests::{A, B, board, land_and_check, pad, request, style, track};
use super::*;
use crate::pcb::draw::new_via;

#[test]
fn a_pushed_track_keeps_clear_of_the_via_a_layer_change_adds() {
    // Net B's bottom run beside the leg's vertical part. Pushed left, out
    // of that leg's way, it would come within the clearance of the 1.7 mm
    // via at the leg's start.
    let mut run = track(11, (0.7, 1.0), (0.7, 3.0), 2);
    run["layer"] = json!("BottomCopper");
    let board = board(Vec::new(), vec![run]);
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let route = RouteRules {
        clearance: &clearance,
        reach: rules.widest_clearance(),
        corner: CornerRule::default(),
        mode: RouteMode::Shove,
        grid_mm: 0.5,
        budget: SEARCH_BUDGET,
    };
    let via = Via {
        net: 1,
        diameter: 1.7,
        drill: 0.8,
        ..new_via(&board, Uuid::from_u128(9), Point::new(0.0, 0.0))
    };
    let changing_layer = LegRequest {
        style: style(CopperLayer::Bottom),
        via: Some(via),
        ..request((0.0, 0.0), (1.0, 5.0))
    };
    let routed = route_leg(
        &board,
        &RouteDraft::default(),
        changing_layer,
        &route,
        Uuid::now_v7,
    )
    .expect("a way there");
    assert!(
        routed.shoved.is_empty(),
        "pushing the run would crowd the via: {:?}",
        routed.shoved
    );
    assert_eq!(
        routed.tracks[0].end,
        Point::new(0.0, 5.0),
        "the flipped bend, clear of the run"
    );
    let mut draft = RouteDraft::default();
    draft.add(routed);
    land_and_check(&board, &draft, &rules);
}

#[test]
fn a_written_rule_between_tracks_and_pads_is_kept() {
    // Net B's pad 0.575 mm off a straight leg: clear of the 0.2 mm board
    // rule, not of the 1 mm written between tracks and pads.
    let board = board(
        vec![
            pad(1, 0.0, 0.0, A),
            pad(2, 10.0, 0.0, A),
            pad(3, 5.0, 1.2, B),
        ],
        Vec::new(),
    );
    let track_pad = UserRule {
        name: "track_pad".into(),
        severity: Severity::Error,
        message: None,
        nets: NetScope::Board,
        layers: None,
        pair: Some([CopperObject::Track, CopperObject::Pad]),
        limits: vec![Limit::MinClearance(1_000_000)],
    };
    let rules =
        DrcRules::for_board(&board, &[])
            .unwrap()
            .with_user_rules(&board, &[], vec![track_pad]);
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let route = RouteRules {
        clearance: &clearance,
        reach: rules.widest_clearance(),
        corner: CornerRule::default(),
        mode: RouteMode::Walkaround,
        grid_mm: 0.5,
        budget: SEARCH_BUDGET,
    };
    let leg = route_leg(
        &board,
        &RouteDraft::default(),
        request((0.0, 0.0), (10.0, 0.0)),
        &route,
        Uuid::now_v7,
    )
    .expect("a way round");
    assert!(
        leg.tracks.len() >= 3,
        "not straight past the pad: {:?}",
        leg.tracks
    );
    let mut draft = RouteDraft::default();
    draft.add(leg);
    land_and_check(&board, &draft, &rules);
}

#[test]
fn a_draft_is_checked_again_against_a_board_that_changed_under_it() {
    let board = board(
        vec![
            pad(1, 0.0, 0.0, A),
            pad(2, 10.0, 0.0, A),
            pad(3, 2.0, 5.0, B),
            pad(4, 8.0, 5.0, B),
        ],
        vec![
            track(11, (2.0, 0.3), (8.0, 0.3), 2),
            track(12, (2.0, 0.3), (2.0, 5.0), 2),
            track(13, (8.0, 5.0), (8.0, 0.3), 2),
        ],
    );
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let reach = rules.widest_clearance();
    let route = RouteRules {
        clearance: &clearance,
        reach,
        corner: CornerRule::default(),
        mode: RouteMode::Shove,
        grid_mm: 0.5,
        budget: SEARCH_BUDGET,
    };
    let pushed = route_leg(
        &board,
        &RouteDraft::default(),
        request((0.0, 0.0), (10.0, 0.0)),
        &route,
        Uuid::now_v7,
    )
    .expect("the run is pushed");
    assert_eq!(
        pushed.shoved_from.len(),
        3,
        "the run and its risers as they were"
    );
    let mut draft = RouteDraft::default();
    draft.add(pushed);
    let fits = |board: &PcbBoard| draft_fits(board, &draft, &clearance, reach);
    assert_eq!(fits(&board), Ok(()), "the board it was routed on");

    let run = Uuid::from_u128(11);
    let mut gone = board.clone();
    gone.segments.retain(|s| s.uuid != run);
    assert_eq!(fits(&gone), Err(RouteRefusal::BoardChanged), "run removed");

    let mut edited = board.clone();
    if let Some(s) = edited.segments.iter_mut().find(|s| s.uuid == run) {
        s.width = 0.3;
    }
    assert_eq!(fits(&edited), Err(RouteRefusal::BoardChanged), "run edited");

    let other_net_via = |y: f64| Via {
        net: 2,
        ..new_via(&board, Uuid::from_u128(30), Point::new(5.0, y))
    };
    let mut crowded = board.clone();
    crowded.vias.push(other_net_via(-0.3));
    assert_eq!(
        fits(&crowded),
        Err(RouteRefusal::Blocked),
        "copper in the way"
    );

    let mut elsewhere = board.clone();
    elsewhere.vias.push(other_net_via(-10.0));
    assert_eq!(fits(&elsewhere), Ok(()), "an edit away from the route");
}
