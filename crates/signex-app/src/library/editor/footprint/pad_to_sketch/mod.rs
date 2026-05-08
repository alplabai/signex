//! Mint sketch entities for literal pads — bidirectional sketch ↔
//! pads sync.
//!
//! When the user enters Sketch mode for a footprint that has literal
//! pads but no sketch entities yet, this module auto-creates a
//! `Point` + `PadAttr` for every pad so the round-trip is identity-
//! preserving.
//!
//! Submodules:
//! - [`helpers`] — small `push_point` / `push_line` / `push_arc_ccw`
//!   primitives that collapse the repeated mint blocks.
//! - [`attr`] — `EditorPad ↔ PadAttr` mapping and the BoardTop plane
//!   helper.
//! - [`mint`] — per-shape `mint_*_pad_geometry` functions.
//! - [`solve`] — post-solve "reverse mirror" helpers.

mod attr;
mod helpers;
mod mint;
mod solve;

#[cfg(test)]
mod tests;

use signex_library::primitive::footprint::{Footprint, PadShape as LibPadShape};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;

use super::state::EditorPad;
use attr::{ensure_board_top_plane, id_slug, pad_attr_from_editor_pad};
use mint::{
    mint_chamfered_pad_geometry, mint_oval_pad_geometry, mint_pad_corner_outline,
    mint_round_pad_geometry, mint_round_rect_pad_geometry,
};

pub use solve::{
    mirror_solve_to_chamfer_anchors, mirror_solve_to_oval_geometry, mirror_solve_to_oval_size,
    mirror_solve_to_pad_stack, mirror_solve_to_round_rect_geometry,
};

/// When the user transitions into Sketch mode for the first time on
/// a footprint that has literal pads but an empty sketch, mint a
/// `Point` + `PadAttr` for each pad. Writes the minted sketch entity
/// IDs back into each `EditorPad.sketch_entity_id` so subsequent
/// Pads-mode edits can mirror through the link. Returns the number
/// of entities minted (zero if the sketch already had content or no
/// literal pads existed).
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
        pad.sketch_entity_id = Some(entity_id);
        // v0.27 — branch on `pad.shape` so legacy pads get the same
        // parametric primitive layout as freshly-placed pads. Round →
        // Circle + diameter; RoundRect / Oval / Chamfered → their
        // shape-specific anchor sets; Rect / Custom → bbox outline.
        // Pre-v0.27 this always called `mint_pad_corner_outline`,
        // which is why round pads opened in Sketch mode showed 4
        // disconnected bbox corners with no diameter handle.
        mint_shape_geometry_for(sketch, plane_id, pad, entity_id);
        minted += 1;
    }
    minted
}

/// v0.15 — when a pad is added in Pads mode, mirror the new pad into
/// the sketch as a `Point` + `PadAttr`. Stores the minted sketch
/// entity ID back on the editor pad so later moves / deletes can
/// mirror through.
///
/// v0.24 Track A — branches on `pad.shape` so each shape mints its
/// own parametric geometry: Round → Circle + diameter param;
/// RoundRect → 4 anchors + 4 inset corners + 4 Lines + 4 Arcs
/// sharing `corner_r`; Oval → stadium with shared `width`/`height`;
/// Chamfered → outline with shared `chamfer_len`. Other shapes get
/// the v0.16 4-Line bbox outline.
pub fn mirror_add_pad_to_sketch(pad: &mut EditorPad, footprint: &mut Footprint) {
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
    mint_shape_geometry_for(sketch, plane_id, pad, entity_id);
}

/// Branch on `pad.shape` to mint the correct sketch geometry — Circle
/// for Round, parametric anchors+arcs for RoundRect, stadium for Oval,
/// chamfer outline for Chamfered, plain bbox-corner outline for Rect /
/// Custom / etc. Writes `pad.corner_entity_ids` accordingly.
///
/// Shared by `mirror_add_pad_to_sketch` (Pads-mode click → mint) and
/// `auto_mint_for_literal_pads` (first Sketch-mode entry on a footprint
/// authored before sketch existed). Without sharing, legacy pads
/// always got the bbox outline regardless of shape — Round pads in
/// particular ended up with 4 disconnected corner Points, so dragging
/// one corner just deformed the outline without resizing the pad.
fn mint_shape_geometry_for(
    sketch: &mut SketchData,
    plane_id: signex_sketch::plane::PlaneId,
    pad: &mut EditorPad,
    entity_id: SketchEntityId,
) {
    match &pad.shape {
        LibPadShape::Round => {
            mint_round_pad_geometry(sketch, plane_id, pad, entity_id);
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
    helpers::set_point_xy(sketch, entity_id, pad.position_mm.0, pad.position_mm.1);
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
            helpers::set_point_xy(sketch, *id, *px, *py);
        }
    }
}

/// v0.15 — when a pad is deleted in Pads mode, also drop its backing
/// sketch entity (and any constraints that referenced it).
///
/// v0.24 Track A — also drop linked Circle / Arc entities and any
/// sketch parameters keyed by the centre-Point UUID slug. RoundRect's
/// anchor / inset-corner Points are pulled into the drop set via a
/// secondary sweep — they're referenced indirectly by Arcs whose
/// `center` is the inset corner.
pub fn mirror_delete_pad_from_sketch(pad: &EditorPad, footprint: &mut Footprint) {
    let Some(entity_id) = pad.sketch_entity_id else {
        return;
    };
    let Some(sketch) = footprint.sketch.as_mut() else {
        return;
    };
    let mut to_drop: Vec<SketchEntityId> = vec![entity_id];
    if let Some(corners) = pad.corner_entity_ids {
        to_drop.extend_from_slice(&corners);
    }
    // v0.24 Track A5 + A6 — seed any sidecar entity IDs stored on
    // `pad.shape_params`. Sidecar values are UUID slugs (no dashes);
    // canonical bindings (`corner_r_<slug>`, etc.) have a leading
    // identifier prefix so they fall through.
    for value in pad.shape_params.values() {
        if let Ok(uuid) = uuid::Uuid::parse_str(value) {
            to_drop.push(SketchEntityId(uuid));
        }
    }
    let mut drop_set: std::collections::HashSet<SketchEntityId> = to_drop.iter().copied().collect();

    // Secondary sweep — pull every Line / Arc / Circle that touches a
    // dropped ID into the drop set (along with the Points it
    // references). One pass suffices because Points are leaves.
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
                !(drop_set.contains(start) || drop_set.contains(end))
            }
            EntityKind::Arc {
                center, start, end, ..
            } => {
                !(drop_set.contains(center)
                    || drop_set.contains(start)
                    || drop_set.contains(end))
            }
            EntityKind::Circle { center, .. } => !drop_set.contains(center),
            EntityKind::Point { .. } => true,
        }
    });
    // Drop dangling constraint refs — coarse rule via Debug
    // stringification (mirrors the SketchEdit::DeleteEntity path).
    let id_str = entity_id.to_string();
    sketch
        .constraints
        .retain(|c| !format!("{:?}", c.kind).contains(&id_str));

    // v0.24 Track A — drop shape parameters keyed by the centre-Point
    // UUID slug.
    let slug = id_slug(entity_id);
    sketch.parameters.0.retain(|name, _| !name.ends_with(&slug));
}
