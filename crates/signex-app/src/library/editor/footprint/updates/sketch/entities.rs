//! Footprint sketch updates — entity placement & drag geometry concern.
//!
//! Carved out of the monolithic `sketch::apply` (ADR-0001 D1/D2). Arm
//! bodies are moved verbatim; each keeps its own inner `use`s.

use crate::library::editor::footprint::state::FootprintEditorState as CanvasState;
use crate::library::messages::FootprintEditorMsg;

pub(in crate::library::editor::footprint::updates) fn apply(
    editor: &mut crate::app::FootprintEditorState,
    msg: FootprintEditorMsg,
) {
    match msg {
        FootprintEditorMsg::SketchPlacePoint { x_mm, y_mm } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use signex_sketch::entity::{Entity, EntityKind};
            use signex_sketch::id::SketchEntityId;
            use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
            // Ensure the sketch has at least one plane so the entity has
            // somewhere to live.
            let plane_id = match editor.primitive().sketch.as_ref() {
                Some(s) if !s.planes.is_empty() => s.planes[0].id,
                _ => {
                    let pid = PlaneId::new();
                    let sketch = editor
                        .primitive_mut()
                        .sketch
                        .get_or_insert_with(signex_sketch::SketchData::default);
                    sketch.planes.push(Plane {
                        id: pid,
                        kind: PlaneKind::BoardTop,
                    });
                    pid
                }
            };
            let id = SketchEntityId::new();
            let mut entity = Entity::new(id, plane_id, EntityKind::Point { x: x_mm, y: y_mm });
            entity.construction = editor.state.construction_mode;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(entity));
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        FootprintEditorMsg::SketchMovePoint { id, dx, dy } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(
                    state,
                    primitive,
                    SketchEdit::MovePoint { id, dx, dy },
                );
            });
            // v0.16.0.1 fix — when the dragged Point is a pad's
            // centre, also translate that pad's outline-corner Points
            // by the same delta so the construction outline tracks
            // the pad. Without this the corner outline was stranded
            // at the previous centre after a sketch-mode drag.
            let centre_pad_idx = editor
                .state
                .pads
                .iter()
                .position(|p| p.sketch_entity_id == Some(id));
            if let Some(pad_idx) = centre_pad_idx {
                if let Some(corners) = editor.state.pads[pad_idx].corner_entity_ids {
                    for corner_id in corners {
                        editor.with_parts(|state, primitive| {
                            apply_sketch_edit_with_warnings(
                                state,
                                primitive,
                                SketchEdit::MovePoint {
                                    id: corner_id,
                                    dx,
                                    dy,
                                },
                            );
                        });
                    }
                }
                // Keep `EditorPad.position_mm` in sync so a Pads-mode
                // tab switch shows the pad at the new world position.
                editor.state.pads[pad_idx].position_mm.0 += dx;
                editor.state.pads[pad_idx].position_mm.1 += dy;
                editor.with_parts(|state, primitive| {
                    CanvasState::sync_pads_to_primitive(state, primitive);
                });
            }
            // v0.16.1 fix — when the dragged Point is one of any
            // pad's outline-corner Points, recompute that pad's bbox
            // from all 4 corner positions. Update the pad's
            // position_mm + size_mm AND rewrite the centre Point's
            // PadAttr.size_x_expr / size_y_expr so the bake re-emits
            // the pad at the new size. This is the "drag-corner-to-
            // resize-pad" behaviour the user expects when they grab
            // a corner of the construction outline.
            let corner_pad_idx = editor.state.pads.iter().position(|p| {
                p.corner_entity_ids
                    .as_ref()
                    .map(|ids| ids.contains(&id))
                    .unwrap_or(false)
            });
            if let Some(pad_idx) = corner_pad_idx {
                use signex_sketch::entity::EntityKind;
                let Some(corners) = editor.state.pads[pad_idx].corner_entity_ids else {
                    // `position()` above already required `is_some()`; this
                    // arm is unreachable in practice but propagating via
                    // early-let-else avoids the matching `.unwrap()` panic
                    // if a future refactor decouples the two.
                    return;
                };
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                if let Some(sketch) = editor.primitive().sketch.as_ref() {
                    for cid in corners {
                        if let Some(e) = sketch.entities.iter().find(|e| e.id == cid) {
                            if let EntityKind::Point { x, y } = e.kind {
                                if x < min_x {
                                    min_x = x;
                                }
                                if y < min_y {
                                    min_y = y;
                                }
                                if x > max_x {
                                    max_x = x;
                                }
                                if y > max_y {
                                    max_y = y;
                                }
                            }
                        }
                    }
                }
                if min_x.is_finite() && min_y.is_finite() {
                    let new_w = (max_x - min_x).max(0.05);
                    let new_h = (max_y - min_y).max(0.05);
                    let new_cx = (min_x + max_x) / 2.0;
                    let new_cy = (min_y + max_y) / 2.0;
                    let pad = &mut editor.state.pads[pad_idx];
                    let old_centre = pad.position_mm;
                    pad.position_mm = (new_cx, new_cy);
                    pad.size_mm = (new_w, new_h);
                    let centre_id = pad.sketch_entity_id;
                    // v0.18.12.1 bugfix — re-align the OTHER three
                    // corner Points to the new pad bbox. Previously
                    // only the dragged corner moved, leaving the
                    // pad rectangle (drawn at the new bbox) and the
                    // non-dragged corners stranded at their old
                    // positions — visible as the dashed-construction
                    // outline drifting off the rendered pad on
                    // subsequent corner drags.
                    let new_positions: [(f64, f64); 4] = [
                        (max_x, min_y), // ne
                        (max_x, max_y), // se
                        (min_x, max_y), // sw
                        (min_x, min_y), // nw
                    ];
                    for (corner_id, (target_x, target_y)) in
                        corners.iter().zip(new_positions.iter())
                    {
                        // Skip the corner the user just dragged — it's
                        // already at the right position, and emitting
                        // a zero-delta MovePoint would still trip the
                        // solver.
                        if *corner_id == id {
                            continue;
                        }
                        let cur = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == *corner_id))
                            .and_then(|e| {
                                if let signex_sketch::entity::EntityKind::Point { x, y } = e.kind {
                                    Some((x, y))
                                } else {
                                    None
                                }
                            });
                        if let Some((cx, cy)) = cur {
                            let cdx = *target_x - cx;
                            let cdy = *target_y - cy;
                            if cdx.abs() > 1e-9 || cdy.abs() > 1e-9 {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::MovePoint {
                                            id: *corner_id,
                                            dx: cdx,
                                            dy: cdy,
                                        },
                                    );
                                });
                            }
                        }
                    }
                    // Move the centre Point to the new bbox centre +
                    // rewrite the PadAttr size exprs so the bake
                    // emits the new size.
                    if let Some(centre_id) = centre_id {
                        let cdx = new_cx - old_centre.0;
                        let cdy = new_cy - old_centre.1;
                        if cdx.abs() > 1e-9 || cdy.abs() > 1e-9 {
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::MovePoint {
                                        id: centre_id,
                                        dx: cdx,
                                        dy: cdy,
                                    },
                                );
                            });
                        }
                        if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                            if let Some(centre) =
                                sketch.entities.iter_mut().find(|e| e.id == centre_id)
                            {
                                if let Some(attr) = centre.pad.as_mut() {
                                    attr.size_x_expr = format!("{:.4}mm", new_w);
                                    attr.size_y_expr = format!("{:.4}mm", new_h);
                                }
                            }
                        }
                    }
                    editor.with_parts(|state, primitive| {
                        CanvasState::sync_pads_to_primitive(state, primitive);
                    });
                }
            }
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        FootprintEditorMsg::SketchMoveLine { id, dx, dy } => {
            // v0.27 — drag a Line edge by translating both its
            // endpoints in one solver pass. The dispatcher reads
            // the Line's start/end IDs, then emits MovePoint for
            // each. The solver re-runs once after both moves so
            // H/V/Distance constraints converge correctly without
            // the brief mid-tick "one corner moved, the other
            // didn't" state a two-message split would produce.
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use signex_sketch::entity::EntityKind;
            let endpoints = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.entities.iter().find(|e| e.id == id))
                .and_then(|e| match e.kind {
                    EntityKind::Line { start, end } => Some((start, end)),
                    _ => None,
                });
            let Some((start, end)) = endpoints else {
                return;
            };
            // v0.27 — snapshot the line's pre-drag endpoint positions.
            // Used after the translate step to detect which bbox edge
            // the line lay on (so the matching pad can resize). Read
            // BEFORE the MovePoint passes shift these Points.
            let pre_drag_endpoints: Option<((f64, f64), (f64, f64))> =
                editor.primitive().sketch.as_ref().and_then(|s| {
                    let pos_of = |pid: signex_sketch::id::SketchEntityId| -> Option<(f64, f64)> {
                        s.entities.iter().find(|e| e.id == pid).and_then(|e| {
                            if let EntityKind::Point { x, y } = e.kind {
                                Some((x, y))
                            } else {
                                None
                            }
                        })
                    };
                    pos_of(start).zip(pos_of(end))
                });
            // v0.27 — gather the arc victim set BEFORE running any
            // moves so adjacency lookups read pre-drag positions.
            // Arc centres + the "other" tangent endpoint of any Arc
            // tangent to a moving line endpoint translate by the
            // same `(dx, dy)` as the rigid edge so rounded-rectangle
            // corners stay rigid (constant radius). The line's own
            // endpoints are handled separately below — they may
            // slide along an adjacent edge rather than translating
            // rigidly (Fusion-style "expand toward dragging").
            let mut arc_victims: std::collections::HashSet<signex_sketch::id::SketchEntityId> =
                std::collections::HashSet::new();
            if let Some(s) = editor.primitive().sketch.as_ref() {
                for e in &s.entities {
                    if let EntityKind::Arc {
                        start: a_s,
                        end: a_e,
                        center: a_c,
                        ..
                    } = e.kind
                    {
                        let touches = a_s == start || a_s == end || a_e == start || a_e == end;
                        if touches {
                            arc_victims.insert(a_c);
                            if a_s != start && a_s != end {
                                arc_victims.insert(a_s);
                            }
                            if a_e != start && a_e != end {
                                arc_victims.insert(a_e);
                            }
                        }
                    }
                }
            }
            // v0.27 — per-endpoint slide. If the endpoint connects
            // to exactly one OTHER line (closed polygon vertex),
            // slide the endpoint along that adjacent line so the
            // dragged edge only stretches/shrinks perpendicular and
            // the adjacent edges retain their direction. The pad
            // bbox case still produces the right answer here because
            // a rect pad's edge endpoints connect to perpendicular
            // edges — sliding along a perpendicular line by the
            // perpendicular drag delta is equivalent to translating.
            // When the endpoint has zero or ≥2 other lines (free
            // vertex, arc tangent, T-junction), fall back to rigid
            // translate so the existing pad / arc-corner flows keep
            // working.
            let read_pos = |pid: signex_sketch::id::SketchEntityId| -> Option<(f64, f64)> {
                editor
                    .primitive()
                    .sketch
                    .as_ref()
                    .and_then(|s| s.entities.iter().find(|e| e.id == pid))
                    .and_then(|e| match e.kind {
                        EntityKind::Point { x, y } => Some((x, y)),
                        _ => None,
                    })
            };
            // Find the unique other line connected to `endpoint`
            // (excluding the dragged line itself). Returns the far
            // endpoint of that line — the one we treat as the
            // slide pivot. Returns `None` when 0 or ≥2 other lines
            // meet at this endpoint.
            let find_far = |endpoint: signex_sketch::id::SketchEntityId|
                -> Option<signex_sketch::id::SketchEntityId> {
                let sketch = editor.primitive().sketch.as_ref()?;
                let mut found: Option<signex_sketch::id::SketchEntityId> = None;
                for e in &sketch.entities {
                    if e.id == id {
                        continue;
                    }
                    if let EntityKind::Line { start: ls, end: le } = e.kind {
                        let far = if ls == endpoint {
                            Some(le)
                        } else if le == endpoint {
                            Some(ls)
                        } else {
                            None
                        };
                        if let Some(f) = far {
                            if found.is_some() {
                                return None;
                            }
                            found = Some(f);
                        }
                    }
                }
                found
            };
            // 2D line-line intersection. `p1 + t*d1 = p2 + s*d2`.
            // Returns `None` for parallel / coincident lines.
            let intersect = |p1: (f64, f64),
                             d1: (f64, f64),
                             p2: (f64, f64),
                             d2: (f64, f64)|
             -> Option<(f64, f64)> {
                let det = d2.0 * d1.1 - d1.0 * d2.1;
                if det.abs() < 1e-9 {
                    return None;
                }
                let t = (d2.0 * (p2.1 - p1.1) - d2.1 * (p2.0 - p1.0)) / det;
                Some((p1.0 + t * d1.0, p1.1 + t * d1.1))
            };
            let target_for =
                |endpoint: signex_sketch::id::SketchEntityId, pos: (f64, f64)| -> (f64, f64) {
                    let rigid = (pos.0 + dx, pos.1 + dy);
                    let Some(far_id) = find_far(endpoint) else {
                        return rigid;
                    };
                    let Some(far_pos) = read_pos(far_id) else {
                        return rigid;
                    };
                    let Some((sx_pre, sy_pre)) = read_pos(start) else {
                        return rigid;
                    };
                    let Some((ex_pre, ey_pre)) = read_pos(end) else {
                        return rigid;
                    };
                    let line_d = (ex_pre - sx_pre, ey_pre - sy_pre);
                    let other_d = (pos.0 - far_pos.0, pos.1 - far_pos.1);
                    intersect(rigid, line_d, far_pos, other_d).unwrap_or(rigid)
                };
            let start_pos_opt = read_pos(start);
            let end_pos_opt = read_pos(end);
            if let (Some(start_pos), Some(end_pos)) = (start_pos_opt, end_pos_opt) {
                let start_target = target_for(start, start_pos);
                let end_target = target_for(end, end_pos);
                let start_delta = (start_target.0 - start_pos.0, start_target.1 - start_pos.1);
                let end_delta = (end_target.0 - end_pos.0, end_target.1 - end_pos.1);
                if start_delta.0.abs() > 1e-9 || start_delta.1.abs() > 1e-9 {
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::MovePoint {
                                id: start,
                                dx: start_delta.0,
                                dy: start_delta.1,
                            },
                        );
                    });
                }
                if end_delta.0.abs() > 1e-9 || end_delta.1.abs() > 1e-9 {
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::MovePoint {
                                id: end,
                                dx: end_delta.0,
                                dy: end_delta.1,
                            },
                        );
                    });
                }
            }
            for pid in arc_victims {
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(
                        state,
                        primitive,
                        SketchEdit::MovePoint { id: pid, dx, dy },
                    );
                });
            }
            // v0.27 — propagate the edge drag to the literal pad
            // bbox. Without this the sketch outline visibly resizes
            // but `pad.size_mm` / `pad.position_mm` (and the baked
            // pad rendering) stay frozen — the user sees the line
            // move while the pad copper underneath does nothing.
            //
            // Strategy: classify the line's pre-drag pose against
            // each pad's bbox to identify which side it lies on
            // (top / bottom / left / right). Only axis-aligned lines
            // qualify — diagonal sketch lines are never pad edges
            // for Rect / RoundRect / Oval / Chamfered shapes.
            const EDGE_EPS: f64 = 1e-4;
            if let Some(((sx, sy), (ex, ey))) = pre_drag_endpoints {
                let is_horizontal = (sy - ey).abs() < EDGE_EPS;
                let is_vertical = (sx - ex).abs() < EDGE_EPS;
                let pad_count = editor.state.pads.len();
                for pad_idx in 0..pad_count {
                    let bbox_data = {
                        let pad = &editor.state.pads[pad_idx];
                        if pad.corner_entity_ids.is_none() {
                            continue;
                        }
                        let (xmin, ymin, xmax, ymax) = pad.bbox_mm();
                        // Both endpoints must lie on the same bbox
                        // side; partial overlap (line extends past a
                        // corner) means it's not a pad edge.
                        let in_x = sx >= xmin - EDGE_EPS
                            && sx <= xmax + EDGE_EPS
                            && ex >= xmin - EDGE_EPS
                            && ex <= xmax + EDGE_EPS;
                        let in_y = sy >= ymin - EDGE_EPS
                            && sy <= ymax + EDGE_EPS
                            && ey >= ymin - EDGE_EPS
                            && ey <= ymax + EDGE_EPS;
                        if !in_x || !in_y {
                            continue;
                        }
                        let edge: Option<&str> = if is_horizontal && (sy - ymin).abs() < EDGE_EPS {
                            Some("top")
                        } else if is_horizontal && (sy - ymax).abs() < EDGE_EPS {
                            Some("bottom")
                        } else if is_vertical && (sx - xmin).abs() < EDGE_EPS {
                            Some("left")
                        } else if is_vertical && (sx - xmax).abs() < EDGE_EPS {
                            Some("right")
                        } else {
                            None
                        };
                        let Some(edge) = edge else {
                            continue;
                        };
                        let (new_xmin, new_ymin, new_xmax, new_ymax) = match edge {
                            "top" => (xmin, ymin + dy, xmax, ymax),
                            "bottom" => (xmin, ymin, xmax, ymax + dy),
                            "left" => (xmin + dx, ymin, xmax, ymax),
                            "right" => (xmin, ymin, xmax + dx, ymax),
                            _ => unreachable!(),
                        };
                        // Reject degenerate drags that would collapse
                        // or invert the bbox. The user has to release
                        // and re-grab if they want sub-50µm pads.
                        if new_xmax - new_xmin < 0.05 || new_ymax - new_ymin < 0.05 {
                            continue;
                        }
                        Some((new_xmin, new_ymin, new_xmax, new_ymax))
                    };
                    let Some((new_xmin, new_ymin, new_xmax, new_ymax)) = bbox_data else {
                        continue;
                    };
                    let new_w = new_xmax - new_xmin;
                    let new_h = new_ymax - new_ymin;
                    let new_cx = (new_xmin + new_xmax) / 2.0;
                    let new_cy = (new_ymin + new_ymax) / 2.0;
                    let (corners_arr, centre_id) = {
                        let pad = &editor.state.pads[pad_idx];
                        (
                            pad.corner_entity_ids.expect("checked is_some above"),
                            pad.sketch_entity_id,
                        )
                    };
                    // Rewrite the centre Point's PadAttr size exprs
                    // FIRST so the next solve+bake reads the new
                    // size. solve_and_bake → refresh_pads_from_primitive
                    // overwrites state.pads.size_mm from the bake
                    // output, so any earlier write here gets wiped.
                    // Updating PadAttr ahead of the solve makes the
                    // bake produce the resized pad on its own.
                    if let Some(centre_id) = centre_id
                        && let Some(sketch) = editor.primitive_mut().sketch.as_mut()
                        && let Some(centre) = sketch.entities.iter_mut().find(|e| e.id == centre_id)
                        && let Some(attr) = centre.pad.as_mut()
                    {
                        attr.size_x_expr = format!("{:.4}mm", new_w);
                        attr.size_y_expr = format!("{:.4}mm", new_h);
                    }
                    // Move the centre Point to the new bbox centre.
                    // Each apply_sketch_edit_with_warnings runs the
                    // solver + bake; refresh_pads_from_primitive then
                    // pulls state.pads from `footprint.pads`, so
                    // reading the centre's pre-edit position needs to
                    // happen RIGHT BEFORE this MovePoint emission.
                    if let Some(centre_id) = centre_id {
                        let cur_centre = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == centre_id))
                            .and_then(|e| {
                                if let EntityKind::Point { x, y } = e.kind {
                                    Some((x, y))
                                } else {
                                    None
                                }
                            });
                        if let Some((cur_cx, cur_cy)) = cur_centre {
                            let cdx = new_cx - cur_cx;
                            let cdy = new_cy - cur_cy;
                            if cdx.abs() > 1e-9 || cdy.abs() > 1e-9 {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::MovePoint {
                                            id: centre_id,
                                            dx: cdx,
                                            dy: cdy,
                                        },
                                    );
                                });
                            }
                        }
                    }
                    // Realign the 4 bbox corner Points to match the
                    // resized bbox. For Rect pads the line drag's
                    // victim loop already shifted the affected
                    // corners; for RoundRect / Oval / Chamfered the
                    // bbox corners aren't in `victims` so they need
                    // explicit catch-up here. Order: [ne, se, sw, nw]
                    // — see mint_pad_corner_outline.
                    let target_positions: [(f64, f64); 4] = [
                        (new_xmax, new_ymin), // ne
                        (new_xmax, new_ymax), // se
                        (new_xmin, new_ymax), // sw
                        (new_xmin, new_ymin), // nw
                    ];
                    for (corner_id, (target_x, target_y)) in
                        corners_arr.iter().zip(target_positions.iter())
                    {
                        let cur = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == *corner_id))
                            .and_then(|e| {
                                if let EntityKind::Point { x, y } = e.kind {
                                    Some((x, y))
                                } else {
                                    None
                                }
                            });
                        if let Some((cx, cy)) = cur {
                            let cdx = *target_x - cx;
                            let cdy = *target_y - cy;
                            if cdx.abs() > 1e-9 || cdy.abs() > 1e-9 {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::MovePoint {
                                            id: *corner_id,
                                            dx: cdx,
                                            dy: cdy,
                                        },
                                    );
                                });
                            }
                        }
                    }
                }
            }
            editor.with_parts(|state, primitive| {
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        FootprintEditorMsg::SketchResizeRoundPad {
            pad_idx,
            diameter_mm,
        } => {
            // v0.27 — round-pad diameter handle drag. Update three
            // sources of truth in lockstep so the on-canvas handle
            // motion, the bake output, and the parameter table stay
            // consistent:
            //   1. `pad.size_mm = (d, d)` — Editor mirror of the bbox.
            //   2. Circle entity radius — sketch-side geometry the
            //      Sketch overlay renders.
            //   3. `diameter_<slug>` parameter expression + the
            //      centre Point's PadAttr size_x_expr / size_y_expr —
            //      the bake reads these to emit the baked pad.
            let d = diameter_mm.max(0.05);
            let centre_id = editor
                .state
                .pads
                .get(pad_idx)
                .and_then(|p| p.sketch_entity_id);
            let diameter_param = editor
                .state
                .pads
                .get(pad_idx)
                .and_then(|p| p.shape_params.get("diameter").cloned());
            if let Some(pad) = editor.state.pads.get_mut(pad_idx) {
                pad.size_mm = (d, d);
            }
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                use signex_sketch::entity::EntityKind;
                if let Some(cid) = centre_id {
                    for entity in sketch.entities.iter_mut() {
                        if let EntityKind::Circle { center, radius } = &mut entity.kind {
                            if *center == cid {
                                *radius = d / 2.0;
                            }
                        }
                        if entity.id == cid {
                            if let Some(attr) = entity.pad.as_mut() {
                                attr.size_x_expr = format!("{:.4}mm", d);
                                attr.size_y_expr = format!("{:.4}mm", d);
                            }
                        }
                    }
                }
                if let Some(name) = diameter_param.as_deref() {
                    sketch
                        .parameters
                        .insert(name.to_string(), format!("{:.4}mm", d));
                }
            }
            editor.with_parts(|state, primitive| {
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        _ => unreachable!(
            "non-entity placement & drag geometry sketch variant routed to sketch_entities.rs"
        ),
    }
}
