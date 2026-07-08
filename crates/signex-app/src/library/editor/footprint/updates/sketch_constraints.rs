//! Footprint sketch updates — parameters & constraints concern.
//!
//! Carved out of the monolithic `sketch::apply` (ADR-0001 D1/D2). Arm
//! bodies are moved verbatim; each keeps its own inner `use`s.

use crate::library::messages::PrimitiveEditorMsg;

pub(super) fn apply(editor: &mut crate::app::FootprintEditorState, msg: PrimitiveEditorMsg) {
    match msg {
        PrimitiveEditorMsg::FootprintSketchEditParameter { name, expr } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(
                    state,
                    primitive,
                    SketchEdit::EditParameter { name, expr },
                );
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchAddConstraintForSelection(tag) => {
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
                (
                    SketchConstraintTag::Coincident,
                    Some("Point"),
                    Some("Point"),
                    Some(p1),
                    Some(p2),
                ) => Some(ConstraintKind::Coincident { p1, p2 }),
                (
                    SketchConstraintTag::DistancePtPt,
                    Some("Point"),
                    Some("Point"),
                    Some(p1),
                    Some(p2),
                ) => dim_target.map(|t| ConstraintKind::DistancePtPt { p1, p2, target: t }),
                (SketchConstraintTag::Horizontal, Some("Line"), _, Some(l), _) => {
                    Some(ConstraintKind::Horizontal { line: l })
                }
                (SketchConstraintTag::Vertical, Some("Line"), _, Some(l), _) => {
                    Some(ConstraintKind::Vertical { line: l })
                }
                (SketchConstraintTag::Parallel, Some("Line"), Some("Line"), Some(l1), Some(l2)) => {
                    Some(ConstraintKind::Parallel { l1, l2 })
                }
                (
                    SketchConstraintTag::Perpendicular,
                    Some("Line"),
                    Some("Line"),
                    Some(l1),
                    Some(l2),
                ) => Some(ConstraintKind::Perpendicular { l1, l2 }),
                (
                    SketchConstraintTag::EqualLength,
                    Some("Line"),
                    Some("Line"),
                    Some(l1),
                    Some(l2),
                ) => Some(ConstraintKind::EqualLength { l1, l2 }),
                (
                    SketchConstraintTag::PointOnLine,
                    Some("Point"),
                    Some("Line"),
                    Some(p),
                    Some(l),
                ) => Some(ConstraintKind::PointOnLine { point: p, line: l }),
                (
                    SketchConstraintTag::PointOnLine,
                    Some("Line"),
                    Some("Point"),
                    Some(l),
                    Some(p),
                ) => Some(ConstraintKind::PointOnLine { point: p, line: l }),
                (SketchConstraintTag::Midpoint, Some("Point"), Some("Line"), Some(p), Some(l)) => {
                    Some(ConstraintKind::Midpoint { point: p, line: l })
                }
                (SketchConstraintTag::Midpoint, Some("Line"), Some("Point"), Some(l), Some(p)) => {
                    Some(ConstraintKind::Midpoint { point: p, line: l })
                }
                // --- v0.15: 9 additional constraint kinds ---
                (
                    SketchConstraintTag::TangentLineArc,
                    Some("Line"),
                    Some("Arc"),
                    Some(line),
                    Some(arc),
                ) => Some(ConstraintKind::TangentLineArc { line, arc }),
                (
                    SketchConstraintTag::TangentLineArc,
                    Some("Arc"),
                    Some("Line"),
                    Some(arc),
                    Some(line),
                ) => Some(ConstraintKind::TangentLineArc { line, arc }),
                (
                    SketchConstraintTag::TangentArcArc,
                    Some("Arc"),
                    Some("Arc"),
                    Some(a1),
                    Some(a2),
                ) => Some(ConstraintKind::TangentArcArc {
                    a1,
                    a2,
                    internal: false,
                }),
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
                (
                    SketchConstraintTag::PointOnArc,
                    Some("Point"),
                    Some("Arc"),
                    Some(point),
                    Some(arc),
                ) => Some(ConstraintKind::PointOnArc { point, arc }),
                (
                    SketchConstraintTag::PointOnArc,
                    Some("Arc"),
                    Some("Point"),
                    Some(arc),
                    Some(point),
                ) => Some(ConstraintKind::PointOnArc { point, arc }),
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

            if let Some(kind) = new_kind {
                let constraint = Constraint {
                    id: ConstraintId::new(),
                    kind,
                };
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(
                        state,
                        primitive,
                        SketchEdit::AddConstraint(constraint),
                    );
                });
                editor.dirty = true;
                editor.canvas_cache.clear();
            }
        }
        _ => unreachable!(
            "non-parameters & constraints sketch variant routed to sketch_constraints.rs"
        ),
    }
}
