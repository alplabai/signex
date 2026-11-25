//! #2389 — interactive routing in the engine: a leg goes around an
//! other-net pad, or pushes an other-net track aside, and what lands on the
//! board passes the design rule check as one undo step.

use pcb_geom::CopperItem;
use serde_json::json;
use signex_drc::{DrcRules, ViolationKind};
use signex_types::corner::CornerRule;
use signex_types::pcb::{PcbBoard, Point, Segment};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;
use crate::pcb::draw::new_via;

pub(super) const A: (u32, &str) = (1, "A");
pub(super) const B: (u32, &str) = (2, "B");

/// A footprint holding one round 1 mm top pad at (`x`, `y`).
pub(super) fn pad(n: u128, x: f64, y: f64, net: (u32, &str)) -> serde_json::Value {
    json!({
        "uuid": Uuid::from_u128(0x100 + n), "reference": format!("P{n}"), "value": "",
        "footprint_id": "Pad", "layer": "TopCopper", "rotation": 0.0,
        "position": {"x": x, "y": y},
        "pads": [{
            "uuid": Uuid::from_u128(0x200 + n), "number": "1", "pad_type": "smd",
            "shape": "circle", "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
            "drill": null, "layers": ["TopCopper"],
            "net": {"number": net.0, "name": net.1}
        }]
    })
}

/// A 0.25 mm top track.
pub(super) fn track(n: u128, a: (f64, f64), b: (f64, f64), net: u32) -> serde_json::Value {
    json!({
        "uuid": Uuid::from_u128(n), "start": {"x": a.0, "y": a.1}, "end": {"x": b.0, "y": b.1},
        "width": 0.25, "layer": "TopCopper", "net": net
    })
}

/// Nets A (1) and B (2), a 0.5 mm grid and a 0.2 mm clearance.
pub(super) fn board(
    footprints: Vec<serde_json::Value>,
    segments: Vec<serde_json::Value>,
) -> PcbBoard {
    serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [{"number": 1, "name": "A"}, {"number": 2, "name": "B"}],
        "footprints": footprints,
        "segments": segments
    }))
    .unwrap()
}

pub(super) fn style(layer: CopperLayer) -> TrackStyle {
    TrackStyle {
        layer,
        width_mm: 0.25,
        net: 1,
    }
}

pub(super) fn request(from: (f64, f64), to: (f64, f64)) -> LegRequest {
    LegRequest {
        from: Point::new(from.0, from.1),
        to: Point::new(to.0, to.1),
        style: style(CopperLayer::Top),
        via: None,
    }
}

/// Land `draft` on `board` and run the design rule check on the result;
/// one undo must give `board` back.
pub(super) fn land_and_check(board: &PcbBoard, draft: &RouteDraft, rules: &DrcRules) -> PcbBoard {
    let before = serde_json::to_string(board).unwrap();
    let mut engine = BoardEngine::new(board.clone(), 1);
    engine
        .execute_batch(draft.commands())
        .expect("the draft lands");
    let routed = engine.board().clone();
    let report = signex_drc::run(&routed, rules);
    assert!(report.is_clean(), "{:?}", report.violations);
    engine.undo().unwrap();
    assert_eq!(
        serde_json::to_string(engine.board()).unwrap(),
        before,
        "the route is one undo step"
    );
    routed
}

#[test]
fn a_leg_goes_around_an_other_net_pad_and_passes_the_rule_check() {
    let board = board(
        vec![
            pad(1, 0.0, 0.0, A),
            pad(2, 10.0, 0.0, A),
            pad(3, 5.0, 0.0, B),
        ],
        Vec::new(),
    );
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
    let draft = RouteDraft::default();
    let leg = route_leg(
        &board,
        &draft,
        request((0.0, 0.0), (10.0, 0.0)),
        &route,
        Uuid::now_v7,
    )
    .expect("a way round");
    assert!(
        leg.tracks.len() >= 3,
        "not straight through: {:?}",
        leg.tracks
    );
    assert_eq!(leg.tracks[0].start, Point::new(0.0, 0.0));
    assert_eq!(
        leg.tracks.last().map(|t| t.end),
        Some(Point::new(10.0, 0.0))
    );
    assert!(
        leg.tracks
            .iter()
            .all(|t| t.net == 1 && t.layer == "TopCopper")
    );
    assert!(leg.shoved.is_empty(), "walkaround moves nothing");
    let mut draft = draft;
    draft.add(leg);
    land_and_check(&board, &draft, &rules);
}

