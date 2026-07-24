//! Footprint sketch tools — drawing tools (carved from `sketch_tools::apply`, ADR-0001 D2).
//!
//! `apply` is a thin router; each `SketchTool` delegates to one named
//! per-tool fn below. Bodies moved verbatim; the preamble locals they
//! read (`flag`, `ctx.plane_id`, `ctx.resolved_id`, raw click
//! `ctx.x_mm`/`ctx.y_mm`) arrive via [`ToolClickCtx`].

use super::ToolClickCtx;
use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::SketchEdit;
use crate::library::editor::footprint::state::{PlacementInputKind, SketchTool, ToolPending};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;

pub(super) fn apply(
    editor: &mut crate::app::FootprintEditorState,
    ctx: &ToolClickCtx,
    tool: SketchTool,
) {
    match tool {
        SketchTool::Select | SketchTool::Point => select_or_point(editor),
        SketchTool::Line => line(editor, ctx),
        SketchTool::Circle => circle(editor, ctx),
        SketchTool::RoundedRectangle => rounded_rectangle(editor, ctx),
        SketchTool::Rectangle => rectangle(editor, ctx),
        SketchTool::Arc => arc(editor, ctx),
        SketchTool::TangentArc => tangent_arc(editor, ctx),
        _ => unreachable!("non-drawing tools tool routed here"),
    }
}

fn select_or_point(editor: &mut crate::app::FootprintEditorState) {
    // Select: ignore (no add). Point: already added above.
    editor.state.tool_pending = ToolPending::Idle;
}

fn line(editor: &mut crate::app::FootprintEditorState, ctx: &ToolClickCtx) {
    match editor.state.tool_pending {
        ToolPending::Idle => {
            editor.state.tool_pending = ToolPending::LineFirst {
                first: ctx.resolved_id,
            };
        }
        ToolPending::LineFirst { first } => {
            let line_id = SketchEntityId::new();
            let line = ctx.flag(Entity::new(
                line_id,
                ctx.plane_id,
                EntityKind::Line {
                    start: first,
                    end: ctx.resolved_id,
                },
            ));
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(line));
            });

            // v0.22 Phase A2 — Auto-Horizontal/Vertical
            // inference. If the line's slope is within ±5°
            // of a cardinal axis, add the matching
            // constraint so the alignment survives a drag.
            // The cursor-snap engine already pulls the
            // click onto the axis when within tolerance,
            // so this just promotes the implicit alignment
            // to an explicit constraint visible in the
            // constraint list.
            {
                use signex_sketch::constraint::{Constraint, ConstraintKind};
                use signex_sketch::id::ConstraintId;
                const AXIS_THRESHOLD_DEG: f64 = 5.0;
                let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                    editor
                        .primitive()
                        .sketch
                        .as_ref()
                        .and_then(|s| s.entities.iter().find(|e| e.id == id))
                        .and_then(|e| match e.kind {
                            EntityKind::Point { x, y } => Some((x, y)),
                            _ => None,
                        })
                };
                if let (Some((x0, y0)), Some((x1, y1))) = (pos_of(first), pos_of(ctx.resolved_id)) {
                    let dx = x1 - x0;
                    let dy = y1 - y0;
                    let len_sq = dx * dx + dy * dy;
                    if len_sq > 1e-12 {
                        let len = len_sq.sqrt();
                        let sin_abs = (dy / len).abs();
                        let cos_abs = (dx / len).abs();
                        let thresh = AXIS_THRESHOLD_DEG.to_radians().sin();
                        let kind = if sin_abs < thresh {
                            Some(ConstraintKind::Horizontal { line: line_id })
                        } else if cos_abs < thresh {
                            Some(ConstraintKind::Vertical { line: line_id })
                        } else {
                            None
                        };
                        if let Some(k) = kind {
                            let constraint = Constraint {
                                id: ConstraintId::new(),
                                kind: k,
                            };
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(constraint),
                                );
                            });
                        }
                    }
                }
            }

            // v0.16.1 — chain: keep the Line tool active
            // and use this click's endpoint as the next
            // segment's anchor. Esc / right-click cancel
            // back to Select. Matches Fusion 2D sketch.
            editor.state.tool_pending = ToolPending::LineFirst {
                first: ctx.resolved_id,
            };
        }
        _ => {
            editor.state.tool_pending = ToolPending::LineFirst {
                first: ctx.resolved_id,
            };
        }
    }
}

