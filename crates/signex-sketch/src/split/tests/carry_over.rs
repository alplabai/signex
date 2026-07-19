//! Attribute / flag / constraint / array / pad-profile-seed carry-over
//! onto `split_line`'s two replacement halves.

use super::{line_endpoints, line_sketch};
use crate::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use crate::attr::{
    BoardCutoutAttr, CourtyardAttr, CustomPadShape, KeepoutAttr, KeepoutKinds, MaskExcludeAttr,
    MaskOpeningAttr, PadAttr, PadShape, PasteApertureAttr, PasteAperturePattern, PourAttr,
    PourFillType, SilkAttr, ThermalRelief, VScoreHintAttr, VScoreSide,
};
use crate::constraint::{Constraint, ConstraintKind, DimTarget};
use crate::split::*;
use signex_types::layer::SignexLayer;

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

/// BLOCKER 1 repro. Before the fix, `build_split_entities` cloned the
/// retired Line's ENTIRE attribute set onto BOTH halves. That's right
/// for a PER-SEGMENT attr (silk, v_score — each independently true of
/// the segment it lands on) but wrong for a CLOSED-PROFILE SEED attr
/// (courtyard, mask_opening, mask_exclude, paste_aperture, pour,
/// keepout, board_cutout): the bake traces the WHOLE loop from ANY
/// entity carrying the attr, so two carriers on one loop emit the
/// region twice (two identical `FpPour`, two routed `FpCutout` on the
/// same board slot, ...). Exactly one entity on the loop — `line_a` —
/// may keep each seed attr after a split.
#[test]
fn closed_profile_seed_attrs_stay_on_line_a_only() {
    let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
    {
        let e = sketch.entities.iter_mut().find(|e| e.id == line).unwrap();
        // Per-segment (must carry to BOTH halves — regression guard).
        e.construction = true;
        e.silk = Some(SilkAttr {
            layer: SignexLayer::TopSilk,
        });
        e.v_score = Some(VScoreHintAttr {
            depth_fraction_expr: "0.5".into(),
            min_web_expr: None,
            side: VScoreSide::Both,
        });
        // Closed-profile seed (must stay on line_a ONLY).
        e.courtyard = Some(CourtyardAttr);
        e.mask_opening = Some(MaskOpeningAttr {
            layer: SignexLayer::TopSolderMask,
        });
        e.mask_exclude = Some(MaskExcludeAttr {
            layer: SignexLayer::BottomSolderMask,
        });
        e.paste_aperture = Some(PasteApertureAttr {
            layer: SignexLayer::TopPaste,
        });
        e.pour = Some(PourAttr {
            layer: SignexLayer::TopCopper,
            net: None,
            fill_type: PourFillType::Solid,
            thermal_relief: ThermalRelief::default(),
            clearance_expr: None,
            min_thickness_expr: None,
            priority: 0,
        });
        e.keepout = Some(KeepoutAttr {
            layer: SignexLayer::TopCopper,
            kinds: KeepoutKinds::default(),
        });
        e.board_cutout = Some(BoardCutoutAttr {
            edge_radius_expr: None,
            through: true,
        });
    }

    let result = split_line(&mut sketch, line, 0.5).unwrap();
    let line_a = sketch
        .entities
        .iter()
        .find(|e| e.id == result.line_a)
        .unwrap();
    let line_b = sketch
        .entities
        .iter()
        .find(|e| e.id == result.line_b)
        .unwrap();

    assert!(line_a.construction && line_b.construction);
    assert!(line_a.silk.is_some() && line_b.silk.is_some());
    assert!(line_a.v_score.is_some() && line_b.v_score.is_some());

    assert!(line_a.courtyard.is_some(), "line_a keeps courtyard");
    assert!(line_b.courtyard.is_none(), "line_b must not carry it too");
    assert!(line_a.mask_opening.is_some());
    assert!(line_b.mask_opening.is_none());
    assert!(line_a.mask_exclude.is_some());
    assert!(line_b.mask_exclude.is_none());
    assert!(line_a.paste_aperture.is_some());
    assert!(line_b.paste_aperture.is_none());
    assert!(line_a.pour.is_some());
    assert!(line_b.pour.is_none());
    assert!(line_a.keepout.is_some());
    assert!(line_b.keepout.is_none());
    assert!(line_a.board_cutout.is_some());
    assert!(line_b.board_cutout.is_none());
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

// ─── Non-constraint id-bearing collections (BLOCKER 1, round 1) ───

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
