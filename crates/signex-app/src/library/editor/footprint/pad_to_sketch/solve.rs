//! Post-solve "reverse mirror" helpers — when a solver run rewrites
//! a sketch parameter (e.g. via the parameter table), these helpers
//! propagate the resolved value back into the literal pad-stack
//! geometry so the canvas + Pads-mode Properties stay in sync.
//!
//! All helpers follow the same defensive pattern: skip pads whose
//! shape doesn't carry the relevant binding, and silently early-out
//! when the bound parameter isn't present in `resolved`.

use std::collections::HashMap;

use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;

use super::super::state::FootprintEditorState;
use super::helpers::set_point_xy;

/// v0.24 Phase 3 (Track A4) — RoundRect: re-derive the
/// `EditorPad.stack.corner_radius_pct` value from the live
/// `corner_r_<slug>` parameter in `resolved`.
pub fn mirror_solve_to_pad_stack(
    state: &mut FootprintEditorState,
    resolved: &HashMap<String, f64>,
) {
    for pad in state.pads.iter_mut() {
        let Some(parameter_name) = pad.shape_params.get("corner_r") else {
            continue;
        };
        let Some(corner_r_mm) = resolved.get(parameter_name).copied() else {
            tracing::warn!(
                target: "signex::v024",
                "mirror_solve_to_pad_stack: parameter {parameter_name} missing from resolved \
                 map; skipping pad {}",
                pad.number
            );
            continue;
        };
        let min_dim = pad.size_mm.0.min(pad.size_mm.1);
        if min_dim <= f64::EPSILON {
            tracing::warn!(
                target: "signex::v024",
                "mirror_solve_to_pad_stack: pad {} has zero/negative min dimension; skipping",
                pad.number
            );
            continue;
        }
        // ratio = corner_r / min(W,H) ∈ [0..0.5]; pct = ratio * 100.
        // A radius_ratio > 0.5 is geometrically degenerate so the
        // mirror caps the surfaced value at 50.
        let pct = (corner_r_mm / min_dim) * 100.0;
        pad.stack.corner_radius_pct = Some(pct.clamp(0.0, 50.0));
    }
}

/// v0.25 polish — Oval reverse-mirror: when the user edits
/// `width_<slug>` or `height_<slug>` from the Properties panel, the
/// resolved value should also propagate back to `pad.size_mm`.
pub fn mirror_solve_to_oval_size(
    state: &mut FootprintEditorState,
    resolved: &HashMap<String, f64>,
) {
    for pad in state.pads.iter_mut() {
        let Some(width_param) = pad.shape_params.get("width") else {
            continue;
        };
        let Some(height_param) = pad.shape_params.get("height") else {
            continue;
        };
        let Some(w) = resolved.get(width_param).copied() else {
            tracing::warn!(
                target: "signex::v025",
                "mirror_solve_to_oval_size: width parameter {width_param} missing \
                 from resolved map; skipping pad {}",
                pad.number
            );
            continue;
        };
        let Some(h) = resolved.get(height_param).copied() else {
            tracing::warn!(
                target: "signex::v025",
                "mirror_solve_to_oval_size: height parameter {height_param} missing \
                 from resolved map; skipping pad {}",
                pad.number
            );
            continue;
        };
        if w <= f64::EPSILON || h <= f64::EPSILON {
            tracing::warn!(
                target: "signex::v025",
                "mirror_solve_to_oval_size: pad {} resolved to non-positive size \
                 ({w}, {h}); skipping",
                pad.number
            );
            continue;
        }
        pad.size_mm = (w, h);
    }
}

