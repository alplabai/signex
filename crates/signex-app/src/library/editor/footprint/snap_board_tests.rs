//! Board-feature snaps of a board sketch (#2518, Graphite mockup row 179):
//! what the backdrop carries, what the cursor reaches, and that a
//! footprint sketch never reaches anything of a board.

use std::sync::Arc;

use signex_sketch::SketchData;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::snap::{SnapKind, snap_cursor};
use super::snap_board::{BoardFeatureKind, feature_name, nearest_feature, reach_mm};
use super::state::{
    BackdropCourtyard, BackdropPad, BoardBackdrop, BoardFeatureSnaps, FootprintEditorState,
    PadCopper,
};

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

fn pad(centre: (f64, f64), designator: &str) -> BackdropPad {
    BackdropPad {
        centre,
        size: (1.0, 0.5),
        rotation_deg: 0.0,
        round: false,
        copper: PadCopper::Top,
        designator: designator.into(),
    }
}

/// A 20 x 20 mm board with one pad of U1 at (10, 10).
fn backdrop() -> BoardBackdrop {
    BoardBackdrop {
        outline: vec![(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)],
        pads: vec![pad((10.0, 10.0), "U1-1")],
        holes: vec![((15.0, 15.0), 3.2)],
        courtyards: vec![BackdropCourtyard {
            reference: "J1".into(),
            points: vec![(2.0, 4.0), (6.0, 4.0), (6.0, 8.0), (2.0, 8.0)],
            closed: true,
        }],
        ..BoardBackdrop::default()
    }
}

/// A board sketch's editor state over [`backdrop`].
fn board_state() -> FootprintEditorState {
    let mut state = FootprintEditorState::empty();
    state.board_backdrop = Some(Arc::new(backdrop()));
    state
}

#[test]
fn near_a_pad_centre_the_cursor_lands_on_it() {
    let state = board_state();
    let r = snap_cursor((10.1, 10.05), None, &state, None);
    assert_eq!(r.kind, SnapKind::BoardFeature(BoardFeatureKind::Pad));
    assert!(close(r.pos, (10.0, 10.0)), "{:?}", r.pos);
}

#[test]
fn a_footprint_sketch_never_snaps_to_a_board_feature() {
    let mut state = board_state();
    state.board_backdrop = None;
    for raw in [(10.1, 10.05), (7.3, 0.1), (2.05, 4.05), (15.1, 15.0)] {
        let r = snap_cursor(raw, None, &state, None);
        assert!(
            !matches!(r.kind, SnapKind::BoardFeature(_)),
            "{raw:?} gave {:?}",
            r.kind
        );
    }
}

#[test]
fn board_features_off_snaps_to_none_of_them() {
    let mut state = board_state();
    state.snap_options.board_features.enabled = false;
    let r = snap_cursor((10.1, 10.05), None, &state, None);
    assert_eq!(r.kind, SnapKind::Grid);
}

#[test]
fn a_kind_turned_off_is_passed_over() {
    let mut state = board_state();
    state.snap_options.board_features.pads = false;
    let r = snap_cursor((10.1, 10.05), None, &state, None);
    assert_eq!(r.kind, SnapKind::Grid);
    let r = snap_cursor((15.1, 15.0), None, &state, None);
    assert_eq!(r.kind, SnapKind::BoardFeature(BoardFeatureKind::Hole));
}

#[test]
fn a_sketch_point_still_wins_over_a_board_feature() {
    let mut sketch = SketchData::default();
    let plane = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    sketch.planes.push(plane.clone());
    let id = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        id,
        plane.id,
        EntityKind::Point { x: 10.2, y: 10.0 },
    ));
    let state = board_state();
    let r = snap_cursor((10.1, 10.05), Some(&sketch), &state, Some(id));
    assert_eq!(r.kind, SnapKind::Point(id));
    assert!(close(r.pos, (10.2, 10.0)));
}

#[test]
fn the_reach_is_the_point_snap_radius_at_the_current_zoom() {
    let mut state = board_state();
    // 8 px at 30 px/mm is about 0.27 mm: 0.4 mm away is out of reach.
    let r = snap_cursor((10.4, 10.0), None, &state, None);
    assert_ne!(r.kind, SnapKind::BoardFeature(BoardFeatureKind::Pad));
    // Zoomed out to 10 px/mm the reach is 0.8 mm.
    state.view_px_per_mm = 10.0;
    let r = snap_cursor((10.4, 10.0), None, &state, None);
    assert_eq!(r.kind, SnapKind::BoardFeature(BoardFeatureKind::Pad));
    assert!(
        (reach_mm(0.0) - reach_mm(30.0)).abs() < 1e-12,
        "unknown zoom"
    );
}

#[test]
fn an_outline_edge_snaps_to_its_nearest_point_and_a_vertex_wins() {
    let state = board_state();
    let r = snap_cursor((7.3, 0.1), None, &state, None);
    assert_eq!(r.kind, SnapKind::BoardFeature(BoardFeatureKind::Outline));
    assert!(close(r.pos, (7.3, 0.0)), "{:?}", r.pos);
    let r = snap_cursor((20.1, 19.9), None, &state, None);
    assert!(close(r.pos, (20.0, 20.0)), "the corner, not the edge");
}

