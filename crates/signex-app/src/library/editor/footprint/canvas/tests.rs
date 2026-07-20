//! v0.18.25.1 — regression tests for the silk hit-test edge cases
//! flagged by the v0.18.25 code review (H1 zero-sweep arc, M1
//! polygon near-horizontal edges).

use super::geometry::{point_in_polygon, point_to_segment_dist, polygon_outline_hit};
use super::silk_f_hit_at;
use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};

fn line(from: [f64; 2], to: [f64; 2]) -> FpGraphic {
    FpGraphic {
        kind: FpGraphicKind::Line { from, to },
        stroke_width: 0.0,
        filled: false,
    }
}

fn arc(center: [f64; 2], radius: f64, start_deg: f64, end_deg: f64) -> FpGraphic {
    FpGraphic {
        kind: FpGraphicKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        },
        stroke_width: 0.0,
        filled: false,
    }
}

#[test]
fn line_hit_on_segment() {
    let g = vec![line([0.0, 0.0], [10.0, 0.0])];
    assert_eq!(silk_f_hit_at(&g, 5.0, 0.05, 0.1), Some(0));
}

#[test]
fn line_miss_above_aabb_below_segment_distance() {
    let g = vec![line([0.0, 0.0], [10.0, 0.0])];
    assert_eq!(silk_f_hit_at(&g, 5.0, 0.5, 0.1), None);
}

#[test]
fn arc_zero_sweep_is_no_hit() {
    let g = vec![arc([0.0, 0.0], 5.0, 90.0, 90.0)];
    assert_eq!(silk_f_hit_at(&g, 5.0, 0.0, 0.1), None);
}

#[test]
fn arc_full_circle_via_360_sweep() {
    let g = vec![arc([0.0, 0.0], 5.0, 0.0, 360.0)];
    assert_eq!(silk_f_hit_at(&g, 5.0, 0.0, 0.1), Some(0));
    assert_eq!(silk_f_hit_at(&g, -5.0, 0.0, 0.1), Some(0));
    assert_eq!(silk_f_hit_at(&g, 0.0, 5.0, 0.1), Some(0));
}

#[test]
fn arc_seam_crossing_includes_zero_degrees() {
    let g = vec![arc([0.0, 0.0], 5.0, 350.0, 10.0)];
    assert_eq!(silk_f_hit_at(&g, 5.0, 0.0, 0.1), Some(0));
}

#[test]
fn arc_excludes_outside_sweep() {
    let g = vec![arc([0.0, 0.0], 5.0, 0.0, 90.0)];
    assert_eq!(silk_f_hit_at(&g, -5.0, 0.0, 0.1), None);
    let s = (5.0_f64) * (45.0_f64.to_radians()).cos();
    assert_eq!(silk_f_hit_at(&g, s, s, 0.1), Some(0));
}

#[test]
fn polygon_horizontal_edge_no_nan_propagation() {
    let square = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    assert!(point_in_polygon(5.0, 5.0, &square));
    assert!(!point_in_polygon(-1.0, 5.0, &square));
    assert!(!point_in_polygon(5.0, -1.0, &square));
    assert!(!point_in_polygon(11.0, 5.0, &square));
}

#[test]
fn polygon_outline_hit_on_edge() {
    let square = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    assert!(polygon_outline_hit(5.0, -0.05, &square, 0.1));
    assert!(!polygon_outline_hit(5.0, 5.0, &square, 0.1));
}

#[test]
fn point_to_segment_dist_zero_length() {
    let d = point_to_segment_dist(3.0, 4.0, 0.0, 0.0, 0.0, 0.0);
    assert!((d - 5.0).abs() < 1e-9);
}

// ─────────────────────────────────────────────────────────────────
// Arc hit-test — `EntityKind::Arc` carries no radius field, so the
// branch used to score raw distance-to-CENTRE. That made the arc
// grabbable by clicking its empty middle and un-grabbable by clicking
// the stroke the user can actually see. The radius is derivable from
// the start point; these two assertions fail in opposite directions
// without it.
// ─────────────────────────────────────────────────────────────────

