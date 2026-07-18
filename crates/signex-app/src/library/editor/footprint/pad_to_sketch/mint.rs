//! Per-shape parametric geometry minting. Each `mint_*_pad_geometry`
//! function takes a fresh centre `Point` ID (pushed by the caller),
//! mints additional geometry (Lines / Arcs / Circle / extra Points)
//! and the matching shape parameters, and returns the bbox-corner IDs
//! that go into `EditorPad.corner_entity_ids`.

use signex_library::primitive::footprint::ChamferedCorners as LibChamferedCorners;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::PlaneId;
use signex_sketch::sketch::SketchData;

use super::super::state::EditorPad;
use super::helpers::{
    bbox_corner_points, bind_shape_param, push_arc_ccw, push_construction_line,
    push_construction_point, push_line, push_point,
};

/// v0.16 — mint 4 corner Points + 4 Lines outlining a pad's bbox.
/// Returns the corner IDs in `[ne, se, sw, nw]` order so the caller
/// can store them on `EditorPad.corner_entity_ids` and reposition
/// them on later pad moves. Both the corner Points and the Lines
/// connecting them are flagged `construction = true` so
/// `signex_bake::bake_pads` skips them and they don't double up the
/// rendered pad geometry.
pub(super) fn mint_pad_corner_outline(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    pad: &EditorPad,
) -> [SketchEntityId; 4] {
    let bbox = pad.bbox_mm();
    let positions: [(f64, f64); 4] = [
        (bbox.2, bbox.1), // ne
        (bbox.2, bbox.3), // se
        (bbox.0, bbox.3), // sw
        (bbox.0, bbox.1), // nw
    ];
    let ids: [SketchEntityId; 4] = std::array::from_fn(|i| {
        let (x, y) = positions[i];
        push_construction_point(sketch, plane_id, x, y)
    });
    // 4 Lines around the loop — N (ne→nw), W (nw→sw), S (sw→se),
    // E (se→ne). Construction-only.
    for (a, b) in [
        (ids[0], ids[3]),
        (ids[3], ids[2]),
        (ids[2], ids[1]),
        (ids[1], ids[0]),
    ] {
        push_construction_line(sketch, plane_id, a, b);
    }
    ids
}

/// v0.24 Track A — mint a Round pad's geometry: 1 Circle entity
/// referencing the centre Point + a `diameter_<slug>` sketch
/// parameter recording the literal diameter for parametric edits.
pub(super) fn mint_round_pad_geometry(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    pad: &mut EditorPad,
    centre_id: SketchEntityId,
) {
    let diameter = pad.size_mm.0;
    let radius = diameter / 2.0;
    sketch.entities.push(Entity::new(
        SketchEntityId::new(),
        plane_id,
        EntityKind::Circle {
            center: centre_id,
            radius,
        },
    ));
    bind_shape_param(sketch, pad, "diameter", centre_id, diameter);
}

