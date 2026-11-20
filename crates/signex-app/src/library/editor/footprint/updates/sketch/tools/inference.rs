//! The constraints a sketch click infers (#188): a Midpoint, Centre or
//! Intersection snap lands its constraint on the click's Point, and a Line
//! within ±5° of an axis gets its Horizontal or Vertical. Ctrl (an `Off`
//! inference) lands none of them. Each lands in the click's undo step.

use super::ToolClickCtx;
use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::SketchEdit;
use crate::library::editor::footprint::snap::{AXIS_THRESHOLD_DEG, SketchInference};
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};

/// The Point tool's click: a Point at `at`, then the constraint its snap
/// infers.
pub(super) fn place_point(
    editor: &mut crate::app::FootprintDocument,
    at: (f64, f64),
    inference: SketchInference,
) {
    let plane = super::resolve_sketch_plane(editor);
    let id = SketchEntityId::new();
    let mut entity = Entity::new(id, plane, EntityKind::Point { x: at.0, y: at.1 });
    entity.construction = editor.state.construction_mode;
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::add_entity(entity));
    });
    land(editor, id, Some(inference));
    editor.canvas_cache.clear();
    editor.mark_dirty();
}

/// Land the constraint `inference` names on `point`. `None` and `Off` land
/// nothing, nor does a constraint that would tie a curve's own endpoint or
/// centre to itself.
pub(super) fn land(
    editor: &mut crate::app::FootprintDocument,
    point: SketchEntityId,
    inference: Option<SketchInference>,
) {
    let kinds: Vec<ConstraintKind> = match inference {
        None | Some(SketchInference::Off) => Vec::new(),
        Some(SketchInference::Midpoint(line)) => match kind_of(editor, line) {
            Some(EntityKind::Line { start, end }) if point != start && point != end => {
                vec![ConstraintKind::Midpoint { point, line }]
            }
            _ => Vec::new(),
        },
        Some(SketchInference::Centre(curve)) => match kind_of(editor, curve) {
            Some(EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. })
                if center != point =>
            {
                vec![ConstraintKind::Coincident {
                    p1: point,
                    p2: center,
                }]
            }
            _ => Vec::new(),
        },
        Some(SketchInference::Intersection(a, b)) => [a, b]
            .into_iter()
            .filter_map(|curve| on_curve(editor, point, curve))
            .collect(),
    };
    for kind in kinds {
        add_constraint(editor, kind);
    }
}

/// The auto Horizontal / Vertical of a new Line from `first` to the click:
/// within [`AXIS_THRESHOLD_DEG`] of an axis, unless Ctrl was held.
pub(super) fn land_axis(
    editor: &mut crate::app::FootprintDocument,
    ctx: &ToolClickCtx,
    line: SketchEntityId,
    first: SketchEntityId,
) {
    if ctx.inference == Some(SketchInference::Off) {
        return;
    }
    let (Some((x0, y0)), Some((x1, y1))) =
        (point_at(editor, first), point_at(editor, ctx.resolved_id))
    else {
        return;
    };
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len = dx.hypot(dy);
    if len <= 1e-6 {
        return;
    }
    let thresh = AXIS_THRESHOLD_DEG.to_radians().sin();
    let kind = if (dy / len).abs() < thresh {
        ConstraintKind::Horizontal { line }
    } else if (dx / len).abs() < thresh {
        ConstraintKind::Vertical { line }
    } else {
        return;
    };
    add_constraint(editor, kind);
}

/// The constraint that holds `point` on `curve`: Point on Line, Point on
/// Arc, or zero distance to a Circle. `None` for the curve's own endpoint.
fn on_curve(
    editor: &crate::app::FootprintDocument,
    point: SketchEntityId,
    curve: SketchEntityId,
) -> Option<ConstraintKind> {
    match kind_of(editor, curve)? {
        EntityKind::Line { start, end } if point != start && point != end => {
            Some(ConstraintKind::PointOnLine { point, line: curve })
        }
        EntityKind::Arc { start, end, .. } if point != start && point != end => {
            Some(ConstraintKind::PointOnArc { point, arc: curve })
        }
        EntityKind::Circle { .. } => Some(ConstraintKind::DistancePtCircle {
            point,
            circle: curve,
            target: DimTarget::Literal(0.0),
        }),
        _ => None,
    }
}

fn kind_of(editor: &crate::app::FootprintDocument, id: SketchEntityId) -> Option<EntityKind> {
    editor
        .primitive()
        .sketch
        .as_ref()
        .and_then(|s| s.entities.iter().find(|e| e.id == id))
        .map(|e| e.kind.clone())
}

/// A Point's authored position.
fn point_at(editor: &crate::app::FootprintDocument, id: SketchEntityId) -> Option<(f64, f64)> {
    match kind_of(editor, id)? {
        EntityKind::Point { x, y } => Some((x, y)),
        _ => None,
    }
}

fn add_constraint(editor: &mut crate::app::FootprintDocument, kind: ConstraintKind) {
    let constraint = Constraint {
        id: ConstraintId::new(),
        kind,
    };
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddConstraint(constraint));
    });
}
