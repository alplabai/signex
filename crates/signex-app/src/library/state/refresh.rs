//! Refreshing an open library's caches from its adapter.
//!
//! The read ([`LibrarySnapshot::read`]) is split from the apply
//! (`LibraryState::apply_snapshot`) so the read — `DiskStamp::of`,
//! `list_tables`, `table_pricing`, `read_table`, the primitive listings —
//! can run on the blocking pool while `update` only does the cheap
//! assignment (#1374). Each request takes a generation; a result whose
//! generation is no longer the latest for its library, or whose library
//! was closed meanwhile, is dropped by [`LibraryState::finish_refresh`].

use super::methods::{apply_primitive_listing, listings_complete};
use super::*;
use crate::library::refresh::RefreshFollowUp;

/// Everything a refresh reads from a library's adapter, not yet applied.
pub struct LibrarySnapshot {
    stamp: Option<signex_library::DiskStamp>,
    tables: HashMap<String, Vec<ComponentRow>>,
    summaries: Vec<ComponentSummary>,
    pricing: HashMap<Uuid, signex_library::RowPricing>,
    classes: Vec<signex_library::ClassEntry>,
    symbols: Result<Vec<PrimitiveSummary>, LibraryError>,
    symbols_partial: bool,
    footprints: Result<Vec<PrimitiveSummary>, LibraryError>,
    sims: Result<Vec<PrimitiveSummary>, LibraryError>,
}

/// What [`LibraryState::finish_refresh`] decided.
pub struct RefreshFinish {
    /// `None` when the read was superseded (or the library closed) and
    /// nothing was applied.
    pub applied: Option<Result<(), LibraryError>>,
    /// Follow-ups to run now; they follow a read that failed or was
    /// superseded only when the caches are known fresh anyway.
    pub follow_ups: Vec<RefreshFollowUp>,
}

/// What a caller needs to run a refresh off the UI thread.
pub struct RefreshStart {
    /// Identifies this request; hand it back to
    /// [`LibraryState::finish_refresh`].
    pub generation: u64,
    /// The mounted adapter itself, so the read sees what a synchronous
    /// refresh would see.
    pub adapter: Arc<dyn LibraryAdapter>,
}

impl LibrarySnapshot {
    /// Read the library at `root` through `adapter`. Pure with respect to
    /// app state; safe on any thread.
    pub fn read(adapter: &dyn LibraryAdapter, root: &Path) -> Result<Self, LibraryError> {
        let stamp = signex_library::DiskStamp::of(root).ok();
        let names = adapter.list_tables()?;
        let mut tables: HashMap<String, Vec<ComponentRow>> = HashMap::new();
        let mut summaries: Vec<ComponentSummary> = Vec::new();
        let mut pricing: HashMap<Uuid, signex_library::RowPricing> = HashMap::new();
        for name in names {
            match adapter.table_pricing(&name) {
                Ok(cached) => pricing.extend(cached),
                // To the Messages panel, not only the log: prices silently
                // missing from the browser look like "never refreshed".
                Err(e) => crate::diagnostics::log_warning(format!(
                    "Library {root:?} table {name}: cached prices could not be read ({e}); none are shown.",
                    root = root.display().to_string()
                )),
            }
            match adapter.read_table(&name) {
                Ok(rows) => {
                    for row in &rows {
                        summaries.push(ComponentSummary {
                            row_id: row.row_id,
                            internal_pn: row.internal_pn.clone(),
                            mpn: row.primary_mpn.mpn.clone(),
                            state: row.state,
                            description: String::new(),
                        });
                    }
                    tables.insert(name, rows);
                }
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        table = %name,
                        error = %e,
                        "refresh_components: read_table failed; entry left empty"
                    );
                    tables.insert(name, Vec::new());
                }
            }
        }
        Ok(Self {
            stamp,
            tables,
            summaries,
            pricing,
            symbols: adapter.list_symbols(),
            symbols_partial: adapter.symbol_listing_is_partial(),
            footprints: adapter.list_footprints(),
            sims: adapter.list_sims(),
            classes: adapter.library_classes(),
        })
    }
}

impl LibraryState {
    /// Refresh the cached table contents for a library — re-reads every
    /// TSV via the mounted adapter's `list_tables` + `read_table`, on the
    /// calling thread.
    ///
    /// For callers that need the caches current before they return. An
    /// `update` handler should use [`Self::begin_refresh`] instead.
    ///
    /// Returns `LibraryError::NotFound` when the library at `root`
    /// isn't mounted, otherwise the underlying adapter error. The
    /// cached `Vec<ComponentSummary>` is rebuilt alongside `tables`
    /// so the picker (summary tier) and the panel grid (row tier)
    /// stay coherent.
    pub fn refresh_components(&mut self, root: &Path) -> Result<(), LibraryError> {
        let adapter = self.adapter_at(root)?;
        let snapshot = LibrarySnapshot::read(adapter, root)?;
        self.supersede_refreshes(root);
        self.apply_snapshot(root, snapshot);
        Ok(())
    }

