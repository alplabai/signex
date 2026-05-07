//! Mint sketch entities for literal pads — first foundational step
//! toward bidirectional sketch ↔ pads sync.
//!
//! When the user enters Sketch mode for a footprint that has literal
//! pads (created in Pads mode) but no sketch entities yet, this
//! module auto-creates a `Point` + `PadAttr` for every pad. The
//! resulting sketch bakes back into the same pad set, so the
//! round-trip is identity-preserving.
//!
//! Future work (v0.15 bidirectional sync):
//! - Pads-mode edits (move / resize / delete) mirror into the
//!   backing sketch entity.
//! - Drag a sketch Point in Sketch mode → pad position updates.
//! - Editing a pad's `PadAttr` from the Properties panel updates
//!   the matching sketch entity.

use signex_library::primitive::footprint::{
    ChamferedCorners as LibChamferedCorners, Footprint, PadKind as LibPadKind,
    PadShape as LibPadShape,
};
use signex_sketch::attr::{
    ChamferedCorners as SkChamferedCorners, CustomPadShape, PadAttr, PadKind as SkPadKind,
    PadShape as SkPadShape, PadSide, PasteAperturePattern,
};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;

use super::state::EditorPad;

/// When the user transitions into Sketch mode for the first time on
/// a footprint that has literal pads but an empty sketch, mint a
/// `Point` + `PadAttr` for each pad. Writes the minted sketch entity
/// IDs back into each `EditorPad.sketch_entity_id` so subsequent
/// Pads-mode edits can mirror through the link. Returns the number
/// of entities minted (zero if the sketch already had content or no
/// literal pads existed).
///
/// The minted sketch produces the same pad set when re-baked through
/// `signex_bake::bake_pads`, so the bake immediately after this call
/// re-emits the original pads — no visual change for the user, but
/// every pad now has a sketch backing they can edit.
pub fn auto_mint_for_literal_pads(pads: &mut [EditorPad], footprint: &mut Footprint) -> usize {
    if pads.is_empty() {
        return 0;
    }
    // Skip if the sketch already has any non-construction entities —
    // assume the user has already started authoring sketch content.
    if let Some(sketch) = footprint.sketch.as_ref() {
        let has_real_entity = sketch.entities.iter().any(|e| !e.construction);
        if has_real_entity {
            return 0;
        }
    }

    let plane_id = ensure_board_top_plane(footprint);
    let sketch = footprint.sketch.get_or_insert_with(SketchData::default);

    let mut minted = 0usize;
    for pad in pads.iter_mut() {
        let entity_id = SketchEntityId::new();
        let mut entity = Entity::new(
            entity_id,
            plane_id,
            EntityKind::Point {
                x: pad.position_mm.0,
                y: pad.position_mm.1,
            },
        );
        entity.pad = Some(pad_attr_from_editor_pad(pad));
        sketch.entities.push(entity);
        // v0.15 — link the editor pad to its backing sketch entity.
        pad.sketch_entity_id = Some(entity_id);
        // v0.16 — also mint 4 outline-corner Points + 4 Lines as
        // construction so the user sees the pad outline as
        // primitives in Sketch mode. `bake_pads` ignores construction
        // entities so this stays purely visual.
        let corners = mint_pad_corner_outline(sketch, plane_id, pad);
        pad.corner_entity_ids = Some(corners);
        minted += 1;
    }
    minted
}

/// v0.15 — when a pad is added in Pads mode (canvas click, etc.),
/// mirror the new pad into the sketch as a `Point` + `PadAttr`.
/// Stores the minted sketch entity ID back on the editor pad so
/// later moves / deletes can mirror through.
///
/// v0.24 Track A — branches on `pad.shape`:
///   - `Round`: mints 1 Circle + a `diameter_<slug>` parameter.
///   - `RoundRect`: mints 4 bbox corner Points + 8 anchor Points +
///     4 inset corner Points (arc centres) + 4 shorter Lines + 4
///     corner Arcs. All four arcs share a single `corner_r_<slug>`
///     parameter so editing it moves every corner in lockstep
///     (Fusion-parity).
///   - `Oval`: mints 4 bbox corner Points + 4 arc-anchor Points +
///     2 Arc-centre Points + 2 Lines on the long-axis edges + 2
///     half-Arcs on the short-axis ends. Two parameters
///     (`width_<slug>`, `height_<slug>`) record the long-axis +
///     short-axis literals; the Properties panel surfaces both as
///     editable rows. A degenerate W ≈ H falls through to the Round
///     path (the oval IS a circle).
///   - Other shapes: existing v0.16 4-Line bbox outline.
///
/// In every case `pad.shape_params` records the parameter name so
/// the Phase 3 Properties row can find the bound parameter.
pub fn mirror_add_pad_to_sketch(pad: &mut EditorPad, footprint: &mut Footprint) {
    // No-op when the sketch already has a backing entity for this
    // pad (e.g. caller already wired it up).
    if pad.sketch_entity_id.is_some() {
        return;
    }
    let plane_id = ensure_board_top_plane(footprint);
    let sketch = footprint.sketch.get_or_insert_with(SketchData::default);
    let entity_id = SketchEntityId::new();
    let mut entity = Entity::new(
        entity_id,
        plane_id,
        EntityKind::Point {
            x: pad.position_mm.0,
            y: pad.position_mm.1,
        },
    );
    entity.pad = Some(pad_attr_from_editor_pad(pad));
    sketch.entities.push(entity);
    pad.sketch_entity_id = Some(entity_id);

    // v0.24 Track A — branch on pad shape.
    match &pad.shape {
        LibPadShape::Round => {
            mint_round_pad_geometry(sketch, plane_id, pad, entity_id);
            // Round pads have no rectangular outline — leave
            // corner_entity_ids unset so move/delete mirrors skip
            // bbox-corner repositioning.
            pad.corner_entity_ids = None;
        }
        LibPadShape::RoundRect { radius_ratio } => {
            let corners =
                mint_round_rect_pad_geometry(sketch, plane_id, pad, entity_id, *radius_ratio);
            pad.corner_entity_ids = Some(corners);
        }
        LibPadShape::Oval => {
            let corners = mint_oval_pad_geometry(sketch, plane_id, pad, entity_id);
            pad.corner_entity_ids = Some(corners);
        }
        LibPadShape::Chamfered {
            chamfer_ratio,
            corners,
        } => {
            let bbox_corners = mint_chamfered_pad_geometry(
                sketch,
                plane_id,
                pad,
                entity_id,
                *chamfer_ratio,
                *corners,
            );
            pad.corner_entity_ids = Some(bbox_corners);
        }
        _ => {
            // v0.16 — outline-corner Points + Lines, construction-only.
            let corners = mint_pad_corner_outline(sketch, plane_id, pad);
            pad.corner_entity_ids = Some(corners);
        }
    }
}

/// v0.15 — when a pad moves in Pads mode (drag), update its backing
/// sketch `Point`'s coordinates so the sketch stays in sync. No-op
/// when the pad has no backing sketch entity yet.
pub fn mirror_move_pad_in_sketch(pad: &EditorPad, footprint: &mut Footprint) {
    let Some(entity_id) = pad.sketch_entity_id else {
        return;
    };
    let Some(sketch) = footprint.sketch.as_mut() else {
        return;
    };
    if let Some(entity) = sketch.entities.iter_mut().find(|e| e.id == entity_id) {
        if let EntityKind::Point { x, y } = &mut entity.kind {
            *x = pad.position_mm.0;
            *y = pad.position_mm.1;
        }
    }
    // v0.16 — also reposition the outline-corner Points so the
    // construction outline tracks the pad bbox.
    if let Some(corners) = pad.corner_entity_ids {
        let bbox = pad.bbox_mm();
        let positions: [(f64, f64); 4] = [
            (bbox.2, bbox.1), // ne
            (bbox.2, bbox.3), // se
            (bbox.0, bbox.3), // sw
            (bbox.0, bbox.1), // nw
        ];
        for (id, (px, py)) in corners.iter().zip(positions.iter()) {
            if let Some(entity) = sketch.entities.iter_mut().find(|e| e.id == *id) {
                if let EntityKind::Point { x, y } = &mut entity.kind {
                    *x = *px;
                    *y = *py;
                }
            }
        }
    }
}