/// Sketch with one arc of radius `r_mm` centred at the origin, plus
/// its centre / start / end Points. Returns the arc's id.
fn arc_sketch(r_mm: f64) -> (signex_sketch::SketchData, signex_sketch::id::SketchEntityId) {
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    let plane_id = PlaneId::new();
    let push = |sketch: &mut signex_sketch::SketchData, kind| {
        let id = SketchEntityId::new();
        sketch.entities.push(Entity::new(id, plane_id, kind));
        id
    };
    let mut sketch = signex_sketch::SketchData {
        planes: vec![Plane {
            id: plane_id,
            kind: PlaneKind::BoardTop,
        }],
        ..signex_sketch::SketchData::default()
    };
    let center = push(&mut sketch, EntityKind::Point { x: 0.0, y: 0.0 });
    let start = push(&mut sketch, EntityKind::Point { x: r_mm, y: 0.0 });
    let end = push(&mut sketch, EntityKind::Point { x: 0.0, y: r_mm });
    let arc = push(
        &mut sketch,
        EntityKind::Arc {
            center,
            start,
            end,
            sweep_ccw: true,
        },
    );
    (sketch, arc)
}

#[test]
fn arc_hit_test_grabs_the_stroke_not_the_centre() {
    use super::FootprintCanvasState;
    use super::hit_test::sketch_hit_other;

    // Default scale is 30 px/mm, so r = 1.5 mm ⇒ 45 px on screen —
    // far outside the 12 px snap radius, which is what makes the
    // centre-vs-edge distinction observable.
    let (sketch, arc_id) = arc_sketch(1.5);
    let cstate = FootprintCanvasState::default();

    assert_eq!(
        sketch_hit_other(Some(&sketch), &cstate, (1.5, 0.0)),
        Some(arc_id),
        "a click ON the arc stroke must hit the arc"
    );
    assert_eq!(
        sketch_hit_other(Some(&sketch), &cstate, (0.0, 0.0)),
        None,
        "a click at the arc's CENTRE is 45 px from the stroke and must not hit"
    );
}

#[test]
fn polygon_filled_silk_uses_even_odd() {
    let pac = vec![
        [0.0, 0.0],
        [10.0, 0.0],
        [10.0, 10.0],
        [6.0, 5.0],
        [10.0, 0.0 + 1e-12],
        [0.0, 10.0],
    ];
    let _ = point_in_polygon(5.0, 5.0, &pac);
}

/// #361 — "Drag Track End" endpoint-biased segment grab. These drive
/// the press/release arms directly (no Iced event loop) and assert on
/// the armed `DragState`, matching the input handlers' contract.
mod drag_track_end {
    use iced::widget::canvas;
    use iced::{Color, Point};

    use signex_sketch::SketchData;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    use crate::library::editor::footprint::state::{EditorMode, FootprintEditorState, SketchTool};
    use crate::library::state::EditorAddress;

    // `super::super` == the `canvas` module (this test file is
    // `canvas::tests`, so `drag_track_end` is `canvas::tests::…`).
    use super::super::{FootprintCanvas, FootprintCanvasState};

