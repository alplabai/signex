//! Which project owns a sheet path, answered from a cached index (#1470).
//!
//! `DocumentState::document_project` runs from many UI-thread call sites and
//! used to rebuild everything per call: the child-reference map cloned out of
//! every engine, then a normalised, case-folded key for every reference and
//! every listed sheet. [`OwnershipIndex`] holds those keys; the owning
//! `DocumentState` keeps one and rebuilds it only when its stamp
//! ([`ownership_stamp`]) moves.
//!
//! The stamp is a hash of exactly what the index is built from — each
//! project's id, file, case rules and listed sheets, and each open engine's
//! path and child-sheet filenames — so the index cannot go stale whichever
//! code path edited the data (a rename, a sheet added or removed, a project
//! closed, an engine swapped in without bumping an epoch). Hashing the inputs
//! costs one allocation-free pass; the keys it spares are the expensive part.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use super::scope::{CaseFold, parent_of, path_key_with};
use super::{DocumentState, LoadedProject};

/// Listed-sheet keys per project and `child → parent` keys per case rule.
#[derive(Debug)]
pub(crate) struct OwnershipIndex {
    stamp: u64,
    /// Per project, in `projects` order: its case rule and the keys of the
    /// sheets its `.snxprj` lists.
    listed: Vec<(CaseFold, HashSet<String>)>,
    /// `child key → parent path`, once per distinct case rule.
    parents: Vec<(CaseFold, HashMap<String, PathBuf>)>,
}

impl OwnershipIndex {
    /// Build the index. `fold_override` compares every project under one rule
    /// (tests); `None` compares each under its own probed rule (#694).
    pub(crate) fn build(
        stamp: u64,
        projects: &[LoadedProject],
        loaded: &HashMap<PathBuf, Vec<String>>,
        fold_override: Option<CaseFold>,
    ) -> Self {
        let fold_of = |p: &LoadedProject| fold_override.unwrap_or(p.case_fold);
        let mut folds: Vec<CaseFold> = Vec::new();
        for project in projects {
            let fold = fold_of(project);
            if !folds.contains(&fold) {
                folds.push(fold);
            }
        }
        let listed = projects
            .iter()
            .map(|p| {
                let fold = fold_of(p);
                let dir = p.dir();
                let keys = p
                    .data
                    .sheets
                    .iter()
                    .map(|s| path_key_with(&dir.join(&s.filename), fold))
                    .collect();
                (fold, keys)
            })
            .collect();
        let parents = folds
            .into_iter()
            .map(|fold| (fold, parent_of(loaded, fold)))
            .collect();
        Self {
            stamp,
            listed,
            parents,
        }
    }

    /// The stamp this index was built under.
    pub(crate) fn stamp(&self) -> u64 {
        self.stamp
    }

    /// The first project (in order) whose case rule passes `include` and
    /// which lists `path`.
    fn listing<'a>(
        &self,
        projects: &'a [LoadedProject],
        path: &Path,
        include: impl Fn(CaseFold) -> bool,
    ) -> Option<&'a LoadedProject> {
        let mut keys: Vec<(CaseFold, String)> = Vec::new();
        projects
            .iter()
            .zip(&self.listed)
            .find_map(|(p, (fold, set))| {
                if !include(*fold) {
                    return None;
                }
                let at = match keys.iter().position(|(f, _)| f == fold) {
                    Some(at) => at,
                    None => {
                        keys.push((*fold, path_key_with(path, *fold)));
                        keys.len() - 1
                    }
                };
                set.contains(&keys[at].1).then_some(p)
            })
    }

    /// Project that owns `path`, directly or as a hierarchical descendant of
    /// one of its sheets. A direct listing in ANY project beats a hierarchical
    /// claim by another. Otherwise the hierarchy is walked once per distinct
    /// rule: a parent found under rule F is accepted only when the project
    /// listing it also has rule F, so a folding map can never attribute a
    /// different file to a case-sensitive project. Cycles terminate on the
    /// visited set.
    pub(crate) fn owner<'a>(
        &self,
        projects: &'a [LoadedProject],
        path: &Path,
    ) -> Option<&'a LoadedProject> {
        if let Some(project) = self.listing(projects, path, |_| true) {
            return Some(project);
        }
        self.parents.iter().find_map(|(fold, parents)| {
            let mut seen: HashSet<PathBuf> = HashSet::new();
            let mut current = path.to_path_buf();
            while seen.insert(current.clone()) {
                let parent = parents.get(&path_key_with(&current, *fold))?;
                if let Some(project) = self.listing(projects, parent, |f| f == *fold) {
                    return Some(project);
                }
                current = parent.clone();
            }
            None
        })
    }
}

