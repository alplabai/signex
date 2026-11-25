//! Footprint sketch tools — Chamfer and Extend (mockup row 187), the Line edits that
//! pick a Line by its stroke. Their geometry is
//! `signex_sketch::geom::line_edit`; see docs/FOOTPRINT_SKETCH.md,
//! "Modify tools".
//!
//! Both read authored Point positions, as Fillet and Trim do. A click that
//! changes nothing leaves a warning in the sketch inspector and the tool
//! armed.

use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::SketchEdit;
use crate::library::editor::footprint::state::{SketchTool, ToolPending};
use signex_sketch::SketchData;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::geom::{ChamferRefusal, Point2, chamfer_cut, extension_hit};
use signex_sketch::id::{ConstraintId, SketchEntityId};

/// Run a Chamfer or Extend click at `click` (world mm). Returns whether
/// the sketch changed.
pub(super) fn apply(
    editor: &mut crate::app::FootprintDocument,
    tool: SketchTool,
    click: (f64, f64),
) -> bool {
    match tool {
        SketchTool::Chamfer => chamfer(editor, click),
        SketchTool::Extend => extend(editor, click),
        _ => unreachable!("non-Chamfer / Extend tool routed here"),
    }
}

/// Report why a click changed nothing and drop any half-made gesture.
fn refuse(editor: &mut crate::app::FootprintDocument, why: &str) -> bool {
    editor.state.solve_warnings.push(why.into());
    editor.state.tool_pending = ToolPending::Idle;
    false
}

/// Authored position of Point `id`.
fn point_at(sketch: &SketchData, id: SketchEntityId) -> Option<Point2> {
    sketch
        .entities
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some(Point2::new(x, y)),
            _ => None,
        })
}

/// The endpoints of Line `id`.
fn line_ends(sketch: &SketchData, id: SketchEntityId) -> Option<(SketchEntityId, SketchEntityId)> {
    sketch
        .entities
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| match e.kind {
            EntityKind::Line { start, end } => Some((start, end)),
            _ => None,
        })
}

/// A new entity on `like`'s plane with its linetype, so a chamfer of a
/// solid outline stays solid whatever the sticky linetype is.
fn entity_like(like: &Entity, id: SketchEntityId, kind: EntityKind) -> Entity {
    let mut entity = Entity::new(id, like.plane, kind);
    entity.construction = like.construction;
    entity.centerline = like.centerline;
    entity
}

// Chamfer: click the first Line, then the second. The distance is the
// dimension input when one is typed (a value that is not a positive length
// is refused, as Fillet refuses it), else DEFAULT_TOOL_DIMENSION_MM.
fn chamfer(editor: &mut crate::app::FootprintDocument, click: (f64, f64)) -> bool {
    let distance = match super::typed_dimension_input_mm(editor) {
        None => super::DEFAULT_TOOL_DIMENSION_MM,
        Some(super::ToolDimension::Accepted { mm, .. }) => mm,
        Some(super::ToolDimension::Rejected { buffer }) => {
            super::reject_tool_dimension(editor, "Chamfer", "chamfer distance", &buffer);
            return false;
        }
    };
    let picked = editor
        .primitive()
        .sketch
        .as_ref()
        .and_then(|sketch| super::pick_line_at(sketch, click.0, click.1));
    match (editor.state.tool_pending.clone(), picked) {
        (ToolPending::ChamferFirst { line: first }, Some(second)) if second != first => {
            let changed = chamfer_corner(editor, first, second, distance);
            editor.state.tool_pending = ToolPending::Idle;
            changed
        }
        (ToolPending::ChamferFirst { .. }, _) => refuse(
            editor,
            "Chamfer: second click missed a different Line — pick the adjacent line",
        ),
        (_, Some(line)) => {
            editor.state.tool_pending = ToolPending::ChamferFirst { line };
            false
        }
        (_, None) => refuse(
            editor,
            "Chamfer: click missed any Line — try clicking closer to a line stroke",
        ),
    }
}

