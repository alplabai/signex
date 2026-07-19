use super::*;
use crate::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use crate::attr::{CustomPadShape, PadAttr, PadShape, PasteAperturePattern, SilkAttr};
use crate::constraint::{Constraint, ConstraintKind, DimTarget};
use crate::plane::PlaneId;
use signex_types::layer::SignexLayer;

/// A single Line `start -> end` plus its two Point entities.
/// Returns `(sketch, line, start, end)`.
fn line_sketch(
    start: (f64, f64),
    end: (f64, f64),
) -> (SketchData, SketchEntityId, SketchEntityId, SketchEntityId) {
    let plane = PlaneId::new();
    let start_id = SketchEntityId::new();
    let end_id = SketchEntityId::new();
    let line_id = SketchEntityId::new();
    let mut sketch = SketchData::default();
    sketch.entities.push(Entity::new(
        start_id,
        plane,
        EntityKind::Point {
            x: start.0,
            y: start.1,
        },
    ));
    sketch.entities.push(Entity::new(
        end_id,
        plane,
        EntityKind::Point { x: end.0, y: end.1 },
    ));
    sketch.entities.push(Entity::new(
        line_id,
        plane,
        EntityKind::Line {
            start: start_id,
            end: end_id,
        },
    ));
    (sketch, line_id, start_id, end_id)
}

fn line_endpoints(sketch: &SketchData, line: SketchEntityId) -> (SketchEntityId, SketchEntityId) {
    match sketch.entities.iter().find(|e| e.id == line).unwrap().kind {
        EntityKind::Line { start, end } => (start, end),
        _ => panic!("not a line"),
    }
}

// ─── Plain split ───

#[test]
fn mid_split_creates_two_lines_and_drops_original() {
    let (mut sketch, line, start, end) = line_sketch((0.0, 0.0), (10.0, 0.0));
    let result = split_line(&mut sketch, line, 0.5).expect("mid split must succeed");

    assert!(
        sketch.entities.iter().all(|e| e.id != line),
        "original line must be gone"
    );
    assert!(sketch.entities.iter().any(|e| e.id == result.mid_point));
    assert!(sketch.entities.iter().any(|e| e.id == result.line_a));
    assert!(sketch.entities.iter().any(|e| e.id == result.line_b));

    assert_eq!(
        line_endpoints(&sketch, result.line_a),
        (start, result.mid_point)
    );
    assert_eq!(
        line_endpoints(&sketch, result.line_b),
        (result.mid_point, end)
    );

    let (mx, my) = entity_point_xy(&sketch, result.mid_point).unwrap();
    assert!(
        (mx - 5.0).abs() < 1e-9,
        "mid x should interpolate to 5.0, got {mx}"
    );
    assert!(
        (my - 0.0).abs() < 1e-9,
        "mid y should interpolate to 0.0, got {my}"
    );
    assert!(result.dropped_constraints.is_empty());
}

#[test]
fn split_at_non_half_t_interpolates_correctly() {
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 20.0));
    let result = split_line(&mut sketch, line, 0.25).unwrap();
    let (mx, my) = entity_point_xy(&sketch, result.mid_point).unwrap();
    assert!((mx - 2.5).abs() < 1e-9);
    assert!((my - 5.0).abs() < 1e-9);
}

#[test]
fn endpoints_shared_not_duplicated() {
    let (mut sketch, line, start, end) = line_sketch((0.0, 0.0), (4.0, 0.0));
    let result = split_line(&mut sketch, line, 0.5).unwrap();

    let mid_count = sketch
        .entities
        .iter()
        .filter(|e| e.id == result.mid_point)
        .count();
    assert_eq!(mid_count, 1, "mid point must exist exactly once");

    // Total entity count: 2 original points + 1 new mid point + 2 new lines.
    assert_eq!(sketch.entities.len(), 5);
    assert!(sketch.entities.iter().any(|e| e.id == start));
    assert!(sketch.entities.iter().any(|e| e.id == end));
}

#[test]
fn bake_attributes_and_flags_carry_onto_both_halves() {
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
    {
        let e = sketch.entities.iter_mut().find(|e| e.id == line).unwrap();
        e.construction = true;
        e.silk = Some(SilkAttr {
            layer: SignexLayer::TopSilk,
        });
    }
    let result = split_line(&mut sketch, line, 0.5).unwrap();
    for id in [result.line_a, result.line_b] {
        let e = sketch.entities.iter().find(|e| e.id == id).unwrap();
        assert!(e.construction, "construction flag must carry over to {id}");
        assert!(e.silk.is_some(), "silk attribute must carry over to {id}");
    }
}

