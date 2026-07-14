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
/// Sorted descending by residual magnitude.
pub(super) fn build_over_constraint_summaries(
    fp: &signex_library::primitive::footprint::Footprint,
    out: &signex_sketch::solver::FullSolveOutput,
) -> Vec<crate::panels::OverConstraintSummary> {
    use crate::panels::OverConstraintSummary;
    use signex_sketch::constraint::ConstraintKind;
    use signex_sketch::solver::residual::residual;

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

    // Re-resolve params for the residual call. Empty fallback on
    // parse failure mirrors the dof.rs HI-14 caveat — parametric
    // constraints will read 0.0 for the residual display, but
    // they're still listed because over_constraints itself was
    // computed with the correct params at solve time.
    let params = signex_sketch::parameter::resolve(&sketch.parameters)
        .unwrap_or_else(|_| signex_sketch::solver::residual::ResolvedParams::new());
    let mut summaries: Vec<OverConstraintSummary> = sketch
        .constraints
        .iter()
        .filter(|c| over_set.contains(&c.id))
        .map(|c| {
            let r = residual(c, &out.result.state, &out.result.index, sketch, &params);
            let mag = match r {
                Ok(v) => v.iter().map(|x| x * x).sum::<f64>().sqrt(),
                Err(_) => 0.0,
            };
            OverConstraintSummary {
                constraint_id: c.id,
                kind_label: kind_label(&c.kind),
                residual_magnitude: mag,
                focus_entity_id: first_focus(&c.kind),
            }
        })
        .collect();
    summaries.sort_by(|a, b| {
        b.residual_magnitude
            .partial_cmp(&a.residual_magnitude)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    summaries
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
