//! Validation + the degenerate-input error taxonomy for `split_line`.
//! Every case here must return `Err` and leave `sketch` byte-for-byte
//! unchanged.

use super::{line_endpoints, line_sketch};
use crate::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use crate::attr::{CustomPadShape, PadAttr, PadShape};
use crate::constraint::{Constraint, ConstraintKind};
use crate::plane::PlaneId;
use crate::split::*;

// ─── t -> MIN_SEGMENT_LEN_MM (BLOCKER 2, round 2) ───

#[test]
fn ulp_absorbed_mid_point_is_rejected() {
    // Reviewer counterexample: t clears any sane parametric epsilon by
    // 2x, but the resulting mid coordinate is absorbed by the ulp of
    // 500.0 and lands bit-identical to `start`. line_a would be
    // exactly zero-length.
    let (mut sketch, line, ..) = line_sketch((500.0, 0.0), (500.000_001, 0.0));
    let before = sketch.clone();
    assert!(split_line(&mut sketch, line, 2e-9).is_err());
    assert_eq!(sketch, before);
}

#[test]
fn mid_too_close_to_endpoint_on_a_long_line_is_rejected() {
    // A long line where `t` alone looks nowhere near an endpoint in a
    // parametric sense, but the resulting mid coordinate is still
    // real-world sub-micron from `start`.
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (1000.0, 0.0));
    let before = sketch.clone();
    let err = split_line(&mut sketch, line, 1e-7).unwrap_err();
    assert_eq!(err, SplitError::TooCloseToEndpoint);
    assert_eq!(sketch, before);
}

#[test]
fn a_realistic_close_to_end_split_still_succeeds() {
    // Corollary check: MIN_SEGMENT_LEN_MM must not reject a split a
    // real click (issue #372 hit-tests at ~0.30 mm tolerance) could
    // plausibly have aimed for.
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
    let result = split_line(&mut sketch, line, 0.01).expect("a 0.1 mm-scale split must succeed");
    let (mx, _) = entity_point_xy(&sketch, result.mid_point).unwrap();
    assert!((mx - 0.1).abs() < 1e-9);
}

// ─── DegenerateLine vs TooCloseToEndpoint taxonomy (finding 4) ───
//
// A line shorter than 2x MIN_SEGMENT_LEN_MM cannot be split at ANY t
// (even t=0.5, the best case, leaves one half under the minimum) — it
// must be DegenerateLine, never TooCloseToEndpoint, whose doc promises
// a nearer-middle retry will succeed.

#[test]
fn line_between_min_and_2x_min_is_degenerate_at_every_t() {
    // Length = 1.5 * MIN_SEGMENT_LEN_MM: long enough to clear the old
    // (pre-fix) `< MIN_SEGMENT_LEN_MM` DegenerateLine check, but no `t`
    // can produce two halves both >= MIN_SEGMENT_LEN_MM.
    let len = 1.5 * MIN_SEGMENT_LEN_MM;
    for t in [0.1, 0.5, 0.9] {
        let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (len, 0.0));
        let before = sketch.clone();
        assert_eq!(
            split_line(&mut sketch, line, t),
            Err(SplitError::DegenerateLine),
            "t={t} on a {len}mm line must be DegenerateLine, not TooCloseToEndpoint \
             (no t could ever succeed on this line)"
        );
        assert_eq!(sketch, before);
    }
}

#[test]
fn line_just_over_2x_min_still_splits_at_its_midpoint() {
    // The boundary must not over-reject: a line just over 2x the
    // minimum CAN split at t=0.5. (Kept a hair above the exact 2x
    // boundary rather than bit-exact — the boundary math itself is
    // exercised by the `2.0 * MIN_SEGMENT_LEN_MM` check directly; this
    // guards the behavioural promise, not a specific rounding.)
    let len = 2.0 * MIN_SEGMENT_LEN_MM * 1.001;
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (len, 0.0));
    split_line(&mut sketch, line, 0.5).expect("just over 2x the minimum must split at its middle");
}

#[test]
fn too_close_to_endpoint_line_always_has_a_better_t() {
    // The taxonomy's promise: whenever TooCloseToEndpoint fires, the
    // SAME line has a different, better t that succeeds (validated by
    // the line being long enough — >= 2x MIN_SEGMENT_LEN_MM — to have
    // passed the DegenerateLine gate).
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (1000.0, 0.0));
    let bad_t = 1e-7; // lands within MIN_SEGMENT_LEN_MM of `start`.
    assert_eq!(
        split_line(&mut sketch.clone(), line, bad_t),
        Err(SplitError::TooCloseToEndpoint)
    );
    split_line(&mut sketch, line, 0.5).expect("t=0.5 on the same line must succeed");
}

// ─── Non-finite coordinates (finding 3) ───

#[test]
fn nan_endpoint_coordinate_is_rejected_not_silently_propagated() {
    // Repro: a NaN endpoint makes `mm_distance` return NaN, and
    // `NaN < MIN_SEGMENT_LEN_MM` is false — both the pre-fix
    // DegenerateLine (length) check and the TooCloseToEndpoint
    // (post-mid-point) check silently pass a NaN straight through.
    // Before the fix this returned `Ok(SplitResult)` with a mid Point
    // at x = NaN, poisoning the solver.
    //
    // Not `assert_eq!(sketch, before)` here: the input itself carries
    // a NaN coordinate, and NaN != NaN under `PartialEq`, so a
    // whole-sketch comparison would fail even on a correctly-untouched
    // sketch. Assert the untouched-ness directly instead.
    let (mut sketch, line, start, end) = line_sketch((0.0, 0.0), (f64::NAN, 0.0));
    let entity_count_before = sketch.entities.len();
    let err = split_line(&mut sketch, line, 0.5).unwrap_err();
    assert_eq!(
        err,
        SplitError::DegenerateLine,
        "a non-finite mid Point must be rejected, not returned as Ok"
    );
    assert_eq!(
        sketch.entities.len(),
        entity_count_before,
        "no entities may be minted on a rejected split"
    );
    assert_eq!(
        line_endpoints(&sketch, line),
        (start, end),
        "the original line's endpoints must be untouched"
    );
}