// ─── Constraint carry-over ───

#[test]
fn horizontal_constraint_duplicates_onto_both_halves() {
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
    let original_cid = ConstraintId::new();
    sketch.constraints.push(Constraint {
        id: original_cid,
        kind: ConstraintKind::Horizontal { line },
    });

    let result = split_line(&mut sketch, line, 0.5).unwrap();

    let horiz: Vec<&Constraint> = sketch
        .constraints
        .iter()
        .filter(|c| matches!(c.kind, ConstraintKind::Horizontal { .. }))
        .collect();
    assert_eq!(horiz.len(), 2, "Horizontal must duplicate onto both halves");
    let lines: Vec<SketchEntityId> = horiz
        .iter()
        .map(|c| match c.kind {
            ConstraintKind::Horizontal { line } => line,
            _ => unreachable!(),
        })
        .collect();
    assert!(lines.contains(&result.line_a));
    assert!(lines.contains(&result.line_b));
    assert!(
        sketch.constraints.iter().all(|c| !references_line(c, line)),
        "no surviving constraint may reference the retired line id"
    );

    // DOF budget: the split minted exactly one new Point (2 fresh
    // DOF: x, y). The duplicated Horizontal pair spends exactly 2
    // residuals (1 each) — it must not exceed the new DOF budget.
    let residuals: usize = horiz.iter().map(|c| c.kind.residual_count()).sum();
    assert_eq!(
        residuals, 2,
        "duplicated Horizontal pair must spend exactly the 2 new DOF"
    );
}

#[test]
fn equal_length_constraint_repoints_to_one_half_only() {
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
    let (other_sketch, other_line, _, _) = line_sketch((0.0, 5.0), (3.0, 5.0));
    sketch.entities.extend(other_sketch.entities);
    let original_cid = ConstraintId::new();
    sketch.constraints.push(Constraint {
        id: original_cid,
        kind: ConstraintKind::EqualLength {
            l1: line,
            l2: other_line,
        },
    });

    let result = split_line(&mut sketch, line, 0.5).unwrap();

    let matches: Vec<&Constraint> = sketch
        .constraints
        .iter()
        .filter(|c| matches!(c.kind, ConstraintKind::EqualLength { .. }))
        .collect();
    assert_eq!(matches.len(), 1, "EqualLength must not duplicate");
    match matches[0].kind {
        ConstraintKind::EqualLength { l1, l2 } => {
            assert_eq!(l1, result.line_a, "must repoint to line_a, not line_b");
            assert_eq!(
                l2, other_line,
                "the untouched partner line must be unchanged"
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(
        matches[0].id, original_cid,
        "id is preserved for the single surviving copy"
    );
    assert!(sketch.constraints.iter().all(|c| !references_line(c, line)));
    assert!(
        sketch
            .constraints
            .iter()
            .all(|c| !references_line(c, result.line_b)),
        "EqualLength must not have been duplicated onto line_b"
    );
}

#[test]
fn distance_pt_pt_on_endpoints_is_untouched() {
    let (mut sketch, line, start, end) = line_sketch((0.0, 0.0), (10.0, 0.0));
    let dist_cid = ConstraintId::new();
    let dist_constraint = Constraint {
        id: dist_cid,
        kind: ConstraintKind::DistancePtPt {
            p1: start,
            p2: end,
            target: DimTarget::Literal(10.0),
        },
    };
    sketch.constraints.push(dist_constraint.clone());

    split_line(&mut sketch, line, 0.5).unwrap();

    let survivors: Vec<&Constraint> = sketch
        .constraints
        .iter()
        .filter(|c| c.id == dist_cid)
        .collect();
    assert_eq!(survivors.len(), 1);
    assert_eq!(
        *survivors[0], dist_constraint,
        "DistancePtPt on endpoints must be byte-identical"
    );
}

#[test]
fn point_on_line_repoints_to_the_half_the_point_falls_on() {
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
    let plane = sketch.entities[0].plane;
    let near_start = SketchEntityId::new();
    let near_end = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        near_start,
        plane,
        EntityKind::Point { x: 2.0, y: 0.0 },
    ));
    sketch.entities.push(Entity::new(
        near_end,
        plane,
        EntityKind::Point { x: 8.0, y: 0.0 },
    ));
    sketch.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnLine {
            point: near_start,
            line,
        },
    });
    sketch.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnLine {
            point: near_end,
            line,
        },
    });

    // Split at t=0.5 (x=5.0): near_start (x=2) falls on line_a,
    // near_end (x=8) falls on line_b.
    let result = split_line(&mut sketch, line, 0.5).unwrap();

    let find_target = |point: SketchEntityId| -> SketchEntityId {
        sketch
            .constraints
            .iter()
            .find_map(|c| match c.kind {
                ConstraintKind::PointOnLine { point: p, line } if p == point => Some(line),
                _ => None,
            })
            .expect("constraint must survive")
    };
    assert_eq!(find_target(near_start), result.line_a);
    assert_eq!(find_target(near_end), result.line_b);
}