#[test]
fn shove_pushes_an_other_net_track_aside_and_keeps_it_legal() {
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
    let start = signex_drc::run(&board, &rules);
    assert_eq!(
        start.count(ViolationKind::Clearance) + start.count(ViolationKind::Short),
        0,
        "the board starts legal"
    );
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let mut route = RouteRules {
        clearance: &clearance,
        reach: rules.widest_clearance(),
        corner: CornerRule::default(),
        mode: RouteMode::Walkaround,
        grid_mm: 0.5,
        budget: SEARCH_BUDGET,
    };
    let leg = |route: &RouteRules| {
        route_leg(
            &board,
            &RouteDraft::default(),
            request((0.0, 0.0), (10.0, 0.0)),
            route,
            Uuid::now_v7,
        )
    };
    let around = leg(&route).expect("walkaround finds a way");
    assert!(around.shoved.is_empty() && around.tracks.len() > 1);

    route.mode = RouteMode::Shove;
    let pushed = leg(&route).expect("the run is pushed");
    assert_eq!(pushed.tracks.len(), 1, "straight through");
    assert_eq!(pushed.shoved.len(), 3, "the run and the two risers");
    let run = moved(&pushed, 11);
    assert!(run.start.y > 0.45 && run.start.y < 0.451, "{run:?}");
    assert_eq!(moved(&pushed, 12).start, run.start, "the riser follows");
    assert_eq!(
        moved(&pushed, 12).end,
        Point::new(2.0, 5.0),
        "its far end stays"
    );
    assert_eq!(moved(&pushed, 13).end, run.end);
    let mut draft = RouteDraft::default();
    draft.add(pushed);
    land_and_check(&board, &draft, &rules);
}

fn moved(leg: &RoutedLeg, id: u128) -> &Segment {
    leg.shoved
        .iter()
        .find(|s| s.uuid == Uuid::from_u128(id))
        .expect("pushed")
}

#[test]
fn a_via_that_does_not_keep_the_clearance_is_refused() {
    let board = board(vec![pad(1, 0.0, 0.9, B)], Vec::new());
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
    let via = Via {
        net: 1,
        ..new_via(&board, Uuid::from_u128(9), Point::new(0.0, 0.0))
    };
    let changing_layer = LegRequest {
        style: style(CopperLayer::Bottom),
        via: Some(via),
        ..request((0.0, 0.0), (5.0, 0.0))
    };
    let refused = route_leg(
        &board,
        &RouteDraft::default(),
        changing_layer,
        &route,
        Uuid::now_v7,
    );
    assert_eq!(refused, Err(RouteRefusal::ViaBlocked));
}

#[test]
fn a_track_pushed_twice_keeps_its_latest_place_in_the_draft() {
    let first = serde_json::from_value::<Segment>(track(5, (0.0, 1.0), (4.0, 1.0), 2)).unwrap();
    let second = Segment {
        start: Point::new(0.0, 2.0),
        ..first.clone()
    };
    let mut draft = RouteDraft::default();
    draft.add(RoutedLeg {
        shoved: vec![first],
        ..RoutedLeg::default()
    });
    draft.add(RoutedLeg {
        shoved: vec![second.clone()],
        ..RoutedLeg::default()
    });
    assert_eq!(draft.shoved, vec![second]);
}

/// A through pad 3 mm wide on Top and 1 mm elsewhere: a Bottom leg ending
/// outside its Bottom copper needs a via, one ending on it does not.
#[test]
fn an_end_via_reads_the_pads_size_on_the_legs_layer() {
    use signex_types::pcb::{LayerSize, LayerSizes, PadType};
    let mut board = board(vec![pad(1, 0.0, 0.0, A)], Vec::new());
    let through = &mut board.footprints[0].pads[0];
    through.pad_type = PadType::Thru;
    through.layer_sizes = Some(LayerSizes::Full {
        layers: vec![LayerSize::new(CopperLayer::Top, Point::new(3.0, 3.0))],
    });
    let mut leg = style(CopperLayer::Bottom);
    let via = |at| new_via(&board, Uuid::from_u128(9), at);
    assert!(end_via(&board, Point::new(1.0, 0.0), &mut leg, via).is_some());
    assert!(end_via(&board, Point::new(0.1, 0.0), &mut leg, via).is_none());
}

/// #2719: each conflict mode has its own token, read back in any case.
#[test]
fn route_mode_tokens_round_trip() {
    for mode in RouteMode::ALL {
        assert_eq!(RouteMode::from_token(mode.token()), Some(mode));
        let upper = mode.token().to_ascii_uppercase();
        assert_eq!(RouteMode::from_token(&upper), Some(mode));
        assert_eq!(mode.next().next(), mode);
    }
    assert_eq!(RouteMode::from_token("shove"), None);
}
