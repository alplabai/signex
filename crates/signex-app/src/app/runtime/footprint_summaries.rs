pub(super) fn footprint_pad_kind_label(
    pad: &crate::library::editor::footprint::state::EditorPad,
) -> &'static str {
    use signex_library::primitive::footprint::PadKind;
    match pad.kind {
        PadKind::Smd => "SMD",
        PadKind::Tht => "Through-hole",
        PadKind::NptHole => "NPT hole",
        PadKind::ConnectorPad => "Connector",
        PadKind::Castellated => "Castellated",
        PadKind::Fiducial => "Fiducial",
        _ => "Unknown",
    }
}

pub(super) fn footprint_pad_shape_label(
    pad: &crate::library::editor::footprint::state::EditorPad,
) -> &'static str {
    use signex_library::primitive::footprint::PadShape;
    match &pad.shape {
        PadShape::Round => "Round",
        PadShape::Rect => "Rect",
        PadShape::Oval => "Oval",
        PadShape::RoundRect { .. } => "RoundRect",
        PadShape::Chamfered { .. } => "Chamfered",
        PadShape::Custom(_) => "Custom",
    }
}

/// v0.22 Phase E3+E4 — Build the per-over-constraint summary list
/// from the solver's `over_constraints` IDs. Resolves each
/// constraint's actual kind (label + first touched entity) so the
/// Properties panel can show meaningful rows + click-to-focus.
///
/// Rows whose residual cannot be evaluated come first, then the rest
/// descending by residual magnitude — see [`residual_magnitude_of`].
pub(super) fn build_over_constraint_summaries(
    fp: &signex_library::primitive::footprint::Footprint,
    out: &signex_sketch::solver::FullSolveOutput,
) -> Vec<crate::panels::OverConstraintSummary> {
    use crate::panels::OverConstraintSummary;

    let sketch = match fp.sketch.as_ref() {
        Some(s) => s,
        None => return Vec::new(),
    };
    if out.over_constraints.is_empty() {
        return Vec::new();
    }
    let over_set: std::collections::HashSet<_> = out.over_constraints.iter().copied().collect();

    let mut summaries: Vec<OverConstraintSummary> = sketch
        .constraints
        .iter()
        .filter(|c| over_set.contains(&c.id))
        .map(|c| {
            let label = c.kind.label();
            OverConstraintSummary {
                constraint_id: c.id,
                kind_label: label,
                residual_magnitude: residual_magnitude_of(c, out, sketch, label),
                focus_entity_id: c.kind.focus_entity(),
            }
        })
        .collect();
    // Unevaluable rows sort FIRST: the user cannot judge them from a
    // number, so they must not be buried under rows that at least
    // carry one. The rest stay descending by magnitude.
    summaries.sort_by(|a, b| match (a.residual_magnitude, b.residual_magnitude) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(x), Some(y)) => y.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal),
    });
    summaries
}

/// The rows for the last rejected edit's conflict. A row's focus entity is
/// dropped when the revert removed it (the rejected edit made it), so a click
/// never selects something that is gone.
pub(super) fn build_rejected_conflict_summaries(
    fp: &signex_library::primitive::footprint::Footprint,
    conflict: &crate::library::editor::footprint::state::conflict::ConflictHighlight,
) -> Vec<crate::panels::RejectedConflictSummary> {
    let exists = |id| {
        fp.sketch
            .as_ref()
            .is_some_and(|s| s.entities.iter().any(|e| e.id == id))
    };
    conflict
        .rows()
        .iter()
        .map(|row| crate::panels::RejectedConflictSummary {
            constraint_id: row.constraint,
            kind_label: row.kind_label,
            focus_entity_id: row.focus.filter(|id| exists(*id)),
        })
        .collect()
}