#[test]
fn midpoint_constraint_on_retired_line_is_dropped_not_relocated() {
    // Reviewer repro: point at x=5.0 with Midpoint{point,line} on a
    // 0->10 line, split at t=0.25. Re-pointing to line_a (0->2.5)
    // would drag the user's point to x=1.25 on the next solve — the
    // constraint must be dropped instead.
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
    let plane = sketch.entities[0].plane;
    let user_point = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        user_point,
        plane,
        EntityKind::Point { x: 5.0, y: 0.0 },
    ));
    let midpoint_cid = ConstraintId::new();
    sketch.constraints.push(Constraint {
        id: midpoint_cid,
        kind: ConstraintKind::Midpoint {
            point: user_point,
            line,
        },
    });

    let result = split_line(&mut sketch, line, 0.25).unwrap();

    assert!(
        sketch.constraints.iter().all(|c| c.id != midpoint_cid),
        "Midpoint on the retired line must not survive"
    );
    assert_eq!(
        result.dropped_constraints,
        vec![midpoint_cid],
        "the dropped id must be reported to the caller"
    );
    assert!(
        sketch.constraints.iter().all(|c| !matches!(
            &c.kind,
            ConstraintKind::Midpoint { line, .. }
                if *line == result.line_a || *line == result.line_b
        )),
        "must not have been re-pointed onto either half"
    );
}

#[test]
fn unrelated_midpoint_constraint_survives_untouched() {
    // A Midpoint naming a DIFFERENT line must not be touched by
    // splitting this one.
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
    let (other_sketch, other_line, ..) = line_sketch((0.0, 5.0), (4.0, 5.0));
    sketch.entities.extend(other_sketch.entities);
    let plane = sketch.entities[0].plane;
    let other_mid = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        other_mid,
        plane,
        EntityKind::Point { x: 2.0, y: 5.0 },
    ));
    let midpoint_cid = ConstraintId::new();
    let midpoint_constraint = Constraint {
        id: midpoint_cid,
        kind: ConstraintKind::Midpoint {
            point: other_mid,
            line: other_line,
        },
    };
    sketch.constraints.push(midpoint_constraint.clone());

    let result = split_line(&mut sketch, line, 0.5).unwrap();

    assert!(result.dropped_constraints.is_empty());
    assert!(
        sketch.constraints.contains(&midpoint_constraint),
        "unrelated Midpoint must be byte-identical"
    );
}

fn references_line(c: &Constraint, line: SketchEntityId) -> bool {
    use ConstraintKind::*;
    match &c.kind {
        PointOnLine { line: l, .. }
        | Horizontal { line: l }
        | Vertical { line: l }
        | DistancePtLine { line: l, .. }
        | TangentLineArc { line: l, .. }
        | SymmetricAboutLine { line: l, .. }
        | Midpoint { line: l, .. } => *l == line,
        Parallel { l1, l2 }
        | Perpendicular { l1, l2 }
        | Angle { l1, l2, .. }
        | EqualLength { l1, l2 } => *l1 == line || *l2 == line,
        _ => false,
    }
}

// ─── Non-constraint id-bearing collections (BLOCKER 1) ───