/// v0.24 Track A6 — Chamfered pads: re-derive the chamfer anchor
/// Point coordinates from the resolved `chamfer_len_<slug>` parameter.
pub fn mirror_solve_to_chamfer_anchors(
    state: &FootprintEditorState,
    sketch: &mut SketchData,
    resolved: &HashMap<String, f64>,
) {
    for pad in state.pads.iter() {
        let Some(parameter_name) = pad.shape_params.get("chamfer_len") else {
            continue;
        };
        let Some(chamfer_len_mm) = resolved.get(parameter_name).copied() else {
            tracing::warn!(
                target: "signex::v024",
                "mirror_solve_to_chamfer_anchors_with_sketch: parameter {parameter_name} \
                 missing from resolved map; skipping pad {}",
                pad.number
            );
            continue;
        };
        let bbox = pad.bbox_mm();
        let (xmin, ymin, xmax, ymax) = bbox;
        let (w, h) = pad.size_mm;
        let r = chamfer_len_mm.max(0.0).min(w.min(h) / 2.0);

        let corners: [(&str, &str, (f64, f64), (f64, f64)); 4] = [
            (
                "chamfer_ne_anchor1",
                "chamfer_ne_anchor2",
                (xmax - r, ymin),
                (xmax, ymin + r),
            ),
            (
                "chamfer_se_anchor1",
                "chamfer_se_anchor2",
                (xmax, ymax - r),
                (xmax - r, ymax),
            ),
            (
                "chamfer_sw_anchor1",
                "chamfer_sw_anchor2",
                (xmin + r, ymax),
                (xmin, ymax - r),
            ),
            (
                "chamfer_nw_anchor1",
                "chamfer_nw_anchor2",
                (xmin, ymin + r),
                (xmin + r, ymin),
            ),
        ];

        for (key1, key2, pos1, pos2) in corners {
            move_anchor_via_sidecar(pad, sketch, key1, pos1);
            move_anchor_via_sidecar(pad, sketch, key2, pos2);
        }
    }
}

/// v0.24 Phase 6 — RoundRect: rewrite the per-corner Arc-centre Point
/// + the two adjacent anchor Points so the rendered geometry matches
/// the resolved radius.
pub fn mirror_solve_to_round_rect_geometry(
    state: &FootprintEditorState,
    sketch: &mut SketchData,
    resolved: &HashMap<String, f64>,
) {
    for pad in state.pads.iter() {
        let Some(shared_param) = pad.shape_params.get("corner_r") else {
            continue;
        };
        let Some(shared_r) = resolved.get(shared_param).copied() else {
            tracing::warn!(
                target: "signex::v024",
                "mirror_solve_to_round_rect_geometry: shared parameter \
                 {shared_param} missing from resolved map; skipping pad {}",
                pad.number
            );
            continue;
        };
        let bbox = pad.bbox_mm();
        let (xmin, ymin, xmax, ymax) = bbox;
        let (w, h) = pad.size_mm;
        let half_min = w.min(h) / 2.0;

        let arc_keys: [&str; 4] = [
            "corner_r_ne_arc",
            "corner_r_se_arc",
            "corner_r_sw_arc",
            "corner_r_nw_arc",
        ];
        let per_corner_keys: [&str; 4] =
            ["corner_r_ne", "corner_r_se", "corner_r_sw", "corner_r_nw"];

        // Per-corner expected positions for the Arc's three Point refs
        // given a resolved radius `r`. Order matches mint's arc_keys.
        let positions = |r: f64| -> [((f64, f64), (f64, f64), (f64, f64)); 4] {
            [
                ((xmax - r, ymin + r), (xmax - r, ymin), (xmax, ymin + r)), // NE
                ((xmax - r, ymax - r), (xmax, ymax - r), (xmax - r, ymax)), // SE
                ((xmin + r, ymax - r), (xmin + r, ymax), (xmin, ymax - r)), // SW
                ((xmin + r, ymin + r), (xmin, ymin + r), (xmin + r, ymin)), // NW
            ]
        };

        for (idx, (arc_key, per_corner_key)) in
            arc_keys.iter().zip(per_corner_keys.iter()).enumerate()
        {
            // Per-corner override wins if present (Phase 3 A3 unlink).
            let r = if let Some(per_corner_param) = pad.shape_params.get(*per_corner_key) {
                resolved.get(per_corner_param).copied().unwrap_or(shared_r)
            } else {
                shared_r
            };
            let r = r.max(0.0).min(half_min);

            let Some(arc_id) = sidecar_to_id(pad, *arc_key) else {
                continue;
            };

            // Read the Arc's three Point references — they didn't move,
            // only their target coordinates change.
            let (centre_id, start_id, end_id) = match sketch
                .entities
                .iter()
                .find(|e| e.id == arc_id)
                .map(|e| &e.kind)
            {
                Some(signex_sketch::entity::EntityKind::Arc {
                    center, start, end, ..
                }) => (*center, *start, *end),
                _ => continue,
            };

            let pos_table = positions(r);
            let (centre_pos, start_pos, end_pos) = pos_table[idx];
            set_point_xy(sketch, centre_id, centre_pos.0, centre_pos.1);
            set_point_xy(sketch, start_id, start_pos.0, start_pos.1);
            set_point_xy(sketch, end_id, end_pos.0, end_pos.1);
        }
    }
}

