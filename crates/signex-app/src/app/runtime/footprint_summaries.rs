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
    use signex_sketch::constraint::ConstraintKind;

    let sketch = match fp.sketch.as_ref() {
        Some(s) => s,
        None => return Vec::new(),
    };
    if out.over_constraints.is_empty() {
        return Vec::new();
    }
    let over_set: std::collections::HashSet<_> = out.over_constraints.iter().copied().collect();

    let kind_label = |k: &ConstraintKind| -> &'static str {
        use ConstraintKind::*;
        match k {
            Coincident { .. } => "Coincident",
            PointOnLine { .. } => "PointOnLine",
            PointOnArc { .. } => "PointOnArc",
            Horizontal { .. } => "Horizontal",
            Vertical { .. } => "Vertical",
            Parallel { .. } => "Parallel",
            Perpendicular { .. } => "Perpendicular",
            DistancePtPt { .. } => "DistancePtPt",
            DistancePtLine { .. } => "DistancePtLine",
            DistancePtCircle { .. } => "DistancePtCircle",
            Angle { .. } => "Angle",
            EqualLength { .. } => "EqualLength",
            EqualRadius { .. } => "EqualRadius",
            TangentLineArc { .. } => "TangentLineArc",
            TangentArcArc { .. } => "TangentArcArc",
            SymmetricAboutLine { .. } => "SymmetricAboutLine",
            SymmetricAboutPoint { .. } => "SymmetricAboutPoint",
            Midpoint { .. } => "Midpoint",
            Fixed { .. } => "Fixed",
        }
    };
    let first_focus = |k: &ConstraintKind| -> Option<signex_sketch::id::SketchEntityId> {
        use ConstraintKind::*;
        match k {
            Coincident { p1, .. } => Some(*p1),
            PointOnLine { point, .. } => Some(*point),
            PointOnArc { point, .. } => Some(*point),
            Horizontal { line } => Some(*line),
            Vertical { line } => Some(*line),
            Parallel { l1, .. } => Some(*l1),
            Perpendicular { l1, .. } => Some(*l1),
            DistancePtPt { p1, .. } => Some(*p1),
            DistancePtLine { point, .. } => Some(*point),
            DistancePtCircle { point, .. } => Some(*point),
            Angle { l1, .. } => Some(*l1),
            EqualLength { l1, .. } => Some(*l1),
            EqualRadius { e1, .. } => Some(*e1),
            TangentLineArc { line, .. } => Some(*line),
            TangentArcArc { a1, .. } => Some(*a1),
            SymmetricAboutLine { p1, .. } => Some(*p1),
            SymmetricAboutPoint { p1, .. } => Some(*p1),
            Midpoint { point, .. } => Some(*point),
            Fixed { point } => Some(*point),
        }
    };

    let mut summaries: Vec<OverConstraintSummary> = sketch
        .constraints
        .iter()
        .filter(|c| over_set.contains(&c.id))
        .map(|c| {
            let label = kind_label(&c.kind);
            OverConstraintSummary {
                constraint_id: c.id,
                kind_label: label,
                residual_magnitude: residual_magnitude_of(c, out, sketch, label),
                focus_entity_id: first_focus(&c.kind),
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
    editor: &crate::app::FootprintEditorState,
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
    // Coarse: count constraints whose Debug-stringified payload
    // mentions this entity ID. Mirrors the dispatcher's existing
    // dangling-ref drop heuristic — good enough for v0.14.2 surface;
    // structured constraint→entity touch-graph lands later.
    let id_str = id.to_string();
    let attached_constraint_count = sketch
        .constraints
        .iter()
        .filter(|c| format!("{:?}", c.kind).contains(&id_str))
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
    /// Conflicting constraints make `solve_lm` legitimately return
    /// `DidNotConverge`, so — exactly as `signex-sketch/tests/dof.rs`
    /// does — the output is assembled at the packed initial state
    /// rather than through `Solver::solve`. Everything
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
}