/// Residual magnitude for one over-constrained constraint, or `None`
/// when it cannot be evaluated.
///
/// GH #599 — this used to substitute `0.0`, which is exactly what a
/// perfectly satisfied constraint reports, and the descending sort in
/// [`build_over_constraint_summaries`] then buried the conflict at the
/// bottom of the diagnostics list. It now returns `None` (which sorts
/// first) and reports the failure: an unevaluable residual means the
/// diagnostics the user is reading are incomplete, and a blank cell in
/// a panel is not a report.
fn residual_magnitude_of(
    c: &signex_sketch::constraint::Constraint,
    out: &signex_sketch::solver::FullSolveOutput,
    sketch: &signex_sketch::sketch::SketchData,
    kind_label: &'static str,
) -> Option<f64> {
    use signex_sketch::solver::residual::residual;

    // `out.params` is the map the solve itself ran with, carried on
    // `FullSolveOutput`. Re-resolving `sketch.parameters` here and
    // falling back to an empty map on error — what this did before —
    // made every parameter-driven constraint fail to evaluate.
    match residual(c, &out.result.state, &out.result.index, sketch, &out.params) {
        Ok(v) => Some(v.iter().map(|x| x * x).sum::<f64>().sqrt()),
        Err(e) => {
            tracing::warn!(
                target: "signex::sketch",
                constraint = %c.id,
                kind = kind_label,
                error = %e,
                "over-constrained sketch: this constraint's residual could not be evaluated, so the diagnostics list cannot rank it",
            );
            None
        }
    }
}

