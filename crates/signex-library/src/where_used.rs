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
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::identity::Version;

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
#[derive(Default)]
pub struct WhereUsedIndex {
    /// project → sheet → entries on that sheet.
    by_project: HashMap<PathBuf, HashMap<PathBuf, Vec<Entry>>>,
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
}
