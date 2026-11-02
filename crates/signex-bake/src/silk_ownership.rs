//! Which silk entries a sketch bake owns.
//!
//! A silk entry the bake made carries the sketch entity it came from
//! (`FpGraphic::sketch_entity`); an entry without one was drawn by hand.
//! A re-bake replaces the marked entries and keeps every hand-drawn one.
//! Files written before the marker existed are classified once on open by
//! [`recover_legacy_silk`]. See `docs/FOOTPRINT_SKETCH.md`, "Bake ownership".

use std::collections::{HashMap, HashSet};

use signex_library::primitive::footprint::{Footprint, FpGraphic, FpGraphicKind};
use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::id::SketchEntityId;
use signex_sketch::solver::Solver;
use signex_types::format::mm_as_stored_nm;

use crate::silk::bake_silk;

/// Where each entry of a silk layer went in a merge: `remap[i]` is the new
/// index of old entry `i`, or `None` when the merge removed it.
pub type SilkRemap = Vec<Option<usize>>;

/// The remaps of both silk layers after [`settle_baked_silk`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SilkRemaps {
    pub silk_f: SilkRemap,
    pub silk_b: SilkRemap,
}

/// Replace the baked entries of `layer` with `baked`, then append every
/// hand-drawn entry in its existing order. Removal is by marker presence:
/// a marked entry goes whether or not its entity still exists. A marked
/// entry is remapped to the new entry baked from the same entity, if any.
pub fn merge_silk_layer(layer: &mut Vec<FpGraphic>, baked: Vec<FpGraphic>) -> SilkRemap {
    let old = std::mem::replace(layer, baked);
    let from_entity: HashMap<SketchEntityId, usize> = layer
        .iter()
        .enumerate()
        .filter_map(|(i, g)| g.sketch_entity.map(|id| (id, i)))
        .collect();
    let mut remap = Vec::with_capacity(old.len());
    for graphic in old {
        match graphic.sketch_entity {
            Some(id) => remap.push(from_entity.get(&id).copied()),
            None => {
                remap.push(Some(layer.len()));
                layer.push(graphic);
            }
        }
    }
    remap
}

/// Merge one silk bake (possibly empty) into both layers of `fp`. Silk is
/// no longer tracked in `baked_kinds`: a token an older build recorded is
/// dropped here, since the markers now say what the sketch owns.
pub fn settle_baked_silk(
    fp: &mut Footprint,
    silk_f: Vec<FpGraphic>,
    silk_b: Vec<FpGraphic>,
) -> SilkRemaps {
    let remaps = SilkRemaps {
        silk_f: merge_silk_layer(&mut fp.silk_f, silk_f),
        silk_b: merge_silk_layer(&mut fp.silk_b, silk_b),
    };
    if let Some(sketch) = fp.sketch.as_mut() {
        sketch.record_baked(BakedKind::SilkF, false);
        sketch.record_baked(BakedKind::SilkB, false);
    }
    remaps
}

/// Whether either silk layer holds an entry a bake made.
pub fn has_baked_silk(fp: &Footprint) -> bool {
    fp.silk_f.iter().chain(&fp.silk_b).any(FpGraphic::is_baked)
}

/// What [`recover_legacy_silk`] did.
#[derive(Clone, Debug, PartialEq)]
pub enum LegacySilk {
    /// Nothing to classify: no sketch silk tag with unmarked entries.
    NotNeeded,
    /// The sketch solved; this many unmarked entries matched a bake output
    /// and are now marked. The rest stay hand-drawn.
    Marked(usize),
    /// The sketch did not solve, so nothing was marked or removed. A later
    /// bake adds its output beside the old entries, which may duplicate them.
    Unsolved(String),
}

/// Whether `fp` needs [`recover_legacy_silk`]: its sketch tags a silk layer
/// that holds unmarked entries.
pub fn needs_legacy_silk_recovery(fp: &Footprint) -> bool {
    let Some(sketch) = fp.sketch.as_ref() else {
        return false;
    };
    let unmarked = |layer: &[FpGraphic]| layer.iter().any(|g| !g.is_baked());
    (sketch.tags_kind(BakedKind::SilkF) && unmarked(&fp.silk_f))
        || (sketch.tags_kind(BakedKind::SilkB) && unmarked(&fp.silk_b))
}