/// v0.15 — when a pad is deleted in Pads mode, also drop its
/// backing sketch entity (and any constraints that referenced it).
/// No-op when the pad has no backing sketch entity yet.
///
/// v0.24 Track A — also drop linked Circle / Arc entities and any
/// sketch parameters keyed by the centre-Point UUID slug
/// (`diameter_<slug>`, `corner_r_<slug>`, `chamfer_len_<slug>`,
/// etc.). RoundRect's anchor / inset-corner Points are pulled into
/// the drop set via a secondary sweep — they're referenced
/// indirectly by Arcs whose `center` is the inset corner; once those
/// Arcs and their adjacent Lines are dropped, the orphan Points get
/// cleaned up too.
///
/// v0.24 Track A6 — Chamfered pads add per-corner anchor Points
/// keyed under `chamfer_<corner>_anchor1` / `..._anchor2` on
/// `pad.shape_params`. The secondary sweep already picks up
/// anchor Points via the Lines that connect them to the bbox
/// corners (whose IDs are in `corner_entity_ids` and therefore in
/// the initial drop set). The chamfer-cut Lines (anchor → anchor)
/// are then caught when their endpoints get added to `drop_set` by
/// the secondary sweep. Anchor IDs themselves are pulled out of
/// `pad.shape_params` for completeness so the drop set covers them
/// even on degenerate sketches where the connecting Lines went
/// missing.
pub fn mirror_delete_pad_from_sketch(pad: &EditorPad, footprint: &mut Footprint) {
    let Some(entity_id) = pad.sketch_entity_id else {
        return;
    };
    let Some(sketch) = footprint.sketch.as_mut() else {
        return;
    };
    // v0.16 — collect the corner-outline entity IDs so we can drop
    // the construction Points + the Lines connecting them. Lines
    // reference the corner Points by ID; we drop any Line whose
    // start or end is one of the dropped corner IDs.
    let mut to_drop: Vec<SketchEntityId> = vec![entity_id];
    if let Some(corners) = pad.corner_entity_ids {
        to_drop.extend_from_slice(&corners);
    }
    // v0.24 Track A5 + A6 — also seed any sidecar entity IDs stored
    // on `pad.shape_params`. RoundRect stores per-corner Arc IDs
    // under `corner_r_{ne,se,sw,nw}_arc`; Oval stores its anchor /
    // arc-centre / Line / Arc IDs under
    // `oval_{anchor,centre,line,arc}_*`; Chamfered stores anchors
    // under `chamfer_<corner>_anchor1` / `..._anchor2`. All sidecar
    // values are UUID slugs (no dashes); `Uuid::parse_str` accepts
    // the slug form so we filter shape_params values that parse
    // cleanly. Canonical binding values like `corner_r_<slug>` /
    // `width_<slug>` / `height_<slug>` / `chamfer_len_<slug>` /
    // `diameter_<slug>` are not pure UUIDs (they have a leading
    // identifier prefix) so they fall through.
    for value in pad.shape_params.values() {
        if let Ok(uuid) = uuid::Uuid::parse_str(value) {
            to_drop.push(SketchEntityId(uuid));
        }
    }
    let mut drop_set: std::collections::HashSet<SketchEntityId> = to_drop.iter().copied().collect();

    // v0.24 Track A — secondary sweep. RoundRect's 8 anchor Points
    // + 4 inset corner Points are referenced only by the 4 corner
    // Arcs and the 4 shorter Lines. Walk the entity list once to
    // collect every Line / Arc / Circle that touches a dropped ID,
    // and pull their referenced Points into the drop set so the
    // sweep fully cleans up the graveyard. One pass is enough
    // because anchor/inset Points are leaves in the reference
    // graph (no Arc/Line references another Arc/Line).
    let mut secondary_drops: std::collections::HashSet<SketchEntityId> =
        std::collections::HashSet::new();
    for entity in &sketch.entities {
        if drop_set.contains(&entity.id) {
            continue;
        }
        match &entity.kind {
            EntityKind::Line { start, end } => {
                if drop_set.contains(start) || drop_set.contains(end) {
                    secondary_drops.insert(entity.id);
                    secondary_drops.insert(*start);
                    secondary_drops.insert(*end);
                }
            }
            EntityKind::Arc {
                center, start, end, ..
            } => {
                if drop_set.contains(center) || drop_set.contains(start) || drop_set.contains(end)
                {
                    secondary_drops.insert(entity.id);
                    secondary_drops.insert(*center);
                    secondary_drops.insert(*start);
                    secondary_drops.insert(*end);
                }
            }
            EntityKind::Circle { center, .. } => {
                if drop_set.contains(center) {
                    secondary_drops.insert(entity.id);
                }
            }
            EntityKind::Point { .. } => {}
        }
    }
    drop_set.extend(secondary_drops);

    sketch.entities.retain(|e| {
        if drop_set.contains(&e.id) {
            return false;
        }
        match &e.kind {
            EntityKind::Line { start, end } => {
                if drop_set.contains(start) || drop_set.contains(end) {
                    return false;
                }
            }
            EntityKind::Arc {
                center, start, end, ..
            } => {
                if drop_set.contains(center) || drop_set.contains(start) || drop_set.contains(end)
                {
                    return false;
                }
            }
            EntityKind::Circle { center, .. } => {
                if drop_set.contains(center) {
                    return false;
                }
            }
            EntityKind::Point { .. } => {}
        }
        true
    });
    // Drop dangling constraint refs — coarse rule via Debug
    // stringification (mirrors the SketchEdit::DeleteEntity path in
    // sketch_dispatch.rs).
    let id_str = entity_id.to_string();
    sketch
        .constraints
        .retain(|c| !format!("{:?}", c.kind).contains(&id_str));

    // v0.24 Track A — drop shape parameters (`diameter_<slug>`,
    // `corner_r_<slug>`, etc.) keyed by the centre-Point UUID slug.
    let slug = id_slug(entity_id);
    sketch.parameters.0.retain(|name, _| !name.ends_with(&slug));
}

/// v0.16 — mint 4 corner Points + 4 Lines outlining a pad's bbox.
/// Returns the corner IDs in `[ne, se, sw, nw]` order so the caller
/// can store them on `EditorPad.corner_entity_ids` and reposition
/// them on later pad moves. Both the corner Points and the Lines
/// connecting them are flagged `construction = true` so
/// `signex_bake::bake_pads` skips them and they don't double up the
/// rendered pad geometry.
fn mint_pad_corner_outline(
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
    let ids: [SketchEntityId; 4] = [
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
    ];
    for (id, (x, y)) in ids.iter().zip(positions.iter()) {
        let mut e = Entity::new(*id, plane_id, EntityKind::Point { x: *x, y: *y });
        e.construction = true;
        sketch.entities.push(e);
    }
    // 4 Lines around the loop — N (ne→nw), W (nw→sw), S (sw→se),
    // E (se→ne). Construction-only.
    for (a, b) in [
        (ids[0], ids[3]),
        (ids[3], ids[2]),
        (ids[2], ids[1]),
        (ids[1], ids[0]),
    ] {
        let mut line = Entity::new(
            SketchEntityId::new(),
            plane_id,
            EntityKind::Line { start: a, end: b },
        );
        line.construction = true;
        sketch.entities.push(line);
    }
    ids
}

/// v0.24 Track A — mint a Round pad's geometry: 1 Circle entity
/// referencing the centre `Point` (the pad's `sketch_entity_id`) +
/// a `diameter_<slug>` sketch parameter recording the literal
/// diameter for later parametric edits. The Properties row (A2)
/// reads this parameter via `pad.shape_params["diameter"]`.
fn mint_round_pad_geometry(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    pad: &mut EditorPad,
    centre_id: SketchEntityId,
) {
    // Round pad's diameter equals its W (and H — it's a circle, so
    // size_mm.0 == size_mm.1 by definition). The Circle entity stores
    // the radius literal so the bake produces correct geometry; the
    // parameter records the diameter for the Properties-row link.
    let diameter = pad.size_mm.0;
    let radius = diameter / 2.0;
    let circle = Entity::new(
        SketchEntityId::new(),
        plane_id,
        EntityKind::Circle {
            center: centre_id,
            radius,
        },
    );
    sketch.entities.push(circle);

    let slug = id_slug(centre_id);
    let param_name = format!("diameter_{slug}");
    sketch
        .parameters
        .insert(param_name.clone(), format!("{}mm", format_f64(diameter)));
    pad.shape_params.insert("diameter".into(), param_name);
}

