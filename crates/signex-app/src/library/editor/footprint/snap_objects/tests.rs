//! Each Objects for snapping switch the footprint snap reads (#2726): on,
//! the hotspot pulls; off, it does not. Snap Distance 0.203 mm and Axis
//! Snap Range 5.08 mm (the defaults).

use signex_library::PadShape;
use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};

use super::{BodyBox, ObjectTargets, axis_snap, object_snap};
use crate::library::editor::footprint::snap::{self, SnapKind};
use crate::library::editor::footprint::state::{
    EditorMode, FootprintEditorState, Guide, GuideAxis, SnapOptions,
};

const TOLERANCE: f64 = 1e-6;

/// A state whose origin does not snap, so a test sees only its own target.
fn state() -> FootprintEditorState {
    let mut state = FootprintEditorState::empty();
    state.snap_options.snap_footprint_origins = false;
    state
}

fn pad(state: &mut FootprintEditorState, at: (f64, f64), size: (f64, f64), shape: PadShape) {
    let idx = state.add_pad_at(at.0, at.1);
    state.pads[idx].size_mm = size;
    state.pads[idx].shape = shape;
}

fn snapped(
    state: &FootprintEditorState,
    targets: ObjectTargets<'_>,
    at: (f64, f64),
) -> Option<(f64, f64)> {
    object_snap(at, state, targets).map(|hit| {
        assert_eq!(hit.kind, SnapKind::Object);
        hit.pos
    })
}

fn assert_near(got: Option<(f64, f64)>, want: (f64, f64)) {
    let got = got.unwrap_or_else(|| panic!("no snap, want {want:?}"));
    assert!(
        (got.0 - want.0).abs() < TOLERANCE && (got.1 - want.1).abs() < TOLERANCE,
        "got {got:?}, want {want:?}"
    );
}

fn silk(kind: FpGraphicKind, filled: bool) -> FpGraphic {
    FpGraphic::drawn(kind, 0.15, filled)
}

#[test]
fn pad_vertices_pull_to_a_rect_pads_corner() {
    let mut s = state();
    pad(&mut s, (5.0, 5.0), (2.0, 1.0), PadShape::Rect);
    let at = (6.1, 5.6);
    assert_eq!(snapped(&s, ObjectTargets::default(), at), None);
    s.snap_options.snap_pad_vertices = true;
    assert_near(snapped(&s, ObjectTargets::default(), at), (6.0, 5.5));
}

#[test]
fn pad_vertices_follow_the_pads_rotation() {
    let mut s = state();
    pad(&mut s, (0.0, 0.0), (2.0, 2.0), PadShape::Rect);
    s.pads[0].rotation_deg = 45.0;
    s.snap_options.snap_pad_vertices = true;
    let corner = (0.0, 2.0_f64.sqrt());
    assert_near(
        snapped(&s, ObjectTargets::default(), (0.05, corner.1 + 0.05)),
        corner,
    );
}

#[test]
fn a_round_pad_has_no_vertices() {
    let mut s = state();
    pad(&mut s, (5.0, 5.0), (2.0, 2.0), PadShape::Round);
    s.snap_options.snap_pad_vertices = true;
    s.snap_options.snap_pad_centers = false;
    assert_eq!(snapped(&s, ObjectTargets::default(), (6.05, 5.0)), None);
}

#[test]
fn pad_edges_pull_to_the_nearest_point_of_the_outline() {
    let mut s = state();
    pad(&mut s, (5.0, 5.0), (2.0, 1.0), PadShape::Rect);
    let at = (6.05, 5.2);
    assert_eq!(snapped(&s, ObjectTargets::default(), at), None);
    s.snap_options.snap_pad_edges = true;
    assert_near(snapped(&s, ObjectTargets::default(), at), (6.0, 5.2));
}

#[test]
fn pad_edges_reach_a_round_pads_rim() {
    let mut s = state();
    pad(&mut s, (5.0, 5.0), (2.0, 2.0), PadShape::Round);
    s.snap_options.snap_pad_edges = true;
    assert_near(
        snapped(&s, ObjectTargets::default(), (6.1, 5.0)),
        (6.0, 5.0),
    );
}

#[test]
fn track_lines_pull_to_the_nearest_point_of_a_silk_line() {
    let mut s = state();
    let lines = [silk(
        FpGraphicKind::Line {
            from: [0.0, 10.0],
            to: [10.0, 10.0],
        },
        false,
    )];
    let targets = ObjectTargets {
        silk_f: &lines,
        ..ObjectTargets::default()
    };
    let at = (5.0, 10.1);
    assert_eq!(snapped(&s, targets, at), None);
    s.snap_options.snap_track_lines = true;
    assert_near(snapped(&s, targets, at), (5.0, 10.0));
}