#[test]
fn a_courtyard_corner_is_reached_and_named() {
    let state = board_state();
    let r = snap_cursor((2.05, 4.05), None, &state, None);
    assert_eq!(r.kind, SnapKind::BoardFeature(BoardFeatureKind::Courtyard));
    assert!(close(r.pos, (2.0, 4.0)));
    let board = backdrop();
    assert_eq!(
        feature_name(&board, BoardFeatureKind::Courtyard, r.pos).as_deref(),
        Some("J1")
    );
    assert_eq!(
        feature_name(&board, BoardFeatureKind::Pad, (10.0, 10.0)).as_deref(),
        Some("U1-1")
    );
    assert_eq!(
        feature_name(&board, BoardFeatureKind::Outline, (0.0, 0.0)),
        None
    );
}

#[test]
fn a_through_hole_pad_reads_as_a_pad_rather_than_its_hole() {
    let mut board = backdrop();
    board.holes.push(((10.0, 10.0), 0.8));
    let hit = nearest_feature(&board, BoardFeatureSnaps::default(), (10.05, 10.0), 0.3);
    assert_eq!(hit.map(|h| h.1), Some(BoardFeatureKind::Pad));
}

/// A board with J1 at (5, 5): an SMD pad, a plated and a non-plated
/// through-hole pad, and a rectangle courtyard; plus a free SMD pad.
fn board() -> PcbBoard {
    let pad = |n: u128, number: &str, kind: &str, shape: &str, x: f64, y: f64, drill: f64| {
        let drill = (drill > 0.0).then(|| serde_json::json!({"diameter": drill}));
        serde_json::json!({
            "uuid": Uuid::from_u128(n), "number": number, "pad_type": kind,
            "shape": shape, "position": {"x": x, "y": y},
            "size": {"x": 1.0, "y": 0.5}, "drill": drill, "layers": ["TopCopper"]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "footprints": [{
            "uuid": Uuid::from_u128(2), "reference": "J1",
            "position": {"x": 5.0, "y": 5.0}, "layer": "TopCopper",
            "pads": [
                pad(3, "1", "smd", "rect", 1.0, 0.0, 0.0),
                pad(4, "2", "thru", "circle", -1.0, 0.0, 0.8),
                pad(5, "3", "np_thru", "circle", 0.0, 2.0, 1.0),
            ],
            "graphics": [
                {"graphic_type": "rect", "layer": "TopCourtyard",
                 "start": {"x": -2.0, "y": -1.0}, "end": {"x": 2.0, "y": 3.0}},
                {"graphic_type": "line", "layer": "TopSilkscreen",
                 "start": {"x": -2.0, "y": -1.0}, "end": {"x": 2.0, "y": -1.0}}
            ]
        }],
        "free": {"pads": [pad(6, "7", "smd", "rect", 12.0, 3.0, 0.0)]}
    }))
    .unwrap()
}

#[test]
fn from_board_carries_pads_holes_and_courtyards() {
    let b = BoardBackdrop::from_board(&board());
    let pads: Vec<(&str, (f64, f64), PadCopper)> = b
        .pads
        .iter()
        .map(|p| (p.designator.as_str(), p.centre, p.copper))
        .collect();
    assert_eq!(pads.len(), 3, "the NPTH pad has no copper: {pads:?}");
    assert_eq!((pads[0].0, pads[0].2), ("J1-1", PadCopper::Top));
    assert!(close(pads[0].1, (6.0, 5.0)));
    assert_eq!((pads[1].0, pads[1].2), ("J1-2", PadCopper::Through));
    assert!(close(pads[1].1, (4.0, 5.0)));
    assert!(b.pads[1].round);
    assert_eq!(pads[2].0, "7", "a free pad is named by its number");
    assert!(close(pads[2].1, (12.0, 3.0)));

    assert_eq!(b.holes.len(), 2);
    assert!(close(b.holes[0].0, (4.0, 5.0)) && b.holes[0].1 == 0.8);
    assert!(close(b.holes[1].0, (5.0, 7.0)) && b.holes[1].1 == 1.0);

    assert_eq!(b.courtyards.len(), 1, "silkscreen is not a courtyard");
    let courtyard = &b.courtyards[0];
    assert_eq!(courtyard.reference, "J1");
    assert!(courtyard.closed);
    let want = [(3.0, 4.0), (7.0, 4.0), (7.0, 8.0), (3.0, 8.0)];
    assert_eq!(courtyard.points.len(), 4);
    for (got, want) in courtyard.points.iter().zip(want) {
        assert!(close(*got, want), "{got:?} != {want:?}");
    }
    let (x0, y0, x1, y1) = b.bbox_mm().expect("a board with parts has a box");
    assert!(x0 <= 3.0 && y0 <= 3.0 && x1 >= 12.0 && y1 >= 8.0);
}

#[test]
fn a_bottom_side_pad_is_drawn_in_bottom_copper() {
    let mut board = board();
    board.footprints[0].layer = "BottomCopper".into();
    let b = BoardBackdrop::from_board(&board);
    assert_eq!(b.pads[0].copper, PadCopper::Bottom);
    // The bottom side mirrors x: pad 1 at local +1 lands at 5 - 1.
    assert!(
        close(b.pads[0].centre, (4.0, 5.0)),
        "{:?}",
        b.pads[0].centre
    );
}
