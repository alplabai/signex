//! The id-preserving form of the pad-sidecar re-mint.
//!
//! [`super::remint_pad_geometry`] drops the pad's sketch geometry and
//! mints it afresh, which is right for a discrete command (rotate,
//! flip, a Properties-panel field) and wrong for a live drag: the
//! pointer holds the id of the entity the user grabbed for the whole
//! gesture. This module mints the same geometry through the same owner
//! and writes it ONTO the entities already there.

use signex_library::primitive::footprint::Footprint;
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;

use super::EditorPad;
use super::{is_sketch_profile_pad, mint_pad_entities, remint_pad_geometry, sidecar_id};

/// Regenerate a pad's sidecar geometry through the same single owner
/// [`remint_pad_geometry`] uses — [`mint_pad_entities`] — but writing
/// the freshly derived positions ONTO the existing entities instead of
/// replacing them.
///
/// This is the form a live drag needs. The pointer latches the id of
/// the Line or Point the user grabbed at press and streams one message
/// per cursor tick against it (`drag_tick_line` / `drag_tick_point` in
/// `canvas/input/pointer.rs`). A drop-and-re-mint invalidates that id
/// on the first tick, so the second tick addresses an entity that no
/// longer exists and the drag freezes one frame in while the cursor
/// keeps moving. Keeping the ids also keeps the constraints the user
/// authored against the outline, which a per-tick re-mint would wipe
/// on every mouse-move.
///
/// The layout rules still live only in `mint`: this mints a REFERENCE
/// copy of the same pad into a scratch footprint, under the same centre
/// id so the shape parameters it binds come out under the names the
/// real sketch already holds, and then copies geometry across the
/// old-id -> new-id correspondence.
///
/// Falls back to the full [`remint_pad_geometry`] when the entity
/// structure itself changed — a shape swap mints a different set, and
/// there is nothing to write onto. Returns `false` for a sketch-profile
/// pad exactly as [`remint_pad_geometry`] does, and the caller owes the
/// user the same warning.
pub fn remint_pad_geometry_in_place(pad: &mut EditorPad, footprint: &mut Footprint) -> bool {
    let Some(centre) = pad.sketch_entity_id else {
        return true;
    };
    if is_sketch_profile_pad(pad, footprint) {
        return false;
    }
    let mut reference = pad.clone();
    reference.sketch_entity_id = None;
    reference.corner_entity_ids = None;
    let mut scratch = Footprint::empty("pad-remint-reference");
    mint_pad_entities(&mut reference, &mut scratch, centre);
    let Some(reference_sketch) = scratch.sketch.take() else {
        return true;
    };
    let pairs = pair_sidecar_entities(pad, &reference, &reference_sketch, footprint)
        // The pairing must reach every bit of the reference mint that
        // carries geometry, or what gets written back is a PARTIAL
        // re-mint — some of the new frame, some of the old, which is
        // the two-frame outline this invariant exists to kill. A shape
        // whose geometry hangs off the centre alone (Round's Circle is
        // named by neither `corner_entity_ids` nor `shape_params`) is
        // not reachable from the seeds, and lands here.
        .filter(|pairs| pairing_covers_all_geometry(pairs, &reference_sketch));
    let Some(pairs) = pairs else {
        return remint_pad_geometry(pad, footprint);
    };
    let Some(sketch) = footprint.sketch.as_mut() else {
        return true;
    };
    for (old_id, new_id) in pairs {
        copy_entity_geometry(&reference_sketch, new_id, sketch, old_id);
    }
    for (name, src) in reference_sketch.parameters.iter() {
        sketch.parameters.insert(name, src);
    }
    true
}

/// Walk the pad's entities and the reference mint's in lockstep,
/// pairing old id to new id. `None` when the two sets are not the same
/// shape — a differing key set, a corner array on one side only, or a
/// pair whose kinds disagree — which is what a shape swap looks like
/// from here.
///
/// A paired walk, not either of the single-sided sweeps: it descends
/// through Line / Arc / Circle because a RoundRect records only its
/// four Arcs on `shape_params` and the anchors and inset centres hang
/// off them.
/// The pairs the walk starts from: the centre, the bbox corners, and
/// whatever [`sidecar_id`] names on `shape_params`. `None` when the two
/// pads do not record the same THINGS — a differing key set, a corner
/// array on one side only, or a key that is an id on one side and a
/// parameter name on the other.
fn seed_sidecar_pairs(
    pad: &EditorPad,
    reference: &EditorPad,
) -> Option<Vec<(SketchEntityId, SketchEntityId)>> {
    if pad.shape_params.len() != reference.shape_params.len() {
        return None;
    }
    let mut seeds: Vec<(SketchEntityId, SketchEntityId)> =
        vec![(pad.sketch_entity_id?, reference.sketch_entity_id?)];
    match (pad.corner_entity_ids, reference.corner_entity_ids) {
        (Some(old), Some(new)) => seeds.extend(old.into_iter().zip(new)),
        (None, None) => {}
        _ => return None,
    }
    for (key, new_value) in &reference.shape_params {
        match (
            sidecar_id(pad.shape_params.get(key)?),
            sidecar_id(new_value),
        ) {
            (Some(old), Some(new)) => seeds.push((old, new)),
            // Both sides bound the same canonical parameter name.
            (None, None) => {}
            _ => return None,
        }
    }
    Some(seeds)
}

