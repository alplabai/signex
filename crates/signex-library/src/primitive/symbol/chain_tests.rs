//! Tests for `chain::chain_into_closed_contour` and friends.
//!
//! Split out of `chain.rs` into this sibling file (per the
//! `crates/signex-app/src/keymap/editor_tests.rs` pattern:
//! `#[cfg(test)] mod chain_tests;` declared alongside `mod chain;` in
//! the parent `primitive/symbol/mod.rs`) to keep `chain.rs` under the
//! house 800-line file cap.
//!
//! Because this file is a *sibling* of `chain`, not a child module of
//! it, it only has access to `chain`'s public surface — exactly the
//! same constraint a real caller has. The `dist_sq`/`signed_area_x2`
//! assertion helpers below are independent re-implementations of the
//! tiny math `chain` also happens to use internally, not a reach into
//! its private internals: a bug in `chain`'s own copy can't quietly
//! cancel out against the same bug here.

use super::chain::{CHAIN_ARC_SAMPLES, ChainError, ChainSegment, chain_into_closed_contour};

fn line(from: [f64; 2], to: [f64; 2]) -> ChainSegment {
    ChainSegment::Line { from, to }
}

fn dist_sq(a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

/// Twice the signed polygon area (shoelace, standard math orientation —
/// positive = counter-clockwise).
fn signed_area_x2(ring: &[[f64; 2]]) -> f64 {
    let n = ring.len();
    let mut sum = 0.0;
    for i in 0..n {
        let [x0, y0] = ring[i];
        let [x1, y1] = ring[(i + 1) % n];
        sum += x0 * y1 - x1 * y0;
    }
    sum
}

fn assert_approx_eq(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "expected {a} ≈ {b}");
}

fn assert_point_approx_eq(a: [f64; 2], b: [f64; 2]) {
    assert!(dist_sq(a, b).sqrt() < 1e-9, "expected {a:?} ≈ {b:?}");
}

#[test]
fn square_from_four_lines_shuffled_and_reversed_closes_ccw() {
    // Arrange: the 4 sides of a 4x4 square, shuffled and some
    // reversed — chaining must not depend on input order/direction.
    let segments = [
        line([4.0, 0.0], [0.0, 0.0]), // bottom, reversed
        line([4.0, 4.0], [4.0, 0.0]), // right, reversed
        line([0.0, 0.0], [0.0, 4.0]), // left, forward
        line([0.0, 4.0], [4.0, 4.0]), // top, forward
    ];

    // Act
    let ring = chain_into_closed_contour(&segments).expect("square should close");

    // Assert
    assert_eq!(ring.len(), 4);
    let mut sorted = ring.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(sorted, vec![[0.0, 0.0], [0.0, 4.0], [4.0, 0.0], [4.0, 4.0]]);
    assert!(signed_area_x2(&ring) > 0.0, "must be wound CCW");
}

#[test]
fn cw_input_square_is_renormalised_to_ccw() {
    // Arrange: 4 sides walked in a CW order (up, right, down, left).
    let segments = [
        line([0.0, 0.0], [0.0, 4.0]),
        line([0.0, 4.0], [4.0, 4.0]),
        line([4.0, 4.0], [4.0, 0.0]),
        line([4.0, 0.0], [0.0, 0.0]),
    ];

    // Act
    let ring = chain_into_closed_contour(&segments).expect("square should close");

    // Assert: winding was flipped to CCW, area magnitude preserved.
    assert_eq!(ring.len(), 4);
    assert_approx_eq(signed_area_x2(&ring), 32.0);
}