/// v0.24 Track A — mint a RoundRect pad's parametric geometry.
/// Returns the four bbox corner IDs in `[ne, se, sw, nw]` order.
pub(super) fn mint_round_rect_pad_geometry(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    pad: &mut EditorPad,
    centre_id: SketchEntityId,
    radius_ratio: f64,
) -> [SketchEntityId; 4] {
    let bbox = pad.bbox_mm();
    let (xmin, ymin, xmax, ymax) = bbox;
    let (w, h) = pad.size_mm;
    let r = (radius_ratio.max(0.0) * w.min(h)).min(w.min(h) / 2.0);

    if r <= f64::EPSILON {
        tracing::warn!(
            target: "signex::v024",
            "RoundRect pad has zero / negative corner radius (ratio = {radius_ratio}); falling \
             back to bbox 4-Line outline"
        );
        return mint_pad_corner_outline(sketch, plane_id, pad);
    }

    // ── 1. bbox corner Points (NE, SE, SW, NW).
    let bbox_corners = bbox_corner_points(sketch, plane_id, pad);

    // ── 2. 8 arc-anchor Points (per corner: edge-anchor + edge-anchor).
    let anchor_positions: [(f64, f64); 8] = [
        (xmax - r, ymin), // 0: NE top-edge anchor
        (xmax, ymin + r), // 1: NE right-edge anchor
        (xmax, ymax - r), // 2: SE right-edge anchor
        (xmax - r, ymax), // 3: SE bottom-edge anchor
        (xmin + r, ymax), // 4: SW bottom-edge anchor
        (xmin, ymax - r), // 5: SW left-edge anchor
        (xmin, ymin + r), // 6: NW left-edge anchor
        (xmin + r, ymin), // 7: NW top-edge anchor
    ];
    let anchor_ids: [SketchEntityId; 8] = std::array::from_fn(|i| {
        push_point(
            sketch,
            plane_id,
            anchor_positions[i].0,
            anchor_positions[i].1,
        )
    });

    // ── 3. 4 inset corner Points (arc centres).
    let inset_positions: [(f64, f64); 4] = [
        (xmax - r, ymin + r), // NE arc centre
        (xmax - r, ymax - r), // SE arc centre
        (xmin + r, ymax - r), // SW arc centre
        (xmin + r, ymin + r), // NW arc centre
    ];
    let inset_ids: [SketchEntityId; 4] = std::array::from_fn(|i| {
        push_point(sketch, plane_id, inset_positions[i].0, inset_positions[i].1)
    });

    // ── 4. 4 shorter Lines connecting adjacent anchors.
    for (start, end) in [
        (anchor_ids[7], anchor_ids[0]),
        (anchor_ids[1], anchor_ids[2]),
        (anchor_ids[3], anchor_ids[4]),
        (anchor_ids[5], anchor_ids[6]),
    ] {
        push_line(sketch, plane_id, start, end);
    }

    // ── 5. 4 corner Arcs. Record per-corner Arc IDs on
    //    `pad.shape_params` via sidecar keys so the Unlink action can
    //    reverse-lookup which corner an Arc represents.
    let arc_keys: [&str; 4] = [
        "corner_r_ne_arc",
        "corner_r_se_arc",
        "corner_r_sw_arc",
        "corner_r_nw_arc",
    ];
    let arc_specs: [(usize, SketchEntityId, SketchEntityId); 4] = [
        (0, anchor_ids[0], anchor_ids[1]),
        (1, anchor_ids[2], anchor_ids[3]),
        (2, anchor_ids[4], anchor_ids[5]),
        (3, anchor_ids[6], anchor_ids[7]),
    ];
    for (corner_idx, start, end) in arc_specs {
        let arc_id = push_arc_ccw(sketch, plane_id, inset_ids[corner_idx], start, end);
        pad.shape_params
            .insert(arc_keys[corner_idx].into(), arc_id.0.simple().to_string());
    }

    // ── 6. Shared corner_r parameter.
    bind_shape_param(sketch, pad, "corner_r", centre_id, r);

    bbox_corners
}