/// Two Lines meeting at a shared endpoint, read out of the sketch.
struct Corner {
    corner: SketchEntityId,
    at: Point2,
    /// The far ends of the first and second Line.
    first_far: Point2,
    second_far: Point2,
    /// The first Line: the chamfer takes its plane and linetype.
    first: Entity,
}

fn corner_of(
    sketch: &SketchData,
    first: SketchEntityId,
    second: SketchEntityId,
) -> Result<Corner, &'static str> {
    let (Some((a_s, a_e)), Some((b_s, b_e))) =
        (line_ends(sketch, first), line_ends(sketch, second))
    else {
        return Err("Chamfer: a picked Line is gone");
    };
    let corner = if a_s == b_s || a_s == b_e {
        a_s
    } else if a_e == b_s || a_e == b_e {
        a_e
    } else {
        return Err(
            "Chamfer: the two Lines do not share an endpoint — bridge them with a Coincident constraint first",
        );
    };
    let first_far = if a_s == corner { a_e } else { a_s };
    let second_far = if b_s == corner { b_e } else { b_s };
    let (Some(at), Some(first_far), Some(second_far)) = (
        point_at(sketch, corner),
        point_at(sketch, first_far),
        point_at(sketch, second_far),
    ) else {
        return Err("Chamfer: a Line endpoint is missing");
    };
    let Some(first) = sketch.entities.iter().find(|e| e.id == first).cloned() else {
        return Err("Chamfer: a picked Line is gone");
    };
    Ok(Corner {
        corner,
        at,
        first_far,
        second_far,
        first,
    })
}

// Cut both Lines back from the corner by `distance` and join the cut ends
// with a new Line. The cut ends are shared Point ids, so the chamfer stays
// joined to both Lines through later edits; the corner Point is left as
// Fillet leaves it.
fn chamfer_corner(
    editor: &mut crate::app::FootprintDocument,
    first: SketchEntityId,
    second: SketchEntityId,
    distance: f64,
) -> bool {
    let corner = match editor.primitive().sketch.as_ref() {
        Some(sketch) => corner_of(sketch, first, second),
        None => return false,
    };
    let corner = match corner {
        Ok(corner) => corner,
        Err(why) => return refuse(editor, why),
    };
    let (cut_first, cut_second) = match chamfer_cut(
        corner.at,
        corner.first_far,
        corner.second_far,
        distance,
    ) {
        Ok(cut) => cut,
        Err(ChamferRefusal::Colinear) => {
            return refuse(
                editor,
                "Chamfer: the Lines are colinear — there is no corner to cut",
            );
        }
        Err(ChamferRefusal::TooLong) => {
            return refuse(
                editor,
                "Chamfer: the distance reaches past the end of a Line — enter a smaller distance",
            );
        }
    };
    let first_cut_id = SketchEntityId::new();
    let second_cut_id = SketchEntityId::new();
    let chamfer_id = SketchEntityId::new();
    let point = |p: Point2| EntityKind::Point { x: p.x, y: p.y };
    for entity in [
        entity_like(&corner.first, first_cut_id, point(cut_first)),
        entity_like(&corner.first, second_cut_id, point(cut_second)),
        entity_like(
            &corner.first,
            chamfer_id,
            EntityKind::Line {
                start: first_cut_id,
                end: second_cut_id,
            },
        ),
    ] {
        editor.with_parts(|state, primitive| {
            apply_sketch_edit_with_warnings(state, primitive, SketchEdit::add_entity(entity));
        });
    }
    // No SketchEdit rewrites a Line's endpoints; Fillet and Trim mutate the
    // schema directly and rebuild, and so does this.
    if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
        for e in sketch.entities.iter_mut() {
            let cut = if e.id == first {
                first_cut_id
            } else if e.id == second {
                second_cut_id
            } else {
                continue;
            };
            if let EntityKind::Line { start, end } = &mut e.kind {
                if *start == corner.corner {
                    *start = cut;
                } else if *end == corner.corner {
                    *end = cut;
                }
            }
        }
    }
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
    });
    true
}