/// Mark the silk entries an older build baked. Solves the sketch as it is
/// (it must not have been edited since it was loaded), bakes its silk, and
/// marks each unmarked entry that matches an unclaimed output of the same
/// layer: same kind, stroke width and fill, and geometry equal at the
/// file's nanometre resolution. Each output is used at most once; an entry
/// that matches nothing is hand-drawn and stays unmarked.
pub fn recover_legacy_silk(fp: &mut Footprint, solver: &Solver) -> LegacySilk {
    if !needs_legacy_silk_recovery(fp) {
        return LegacySilk::NotNeeded;
    }
    let Some(sketch) = fp.sketch.as_ref() else {
        return LegacySilk::NotNeeded;
    };
    let resolved = match signex_sketch::parameter::resolve(&sketch.parameters) {
        Ok(resolved) => resolved,
        Err(e) => return LegacySilk::Unsolved(e.to_string()),
    };
    let solved = match solver.solve(sketch, &resolved) {
        Ok(solved) => solved,
        Err(e) => return LegacySilk::Unsolved(e.to_string()),
    };
    let (mut silk_f, mut silk_b, mut warnings) = (Vec::new(), Vec::new(), Vec::new());
    if let Err(e) = bake_silk(sketch, &solved, &mut silk_f, &mut silk_b, &mut warnings) {
        return LegacySilk::Unsolved(e.to_string());
    }
    LegacySilk::Marked(
        mark_matching(&mut fp.silk_f, silk_f) + mark_matching(&mut fp.silk_b, silk_b),
    )
}

/// Mark the unmarked entries of `layer` that match an output of `baked`
/// not already claimed by a marked entry. Returns how many were marked.
fn mark_matching(layer: &mut [FpGraphic], baked: Vec<FpGraphic>) -> usize {
    let claimed: HashSet<SketchEntityId> = layer.iter().filter_map(|g| g.sketch_entity).collect();
    let mut pool: Vec<Option<FpGraphic>> = baked
        .into_iter()
        .filter(|g| g.sketch_entity.is_some_and(|id| !claimed.contains(&id)))
        .map(Some)
        .collect();
    let mut marked = 0;
    for entry in layer.iter_mut().filter(|g| !g.is_baked()) {
        let slot = pool
            .iter_mut()
            .find(|slot| slot.as_ref().is_some_and(|out| same_graphic(entry, out)));
        if let Some(out) = slot.and_then(Option::take) {
            entry.sketch_entity = out.sketch_entity;
            marked += 1;
        }
    }
    marked
}

/// Equal as stored: the one comparison the file format defines for lengths
/// (`mm_as_stored_nm`, integer nanometres).
fn same_mm(a: f64, b: f64) -> bool {
    matches!((mm_as_stored_nm(a), mm_as_stored_nm(b)), (Some(x), Some(y)) if x == y)
}

fn same_point(a: [f64; 2], b: [f64; 2]) -> bool {
    same_mm(a[0], b[0]) && same_mm(a[1], b[1])
}

/// The point at `deg` on the circle, so arc angles compare as positions.
fn on_circle(center: [f64; 2], radius: f64, deg: f64) -> [f64; 2] {
    let rad = deg.to_radians();
    [
        center[0] + radius * rad.cos(),
        center[1] + radius * rad.sin(),
    ]
}

fn same_graphic(a: &FpGraphic, b: &FpGraphic) -> bool {
    if a.filled != b.filled || !same_mm(a.stroke_width, b.stroke_width) {
        return false;
    }
    match (&a.kind, &b.kind) {
        (
            FpGraphicKind::Line { from, to },
            FpGraphicKind::Line {
                from: b_from,
                to: b_to,
            },
        ) => same_point(*from, *b_from) && same_point(*to, *b_to),
        (
            FpGraphicKind::Circle { center, radius },
            FpGraphicKind::Circle {
                center: b_center,
                radius: b_radius,
            },
        ) => same_point(*center, *b_center) && same_mm(*radius, *b_radius),
        (
            FpGraphicKind::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            },
            FpGraphicKind::Arc {
                center: bc,
                radius: br,
                start_deg: bs,
                end_deg: be,
            },
        ) => {
            same_point(*center, *bc)
                && same_mm(*radius, *br)
                && same_point(
                    on_circle(*center, *radius, *start_deg),
                    on_circle(*bc, *br, *bs),
                )
                && same_point(
                    on_circle(*center, *radius, *end_deg),
                    on_circle(*bc, *br, *be),
                )
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "silk_ownership_tests.rs"]
mod tests;
