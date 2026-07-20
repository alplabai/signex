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
//! - [`remint_in_place`] — the id-preserving re-mint a live drag needs.
//! - [`solve`] — post-solve "reverse mirror" helpers.
//! - [`ownership`] — the single answer to "which sketch entities does
//!   this pad own?", shared by the move and delete mirrors.

mod attr;
mod helpers;
mod mint;
mod ownership;
mod remint_in_place;
mod solve;

#[cfg(test)]
mod tests;

use signex_library::primitive::footprint::{Footprint, PadShape as LibPadShape};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use std::collections::HashSet;

use super::state::EditorPad;
pub use attr::{mirror_pad_attrs_into_sketch, mirror_rotation_expr};
use attr::{ensure_board_top_plane, id_slug, pad_attr_from_editor_pad};
use mint::{
    mint_chamfered_pad_geometry, mint_oval_pad_geometry, mint_pad_corner_outline,
    mint_round_pad_geometry, mint_round_rect_pad_geometry,
};

/// The sketch-side expression for a pad's rotation. Shared by the
/// mint path (`pad_attr_from_editor_pad`) and the Pads→Sketch
/// attribute mirror in `sync_pads_to_primitive`, so both write the
/// identical string and the two persistence paths cannot drift.
/// Emits an explicit `deg` unit; `signex_bake::pad` reads it back
/// through the Angle unit family.
pub fn rotation_expr(deg: f64) -> String {
    format!("{}deg", attr::format_f64(deg))
}

pub use remint_in_place::remint_pad_geometry_in_place;
pub use solve::{
    mirror_solve_to_chamfer_anchors, mirror_solve_to_oval_geometry, mirror_solve_to_oval_size,
    mirror_solve_to_pad_stack, mirror_solve_to_round_rect_geometry,
};

/// Whether the footprint's sketch already holds authored (non-
/// construction) content.
///
/// This is exactly the condition [`auto_mint_for_literal_pads`]
/// early-returns on, so a caller that mints a single pad (paste) can
/// use it to tell the two cases apart: authored → auto-mint will never
/// pick this pad up, mint it now; not authored → auto-mint still
/// covers it, and minting early is what would break that.
pub fn sketch_is_authored(footprint: &Footprint) -> bool {
    footprint
        .sketch
        .as_ref()
        .is_some_and(|s| s.entities.iter().any(|e| !e.construction))
}

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
    if sketch_is_authored(footprint) {
        return 0;
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
    mint_pad_entities(pad, footprint, SketchEntityId::new());
}