#[test]
fn track_lines_follow_an_arc_within_its_sweep_only() {
    let mut s = state();
    s.snap_options.snap_arc_centers = false;
    s.snap_options.snap_track_lines = true;
    let arcs = [silk(
        FpGraphicKind::Arc {
            center: [20.0, 0.0],
            radius: 2.0,
            start_deg: 0.0,
            end_deg: 90.0,
        },
        false,
    )];
    let targets = ObjectTargets {
        silk_b: &arcs,
        ..ObjectTargets::default()
    };
    let on = 45.0_f64.to_radians();
    assert_near(
        snapped(&s, targets, (20.0 + 2.1 * on.cos(), 2.1 * on.sin())),
        (20.0 + 2.0 * on.cos(), 2.0 * on.sin()),
    );
    // Opposite the sweep the nearest arc point is an end, out of reach.
    assert_eq!(snapped(&s, targets, (17.9, 0.0)), None);
}

#[test]
fn texts_pull_to_a_silk_strings_anchor() {
    let mut s = state();
    let texts = [silk(
        FpGraphicKind::Text {
            position: [3.0, 3.0],
            content: "REF".into(),
            size: 1.0,
            frame: None,
        },
        false,
    )];
    let targets = ObjectTargets {
        silk_f: &texts,
        ..ObjectTargets::default()
    };
    assert_eq!(snapped(&s, targets, (3.1, 3.1)), None);
    s.snap_options.snap_texts = true;
    assert_near(snapped(&s, targets, (3.1, 3.1)), (3.0, 3.0));
}

#[test]
fn regions_pull_to_a_filled_polygons_edge_and_vertices() {
    let mut s = state();
    s.snap_options.snap_track_vertices = false;
    let square = |filled| {
        silk(
            FpGraphicKind::Polygon {
                vertices: vec![[10.0, 10.0], [14.0, 10.0], [14.0, 14.0], [10.0, 14.0]],
            },
            filled,
        )
    };
    let regions = [square(true)];
    let targets = ObjectTargets {
        silk_f: &regions,
        ..ObjectTargets::default()
    };
    assert_eq!(snapped(&s, targets, (12.0, 10.1)), None);
    s.snap_options.snap_regions = true;
    assert_near(snapped(&s, targets, (12.0, 10.1)), (12.0, 10.0));
    assert_near(snapped(&s, targets, (14.1, 14.1)), (14.0, 14.0));
    // An outline is no region.
    let outlines = [square(false)];
    let outline = ObjectTargets {
        silk_f: &outlines,
        ..ObjectTargets::default()
    };
    assert_eq!(snapped(&s, outline, (12.0, 10.1)), None);
}

#[test]
fn body_points_pull_to_the_3d_bodys_corners_centre_and_axes() {
    let mut s = state();
    let targets = ObjectTargets {
        body: Some(BodyBox {
            min: (-2.0, -1.0),
            max: (2.0, 1.0),
        }),
        ..ObjectTargets::default()
    };
    assert_eq!(snapped(&s, targets, (2.1, 1.1)), None);
    s.snap_options.snap_3d_body_points = true;
    assert_near(snapped(&s, targets, (2.1, 1.1)), (2.0, 1.0));
    assert_near(snapped(&s, targets, (1.0, 0.1)), (1.0, 0.0));
    // The centre, where the vertical axis meets the plane, is a fixed
    // point Axes lines up with.
    s.snap_options.snap_to_axes = true;
    assert_eq!(axis_snap((0.1, 3.0), &s, targets), (Some(0.0), None));
}

#[test]
fn a_footprints_body_box_is_the_one_the_3d_preview_draws() {
    let mut footprint = signex_library::Footprint::empty("body");
    footprint.body_3d.outline = Some(signex_library::primitive::footprint::Polygon::new(vec![
        [-1.0, -0.5],
        [3.0, -0.5],
        [3.0, 2.5],
    ]));
    let body = BodyBox::of(&footprint);
    assert_eq!(body.min, (-1.0, -0.5));
    assert_eq!(body.max, (3.0, 2.5));
    assert_eq!(body.centre(), (1.0, 1.0));
}