/// v0.24 Track A — mint a RoundRect pad's parametric geometry:
///   - 4 bbox corner Points (returned for `corner_entity_ids` so
///     move-mirror keeps the bbox tracking the pad).
///   - 8 arc-anchor Points where each corner arc tangents touch the
///     two adjacent edge lines (inset distance =
///     `radius_ratio * min(W, H)`).
///   - 4 inset corner Points (arc centres).
///   - 4 shorter Lines connecting anchor → anchor (replacing the
///     v0.16 corner-to-corner Lines).
///   - 4 Arc entities; all four read radius from a single
///     `corner_r_<slug>` sketch parameter so they stay linked
///     implicitly. Phase 3 will attach the
///     [`signex_sketch::LinkedRadius::Shared`] enum value to encode
///     the link explicitly when A2 (Properties row) and A3 (Unlink)
///     ship.
///
/// All entities are non-construction — they're the canonical pad
/// geometry now (not a derived outline overlay). The bake reads
/// `pad.shape` directly in v0.24 phase 2; this geometry is purely
/// for editing UX until A4 (reverse mirror) lands.
fn mint_round_rect_pad_geometry(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    pad: &mut EditorPad,
    centre_id: SketchEntityId,
    radius_ratio: f64,
) -> [SketchEntityId; 4] {
    let bbox = pad.bbox_mm();
    let (xmin, ymin, xmax, ymax) = bbox;
    let (w, h) = pad.size_mm;
    // Inset distance = radius_ratio * min(W, H). Clamp to the bbox
    // half-extent so a pathological radius_ratio (>0.5) cannot push
    // anchors past each other.
    let r = (radius_ratio.max(0.0) * w.min(h)).min(w.min(h) / 2.0);

    if r <= f64::EPSILON {
        tracing::warn!(
            target: "signex::v024",
            "RoundRect pad has zero / negative corner radius (ratio = {radius_ratio}); falling \
             back to bbox 4-Line outline"
        );
        return mint_pad_corner_outline(sketch, plane_id, pad);
    }

    // ── 1. bbox corner Points (NE, SE, SW, NW). The same `[ne, se,
    //    sw, nw]` order used everywhere in pad_to_sketch.rs.
    let bbox_corner_positions: [(f64, f64); 4] = [
        (xmax, ymin), // ne
        (xmax, ymax), // se
        (xmin, ymax), // sw
        (xmin, ymin), // nw
    ];
    let bbox_corners: [SketchEntityId; 4] = std::array::from_fn(|_| SketchEntityId::new());
    for (id, (x, y)) in bbox_corners.iter().zip(bbox_corner_positions.iter()) {
        let entity = Entity::new(*id, plane_id, EntityKind::Point { x: *x, y: *y });
        sketch.entities.push(entity);
    }

    // ── 2. 8 arc-anchor Points (per corner: edge-anchor + edge-anchor).
    //    Order paired by corner: NE_top, NE_right, SE_right, SE_bottom,
    //    SW_bottom, SW_left, NW_left, NW_top.
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
    let anchor_ids: [SketchEntityId; 8] = std::array::from_fn(|_| SketchEntityId::new());
    for (id, (x, y)) in anchor_ids.iter().zip(anchor_positions.iter()) {
        let entity = Entity::new(*id, plane_id, EntityKind::Point { x: *x, y: *y });
        sketch.entities.push(entity);
    }

    // ── 3. 4 inset corner Points (arc centres).
    let inset_positions: [(f64, f64); 4] = [
        (xmax - r, ymin + r), // NE arc centre
        (xmax - r, ymax - r), // SE arc centre
        (xmin + r, ymax - r), // SW arc centre
        (xmin + r, ymin + r), // NW arc centre
    ];
    let inset_ids: [SketchEntityId; 4] = std::array::from_fn(|_| SketchEntityId::new());
    for (id, (x, y)) in inset_ids.iter().zip(inset_positions.iter()) {
        let entity = Entity::new(*id, plane_id, EntityKind::Point { x: *x, y: *y });
        sketch.entities.push(entity);
    }

    // ── 4. 4 shorter Lines connecting adjacent anchors.
    //   Top:    NW_top   → NE_top    (anchor[7] → anchor[0])
    //   Right:  NE_right → SE_right  (anchor[1] → anchor[2])
    //   Bottom: SE_bot   → SW_bot    (anchor[3] → anchor[4])
    //   Left:   SW_left  → NW_left   (anchor[5] → anchor[6])
    for (start, end) in [
        (anchor_ids[7], anchor_ids[0]),
        (anchor_ids[1], anchor_ids[2]),
        (anchor_ids[3], anchor_ids[4]),
        (anchor_ids[5], anchor_ids[6]),
    ] {
        let line = Entity::new(
            SketchEntityId::new(),
            plane_id,
            EntityKind::Line { start, end },
        );
        sketch.entities.push(line);
    }

    // ── 5. 4 corner Arcs.
    //   NE: start = NE_top   (anchor[0]), end = NE_right (anchor[1])
    //   SE: start = SE_right (anchor[2]), end = SE_bot   (anchor[3])
    //   SW: start = SW_bot   (anchor[4]), end = SW_left  (anchor[5])
    //   NW: start = NW_left  (anchor[6]), end = NW_top   (anchor[7])
    //
    // v0.24 Phase 3 (Track A3) — also record per-corner Arc IDs on
    // `pad.shape_params` via sidecar keys (`corner_r_ne_arc` ..
    // `corner_r_nw_arc`). The Unlink action looks up which corner an
    // Arc represents by reverse-lookup against this map; without the
    // sidecar we'd have to infer corner from arc-centre position vs
    // pad bbox centre, which gets brittle when the pad is rotated or
    // an array instance applies a flip.
    let arc_keys: [&str; 4] = ["corner_r_ne_arc", "corner_r_se_arc",
                                "corner_r_sw_arc", "corner_r_nw_arc"];
    for (corner_idx, (centre_idx, start, end)) in [
        (0usize, anchor_ids[0], anchor_ids[1]),
        (1, anchor_ids[2], anchor_ids[3]),
        (2, anchor_ids[4], anchor_ids[5]),
        (3, anchor_ids[6], anchor_ids[7]),
    ]
    .into_iter()
    .enumerate()
    {
        let arc_id = SketchEntityId::new();
        let arc = Entity::new(
            arc_id,
            plane_id,
            EntityKind::Arc {
                center: inset_ids[centre_idx],
                start,
                end,
                sweep_ccw: true,
            },
        );
        sketch.entities.push(arc);
        pad.shape_params
            .insert(arc_keys[corner_idx].into(), arc_id.0.simple().to_string());
    }

    // ── 6. Shared corner_r parameter. All four arcs read radius
    //    implicitly from this parameter at bake time (Phase 3 ties
    //    the link explicitly via LinkedRadius::Shared). The literal
    //    radius is stored as the parameter expression so a fresh
    //    sketch round-trips identity.
    let slug = id_slug(centre_id);
    let param_name = format!("corner_r_{slug}");
    sketch
        .parameters
        .insert(param_name.clone(), format!("{}mm", format_f64(r)));
    pad.shape_params.insert("corner_r".into(), param_name);

    bbox_corners
}

