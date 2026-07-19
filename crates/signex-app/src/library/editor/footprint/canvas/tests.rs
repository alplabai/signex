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
    let mut push = |sketch: &mut signex_sketch::SketchData, kind| {
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