#[test]
fn triangle_with_one_arc_side_closes_with_tessellated_arc_points() {
    // Arrange: two straight sides plus one arc side (270° sweep)
    // closing the loop back to the origin.
    let p0 = [0.0, 0.0];
    let p1 = [4.0, 0.0];
    let p2 = [4.0, 4.0];
    // Arc centered at [0, 4], radius 4: p2 sits at 0°, p0 sits at
    // 270° from that center, so start_deg=0, end_deg=270 chains
    // p2 -> p0 exactly.
    let segments = [
        line(p0, p1),
        line(p1, p2),
        ChainSegment::Arc {
            center: [0.0, 4.0],
            radius: 4.0,
            start_deg: 0.0,
            end_deg: 270.0,
        },
    ];

    // Act
    let ring = chain_into_closed_contour(&segments).expect("D-shape should close");

    // Assert: 3 straight corners + (CHAIN_ARC_SAMPLES - 1) arc
    // interior points, no duplicate seam points, closed and CCW.
    assert_eq!(ring.len(), 3 + (CHAIN_ARC_SAMPLES - 1));
    for w in ring.windows(2) {
        assert!(dist_sq(w[0], w[1]) > 1e-12, "no duplicate seam points");
    }
    assert!(signed_area_x2(&ring) > 0.0);
    // The 90° arc sample (bulging away from the two straight sides)
    // must be present among the vertices.
    assert!(ring.iter().any(|p| p[1] > 4.0 + 1e-9));
}

#[test]
fn wraparound_arc_through_zero_degrees_samples_lie_on_circle() {
    // Arrange: an Arc sweeping CCW from 330° through 0° to 30° (a
    // 60° sweep), closed by the straight chord between its two
    // endpoints — a "circular segment" / lens shape. Exercises the
    // `start_deg > end_deg` wraparound branch end-to-end through
    // the public API, not just via bulge direction.
    let center = [0.0, 0.0];
    let radius = 2.0;
    let point_at = |deg: f64| -> [f64; 2] {
        [
            center[0] + radius * deg.to_radians().cos(),
            center[1] + radius * deg.to_radians().sin(),
        ]
    };
    let segments = [
        ChainSegment::Arc {
            center,
            radius,
            start_deg: 330.0,
            end_deg: 30.0,
        },
        line(point_at(30.0), point_at(330.0)), // closing chord
    ];

    // Act
    let ring = chain_into_closed_contour(&segments).expect("lens shape should close");

    // Assert: the chord duplicates the arc's own two endpoints, so
    // the ring is exactly the arc's CHAIN_ARC_SAMPLES + 1 samples.
    assert_eq!(ring.len(), CHAIN_ARC_SAMPLES + 1);
    assert!(signed_area_x2(&ring) > 0.0, "must be wound CCW");

    // Pin the actual through-0° sample positions (not just "some
    // point is above the center") at 345°, 360°/0°, and 15° — the
    // three interior samples straddling the wraparound.
    for deg in [345.0_f64, 360.0, 375.0] {
        let expected = point_at(deg);
        let found = ring
            .iter()
            .find(|p| dist_sq(**p, expected) < 1e-12)
            .unwrap_or_else(|| {
                panic!("expected a tessellated sample near {expected:?} (angle {deg}°) in {ring:?}")
            });
        assert_approx_eq(dist_sq(*found, center).sqrt(), radius);
    }
}

#[test]
fn near_full_sweep_arc_closes_via_its_own_tiny_chord_gap() {
    // Arrange: a 359.9° sweep — real, hit-test-selectable geometry
    // whose chord (the straight-line distance between its own two
    // endpoints, ≈ 2·r·sin(0.05°) ≈ 0.0035 mm) is *smaller* than
    // CHAIN_ENDPOINT_EPSILON_MM even though its true length
    // (≈ 12.56 mm) is nowhere near degenerate. Gating rejection on the
    // chord used to reject this outright; gating on tessellated
    // polyline length instead lets it through, and its own tiny chord
    // gap then self-closes it into a ring during normal endpoint
    // clustering — no special-casing needed.
    let center = [0.0, 0.0];
    let radius = 2.0;
    let segments = [ChainSegment::Arc {
        center,
        radius,
        start_deg: 0.0,
        end_deg: 359.9,
    }];

    // Act
    let ring = chain_into_closed_contour(&segments).expect("near-full arc should self-close");

    // Assert: CHAIN_ARC_SAMPLES vertices (its own near-coincident
    // endpoints collapse at the wrap-around dedup), all on the circle,
    // wound CCW.
    assert_eq!(ring.len(), CHAIN_ARC_SAMPLES);
    for p in &ring {
        assert_approx_eq(dist_sq(*p, center).sqrt(), radius);
    }
    assert!(signed_area_x2(&ring) > 0.0);
}