/// v0.24 Track A5 — mint an Oval pad's parametric geometry. An Oval
/// is a stadium / discorectangle: a rectangle whose two short-axis
/// ends are replaced by half-circles that span the entire short
/// dimension. Layout follows Fusion 360's stadium primitive.
///
/// Mints (in addition to the centre Point already pushed by the
/// caller via `entity_id`):
///   - 4 bbox corner Points (returned for `corner_entity_ids` so
///     move-mirror keeps the bbox tracking the pad).
///   - 4 arc-anchor Points where each rounded end meets the straight
///     edge.
///   - 2 Arc-centre Points (midpoints of the two short-axis edges,
///     offset inward by half the short axis).
///   - 2 straight Lines on the long-axis edges connecting consecutive
///     anchors.
///   - 2 Arcs on the short-axis ends, each spanning 180°. Radius =
///     half the short axis = half the smaller pad dimension.
///
/// Records two parameters in `sketch.parameters`:
///   - `width_<slug>`  → the long-axis dimension (max(W, H)).
///   - `height_<slug>` → the short-axis dimension (min(W, H)).
///
/// Both arcs read radius = `height / 2` (or `width / 2` for tall
/// ovals) implicitly via the parameter table; the literal radius is
/// stored at mint time so a fresh sketch round-trips identity. The
/// `pad.shape_params` map binds the canonical feature keys
/// (`"width"`, `"height"`) so the Properties panel surfaces both as
/// editable rows independently of orientation — the visible labels
/// stay W=long / H=short for clarity.
///
/// W ≈ H is degenerate (the oval IS a circle); the caller should
/// route to `mint_round_pad_geometry` instead. We log a warning and
/// fall back to a bbox 4-Line outline so the user still sees the pad
/// outlined, but the parametric handles are skipped — re-entering
/// Pads mode with the user changing W to differ from H repaints the
/// proper oval geometry on next mirror.
fn mint_oval_pad_geometry(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    pad: &mut EditorPad,
    centre_id: SketchEntityId,
) -> [SketchEntityId; 4] {
    let bbox = pad.bbox_mm();
    let (xmin, ymin, xmax, ymax) = bbox;
    let (w, h) = pad.size_mm;

    // Degenerate case: W ≈ H means the oval is a circle. Per the
    // task spec, log a warning and fall back to a bbox 4-Line outline
    // so move/delete mirrors stay consistent with Rect / RoundRect.
    // The user can route through Round geometry by switching the
    // pad's shape to `Round` once they realise the dims are equal;
    // we don't auto-convert because the data round-trip would surface
    // a different shape on save.
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
    let arc_radius = short_axis / 2.0;
    let inset = (long_axis - short_axis) / 2.0; // == long_axis/2 - short_axis/2

    // ── 1. bbox corner Points (NE, SE, SW, NW). Same `[ne, se, sw,
    //    nw]` order used everywhere else in pad_to_sketch.rs.
    let bbox_corner_positions: [(f64, f64); 4] = [
        (xmax, ymin), // ne
        (xmax, ymax), // se
        (xmin, ymax), // sw
        (xmin, ymin), // nw
    ];
    let bbox_corners: [SketchEntityId; 4] = std::array::from_fn(|_| SketchEntityId::new());
    for (id, (x, y)) in bbox_corners.iter().zip(bbox_corner_positions.iter()) {
        let entity = Entity::new(*id, plane_id, EntityKind::Point { x: *x, y: *y });
        sketch.entities.push(entity);
    }

    // ── 2. 4 arc-anchor Points + 2 Arc-centre Points + 2 Lines + 2
    //    Arcs. The geometry differs by orientation (wide vs. tall):
    //
    //    Wide oval (W > H): arcs on left + right. Anchors split the
    //    top + bottom edges into the straight-edge run. Lines run
    //    horizontally; arcs sweep around the left + right ends.
    //
    //    Tall oval (H > W): arcs on top + bottom. Anchors split the
    //    left + right edges. Lines run vertically; arcs sweep around
    //    the top + bottom ends.
    //
    //    `inset` is the offset from the bbox short-axis edge to the
    //    arc-anchor along the long axis.
    let wide = w >= h;

    // Anchor positions (4 entries — one pair per "end" of the oval).
    // Pair indexing for clarity:
    //   wide ovals: anchors[0..2] = top edge (top-left, top-right)
    //               anchors[2..4] = bottom edge (bottom-right, bottom-left)
    //   tall ovals: anchors[0..2] = right edge (top-right, bottom-right)
    //               anchors[2..4] = left edge (bottom-left, top-left)
    //
    // The pairing is chosen so each Line connects anchors[0]→[1] and
    // anchors[2]→[3], i.e. consecutive indices. Arcs then close the
    // loop from anchors[1]→[2] and anchors[3]→[0].
    let anchor_positions: [(f64, f64); 4] = if wide {
        [
            (xmin + inset, ymin), // 0: top-left (start of top straight)
            (xmax - inset, ymin), // 1: top-right (end of top straight)
            (xmax - inset, ymax), // 2: bottom-right (start of bottom straight)
            (xmin + inset, ymax), // 3: bottom-left (end of bottom straight)
        ]
    } else {
        [
            (xmax, ymin + inset), // 0: top-right (start of right straight)
            (xmax, ymax - inset), // 1: bottom-right (end of right straight)
            (xmin, ymax - inset), // 2: bottom-left (start of left straight)
            (xmin, ymin + inset), // 3: top-left (end of left straight)
        ]
    };
    let anchor_ids: [SketchEntityId; 4] = std::array::from_fn(|_| SketchEntityId::new());
    for (id, (x, y)) in anchor_ids.iter().zip(anchor_positions.iter()) {
        let entity = Entity::new(*id, plane_id, EntityKind::Point { x: *x, y: *y });
        sketch.entities.push(entity);
    }

    // ── 3. 2 Arc-centre Points. Centre = the midpoint of the short
    //    axis ON each end, offset inward by `inset` so the arc sits
    //    exactly tangent to the straight-edge run. For a wide oval,
    //    centres lie on the y=midline at xmin+inset and xmax-inset;
    //    for a tall oval, on x=midline at ymin+inset and ymax-inset.
    let arc_centres: [(f64, f64); 2] = if wide {
        [
            (xmin + inset, (ymin + ymax) / 2.0), // left end
            (xmax - inset, (ymin + ymax) / 2.0), // right end
        ]
    } else {
        [
            ((xmin + xmax) / 2.0, ymin + inset), // top end
            ((xmin + xmax) / 2.0, ymax - inset), // bottom end
        ]
    };
    let arc_centre_ids: [SketchEntityId; 2] = std::array::from_fn(|_| SketchEntityId::new());
    for (id, (x, y)) in arc_centre_ids.iter().zip(arc_centres.iter()) {
        let entity = Entity::new(*id, plane_id, EntityKind::Point { x: *x, y: *y });
        sketch.entities.push(entity);
    }

    // ── 4. 2 Lines on the long-axis edges connecting consecutive
    //    anchors. Wide → top + bottom horizontal Lines; Tall → right
    //    + left vertical Lines.
    let line_ids: [SketchEntityId; 2] = std::array::from_fn(|_| SketchEntityId::new());
    for (line_id, (start, end)) in line_ids.iter().zip(
        [
            (anchor_ids[0], anchor_ids[1]),
            (anchor_ids[2], anchor_ids[3]),
        ]
        .iter()
        .copied(),
    ) {
        let line = Entity::new(*line_id, plane_id, EntityKind::Line { start, end });
        sketch.entities.push(line);
    }

    // ── 5. 2 Arcs on the short-axis ends. Each spans 180° from the
    //    end of one straight to the start of the other (closing the
    //    loop). Sweep direction matches RoundRect's CCW convention so
    //    the bake's closed-loop walker reads consistent winding.
    //
    //    Wide oval:
    //      - Right arc: centre = arc_centres[1], start = anchors[1]
    //        (top-right), end = anchors[2] (bottom-right).
    //      - Left arc: centre = arc_centres[0], start = anchors[3]
    //        (bottom-left), end = anchors[0] (top-left).
    //
    //    Tall oval:
    //      - Bottom arc: centre = arc_centres[1], start = anchors[1]
    //        (bottom-right), end = anchors[2] (bottom-left).
    //      - Top arc: centre = arc_centres[0], start = anchors[3]
    //        (top-left), end = anchors[0] (top-right).
    let arc_specs: [(SketchEntityId, SketchEntityId, SketchEntityId); 2] = [
        (arc_centre_ids[1], anchor_ids[1], anchor_ids[2]),
        (arc_centre_ids[0], anchor_ids[3], anchor_ids[0]),
    ];
    let arc_ids: [SketchEntityId; 2] = std::array::from_fn(|_| SketchEntityId::new());
    for (arc_id, (centre, start, end)) in arc_ids.iter().zip(arc_specs.iter().copied()) {
        let arc = Entity::new(
            *arc_id,
            plane_id,
            EntityKind::Arc {
                center: centre,
                start,
                end,
                sweep_ccw: true,
            },
        );
        sketch.entities.push(arc);
    }

    // ── 6. Two parameters: width (long axis) + height (short axis).
    //    Both keyed by the centre Point's UUID slug so the delete
    //    sweep cleans them up in one pass with the rest of the pad's
    //    shape parameters.
    let slug = id_slug(centre_id);
    let width_param = format!("width_{slug}");
    let height_param = format!("height_{slug}");
    sketch
        .parameters
        .insert(width_param.clone(), format!("{}mm", format_f64(long_axis)));
    sketch
        .parameters
        .insert(height_param.clone(), format!("{}mm", format_f64(short_axis)));
    pad.shape_params.insert("width".into(), width_param);
    pad.shape_params.insert("height".into(), height_param);

    // ── 7. Sidecar bindings — record the entity IDs of each minted
    //    outline element on `pad.shape_params` so the delete sweep
    //    seeds them into the drop set. Without these, the bbox corner
    //    Points would be the only Oval-related drop seed and the
    //    secondary sweep would miss every anchor / arc-centre / Line
    //    / Arc (none of them reference the bbox corners). Sidecar
    //    keys use the `oval_*_<idx>` prefix so the runtime Properties
    //    filter recognises them and skips rendering. The runtime side
    //    treats `key.starts_with("oval_")` as a sidecar marker; the
    //    canonical user-editable bindings are `"width"` and
    //    `"height"`, both of which lack the prefix.
    for (idx, anchor_id) in anchor_ids.iter().enumerate() {
        pad.shape_params
            .insert(format!("oval_anchor_{idx}"), anchor_id.0.simple().to_string());
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

    // Sanity check on the arc radius equality — both Arcs implicitly
    // share `height_<slug>` (or `width_<slug>` for tall ovals). The
    // mint side encodes this with literal-equal radii at place time;
    // the post-mint geometry exercises this in the regression test.
    let _ = arc_radius;

    bbox_corners
}

/// v0.24 Track A6 — mint a Chamfered pad's parametric geometry.
/// Like RoundRect, the bbox corner Points are minted in the
/// canonical `[ne, se, sw, nw]` order (returned for
/// `corner_entity_ids`). For each ENABLED chamfered corner (per
/// `chamfer_corners.<key>`), two "chamfer-anchor" Points are minted
/// along the two edges adjacent to that bbox corner, each `r`
/// (= chamfer length) away from the bbox corner. Adjacent anchors
/// (and disabled bbox corners) are then connected by Lines so the
/// resulting outline hugs the chamfered shape — disabled corners
/// stay as 90° angles.
///
/// All four enabled corners read their length from a single shared
/// `chamfer_len_<slug>` sketch parameter (mirrors RoundRect's
/// shared `corner_r` pattern). A future "Unlink chamfer length"
/// action (out of scope for A6 MVP) can mint per-corner override
/// parameters; the per-corner anchor sidecar keys
/// (`chamfer_ne_anchor1` / `..._anchor2`) record which Points belong
/// to which corner so the unlink path has the data it needs.
///
/// Initial value of `chamfer_len_<slug>` is
/// `chamfer_ratio * min(W, H)` so existing pads on disk mint with
/// the right visual length.
///
/// Degenerate case (no corners enabled, or chamfer_len ≤ 0): warns
/// and falls through to the v0.16 4-Line bbox outline.
fn mint_chamfered_pad_geometry(
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
    // Chamfer length = chamfer_ratio * min(W, H). Clamp to the bbox
    // half-extent so a pathological chamfer_ratio (>0.5) cannot push
    // anchors past each other on a single edge.
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

    // ── 1. bbox corner Points (NE, SE, SW, NW). Same canonical
    //    order used everywhere else in pad_to_sketch.rs. The
    //    `corner_entity_ids` array maps directly to these so move/
    //    delete mirrors keep tracking the bbox.
    //
    //    `LibChamferedCorners` uses Y-down naming (top_left = NW,
    //    top_right = NE, bottom_left = SW, bottom_right = SE) so we
    //    align corner_flags → (NE/SE/SW/NW) explicitly here.
    let bbox_corner_positions: [(f64, f64); 4] = [
        (xmax, ymin), // ne — top_right (Y-down: top is min-Y)
        (xmax, ymax), // se — bottom_right
        (xmin, ymax), // sw — bottom_left
        (xmin, ymin), // nw — top_left
    ];
    let bbox_corners: [SketchEntityId; 4] = std::array::from_fn(|_| SketchEntityId::new());
    for (id, (x, y)) in bbox_corners.iter().zip(bbox_corner_positions.iter()) {
        let entity = Entity::new(*id, plane_id, EntityKind::Point { x: *x, y: *y });
        sketch.entities.push(entity);
    }

    // ── 2. Per-corner anchor Points (only for ENABLED corners).
    //    For an enabled corner, two anchors sit on the two adjacent
    //    edges, each `r` away from the bbox corner. `anchor1` /
    //    `anchor2` follow CCW outline traversal so the Lines pick
    //    them up cleanly:
    //      NE: anchor1 = top-edge anchor   (xmax - r, ymin)
    //          anchor2 = right-edge anchor (xmax,     ymin + r)
    //      SE: anchor1 = right-edge anchor (xmax,     ymax - r)
    //          anchor2 = bot-edge anchor   (xmax - r, ymax)
    //      SW: anchor1 = bot-edge anchor   (xmin + r, ymax)
    //          anchor2 = left-edge anchor  (xmin,     ymax - r)
    //      NW: anchor1 = left-edge anchor  (xmin,     ymin + r)
    //          anchor2 = top-edge anchor   (xmin + r, ymin)
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

    // anchors[i] = Some((a1, a2)) for enabled corner i, or None.
    // We hold IDs paired with the bbox corner index they belong to
    // so step 3 can stitch the outline correctly.
    let mut anchors: [Option<(SketchEntityId, SketchEntityId)>; 4] = [None, None, None, None];
    for (corner_idx, enabled, key1, key2, pos1, pos2) in corner_specs {
        if !enabled {
            continue;
        }
        let a1_id = SketchEntityId::new();
        let a2_id = SketchEntityId::new();
        sketch.entities.push(Entity::new(
            a1_id,
            plane_id,
            EntityKind::Point { x: pos1.0, y: pos1.1 },
        ));
        sketch.entities.push(Entity::new(
            a2_id,
            plane_id,
            EntityKind::Point { x: pos2.0, y: pos2.1 },
        ));
        anchors[corner_idx] = Some((a1_id, a2_id));
        // Per-corner sidecar keys — record which Points belong to
        // which corner so a future Unlink-chamfer-length action has
        // the data it needs. The Properties summary loop filters any
        // key ending in `_anchor` so these don't render as rows.
        pad.shape_params
            .insert(key1.into(), a1_id.0.simple().to_string());
        pad.shape_params
            .insert(key2.into(), a2_id.0.simple().to_string());
    }

    // ── 3. Outline traversal. Walking CCW: NE → SE → SW → NW → NE.
    //    For each consecutive corner pair (i, i+1):
    //      - The chamfer-cut line at corner i (anchor1 → anchor2)
    //        is added once per enabled corner.
    //      - The edge between corner i and corner i+1 connects
    //        end-of-i (= anchor2 if enabled, else bbox corner) to
    //        start-of-(i+1) (= anchor1 if enabled, else bbox corner).
    //
    //    Yields: each enabled corner contributes 1 chamfer-cut line +
    //    its outgoing edge; each disabled corner contributes only its
    //    outgoing edge (the bbox corner stays as a sharp 90°). Total
    //    Lines = enabled_count + 4.
    for i in 0..4 {
        let next = (i + 1) % 4;
        // Chamfer-cut line for corner i, only when enabled.
        if let Some((a1, a2)) = anchors[i] {
            let line = Entity::new(
                SketchEntityId::new(),
                plane_id,
                EntityKind::Line { start: a1, end: a2 },
            );
            sketch.entities.push(line);
        }
        // Edge connecting the end of corner i to the start of corner
        // i+1.
        let edge_start = match anchors[i] {
            Some((_, a2)) => a2,
            None => bbox_corners[i],
        };
        let edge_end = match anchors[next] {
            Some((a1, _)) => a1,
            None => bbox_corners[next],
        };
        let line = Entity::new(
            SketchEntityId::new(),
            plane_id,
            EntityKind::Line {
                start: edge_start,
                end: edge_end,
            },
        );
        sketch.entities.push(line);
    }

    // ── 4. Shared chamfer_len parameter. All enabled corners share
    //    this single parameter (Fusion-parity). The literal length is
    //    stored as the parameter expression so a fresh sketch
    //    round-trips identity.
    let slug = id_slug(centre_id);
    let param_name = format!("chamfer_len_{slug}");
    sketch
        .parameters
        .insert(param_name.clone(), format!("{}mm", format_f64(r)));
    pad.shape_params.insert("chamfer_len".into(), param_name);

    bbox_corners
}

/// v0.24 Track A — UUID slug for parameter-name namespacing. Strips
/// dashes so the resulting parameter name is a valid identifier in
/// the expression language.
fn id_slug(id: SketchEntityId) -> String {
    id.0.simple().to_string()
}

fn ensure_board_top_plane(footprint: &mut Footprint) -> PlaneId {
    let sketch = footprint.sketch.get_or_insert_with(SketchData::default);
    if let Some(p) = sketch
        .planes
        .iter()
        .find(|p| matches!(p.kind, PlaneKind::BoardTop))
    {
        return p.id;
    }
    let p = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    let id = p.id;
    sketch.planes.push(p);
    id
}

fn pad_attr_from_editor_pad(pad: &EditorPad) -> PadAttr {
    use signex_library::primitive::footprint::PadKind as LibPadKind;
    // v0.18.12.1 — carry `drill_diameter_mm` into the sketch
    // PadAttr. Without this, NPT-hole pads minted via Place Hole
    // lose their drill on the first sketch round-trip (the bake
    // emits `Pad::drill = None`). Plated/NPT semantics follow the
    // pad kind.
    let drill = pad
        .drill_diameter_mm
        .map(|d| signex_sketch::attr::DrillSpec {
            diameter_expr: format!("{}mm", format_f64(d)),
            slot_length_expr: None,
            plated: !matches!(pad.kind, LibPadKind::NptHole),
        });
    PadAttr {
        number: pad.number.clone(),
        kind: map_kind(pad.kind),
        side: map_side(pad),
        shape: map_shape(&pad.shape),
        size_x_expr: format!("{}mm", format_f64(pad.size_mm.0)),
        size_y_expr: format!("{}mm", format_f64(pad.size_mm.1)),
        rotation_expr: None,
        offset_x_expr: None,
        offset_y_expr: None,
        drill,
        mask_margin_expr: None,
        paste_margin_expr: None,
        paste_apertures: PasteAperturePattern::Single,
        ..PadAttr::default()
    }
}

fn map_kind(k: LibPadKind) -> SkPadKind {
    match k {
        LibPadKind::Smd => SkPadKind::Smd,
        LibPadKind::Tht => SkPadKind::Tht,
        LibPadKind::NptHole => SkPadKind::NptHole,
        LibPadKind::ConnectorPad => SkPadKind::ConnectorPad,
        LibPadKind::Castellated => SkPadKind::Castellated,
        LibPadKind::Fiducial => SkPadKind::Fiducial,
        // Future-proof the non_exhaustive lib enum.
        _ => SkPadKind::Smd,
    }
}

fn map_side(pad: &EditorPad) -> PadSide {
    use crate::library::editor::footprint::layers::FpLayer;
    let primary = pad.primary_layer();
    match primary {
        FpLayer::FCu | FpLayer::FFab | FpLayer::FSilks => PadSide::Top,
        FpLayer::BCu | FpLayer::BFab | FpLayer::BSilks => PadSide::Bottom,
        _ => PadSide::All,
    }
}

fn map_shape(s: &LibPadShape) -> SkPadShape {
    match s {
        LibPadShape::Round => SkPadShape::Round,
        LibPadShape::Rect => SkPadShape::Rect,
        LibPadShape::Oval => SkPadShape::Oval,
        LibPadShape::RoundRect { radius_ratio } => SkPadShape::RoundRect {
            radius_ratio_expr: format_f64(*radius_ratio),
        },
        LibPadShape::Chamfered {
            chamfer_ratio,
            corners,
        } => SkPadShape::Chamfered {
            chamfer_ratio_expr: format_f64(*chamfer_ratio),
            corners: SkChamferedCorners {
                top_left: corners.top_left,
                top_right: corners.top_right,
                bottom_left: corners.bottom_left,
                bottom_right: corners.bottom_right,
            },
        },
        LibPadShape::Custom(poly) => {
            // Convert lib's free-form polygon into a sketch
            // CustomPadShape::StaticPoints — sketch-profile bake
            // (closed-loop walker) is not used here since literal
            // pads don't have a sketch profile to walk.
            SkPadShape::Custom(CustomPadShape::StaticPoints {
                points: poly.points.clone(),
            })
        }
    }
}

/// Format a float with up to 4 fractional digits, trimming trailing
/// zeros. Keeps the generated expression strings readable
/// (e.g. `1.5` rather than `1.5000000000000`).
fn format_f64(v: f64) -> String {
    let s = format!("{v:.4}");
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// v0.24 Phase 3 (Track A4) — reverse mirror. After every successful
/// solve, walk each pad's `shape_params` and re-derive the
/// `EditorPad.stack.corner_radius_pct` value from the live
/// `sketch.parameters[corner_r_<slug>]` expression. Keeps the
/// Pads-mode "Corner radius %" input in sync when the user edits the
/// sketch parameter from the Sketch-mode Properties row, drags a
/// corner handle, or uses the parameter table.
///
/// Uses the resolved-parameter map (canonical-mm) computed by the
/// solver so dependent expressions like `"corner_r_<slug> = w/4"`
/// are reflected correctly. Silently skips pads whose `shape_params`
/// has no `"corner_r"` binding (Round / Rect / Oval / Chamfered) or
/// whose bound parameter isn't in the resolved map (defensive — a
/// missing parameter shouldn't desync the mirror).
pub fn mirror_solve_to_pad_stack(
    state: &mut super::state::FootprintEditorState,
    resolved: &std::collections::HashMap<String, f64>,
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
        let pct = (corner_r_mm / min_dim) * 100.0;
        // Clamp to valid range (0..50). A radius_ratio > 0.5 is
        // geometrically degenerate (corners would overlap) so the
        // mirror caps the surfaced value rather than letting the UI
        // show a bad number.
        let clamped = pct.clamp(0.0, 50.0);
        pad.stack.corner_radius_pct = Some(clamped);
    }
}

/// v0.24 Track A6 — after every successful solve, re-derive the
/// chamfer anchor Point coordinates from the resolved
/// `chamfer_len_<slug>` parameter. Keeps anchors moving when the
/// Properties-row edit (or any other parameter-table edit) rewrites
/// the shared `chamfer_len` value.
///
/// MVP scope — anchor coords are otherwise literal at mint time.
/// This helper is what makes the shared-parameter binding feel
/// "live" without introducing solver-side constraints (a follow-up
/// task on Track A). For each pad with a `chamfer_len` binding:
///
///   1. Look up the bound parameter in the `resolved` map (canonical
///      mm).
///   2. Walk `pad.shape_params` for every `chamfer_<corner>_anchor1`
///      / `..._anchor2` sidecar; resolve each UUID to a Point in
///      `sketch.entities`; recompute its (x, y) given the pad bbox
///      and the corner identity.
///
/// Defensive on missing data — a pad without `chamfer_len` is
/// silently skipped, a sidecar whose UUID doesn't resolve is
/// logged at warn level.
///
/// The sketch is taken as an explicit `&mut SketchData` borrow
/// rather than via `&mut FootprintEditorState` so the dispatcher
/// can hold both the editor state (immutable for `pads`) and the
/// sketch (mutable for entity coords) at the same call site
/// without overlapping mutable borrows on `Footprint`.
pub fn mirror_solve_to_chamfer_anchors(
    state: &super::state::FootprintEditorState,
    sketch: &mut SketchData,
    resolved: &std::collections::HashMap<String, f64>,
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

        // Per-corner expected (x, y) for each (anchor1, anchor2),
        // matching the `mint_chamfered_pad_geometry` order.
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
            let Some(slug1) = pad.shape_params.get(key1) else {
                continue;
            };
            let Some(slug2) = pad.shape_params.get(key2) else {
                continue;
            };
            let id1 = match uuid::Uuid::parse_str(slug1) {
                Ok(u) => SketchEntityId(u),
                Err(_) => continue,
            };
            let id2 = match uuid::Uuid::parse_str(slug2) {
                Ok(u) => SketchEntityId(u),
                Err(_) => continue,
            };
            for entity in sketch.entities.iter_mut() {
                if entity.id == id1 {
                    if let EntityKind::Point { x, y } = &mut entity.kind {
                        *x = pos1.0;
                        *y = pos1.1;
                    }
                } else if entity.id == id2 {
                    if let EntityKind::Point { x, y } = &mut entity.kind {
                        *x = pos2.0;
                        *y = pos2.1;
                    }
                }
            }
        }
    }
}