/// What an Extend click grows, read out of the sketch.
struct Extension {
    /// The end Point that moves.
    end: SketchEntityId,
    /// How far it moves.
    by: (f64, f64),
    /// Keeps the moved end on what it reached.
    on_target: ConstraintKind,
}

// Extend: the Line under the click grows from the end nearer the click to
// the nearest Line, Circle or Arc ahead of that end. Only a free end
// extends: one joined to other geometry or named by a constraint is
// refused rather than torn off it.
fn extend(editor: &mut crate::app::FootprintDocument, click: (f64, f64)) -> bool {
    let plan = match editor.primitive().sketch.as_ref() {
        Some(sketch) => plan_extension(sketch, click),
        None => Err("Extend: click missed any Line — click near the end to extend"),
    };
    let ext = match plan {
        Ok(ext) => ext,
        Err(why) => return refuse(editor, why),
    };
    let constraint = Constraint {
        id: ConstraintId::new(),
        kind: ext.on_target,
    };
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(
            state,
            primitive,
            SketchEdit::MovePoint {
                id: ext.end,
                dx: ext.by.0,
                dy: ext.by.1,
            },
        );
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddConstraint(constraint));
    });
    editor.state.tool_pending = ToolPending::Idle;
    true
}

fn plan_extension(sketch: &SketchData, click: (f64, f64)) -> Result<Extension, &'static str> {
    let line = super::pick_line_at(sketch, click.0, click.1)
        .ok_or("Extend: click missed any Line — click near the end to extend")?;
    let (start, end) = line_ends(sketch, line).ok_or("Extend: the Line is gone")?;
    let (Some(a), Some(b)) = (point_at(sketch, start), point_at(sketch, end)) else {
        return Err("Extend: a Line endpoint is missing");
    };
    let click = Point2::new(click.0, click.1);
    let (moving_id, fixed, moving) = if click.distance(a) < click.distance(b) {
        (start, b, a)
    } else {
        (end, a, b)
    };
    if !is_free_end(sketch, moving_id, line) {
        return Err(
            "Extend: that end is joined to other geometry or constrained — only a free end extends",
        );
    }
    let hit = extension_hit(sketch, line, fixed, moving, |id| point_at(sketch, id))
        .ok_or("Extend: nothing lies ahead of that end — the Line was left as it is")?;
    let on_target = match sketch
        .entities
        .iter()
        .find(|e| e.id == hit.target)
        .map(|e| &e.kind)
    {
        Some(EntityKind::Line { .. }) => ConstraintKind::PointOnLine {
            point: moving_id,
            line: hit.target,
        },
        Some(EntityKind::Arc { .. }) => ConstraintKind::PointOnArc {
            point: moving_id,
            arc: hit.target,
        },
        Some(EntityKind::Circle { .. }) => ConstraintKind::DistancePtCircle {
            point: moving_id,
            circle: hit.target,
            target: DimTarget::Literal(0.0),
        },
        _ => return Err("Extend: nothing lies ahead of that end — the Line was left as it is"),
    };
    Ok(Extension {
        end: moving_id,
        by: (hit.at.x - moving.x, hit.at.y - moving.y),
        on_target,
    })
}

/// Whether Point `end` belongs to Line `line` alone: no other entity uses
/// it, no constraint or array names it, and it carries no pad.
fn is_free_end(sketch: &SketchData, end: SketchEntityId, line: SketchEntityId) -> bool {
    let shared = sketch
        .entities
        .iter()
        .filter(|e| e.id != line && e.id != end)
        .any(|e| e.point_refs().contains(&end));
    let carries_pad = sketch
        .entities
        .iter()
        .any(|e| e.id == end && e.pad.is_some());
    let constrained = sketch.constraints.iter().any(|c| c.kind.touches(end));
    let arrayed = sketch
        .arrays
        .iter()
        .any(|a| a.kind.entity_refs().contains(&end));
    !(shared || carries_pad || constrained || arrayed)
}
