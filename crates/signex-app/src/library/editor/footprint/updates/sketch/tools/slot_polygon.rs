//! The Slot and Polygon Create tools (mockup row 187). Slot: click the first centre,
//! the second (a typed dimension input is the centre distance), then a
//! click sets the half width. Polygon: click the centre, then the first
//! vertex (the dimension input is the side count, empty is 6). The closing
//! click places the whole shape (`signex_sketch::geom::shapes`) as one
//! edit, so it is one undo step, and back to the tool's first click.

use super::ToolClickCtx;
use crate::library::editor::footprint::sketch_conflict::mutate_solve_or_revert;
use crate::library::editor::footprint::sketch_dispatch::report_solve_failure;
use crate::library::editor::footprint::snap::{SketchInference, point_pos};
use crate::library::editor::footprint::state::{SketchTool, ToolPending};
use signex_sketch::geom::shapes::{self, PlacedPoint, ShapeParts, ShapeRefusal};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::PlaneId;

/// The clicks that place a centre: the Slot's two, the Polygon's one.
pub(super) fn apply(
    editor: &mut crate::app::FootprintDocument,
    ctx: &ToolClickCtx,
    tool: SketchTool,
) {
    let clicked = ctx.resolved_id;
    editor.state.tool_pending = match (tool, editor.state.tool_pending.clone()) {
        (SketchTool::Slot, ToolPending::SlotFirst { a }) if a != clicked => {
            ToolPending::SlotSecond { a, b: clicked }
        }
        (SketchTool::Slot, ToolPending::SlotFirst { a }) => ToolPending::SlotFirst { a },
        (SketchTool::Slot, _) => ToolPending::SlotFirst { a: clicked },
        (_, _) => ToolPending::PolygonCentre { centre: clicked },
    };
}

/// The second slot centre at the typed centre distance from `a`, along
/// the cursor azimuth; `None` when no distance is typed or the cursor is
/// on `a`.
pub(super) fn pinned_second_centre(
    editor: &crate::app::FootprintDocument,
    a: SketchEntityId,
    cursor: (f64, f64),
) -> Option<(f64, f64)> {
    let Some(super::ToolDimension::Accepted { mm, .. }) = super::typed_dimension_input_mm(editor)
    else {
        return None;
    };
    let from = position(editor, a)?;
    let (dx, dy) = (cursor.0 - from.0, cursor.1 - from.1);
    let len = dx.hypot(dy);
    if len <= 1e-9 {
        return None;
    }
    Some((from.0 + mm * dx / len, from.1 + mm * dy / len))
}

/// The clicks handled before a click Point is minted: the Slot's width
/// click, the Polygon's first-vertex click, and a Slot second click whose
/// typed centre distance is unreadable. `Some(changed)` when it was one.
pub(super) fn try_closing_click(
    editor: &mut crate::app::FootprintDocument,
    plane: PlaneId,
    click: (f64, f64),
    flags: (bool, bool),
    inference: Option<SketchInference>,
) -> Option<bool> {
    match (editor.state.active_tool, editor.state.tool_pending.clone()) {
        (SketchTool::Slot, ToolPending::SlotFirst { .. }) => {
            match super::typed_dimension_input_mm(editor) {
                Some(super::ToolDimension::Rejected { buffer }) => {
                    super::reject_tool_dimension(editor, "Slot", "centre distance", &buffer);
                    Some(false)
                }
                _ => None,
            }
        }
        (SketchTool::Slot, ToolPending::SlotSecond { a, b }) => {
            Some(close_slot(editor, plane, (a, b), click, flags))
        }
        (SketchTool::Polygon, ToolPending::PolygonCentre { centre }) => Some(close_polygon(
            editor, plane, centre, click, flags, inference,
        )),
        _ => None,
    }
}

fn close_slot(
    editor: &mut crate::app::FootprintDocument,
    plane: PlaneId,
    (a, b): (SketchEntityId, SketchEntityId),
    click: (f64, f64),
    flags: (bool, bool),
) -> bool {
    let (Some(a_at), Some(b_at)) = (position(editor, a), position(editor, b)) else {
        editor.state.tool_pending = ToolPending::Idle;
        return false;
    };
    let half = shapes::slot_half_width(a_at, b_at, click);
    let a = PlacedPoint { id: a, at: a_at };
    let b = PlacedPoint { id: b, at: b_at };
    match shapes::slot(plane, a, b, half) {
        Ok(parts) => {
            add_parts(editor, parts, flags);
            editor.state.tool_pending = ToolPending::Idle;
            true
        }
        Err(ShapeRefusal::NoWidth) => {
            refuse(
                editor,
                "Slot: click away from the centre line to set the width.",
            );
            false
        }
        Err(_) => {
            refuse(
                editor,
                "Slot: the two centres are one point; nothing was created.",
            );
            editor.state.tool_pending = ToolPending::Idle;
            false
        }
    }
}

fn close_polygon(
    editor: &mut crate::app::FootprintDocument,
    plane: PlaneId,
    centre: SketchEntityId,
    click: (f64, f64),
    flags: (bool, bool),
    inference: Option<SketchInference>,
) -> bool {
    let typed = editor.state.dimension_input.trim().to_string();
    let Some(sides) = shapes::parse_side_count(&typed) else {
        refuse(
            editor,
            &format!(
                "Polygon: \"{typed}\" is not a side count from {} to {}; nothing was created.",
                shapes::POLYGON_SIDES_MIN,
                shapes::POLYGON_SIDES_MAX
            ),
        );
        return false;
    };
    let Some(at) = position(editor, centre) else {
        editor.state.tool_pending = ToolPending::Idle;
        return false;
    };
    match shapes::regular_polygon(plane, PlacedPoint { id: centre, at }, click, sides) {
        Ok(parts) => {
            let first = parts.clicked_point;
            add_parts(editor, parts, flags);
            if let Some(first) = first {
                super::inference::land(editor, first, inference);
            }
            editor.state.tool_pending = ToolPending::Idle;
            true
        }
        Err(_) => {
            refuse(
                editor,
                "Polygon: click away from the centre to set the size.",
            );
            false
        }
    }
}

/// Add a shape's entities and constraints in one edit and one solve. The
/// sticky construction / centerline flags apply to every entity the shape
/// does not already make construction.
fn add_parts(
    editor: &mut crate::app::FootprintDocument,
    parts: ShapeParts,
    (construction, centerline): (bool, bool),
) {
    let ShapeParts {
        mut entities,
        constraints,
        ..
    } = parts;
    for entity in entities.iter_mut().filter(|e| !e.construction) {
        entity.construction = construction;
        entity.centerline = centerline;
    }
    editor.with_parts(|state, primitive| {
        let added = mutate_solve_or_revert(state, primitive, |fp| {
            let sketch = fp
                .sketch
                .get_or_insert_with(signex_sketch::SketchData::default);
            sketch.entities.extend(entities);
            sketch.constraints.extend(constraints);
        });
        if let Err(error) = added {
            report_solve_failure(state, &error);
        }
    });
}

/// Say why the click placed nothing, in Properties and the log.
fn refuse(editor: &mut crate::app::FootprintDocument, text: &str) {
    tracing::warn!(target: "signex::sketch_tools", "{text}");
    editor.state.solve_warnings.push(text.to_string());
}

fn position(editor: &crate::app::FootprintDocument, id: SketchEntityId) -> Option<(f64, f64)> {
    point_pos(id, editor.primitive().sketch.as_ref(), &editor.state)
}
