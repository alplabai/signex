//! Footprint sketch tools — transform & pattern (carved from `sketch_tools::apply`, ADR-0001 D2).
//!
//! Tool-branch bodies moved verbatim; the preamble locals they read
//! (`flag`, `ctx.plane_id`, `ctx.resolved_id`, raw click `ctx.x_mm`/`ctx.y_mm`) arrive via
//! [`ToolClickCtx`].

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
        SketchTool::Mirror => {
            // v0.22 Phase B1 + extension — Mirror tool.
            // Pre-condition: a Line entity must already be
            // selected via the Select tool; clicks while no
            // Line is selected are silent no-ops with a
            // warning surfaced via `solve_warnings`.
            //
            // The picked entity's geometry is reflected
            // across the selected Line and a fresh entity is
            // minted referencing mirrored copies of every
            // Point it touches. Each mirrored Point pair
            // gets a `SymmetricAboutLine` constraint so the
            // solver maintains symmetry through subsequent
            // edits (drag the source and the mirror tracks
            // it parametrically).
            //
            // Scope: Points / Lines / Arcs / Circles.
            // Mirrored Arcs flip `sweep_ccw` because
            // reflection inverts winding. Mirrored Circles
            // re-use the source radius (Circle's `radius` is
            // a literal, not a referenced Point, so it
            // round-trips unchanged).
            use signex_sketch::constraint::{Constraint, ConstraintKind};
            use signex_sketch::id::ConstraintId;

            let line_id = match editor.state.selected_sketch {
                Some(id) => id,
                None => {
                    editor.state.solve_warnings.push(
                                "Mirror: select a Line first (Select tool, click a Line, then click here to mirror)"
                                    .into(),
                            );
                    editor.state.tool_pending = ToolPending::Idle;
                    editor.canvas_cache.clear();
                    return;
                }
            };

            let sketch_ref = match editor.primitive().sketch.as_ref() {
                Some(s) => s,
                None => {
                    editor.state.tool_pending = ToolPending::Idle;
                    return;
                }
            };
            let line_endpoints = sketch_ref
                .entities
                .iter()
                .find(|e| e.id == line_id)
                .and_then(|e| match e.kind {
                    EntityKind::Line { start, end } => Some((start, end)),
                    _ => None,
                });
            let (a_id, b_id) = match line_endpoints {
                Some(p) => p,
                None => {
                    editor
                        .state
                        .solve_warnings
                        .push("Mirror: selection is not a Line — pick a Line entity first".into());
                    editor.state.tool_pending = ToolPending::Idle;
                    editor.canvas_cache.clear();
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
            let kind_of = sketch_ref
                .entities
                .iter()
                .find(|e| e.id == ctx.resolved_id)
                .map(|e| e.kind.clone());
            let kind_of = match kind_of {
                Some(k) => k,
                None => {
                    editor.state.tool_pending = ToolPending::Idle;
                    return;
                }
            };

            let (ax, ay) = match pos_of(a_id) {
                Some(p) => p,
                None => return,
            };
            let (bx, by) = match pos_of(b_id) {
                Some(p) => p,
                None => return,
            };
            let vx = bx - ax;
            let vy = by - ay;
            let v_dot_v = vx * vx + vy * vy;
            if v_dot_v <= 1e-12 {
                editor
                    .state
                    .solve_warnings
                    .push("Mirror: degenerate Line (zero length)".into());
                editor.state.tool_pending = ToolPending::Idle;
                editor.canvas_cache.clear();
                return;
            }
            let reflect = |px: f64, py: f64| -> (f64, f64) {
                let t = ((px - ax) * vx + (py - ay) * vy) / v_dot_v;
                let foot_x = ax + t * vx;
                let foot_y = ay + t * vy;
                (2.0 * foot_x - px, 2.0 * foot_y - py)
            };

            // Mirror a Point entity by ID: emits a new Point
            // at the reflected position and a
            // SymmetricAboutLine constraint linking source
            // and mirror. Returns the new Point's ID.
            // Captured by reference so the closure can be
            // called repeatedly for chained-Point entities.
            let mint_mirror_point = |editor: &mut crate::app::FootprintEditorState,
                                     pt_id: SketchEntityId,
                                     pos: (f64, f64)|
             -> SketchEntityId {
                let (rx, ry) = reflect(pos.0, pos.1);
                let new_id = SketchEntityId::new();
                let new_entity = ctx.flag(Entity::new(
                    new_id,
                    ctx.plane_id,
                    EntityKind::Point { x: rx, y: ry },
                ));
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(
                        state,
                        primitive,
                        SketchEdit::AddEntity(new_entity),
                    );
                });
                let constraint = Constraint {
                    id: ConstraintId::new(),
                    kind: ConstraintKind::SymmetricAboutLine {
                        p1: pt_id,
                        p2: new_id,
                        line: line_id,
                    },
                };
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(
                        state,
                        primitive,
                        SketchEdit::AddConstraint(constraint),
                    );
                });
                new_id
            };

            match kind_of {
                EntityKind::Point { x, y } => {
                    mint_mirror_point(editor, ctx.resolved_id, (x, y));
                }
                EntityKind::Line { start, end } => {
                    let s_pos = match pos_of(start) {
                        Some(p) => p,
                        None => return,
                    };
                    let e_pos = match pos_of(end) {
                        Some(p) => p,
                        None => return,
                    };
                    let new_start = mint_mirror_point(editor, start, s_pos);
                    let new_end = mint_mirror_point(editor, end, e_pos);
                    let new_line_id = SketchEntityId::new();
                    let new_line = ctx.flag(Entity::new(
                        new_line_id,
                        ctx.plane_id,
                        EntityKind::Line {
                            start: new_start,
                            end: new_end,
                        },
                    ));
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(new_line),
                        );
                    });
                }
                EntityKind::Arc {
                    center,
                    start,
                    end,
                    sweep_ccw,
                } => {
                    let c_pos = match pos_of(center) {
                        Some(p) => p,
                        None => return,
                    };
                    let s_pos = match pos_of(start) {
                        Some(p) => p,
                        None => return,
                    };
                    let e_pos = match pos_of(end) {
                        Some(p) => p,
                        None => return,
                    };
                    let new_center = mint_mirror_point(editor, center, c_pos);
                    let new_start = mint_mirror_point(editor, start, s_pos);
                    let new_end = mint_mirror_point(editor, end, e_pos);
                    // Reflection inverts winding — flip
                    // sweep_ccw so the mirrored arc traces
                    // the same arc on the other side.
                    let new_arc_id = SketchEntityId::new();
                    let new_arc = ctx.flag(Entity::new(
                        new_arc_id,
                        ctx.plane_id,
                        EntityKind::Arc {
                            center: new_center,
                            start: new_start,
                            end: new_end,
                            sweep_ccw: !sweep_ccw,
                        },
                    ));
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(new_arc),
                        );
                    });
                }
                EntityKind::Circle { center, radius } => {
                    let c_pos = match pos_of(center) {
                        Some(p) => p,
                        None => return,
                    };
                    let new_center = mint_mirror_point(editor, center, c_pos);
                    let new_circle_id = SketchEntityId::new();
                    let new_circle = ctx.flag(Entity::new(
                        new_circle_id,
                        ctx.plane_id,
                        EntityKind::Circle {
                            center: new_center,
                            radius,
                        },
                    ));
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(new_circle),
                        );
                    });
                }
            }
            editor.state.tool_pending = ToolPending::Idle;
        }
        SketchTool::Offset => {
            // v0.22 Phase B2 — Offset tool. Pre-condition: a
            // Line / Arc / Circle is in `selected_sketch`. The
            // click position determines which side of the
            // source curve the offset lands on. Offset
            // distance comes from `state.dimension_input`,
            // default 0.5 mm.
            //
            // Lines: emits a parallel Line at perpendicular
            // distance and adds (Parallel + DistancePtLine)
            // constraints so the relationship survives source
            // edits.
            //
            // Circles / Arcs: emits a concentric copy that
            // shares the source's centre Point so the centres
            // stay locked. The new radius is a literal
            // (source.radius ± dist) — the schema has no
            // radius-dimension constraint, so further radius
            // edits don't auto-propagate; the user can
            // re-offset or edit the literal directly.
            use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
            use signex_sketch::id::ConstraintId;

            let source_id = match editor.state.selected_sketch {
                Some(id) => id,
                None => {
                    editor.state.solve_warnings.push(
                                "Offset: select a Line / Arc / Circle first (Select tool, click the curve, then click on the side to offset)"
                                    .into(),
                            );
                    editor.state.tool_pending = ToolPending::Idle;
                    editor.canvas_cache.clear();
                    return;
                }
            };
            // v0.25 polish — prefer placement_input over the
            // legacy `dimension_input` text field. The
            // keypress-driven cursor overlay is the
            // discoverable path; `dimension_input` stays as
            // the Properties-panel fallback for users who
            // already have a value there.
            let dist_from_placement = editor
                .state
                .placement_input
                .as_ref()
                .filter(|p| p.kind == PlacementInputKind::OffsetDistance)
                .and_then(|p| p.buffer.parse::<f64>().ok())
                .filter(|d| d.is_finite() && *d > 1e-9);
            let dist = dist_from_placement.unwrap_or_else(|| {
                editor
                    .state
                    .dimension_input
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|d| d.is_finite() && *d > 1e-9)
                    .unwrap_or(0.5)
            });
            // Clear the buffer so the next Offset click
            // doesn''t accidentally reuse the old value.
            if dist_from_placement.is_some() {
                editor.state.placement_input = None;
            }

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
            let source_kind = sketch_ref
                .entities
                .iter()
                .find(|e| e.id == source_id)
                .map(|e| e.kind.clone());
            let source_kind = match source_kind {
                Some(k) => k,
                None => {
                    editor
                        .state
                        .solve_warnings
                        .push("Offset: selection no longer exists in the sketch".into());
                    editor.state.tool_pending = ToolPending::Idle;
                    editor.canvas_cache.clear();
                    return;
                }
            };

            match source_kind {
                EntityKind::Line { start, end } => {
                    let (ax, ay) = match pos_of(start) {
                        Some(p) => p,
                        None => return,
                    };
                    let (bx, by) = match pos_of(end) {
                        Some(p) => p,
                        None => return,
                    };
                    let dx = bx - ax;
                    let dy = by - ay;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len < 1e-9 {
                        editor
                            .state
                            .solve_warnings
                            .push("Offset: degenerate Line (zero length)".into());
                        editor.state.tool_pending = ToolPending::Idle;
                        editor.canvas_cache.clear();
                        return;
                    }
                    // Perpendicular unit vector. Sign from the
                    // cross of (line direction) × (click −
                    // line start): positive = click is on the
                    // (-dy, dx) side, negative = (dy, -dx)
                    // side.
                    let cx = ctx.x_mm - ax;
                    let cy = ctx.y_mm - ay;
                    let cross = dx * cy - dy * cx;
                    let sign = if cross >= 0.0 { 1.0 } else { -1.0 };
                    let nx = -dy / len * sign;
                    let ny = dx / len * sign;
                    let off_x = nx * dist;
                    let off_y = ny * dist;

                    let new_a = SketchEntityId::new();
                    let new_b = SketchEntityId::new();
                    let new_line_id = SketchEntityId::new();
                    let a_entity = ctx.flag(Entity::new(
                        new_a,
                        ctx.plane_id,
                        EntityKind::Point {
                            x: ax + off_x,
                            y: ay + off_y,
                        },
                    ));
                    let b_entity = ctx.flag(Entity::new(
                        new_b,
                        ctx.plane_id,
                        EntityKind::Point {
                            x: bx + off_x,
                            y: by + off_y,
                        },
                    ));
                    let new_line = ctx.flag(Entity::new(
                        new_line_id,
                        ctx.plane_id,
                        EntityKind::Line {
                            start: new_a,
                            end: new_b,
                        },
                    ));
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(a_entity),
                        );
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(b_entity),
                        );
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(new_line),
                        );
                    });
                    // Parallel + DistancePtLine on the start
                    // endpoint pins the offset distance
                    // parametrically. The end endpoint is left
                    // free along the offset line direction —
                    // the user can drag it without breaking
                    // the offset relationship.
                    let parallel = Constraint {
                        id: ConstraintId::new(),
                        kind: ConstraintKind::Parallel {
                            l1: source_id,
                            l2: new_line_id,
                        },
                    };
                    let dist_constraint = Constraint {
                        id: ConstraintId::new(),
                        kind: ConstraintKind::DistancePtLine {
                            point: new_a,
                            line: source_id,
                            target: DimTarget::Literal(dist),
                        },
                    };
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddConstraint(parallel),
                        );
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddConstraint(dist_constraint),
                        );
                    });
                }
                EntityKind::Circle { center, radius } => {
                    let (cx, cy) = match pos_of(center) {
                        Some(p) => p,
                        None => return,
                    };
                    // Click distance from centre — inside the
                    // circle = shrink (-dist), outside =
                    // expand (+dist). Clamp to a positive
                    // radius so we don't mint a degenerate
                    // shape.
                    let click_r = ((ctx.x_mm - cx).powi(2) + (ctx.y_mm - cy).powi(2)).sqrt();
                    let signed = if click_r < radius { -dist } else { dist };
                    let new_radius = (radius + signed).max(1e-6);
                    let new_circle_id = SketchEntityId::new();
                    let new_circle = ctx.flag(Entity::new(
                        new_circle_id,
                        ctx.plane_id,
                        EntityKind::Circle {
                            center,
                            radius: new_radius,
                        },
                    ));
                    // v0.23 — parametric link: mint an anchor
                    // Point on the new circle and pin its
                    // distance to the source circle to
                    // `signed`. Combined with a DistancePtCircle
                    // on the new circle (target=0), this
                    // forces `new_radius = source_radius +
                    // signed` through the solver — so when
                    // the user edits the target via the
                    // Properties panel later, the new
                    // circle's radius follows.
                    let scale = if click_r > 1e-9 {
                        new_radius / click_r
                    } else {
                        1.0
                    };
                    let anchor_id = SketchEntityId::new();
                    let anchor = ctx.flag(Entity::new(
                        anchor_id,
                        ctx.plane_id,
                        EntityKind::Point {
                            x: cx + (ctx.x_mm - cx) * scale,
                            y: cy + (ctx.y_mm - cy) * scale,
                        },
                    ));
                    let on_new_circle = Constraint {
                        id: ConstraintId::new(),
                        kind: ConstraintKind::DistancePtCircle {
                            point: anchor_id,
                            circle: new_circle_id,
                            target: DimTarget::Literal(0.0),
                        },
                    };
                    let parametric_offset = Constraint {
                        id: ConstraintId::new(),
                        kind: ConstraintKind::DistancePtCircle {
                            point: anchor_id,
                            circle: source_id,
                            target: DimTarget::Literal(signed),
                        },
                    };
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(anchor),
                        );
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(new_circle),
                        );
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddConstraint(on_new_circle),
                        );
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddConstraint(parametric_offset),
                        );
                    });
                }
                EntityKind::Arc {
                    center,
                    start,
                    end,
                    sweep_ccw,
                } => {
                    let (cx, cy) = match pos_of(center) {
                        Some(p) => p,
                        None => return,
                    };
                    let (sx, sy) = match pos_of(start) {
                        Some(p) => p,
                        None => return,
                    };
                    let (ex, ey) = match pos_of(end) {
                        Some(p) => p,
                        None => return,
                    };
                    // Source radius from start position;
                    // direction from start angle.
                    let source_r = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                    let click_r = ((ctx.x_mm - cx).powi(2) + (ctx.y_mm - cy).powi(2)).sqrt();
                    let signed = if click_r < source_r { -dist } else { dist };
                    let new_r = (source_r + signed).max(1e-6);
                    let scale = new_r / source_r.max(1e-9);

                    let new_start = SketchEntityId::new();
                    let new_end = SketchEntityId::new();
                    let new_arc_id = SketchEntityId::new();
                    let s_entity = ctx.flag(Entity::new(
                        new_start,
                        ctx.plane_id,
                        EntityKind::Point {
                            x: cx + (sx - cx) * scale,
                            y: cy + (sy - cy) * scale,
                        },
                    ));
                    let e_entity = ctx.flag(Entity::new(
                        new_end,
                        ctx.plane_id,
                        EntityKind::Point {
                            x: cx + (ex - cx) * scale,
                            y: cy + (ey - cy) * scale,
                        },
                    ));
                    let new_arc = ctx.flag(Entity::new(
                        new_arc_id,
                        ctx.plane_id,
                        EntityKind::Arc {
                            center,
                            start: new_start,
                            end: new_end,
                            sweep_ccw,
                        },
                    ));
                    // v0.23 — parametric link: pin both new
                    // endpoints to be `signed` away from the
                    // source arc's underlying circle. Since
                    // both arcs share the same `center`, this
                    // forces the new arc's radius to track
                    // source_radius + signed through the
                    // solver. End Point's angle is left free
                    // — the user can drag it without breaking
                    // the parametric distance.
                    let dist_start = Constraint {
                        id: ConstraintId::new(),
                        kind: ConstraintKind::DistancePtCircle {
                            point: new_start,
                            circle: source_id,
                            target: DimTarget::Literal(signed),
                        },
                    };
                    let dist_end = Constraint {
                        id: ConstraintId::new(),
                        kind: ConstraintKind::DistancePtCircle {
                            point: new_end,
                            circle: source_id,
                            target: DimTarget::Literal(signed),
                        },
                    };
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(s_entity),
                        );
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(e_entity),
                        );
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(new_arc),
                        );
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddConstraint(dist_start),
                        );
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddConstraint(dist_end),
                        );
                    });
                }
                EntityKind::Point { .. } => {
                    editor
                        .state
                        .solve_warnings
                        .push("Offset: selection is a Point — pick a Line / Arc / Circle".into());
                }
            }
            editor.state.tool_pending = ToolPending::Idle;
        }
        SketchTool::RectPattern => {
            // v0.22 Phase B3 — Rectangular Pattern. Click 1
            // picks the source entity (whatever was clicked,
            // including a freshly-minted Point if the click
            // missed everything). Mints a default 2×2 grid
            // with 5 mm spacing, sequential numbering. User
            // edits via JSON until a Properties sub-form
            // lands.
            use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
            let array = Array {
                id: ArrayId::new(),
                kind: ArrayKind::Grid {
                    source: ctx.resolved_id,
                    nx_expr: "2".into(),
                    ny_expr: "2".into(),
                    dx_expr: "5mm".into(),
                    dy_expr: "5mm".into(),
                    depopulation: None,
                },
                numbering: NumberingScheme::default(),
            };
            let sketch = editor
                .primitive_mut()
                .sketch
                .get_or_insert_with(signex_sketch::SketchData::default);
            sketch.arrays.push(array);
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.state.tool_pending = ToolPending::Idle;
        }
        SketchTool::CircularPattern => {
            // v0.22 Phase B4 — Circular Pattern. Click 1
            // picks the source entity. The polar array
            // requires a centre Point — mint a fresh one
            // 5 mm to the right of the click position so the
            // array doesn't all stack on the source. Default
            // count 4, sweep 360°.
            use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
            let centre_id = SketchEntityId::new();
            let centre = ctx.flag(Entity::new(
                centre_id,
                ctx.plane_id,
                EntityKind::Point {
                    x: ctx.x_mm + 5.0,
                    y: ctx.y_mm,
                },
            ));
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(centre));
            });
            let array = Array {
                id: ArrayId::new(),
                kind: ArrayKind::Polar {
                    source: ctx.resolved_id,
                    center: centre_id,
                    count_expr: "4".into(),
                    sweep_angle_expr: "360deg".into(),
                    depopulation: None,
                },
                numbering: NumberingScheme::default(),
            };
            let sketch = editor
                .primitive_mut()
                .sketch
                .get_or_insert_with(signex_sketch::SketchData::default);
            sketch.arrays.push(array);
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.state.tool_pending = ToolPending::Idle;
        }
        _ => unreachable!("non-transform & pattern tool routed here"),
    }
}