#[test]
fn infinite_endpoint_coordinate_is_rejected() {
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (f64::INFINITY, 0.0));
    let before = sketch.clone();
    assert_eq!(
        split_line(&mut sketch, line, 0.5),
        Err(SplitError::DegenerateLine)
    );
    assert_eq!(sketch, before);
}

// ─── Degenerate inputs — every one must return Err and leave
// `sketch` byte-for-byte unchanged. ───

#[test]
fn missing_line_id_returns_err() {
    let (mut sketch, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
    let before = sketch.clone();
    let bogus = SketchEntityId::new();
    assert_eq!(
        split_line(&mut sketch, bogus, 0.5),
        Err(SplitError::NotALine(bogus))
    );
    assert_eq!(sketch, before);
}

#[test]
fn wrong_kind_returns_err() {
    let (mut sketch, _line, start, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
    let before = sketch.clone();
    // `start` resolves to a Point, not a Line.
    assert_eq!(
        split_line(&mut sketch, start, 0.5),
        Err(SplitError::NotALine(start))
    );
    assert_eq!(sketch, before);
}

#[test]
fn zero_length_line_returns_err() {
    let (mut sketch, line, ..) = line_sketch((3.0, 3.0), (3.0, 3.0));
    let before = sketch.clone();
    assert_eq!(
        split_line(&mut sketch, line, 0.5),
        Err(SplitError::DegenerateLine)
    );
    assert_eq!(sketch, before);
}

#[test]
fn t_at_or_beyond_an_endpoint_returns_err() {
    for t in [0.0, 1.0, -0.1, 1.1, -5.0, 5.0] {
        let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
        let before = sketch.clone();
        assert_eq!(
            split_line(&mut sketch, line, t),
            Err(SplitError::InvalidParameter(t)),
            "t={t} must be rejected"
        );
        assert_eq!(sketch, before, "t={t} must leave sketch unchanged");
    }
}

#[test]
fn t_within_min_segment_len_of_an_endpoint_returns_err() {
    for t in [1e-12, 1.0 - 1e-12] {
        let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
        let before = sketch.clone();
        assert_eq!(
            split_line(&mut sketch, line, t),
            Err(SplitError::TooCloseToEndpoint),
            "t={t} must be rejected"
        );
        assert_eq!(sketch, before);
    }
}

#[test]
fn t_nan_or_infinite_returns_err() {
    for t in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
        let before = sketch.clone();
        // NaN != NaN under PartialEq, so match the variant rather than
        // assert_eq! against a NaN-carrying payload.
        let err = split_line(&mut sketch, line, t).unwrap_err();
        assert!(
            matches!(err, SplitError::InvalidParameter(_)),
            "t={t:?} must be InvalidParameter, got {err:?}"
        );
        assert_eq!(sketch, before);
    }
}

#[test]
fn unresolvable_endpoint_returns_err() {
    let plane = PlaneId::new();
    let start_id = SketchEntityId::new();
    let dangling_end = SketchEntityId::new(); // never inserted as an entity
    let line_id = SketchEntityId::new();
    let mut sketch = SketchData::default();
    sketch.entities.push(Entity::new(
        start_id,
        plane,
        EntityKind::Point { x: 0.0, y: 0.0 },
    ));
    sketch.entities.push(Entity::new(
        line_id,
        plane,
        EntityKind::Line {
            start: start_id,
            end: dangling_end,
        },
    ));
    let before = sketch.clone();
    assert_eq!(
        split_line(&mut sketch, line_id, 0.5),
        Err(SplitError::NotALine(line_id))
    );
    assert_eq!(sketch, before);
}

#[test]
fn failed_split_leaves_sketch_byte_identical() {
    // Belt-and-braces (finding 6): a rejected split must not touch
    // ANY mutation site `commit_split` reaches on the success path —
    // not just `entities`, but `constraints`, `arrays` (round-1
    // blocker), and a pad's Custom-shape profile-seed list nested
    // inside `Entity::pad` (round-2 blocker). Populating only
    // `constraints`, as this test used to, never exercised those.
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
    sketch.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line },
    });
    sketch.arrays.push(Array {
        id: ArrayId::new(),
        kind: ArrayKind::Linear {
            source: line,
            count_expr: "2".into(),
            dx_expr: "1".into(),
            dy_expr: "0".into(),
        },
        numbering: NumberingScheme::default(),
    });
    let plane = sketch.entities[0].plane;
    let pad_point = SketchEntityId::new();
    let mut pad_entity = Entity::new(pad_point, plane, EntityKind::Point { x: 20.0, y: 20.0 });
    pad_entity.pad = Some(PadAttr {
        number: "1".into(),
        shape: PadShape::Custom(CustomPadShape::SketchProfile { source: vec![line] }),
        size_x_expr: "1".into(),
        size_y_expr: "1".into(),
        ..PadAttr::default()
    });
    sketch.entities.push(pad_entity);

    let before = sketch.clone();
    assert!(split_line(&mut sketch, line, 1.5).is_err());
    assert_eq!(
        sketch, before,
        "a rejected split must not touch entities, constraints, arrays, or pad profile lists"
    );
}