/// Mint a pad's centre `Point` + `PadAttr` + per-shape sidecar geometry
/// under `entity_id`. The single body behind both the first mint
/// (`mirror_add_pad_to_sketch`) and the re-mint
/// (`remint_pad_geometry`), so a transform can never regenerate
/// geometry through a second, differently-behaved copy of the layout
/// rules.
fn mint_pad_entities(pad: &mut EditorPad, footprint: &mut Footprint, entity_id: SketchEntityId) {
    let plane_id = ensure_board_top_plane(footprint);
    let sketch = footprint.sketch.get_or_insert_with(SketchData::default);
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

/// Regenerate a pad's sketch sidecar after a transform that changes
/// the pad FRAME — rotate, flip, anything that moves
/// `local_to_world_mm`. Drops the old geometry through
/// [`mirror_delete_pad_from_sketch`] and re-mints it through
/// [`mint_pad_entities`], so the new angle is honoured BY
/// CONSTRUCTION.
///
/// This is the whole point: per-shape layout knowledge (where a
/// chamfer anchor sits, where a round-rect arc centre sits) lives in
/// `mint` and nowhere else. Repositioning the four bbox corners with
/// [`mirror_move_pad_in_sketch`] is correct only for the shapes whose
/// entire outline IS those four corners — every parametric shape ends
/// up with rotated corners joined to un-rotated anchors, an outline
/// that is neither the old shape nor the new one. Teaching the corner
/// mover each shape's layout would make a third copy of those rules;
/// re-minting keeps one.
///
/// Re-minting also rewrites the `PadAttr` via `pad_attr_from_editor_pad`,
/// which is what keeps `attr.shape` — the field `signex_bake::pad`
/// reads — in step with the editor's `pad.shape` after a flip swaps
/// the chamfer corners.
///
/// Returns `false` for a sketch-profile pad, where nothing was
/// regenerated: its copper is a traced loop, not a parametric shape,
/// so there is no layout to re-derive and the wildcard mint branch
/// would fabricate a bbox outline it never had. The caller owes the
/// user a warning in that case.
///
/// COST: constraints the user authored against the old outline
/// entities go with the old entities. The delete path already sweeps
/// them; a rotate is therefore a constraint-dropping edit. It is
/// undoable in one step — the history snapshot carries the whole
/// footprint file, sketch included.
pub fn remint_pad_geometry(pad: &mut EditorPad, footprint: &mut Footprint) -> bool {
    let Some(entity_id) = pad.sketch_entity_id else {
        // No sketch link yet — nothing minted, nothing to regenerate.
        return true;
    };
    if is_sketch_profile_pad(pad, footprint) {
        return false;
    }
    mirror_delete_pad_from_sketch(pad, footprint);
    // Reuse the SAME centre id: selection state and any external
    // handle on the pad's centre `Point` keep pointing at the pad
    // rather than dangling on a fresh UUID.
    pad.corner_entity_ids = None;
    mint_pad_entities(pad, footprint, entity_id);
    true
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
    // This fn (re)generates the pad's shape geometry, so it owns the
    // shape-parameter sidecar. Clear it first: when a pad's shape
    // changes (e.g. RoundRect → Round), stale keys like `corner_r_*`
    // would otherwise linger alongside the new shape's params and
    // confuse the solver / next bake. Each shape branch below
    // re-inserts exactly the keys it needs.
    pad.shape_params.clear();
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
    // Record the durable ownership ledger onto the centre `PadAttr`.
    // The three `EditorPad` fields the branches above just wrote are
    // session-volatile — `EditorPad::from_pad` drops all of them — so
    // without this the reopened pad owns nothing and a Pads-mode move
    // strands the outline it just minted.
    ownership::record_ledger(sketch, pad, entity_id);
}

/// v0.15 — when a pad moves in Pads mode (drag), update its backing
/// sketch `Point`'s coordinates so the sketch stays in sync. No-op
/// when the pad has no backing sketch entity yet.
///
/// v0.14.2 — a `Custom(SketchProfile)` pad does not own its geometry:
/// "Make Pad from Profile" mints only a centre `Point` at the loop's
/// centroid and references the loop by seed Line, leaving
/// `corner_entity_ids` `None`. Moving such a pad therefore has to
/// translate the profile itself, or the loop stays where it was drawn
/// — visibly the sketch shape doesn't follow the pad, and silently the
/// bake (`local_pts = world_pts - pad_position`, `signex-bake`
/// `pad.rs`) resolves the copper back to the ORIGINAL location, so the
/// exported footprint has the pad in the wrong place.
///
/// The profile is authoritative for a sketch-profile pad; the pad's
/// position is a derived handle, and moving the handle means
/// translating the geometry.
///
/// Translates the pad's WHOLE owned set
/// ([`ownership::owned_sketch_entities`]) rather than just the centre
/// and the four bbox corners. RoundRect's 8 edge anchors + 4 inset
/// arc-centres, Oval's 4 anchors + 2 arc-centres and Chamfered's
/// per-corner anchors are all minted NON-construction, so leaving them
/// behind made the bake emit copper from the stranded geometry — and
/// nothing downstream repaired it (`sync_pads_to_primitive` copies
/// attributes only, it never re-mints).
pub fn mirror_move_pad_in_sketch(pad: &EditorPad, footprint: &mut Footprint) {
    let Some(entity_id) = pad.sketch_entity_id else {
        return;
    };
    let Some(sketch) = footprint.sketch.as_mut() else {
        return;
    };
    // Read the delta BEFORE any mutation — the centre is the only
    // absolute reference, and every other owned Point rides on it.
    let Some((old_x, old_y)) = point_xy_of(sketch, entity_id) else {
        return;
    };
    let (dx, dy) = (pad.position_mm.0 - old_x, pad.position_mm.1 - old_y);
    // Must run BEFORE the centre is overwritten — the delta is measured
    // from the centre's current position. The centre is a standalone
    // minted Point and never part of the traced loop, so it cannot be
    // translated twice. Everything the profile trace DID move is
    // returned and skipped below rather than assumed disjoint: a
    // `Custom` pad minted through `mint_shape_geometry_for` gets
    // `corner_entity_ids`, and if its `PadAttr` is a `SketchProfile`
    // over that same outline the delta would otherwise land twice.
    let already_translated = translate_profile_with_pad(sketch, entity_id, pad.position_mm);
    // A pad move is a pure translation, so a single delta covers every
    // shape — no per-shape position tables (those belong to `solve`,
    // which handles radius / size changes). Every owned entity (centre,
    // bbox corners, and the parametric anchors / arc-centres a RoundRect
    // / Oval / Chamfered pad mints) rides the same delta through the one
    // `ownership::owned_sketch_entities` ledger — no second, divergent
    // "what does a pad own" enumeration (#424). Lines, Arcs and the Round
    // Circle follow their endpoints, and `set_point_xy` no-ops on them.
    let owned = ownership::owned_sketch_entities(pad, sketch);
    for id in owned {
        if id == entity_id || already_translated.contains(&id) {
            continue;
        }
        if let Some((x, y)) = point_xy_of(sketch, id) {
            helpers::set_point_xy(sketch, id, x + dx, y + dy);
        }
    }
    // The centre is set absolutely, not by delta: `old + (new - old)`
    // is not bit-identical to `new` in floating point, and the centre
    // is the pad's authoritative handle.
    helpers::set_point_xy(sketch, entity_id, pad.position_mm.0, pad.position_mm.1);
    reassert_bbox_corners(sketch, pad, &already_translated);
}

/// Re-state the four bbox-corner Points ABSOLUTELY from `pad.bbox_mm()`
/// after the delta pass.
///
/// A no-op on a healthy pad — the delta already landed them there. It
/// exists for the unhealthy one: the corners are the only owned Points
/// whose position is fully derivable from `Pad`, so any drift between
/// the pad's declared size and its sketch outline is repairable, and
/// repairing it on every move is what stops repeated moves from
/// accumulating f64 rounding in the outline forever.
///
/// Skips anything the profile trace already placed — a
/// `Custom(SketchProfile)` loop is authoritative over its own points
/// and owes nothing to the pad's bbox.
fn reassert_bbox_corners(
    sketch: &mut SketchData,
    pad: &EditorPad,
    already_translated: &HashSet<SketchEntityId>,
) {
    let Some(corners) = pad.corner_entity_ids else {
        return;
    };
    // `[ne, se, sw, nw]` — the order `helpers::bbox_corner_points` and
    // `mint::mint_pad_corner_outline` both mint in. Uses the ROTATED
    // corners (#433): for `rotation_deg == 0` these ARE the axis-aligned
    // bbox corners (an unrotated pad is unchanged), but a rotated pad's
    // corners follow its frame across a move instead of snapping back to
    // axis-aligned. Parametric anchors (RoundRect / Oval / Chamfered) are
    // NOT handled here — `mirror_move_pad_in_sketch`'s owned-entity delta
    // pass already translated every one of them through the ownership
    // ledger, so a second sweep here would double-apply the delta.
    let positions = pad.rotated_corners_mm();
    for (id, (x, y)) in corners.into_iter().zip(positions) {
        if already_translated.contains(&id) {
            continue;
        }
        helpers::set_point_xy(sketch, id, x, y);
    }
}

/// THE SEEDING RULE, in one place. A `pad.shape_params` VALUE that
/// parses as a UUID names one of this pad's own sidecar entities; a
/// canonical parameter binding (`corner_r` -> `corner_r_<slug>`, see
/// `helpers::bind_shape_param`) is a parameter NAME, does not parse,
/// and falls through.
///
/// Three readers need it — the move path's sidecar sweep, the delete
/// path's drop set, and the in-place re-mint's pairing — and their
/// TRAVERSALS are deliberately different. The seed must not be: a
/// future shape that records its ids under a new key convention has to
/// be taught here once, or it gets anchors that translate on a drag
/// and survive a delete.
fn sidecar_id(value: &str) -> Option<SketchEntityId> {
    uuid::Uuid::parse_str(value).ok().map(SketchEntityId)
}

/// A sketch-profile pad's copper is a traced loop, not a parametric
/// shape — there is nothing on the pad for a frame transform to ride
/// on, and the loop stays exactly as drawn. This is the warning
/// [`remint_pad_geometry`]'s `false` return obliges its caller to
/// emit, kept next to the function that owes it so every caller says
/// the same thing.
pub fn warn_profile_pad_untransformed(op: &str, pad_number: &str) {
    crate::diagnostics::log_warning(format!(
        "{op}: pad {pad_number} is a sketch-profile pad — its outline loop was NOT transformed. \
         Transform the sketch loop by hand before baking."
    ));
}

/// True when this pad's copper is a traced sketch loop ("Make Pad
/// from Profile") rather than a parametric shape.
///
/// Such a pad owns no `size_mm` / `shape` geometry for a transform to
/// ride on — its outline is the loop. A caller that mirrors or
/// otherwise reshapes pad copper must either transform the loop too or
/// say out loud that it did not; silently leaving the loop put bakes
/// the un-transformed shape.
pub fn is_sketch_profile_pad(pad: &EditorPad, footprint: &Footprint) -> bool {
    let Some(centre) = pad.sketch_entity_id else {
        return false;
    };
    let Some(sketch) = footprint.sketch.as_ref() else {
        return false;
    };
    profile_seed_line(sketch, centre).is_some()
}

/// Raw x/y of a sketch `Point` entity, straight off `SketchData` —
/// the pre-solve authored position, which is what the mirror mutates.
fn point_xy_of(sketch: &SketchData, id: SketchEntityId) -> Option<(f64, f64)> {
    sketch
        .entities
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
}

/// Seed Line of a `Custom(SketchProfile)` pad, read off the `PadAttr`
/// carried by its centre `Point`. `None` for every other shape — those
/// pads own their geometry through `corner_entity_ids` instead.
fn profile_seed_line(sketch: &SketchData, centre: SketchEntityId) -> Option<SketchEntityId> {
    use signex_sketch::attr::{CustomPadShape, PadShape as SkPadShape};

    let attr = sketch
        .entities
        .iter()
        .find(|e| e.id == centre)?
        .pad
        .as_ref()?;
    match &attr.shape {
        SkPadShape::Custom(CustomPadShape::SketchProfile { source }) => source.first().copied(),
        _ => None,
    }
}

/// Translate a sketch-profile pad's loop so it tracks the pad to
/// `new_position`, returning every Point it moved so the caller can
/// keep the delta off them a second time. No-op (and empty) for
/// non-profile pads, for a zero delta, or when the loop can't be
/// traced.
///
/// Raw-position mutation matches how a user-driven sketch drag behaves
/// (`SketchEdit::MovePoint` writes entity x/y and lets the next solve
/// run): a uniform translate preserves translation-invariant
/// constraints, and anything anchored absolutely re-asserts itself on
/// the next solve — the same outcome as dragging the loop by hand.
///
/// An untraceable loop (open / branching after a Sketch-mode edit)
/// leaves the profile put. That is not silent: the bake re-walks the
/// same loop and pushes its own "trace failed … falling back to Rect"
/// warning.
fn translate_profile_with_pad(
    sketch: &mut SketchData,
    centre: SketchEntityId,
    new_position: (f64, f64),
) -> HashSet<SketchEntityId> {
    let mut moved: HashSet<SketchEntityId> = HashSet::new();
    let Some(seed) = profile_seed_line(sketch, centre) else {
        return moved;
    };
    let Some((old_x, old_y)) = point_xy_of(sketch, centre) else {
        return moved;
    };
    let (dx, dy) = (new_position.0 - old_x, new_position.1 - old_y);
    if dx == 0.0 && dy == 0.0 {
        return moved;
    }
    let Ok(traced) = signex_bake::profile::trace_closed_profile_entities(sketch, seed) else {
        return moved;
    };
    // `traced.points` is already deduplicated and includes Arc centres,
    // so each owned Point takes the delta exactly once.
    for id in traced.points {
        if let Some((x, y)) = point_xy_of(sketch, id) {
            helpers::set_point_xy(sketch, id, x + dx, y + dy);
            moved.insert(id);
        }
    }
    moved
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
    // v0.24 Track A5 + A6 — the seed spans all three ownership fields
    // (centre, bbox corners, `shape_params` sidecars); see [`ownership`].
    let to_drop = ownership::owned_sketch_entities(pad, sketch);
    let mut drop_set: HashSet<SketchEntityId> = to_drop.iter().copied().collect();

    // Secondary sweep — a Line / Arc / Circle that references a dropped
    // Point cannot survive it, so it joins the drop set (which also
    // feeds the constraint sweep below). One pass suffices because
    // Points are leaves.
    //
    // Only the dependent entity is added, never the Points it
    // references. Pulling those in too was over-collection: the pad's
    // own anchors / inset arc-centres are already reached by
    // [`ownership::owned_sketch_entities`] expanding its Arcs forward,
    // so the only ids the far side of this edge can contribute are
    // FOREIGN — the far endpoint of a user-drawn silk Line that merely
    // happens to share one of the pad's anchor Points. Deleting a pad
    // must not take user geometry with it.
    let mut secondary_drops: HashSet<SketchEntityId> = HashSet::new();
    for entity in &sketch.entities {
        if drop_set.contains(&entity.id) {
            continue;
        }
        let touched = match &entity.kind {
            EntityKind::Line { start, end } => drop_set.contains(start) || drop_set.contains(end),
            EntityKind::Arc {
                center, start, end, ..
            } => drop_set.contains(center) || drop_set.contains(start) || drop_set.contains(end),
            EntityKind::Circle { center, .. } => drop_set.contains(center),
            EntityKind::Point { .. } => false,
        };
        if touched {
            secondary_drops.insert(entity.id);
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
            } => !(drop_set.contains(center) || drop_set.contains(start) || drop_set.contains(end)),
            EntityKind::Circle { center, .. } => !drop_set.contains(center),
            EntityKind::Point { .. } => true,
        }
    });
    // Drop dangling constraint refs — coarse rule via Debug
    // stringification (mirrors the SketchEdit::DeleteEntity path).
    //
    // Tested against the WHOLE drop set, not the centre alone. Every
    // outline corner and every anchor just went with it, so a
    // constraint the user authored against a chamfer anchor is as
    // dangling as one against the centre; matching only the centre
    // left those rows behind pointing at entities that no longer
    // exist. It matters more now that re-mint runs this path on every
    // rotate and flip rather than only on a pad delete — the stale
    // rows would otherwise accumulate one transform at a time.
    let dropped_ids: Vec<String> = drop_set.iter().map(|id| id.to_string()).collect();
    sketch.constraints.retain(|c| {
        let rendered = format!("{:?}", c.kind);
        !dropped_ids.iter().any(|id| rendered.contains(id))
    });

    // v0.24 Track A — drop shape parameters keyed by the centre-Point
    // UUID slug. `contains`, not `ends_with`: the per-corner unlink
    // override is minted as `corner_r_<slug>_ne` (see
    // `updates/sketch/pad_bridge.rs`), which ends with the corner
    // suffix and would otherwise be orphaned in the parameter table
    // forever. Slugs are 32-hex UUIDs, so a false positive is nil.
    let slug = id_slug(entity_id);
    sketch.parameters.0.retain(|name, _| !name.contains(&slug));
}