impl DocumentState {
    /// Project that owns the document at `path` (the explicit-path form of
    /// [`active_document_project`](Self::active_document_project), for async
    /// completions that resolve against the document they started from).
    pub fn document_project(&self, path: &std::path::Path) -> Option<&LoadedProject> {
        self.ownership_index().owner(&self.projects, path)
    }

    /// The ownership index for the current projects and engines, rebuilt only
    /// when the data it is built from changed (compared by content stamp, so
    /// no edit path has to remember to invalidate it).
    pub(crate) fn ownership_index(&self) -> std::cell::Ref<'_, OwnershipIndex> {
        let stamp = ownership_stamp(&self.projects, &self.engines);
        let fresh = self
            .ownership
            .borrow()
            .as_ref()
            .is_some_and(|index| index.stamp() == stamp);
        if !fresh {
            *self.ownership.borrow_mut() = Some(OwnershipIndex::build(
                stamp,
                &self.projects,
                &self.child_sheet_refs(),
                None,
            ));
        }
        std::cell::Ref::map(self.ownership.borrow(), |slot| {
            slot.as_ref().expect("built above")
        })
    }

    /// Open engine paths that belong to no loaded project — the page set of a
    /// loose-document export. An open tab belonging to some *other* loaded
    /// project would otherwise ride along as an extra page.
    ///
    /// Sorted by path: `engines` is a `HashMap`, so an unsorted result orders
    /// the loose export's pages (and the print-preview file-picker rows, which
    /// are re-seeded from this set on every rerasterize) by hash iteration
    /// order — visibly reshuffling between rerasterizes with two or more loose
    /// schematics open. Callers that want a specific page first re-order after.
    pub fn unowned_engine_paths(&self) -> Vec<PathBuf> {
        let index = self.ownership_index();
        let mut paths: Vec<PathBuf> = self
            .engines
            .keys()
            .filter(|p| index.owner(&self.projects, p).is_none())
            .cloned()
            .collect();
        paths.sort();
        paths
    }

    /// `sheet path → the `filename` strings it references as child sheets`,
    /// over every loaded engine. Input to the hierarchy walk in
    /// [`active_document_project`](Self::active_document_project).
    pub(crate) fn child_sheet_refs(&self) -> std::collections::HashMap<PathBuf, Vec<String>> {
        self.engines
            .iter()
            .map(|(path, engine)| {
                let refs = engine
                    .document()
                    .child_sheets
                    .iter()
                    .map(|cs| cs.filename.clone())
                    .collect();
                (path.clone(), refs)
            })
            .collect()
    }
}