fn pair_sidecar_entities(
    pad: &EditorPad,
    reference: &EditorPad,
    reference_sketch: &SketchData,
    footprint: &Footprint,
) -> Option<Vec<(SketchEntityId, SketchEntityId)>> {
    use std::collections::HashSet;

    let sketch = footprint.sketch.as_ref()?;
    let mut queue: Vec<(SketchEntityId, SketchEntityId)> = seed_sidecar_pairs(pad, reference)?;
    let mut seen: HashSet<SketchEntityId> = HashSet::new();
    let mut pairs: Vec<(SketchEntityId, SketchEntityId)> = Vec::new();
    while let Some((old_id, new_id)) = queue.pop() {
        if !seen.insert(old_id) {
            continue;
        }
        let old = sketch.entities.iter().find(|e| e.id == old_id)?;
        let new = reference_sketch.entities.iter().find(|e| e.id == new_id)?;
        match (&old.kind, &new.kind) {
            (EntityKind::Point { .. }, EntityKind::Point { .. }) => {}
            (EntityKind::Circle { center: a, .. }, EntityKind::Circle { center: b, .. }) => {
                queue.push((*a, *b));
            }
            (EntityKind::Line { start: a1, end: a2 }, EntityKind::Line { start: b1, end: b2 }) => {
                queue.push((*a1, *b1));
                queue.push((*a2, *b2));
            }
            (
                EntityKind::Arc {
                    center: ac,
                    start: a1,
                    end: a2,
                    ..
                },
                EntityKind::Arc {
                    center: bc,
                    start: b1,
                    end: b2,
                    ..
                },
            ) => {
                queue.push((*ac, *bc));
                queue.push((*a1, *b1));
                queue.push((*a2, *b2));
            }
            _ => return None,
        }
        pairs.push((old_id, new_id));
    }
    Some(pairs)
}

/// True when every reference entity that CARRIES geometry — a Point's
/// position, a Circle's radius — was paired. A Line holds nothing but
/// references to Points, so leaving one unpaired changes no geometry;
/// a stranded Point or Circle means a partial write-back.
fn pairing_covers_all_geometry(
    pairs: &[(SketchEntityId, SketchEntityId)],
    reference_sketch: &SketchData,
) -> bool {
    let paired: std::collections::HashSet<SketchEntityId> =
        pairs.iter().map(|&(_, new)| new).collect();
    reference_sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Point { .. } | EntityKind::Circle { .. }))
        .all(|e| paired.contains(&e.id))
}

/// Copy one reference entity's geometry onto the entity it pairs with.
/// Points carry their position, Circles their radius, Arcs their sweep;
/// a Line's geometry is entirely in the Points it references. The
/// centre also carries its `PadAttr`, which is how the size expressions
/// `signex_bake::pad` reads stay in step with the new frame.
fn copy_entity_geometry(
    from: &SketchData,
    from_id: SketchEntityId,
    into: &mut SketchData,
    into_id: SketchEntityId,
) {
    let Some(source) = from.entities.iter().find(|e| e.id == from_id) else {
        return;
    };
    let (kind, attr) = (source.kind.clone(), source.pad.clone());
    let Some(target) = into.entities.iter_mut().find(|e| e.id == into_id) else {
        return;
    };
    match (&mut target.kind, kind) {
        (EntityKind::Point { x, y }, EntityKind::Point { x: nx, y: ny }) => {
            *x = nx;
            *y = ny;
        }
        (EntityKind::Circle { radius, .. }, EntityKind::Circle { radius: nr, .. }) => *radius = nr,
        (EntityKind::Arc { sweep_ccw, .. }, EntityKind::Arc { sweep_ccw: n, .. }) => *sweep_ccw = n,
        _ => {}
    }
    if attr.is_some() {
        target.pad = attr;
    }
}