/// v0.24 Phase 6 — Oval: rewrite the 4 anchor Points + 2 arc-centre
/// Points based on the resolved width / height parameters.
pub fn mirror_solve_to_oval_geometry(
    state: &FootprintEditorState,
    sketch: &mut SketchData,
    resolved: &HashMap<String, f64>,
) {
    for pad in state.pads.iter() {
        let Some(width_param) = pad.shape_params.get("width") else {
            continue;
        };
        let Some(height_param) = pad.shape_params.get("height") else {
            continue;
        };
        let Some(width_mm) = resolved.get(width_param).copied() else {
            continue;
        };
        let Some(height_mm) = resolved.get(height_param).copied() else {
            continue;
        };

        let (cx, cy) = pad.position_mm;
        let (w, h) = pad.size_mm;
        let xmin = cx - w / 2.0;
        let xmax = cx + w / 2.0;
        let ymin = cy - h / 2.0;
        let ymax = cy + h / 2.0;
        let wide = width_mm >= height_mm;
        let long_axis = width_mm.max(height_mm);
        let short_axis = width_mm.min(height_mm);
        let inset = (long_axis - short_axis) / 2.0;

        let anchor_positions: [(f64, f64); 4] = if wide {
            [
                (xmin + inset, ymin),
                (xmax - inset, ymin),
                (xmax - inset, ymax),
                (xmin + inset, ymax),
            ]
        } else {
            [
                (xmax, ymin + inset),
                (xmax, ymax - inset),
                (xmin, ymax - inset),
                (xmin, ymin + inset),
            ]
        };
        let centre_positions: [(f64, f64); 2] = if wide {
            [
                (xmin + inset, (ymin + ymax) / 2.0),
                (xmax - inset, (ymin + ymax) / 2.0),
            ]
        } else {
            [
                ((xmin + xmax) / 2.0, ymin + inset),
                ((xmin + xmax) / 2.0, ymax - inset),
            ]
        };

        for (idx, target) in anchor_positions.iter().enumerate() {
            move_anchor_via_sidecar(pad, sketch, &format!("oval_anchor_{idx}"), *target);
        }
        for (idx, target) in centre_positions.iter().enumerate() {
            move_anchor_via_sidecar(pad, sketch, &format!("oval_centre_{idx}"), *target);
        }
    }
}

/// Resolve a `pad.shape_params[key]` UUID-slug sidecar into a
/// `SketchEntityId`. Returns `None` when the key is absent or its
/// value isn't a valid UUID.
fn sidecar_to_id(pad: &super::super::state::EditorPad, key: &str) -> Option<SketchEntityId> {
    let slug = pad.shape_params.get(key)?;
    uuid::Uuid::parse_str(slug).ok().map(SketchEntityId)
}

/// Look up an anchor by sidecar key and reposition the matching
/// Point. Silently no-ops when the sidecar key is absent / unparseable
/// or the entity isn't a Point.
fn move_anchor_via_sidecar(
    pad: &super::super::state::EditorPad,
    sketch: &mut SketchData,
    key: &str,
    target: (f64, f64),
) {
    if let Some(id) = sidecar_to_id(pad, key) {
        set_point_xy(sketch, id, target.0, target.1);
    }
}