/// v0.24 Phase 6 — post-solve mirror for RoundRect pad geometry.
/// When the user edits `corner_r_<slug>` from the Properties panel,
/// the parameter table records the new value but the Arc / anchor /
/// inset-corner Points still hold the literal coords from mint time.
/// This helper walks each pad with a `corner_r` binding, reads the
/// resolved parameter from `resolved`, and rewrites the per-corner
/// Arc-centre Point + the two adjacent anchor Points so the rendered
/// geometry matches the new radius.
///
/// Sister to `mirror_solve_to_chamfer_anchors` — same post-solve hook
/// site (`apply_sketch_edit_with_warnings`), same pattern of
/// "parameter is canonical; geometry follows".
///
/// **Per-corner override caveat:** when a corner has been unlinked
/// (Phase 3 A3 stored a `corner_r_<corner>` per-corner key on
/// `shape_params`), this helper reads that per-corner parameter
/// instead of the shared `corner_r`. Linked corners read shared.
pub fn mirror_solve_to_round_rect_geometry(
    state: &super::state::FootprintEditorState,
    sketch: &mut SketchData,
    resolved: &std::collections::HashMap<String, f64>,
) {
    for pad in state.pads.iter() {
        // Only RoundRect pads carry the shared `corner_r` sidecar.
        // Skip everything else (Round / Oval / Chamfered have their
        // own helpers; Rect / Custom have no parametric corners).
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

        // Per-corner expected positions for the Arc's three Point
        // references (centre, start, end) given a resolved radius `r`.
        // Order matches `mint_round_rect_pad_geometry`'s arc_keys
        // (NE / SE / SW / NW).
        let positions = |r: f64| -> [(SketchEntityId, (f64, f64), (f64, f64), (f64, f64)); 4] {
            // Returns (placeholder_id, centre_pos, start_pos, end_pos)
            // per corner. Caller substitutes the actual ID by reading
            // the Arc entity.
            [
                (
                    SketchEntityId(uuid::Uuid::nil()),
                    (xmax - r, ymin + r),       // NE centre
                    (xmax - r, ymin),            // NE start (top edge)
                    (xmax, ymin + r),            // NE end (right edge)
                ),
                (
                    SketchEntityId(uuid::Uuid::nil()),
                    (xmax - r, ymax - r),       // SE centre
                    (xmax, ymax - r),            // SE start (right edge)
                    (xmax - r, ymax),            // SE end (bottom edge)
                ),
                (
                    SketchEntityId(uuid::Uuid::nil()),
                    (xmin + r, ymax - r),       // SW centre
                    (xmin + r, ymax),            // SW start (bottom edge)
                    (xmin, ymax - r),            // SW end (left edge)
                ),
                (
                    SketchEntityId(uuid::Uuid::nil()),
                    (xmin + r, ymin + r),       // NW centre
                    (xmin, ymin + r),            // NW start (left edge)
                    (xmin + r, ymin),            // NW end (top edge)
                ),
            ]
        };

        let arc_keys: [&str; 4] = [
            "corner_r_ne_arc",
            "corner_r_se_arc",
            "corner_r_sw_arc",
            "corner_r_nw_arc",
        ];
        let per_corner_keys: [&str; 4] = [
            "corner_r_ne",
            "corner_r_se",
            "corner_r_sw",
            "corner_r_nw",
        ];

        for (idx, (arc_key, per_corner_key)) in
            arc_keys.iter().zip(per_corner_keys.iter()).enumerate()
        {
            // Resolve the radius for this corner — per-corner override
            // wins if present (Phase 3 A3 unlink path); shared
            // otherwise.
            let r = if let Some(per_corner_param) = pad.shape_params.get(*per_corner_key) {
                resolved.get(per_corner_param).copied().unwrap_or(shared_r)
            } else {
                shared_r
            };
            // Clamp to the bbox half-extent so a runaway parameter
            // can't push anchors past each other.
            let r = r.max(0.0).min(half_min);

            let Some(arc_slug) = pad.shape_params.get(*arc_key) else {
                continue;
            };
            let Ok(arc_uuid) = uuid::Uuid::parse_str(arc_slug) else {
                continue;
            };
            let arc_id = SketchEntityId(arc_uuid);

            // Read the Arc's three Point references — the Arc entity
            // wasn't moved, so its `center` / `start` / `end` IDs are
            // still the same Points the mint created.
            let (centre_id, start_id, end_id) =
                match sketch.entities.iter().find(|e| e.id == arc_id) {
                    Some(e) => match e.kind {
                        EntityKind::Arc { center, start, end, .. } => (center, start, end),
                        _ => continue,
                    },
                    None => continue,
                };

            let pos_table = positions(r);
            let (_, centre_pos, start_pos, end_pos) = pos_table[idx];

            // Reposition centre, start, end. Each lookup is O(N) but N
            // is small (~20 entities per pad × pad count); the post-
            // solve hook fires at most a few times per second, so the
            // simple find loop stays cheap.
            for entity in sketch.entities.iter_mut() {
                if entity.id == centre_id {
                    if let EntityKind::Point { x, y } = &mut entity.kind {
                        *x = centre_pos.0;
                        *y = centre_pos.1;
                    }
                } else if entity.id == start_id {
                    if let EntityKind::Point { x, y } = &mut entity.kind {
                        *x = start_pos.0;
                        *y = start_pos.1;
                    }
                } else if entity.id == end_id {
                    if let EntityKind::Point { x, y } = &mut entity.kind {
                        *x = end_pos.0;
                        *y = end_pos.1;
                    }
                }
            }
        }
    }
}