fn circle(editor: &mut crate::app::FootprintEditorState, ctx: &ToolClickCtx) {
    match editor.state.tool_pending {
        ToolPending::Idle => {
            editor.state.tool_pending = ToolPending::CircleCenter {
                center: ctx.resolved_id,
            };
        }
        ToolPending::CircleCenter { center } => {
            // Compute radius from centre + edge points.
            let r = if let (Some(c_pt), Some(e_pt)) = (
                editor
                    .primitive()
                    .sketch
                    .as_ref()
                    .and_then(|s| s.entities.iter().find(|e| e.id == center))
                    .and_then(|e| match e.kind {
                        EntityKind::Point { x, y } => Some((x, y)),
                        _ => None,
                    }),
                editor
                    .primitive()
                    .sketch
                    .as_ref()
                    .and_then(|s| s.entities.iter().find(|e| e.id == ctx.resolved_id))
                    .and_then(|e| match e.kind {
                        EntityKind::Point { x, y } => Some((x, y)),
                        _ => None,
                    }),
            ) {
                ((e_pt.0 - c_pt.0).powi(2) + (e_pt.1 - c_pt.1).powi(2)).sqrt()
            } else {
                1.0
            };
            let circle_id = SketchEntityId::new();
            let circle = ctx.flag(Entity::new(
                circle_id,
                ctx.plane_id,
                EntityKind::Circle { center, radius: r },
            ));
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(circle));
            });
            editor.state.tool_pending = ToolPending::Idle;
        }
        _ => {
            editor.state.tool_pending = ToolPending::CircleCenter {
                center: ctx.resolved_id,
            };
        }
    }
}

fn rounded_rectangle(editor: &mut crate::app::FootprintEditorState, ctx: &ToolClickCtx) {
    match editor.state.tool_pending {
        ToolPending::Idle => {
            editor.state.tool_pending = ToolPending::RoundedRectangleFirst {
                first: ctx.resolved_id,
            };
        }
        ToolPending::RoundedRectangleFirst { first } => {
            // v0.16 — commit the rounded rectangle. Read
            // first/opposite corner positions, derive the
            // axis-aligned bbox, clamp the corner radius,
            // and emit 4 arc-centre Points + 8 arc-end /
            // line-end Points + 4 Lines (axis-aligned,
            // shortened by the radius) + 4 Arcs (one per
            // corner, sweep CCW around the centre).
            let first_pos = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.entities.iter().find(|e| e.id == first))
                .and_then(|e| match e.kind {
                    EntityKind::Point { x, y } => Some((x, y)),
                    _ => None,
                });
            let opposite_pos = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.entities.iter().find(|e| e.id == ctx.resolved_id))
                .and_then(|e| match e.kind {
                    EntityKind::Point { x, y } => Some((x, y)),
                    _ => None,
                });
            if let (Some((fx, fy)), Some((ox, oy))) = (first_pos, opposite_pos) {
                let x0 = fx.min(ox);
                let y0 = fy.min(oy);
                let x1 = fx.max(ox);
                let y1 = fy.max(oy);
                let half_w = (x1 - x0) / 2.0;
                let half_h = (y1 - y0) / 2.0;
                // v0.14-footprint — corner radius source:
                // prefer a typed RRectRadius (the third Tab
                // field), then the legacy `dimension_input`
                // text, else 0.5 mm. Clamp to [0.05, half_min].
                let r_input = std::iter::once(editor.state.placement_input.as_ref())
                    .chain(editor.state.placement_input_others.iter().map(Some))
                    .flatten()
                    .find(|p| p.kind == PlacementInputKind::RRectRadius)
                    .and_then(|p| p.buffer.parse::<f64>().ok())
                    .or_else(|| editor.state.dimension_input.trim().parse::<f64>().ok())
                    .unwrap_or(0.5);
                let r_max = half_w.min(half_h).max(0.05);
                let r = r_input.clamp(0.05, r_max);

                let tl_c = SketchEntityId::new();
                let tr_c = SketchEntityId::new();
                let br_c = SketchEntityId::new();
                let bl_c = SketchEntityId::new();
                let tl_right = SketchEntityId::new();
                let tr_left = SketchEntityId::new();
                let tr_top = SketchEntityId::new();
                let br_top = SketchEntityId::new();
                let br_right = SketchEntityId::new();
                let bl_left = SketchEntityId::new();
                let bl_bot = SketchEntityId::new();
                let tl_bot = SketchEntityId::new();

                for (id, x, y) in [
                    (tl_c, x0 + r, y0 + r),
                    (tr_c, x1 - r, y0 + r),
                    (br_c, x1 - r, y1 - r),
                    (bl_c, x0 + r, y1 - r),
                    (tl_right, x0 + r, y0),
                    (tr_left, x1 - r, y0),
                    (tr_top, x1, y0 + r),
                    (br_top, x1, y1 - r),
                    (br_right, x1 - r, y1),
                    (bl_left, x0 + r, y1),
                    (bl_bot, x0, y1 - r),
                    (tl_bot, x0, y0 + r),
                ] {
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(ctx.flag(Entity::new(
                                id,
                                ctx.plane_id,
                                EntityKind::Point { x, y },
                            ))),
                        );
                    });
                }
                // Lines: top, right, bottom, left.
                for (s, e) in [
                    (tl_right, tr_left),
                    (tr_top, br_top),
                    (br_right, bl_left),
                    (bl_bot, tl_bot),
                ] {
                    let line_id = SketchEntityId::new();
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(ctx.flag(Entity::new(
                                line_id,
                                ctx.plane_id,
                                EntityKind::Line { start: s, end: e },
                            ))),
                        );
                    });
                }
                // Arcs: TR, BR, BL, TL — sweep CCW around
                // each centre so each subtends 90°.
                for (center, start, end) in [
                    (tr_c, tr_left, tr_top),
                    (br_c, br_top, br_right),
                    (bl_c, bl_left, bl_bot),
                    (tl_c, tl_bot, tl_right),
                ] {
                    let arc_id = SketchEntityId::new();
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(ctx.flag(Entity::new(
                                arc_id,
                                ctx.plane_id,
                                EntityKind::Arc {
                                    center,
                                    start,
                                    end,
                                    sweep_ccw: true,
                                },
                            ))),
                        );
                    });
                }
            }
            editor.state.tool_pending = ToolPending::Idle;
        }
        _ => {
            editor.state.tool_pending = ToolPending::RoundedRectangleFirst {
                first: ctx.resolved_id,
            };
        }
    }
}

