//! Which sketch entities a pad OWNS.
//!
//! Ownership is spread across three `EditorPad` fields:
//! - `sketch_entity_id` — the centre `Point` carrying the `PadAttr`.
//! - `corner_entity_ids` — the four bbox-corner Points.
//! - `shape_params` — the per-shape sidecar ledger. RoundRect records
//!   its four corner Arcs (`corner_r_*_arc`), Oval its anchors /
//!   arc-centres / Lines / Arcs (`oval_*`), Chamfered its per-corner
//!   anchors (`chamfer_*_anchor*`).
//!
//! Any operation that acts on "the pad's geometry" has to consult all
//! three. Handling a subset is what stranded RoundRect anchors on a
//! move and leaked constraints / parameters on a delete — this module
//! exists so move and delete cannot drift apart again.

use std::collections::HashSet;

use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;

use super::super::state::EditorPad;

/// Every sketch entity `pad` owns, deduplicated, centre first.
///
/// Seeded from the three ownership fields, then expanded FORWARD once:
/// a seeded `Line` also owns its `start` / `end`, an `Arc` its
/// `center` / `start` / `end`, a `Circle` its `center`. One pass is
/// enough because Points are leaves, and forward expansion alone is
/// complete for every shape — RoundRect's four sidecar Arcs yield
/// exactly its 4 inset centres + 8 edge anchors, while Oval and
/// Chamfered record their anchors directly.
///
/// Deliberately NOT expanded in reverse: a user-drawn line that
/// happens to snap to a pad corner is not the pad's to drag around.
/// (Delete does sweep in reverse, but that is its own decision —
/// deleting a pad should take the dangling geometry with it.)
pub(super) fn owned_sketch_entities(pad: &EditorPad, sketch: &SketchData) -> Vec<SketchEntityId> {
    let mut seeds: Vec<SketchEntityId> = Vec::new();
    seeds.extend(pad.sketch_entity_id);
    if let Some(corners) = pad.corner_entity_ids {
        seeds.extend_from_slice(&corners);
    }
    // Sidecar values are bare UUID slugs. Canonical parameter bindings
    // (`corner_r_<slug>`, `diameter_<slug>`, …) carry an identifier
    // prefix, so they fail to parse and fall through.
    seeds.extend(
        pad.shape_params
            .values()
            .filter_map(|v| uuid::Uuid::parse_str(v).ok())
            .map(SketchEntityId),
    );

    let mut owned: Vec<SketchEntityId> = Vec::with_capacity(seeds.len());
    let mut seen: HashSet<SketchEntityId> = HashSet::new();
    for id in seeds {
        if seen.insert(id) {
            owned.push(id);
        }
        let Some(entity) = sketch.entities.iter().find(|e| e.id == id) else {
            continue;
        };
        let (a, b, c) = match &entity.kind {
            EntityKind::Line { start, end } => (Some(*start), Some(*end), None),
            EntityKind::Arc {
                center, start, end, ..
            } => (Some(*center), Some(*start), Some(*end)),
            EntityKind::Circle { center, .. } => (Some(*center), None, None),
            EntityKind::Point { .. } => continue,
        };
        for referenced in [a, b, c].into_iter().flatten() {
            if seen.insert(referenced) {
                owned.push(referenced);
            }
        }
    }
    owned
}