#[test]
fn open_chain_three_sides_of_square_reports_gap() {
    // Arrange: bottom, right, top sides present; left side missing.
    let segments = [
        line([0.0, 0.0], [4.0, 0.0]),
        line([4.0, 0.0], [4.0, 4.0]),
        line([4.0, 4.0], [0.0, 4.0]),
    ];

    // Act
    let err = chain_into_closed_contour(&segments).unwrap_err();

    // Assert
    match err {
        ChainError::OpenChain { gap_mm, ends } => {
            assert_approx_eq(gap_mm, 4.0);
            // Loose ends are the two open corners of the missing left
            // side: (0,0) and (0,4), in either order.
            let mut sorted = ends.to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert_eq!(sorted, vec![[0.0, 0.0], [0.0, 4.0]]);
        }
        other => panic!("expected OpenChain, got {other:?}"),
    }
}

#[test]
fn t_junction_of_three_lines_reports_branching() {
    // Arrange: three segments all touching the origin.
    let segments = [
        line([0.0, 0.0], [2.0, 0.0]),
        line([0.0, 0.0], [0.0, 2.0]),
        line([0.0, 0.0], [-2.0, 0.0]),
    ];

    // Act
    let err = chain_into_closed_contour(&segments).unwrap_err();

    // Assert
    match err {
        ChainError::Branching { at } => assert_point_approx_eq(at, [0.0, 0.0]),
        other => panic!("expected Branching, got {other:?}"),
    }
}

#[test]
fn two_separate_triangles_report_disjoint() {
    // Arrange: two fully-closed, spatially separate triangles.
    let triangle_a = [
        line([0.0, 0.0], [2.0, 0.0]),
        line([2.0, 0.0], [1.0, 2.0]),
        line([1.0, 2.0], [0.0, 0.0]),
    ];
    let triangle_b = [
        line([10.0, 0.0], [12.0, 0.0]),
        line([12.0, 0.0], [11.0, 2.0]),
        line([11.0, 2.0], [10.0, 0.0]),
    ];
    let segments: Vec<ChainSegment> = triangle_a.into_iter().chain(triangle_b).collect();

    // Act
    let err = chain_into_closed_contour(&segments).unwrap_err();

    // Assert
    assert_eq!(err, ChainError::Disjoint);
}

#[test]
fn line_plus_its_own_reverse_is_degenerate() {
    // Arrange: two segments tracing the exact same edge back and
    // forth — a closed but zero-area 2-vertex loop. Neither segment
    // is individually sub-epsilon (each is 4 mm long), so this is a
    // whole-ring degeneracy, not a single degenerate segment.
    let segments = [line([0.0, 0.0], [4.0, 0.0]), line([4.0, 0.0], [0.0, 0.0])];

    // Act
    let err = chain_into_closed_contour(&segments).unwrap_err();

    // Assert
    assert_eq!(err, ChainError::DegenerateResult);
}

#[test]
fn sub_epsilon_stub_segment_is_rejected_with_its_index() {
    // Arrange: a clean triangle corner at (4,0), plus a near-zero-
    // length "stub" line hanging off that same corner (0.005 mm — a
    // real artifact size, e.g. a mis-drawn duplicate click). Before
    // the up-front sub-epsilon rejection, this stub's own two ends
    // both clustered onto the corner node, faking a 4-way Branching
    // there instead of being recognised as junk input.
    let segments = [
        line([0.0, 0.0], [4.0, 0.0]),
        line([4.0, 0.0], [4.0, 4.0]),
        line([4.0, 4.0], [0.0, 0.0]),
        line([4.0, 0.0], [4.005, 0.0]), // stub, 0.005 mm < epsilon — index 3
    ];

    // Act
    let err = chain_into_closed_contour(&segments).unwrap_err();

    // Assert: rejected as a degenerate *segment* (index pinned to the
    // stub), not misreported as Branching or an index-less DegenerateResult.
    assert_eq!(err, ChainError::DegenerateSegment { segment_index: 3 });
}

