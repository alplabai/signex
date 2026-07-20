//! Footprint sketch tools — curve edits (carved from `sketch_tools::apply`, ADR-0001 D2).
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
        SketchTool::Fillet => {
            // v0.27 — EDA Fillet. Two-click gesture:
            //   click 1: pick the first Line (we hit-test for
            //     a Line near the click — fall back to a
            //     warning if none).
            //   click 2: pick the second Line that shares an
            //     endpoint with the first. Compute tangent
            //     points at radius `r` from the shared corner
            //     along each line, splice in an Arc connecting
            //     them centred on the angle bisector, and
            //     shorten both lines to end at the tangent
            //     points.
            //
            // Radius source — `state.placement_input` (kind
            // FilletRadius) when the user typed one; else
            // `state.dimension_input`; else 0.5 mm.
            fn pick_line_at(
                sketch: &signex_sketch::SketchData,
                x: f64,
                y: f64,
            ) -> Option<SketchEntityId> {
                const TOL_MM: f64 = 0.30;
                let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                    sketch
                        .entities
                        .iter()
                        .find(|e| e.id == id)
                        .and_then(|e| match e.kind {
                            EntityKind::Point { x, y } => Some((x, y)),
                            _ => None,
                        })
                };
                let mut best: Option<(f64, SketchEntityId)> = None;
                for e in &sketch.entities {
                    if let EntityKind::Line { start, end } = e.kind {
                        let (Some(a), Some(b)) = (pos_of(start), pos_of(end)) else {
                            continue;
                        };
                        let dx = b.0 - a.0;
                        let dy = b.1 - a.1;
                        let llen2 = dx * dx + dy * dy;
                        if llen2 <= 1e-12 {
                            continue;
                        }
                        let t = ((x - a.0) * dx + (y - a.1) * dy) / llen2;
                        let tc = t.clamp(0.0, 1.0);
                        let px = a.0 + tc * dx;
                        let py = a.1 + tc * dy;
                        let d2 = (px - x).powi(2) + (py - y).powi(2);
                        if d2 <= TOL_MM * TOL_MM && best.as_ref().is_none_or(|(b2, _)| d2 < *b2) {
                            best = Some((d2, e.id));
                        }
                    }
                }
                best.map(|(_, id)| id)
            }

            let click_xy = (ctx.x_mm, ctx.y_mm);
            let radius_mm = editor
                .state
                .placement_input
                .as_ref()
                .filter(|p| p.kind == PlacementInputKind::FilletRadius)
                .and_then(|p| p.buffer.parse::<f64>().ok())
                .filter(|r| r.is_finite() && *r > 1e-9)
                .unwrap_or_else(|| {
                    editor
                        .state
                        .dimension_input
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|r| r.is_finite() && *r > 1e-9)
                        .unwrap_or(0.5)
                });

            match editor.state.tool_pending {
                ToolPending::FilletFirst { line: first_line } => {
                    let sketch_ref = match editor.primitive().sketch.as_ref() {
                        Some(s) => s,
                        None => {
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };
                    let second_line = match pick_line_at(sketch_ref, click_xy.0, click_xy.1) {
                        Some(id) if id != first_line => id,
                        _ => {
                            editor.state.solve_warnings.push(
                                        "Fillet: second click missed a different Line — pick the adjacent line".into(),
                                    );
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };
                    // Resolve the two Lines' endpoints.
                    let line_endpoints =
                        |id: SketchEntityId| -> Option<(SketchEntityId, SketchEntityId)> {
                            sketch_ref
                                .entities
                                .iter()
                                .find(|e| e.id == id)
                                .and_then(|e| match e.kind {
                                    EntityKind::Line { start, end } => Some((start, end)),
                                    _ => None,
                                })
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
                    let (a_s, a_e) = match line_endpoints(first_line) {
                        Some(p) => p,
                        None => {
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };
                    let (b_s, b_e) = match line_endpoints(second_line) {
                        Some(p) => p,
                        None => {
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };
                    // Find the shared corner Point.
                    let corner_id = if a_s == b_s || a_s == b_e {
                        a_s
                    } else if a_e == b_s || a_e == b_e {
                        a_e
                    } else {
                        editor.state.solve_warnings.push(
                                    "Fillet: the two Lines do not share an endpoint — bridge them with a Coincident constraint first".into(),
                                );
                        editor.state.tool_pending = ToolPending::Idle;
                        return;
                    };
                    // Identify the "outer" endpoint of each line.
                    let a_other = if a_s == corner_id { a_e } else { a_s };
                    let b_other = if b_s == corner_id { b_e } else { b_s };
                    let (cx, cy) = match pos_of(corner_id) {
                        Some(p) => p,
                        None => {
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };
                    let (ax, ay) = match pos_of(a_other) {
                        Some(p) => p,
                        None => {
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };
                    let (bx, by) = match pos_of(b_other) {
                        Some(p) => p,
                        None => {
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };
                    // Direction unit vectors away from corner.
                    let dax = ax - cx;
                    let day = ay - cy;
                    let dbx = bx - cx;
                    let dby = by - cy;
                    let alen = (dax * dax + day * day).sqrt();
                    let blen = (dbx * dbx + dby * dby).sqrt();
                    if alen <= 1e-9 || blen <= 1e-9 {
                        editor.state.tool_pending = ToolPending::Idle;
                        return;
                    }
                    let aux = dax / alen;
                    let auy = day / alen;
                    let bux = dbx / blen;
                    let buy = dby / blen;
                    // Half-angle between the two lines via dot product.
                    let cos_theta = (aux * bux + auy * buy).clamp(-1.0, 1.0);
                    let theta = cos_theta.acos();
                    if theta < 1e-3 || (std::f64::consts::PI - theta) < 1e-3 {
                        editor
                            .state
                            .solve_warnings
                            .push("Fillet: lines are colinear — nothing to round".into());
                        editor.state.tool_pending = ToolPending::Idle;
                        return;
                    }
                    let half = theta * 0.5;
                    // Distance from corner to tangent point along each line.
                    let trim = radius_mm / half.tan();
                    let cap = trim.min(alen * 0.999).min(blen * 0.999);
                    if cap < radius_mm * 0.05 {
                        editor.state.solve_warnings.push(
                            "Fillet: radius too large for these lines — pick a smaller r".into(),
                        );
                        editor.state.tool_pending = ToolPending::Idle;
                        return;
                    }
                    let r_used = cap * half.tan();
                    let ta_x = cx + aux * cap;
                    let ta_y = cy + auy * cap;
                    let tb_x = cx + bux * cap;
                    let tb_y = cy + buy * cap;
                    // Arc centre — on the angle bisector at
                    // distance r / sin(half) from the corner.
                    let bis_x = (aux + bux).abs() + (auy + buy).abs();
                    let _ = bis_x; // appease borrow checker, no-op
                    let mid_x = aux + bux;
                    let mid_y = auy + buy;
                    let mid_len = (mid_x * mid_x + mid_y * mid_y).sqrt().max(1e-9);
                    let bx_unit = mid_x / mid_len;
                    let by_unit = mid_y / mid_len;
                    let centre_off = r_used / half.sin();
                    let centre_x = cx + bx_unit * centre_off;
                    let centre_y = cy + by_unit * centre_off;
                    // Determine sweep direction — the arc opens
                    // away from the corner; pick CCW if the
                    // cross product (a -> b) is positive.
                    let cross = aux * buy - auy * bux;
                    let sweep_ccw = cross > 0.0;
                    // Mint two new tangent Points + an Arc; replace
                    // the corner endpoint references on the source
                    // Lines with the new tangent Points so the
                    // arc bridges them. We do this by updating the
                    // existing Line entities in-place via the
                    // sketch (no SketchEdit::EditLine variant
                    // exists yet — fall back to delete + re-add).
                    let ta_id = SketchEntityId::new();
                    let tb_id = SketchEntityId::new();
                    let centre_id = SketchEntityId::new();
                    let arc_id = SketchEntityId::new();
                    let entities = vec![
                        ctx.flag(Entity::new(
                            ta_id,
                            ctx.plane_id,
                            EntityKind::Point { x: ta_x, y: ta_y },
                        )),
                        ctx.flag(Entity::new(
                            tb_id,
                            ctx.plane_id,
                            EntityKind::Point { x: tb_x, y: tb_y },
                        )),
                        ctx.flag(Entity::new(
                            centre_id,
                            ctx.plane_id,
                            EntityKind::Point {
                                x: centre_x,
                                y: centre_y,
                            },
                        )),
                        ctx.flag(Entity::new(
                            arc_id,
                            ctx.plane_id,
                            EntityKind::Arc {
                                center: centre_id,
                                start: ta_id,
                                end: tb_id,
                                sweep_ccw,
                            },
                        )),
                    ];
                    for ent in entities {
                        editor.with_parts(|state, primitive| {
                            apply_sketch_edit_with_warnings(
                                state,
                                primitive,
                                SketchEdit::AddEntity(ent),
                            );
                        });
                    }
                    // Rewrite the two source Lines so the corner
                    // endpoint becomes the new tangent point.
                    // No public SketchEdit variant rewrites a
                    // Line's endpoints, so we mutate the schema
                    // directly and trigger a force-rebuild.
                    if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                        for e in sketch.entities.iter_mut() {
                            if e.id == first_line {
                                if let EntityKind::Line { start, end } = &mut e.kind {
                                    if *start == corner_id {
                                        *start = ta_id;
                                    } else if *end == corner_id {
                                        *end = ta_id;
                                    }
                                }
                            }
                            if e.id == second_line {
                                if let EntityKind::Line { start, end } = &mut e.kind {
                                    if *start == corner_id {
                                        *start = tb_id;
                                    } else if *end == corner_id {
                                        *end = tb_id;
                                    }
                                }
                            }
                        }
                    }
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
                    });
                    editor.state.tool_pending = ToolPending::Idle;
                }
                _ => {
                    // First click — pick the first Line.
                    let sketch_ref = match editor.primitive().sketch.as_ref() {
                        Some(s) => s,
                        None => {
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };
                    match pick_line_at(sketch_ref, click_xy.0, click_xy.1) {
                        Some(id) => {
                            editor.state.tool_pending = ToolPending::FilletFirst { line: id };
                        }
                        None => {
                            editor.state.solve_warnings.push(
                                        "Fillet: click missed any Line — try clicking closer to a line stroke".into(),
                                    );
                            editor.state.tool_pending = ToolPending::Idle;
                        }
                    }
                }
            }
        }
        SketchTool::Trim => {
            // v0.27 — EDA Trim. Single click on a Line: find
            // its self-intersections with all other Lines,
            // pick the two intersections that bracket the
            // click point on the line, split the line into
            // up-to-three segments, and remove the middle
            // segment containing the click. If only one
            // intersection exists, remove the side containing
            // the click. If no intersection exists, remove
            // the whole Line (Fusion-style "trim to nothing"
            // is a useful EDA fallback for stripping a stray
            // overlap).
            fn line_xy(
                sketch: &signex_sketch::SketchData,
                id: SketchEntityId,
            ) -> Option<((f64, f64), (f64, f64))> {
                let pos_of = |pid: SketchEntityId| -> Option<(f64, f64)> {
                    sketch
                        .entities
                        .iter()
                        .find(|e| e.id == pid)
                        .and_then(|e| match e.kind {
                            EntityKind::Point { x, y } => Some((x, y)),
                            _ => None,
                        })
                };
                sketch
                    .entities
                    .iter()
                    .find(|e| e.id == id)
                    .and_then(|e| match e.kind {
                        EntityKind::Line { start, end } => Some((pos_of(start)?, pos_of(end)?)),
                        _ => None,
                    })
            }
            fn pick_line_at_for_trim(
                sketch: &signex_sketch::SketchData,
                x: f64,
                y: f64,
            ) -> Option<SketchEntityId> {
                const TOL_MM: f64 = 0.30;
                let mut best: Option<(f64, SketchEntityId)> = None;
                for e in &sketch.entities {
                    if let EntityKind::Line { .. } = e.kind
                        && let Some(((ax, ay), (bx, by))) = line_xy(sketch, e.id)
                    {
                        let dx = bx - ax;
                        let dy = by - ay;
                        let llen2 = dx * dx + dy * dy;
                        if llen2 <= 1e-12 {
                            continue;
                        }
                        let t = ((x - ax) * dx + (y - ay) * dy) / llen2;
                        let tc = t.clamp(0.0, 1.0);
                        let px = ax + tc * dx;
                        let py = ay + tc * dy;
                        let d2 = (px - x).powi(2) + (py - y).powi(2);
                        if d2 <= TOL_MM * TOL_MM && best.as_ref().is_none_or(|(b2, _)| d2 < *b2) {
                            best = Some((d2, e.id));
                        }
                    }
                }
                best.map(|(_, id)| id)
            }

            let target_line = match editor.primitive().sketch.as_ref() {
                Some(s) => pick_line_at_for_trim(s, ctx.x_mm, ctx.y_mm),
                None => None,
            };
            let Some(target_line) = target_line else {
                editor.state.solve_warnings.push(
                    "Trim: click missed any Line — try clicking closer to a line stroke".into(),
                );
                editor.state.tool_pending = ToolPending::Idle;
                return;
            };
            // Compute intersections of `target_line` with every
            // other Line; collect parametric `t` values.
            let mut hits: Vec<f64> = Vec::new();
            if let Some(s) = editor.primitive().sketch.as_ref()
                && let Some(((ax, ay), (bx, by))) = line_xy(s, target_line)
            {
                let dx = bx - ax;
                let dy = by - ay;
                let llen2 = dx * dx + dy * dy;
                if llen2 > 1e-12 {
                    for e in &s.entities {
                        if e.id == target_line {
                            continue;
                        }
                        if let EntityKind::Line { .. } = e.kind
                            && let Some(((cx, cy), (ex, ey))) = line_xy(s, e.id)
                        {
                            let r_x = dx;
                            let r_y = dy;
                            let s_x = ex - cx;
                            let s_y = ey - cy;
                            let denom = r_x * s_y - r_y * s_x;
                            if denom.abs() <= 1e-12 {
                                continue;
                            }
                            let qx = cx - ax;
                            let qy = cy - ay;
                            let t = (qx * s_y - qy * s_x) / denom;
                            let u = (qx * r_y - qy * r_x) / denom;
                            if (1e-6..=1.0 - 1e-6).contains(&t) && (-1e-6..=1.0 + 1e-6).contains(&u)
                            {
                                hits.push(t);
                            }
                        }
                    }
                }
                // Click t-value on target_line.
                let click_t = if llen2 > 1e-12 {
                    ((ctx.x_mm - ax) * dx + (ctx.y_mm - ay) * dy) / llen2
                } else {
                    0.5
                };
                // Bracketing the click between the nearest
                // intersection below and above.
                let lo = hits
                    .iter()
                    .copied()
                    .filter(|t| *t < click_t)
                    .fold(0.0_f64, f64::max);
                let hi = hits
                    .iter()
                    .copied()
                    .filter(|t| *t > click_t)
                    .fold(1.0_f64, f64::min);
                // Three cases: full line (hits empty), half line
                // (one hit), middle slice (two hits).
                let trim_full = hits.is_empty();
                let trim_lo = (lo - 0.0).abs() < 1e-9;
                let trim_hi = (hi - 1.0).abs() < 1e-9;

                if trim_full {
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::DeleteEntity(target_line),
                        );
                    });
                } else if trim_lo && !trim_hi {
                    // Click is before the first intersection —
                    // shorten the line to start at `hi`.
                    let new_start = (ax + dx * hi, ay + dy * hi);
                    // Replace the line's start endpoint with a
                    // new Point at `new_start`.
                    let new_pid = SketchEntityId::new();
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(ctx.flag(Entity::new(
                                new_pid,
                                ctx.plane_id,
                                EntityKind::Point {
                                    x: new_start.0,
                                    y: new_start.1,
                                },
                            ))),
                        );
                    });
                    if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                        for e in sketch.entities.iter_mut() {
                            if e.id == target_line
                                && let EntityKind::Line { start, .. } = &mut e.kind
                            {
                                *start = new_pid;
                            }
                        }
                    }
                } else if trim_hi && !trim_lo {
                    // Click is after the last intersection —
                    // shorten the line to end at `lo`.
                    let new_end = (ax + dx * lo, ay + dy * lo);
                    let new_pid = SketchEntityId::new();
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(ctx.flag(Entity::new(
                                new_pid,
                                ctx.plane_id,
                                EntityKind::Point {
                                    x: new_end.0,
                                    y: new_end.1,
                                },
                            ))),
                        );
                    });
                    if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                        for e in sketch.entities.iter_mut() {
                            if e.id == target_line
                                && let EntityKind::Line { end, .. } = &mut e.kind
                            {
                                *end = new_pid;
                            }
                        }
                    }
                } else {
                    // Click bracketed by two intersections —
                    // split the line into two: [start..lo] and
                    // [hi..end]. We keep the original entity as
                    // the [start..lo] piece (rewriting its end)
                    // and mint a new Line for [hi..end].
                    let lo_pt = (ax + dx * lo, ay + dy * lo);
                    let hi_pt = (ax + dx * hi, ay + dy * hi);
                    let lo_pid = SketchEntityId::new();
                    let hi_pid = SketchEntityId::new();
                    let new_line_id = SketchEntityId::new();
                    // Capture the original end-point id so the
                    // mint of the second segment is correct.
                    let orig_end = if let Some(sk) = editor.primitive().sketch.as_ref() {
                        sk.entities
                            .iter()
                            .find(|e| e.id == target_line)
                            .and_then(|e| match e.kind {
                                EntityKind::Line { end, .. } => Some(end),
                                _ => None,
                            })
                    } else {
                        None
                    };
                    let Some(orig_end) = orig_end else {
                        editor.state.tool_pending = ToolPending::Idle;
                        return;
                    };
                    for ent in [
                        ctx.flag(Entity::new(
                            lo_pid,
                            ctx.plane_id,
                            EntityKind::Point {
                                x: lo_pt.0,
                                y: lo_pt.1,
                            },
                        )),
                        ctx.flag(Entity::new(
                            hi_pid,
                            ctx.plane_id,
                            EntityKind::Point {
                                x: hi_pt.0,
                                y: hi_pt.1,
                            },
                        )),
                        ctx.flag(Entity::new(
                            new_line_id,
                            ctx.plane_id,
                            EntityKind::Line {
                                start: hi_pid,
                                end: orig_end,
                            },
                        )),
                    ] {
                        editor.with_parts(|state, primitive| {
                            apply_sketch_edit_with_warnings(
                                state,
                                primitive,
                                SketchEdit::AddEntity(ent),
                            );
                        });
                    }
                    if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                        for e in sketch.entities.iter_mut() {
                            if e.id == target_line
                                && let EntityKind::Line { end, .. } = &mut e.kind
                            {
                                *end = lo_pid;
                            }
                        }
                    }
                }
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
                });
            }
            editor.state.tool_pending = ToolPending::Idle;
        }
        SketchTool::BreakTrack => {
            // #372 — Break Track. Single click on a sketch Line:
            // hit-test the click against every Line (0.30 mm
            // tolerance, nearest stroke wins — the same pick shape as
            // the Trim arm above), project the click onto that Line to
            // a parameter `t`, and hand off to the `split_line`
            // primitive (#360), which divides the Line into two Lines
            // meeting at a new mid Point.
            //
            // History — ONE undo step. `SketchToolClick` is classified
            // as a footprint mutation, so the router captured exactly
            // one pre-mutation snapshot before dispatching here (see
            // `updates/mod.rs::mutates_footprint_state` + its blanket
            // pre-push). This arm must NOT push again — mirroring the
            // Fillet / Trim arms — so a split stays a single undo step.
            //
            // Errors — graceful, no mutation. `split_line` leaves the
            // sketch byte-for-byte unchanged on every failure (a
            // degenerate line, a `t` too close to an endpoint, …). On
            // ANY `Err` — and on a click that misses every Line — we
            // surface a warning the same shape as the Trim miss, leave
            // the tool armed, and mutate nothing.
            fn pick_line_and_param(
                sketch: &signex_sketch::SketchData,
                x: f64,
                y: f64,
            ) -> Option<(SketchEntityId, f64)> {
                const TOL_MM: f64 = 0.30;
                let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                    sketch
                        .entities
                        .iter()
                        .find(|e| e.id == id)
                        .and_then(|e| match e.kind {
                            EntityKind::Point { x, y } => Some((x, y)),
                            _ => None,
                        })
                };
                let mut best: Option<(f64, SketchEntityId, f64)> = None;
                for e in &sketch.entities {
                    if let EntityKind::Line { start, end } = e.kind {
                        let (Some(a), Some(b)) = (pos_of(start), pos_of(end)) else {
                            continue;
                        };
                        let dx = b.0 - a.0;
                        let dy = b.1 - a.1;
                        let llen2 = dx * dx + dy * dy;
                        if llen2 <= 1e-12 {
                            continue;
                        }
                        // Unclamped parametric position of the click's
                        // foot on the (infinite) line; `split_line`
                        // validates `t ∈ (0, 1)` itself, so a click
                        // that projects just past an endpoint arrives
                        // as an out-of-range `t` and is rejected
                        // gracefully rather than silently clamped onto
                        // the endpoint.
                        let t = ((x - a.0) * dx + (y - a.1) * dy) / llen2;
                        // Distance is measured to the CLAMPED foot so
                        // the tolerance test stays on the finite
                        // segment (matching `pick_line_at_for_trim`).
                        let tc = t.clamp(0.0, 1.0);
                        let px = a.0 + tc * dx;
                        let py = a.1 + tc * dy;
                        let d2 = (px - x).powi(2) + (py - y).powi(2);
                        if d2 <= TOL_MM * TOL_MM && best.as_ref().is_none_or(|(b2, _, _)| d2 < *b2)
                        {
                            best = Some((d2, e.id, t));
                        }
                    }
                }
                best.map(|(_, id, t)| (id, t))
            }

            let pick = match editor.primitive().sketch.as_ref() {
                Some(s) => pick_line_and_param(s, ctx.x_mm, ctx.y_mm),
                None => None,
            };
            let Some((line_id, t)) = pick else {
                editor.state.solve_warnings.push(
                    "Break Track: click missed any Line — try clicking closer to a line stroke"
                        .into(),
                );
                editor.state.tool_pending = ToolPending::Idle;
                return;
            };

            // Run the #360 primitive against the live sketch. On `Err`
            // the sketch is left unchanged, so we only warn.
            let outcome = editor
                .primitive_mut()
                .sketch
                .as_mut()
                .map(|s| signex_sketch::split_line(s, line_id, t));
            match outcome {
                Some(Ok(result)) => {
                    // Re-select `line_a`. Only it keeps the original
                    // `start` and any closed-profile seed role
                    // (pour / keepout / cutout / pad seed); leaving the
                    // user selected on `line_b` would present it as
                    // "Unassigned" and invite them to re-tag it, which
                    // plants a SECOND seed on the loop and reproduces
                    // the double-emit `split_line` guards against (see
                    // its doc). Clear the secondary / extra selection
                    // so the split's `line_a` is the sole selection.
                    editor.state.selected_sketch = Some(result.line_a);
                    editor.state.selected_sketch_secondary = None;
                    editor.state.selected_sketch_extra.clear();
                    // Re-solve + bake so the two halves materialise in
                    // the footprint output — the same trailing rebuild
                    // the Trim arm runs.
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
                    });
                    // Surface a dropped-constraint note AFTER the
                    // rebuild: `solve_and_bake` clears `solve_warnings`
                    // on entry, so a warning pushed before it would be
                    // wiped. `split_line` only drops a `Midpoint`
                    // constraint (its midpoint belongs to neither half).
                    if !result.dropped_constraints.is_empty() {
                        editor.state.solve_warnings.push(
                            "Break Track: a Midpoint constraint on the split Line was dropped"
                                .into(),
                        );
                    }
                }
                Some(Err(e)) => {
                    editor
                        .state
                        .solve_warnings
                        .push(format!("Break Track: {e}"));
                }
                None => {}
            }
            editor.state.tool_pending = ToolPending::Idle;
        }
        _ => unreachable!("non-curve edits tool routed here"),
    }
}
