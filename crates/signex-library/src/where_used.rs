//! Where-used reverse index — keyed by `RowId` for the DBLib model.
//!
//! Pure data structure. The consumer (signex-app) pushes references in via
//! `ingest_sheet` whenever a sheet is opened or saved, and drops a project's
//! entries via `drop_project` when the project closes. There is no filesystem
//! walking here — that is the consumer's job.
//!
//! Lookups by `RowId` return the list of every (project, sheet, instance)
//! site where the row is used. UI consumes the results.
//!
//! Storage shape: a `HashMap<PathBuf /* project */, HashMap<PathBuf /* sheet */, Vec<Entry>>>`.
//! Re-ingesting a sheet replaces (does not append) its previous entries — this
//! is what makes the index incremental and idempotent under repeated open/save.
//!
//! Per `v0.9-refactor-2-plan.md` §6 step 1.8, the index is keyed by
//! [`RowId`] (component-table row), not by the legacy `ComponentId`. The
//! `primitive_to_rows` reverse index is rebuilt by adapters via
//! `iter_rows()` — see [`Self::rebuild_from_rows`].

use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use crate::component::ComponentRow;
use crate::identity::RowId;
use crate::primitive::PrimitiveRef;

/// One occurrence of a row on a schematic sheet.
///
/// Per the v0.9-refactor-2 model, rows have no per-row version chain —
/// schematic instances reference their library row by `row_id` only. The
/// historical change log lives in `git log` (LocalGit) or the audit table
/// (Database) and is surfaced separately.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UseSite {
    /// Project root path (e.g. `/projects/alpha.snxprj`). Stored as-given;
    /// the consumer is responsible for any canonicalisation.
    pub project_path: PathBuf,
    /// Sheet path within the project (e.g. `/projects/alpha/main.snxsch`).
    pub sheet_path: PathBuf,
    /// Reference designator / instance id on the sheet (e.g. `R1`, `U3`).
    pub instance_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    row_id: RowId,
    instance_id: String,
}

/// Reverse index from row id → list of [`UseSite`].
///
/// Built incrementally — call [`ingest_sheet`](Self::ingest_sheet) when a sheet
/// is opened or saved, and [`drop_project`](Self::drop_project) when a project
/// is closed. Look up via [`where_used`](Self::where_used).
///
/// L3: SAFETY — this index is **not** `Sync`. The mutating API (`ingest_sheet`,
/// `drop_project`) takes `&mut self` and the read API (`where_used`) takes
/// `&self`, but the consumer is the UI thread which serialises every call
/// through the iced update loop. There is no need for interior mutability and
/// no plan to share the index across worker threads. The `PhantomData<Cell<()>>`
/// marker opts out of `Sync` so a future refactor can't accidentally hand the
/// index to a background thread without the compiler shouting first.
///
/// Stays `Send` so single-thread ownership transfer (e.g. moving into a
/// `tokio::task::spawn_blocking` closure) keeps working.
#[derive(Default)]
pub struct WhereUsedIndex {
    /// project → sheet → entries on that sheet.
    by_project: HashMap<PathBuf, HashMap<PathBuf, Vec<Entry>>>,
    /// `(library_id, primitive_uuid)` → list of rows referencing this
    /// primitive. Populated via [`Self::ingest_row`] /
    /// [`Self::rebuild_from_rows`]. Used by the
    /// "where-is-this-symbol-used" editor surfaces.
    primitive_to_rows: HashMap<PrimitiveRef, Vec<RowId>>,
    /// Inverse of `primitive_to_rows`: the primitives each row links, so
    /// replacing one row touches only its own links, not every bucket.
    row_to_primitives: HashMap<RowId, Vec<PrimitiveRef>>,
    /// `row_id` → the `(project, sheet)` pairs that hold at least one
    /// instance of it, so [`Self::where_used`] visits only those sheets
    /// instead of scanning every indexed project.
    row_to_sheets: HashMap<RowId, HashSet<(PathBuf, PathBuf)>>,
    /// `Cell<()>` is `!Sync`, which propagates through `PhantomData`. No
    /// runtime cost; the field is zero-sized.
    _not_sync: PhantomData<std::cell::Cell<()>>,
}