#[test]
fn array_source_and_polar_center_retarget_to_line_a() {
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
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
    sketch.arrays.push(Array {
        id: ArrayId::new(),
        kind: ArrayKind::Grid {
            source: line,
            nx_expr: "2".into(),
            ny_expr: "2".into(),
            dx_expr: "1".into(),
            dy_expr: "1".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::default(),
    });
    sketch.arrays.push(Array {
        id: ArrayId::new(),
        kind: ArrayKind::Polar {
            source: line,
            center: line,
            count_expr: "4".into(),
            sweep_angle_expr: "360".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::default(),
    });

    let result = split_line(&mut sketch, line, 0.5).unwrap();

    assert_eq!(sketch.arrays.len(), 3, "arrays must survive the split");
    for array in &sketch.arrays {
        match &array.kind {
            ArrayKind::Linear { source, .. } | ArrayKind::Grid { source, .. } => {
                assert_eq!(*source, result.line_a, "source must retarget to line_a");
            }
            ArrayKind::Polar { source, center, .. } => {
                assert_eq!(*source, result.line_a, "Polar source must retarget");
                assert_eq!(*center, result.line_a, "Polar center must retarget");
            }
        }
    }
}

#[test]
fn custom_pad_shape_profile_source_retargets_to_line_a() {
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
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

    let result = split_line(&mut sketch, line, 0.5).unwrap();

    let pad = sketch.entities.iter().find(|e| e.id == pad_point).unwrap();
    match &pad.pad.as_ref().unwrap().shape {
        PadShape::Custom(CustomPadShape::SketchProfile { source }) => {
            assert_eq!(
                source,
                &vec![result.line_a],
                "profile seed must retarget to line_a"
            );
        }
        other => panic!("shape must still be Custom::SketchProfile, got {other:?}"),
    }
}

#[test]
fn paste_aperture_custom_source_retargets_to_line_a() {
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
    let plane = sketch.entities[0].plane;
    let pad_point = SketchEntityId::new();
    let mut pad_entity = Entity::new(pad_point, plane, EntityKind::Point { x: 20.0, y: 20.0 });
    pad_entity.pad = Some(PadAttr {
        number: "1".into(),
        size_x_expr: "1".into(),
        size_y_expr: "1".into(),
        paste_apertures: PasteAperturePattern::Custom { source: vec![line] },
        ..PadAttr::default()
    });
    sketch.entities.push(pad_entity);

    let result = split_line(&mut sketch, line, 0.5).unwrap();

    let pad = sketch.entities.iter().find(|e| e.id == pad_point).unwrap();
    match &pad.pad.as_ref().unwrap().paste_apertures {
        PasteAperturePattern::Custom { source } => {
            assert_eq!(
                source,
                &vec![result.line_a],
                "paste-aperture seed must retarget to line_a"
            );
        }
        other => panic!("paste_apertures must still be Custom, got {other:?}"),
    }
}

// ─── T_EPS -> MIN_SEGMENT_LEN_MM (BLOCKER 2) ───

#[test]
fn ulp_absorbed_mid_point_is_rejected() {
    // Reviewer counterexample: t clears any sane parametric epsilon by
    // 2x, but the resulting mid coordinate is absorbed by the ulp of
    // 500.0 and lands bit-identical to `start` — line_a would be
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
    // Belt-and-braces: a constraint present pre-call must survive
    // untouched too, not just the entity list.
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
    sketch.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line },
    });
    let before = sketch.clone();
    assert!(split_line(&mut sketch, line, 1.5).is_err());
    assert_eq!(
        sketch, before,
        "a rejected split must not touch entities or constraints"
    );
}

// ─── Solver acceptance criterion (BLOCKER 3 / issue #360) ───

/// Perturbed axis-aligned rectangle — `p1` Fixed at the origin, width
/// 10 mm, height 5 mm — plus the constraint set that pins it there:
/// Horizontal on the bottom/top edges, Vertical on the left/right
/// edges, and one `DistancePtPt` per axis. 6 residuals over 6 free
/// scalars (`p1` is Fixed and excluded from the state vector) — the
/// system is exactly determined, so it converges to a unique solution
/// near the perturbed initial guess rather than trivially no-op-ing.
/// Returns `(sketch, p1, p2, p3, p4, l1)`; `l1` (the bottom edge,
/// `p1 -> p2`) is the one the test below splits.
fn perturbed_rectangle() -> (
    SketchData,
    SketchEntityId,
    SketchEntityId,
    SketchEntityId,
    SketchEntityId,
    SketchEntityId,
) {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(crate::plane::Plane {
        id: plane,
        kind: crate::plane::PlaneKind::BoardTop,
    });

    let pts = [(0.0, 0.0), (9.8, 0.15), (9.85, 4.8), (0.2, 5.15)];
    let ids: Vec<SketchEntityId> = pts
        .iter()
        .map(|&(x, y)| {
            let id = SketchEntityId::new();
            sketch
                .entities
                .push(Entity::new(id, plane, EntityKind::Point { x, y }));
            id
        })
        .collect();
    let (p1, p2, p3, p4) = (ids[0], ids[1], ids[2], ids[3]);

    let edges = [(p1, p2), (p2, p3), (p3, p4), (p4, p1)];
    let line_ids: Vec<SketchEntityId> = edges
        .iter()
        .map(|&(start, end)| {
            let id = SketchEntityId::new();
            sketch
                .entities
                .push(Entity::new(id, plane, EntityKind::Line { start, end }));
            id
        })
        .collect();
    let (l1, l2, l3, l4) = (line_ids[0], line_ids[1], line_ids[2], line_ids[3]);

    use ConstraintKind::*;
    let kinds = [
        Fixed { point: p1 },
        Horizontal { line: l1 },
        Vertical { line: l2 },
        Horizontal { line: l3 },
        Vertical { line: l4 },
        DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(10.0),
        },
        DistancePtPt {
            p1,
            p2: p4,
            target: DimTarget::Literal(5.0),
        },
    ];
    for kind in kinds {
        sketch.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind,
        });
    }

    (sketch, p1, p2, p3, p4, l1)
}