fn rectangle(editor: &mut crate::app::FootprintEditorState, ctx: &ToolClickCtx) {
    match editor.state.tool_pending {
        ToolPending::Idle => {
            editor.state.tool_pending = ToolPending::RectangleFirst {
                first: ctx.resolved_id,
            };
        }
        ToolPending::RectangleFirst { first } => {
            // v0.15 — commit the rectangle. Resolve the
            // first corner's world position from the
            // sketch, then mint 2 new Points (the two
            // mid-axis corners) and 4 Lines connecting
            // (first, midA, opposite, midB) into a
            // closed loop. ctx.resolved_id is the opposite
            // corner the user just clicked.
            let first_pos = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.entities.iter().find(|e| e.id == first))
                .and_then(|e| match e.kind {
                    EntityKind::Point { x, y } => Some((x, y)),
                    _ => None,
                });
            let opposite_pos = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.entities.iter().find(|e| e.id == ctx.resolved_id))
                .and_then(|e| match e.kind {
                    EntityKind::Point { x, y } => Some((x, y)),
                    _ => None,
                });
            if let (Some((x0, y0)), Some((x1, y1))) = (first_pos, opposite_pos) {
                // Mint the two mid-axis corners.
                let mid_a_id = SketchEntityId::new();
                let mid_b_id = SketchEntityId::new();
                let mid_a = ctx.flag(Entity::new(
                    mid_a_id,
                    ctx.plane_id,
                    EntityKind::Point { x: x1, y: y0 },
                ));
                let mid_b = ctx.flag(Entity::new(
                    mid_b_id,
                    ctx.plane_id,
                    EntityKind::Point { x: x0, y: y1 },
                ));
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(mid_a));
                });
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(mid_b));
                });
                // Now the 4 lines: first → mid_a →
                // opposite → mid_b → first.
                for (s, e) in [
                    (first, mid_a_id),
                    (mid_a_id, ctx.resolved_id),
                    (ctx.resolved_id, mid_b_id),
                    (mid_b_id, first),
                ] {
                    let line_id = SketchEntityId::new();
                    let line = ctx.flag(Entity::new(
                        line_id,
                        ctx.plane_id,
                        EntityKind::Line { start: s, end: e },
                    ));
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(line),
                        );
                    });
                }
            }
            editor.state.tool_pending = ToolPending::Idle;
        }
        _ => {
            editor.state.tool_pending = ToolPending::RectangleFirst {
                first: ctx.resolved_id,
            };
        }
    }
}