/// v0.24 Phase 6 — post-solve mirror for Oval pad geometry. When the
/// user edits `width_<slug>` or `height_<slug>` from the Properties
/// panel, this helper rewrites the 4 anchor Points + 2 arc-centre
/// Points based on the resolved parameters. Bbox corner Points stay
/// pinned (they track `pad.size_mm`, which doesn't change from
/// parameter edits today — that reverse-mirror is a separate v0.25
/// follow-up).
///
/// The helper handles both wide (W ≥ H) and tall (H > W) orientations
/// — it reads the orientation from the resolved parameter values, not
/// the literal `pad.size_mm`, so a parameter edit can swap orientation
/// (e.g. typing W=1, H=3 makes a previously-wide oval go tall).
pub fn mirror_solve_to_oval_geometry(
    state: &super::state::FootprintEditorState,
    sketch: &mut SketchData,
    resolved: &std::collections::HashMap<String, f64>,
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

        // Pad bbox stays anchored to the pad's centre + size_mm. The
        // anchors / arc centres reposition relative to bbox using the
        // resolved width / height as the long / short axes.
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
        let arc_radius = short_axis / 2.0;
        let _ = arc_radius;

        // Anchor positions match `mint_oval_pad_geometry`'s order.
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

        // Reposition anchors via the `oval_anchor_<idx>` sidecars.
        for (idx, target) in anchor_positions.iter().enumerate() {
            let key = format!("oval_anchor_{idx}");
            let Some(slug) = pad.shape_params.get(&key) else {
                continue;
            };
            let Ok(uuid) = uuid::Uuid::parse_str(slug) else {
                continue;
            };
            let id = SketchEntityId(uuid);
            for entity in sketch.entities.iter_mut() {
                if entity.id == id {
                    if let EntityKind::Point { x, y } = &mut entity.kind {
                        *x = target.0;
                        *y = target.1;
                    }
                }
            }
        }
        // Reposition arc centres via the `oval_centre_<idx>` sidecars.
        for (idx, target) in centre_positions.iter().enumerate() {
            let key = format!("oval_centre_{idx}");
            let Some(slug) = pad.shape_params.get(&key) else {
                continue;
            };
            let Ok(uuid) = uuid::Uuid::parse_str(slug) else {
                continue;
            };
            let id = SketchEntityId(uuid);
            for entity in sketch.entities.iter_mut() {
                if entity.id == id {
                    if let EntityKind::Point { x, y } = &mut entity.kind {
                        *x = target.0;
                        *y = target.1;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor_pad(number: &str, x: f64, y: f64) -> EditorPad {
        let mut p = EditorPad::new_default(number.into(), (x, y));
        p.size_mm = (1.0, 0.5);
        p
    }

    #[test]
    fn empty_pads_mint_nothing() {
        let mut fp = Footprint::empty("test");
        let mut pads: Vec<EditorPad> = Vec::new();
        let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
        assert_eq!(n, 0);
        assert!(fp.sketch.is_none() || fp.sketch.as_ref().unwrap().entities.is_empty());
    }

    #[test]
    fn three_pads_mint_three_points_with_pad_attrs() {
        let mut fp = Footprint::empty("test");
        let mut pads = vec![
            editor_pad("1", 0.0, 0.0),
            editor_pad("2", 1.27, 0.0),
            editor_pad("3", 2.54, 0.0),
        ];
        let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
        assert_eq!(n, 3);
        let sketch = fp.sketch.as_ref().unwrap();
        // 1 plane.
        assert_eq!(sketch.planes.len(), 1);
        // v0.16 — per pad: 1 centre Point + 4 corner Points + 4
        // outline Lines = 9 entities. 3 pads × 9 = 27.
        assert_eq!(sketch.entities.len(), 27);
        // The 3 PadAttr-carrying centres should still match v0.15
        // expectations.
        let attr_carriers: Vec<&Entity> =
            sketch.entities.iter().filter(|e| e.pad.is_some()).collect();
        assert_eq!(attr_carriers.len(), 3);
        for entity in attr_carriers {
            assert!(matches!(entity.kind, EntityKind::Point { .. }));
            assert!(!entity.construction);
            let attr = entity.pad.as_ref().unwrap();
            assert!(!attr.number.is_empty());
            assert_eq!(attr.size_x_expr, "1mm");
            assert_eq!(attr.size_y_expr, "0.5mm");
        }
        // v0.15: every pad should now carry the minted entity ID.
        for pad in &pads {
            assert!(pad.sketch_entity_id.is_some());
            assert!(pad.corner_entity_ids.is_some());
        }
    }

    #[test]
    fn skip_when_sketch_already_has_entities() {
        let mut fp = Footprint::empty("test");
        // Pre-populate sketch with one non-construction entity.
        let mut sketch = SketchData::default();
        let plane = Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        };
        sketch.planes.push(plane.clone());
        sketch.entities.push(Entity::new(
            SketchEntityId::new(),
            plane.id,
            EntityKind::Point { x: 0.0, y: 0.0 },
        ));
        fp.sketch = Some(sketch);

        let mut pads = vec![editor_pad("1", 0.0, 0.0)];
        let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
        assert_eq!(n, 0, "auto-mint must skip when sketch is already populated");
        assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 1);
        assert!(
            pads[0].sketch_entity_id.is_none(),
            "skip leaves the link unset"
        );
    }

    #[test]
    fn skip_when_sketch_only_has_construction_entities() {
        // Construction-only sketches are still treated as "no real
        // user authoring", so auto-mint should fire.
        let mut fp = Footprint::empty("test");
        let mut sketch = SketchData::default();
        let plane = Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        };
        let plane_id = plane.id;
        sketch.planes.push(plane);
        let mut construction = Entity::new(
            SketchEntityId::new(),
            plane_id,
            EntityKind::Point { x: 0.0, y: 0.0 },
        );
        construction.construction = true;
        sketch.entities.push(construction);
        fp.sketch = Some(sketch);

        let mut pads = vec![editor_pad("1", 0.0, 0.0)];
        let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
        assert_eq!(n, 1);
        // v0.16 — pre-existing construction entity (1) + minted
        // centre (1) + 4 corner Points + 4 outline Lines = 10.
        assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 10);
        assert!(pads[0].sketch_entity_id.is_some());
        assert!(pads[0].corner_entity_ids.is_some());
    }

    #[test]
    fn mirror_add_pad_links_to_new_sketch_entity() {
        let mut fp = Footprint::empty("test");
        let mut pad = editor_pad("X", 5.0, 5.0);
        assert!(pad.sketch_entity_id.is_none());
        mirror_add_pad_to_sketch(&mut pad, &mut fp);
        let id = pad.sketch_entity_id.expect("mirror should mint id");
        let sketch = fp.sketch.as_ref().unwrap();
        let entity = sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .expect("entity exists");
        match entity.kind {
            EntityKind::Point { x, y } => {
                assert_eq!((x, y), (5.0, 5.0));
            }
            _ => panic!("minted entity must be a Point"),
        }
        assert!(entity.pad.is_some(), "Point should carry PadAttr");
    }

    #[test]
    fn mirror_add_pad_with_existing_link_is_noop() {
        let mut fp = Footprint::empty("test");
        let mut pad = editor_pad("X", 0.0, 0.0);
        pad.sketch_entity_id = Some(SketchEntityId::new());
        mirror_add_pad_to_sketch(&mut pad, &mut fp);
        // Sketch should not have been touched.
        assert!(fp.sketch.is_none() || fp.sketch.as_ref().unwrap().entities.is_empty());
    }

    #[test]
    fn mirror_move_pad_updates_sketch_point() {
        let mut fp = Footprint::empty("test");
        let mut pad = editor_pad("X", 0.0, 0.0);
        mirror_add_pad_to_sketch(&mut pad, &mut fp);
        // Now move the pad.
        pad.position_mm = (3.5, 7.25);
        mirror_move_pad_in_sketch(&pad, &mut fp);
        let id = pad.sketch_entity_id.unwrap();
        let entity = fp
            .sketch
            .as_ref()
            .unwrap()
            .entities
            .iter()
            .find(|e| e.id == id)
            .unwrap();
        match entity.kind {
            EntityKind::Point { x, y } => assert_eq!((x, y), (3.5, 7.25)),
            _ => panic!("entity must still be a Point"),
        }
    }

    #[test]
    fn mirror_delete_pad_drops_sketch_entity() {
        let mut fp = Footprint::empty("test");
        let mut pad = editor_pad("X", 0.0, 0.0);
        mirror_add_pad_to_sketch(&mut pad, &mut fp);
        // v0.16 — 1 centre + 4 corners + 4 lines = 9.
        assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 9);
        mirror_delete_pad_from_sketch(&pad, &mut fp);
        // Drop the centre + corners + outline lines that referenced
        // the dropped corners → 0 left.
        assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 0);
    }

    #[test]
    fn format_f64_trims_trailing_zeros() {
        assert_eq!(format_f64(1.0), "1");
        assert_eq!(format_f64(1.5), "1.5");
        assert_eq!(format_f64(0.25), "0.25");
        assert_eq!(format_f64(1.27), "1.27");
        assert_eq!(format_f64(0.0), "0");
    }

    #[test]
    fn shape_change_preserves_corner_positions() {
        // v0.22 Phase D3 — verifying that flipping a pad's shape
        // (Rect → Oval, etc.) leaves the corner-outline Points
        // untouched. The corners track the pad's bbox, which is
        // derived from position + size only — shape isn't an input,
        // so no re-mint or re-position is needed on shape change.
        //
        // v0.24 Track A note: Round / RoundRect now mint
        // shape-specific geometry (Circle / Arcs) instead of the
        // v0.16 bbox outline, so this test exercises Rect → Oval —
        // both of which still mint the 4-Point bbox outline. Round /
        // RoundRect get their own dedicated regression coverage in
        // `crates/signex-app/tests/regression.rs`.
        use crate::library::editor::footprint::state::FootprintEditorState;

        let mut fp = Footprint::empty("test");
        let mut pad = editor_pad("1", 0.0, 0.0);
        pad.shape = LibPadShape::Rect;
        mirror_add_pad_to_sketch(&mut pad, &mut fp);

        let corner_ids = pad.corner_entity_ids.expect("corners minted");
        let snapshot_corner_pos = |fp: &Footprint| -> Vec<(f64, f64)> {
            corner_ids
                .iter()
                .map(|id| {
                    let entity = fp
                        .sketch
                        .as_ref()
                        .unwrap()
                        .entities
                        .iter()
                        .find(|e| e.id == *id)
                        .expect("corner Point present");
                    match entity.kind {
                        EntityKind::Point { x, y } => (x, y),
                        _ => panic!("corner must be Point"),
                    }
                })
                .collect()
        };

        let before = snapshot_corner_pos(&fp);

        // Flip the shape — emulating a Properties-panel shape change.
        // Pads-mode dispatch paths call `with_selected_pad` which
        // ultimately calls `sync_pads_to_primitive`; that path does
        // NOT touch corner positions because shape is bbox-orthogonal.
        pad.shape = LibPadShape::Oval;
        let mut s = FootprintEditorState::empty();
        s.pads = vec![pad.clone()];
        FootprintEditorState::sync_pads_to_primitive(&s, &mut fp);

        let after = snapshot_corner_pos(&fp);

        assert_eq!(
            before, after,
            "corner positions must remain stable across shape changes"
        );
    }
}
