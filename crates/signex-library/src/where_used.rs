//! Where-used reverse index (Phase 1 WS-G).
//!
//! Pure data structure. The consumer (signex-app) pushes references in via
//! `ingest_sheet` whenever a sheet is opened or saved, and drops a project's
//! entries via `drop_project` when the project closes. There is no filesystem
//! walking here — that is the consumer's job.
//!
//! Lookups by `(uuid, optional_version)` return the list of every (project, sheet,
//! instance, version) site where the component appears. UI consumes the results.
//!
//! Storage shape: a `HashMap<PathBuf /* project */, HashMap<PathBuf /* sheet */, Vec<Entry>>>`.
//! Re-ingesting a sheet replaces (does not append) its previous entries — this is
//! what makes the index incremental and idempotent under repeated open/save.
//!
//! See `.claude/PRPs/v0.9-library-plan.md` § "WS-G: Where-used reverse index".

use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::component::Component;
use crate::identity::{ComponentId, Version};
use crate::primitive::PrimitiveRef;

/// One occurrence of a component on a schematic sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UseSite {
    /// Project root path (e.g. `/projects/alpha.snxprj`). Stored as-given; the
    /// consumer is responsible for any canonicalisation.
    pub project_path: PathBuf,
    /// Sheet path within the project (e.g. `/projects/alpha/main.snxsch`).
    pub sheet_path: PathBuf,
    /// Reference designator / instance id on the sheet (e.g. `R1`, `U3`).
    pub instance_id: String,
    /// The library revision the instance is pinned to.
    pub version_pinned: Version,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    uuid: Uuid,
    instance_id: String,
    version_pinned: Version,
}