#[test]
fn axes_line_the_pointer_up_with_a_pad_centre_in_range() {
    let mut s = state();
    pad(&mut s, (5.0, 5.0), (1.0, 1.0), PadShape::Rect);
    let at = (5.1, 8.0);
    assert_eq!(axis_snap(at, &s, ObjectTargets::default()), (None, None));
    s.snap_options.snap_to_axes = true;
    assert_eq!(
        axis_snap(at, &s, ObjectTargets::default()),
        (Some(5.0), None)
    );
    // Through the chain the free axis takes the 1 mm grid.
    let hit = snap::snap_cursor_among(
        (5.1, 8.3),
        None,
        &s,
        None,
        true,
        Some(ObjectTargets::default()),
    );
    assert_eq!(hit.kind, SnapKind::Object);
    assert_near(Some(hit.pos), (5.0, 8.0));
}

#[test]
fn axis_snap_range_bounds_the_axes() {
    let mut s = state();
    pad(&mut s, (5.0, 5.0), (1.0, 1.0), PadShape::Rect);
    s.snap_options.snap_to_axes = true;
    s.snap_options.axis_snap_range_mm = 2.0;
    assert_eq!(
        axis_snap((5.1, 8.0), &s, ObjectTargets::default()),
        (None, None)
    );
    assert_eq!(
        axis_snap((5.1, 6.5), &s, ObjectTargets::default()),
        (Some(5.0), None)
    );
}

#[test]
fn guides_snap_by_default_and_the_guides_switch_turns_them_off() {
    assert!(SnapOptions::default().snap_to_guides);
    let mut s = state();
    s.guides.push(Guide {
        axis: GuideAxis::Vertical,
        position_mm: 3.3,
        enabled: true,
    });
    let hit = snap::snap_cursor((3.5, 7.4), None, &s, None);
    assert_eq!(hit.kind, SnapKind::Guide);
    assert_near(Some(hit.pos), (3.3, 7.4));
    s.snap_options.snap_to_guides = false;
    let hit = snap::snap_cursor((3.5, 7.4), None, &s, None);
    assert_eq!(hit.kind, SnapKind::Grid);
    assert_near(Some(hit.pos), (4.0, 7.0));
}

#[test]
fn a_carried_selection_is_no_target_of_its_own_drag() {
    let mut s = state();
    pad(&mut s, (0.0, 0.0), (1.0, 1.0), PadShape::Rect);
    pad(&mut s, (5.0, 0.0), (1.0, 1.0), PadShape::Rect);
    s.selected_pad = Some(0);
    s.selected_pads_extra = vec![1];
    let carried = ObjectTargets {
        moving_pad: Some(0),
        moving_selection: true,
        ..ObjectTargets::default()
    };
    assert_eq!(snapped(&s, carried, (5.1, 0.0)), None);
    let alone = ObjectTargets {
        moving_selection: false,
        ..carried
    };
    assert_near(snapped(&s, alone, (5.1, 0.0)), (5.0, 0.0));
}

#[test]
fn sketch_mode_snaps_to_what_the_sketch_does_not_draw() {
    let mut s = state();
    s.snap_options.snap_footprint_origins = true;
    s.snap_options.snap_3d_body_points = true;
    pad(&mut s, (5.0, 0.0), (1.0, 1.0), PadShape::Rect);
    pad(&mut s, (9.0, 0.0), (1.0, 1.0), PadShape::Rect);
    s.pads[1].sketch_entity_id = Some(signex_sketch::id::SketchEntityId::new());
    let hand = silk(
        FpGraphicKind::Line {
            from: [0.0, 4.0],
            to: [3.0, 4.0],
        },
        false,
    );
    let mut baked = silk(
        FpGraphicKind::Line {
            from: [0.0, 8.0],
            to: [3.0, 8.0],
        },
        false,
    );
    baked.sketch_entity = Some(signex_sketch::id::SketchEntityId::new());
    let graphics = [hand, baked];
    let targets = ObjectTargets {
        silk_f: &graphics,
        body: Some(BodyBox {
            min: (20.0, 20.0),
            max: (22.0, 22.0),
        }),
        ..ObjectTargets::default()
    };
    s.mode = EditorMode::Sketch;
    assert_near(snapped(&s, targets, (5.1, 0.0)), (5.0, 0.0));
    assert_near(snapped(&s, targets, (3.1, 4.0)), (3.0, 4.0));
    assert_eq!(snapped(&s, targets, (9.1, 0.0)), None, "a sketch pad");
    assert_eq!(snapped(&s, targets, (3.1, 8.0)), None, "baked silk");
    assert_eq!(snapped(&s, targets, (0.1, 0.0)), None, "the origin");
    assert_eq!(snapped(&s, targets, (22.1, 22.0)), None, "the body");
    s.mode = EditorMode::View3d;
    assert_eq!(snapped(&s, targets, (5.1, 0.0)), None);
}