#[test]
fn split_then_solve_leaves_rectangle_visually_unchanged() {
    use crate::solver::Solver;
    use crate::solver::residual::ResolvedParams;
    use crate::solver::state::point_xy;

    let (mut sketch, p1, p2, p3, p4, l1) = perturbed_rectangle();
    let solver = Solver::default();
    let params = ResolvedParams::new();
    const TOL: f64 = 1e-6;

    // Baseline solve, and commit the solved positions back onto the
    // entities — split_line reads raw entity coordinates, not solver
    // state, so a caller must commit before splitting (mirrors what
    // the app does after a solve).
    let solved = solver
        .solve(&sketch, &params)
        .expect("baseline solve must converge");
    let expect_xy = [
        (p1, (0.0, 0.0)),
        (p2, (10.0, 0.0)),
        (p3, (10.0, 5.0)),
        (p4, (0.0, 5.0)),
    ];
    for (id, (ex, ey)) in expect_xy {
        let (x, y) = point_xy(id, &solved.result.state, &solved.result.index, &sketch).unwrap();
        assert!(
            (x - ex).abs() < TOL && (y - ey).abs() < TOL,
            "baseline solve did not converge {id} to ({ex},{ey}), got ({x},{y})"
        );
        sketch
            .entities
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .kind = EntityKind::Point { x, y };
    }

    // Split the bottom edge off-centre, then knock the new mid point's
    // AND p2's y off-line — exactly the scenario the duplicated
    // Horizontal pair exists to correct on the next solve. Perturbing
    // p2 specifically exercises `line_b`'s own copy: p2 is only tied
    // to `mid.y` through it (p2's other two constraints, Vertical and
    // DistancePtPt, don't pin p2.y on their own), so a carry-over that
    // silently dropped or misdirected `line_b`'s copy would leave p2
    // free to drift instead of settling back onto the line.
    let result = split_line(&mut sketch, l1, 0.3).expect("split of a solved edge must succeed");
    sketch
        .entities
        .iter_mut()
        .find(|e| e.id == result.mid_point)
        .unwrap()
        .kind = EntityKind::Point { x: 3.0, y: 0.2 };
    sketch
        .entities
        .iter_mut()
        .find(|e| e.id == p2)
        .unwrap()
        .kind = EntityKind::Point { x: 10.0, y: 0.15 };

    let resolved = solver.solve(&sketch, &params).expect(
        "post-split solve must converge — a broken carry-over would over- or \
                 under-constrain this and fail here",
    );
    let solved_xy =
        |id| point_xy(id, &resolved.result.state, &resolved.result.index, &sketch).unwrap();

    // The three untouched corners are visually unchanged.
    for (id, (ex, ey)) in expect_xy {
        let (x, y) = solved_xy(id);
        assert!(
            (x - ex).abs() < TOL && (y - ey).abs() < TOL,
            "post-split corner {id} moved to ({x},{y}), expected ({ex},{ey})"
        );
    }
    // The duplicated Horizontal pair pulls the perturbed mid point
    // back onto the line (y -> 0)...
    let (mx, my) = solved_xy(result.mid_point);
    assert!(my.abs() < TOL, "mid.y = {my}, should have settled to 0");
    // ...without moving its x: nothing constrains mid.x, so it stays
    // exactly where the split placed it.
    assert!(
        (mx - 3.0).abs() < TOL,
        "mid.x drifted to {mx}, expected 3.0 (unconstrained)"
    );
}