/// Hash of everything an [`OwnershipIndex`] is built from. Engine order is
/// random (`HashMap`), so per-engine hashes are summed, not chained.
pub(crate) fn ownership_stamp(
    projects: &[LoadedProject],
    engines: &HashMap<PathBuf, signex_engine::Engine>,
) -> u64 {
    let mut h = std::hash::DefaultHasher::new();
    projects.len().hash(&mut h);
    for p in projects {
        p.id.hash(&mut h);
        p.path.hash(&mut h);
        p.case_fold.hash(&mut h);
        for s in &p.data.sheets {
            s.filename.hash(&mut h);
        }
        // Delimit this project's sheet list from the next project's fields.
        p.data.sheets.len().hash(&mut h);
    }
    let engines_hash = engines.iter().fold(0u64, |acc, (path, engine)| {
        let mut e = std::hash::DefaultHasher::new();
        path.hash(&mut e);
        for cs in &engine.document().child_sheets {
            cs.filename.hash(&mut e);
        }
        acc.wrapping_add(e.finish())
    });
    engines_hash.hash(&mut h);
    engines.len().hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use signex_types::project::{ProjectData, SheetEntry};
    use signex_types::schematic::{ChildSheet, SchematicSheet};

    use super::*;
    use crate::app::state::{DocumentState, ProjectId};

    fn entry(name: &str) -> SheetEntry {
        SheetEntry {
            name: name.to_string(),
            filename: name.to_string(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        }
    }

    fn project(id: ProjectId, dir: &str, sheets: &[&str]) -> LoadedProject {
        LoadedProject {
            id,
            path: PathBuf::from(dir).join("p.snxprj"),
            data: ProjectData {
                description: None,
                name: "p".to_string(),
                dir: dir.to_string(),
                schematic_root: sheets.first().map(|s| (*s).to_string()),
                pcb_file: None,
                sheets: sheets.iter().map(|s| entry(s)).collect(),
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: Vec::new(),
                enable_git: false,
                net_classes: Vec::new(),
                rules: None,
                annotation: None,
                parameters: Default::default(),
                unknown: Default::default(),
            },
            pending_libraries: HashMap::new(),
            case_fold: CaseFold::from_probe(false),
            case_probed: false,
            case_decided: false,
        }
    }

    fn sheet_referencing(filename: &str) -> signex_engine::Engine {
        let mut sheet = SchematicSheet::default();
        sheet.child_sheets.push(ChildSheet {
            uuid: uuid::Uuid::now_v7(),
            name: "c".to_string(),
            filename: filename.to_string(),
            position: Default::default(),
            size: (10.0, 10.0),
            stroke_width: 0.0,
            fill: Default::default(),
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        });
        signex_engine::Engine::new(sheet).unwrap()
    }

    fn owner(ds: &DocumentState, path: &str) -> Option<ProjectId> {
        ds.document_project(Path::new(path)).map(|p| p.id)
    }

    fn state() -> (DocumentState, ProjectId) {
        let (app, _task) = crate::app::Signex::new();
        let mut ds = app.document_state;
        let id = ds.mint_project_id();
        ds.projects
            .push(project(id, "/w/a", &["top.snxsch", "old.snxsch"]));
        (ds, id)
    }

    #[test]
    fn a_renamed_listing_moves_ownership() {
        let (mut ds, id) = state();
        assert_eq!(owner(&ds, "/w/a/old.snxsch"), Some(id));
        ds.projects[0].data.sheets[1].filename = "new.snxsch".to_string();
        assert_eq!(owner(&ds, "/w/a/old.snxsch"), None);
        assert_eq!(owner(&ds, "/w/a/new.snxsch"), Some(id));
    }

    #[test]
    fn adding_and_removing_a_listed_sheet_is_seen() {
        let (mut ds, id) = state();
        assert_eq!(owner(&ds, "/w/a/extra.snxsch"), None);
        ds.projects[0].data.sheets.push(entry("extra.snxsch"));
        assert_eq!(owner(&ds, "/w/a/extra.snxsch"), Some(id));
        ds.projects[0].data.sheets.pop();
        assert_eq!(owner(&ds, "/w/a/extra.snxsch"), None);
    }

    #[test]
    fn closing_the_project_drops_its_sheets() {
        let (mut ds, id) = state();
        assert_eq!(owner(&ds, "/w/a/top.snxsch"), Some(id));
        ds.projects.retain(|p| p.id != id);
        assert_eq!(owner(&ds, "/w/a/top.snxsch"), None);
    }

    #[test]
    fn a_child_reference_edit_on_an_open_sheet_is_seen_without_an_epoch_bump() {
        let (mut ds, id) = state();
        // A hierarchical child no `.snxprj` lists: loose until `top` references it.
        assert_eq!(owner(&ds, "/w/a/sub/leaf.snxsch"), None);
        ds.engines.insert(
            PathBuf::from("/w/a/top.snxsch"),
            sheet_referencing("sub/leaf.snxsch"),
        );
        assert_eq!(owner(&ds, "/w/a/sub/leaf.snxsch"), Some(id));
        // The reference is removed again (a sheet symbol deleted): loose again.
        ds.engines.remove(Path::new("/w/a/top.snxsch"));
        ds.engines.insert(
            PathBuf::from("/w/a/top.snxsch"),
            signex_engine::Engine::new(SchematicSheet::default()).unwrap(),
        );
        assert_eq!(owner(&ds, "/w/a/sub/leaf.snxsch"), None);
    }

    #[test]
    fn an_unchanged_state_reuses_the_index() {
        let (ds, _id) = state();
        let first = ds.ownership_index().stamp();
        let again = ds.ownership_index().stamp();
        assert_eq!(first, again);
        assert!(ds.ownership.borrow().is_some());
    }
}
