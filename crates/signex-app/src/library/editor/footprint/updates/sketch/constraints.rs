//! Footprint sketch updates — parameters & constraints concern.
//!
//! Carved out of the monolithic `sketch::apply` (ADR-0001 D1/D2). `apply`
//! is a thin router; each variant delegates to one named per-action fn
//! below (object→action, ADR-0001 D2).

use crate::library::messages::FootprintEditorMsg;

pub(in crate::library::editor::footprint::updates) fn apply(
    editor: &mut crate::app::FootprintEditorState,
    msg: FootprintEditorMsg,
) {
    match msg {
        FootprintEditorMsg::SketchEditParameter { name, expr } => {
            edit_parameter(editor, name, expr)
        }
        FootprintEditorMsg::SketchAddConstraintForSelection(tag) => {
            add_constraint_for_selection(editor, tag)
        }
        _ => unreachable!(
            "non-parameters & constraints sketch variant routed to sketch_constraints.rs"
        ),
    }
}

fn edit_parameter(editor: &mut crate::app::FootprintEditorState, name: String, expr: String) {
    use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
    use crate::library::editor::footprint::sketch_mode::SketchEdit;
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::EditParameter { name, expr });
    });
    editor.canvas_cache.clear();
    editor.dirty = true;
}

fn add_constraint_for_selection(
    editor: &mut crate::app::FootprintEditorState,
    tag: crate::library::messages::SketchConstraintTag,
) {
    use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
    use crate::library::editor::footprint::sketch_mode::SketchEdit;
    use crate::library::messages::SketchConstraintTag;
    use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
    use signex_sketch::id::ConstraintId;

    let primary = editor.state.selected_sketch;
    let secondary = editor.state.selected_sketch_secondary;
    let dim_target = editor
        .state
        .dimension_input
        .trim()
        .parse::<f64>()
        .ok()
        .map(DimTarget::Literal);

    // Determine selected entity kinds (Point / Line / Arc / Circle)
    // by inspecting the sketch.
    let kind_of = |id: signex_sketch::id::SketchEntityId| -> Option<&'static str> {
        use signex_sketch::entity::EntityKind;
        editor
            .primitive()
            .sketch
            .as_ref()?
            .entities
            .iter()
            .find(|e| e.id == id)
            .map(|e| match e.kind {
                EntityKind::Point { .. } => "Point",
                EntityKind::Line { .. } => "Line",
                EntityKind::Arc { .. } => "Arc",
                EntityKind::Circle { .. } => "Circle",
            })
    };
    let p_kind = primary.and_then(kind_of);
    let s_kind = secondary.and_then(kind_of);
    // v0.15 — third entity for the 3-entity Symmetric
    // constraints comes from the rubber-band extra slot.
    let extra = editor.state.selected_sketch_extra.first().copied();
    let extra_kind = extra.and_then(kind_of);
    // Angle's DimTarget is stored in radians (canonical unit);
    // the dim-input field is degrees, so convert here.
    let angle_target = editor
        .state
        .dimension_input
        .trim()
        .parse::<f64>()
        .ok()
        .map(|deg| DimTarget::Literal(deg.to_radians()));

    let new_kind: Option<ConstraintKind> = match (tag, p_kind, s_kind, primary, secondary) {
        (SketchConstraintTag::Fixed, Some("Point"), _, Some(p), _) => {
            Some(ConstraintKind::Fixed { point: p })
        }
        (SketchConstraintTag::Coincident, Some("Point"), Some("Point"), Some(p1), Some(p2)) => {
            Some(ConstraintKind::Coincident { p1, p2 })
        }
        (SketchConstraintTag::DistancePtPt, Some("Point"), Some("Point"), Some(p1), Some(p2)) => {
            dim_target.map(|t| ConstraintKind::DistancePtPt { p1, p2, target: t })
        }
        (SketchConstraintTag::Horizontal, Some("Line"), _, Some(l), _) => {
            Some(ConstraintKind::Horizontal { line: l })
        }
        (SketchConstraintTag::Vertical, Some("Line"), _, Some(l), _) => {
            Some(ConstraintKind::Vertical { line: l })
        }
        (SketchConstraintTag::Parallel, Some("Line"), Some("Line"), Some(l1), Some(l2)) => {
            Some(ConstraintKind::Parallel { l1, l2 })
        }
        (SketchConstraintTag::Perpendicular, Some("Line"), Some("Line"), Some(l1), Some(l2)) => {
            Some(ConstraintKind::Perpendicular { l1, l2 })
        }
        (SketchConstraintTag::EqualLength, Some("Line"), Some("Line"), Some(l1), Some(l2)) => {
            Some(ConstraintKind::EqualLength { l1, l2 })
        }
        (SketchConstraintTag::PointOnLine, Some("Point"), Some("Line"), Some(p), Some(l)) => {
            Some(ConstraintKind::PointOnLine { point: p, line: l })
        }
        (SketchConstraintTag::PointOnLine, Some("Line"), Some("Point"), Some(l), Some(p)) => {
            Some(ConstraintKind::PointOnLine { point: p, line: l })
        }
        (SketchConstraintTag::Midpoint, Some("Point"), Some("Line"), Some(p), Some(l)) => {
            Some(ConstraintKind::Midpoint { point: p, line: l })
        }
        (SketchConstraintTag::Midpoint, Some("Line"), Some("Point"), Some(l), Some(p)) => {
            Some(ConstraintKind::Midpoint { point: p, line: l })
        }
        // --- v0.15: 9 additional constraint kinds ---
        (SketchConstraintTag::TangentLineArc, Some("Line"), Some("Arc"), Some(line), Some(arc)) => {
            Some(ConstraintKind::TangentLineArc { line, arc })
        }
        (SketchConstraintTag::TangentLineArc, Some("Arc"), Some("Line"), Some(arc), Some(line)) => {
            Some(ConstraintKind::TangentLineArc { line, arc })
        }
        (SketchConstraintTag::TangentArcArc, Some("Arc"), Some("Arc"), Some(a1), Some(a2)) => {
            Some(ConstraintKind::TangentArcArc {
                a1,
                a2,
                internal: false,
            })
        }
        (SketchConstraintTag::Angle, Some("Line"), Some("Line"), Some(l1), Some(l2)) => {
            angle_target.map(|t| ConstraintKind::Angle { l1, l2, target: t })
        }
        // EqualRadius spans any two of Circle / Arc.
        (
            SketchConstraintTag::EqualRadius,
            Some("Circle") | Some("Arc"),
            Some("Circle") | Some("Arc"),
            Some(e1),
            Some(e2),
        ) => Some(ConstraintKind::EqualRadius { e1, e2 }),
        (SketchConstraintTag::PointOnArc, Some("Point"), Some("Arc"), Some(point), Some(arc)) => {
            Some(ConstraintKind::PointOnArc { point, arc })
        }
        (SketchConstraintTag::PointOnArc, Some("Arc"), Some("Point"), Some(arc), Some(point)) => {
            Some(ConstraintKind::PointOnArc { point, arc })
        }
        (
            SketchConstraintTag::DistancePtLine,
            Some("Point"),
            Some("Line"),
            Some(point),
            Some(line),
        ) => dim_target.map(|t| ConstraintKind::DistancePtLine {
            point,
            line,
            target: t,
        }),
        (
            SketchConstraintTag::DistancePtLine,
            Some("Line"),
            Some("Point"),
            Some(line),
            Some(point),
        ) => dim_target.map(|t| ConstraintKind::DistancePtLine {
            point,
            line,
            target: t,
        }),
        // DistancePtCircle: the `circle` field accepts a Circle
        // or an Arc (radius read from live state in both cases).
        (
            SketchConstraintTag::DistancePtCircle,
            Some("Point"),
            Some("Circle") | Some("Arc"),
            Some(point),
            Some(circle),
        ) => dim_target.map(|t| ConstraintKind::DistancePtCircle {
            point,
            circle,
            target: t,
        }),
        (
            SketchConstraintTag::DistancePtCircle,
            Some("Circle") | Some("Arc"),
            Some("Point"),
            Some(circle),
            Some(point),
        ) => dim_target.map(|t| ConstraintKind::DistancePtCircle {
            point,
            circle,
            target: t,
        }),
        // 3-entity Symmetric constraints: primary + secondary
        // are the two governed Points; the third entity (mirror
        // Line / centre Point) comes from the extra slot.
        (
            SketchConstraintTag::SymmetricAboutLine,
            Some("Point"),
            Some("Point"),
            Some(p1),
            Some(p2),
        ) if extra_kind == Some("Line") => {
            extra.map(|line| ConstraintKind::SymmetricAboutLine { p1, p2, line })
        }
        (
            SketchConstraintTag::SymmetricAboutPoint,
            Some("Point"),
            Some("Point"),
            Some(p1),
            Some(p2),
        ) if extra_kind == Some("Point") => {
            extra.map(|center| ConstraintKind::SymmetricAboutPoint { p1, p2, center })
        }
        _ => None,
    };

    let Some(kind) = new_kind else {
        // GH #599 — before this `else` existed the whole gesture was a
        // no-op with no signal of any kind: no constraint, no message,
        // no highlight, `dirty` untouched and the canvas cache intact,
        // so even the redraw was byte-identical. The commonest cause is
        // a dimension the field cannot read — `1,5` typed on a
        // comma-decimal keyboard, or `1.5mm` with the unit spelled out.
        let bad_dimension = dim_input_error(&editor.state.dimension_input);
        report_constraint_not_added(editor, tag, bad_dimension);
        return;
    };
    let constraint = Constraint {
        id: ConstraintId::new(),
        kind,
    };
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddConstraint(constraint));
    });
    editor.dirty = true;
    editor.canvas_cache.clear();
}