#[test]
fn endpoints_within_epsilon_still_chain() {
    // Arrange: a triangle where one joint is off by 0.005 mm — under
    // CHAIN_ENDPOINT_EPSILON_MM (0.01).
    let a = [0.0, 0.0];
    let b = [4.0, 0.0];
    let c = [2.0, 3.0];
    let a_near = [a[0] + 0.005, a[1]];
    let segments = [line(a, b), line(b, c), line(c, a_near)];

    // Act
    let ring = chain_into_closed_contour(&segments);

    // Assert
    assert!(ring.is_ok(), "sub-epsilon gap should still chain");
    assert_eq!(ring.unwrap().len(), 3);
}

#[test]
fn endpoints_beyond_epsilon_report_open_chain() {
    // Arrange: same triangle, but the joint is off by 0.02 mm — over
    // CHAIN_ENDPOINT_EPSILON_MM (0.01).
    let a = [0.0, 0.0];
    let b = [4.0, 0.0];
    let c = [2.0, 3.0];
    let a_far = [a[0] + 0.02, a[1]];
    let segments = [line(a, b), line(b, c), line(c, a_far)];

    // Act
    let err = chain_into_closed_contour(&segments).unwrap_err();

    // Assert
    match err {
        ChainError::OpenChain { gap_mm, ends } => {
            assert_approx_eq(gap_mm, 0.02);
            let mut sorted = ends.to_vec();
            sorted.sort_by(|p, q| p.partial_cmp(q).unwrap());
            assert_point_approx_eq(sorted[0], a);
            assert_point_approx_eq(sorted[1], a_far);
        }
        other => panic!("expected OpenChain, got {other:?}"),
    }
}

#[test]
fn zero_sweep_arc_alone_is_rejected_as_degenerate_segment() {
    // Arrange: start_deg == end_deg. Per the product decision (see
    // the module doc comment), this is a degenerate zero-length
    // segment, not an implicit full circle: every runtime consumer
    // of SymbolGraphicKind::Arc (hit_test.rs, arc.wgsl, the CPU
    // canvas draw path) treats start==end as a zero-sweep point, so
    // chaining must never materialise a circle the user never saw.
    // Its polyline length is exactly 0.0, so it's caught by the same
    // up-front rule as a near-zero-length Line stub.
    let segments = [ChainSegment::Arc {
        center: [1.0, 1.0],
        radius: 2.0,
        start_deg: 40.0,
        end_deg: 40.0,
    }];

    // Act
    let err = chain_into_closed_contour(&segments).unwrap_err();

    // Assert
    assert_eq!(err, ChainError::DegenerateSegment { segment_index: 0 });
}

#[test]
fn non_finite_line_coordinate_reports_invalid_input() {
    // Arrange
    let segments = [line([0.0, 0.0], [f64::NAN, 4.0])];

    // Act
    let err = chain_into_closed_contour(&segments).unwrap_err();

    // Assert
    assert_eq!(err, ChainError::InvalidInput { segment_index: 0 });
}

#[test]
fn non_finite_arc_radius_reports_invalid_input() {
    // Arrange
    let segments = [
        line([0.0, 0.0], [4.0, 0.0]),
        ChainSegment::Arc {
            center: [0.0, 0.0],
            radius: f64::INFINITY,
            start_deg: 0.0,
            end_deg: 90.0,
        },
    ];

    // Act
    let err = chain_into_closed_contour(&segments).unwrap_err();

    // Assert
    assert_eq!(err, ChainError::InvalidInput { segment_index: 1 });
}

#[test]
fn empty_input_reports_empty() {
    assert_eq!(
        chain_into_closed_contour(&[]).unwrap_err(),
        ChainError::Empty
    );
}