impl WhereUsedIndex {
    /// Construct an empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace all entries for `sheet` under `project` with `refs`.
    ///
    /// Called when a sheet is opened, saved, or otherwise re-scanned by the
    /// consumer. Earlier entries for the same `(project, sheet)` are dropped
    /// before the new refs are inserted, so the index never accumulates stale
    /// duplicates from re-ingestion.
    pub fn ingest_sheet(&mut self, project: &Path, sheet: &Path, refs: &[(RowId, String)]) {
        let entries: Vec<Entry> = refs
            .iter()
            .map(|(row_id, instance_id)| Entry {
                row_id: *row_id,
                instance_id: instance_id.clone(),
            })
            .collect();

        let project_map = self.by_project.entry(project.to_path_buf()).or_default();
        let previous: Option<Vec<Entry>> = if entries.is_empty() {
            // Empty refs ⇒ sheet contains no library rows: still erase any
            // previous entries so the (project, sheet) bucket reflects truth.
            project_map.remove(sheet)
        } else {
            for entry in &entries {
                self.row_to_sheets
                    .entry(entry.row_id)
                    .or_default()
                    .insert((project.to_path_buf(), sheet.to_path_buf()));
            }
            let kept: HashSet<RowId> = entries.iter().map(|e| e.row_id).collect();
            project_map.insert(sheet.to_path_buf(), entries).map(|old| {
                old.into_iter()
                    .filter(|e| !kept.contains(&e.row_id))
                    .collect()
            })
        };
        for entry in previous.unwrap_or_default() {
            self.forget_sheet_row(entry.row_id, project, sheet);
        }
    }

    fn forget_sheet_row(&mut self, row_id: RowId, project: &Path, sheet: &Path) {
        if let Some(sheets) = self.row_to_sheets.get_mut(&row_id) {
            sheets.remove(&(project.to_path_buf(), sheet.to_path_buf()));
            if sheets.is_empty() {
                self.row_to_sheets.remove(&row_id);
            }
        }
    }

    /// Drop every entry for `project` (called on project close).
    ///
    /// No-op if the project is not currently indexed.
    pub fn drop_project(&mut self, project: &Path) {
        let Some(sheets) = self.by_project.remove(project) else {
            return;
        };
        for (sheet, entries) in sheets {
            for entry in entries {
                self.forget_sheet_row(entry.row_id, project, &sheet);
            }
        }
    }

    /// Rebuild the `primitive_to_rows` reverse index from a row scan
    /// (`(table_name, row)` tuples — table_name is currently unused but
    /// kept on the API so future per-table filters don't change the
    /// signature).
    ///
    /// Per the plan, every adapter exposes `iter_rows()` returning the same
    /// shape. Rebuild is called on adapter open and after any row write
    /// that the consumer hasn't otherwise tracked through `ingest_row`.
    pub fn rebuild_from_rows(&mut self, rows: &[(String, ComponentRow)]) {
        self.primitive_to_rows.clear();
        self.row_to_primitives.clear();
        for (_table, row) in rows {
            self.add_primitive_links(row);
        }
    }

    /// Replace the `primitive_to_rows` entries for a single row (idempotent).
    ///
    /// Used by adapters when a single row is saved or updated mid-session,
    /// so we don't have to re-scan the whole library.
    pub fn ingest_row(&mut self, row: &ComponentRow) {
        let row_id = RowId::from_uuid(row.row_id);
        // Drop any prior entries pointing back to this row id.
        for target in self.row_to_primitives.remove(&row_id).unwrap_or_default() {
            if let Some(rows) = self.primitive_to_rows.get_mut(&target) {
                rows.retain(|id| *id != row_id);
                if rows.is_empty() {
                    self.primitive_to_rows.remove(&target);
                }
            }
        }
        self.add_primitive_links(row);
    }

    fn add_primitive_links(&mut self, row: &ComponentRow) {
        let row_id = RowId::from_uuid(row.row_id);
        // An alternate symbol or footprint is a use of that primitive too.
        let symbols = std::iter::once(row.symbol_ref)
            .chain(row.alternate_symbols.iter().map(|a| a.symbol_ref));
        let footprints = row
            .footprint_ref
            .into_iter()
            .chain(row.alternate_footprints.iter().map(|a| a.footprint));
        let targets = symbols.chain(footprints).chain(row.sim_ref);
        for target in targets {
            let rows = self.primitive_to_rows.entry(target).or_default();
            if !rows.contains(&row_id) {
                rows.push(row_id);
                self.row_to_primitives
                    .entry(row_id)
                    .or_default()
                    .push(target);
            }
        }
    }