    /// Start an off-thread refresh of the library at `root`: records a new
    /// generation and hands back the adapter to read through. The caller
    /// runs [`LibrarySnapshot::read`] on the blocking pool and sends the
    /// result to [`Self::finish_refresh`].
    ///
    /// `follow_up` is kept per library until a read lands (the latest one,
    /// or the last one still in flight after a synchronous refresh
    /// superseded it), so a request that a newer one supersedes still gets
    /// its follow-up run.
    pub fn begin_refresh(
        &mut self,
        root: &Path,
        follow_up: RefreshFollowUp,
    ) -> Result<RefreshStart, LibraryError> {
        let library_id = self
            .library_at(root)
            .map(|lib| lib.library_id)
            .ok_or_else(|| LibraryError::NotFound(root.display().to_string()))?;
        let adapter = self
            .set
            .get_shared(library_id)
            .ok_or_else(|| LibraryError::NotFound(root.display().to_string()))?;
        if follow_up != RefreshFollowUp::Nothing {
            self.pending_follow_ups
                .entry(root.to_path_buf())
                .or_default()
                .push(follow_up);
        }
        *self
            .refresh_in_flight
            .entry(root.to_path_buf())
            .or_default() += 1;
        Ok(RefreshStart {
            generation: self.next_refresh_generation(root),
            adapter,
        })
    }

    /// An off-thread refresh of `root` has been started and has not
    /// finished. The external-edit poll leaves such a library alone: its
    /// caches are about to change, and a write the app just made would
    /// otherwise look like an outside edit.
    pub fn refresh_in_flight(&self, root: &Path) -> bool {
        self.refresh_in_flight.contains_key(root)
    }

    /// Make every off-thread read of `root` that is still running stale,
    /// because the caches were just filled (or are about to be) by other
    /// means. Their follow-ups stay pending and run when the last of those
    /// reads reports in.
    pub fn supersede_refreshes(&mut self, root: &Path) {
        self.next_refresh_generation(root);
    }

    /// Account for one finished off-thread read and apply it if it is
    /// still the latest. A read that was superseded changes nothing.
    ///
    /// The returned follow-ups are the ones to run now: all pending ones
    /// when this read was the latest, or, once the last read of a library
    /// that a synchronous refresh superseded has reported in, those too
    /// (the caches are fresh by then). Otherwise they wait for the read
    /// that will land.
    pub fn finish_refresh(
        &mut self,
        root: &Path,
        generation: u64,
        snapshot: Result<LibrarySnapshot, LibraryError>,
    ) -> RefreshFinish {
        let last_in_flight = match self.refresh_in_flight.get_mut(root) {
            Some(count) if *count > 1 => {
                *count -= 1;
                false
            }
            _ => {
                self.refresh_in_flight.remove(root);
                true
            }
        };
        let mounted = self.library_at(root).is_some();
        let latest = mounted && self.refresh_generations.get(root) == Some(&generation);
        let applied = latest.then(|| snapshot.map(|snapshot| self.apply_snapshot(root, snapshot)));
        let follow_ups = if mounted && (latest || last_in_flight) {
            self.pending_follow_ups.remove(root).unwrap_or_default()
        } else {
            Vec::new()
        };
        RefreshFinish {
            applied,
            follow_ups,
        }
    }

    /// Show a row the adapter just saved, ahead of the re-read that is on its
    /// way: the cached row is replaced and carries the new base `revision`.
    /// A row or table the cache does not hold is left alone — the re-read
    /// brings it.
    pub fn patch_cached_row(
        &mut self,
        root: &Path,
        table: &str,
        mut row: ComponentRow,
        revision: Option<u64>,
    ) {
        row.server_revision = revision;
        self.touch_data();
        let cached = self
            .library_at_mut(root)
            .and_then(|lib| lib.tables.get_mut(table))
            .and_then(|rows| rows.iter_mut().find(|r| r.row_id == row.row_id));
        if let Some(slot) = cached {
            *slot = row;
        }
    }

    fn adapter_at(&self, root: &Path) -> Result<&dyn LibraryAdapter, LibraryError> {
        let not_found = || LibraryError::NotFound(root.display().to_string());
        let library_id = self.library_at(root).ok_or_else(not_found)?.library_id;
        self.set.get(library_id).ok_or_else(not_found)
    }

    fn next_refresh_generation(&mut self, root: &Path) -> u64 {
        self.refresh_generation_counter += 1;
        let generation = self.refresh_generation_counter;
        self.refresh_generations
            .insert(root.to_path_buf(), generation);
        generation
    }

    fn apply_snapshot(&mut self, root: &Path, snapshot: LibrarySnapshot) {
        self.touch_data();
        self.clock = chrono::Utc::now();
        if let Some(lib) = self.library_at_mut(root) {
            lib.tables = snapshot.tables;
            lib.pricing = snapshot.pricing;
            lib.cached_components = snapshot.summaries;
            lib.cached_classes = snapshot.classes;
            lib.observed_stamp = snapshot.stamp;
            lib.own_edit_stamp = None;
            // A failed listing keeps the previous cache and warns —
            // blanking it here would render a mounted library as
            // empty. See `apply_primitive_listing`.
            let name = lib.display_name.clone();
            let ok = [
                apply_primitive_listing(
                    &mut lib.cached_symbols,
                    snapshot.symbols,
                    &name,
                    "symbols",
                ),
                apply_primitive_listing(
                    &mut lib.cached_footprints,
                    snapshot.footprints,
                    &name,
                    "footprints",
                ),
                apply_primitive_listing(&mut lib.cached_sims, snapshot.sims, &name, "sims"),
            ];
            lib.dangling.listings_complete = listings_complete(ok, snapshot.symbols_partial);
        }
        self.sweep_dangling_refs();
        // The library's symbols may have changed on disk.
        self.drift.check_pending = true;
    }
}