fn arc(editor: &mut crate::app::FootprintEditorState, ctx: &ToolClickCtx) {
    match editor.state.tool_pending {
        ToolPending::Idle => {
            editor.state.tool_pending = ToolPending::ArcCenter {
                center: ctx.resolved_id,
            };
        }
        ToolPending::ArcCenter { center } => {
            editor.state.tool_pending = ToolPending::ArcStart {
                center,
                start: ctx.resolved_id,
            };
        }
        ToolPending::ArcStart { center, start } => {
            let arc_id = SketchEntityId::new();
            let arc = ctx.flag(Entity::new(
                arc_id,
                ctx.plane_id,
                EntityKind::Arc {
                    center,
                    start,
                    end: ctx.resolved_id,
                    sweep_ccw: true,
                },
            ));
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(arc));
            });
            editor.state.tool_pending = ToolPending::Idle;
        }
        _ => {
            editor.state.tool_pending = ToolPending::ArcCenter {
                center: ctx.resolved_id,
            };
        }
    }
}

// v0.24 Track C — Tangent Arc. Two-click chained arc segment that mints
// an Arc tangent to the most recently committed Line whose end Point
// matches the first click. The dispatcher also emits a `TangentLineArc`
// constraint so the tangency survives further edits.
//
// - Click 1: stash the resolved Point as
//   `ToolPending::TangentArcFirst { first }`. Mirrors the Line tool's
//   first-click flow.
// - Click 2: locate a Line whose `end == first`. Compute the tangent
//   centre on the line's perpendicular bisector through `first` so the
//   arc starts off the line tangentially. Mint an Arc entity +
//   TangentLineArc constraint and chain back to Idle.
//
// Fallback: when no incident Line is found, the dispatcher mints a
// placeholder centre at the perpendicular bisector of the chord (no
// tangency reference) and publishes a warning via `solve_warnings`. The
// Arc still appears in the sketch so the user can constrain it manually
// if desired.
fn tangent_arc(editor: &mut crate::app::FootprintEditorState, ctx: &ToolClickCtx) {
    use signex_sketch::constraint::{Constraint, ConstraintKind};
    use signex_sketch::id::ConstraintId;

    match editor.state.tool_pending {
        ToolPending::TangentArcFirst { first } => {
            // Look up the first endpoint position +
            // any Line ending at `first`.
            let (first_pos, end_pos, incident_line): (
                (f64, f64),
                (f64, f64),
                Option<(SketchEntityId, (f64, f64))>,
            ) = {
                let sketch_ref = match editor.primitive().sketch.as_ref() {
                    Some(s) => s,
                    None => {
                        editor.state.tool_pending = ToolPending::Idle;
                        return;
                    }
                };
                let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                    sketch_ref
                        .entities
                        .iter()
                        .find(|e| e.id == id)
                        .and_then(|e| match e.kind {
                            EntityKind::Point { x, y } => Some((x, y)),
                            _ => None,
                        })
                };
                let first_p = match pos_of(first) {
                    Some(p) => p,
                    None => {
                        editor.state.tool_pending = ToolPending::Idle;
                        return;
                    }
                };
                let end_p = match pos_of(ctx.resolved_id) {
                    Some(p) => p,
                    None => {
                        editor.state.tool_pending = ToolPending::Idle;
                        return;
                    }
                };
                // Find a Line whose end matches `first`.
                // Prefer the most recently authored one
                // (last in the list) so chained sketches
                // pick up the immediately preceding
                // Line, not an unrelated old one.
                let line = sketch_ref.entities.iter().rev().find_map(|e| match e.kind {
                    EntityKind::Line { start, end } if end == first => {
                        pos_of(start).map(|p| (e.id, p))
                    }
                    EntityKind::Line { start, end } if start == first => {
                        pos_of(end).map(|p| (e.id, p))
                    }
                    _ => None,
                });
                (first_p, end_p, line)
            };

            // Compute the tangent centre.
            //
            // With an incident Line, the centre lies
            // on the line's perpendicular through
            // `first`. We pick the side of the chord
            // (`first` → `end_pos`) that lets the arc
            // reach `end` along that perpendicular,
            // and place the centre on the
            // perpendicular bisector of the chord
            // intersected with the line-perpendicular
            // through `first`. That intersection is
            // the unique circle that is tangent to
            // the line at `first` and passes through
            // `end_pos`.
            //
            // Without an incident Line, fall back to
            // the chord's perpendicular bisector
            // midpoint shifted by half-chord —
            // produces a 90° arc as a sane default.
            let (cx, cy) = match incident_line {
                Some((_, line_other_pos)) => {
                    // Line direction (line_other -> first)
                    let lx = first_pos.0 - line_other_pos.0;
                    let ly = first_pos.1 - line_other_pos.1;
                    let llen_sq = lx * lx + ly * ly;
                    if llen_sq <= 1e-12 {
                        // Degenerate; treat as no line.
                        let mx = (first_pos.0 + end_pos.0) * 0.5;
                        let my = (first_pos.1 + end_pos.1) * 0.5;
                        let dx = end_pos.0 - first_pos.0;
                        let dy = end_pos.1 - first_pos.1;
                        // Rotate 90° CCW for placeholder.
                        (mx + (-dy) * 0.5, my + dx * 0.5)
                    } else {
                        // Perpendicular to the line at first.
                        let llen = llen_sq.sqrt();
                        let nx = -ly / llen;
                        let ny = lx / llen;
                        // Centre is on the line through `first`
                        // along (nx, ny). Solve for the t such
                        // that |centre - end| = |centre - first|:
                        //   (first.x + t*nx - end.x)^2
                        //   + (first.y + t*ny - end.y)^2 = t^2
                        // Expanding:
                        //   |first - end|^2
                        //   + 2*t*((first.x - end.x)*nx + (first.y - end.y)*ny)
                        //   = 0
                        // → t = -|first - end|^2 /
                        //       (2 * ((first - end) · n))
                        let dx = first_pos.0 - end_pos.0;
                        let dy = first_pos.1 - end_pos.1;
                        let denom = 2.0 * (dx * nx + dy * ny);
                        let chord_sq = dx * dx + dy * dy;
                        if denom.abs() <= 1e-9 {
                            // end is on the line — tangent
                            // circle is undefined (would be
                            // infinite radius / a straight
                            // line). Fall back to the chord
                            // midpoint perpendicular.
                            let mx = (first_pos.0 + end_pos.0) * 0.5;
                            let my = (first_pos.1 + end_pos.1) * 0.5;
                            (mx + nx * 0.5, my + ny * 0.5)
                        } else {
                            let t = -chord_sq / denom;
                            (first_pos.0 + t * nx, first_pos.1 + t * ny)
                        }
                    }
                }
                None => {
                    // Placeholder centre — perpendicular
                    // to the chord at the midpoint, half
                    // chord length out (gives a 90°
                    // arc). The user will typically
                    // re-constrain manually.
                    editor
                        .state
                        .solve_warnings
                        .push("Tangent Arc: no incident line found, placeholder centre".into());
                    let mx = (first_pos.0 + end_pos.0) * 0.5;
                    let my = (first_pos.1 + end_pos.1) * 0.5;
                    let dx = end_pos.0 - first_pos.0;
                    let dy = end_pos.1 - first_pos.1;
                    // Rotate 90° CCW.
                    (mx + (-dy) * 0.5, my + dx * 0.5)
                }
            };

            // Mint the centre Point.
            let centre_id = SketchEntityId::new();
            let centre = ctx.flag(Entity::new(
                centre_id,
                ctx.plane_id,
                EntityKind::Point { x: cx, y: cy },
            ));
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(centre));
            });

            // Mint the Arc entity. Sweep direction
            // chosen so the arc opens away from the
            // incident line (when present); without a
            // line, default CCW.
            let arc_id = SketchEntityId::new();
            let sweep_ccw = match incident_line {
                Some((_, line_other_pos)) => {
                    // Cross product of (line_other -> first)
                    // and (first -> end) tells us which
                    // side of the line `end` is on.
                    let lx = first_pos.0 - line_other_pos.0;
                    let ly = first_pos.1 - line_other_pos.1;
                    let ex = end_pos.0 - first_pos.0;
                    let ey = end_pos.1 - first_pos.1;
                    // Cross > 0 → end is to the left of
                    // the incoming line direction → CCW
                    // arc opens left.
                    lx * ey - ly * ex >= 0.0
                }
                None => true,
            };
            let arc = ctx.flag(Entity::new(
                arc_id,
                ctx.plane_id,
                EntityKind::Arc {
                    center: centre_id,
                    start: first,
                    end: ctx.resolved_id,
                    sweep_ccw,
                },
            ));
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(arc));
            });

            // Add the TangentLineArc constraint when
            // we have an incident Line.
            if let Some((line_id, _)) = incident_line {
                let constraint = Constraint {
                    id: ConstraintId::new(),
                    kind: ConstraintKind::TangentLineArc {
                        line: line_id,
                        arc: arc_id,
                    },
                };
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(
                        state,
                        primitive,
                        SketchEdit::AddConstraint(constraint),
                    );
                });
            }

            editor.state.tool_pending = ToolPending::Idle;
        }
        _ => {
            // First click — stash the endpoint and
            // wait for click 2.
            editor.state.tool_pending = ToolPending::TangentArcFirst {
                first: ctx.resolved_id,
            };
        }
    }
}