    /// All rows that reference the given primitive. Returned slice is empty
    /// when the primitive isn't referenced.
    pub fn rows_for_primitive(&self, r: &PrimitiveRef) -> &[RowId] {
        self.primitive_to_rows
            .get(r)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Find every site where `row_id` is used. Returned order is
    /// unspecified — callers should sort if they need determinism.
    pub fn where_used(&self, row_id: RowId) -> Vec<UseSite> {
        let mut sites = Vec::new();
        let Some(sheets) = self.row_to_sheets.get(&row_id) else {
            return sites;
        };
        for (project_path, sheet_path) in sheets {
            let Some(entries) = self
                .by_project
                .get(project_path)
                .and_then(|project| project.get(sheet_path))
            else {
                continue;
            };
            for entry in entries.iter().filter(|e| e.row_id == row_id) {
                sites.push(UseSite {
                    project_path: project_path.clone(),
                    sheet_path: sheet_path.clone(),
                    instance_id: entry.instance_id.clone(),
                });
            }
        }
        sites
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alternate_symbol::AlternateSymbol;
    use crate::component::{DatasheetRef, PinPadOverride, PlmReserved};
    use crate::identity::{ComponentClass, InternalPn};
    use crate::lifecycle::LifecycleState;
    use crate::manufacturer::ManufacturerPart;
    use crate::param::ParamMap;
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn _assert_send_not_sync() {
        fn is_send<T: Send>() {}
        is_send::<WhereUsedIndex>();
    }

    fn fixture_row(symbol: PrimitiveRef, footprint: Option<PrimitiveRef>) -> ComponentRow {
        let t = Utc.with_ymd_and_hms(2026, 4, 1, 12, 0, 0).unwrap();
        ComponentRow {
            row_id: Uuid::now_v7(),
            internal_pn: InternalPn::new("R_TEST"),
            class: ComponentClass::generic(),
            datasheet: DatasheetRef::url(""),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Released,
            symbol_ref: symbol,
            alternate_symbols: Vec::new(),
            footprint_ref: footprint,
            alternate_footprints: Vec::new(),
            sim_ref: None,
            pin_map_overrides: Vec::<PinPadOverride>::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "A"),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: t,
            updated: t,
            content_hash: [0u8; 32],
            server_revision: None,
        }
    }

    #[test]
    fn new_index_is_empty() {
        let idx = WhereUsedIndex::new();
        assert!(idx.where_used(RowId::new()).is_empty());
    }

    #[test]
    fn ingesting_empty_refs_clears_a_previous_sheet_entry() {
        let project = PathBuf::from("/p.snxprj");
        let sheet = PathBuf::from("/p/main.snxsch");
        let row_id = RowId::new();

        let mut idx = WhereUsedIndex::new();
        idx.ingest_sheet(&project, &sheet, &[(row_id, "R1".into())]);
        assert_eq!(idx.where_used(row_id).len(), 1);

        idx.ingest_sheet(&project, &sheet, &[]);
        assert_eq!(idx.where_used(row_id).len(), 0);
    }

    #[test]
    fn rows_for_primitive_returns_referencing_row() {
        let lib = Uuid::now_v7();
        let sym_ref = PrimitiveRef::new(lib, Uuid::now_v7());
        let fpt_ref = PrimitiveRef::new(lib, Uuid::now_v7());
        let row = fixture_row(sym_ref, Some(fpt_ref));
        let row_id = RowId::from_uuid(row.row_id);

        let mut idx = WhereUsedIndex::new();
        idx.ingest_row(&row);
        assert_eq!(idx.rows_for_primitive(&sym_ref), &[row_id]);
        assert_eq!(idx.rows_for_primitive(&fpt_ref), &[row_id]);
        let unknown = PrimitiveRef::new(lib, Uuid::now_v7());
        assert!(idx.rows_for_primitive(&unknown).is_empty());
    }

    #[test]
    fn alternate_symbol_counts_as_a_use_of_that_symbol() {
        let lib = Uuid::now_v7();
        let default_sym = PrimitiveRef::new(lib, Uuid::now_v7());
        let alt_sym = PrimitiveRef::new(lib, Uuid::now_v7());
        let mut row = fixture_row(default_sym, None);
        row.alternate_symbols
            .push(AlternateSymbol::new(alt_sym, "0.0.1", "IEC"));
        let row_id = RowId::from_uuid(row.row_id);

        let mut idx = WhereUsedIndex::new();
        idx.ingest_row(&row);
        assert_eq!(idx.rows_for_primitive(&alt_sym), &[row_id]);
        assert_eq!(idx.rows_for_primitive(&default_sym), &[row_id]);

        row.alternate_symbols.clear();
        idx.ingest_row(&row);
        assert!(idx.rows_for_primitive(&alt_sym).is_empty());
    }

    #[test]
    fn alternate_footprint_counts_as_a_use_of_that_footprint() {
        let lib = Uuid::now_v7();
        let sym = PrimitiveRef::new(lib, Uuid::now_v7());
        let default_fp = PrimitiveRef::new(lib, Uuid::now_v7());
        let alt_fp = PrimitiveRef::new(lib, Uuid::now_v7());
        let mut row = fixture_row(sym, Some(default_fp));
        row.alternate_footprints
            .push(crate::footprint_binding::FootprintBinding::new(alt_fp));
        let row_id = RowId::from_uuid(row.row_id);

        let mut idx = WhereUsedIndex::new();
        idx.ingest_row(&row);
        assert_eq!(idx.rows_for_primitive(&alt_fp), &[row_id]);
        assert_eq!(idx.rows_for_primitive(&default_fp), &[row_id]);

        row.alternate_footprints.clear();
        idx.ingest_row(&row);
        assert!(idx.rows_for_primitive(&alt_fp).is_empty());
    }

    #[test]
    fn rebuild_from_rows_replaces_state() {
        let lib = Uuid::now_v7();
        let old_sym = PrimitiveRef::new(lib, Uuid::now_v7());
        let new_sym = PrimitiveRef::new(lib, Uuid::now_v7());

        let mut idx = WhereUsedIndex::new();
        let row_a = fixture_row(old_sym, None);
        idx.rebuild_from_rows(&[("resistors".into(), row_a.clone())]);
        let row_a_id = RowId::from_uuid(row_a.row_id);
        assert_eq!(idx.rows_for_primitive(&old_sym), &[row_a_id]);

        // Now rebuild with a different row whose symbol is `new_sym`. After
        // rebuild, `old_sym` should no longer reference `row_a` — every prior
        // entry is wiped.
        let row_b = fixture_row(new_sym, None);
        idx.rebuild_from_rows(&[("resistors".into(), row_b.clone())]);
        assert!(idx.rows_for_primitive(&old_sym).is_empty());
        assert_eq!(
            idx.rows_for_primitive(&new_sym),
            &[RowId::from_uuid(row_b.row_id)]
        );
    }

    #[test]
    fn ingest_row_replaces_prior_primitive_links() {
        let lib = Uuid::now_v7();
        let old_sym = PrimitiveRef::new(lib, Uuid::now_v7());
        let new_sym = PrimitiveRef::new(lib, Uuid::now_v7());

        let mut row = fixture_row(old_sym, None);
        let mut idx = WhereUsedIndex::new();
        idx.ingest_row(&row);
        let row_id = RowId::from_uuid(row.row_id);
        assert_eq!(idx.rows_for_primitive(&old_sym), &[row_id]);

        // Mutate the row's symbol_ref and re-ingest: stale link must evict.
        row.symbol_ref = new_sym;
        idx.ingest_row(&row);
        assert!(idx.rows_for_primitive(&old_sym).is_empty());
        assert_eq!(idx.rows_for_primitive(&new_sym), &[row_id]);
    }

    #[test]
    fn reingesting_a_sheet_moves_a_row_between_sheets() {
        let project = PathBuf::from("/p.snxprj");
        let (sheet_a, sheet_b) = (PathBuf::from("/p/a.snxsch"), PathBuf::from("/p/b.snxsch"));
        let (kept, moved) = (RowId::new(), RowId::new());

        let mut idx = WhereUsedIndex::new();
        idx.ingest_sheet(
            &project,
            &sheet_a,
            &[(kept, "R1".into()), (moved, "R2".into())],
        );
        idx.ingest_sheet(&project, &sheet_a, &[(kept, "R1".into())]);
        idx.ingest_sheet(&project, &sheet_b, &[(moved, "R9".into())]);

        assert_eq!(idx.where_used(kept).len(), 1);
        let sites = idx.where_used(moved);
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].sheet_path, sheet_b);
        assert_eq!(sites[0].instance_id, "R9");
    }

    #[test]
    fn a_row_used_twice_on_one_sheet_reports_both_instances() {
        let (project, sheet) = (PathBuf::from("/p.snxprj"), PathBuf::from("/p/a.snxsch"));
        let row_id = RowId::new();
        let mut idx = WhereUsedIndex::new();
        idx.ingest_sheet(
            &project,
            &sheet,
            &[(row_id, "R1".into()), (row_id, "R2".into())],
        );
        let mut ids: Vec<String> = idx
            .where_used(row_id)
            .into_iter()
            .map(|s| s.instance_id)
            .collect();
        ids.sort();
        assert_eq!(ids, ["R1", "R2"]);
    }

    #[test]
    fn dropping_a_project_forgets_only_its_sites() {
        let (p1, p2) = (PathBuf::from("/1.snxprj"), PathBuf::from("/2.snxprj"));
        let sheet = PathBuf::from("/s.snxsch");
        let row_id = RowId::new();
        let mut idx = WhereUsedIndex::new();
        idx.ingest_sheet(&p1, &sheet, &[(row_id, "R1".into())]);
        idx.ingest_sheet(&p2, &sheet, &[(row_id, "R1".into())]);

        idx.drop_project(&p1);
        let sites = idx.where_used(row_id);
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].project_path, p2);
        idx.drop_project(&p2);
        assert!(idx.where_used(row_id).is_empty());
    }
}
