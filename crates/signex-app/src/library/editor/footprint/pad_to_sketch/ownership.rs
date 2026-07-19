//! Which sketch entities a pad OWNS.
//!
//! The DURABLE answer is [`signex_sketch::attr::PadAttr::owned`], a
//! ledger written onto the centre Point's `PadAttr` at mint time by
//! [`record_ledger`]. It is the only one of the four ownership records
//! that survives a save + reopen.
//!
//! The other three live on `EditorPad` and are session-volatile —
//! `EditorPad::from_pad` sets all of them to `None` / empty because
//! they have no home on `Pad`:
//! - `sketch_entity_id` — the centre `Point` carrying the `PadAttr`.
//! - `corner_entity_ids` — the four bbox-corner Points.
//! - `shape_params` — the per-shape sidecar ledger. RoundRect records
//!   its four corner Arcs (`corner_r_*_arc`), Oval its anchors /
//!   arc-centres / Lines / Arcs (`oval_*`), Chamfered its per-corner
//!   anchors (`chamfer_*_anchor*`).
//!
//! They are still consulted, because a pad written before the durable
//! ledger existed has an empty `PadAttr::owned` and the volatile fields
//! are all it has within the minting session.
//!
//! Any operation that acts on "the pad's geometry" has to consult all
//! four. Handling a subset is what stranded RoundRect anchors on a
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
    // The durable ledger first — it is the whole answer for a pad that
    // came back from disk, where the three fields below are empty.
    if let Some(centre) = pad.sketch_entity_id {
        seeds.extend(persisted_ledger(sketch, centre));
    }
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
        // Existence gates the push, not just the expansion. A seed that
        // names no live entity is not owned geometry, and letting it
        // through put a dead UUID into the delete drop set — where it is
        // substring-matched against every constraint's `Debug` rendering
        // and would take unrelated constraints with it.
        let Some(entity) = sketch.entities.iter().find(|e| e.id == id) else {
            continue;
        };
        if seen.insert(id) {
            owned.push(id);
        }
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

/// The durable ledger carried by `centre`'s `PadAttr`. Empty when the
/// entity is gone, carries no `PadAttr`, or predates the field.
fn persisted_ledger(sketch: &SketchData, centre: SketchEntityId) -> Vec<SketchEntityId> {
    sketch
        .entities
        .iter()
        .find(|e| e.id == centre)
        .and_then(|e| e.pad.as_ref())
        .map(|attr| attr.owned.clone())
        .unwrap_or_default()
}

/// Write the pad's current owned set onto its centre `PadAttr`, so the
/// answer survives the save + reopen that wipes every `EditorPad`
/// ownership field.
///
/// Called at the end of minting, which is the only moment the full set
/// is known from the volatile fields, and it is the ONLY write site —
/// nothing re-records afterwards. That is sound only because nothing
/// re-mints afterwards either: changing a pad's shape
/// (`fp_editor_set_selected_pad_shape`) writes `pad.shape` and never
/// calls back into `pad_to_sketch`, so the sketch still holds exactly
/// the geometry this ledger names. A delete drops the centre `PadAttr`
/// and the ledger with it.
///
/// If a re-mint path is ever added — a shape change that regenerates
/// geometry, say — it MUST route through `mint_shape_geometry_for` (or
/// call this) or the ledger will name entities that no longer exist
/// while the new ones are unowned.
pub(super) fn record_ledger(sketch: &mut SketchData, pad: &EditorPad, centre: SketchEntityId) {
    let owned = owned_sketch_entities(pad, sketch);
    if let Some(attr) = sketch
        .entities
        .iter_mut()
        .find(|e| e.id == centre)
        .and_then(|e| e.pad.as_mut())
    {
        attr.owned = owned;
    }
}