pub(super) fn build_sketch_entity_summary(
    editor: &crate::app::FootprintDocument,
    id: signex_sketch::id::SketchEntityId,
) -> Option<crate::panels::FootprintSketchEntitySummary> {
    use signex_sketch::entity::EntityKind;
    let sketch = editor.primitive().sketch.as_ref()?;
    let entity = sketch.entities.iter().find(|e| e.id == id)?;
    let (kind_label, position_mm) = match entity.kind {
        EntityKind::Point { x, y } => ("Point", Some([x, y])),
        EntityKind::Line { .. } => ("Line", None),
        EntityKind::Arc { .. } => ("Arc", None),
        EntityKind::Circle { .. } => ("Circle", None),
    };
    let attached_constraint_count = sketch
        .constraints
        .iter()
        .filter(|c| c.kind.touches(id))
        .count();
    // v0.22 Phase A3 — Look up the entity's solver DOF colour, if any.
    // Only Points carry a per-entity colour in `last_solve.colours`;
    // other kinds inherit from their endpoints (caller decides whether
    // to render).
    let dof_state = editor
        .state
        .last_solve
        .as_ref()
        .and_then(|s| s.colours.get(&id).copied());
    Some(crate::panels::FootprintSketchEntitySummary {
        kind_label,
        position_mm,
        attached_constraint_count,
        construction: entity.construction,
        dof_state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::primitive::footprint::Footprint;
    use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::{ConstraintId, SketchEntityId};
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::sketch::SketchData;
    use signex_sketch::solver::FullSolveOutput;
    use signex_sketch::solver::lm::SolveResult;
    use signex_sketch::solver::residual::ResolvedParams;
    use signex_sketch::solver::state::pack;

    /// Two Points 5 mm apart, ready for the caller to attach
    /// `DistancePtPt` constraints between them.
    fn two_point_scaffold() -> SketchData {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        data.entities.push(Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Point { x: 0.0, y: 0.0 },
        ));
        data.entities.push(Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Point { x: 5.0, y: 0.0 },
        ));
        data
    }

    fn distance(sketch: &SketchData, target: DimTarget) -> Constraint {
        Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::DistancePtPt {
                p1: sketch.entities[0].id,
                p2: sketch.entities[1].id,
                target,
            },
        }
    }

    /// Attach `constraints` to `data`, flag them all over-constrained,
    /// and assemble the `FullSolveOutput` the panel reads.
    ///
    /// `Solver::solve` reports a conflicting set as `OverConstrained`
    /// and returns no output, so the output is assembled at the packed
    /// initial state instead. Everything
    /// `build_over_constraint_summaries` reads is real: the packed
    /// state, the entity index, and the params the solve ran with.
    fn output_for(
        mut data: SketchData,
        constraints: Vec<Constraint>,
        params: ResolvedParams,
    ) -> (Footprint, FullSolveOutput) {
        let over_constraints = constraints.iter().map(|c| c.id).collect();
        data.constraints = constraints;

        let packed = pack(&data);
        let out = FullSolveOutput {
            result: SolveResult {
                state: packed.vector.clone(),
                index: packed.index.clone(),
                iterations: 0,
                final_residual_norm: 0.0,
                elapsed_ms: 0,
            },
            colours: std::collections::HashMap::new(),
            over_constraints,
            jacobian: Vec::new(),
            params,
        };

        let mut fp = Footprint::empty("test");
        fp.sketch = Some(data);
        (fp, out)
    }

    /// GH #599 — the panel used to re-resolve `sketch.parameters` and
    /// fall back to an empty map when that failed. One unrelated
    /// unparseable parameter was enough: every parameter-driven
    /// constraint then failed to evaluate and was displayed as a
    /// residual of exactly 0.0 — what a satisfied constraint shows.
    #[test]
    fn parametric_residual_uses_the_params_the_solve_ran_with() {
        let scaffold = two_point_scaffold();
        let c = distance(&scaffold, DimTarget::Expr("= d_gap".to_string()));
        let cid = c.id;
        let mut params = ResolvedParams::new();
        params.insert("d_gap".to_string(), 9.0);
        let (mut fp, out) = output_for(scaffold, vec![c], params);

        // The user has since typo'd a parameter, so re-resolving the
        // table here yields `Err` — the old code's empty-map path.
        fp.sketch
            .as_mut()
            .expect("sketch present")
            .parameters
            .insert("w", "1 +");

        let summaries = build_over_constraint_summaries(&fp, &out);

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].constraint_id, cid);
        let mag = summaries[0]
            .residual_magnitude
            .expect("the solve's own params resolve `d_gap`, so the residual evaluates");
        assert!(
            (mag - 4.0).abs() < 1e-9,
            "points are 5 mm apart and the target is 9 mm, so the residual is 4; got {mag}"
        );
    }

    /// GH #599 — a residual that cannot be evaluated is reported as
    /// unavailable and sorts FIRST. It used to be substituted with
    /// 0.0, which the descending sort then pushed below constraints
    /// with tiny nonzero residuals, steering the user away from the
    /// conflict they opened the panel to find.
    #[test]
    fn unevaluable_residual_is_not_zero_and_sorts_first() {
        let scaffold = two_point_scaffold();
        // Satisfied to within 1e-6 — a real but negligible residual.
        let nearly_satisfied = distance(&scaffold, DimTarget::Literal(5.000001));
        let unevaluable = distance(&scaffold, DimTarget::Expr("= d_missing".to_string()));
        let satisfied_id = nearly_satisfied.id;
        let unevaluable_id = unevaluable.id;

        // `d_missing` is in no parameter map, so the residual errors.
        let (fp, out) = output_for(
            scaffold,
            vec![nearly_satisfied, unevaluable],
            ResolvedParams::new(),
        );

        let summaries = build_over_constraint_summaries(&fp, &out);

        assert_eq!(summaries.len(), 2);
        assert_eq!(
            summaries[0].constraint_id, unevaluable_id,
            "the row the user cannot judge from a number must lead, not trail"
        );
        assert_eq!(
            summaries[0].residual_magnitude, None,
            "an unevaluable residual must not be reported as 0.0 — that is what a \
             perfectly satisfied constraint shows"
        );
        assert_eq!(summaries[1].constraint_id, satisfied_id);
        assert!(
            summaries[1].residual_magnitude.is_some(),
            "the literal-target row still carries its magnitude"
        );
    }

    /// #1470 — the structured `entities()` API must agree with the old
    /// Debug-text scan on every constraint kind.
    #[test]
    fn constraint_entity_api_matches_debug_scan() {
        use ConstraintKind::*;
        let e: Vec<SketchEntityId> = (0..3).map(|_| SketchEntityId::new()).collect();
        let (a, b, c) = (e[0], e[1], e[2]);
        let t = || DimTarget::Literal(1.0);
        let kinds = vec![
            Coincident { p1: a, p2: b },
            PointOnLine { point: a, line: b },
            PointOnArc { point: a, arc: b },
            Horizontal { line: a },
            Vertical { line: b },
            Parallel { l1: a, l2: b },
            Perpendicular { l1: b, l2: c },
            DistancePtPt {
                p1: a,
                p2: c,
                target: t(),
            },
            DistancePtLine {
                point: a,
                line: b,
                target: t(),
            },
            DistancePtCircle {
                point: b,
                circle: c,
                target: t(),
            },
            Angle {
                l1: a,
                l2: b,
                target: t(),
            },
            EqualLength { l1: a, l2: b },
            EqualRadius { e1: a, e2: c },
            TangentLineArc { line: a, arc: b },
            TangentArcArc {
                a1: b,
                a2: c,
                internal: true,
            },
            SymmetricAboutLine {
                p1: a,
                p2: b,
                line: c,
            },
            SymmetricAboutPoint {
                p1: a,
                p2: b,
                center: c,
            },
            Midpoint { point: c, line: a },
            Fixed { point: b },
        ];
        for id in &e {
            let old = kinds
                .iter()
                .filter(|k| format!("{k:?}").contains(&id.to_string()))
                .count();
            let new = kinds.iter().filter(|k| k.touches(*id)).count();
            assert_eq!(old, new);
            assert!(new > 0);
        }
        assert_eq!(Fixed { point: a }.entities().count(), 1);
    }

    /// #685 — an edit the solver rejects as a conflict leaves rows for the
    /// constraints panel (kind plus a click target on a surviving point), and
    /// the next edit that solves empties them again.
    #[test]
    fn a_rejected_edit_lists_its_conflict_in_the_panel_rows() {
        use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit;
        use crate::library::editor::footprint::sketch_mode::SketchEdit;
        use crate::library::editor::footprint::state::FootprintEditorState;

        let plane = PlaneId::new();
        let (p1, p2) = (SketchEntityId::new(), SketchEntityId::new());
        let distance = |mm: f64| Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::DistancePtPt {
                p1,
                p2,
                target: DimTarget::Literal(mm),
            },
        };
        let mut fp = Footprint::empty("rows");
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![
                Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
                Entity::new(p2, plane, EntityKind::Point { x: 1.0, y: 0.0 }),
            ],
            constraints: vec![
                Constraint {
                    id: ConstraintId::new(),
                    kind: ConstraintKind::Fixed { point: p1 },
                },
                distance(5.0),
            ],
            ..SketchData::default()
        });
        let mut state = FootprintEditorState::from_footprint(&fp);
        apply_sketch_edit(&mut state, &mut fp, SketchEdit::ForceRebuild).expect("solvable");
        assert!(build_rejected_conflict_summaries(&fp, &state.conflict).is_empty());

        let rejected = distance(9.0);
        let rejected_id = rejected.id;
        apply_sketch_edit(&mut state, &mut fp, SketchEdit::AddConstraint(rejected))
            .expect_err("9 mm contradicts 5 mm");

        let rows = build_rejected_conflict_summaries(&fp, &state.conflict);
        assert!(
            rows.iter().any(|r| r.constraint_id == rejected_id
                && r.kind_label == "DistancePtPt"
                && r.focus_entity_id == Some(p1)),
            "{rows:?}"
        );
        assert!(
            !fp.sketch
                .as_ref()
                .expect("sketch")
                .constraints
                .iter()
                .any(|c| c.id == rejected_id),
            "geometry stays at the last good solve"
        );

        apply_sketch_edit(&mut state, &mut fp, SketchEdit::ForceRebuild).expect("solves");
        assert!(build_rejected_conflict_summaries(&fp, &state.conflict).is_empty());
    }
}