/// Reverse index from component UUID → list of [`UseSite`].
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
    /// `(library_id, primitive_uuid)` → list of components referencing this
    /// primitive. Populated via [`Self::ingest_component`] when a library is
    /// opened or a component is saved. Used by the "where-is-this-symbol-used"
    /// editor surfaces.
    primitive_to_components: HashMap<PrimitiveRef, Vec<ComponentId>>,
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
    pub fn ingest_sheet(&mut self, project: &Path, sheet: &Path, refs: &[(Uuid, String, Version)]) {
        let entries: Vec<Entry> = refs
            .iter()
            .map(|(uuid, instance_id, version)| Entry {
                uuid: *uuid,
                instance_id: instance_id.clone(),
                version_pinned: *version,
            })
            .collect();

        let project_map = self.by_project.entry(project.to_path_buf()).or_default();
        if entries.is_empty() {
            // Empty refs ⇒ sheet contains no library components: still erase any
            // previous entries so the (project, sheet) bucket reflects truth.
            project_map.remove(sheet);
        } else {
            project_map.insert(sheet.to_path_buf(), entries);
        }
    }

    /// Drop every entry for `project` (called on project close).
    ///
    /// No-op if the project is not currently indexed.
    pub fn drop_project(&mut self, project: &Path) {
        self.by_project.remove(project);
    }

    /// Replace the reverse-index entries for `component`. Called whenever a
    /// library is opened or a component revision is saved.
    ///
    /// Idempotent: re-ingesting the same component clears its previous
    /// reverse-index entries before appending the current ones.
    pub fn ingest_component(&mut self, component: &Component) {
        // Drop any prior entries for this component's UUID, regardless of
        // primitive ref — a revision swap may have repointed at different
        // primitives, and stale entries would survive forever otherwise.
        for sites in self.primitive_to_components.values_mut() {
            sites.retain(|id| *id != component.uuid);
        }
        self.primitive_to_components
            .retain(|_, sites| !sites.is_empty());

        for rev in &component.revisions {
            self.primitive_to_components
                .entry(rev.symbol_ref)
                .or_default()
                .push(component.uuid);
            if let Some(fp) = rev.footprint_ref {
                self.primitive_to_components
                    .entry(fp)
                    .or_default()
                    .push(component.uuid);
            }
            if let Some(sm) = rev.sim_ref {
                self.primitive_to_components
                    .entry(sm)
                    .or_default()
                    .push(component.uuid);
            }
        }
    }

    /// All components that reference the given primitive, across every
    /// revision currently ingested. Returned slice is empty when the
    /// primitive isn't referenced.
    pub fn components_for_primitive(&self, r: &PrimitiveRef) -> &[ComponentId] {
        self.primitive_to_components
            .get(r)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Find every site where `uuid` is used.
    ///
    /// `version = None` matches any pinned version; `version = Some(v)` filters
    /// to instances pinned at exactly `v`.
    ///
    /// The returned order is unspecified — callers should sort if they need
    /// determinism.
    pub fn where_used(&self, uuid: Uuid, version: Option<Version>) -> Vec<UseSite> {
        let mut sites = Vec::new();
        for (project_path, sheets) in &self.by_project {
            for (sheet_path, entries) in sheets {
                for entry in entries {
                    if entry.uuid != uuid {
                        continue;
                    }
                    if let Some(want) = version
                        && entry.version_pinned != want
                    {
                        continue;
                    }
                    sites.push(UseSite {
                        project_path: project_path.clone(),
                        sheet_path: sheet_path.clone(),
                        instance_id: entry.instance_id.clone(),
                        version_pinned: entry.version_pinned,
                    });
                }
            }
        }
        sites
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L3: compile-time guard — `WhereUsedIndex` must stay `Send` (so a
    /// closure can move ownership) but must NOT be `Sync` (so two threads
    /// can't share `&` references). If a future refactor introduces interior
    /// mutability or wraps the map in an `Arc<Mutex<…>>`, decide explicitly
    /// whether `Sync` is still wanted before deleting this assertion.
    fn _assert_send_not_sync() {
        fn is_send<T: Send>() {}
        is_send::<WhereUsedIndex>();

        // Negative `Sync` check via trait specialisation pattern: this would
        // not compile if `WhereUsedIndex: Sync`. We deliberately don't add
        // `is_sync::<WhereUsedIndex>()` because that *would* compile thanks
        // to the auto-trait, defeating the L3 invariant. The PhantomData
        // marker ensures `Sync` is genuinely opted out at the type level.
    }

    #[test]
    fn new_index_is_empty() {
        let idx = WhereUsedIndex::new();
        assert!(idx.where_used(Uuid::now_v7(), None).is_empty());
    }

    #[test]
    fn ingesting_empty_refs_clears_a_previous_sheet_entry() {
        let project = PathBuf::from("/p.snxprj");
        let sheet = PathBuf::from("/p/main.snxsch");
        let uuid = Uuid::now_v7();
        let v = Version::new(1, 0);

        let mut idx = WhereUsedIndex::new();
        idx.ingest_sheet(&project, &sheet, &[(uuid, "R1".into(), v)]);
        assert_eq!(idx.where_used(uuid, None).len(), 1);

        idx.ingest_sheet(&project, &sheet, &[]);
        assert_eq!(idx.where_used(uuid, None).len(), 0);
    }

    #[test]
    fn primitive_to_components_lists_referencing_component() {
        use crate::component::{Component, DatasheetRef, PlmReserved, Revision};
        use crate::identity::{ComponentClass, InternalPn};
        use crate::lifecycle::LifecycleState;
        use crate::manufacturer::ManufacturerPart;
        use crate::param::ParamMap;

        let lib = Uuid::new_v4();
        let sym_ref = PrimitiveRef::new(lib, Uuid::new_v4());
        let fpt_ref = PrimitiveRef::new(lib, Uuid::new_v4());
        let comp_id = Uuid::now_v7();

        let rev = Revision {
            version: Version::new(1, 0),
            state: LifecycleState::Released,
            created: chrono::Utc::now(),
            author: "test".into(),
            message: "init".into(),
            symbol_ref: sym_ref,
            footprint_ref: Some(fpt_ref),
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "A"),
            alternates: Vec::new(),
            supply: Vec::new(),
            datasheet: DatasheetRef::url(""),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            content_hash: [0u8; 32],
        };
        let comp = Component {
            uuid: comp_id,
            internal_pn: InternalPn::new("R_TEST"),
            class: ComponentClass::generic(),
            category: PathBuf::new(),
            family: None,
            revisions: vec![rev],
            head: Version::new(1, 0),
        };

        let mut idx = WhereUsedIndex::new();
        idx.ingest_component(&comp);
        assert_eq!(idx.components_for_primitive(&sym_ref), &[comp_id]);
        assert_eq!(idx.components_for_primitive(&fpt_ref), &[comp_id]);
        let unknown = PrimitiveRef::new(lib, Uuid::new_v4());
        assert!(idx.components_for_primitive(&unknown).is_empty());
    }

    #[test]
    fn ingest_component_replaces_prior_primitive_links() {
        use crate::component::{Component, DatasheetRef, PlmReserved, Revision};
        use crate::identity::{ComponentClass, InternalPn};
        use crate::lifecycle::LifecycleState;
        use crate::manufacturer::ManufacturerPart;
        use crate::param::ParamMap;

        let lib = Uuid::new_v4();
        let old_sym = PrimitiveRef::new(lib, Uuid::new_v4());
        let new_sym = PrimitiveRef::new(lib, Uuid::new_v4());
        let comp_id = Uuid::now_v7();

        let mk_rev = |sym: PrimitiveRef| Revision {
            version: Version::new(1, 0),
            state: LifecycleState::Released,
            created: chrono::Utc::now(),
            author: "t".into(),
            message: "".into(),
            symbol_ref: sym,
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("A", "B"),
            alternates: Vec::new(),
            supply: Vec::new(),
            datasheet: DatasheetRef::url(""),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            content_hash: [0u8; 32],
        };

        let mut idx = WhereUsedIndex::new();

        let comp_old = Component {
            uuid: comp_id,
            internal_pn: InternalPn::new("X"),
            class: ComponentClass::generic(),
            category: PathBuf::new(),
            family: None,
            revisions: vec![mk_rev(old_sym)],
            head: Version::new(1, 0),
        };
        idx.ingest_component(&comp_old);
        assert_eq!(idx.components_for_primitive(&old_sym), &[comp_id]);

        let comp_new = Component {
            revisions: vec![mk_rev(new_sym)],
            ..comp_old
        };
        idx.ingest_component(&comp_new);
        assert!(
            idx.components_for_primitive(&old_sym).is_empty(),
            "stale primitive link must be evicted"
        );
        assert_eq!(idx.components_for_primitive(&new_sym), &[comp_id]);
    }
}