    /// A single horizontal line from (0,0) to (10,0). Returns the
    /// sketch plus the start / end Point ids and the Line id.
    fn line_sketch() -> (SketchData, SketchEntityId, SketchEntityId, SketchEntityId) {
        let mut sketch = SketchData::default();
        let plane = Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        };
        let pid = plane.id;
        sketch.planes.push(plane);
        let start = SketchEntityId::new();
        let end = SketchEntityId::new();
        let line = SketchEntityId::new();
        sketch.entities.push(Entity::new(
            start,
            pid,
            EntityKind::Point { x: 0.0, y: 0.0 },
        ));
        sketch
            .entities
            .push(Entity::new(end, pid, EntityKind::Point { x: 10.0, y: 0.0 }));
        sketch
            .entities
            .push(Entity::new(line, pid, EntityKind::Line { start, end }));
        (sketch, start, end, line)
    }

    fn state_with_tool(tool: SketchTool) -> FootprintEditorState {
        let mut s = FootprintEditorState::empty();
        s.mode = EditorMode::Sketch;
        s.active_tool = tool;
        s
    }

    fn make_canvas<'a>(
        state: &'a FootprintEditorState,
        sketch: &'a SketchData,
        cache: &'a canvas::Cache,
    ) -> FootprintCanvas<'a> {
        FootprintCanvas {
            state,
            address: EditorAddress::new(
                std::path::PathBuf::from("/tmp/lib.snxlib"),
                "footprints".to_string(),
                signex_library::RowId::new(),
            ),
            bg_color: Color::WHITE,
            grid_color: Color::BLACK,
            cache,
            sketch: Some(sketch),
            silk_f: &[],
            silk_b: &[],
        }
    }

    #[test]
    fn press_near_line_end_arms_nearer_endpoint_point_drag() {
        let (sketch, _start, end, _line) = line_sketch();
        let state = state_with_tool(SketchTool::DragTrackEnd);
        let cache = canvas::Cache::new();
        let fpc = make_canvas(&state, &sketch, &cache);
        let mut cstate = FootprintCanvasState::default();

        // Press ON the line but nearer the (10,0) end — well outside
        // the 12 px point-snap radius of either vertex.
        let world = (8.0, 0.0);
        let cursor = cstate.world_to_screen(world);
        let action = fpc.try_drag_track_end_grab(&mut cstate, cursor, world);

        assert!(action.is_some(), "grab must claim a press on the line");
        let drag = cstate.drag.expect("an endpoint drag must be armed");
        assert_eq!(
            drag.sketch_point,
            Some(end),
            "the NEARER endpoint (10,0 end) is grabbed"
        );
        assert_eq!(drag.sketch_line, None, "must NOT arm a whole-line drag");
        assert_eq!(drag.pad_idx, usize::MAX);
    }

    #[test]
    fn press_equidistant_resolves_to_start_deterministically() {
        let (sketch, start, _end, _line) = line_sketch();
        let state = state_with_tool(SketchTool::DragTrackEnd);
        let cache = canvas::Cache::new();
        let fpc = make_canvas(&state, &sketch, &cache);
        let mut cstate = FootprintCanvasState::default();

        // Exact midpoint — equidistant from both ends. `<` (not `<=`)
        // in the picker means a tie resolves to `start`.
        let world = (5.0, 0.0);
        let cursor = cstate.world_to_screen(world);
        let action = fpc.try_drag_track_end_grab(&mut cstate, cursor, world);

        assert!(action.is_some());
        let drag = cstate.drag.expect("an endpoint drag must be armed");
        assert_eq!(
            drag.sketch_point,
            Some(start),
            "an equidistant press resolves deterministically to `start`"
        );
        assert_eq!(drag.sketch_line, None);
    }

    /// Deterministic canvas transform (scale 10 px/mm, world origin at
    /// screen (100,300)) so a chosen world point maps to a screen cursor
    /// that lands inside the test bounds and inverts back exactly.
    fn seated_cstate() -> FootprintCanvasState {
        let mut cstate = FootprintCanvasState::default();
        cstate.scale = 10.0;
        cstate.offset = Point::new(100.0, 300.0);
        cstate
    }

    fn left_press_at(
        fpc: &FootprintCanvas<'_>,
        cstate: &mut FootprintCanvasState,
        world: (f64, f64),
    ) -> Option<canvas::Action<crate::library::messages::LibraryMessage>> {
        let bounds = iced::Rectangle::new(Point::ORIGIN, iced::Size::new(800.0, 600.0));
        let cursor = iced::mouse::Cursor::Available(cstate.world_to_screen(world));
        fpc.on_button_pressed(cstate, &iced::mouse::Button::Left, bounds, cursor)
    }

    #[test]
    fn disarmed_select_tool_still_drags_whole_line_via_dispatcher() {
        // Regression guard through the REAL press dispatcher
        // (`on_button_pressed` → `on_primary_pressed` walk order): with the
        // DragTrackEnd tool DISARMED (Select active), a press on the line
        // body arms the existing whole-line drag, exactly as before #361.
        let (sketch, _start, _end, line) = line_sketch();
        let state = state_with_tool(SketchTool::Select);
        let cache = canvas::Cache::new();
        let fpc = make_canvas(&state, &sketch, &cache);
        let mut cstate = seated_cstate();

        let action = left_press_at(&fpc, &mut cstate, (5.0, 0.0));
        assert!(action.is_some(), "the press claims the line");
        let drag = cstate.drag.expect("a whole-line drag must be armed");
        assert_eq!(drag.sketch_line, Some(line), "the whole line is grabbed");
        assert_eq!(drag.sketch_point, None, "not an endpoint drag");
    }

    #[test]
    fn armed_drag_track_end_wins_walk_order_via_dispatcher() {
        // Through the real press dispatcher: with DragTrackEnd armed, a
        // press on the line body arms the ENDPOINT drag on the nearer
        // endpoint — proving `try_drag_track_end_grab` runs before
        // `try_sketch_line_grab` in the walk order (not a whole-line drag).
        let (sketch, _start, end, _line) = line_sketch();
        let state = state_with_tool(SketchTool::DragTrackEnd);
        let cache = canvas::Cache::new();
        let fpc = make_canvas(&state, &sketch, &cache);
        let mut cstate = seated_cstate();

        let action = left_press_at(&fpc, &mut cstate, (8.0, 0.0));
        assert!(action.is_some(), "the press claims the line");
        let drag = cstate.drag.expect("an endpoint drag must be armed");
        assert_eq!(
            drag.sketch_point,
            Some(end),
            "nearer endpoint grabbed via the real dispatcher"
        );
        assert_eq!(
            drag.sketch_line, None,
            "walk order picked DragTrackEnd, not the whole-line grab"
        );
    }

    #[test]
    fn release_cleanly_ends_a_sketch_point_drag() {
        // The release path is reused unchanged: arming a `sketch_point`
        // drag then releasing must clear the drag and publish nothing
        // (per-tick MovePoint messages already streamed the motion).
        let (sketch, _start, end, _line) = line_sketch();
        let state = state_with_tool(SketchTool::DragTrackEnd);
        let cache = canvas::Cache::new();
        let fpc = make_canvas(&state, &sketch, &cache);
        let mut cstate = FootprintCanvasState::default();

        let world = (8.0, 0.0);
        let cursor = cstate.world_to_screen(world);
        assert!(
            fpc.try_drag_track_end_grab(&mut cstate, cursor, world)
                .is_some()
        );
        assert_eq!(cstate.drag.and_then(|d| d.sketch_point), Some(end));

        let bounds = iced::Rectangle::new(Point::ORIGIN, iced::Size::new(800.0, 600.0));
        let released = fpc.on_button_released(
            &mut cstate,
            &iced::mouse::Button::Left,
            bounds,
            iced::mouse::Cursor::Unavailable,
        );
        assert!(
            released.is_none(),
            "a sketch-point release publishes nothing"
        );
        assert!(cstate.drag.is_none(), "the release clears the drag state");
    }

    #[test]
    fn hit_test_uses_solved_positions_not_stale_authored_coords() {
        // #361 review (Finding #1): the line hit-test must follow the
        // SOLVER, not the authored Point coords. Build a line p1—p2, Fix p1
        // at the origin, then constrain |p1 p2| = 5 mm so the solver moves
        // p2 from its authored (1,0) to (5,0). The authored Point coords
        // stay (1,0) — only `last_solve` reflects the move — so a press at
        // (4,0) lies ON the SOLVED segment (0,0)–(5,0) but 3 mm PAST the
        // authored segment (0,0)–(1,0). The old raw-coords hit-test
        // (`sketch_hit_other`) missed it; the solve-aware one grabs p2.
        use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
        use signex_sketch::id::ConstraintId;

        let plane = Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        };
        let pid = plane.id;
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let line = SketchEntityId::new();
        let sketch = SketchData {
            planes: vec![plane],
            entities: vec![
                Entity::new(p1, pid, EntityKind::Point { x: 0.0, y: 0.0 }),
                Entity::new(p2, pid, EntityKind::Point { x: 1.0, y: 0.0 }),
                Entity::new(line, pid, EntityKind::Line { start: p1, end: p2 }),
            ],
            constraints: vec![Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::Fixed { point: p1 },
            }],
            ..SketchData::default()
        };
        let mut fp = signex_library::primitive::footprint::Footprint::empty("t");
        fp.sketch = Some(sketch);
        let mut state = FootprintEditorState::from_footprint(&fp);

        // The distance constraint drives the solve; it populates last_solve
        // and moves p2 to (5,0) without rewriting the authored Point coords.
        crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit(
            &mut state,
            &mut fp,
            crate::library::editor::footprint::sketch_mode::SketchEdit::AddConstraint(Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::DistancePtPt {
                    p1,
                    p2,
                    target: DimTarget::Literal(5.0),
                },
            }),
        )
        .unwrap();
        assert!(state.last_solve.is_some(), "the solve must have run");

        let solved_sketch = fp.sketch.clone().expect("sketch present after solve");
        let raw_p2_x = solved_sketch
            .entities
            .iter()
            .find(|e| e.id == p2)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, .. } => Some(x),
                _ => None,
            })
            .expect("p2 is a Point");
        assert!(
            (raw_p2_x - 1.0).abs() < 1e-9,
            "the authored p2 x stays stale at 1.0 — the divergence the fix handles"
        );

        state.mode = EditorMode::Sketch;
        state.active_tool = SketchTool::DragTrackEnd;
        let cache = canvas::Cache::new();
        let fpc = make_canvas(&state, &solved_sketch, &cache);
        let mut cstate = FootprintCanvasState::default();

        // (4,0): past the authored segment end, on the solved segment.
        let world = (4.0, 0.0);
        let cursor = cstate.world_to_screen(world);
        let action = fpc.try_drag_track_end_grab(&mut cstate, cursor, world);
        assert!(
            action.is_some(),
            "the solve-aware hit-test finds the rubber-banded line"
        );
        let drag = cstate.drag.expect("an endpoint drag must be armed");
        assert_eq!(
            drag.sketch_point,
            Some(p2),
            "grabs p2 — the endpoint nearer the click in SOLVED coords"
        );
        assert_eq!(drag.sketch_line, None, "an endpoint drag, not a line drag");
    }
}
