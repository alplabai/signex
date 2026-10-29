//! Surfacing component rows whose primitive bindings dangle (#968).
//!
//! A row can bind a symbol / footprint / sim that has since been
//! deleted, or live in a library that is no longer mounted. Nothing
//! used to say so: the row just rendered without its graphics. The
//! sweep here compares every open library's rows against the primitive
//! listings the app already caches, so it does no IO and is safe to run
//! on the `update` path after every mount or table refresh.
//!
//! Each newly dangling binding is reported to the Messages panel once;
//! the full current set stays on [`OpenLibrary::dangling`] so the
//! browser can mark the rows.

use std::collections::{HashMap, HashSet};

use signex_library::{DanglingRef, PrimitiveRef, RefSlot, dangling_row_refs};
use uuid::Uuid;

use super::state::LibraryState;

/// Most bindings reported individually per library per sweep; the rest
/// are counted in one trailing line so a wrecked library cannot flood
/// the Messages panel.
const MAX_REPORTED_PER_SWEEP: usize = 20;

/// Per-library result of the last sweep.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DanglingSweep {
    /// True when all three primitive listings were read successfully and
    /// no symbol container was skipped as unreadable. A failed or partial
    /// listing says nothing about what is missing, so such a library is
    /// not swept. False until the first listing has actually run.
    pub listings_complete: bool,
    /// Bindings that resolved to nothing at the last sweep.
    pub refs: Vec<DanglingRef>,
}

impl DanglingSweep {
    /// True when `row_id` has a dangling symbol or footprint binding —
    /// the two the browser shows as columns.
    pub fn marks_row(&self, row_id: Uuid, slot: RefSlot) -> bool {
        self.refs
            .iter()
            .any(|r| r.row_id == row_id && r.slot == slot)
    }
}

/// Primitive UUIDs a library is known to hold, per kind.
#[derive(Default)]
struct KnownPrimitives {
    symbols: HashSet<Uuid>,
    footprints: HashSet<Uuid>,
    sims: HashSet<Uuid>,
    /// Listings failed: the sets above are not authoritative.
    unknown: bool,
}

impl KnownPrimitives {
    fn holds(&self, slot: RefSlot, uuid: Uuid) -> bool {
        self.unknown
            || match slot {
                RefSlot::Symbol => self.symbols.contains(&uuid),
                RefSlot::Footprint | RefSlot::AlternateFootprint => self.footprints.contains(&uuid),
                RefSlot::Sim => self.sims.contains(&uuid),
            }
    }
}

impl LibraryState {
    /// Recompute every open library's dangling bindings and report the
    /// ones that were not dangling at the previous sweep.
    ///
    /// Deferred while a mount is still being prepared: a binding into a
    /// library that is mounting right now is not dangling, and reporting
    /// it would be a false alarm. The mount's completion sweeps again.
    pub fn sweep_dangling_refs(&mut self) {
        if !self.pending_mounts.is_empty() {
            return;
        }
        let known = self.known_primitives();
        let mounted_elsewhere: HashSet<Uuid> = self
            .set
            .library_ids()
            .filter(|id| !known.contains_key(id))
            .collect();
        let resolves = |slot: RefSlot, target: &PrimitiveRef| match known.get(&target.library_id) {
            Some(lib) => lib.holds(slot, target.uuid),
            // Mounted but not file-backed (no cached listing): cannot
            // be checked without IO, so assume it resolves.
            None => mounted_elsewhere.contains(&target.library_id),
        };
        for lib in &mut self.open_libraries {
            if !lib.dangling.listings_complete {
                continue;
            }
            let found = dangling_row_refs(
                lib.tables
                    .iter()
                    .map(|(t, rows)| (t.as_str(), rows.as_slice())),
                resolves,
            );
            let fresh: Vec<&DanglingRef> = found
                .iter()
                .filter(|r| !lib.dangling.refs.contains(r))
                .collect();
            report_new(&lib.display_name, &fresh);
            lib.dangling.refs = found;
        }
    }

    fn known_primitives(&self) -> HashMap<Uuid, KnownPrimitives> {
        let mut known: HashMap<Uuid, KnownPrimitives> = HashMap::new();
        for lib in &self.open_libraries {
            let entry = known.entry(lib.library_id).or_default();
            entry.unknown |= !lib.dangling.listings_complete;
            entry
                .symbols
                .extend(lib.cached_symbols.iter().map(|s| s.uuid));
            entry
                .footprints
                .extend(lib.cached_footprints.iter().map(|s| s.uuid));
            entry.sims.extend(lib.cached_sims.iter().map(|s| s.uuid));
        }
        known
    }
}

fn report_new(library: &str, fresh: &[&DanglingRef]) {
    for r in fresh.iter().take(MAX_REPORTED_PER_SWEEP) {
        tracing::warn!(
            target: "signex::library",
            "library {library}: row {} (table {}) binds {} {} in library {} \
             which cannot be found",
            r.internal_pn,
            r.table,
            r.slot.label(),
            r.target.uuid,
            r.target.library_id,
        );
    }
    if fresh.len() > MAX_REPORTED_PER_SWEEP {
        tracing::warn!(
            target: "signex::library",
            "library {library}: {} more rows bind primitives that cannot be found",
            fresh.len() - MAX_REPORTED_PER_SWEEP,
        );
    }
}