/// v0.24 Track A5 — mint an Oval pad's parametric geometry. An Oval is
/// a stadium / discorectangle.
pub(super) fn mint_oval_pad_geometry(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    pad: &mut EditorPad,
    centre_id: SketchEntityId,
) -> [SketchEntityId; 4] {
    let bbox = pad.bbox_mm();
    let (xmin, ymin, xmax, ymax) = bbox;
    let (w, h) = pad.size_mm;

    // Degenerate case: W ≈ H means the oval is a circle.
    if (w - h).abs() <= f64::EPSILON {
        tracing::warn!(
            target: "signex::v024",
            "Oval pad has equal long+short axes (W={w}, H={h}); falling back to bbox 4-Line \
             outline. Switch to Round shape for circular pads."
        );
        return mint_pad_corner_outline(sketch, plane_id, pad);
    }

    let long_axis = w.max(h);
    let short_axis = w.min(h);
    let inset = (long_axis - short_axis) / 2.0;

    // ── 1. bbox corner Points.
    let bbox_corners = bbox_corner_points(sketch, plane_id, pad);

    // ── 2. 4 arc-anchor Points + 2 Arc-centre Points.
    let wide = w >= h;
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
    let anchor_ids: [SketchEntityId; 4] = std::array::from_fn(|i| {
        push_point(
            sketch,
            plane_id,
            anchor_positions[i].0,
            anchor_positions[i].1,
        )
    });

    let arc_centres: [(f64, f64); 2] = if wide {
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
    let arc_centre_ids: [SketchEntityId; 2] =
        std::array::from_fn(|i| push_point(sketch, plane_id, arc_centres[i].0, arc_centres[i].1));

    // ── 4. 2 Lines on the long-axis edges.
    let line_ids: [SketchEntityId; 2] = [
        push_line(sketch, plane_id, anchor_ids[0], anchor_ids[1]),
        push_line(sketch, plane_id, anchor_ids[2], anchor_ids[3]),
    ];

    // ── 5. 2 Arcs on the short-axis ends (CCW sweep).
    let arc_ids: [SketchEntityId; 2] = [
        push_arc_ccw(
            sketch,
            plane_id,
            arc_centre_ids[1],
            anchor_ids[1],
            anchor_ids[2],
        ),
        push_arc_ccw(
            sketch,
            plane_id,
            arc_centre_ids[0],
            anchor_ids[3],
            anchor_ids[0],
        ),
    ];

    // ── 6. width / height parameters.
    bind_shape_param(sketch, pad, "width", centre_id, long_axis);
    bind_shape_param(sketch, pad, "height", centre_id, short_axis);

    // ── 7. Sidecar bindings for the delete-sweep seed list.
    for (idx, anchor_id) in anchor_ids.iter().enumerate() {
        pad.shape_params.insert(
            format!("oval_anchor_{idx}"),
            anchor_id.0.simple().to_string(),
        );
    }
    for (idx, centre) in arc_centre_ids.iter().enumerate() {
        pad.shape_params
            .insert(format!("oval_centre_{idx}"), centre.0.simple().to_string());
    }
    for (idx, line_id) in line_ids.iter().enumerate() {
        pad.shape_params
            .insert(format!("oval_line_{idx}"), line_id.0.simple().to_string());
    }
    for (idx, arc_id) in arc_ids.iter().enumerate() {
        pad.shape_params
            .insert(format!("oval_arc_{idx}"), arc_id.0.simple().to_string());
    }

    bbox_corners
}

/// v0.24 Track A6 — mint a Chamfered pad's parametric geometry.
pub(super) fn mint_chamfered_pad_geometry(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    pad: &mut EditorPad,
    centre_id: SketchEntityId,
    chamfer_ratio: f64,
    corner_flags: LibChamferedCorners,
) -> [SketchEntityId; 4] {
    let bbox = pad.bbox_mm();
    let (xmin, ymin, xmax, ymax) = bbox;
    let (w, h) = pad.size_mm;
    let r = (chamfer_ratio.max(0.0) * w.min(h)).min(w.min(h) / 2.0);

    let any_enabled = corner_flags.top_left
        || corner_flags.top_right
        || corner_flags.bottom_left
        || corner_flags.bottom_right;
    if !any_enabled {
        tracing::warn!(
            target: "signex::v024",
            "Chamfered pad has no enabled corners; falling back to bbox 4-Line outline"
        );
        return mint_pad_corner_outline(sketch, plane_id, pad);
    }
    if r <= f64::EPSILON {
        tracing::warn!(
            target: "signex::v024",
            "Chamfered pad has zero / negative chamfer length (ratio = {chamfer_ratio}); \
             falling back to bbox 4-Line outline"
        );
        return mint_pad_corner_outline(sketch, plane_id, pad);
    }

    // ── 1. bbox corner Points (NE, SE, SW, NW).
    let bbox_corners = bbox_corner_points(sketch, plane_id, pad);

    // ── 2. Per-corner anchor Points (only for ENABLED corners).
    let corner_specs: [(usize, bool, &str, &str, (f64, f64), (f64, f64)); 4] = [
        (
            0,
            corner_flags.top_right,
            "chamfer_ne_anchor1",
            "chamfer_ne_anchor2",
            (xmax - r, ymin),
            (xmax, ymin + r),
        ),
        (
            1,
            corner_flags.bottom_right,
            "chamfer_se_anchor1",
            "chamfer_se_anchor2",
            (xmax, ymax - r),
            (xmax - r, ymax),
        ),
        (
            2,
            corner_flags.bottom_left,
            "chamfer_sw_anchor1",
            "chamfer_sw_anchor2",
            (xmin + r, ymax),
            (xmin, ymax - r),
        ),
        (
            3,
            corner_flags.top_left,
            "chamfer_nw_anchor1",
            "chamfer_nw_anchor2",
            (xmin, ymin + r),
            (xmin + r, ymin),
        ),
    ];

    let mut anchors: [Option<(SketchEntityId, SketchEntityId)>; 4] = [None, None, None, None];
    for (corner_idx, enabled, key1, key2, pos1, pos2) in corner_specs {
        if !enabled {
            continue;
        }
        let a1_id = push_point(sketch, plane_id, pos1.0, pos1.1);
        let a2_id = push_point(sketch, plane_id, pos2.0, pos2.1);
        anchors[corner_idx] = Some((a1_id, a2_id));
        pad.shape_params
            .insert(key1.into(), a1_id.0.simple().to_string());
        pad.shape_params
            .insert(key2.into(), a2_id.0.simple().to_string());
    }

    // ── 3. Outline traversal (CCW).
    for i in 0..4 {
        let next = (i + 1) % 4;
        if let Some((a1, a2)) = anchors[i] {
            push_line(sketch, plane_id, a1, a2);
        }
        let edge_start = match anchors[i] {
            Some((_, a2)) => a2,
            None => bbox_corners[i],
        };
        let edge_end = match anchors[next] {
            Some((a1, _)) => a1,
            None => bbox_corners[next],
        };
        push_line(sketch, plane_id, edge_start, edge_end);
    }

    // ── 4. Shared chamfer_len parameter.
    bind_shape_param(sketch, pad, "chamfer_len", centre_id, r);

    bbox_corners
}