/// The `dimension_input` text when it is present but unreadable as a
/// number, `None` when it is empty or valid.
///
/// An empty field is not an error on its own: most constraint tags
/// (Coincident, Parallel, Horizontal, …) take no dimension at all, so
/// the failure there is a selection mismatch, not a bad number.
fn dim_input_error(dimension_input: &str) -> Option<String> {
    let typed = dimension_input.trim();
    if typed.is_empty() || typed.parse::<f64>().is_ok() {
        return None;
    }
    Some(typed.to_string())
}

/// Report a constraint the user asked for and did not get.
///
/// GH #599 — the dimensional tags (Distance, Angle, point-to-circle,
/// point-to-line) drop the `ParseFloatError` from `dimension_input`, and
/// every tag drops a selection that does not match its arm. Both used to
/// end in the same silent no-op. Reported at `error!`: the requested
/// constraint was withheld outright, and the default log filter is
/// `LevelFilter::Info`, so `debug!` would never reach the Messages panel.
fn report_constraint_not_added(
    editor: &mut crate::app::FootprintEditorState,
    tag: crate::library::messages::SketchConstraintTag,
    bad_dimension: Option<String>,
) {
    let warning = match &bad_dimension {
        Some(buffer) => {
            tracing::error!(
                target: "signex::sketch_constraints",
                constraint = ?tag,
                dimension_input = buffer.as_str(),
                "constraint not added: the dimension field is not a number"
            );
            format!(
                "{tag:?}: \"{buffer}\" is not a valid dimension — no constraint was added. \
                 Enter a number using \".\" as the decimal separator (degrees for Angle)."
            )
        }
        None => {
            tracing::error!(
                target: "signex::sketch_constraints",
                constraint = ?tag,
                "constraint not added: the selection does not match this constraint"
            );
            format!(
                "{tag:?}: the current selection does not match this constraint — \
                 no constraint was added. Check the number and kinds of selected entities."
            )
        }
    };
    editor.state.solve_warnings.push(warning);
    editor.canvas_cache.clear();
}
