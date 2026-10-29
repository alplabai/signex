//! The project net index, built off the UI thread and cached per project.
//!
//! Hovering, highlighting and jumping all read it; none of them builds it on
//! the UI thread or per pointer move. A project's index is rebuilt only when
//! its [`ProjectKey`] moves (an edit that can change a net, a sheet list or
//! root change, a class-table edit); an answer for a key no longer wanted is
//! dropped.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_net::{NetIndex, SheetKey};
use signex_types::net_class::NetClass;

use crate::app::state::net_class_styles::ProjectKey;
use crate::app::state::{CaseFold, ProjectId};

/// One project's stitched net index with what it takes to map its sheet keys
/// to the files open in tabs and back.
#[derive(Debug)]
pub struct ProjectNetData {
    pub index: NetIndex,
    /// The file each sheet key was assembled from.
    pub paths: HashMap<SheetKey, PathBuf>,
    pub base_dir: Option<PathBuf>,
    pub fold: CaseFold,
    pub classes: Vec<NetClass>,
}

impl ProjectNetData {
    /// The key the stitch knows the sheet at `path` by.
    pub fn key_of(&self, path: &Path) -> Option<SheetKey> {
        self.paths
            .iter()
            .find(|(_, held)| held.as_path() == path)
            .map(|(key, _)| key.clone())
            .or_else(|| {
                let key = crate::app::project_sheets::sheet_key_with(
                    path,
                    self.base_dir.as_deref(),
                    self.fold,
                );
                self.paths.contains_key(&key).then_some(key)
            })
    }

    pub fn path_of(&self, key: &SheetKey) -> Option<&Path> {
        self.paths.get(key).map(PathBuf::as_path)
    }

    /// A sheet's name for display: its file stem, else the raw key.
    pub fn sheet_label(&self, key: &SheetKey) -> String {
        self.path_of(key)
            .and_then(Path::file_stem)
            .map_or_else(|| key.to_string(), |s| s.to_string_lossy().into_owned())
    }
}

/// A finished background build.
#[derive(Debug)]
pub struct IndexOutcome {
    pub project: ProjectId,
    pub key: ProjectKey,
    /// The index, or why none came.
    pub result: Result<Arc<ProjectNetData>, String>,
    /// Sheets that would not read and stitch issues, for the Messages panel.
    pub warnings: Vec<String>,
}

/// What storing an [`IndexOutcome`] came to.
#[derive(Debug)]
pub enum Stored {
    /// The index for the key still wanted: now the cache's.
    Fresh(Arc<ProjectNetData>),
    /// The build failed; the key is not retried until it moves.
    Failed(String),
    /// A newer request replaced this one: dropped.
    Stale,
}

#[derive(Default)]
pub struct NetIndexCache {
    entries: HashMap<ProjectId, (ProjectKey, Arc<ProjectNetData>)>,
    wanted: HashMap<ProjectId, ProjectKey>,
    failed: HashMap<ProjectId, ProjectKey>,
}

impl NetIndexCache {
    /// The index held for `project` when it was built for exactly `key`.
    pub fn fresh(&self, project: ProjectId, key: &ProjectKey) -> Option<Arc<ProjectNetData>> {
        self.entries
            .get(&project)
            .filter(|(held, _)| held == key)
            .map(|(_, data)| Arc::clone(data))
    }

    /// The index held for `project`, however old; for reading a result
    /// already shown, never for deciding connectivity.
    pub fn latest(&self, project: ProjectId) -> Option<Arc<ProjectNetData>> {
        self.entries.get(&project).map(|(_, data)| Arc::clone(data))
    }

    /// Record that `project` should hold the index for `key`. `true` when a
    /// worker has to build it: the cache neither holds it, has it in flight,
    /// nor already failed on it.
    pub fn want(&mut self, project: ProjectId, key: &ProjectKey) -> bool {
        if self.failed.get(&project) == Some(key) {
            return false;
        }
        self.failed.remove(&project);
        if self.fresh(project, key).is_some() || self.wanted.get(&project) == Some(key) {
            return false;
        }
        self.wanted.insert(project, key.clone());
        true
    }

    /// Whether the build for `key` is still in flight.
    pub fn is_building(&self, project: ProjectId, key: &ProjectKey) -> bool {
        self.wanted.get(&project) == Some(key) && self.fresh(project, key).is_none()
    }

    /// Store a finished build; a stale answer is dropped. A failure is
    /// remembered so the same key is not retried until the project's key
    /// moves.
    pub fn apply(&mut self, outcome: &IndexOutcome) -> Stored {
        if self.wanted.get(&outcome.project) != Some(&outcome.key) {
            return Stored::Stale;
        }
        self.wanted.remove(&outcome.project);
        match &outcome.result {
            Ok(data) => {
                self.entries
                    .insert(outcome.project, (outcome.key.clone(), Arc::clone(data)));
                Stored::Fresh(Arc::clone(data))
            }
            Err(reason) => {
                self.failed.insert(outcome.project, outcome.key.clone());
                Stored::Failed(reason.clone())
            }
        }
    }

    /// Drop every project not in `keep`.
    pub fn retain_projects(&mut self, keep: &[ProjectId]) {
        self.entries.retain(|id, _| keep.contains(id));
        self.wanted.retain(|id, _| keep.contains(id));
        self.failed.retain(|id, _| keep.contains(id));
    }

    /// Forget requests in flight and failures, for a caller that lost its tasks.
    pub fn forget_pending(&mut self) {
        self.wanted.clear();
        self.failed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(stamp: u64) -> ProjectKey {
        ProjectKey {
            connectivity: stamp,
            pages: Vec::new(),
            root: PathBuf::from("root.snxsch"),
            fold: CaseFold::default(),
            classes: Vec::new(),
        }
    }

    fn data() -> Arc<ProjectNetData> {
        Arc::new(ProjectNetData {
            index: NetIndex::default(),
            paths: HashMap::new(),
            base_dir: None,
            fold: CaseFold::default(),
            classes: Vec::new(),
        })
    }

    fn outcome(project: ProjectId, key: ProjectKey) -> IndexOutcome {
        IndexOutcome {
            project,
            key,
            result: Ok(data()),
            warnings: Vec::new(),
        }
    }

    fn failure(project: ProjectId, key: ProjectKey) -> IndexOutcome {
        IndexOutcome {
            result: Err("no root".into()),
            ..outcome(project, key)
        }
    }

    #[test]
    fn a_key_is_built_once_while_it_is_in_flight_and_after() {
        let p = ProjectId::test_id(1);
        let mut cache = NetIndexCache::default();
        assert!(cache.want(p, &key(1)), "first ask builds");
        assert!(!cache.want(p, &key(1)), "in flight: no second build");
        assert!(cache.is_building(p, &key(1)));
        assert!(matches!(cache.apply(&outcome(p, key(1))), Stored::Fresh(_)));
        assert!(cache.fresh(p, &key(1)).is_some());
        assert!(!cache.want(p, &key(1)), "held: no rebuild");
    }

    #[test]
    fn an_edit_moves_the_key_and_asks_for_a_rebuild() {
        let p = ProjectId::test_id(1);
        let mut cache = NetIndexCache::default();
        cache.want(p, &key(1));
        cache.apply(&outcome(p, key(1)));
        assert!(cache.fresh(p, &key(2)).is_none(), "stale for the new key");
        assert!(cache.want(p, &key(2)));
        assert!(cache.latest(p).is_some(), "the old index stays readable");
    }

    #[test]
    fn an_answer_for_a_key_no_longer_wanted_is_dropped() {
        let p = ProjectId::test_id(1);
        let mut cache = NetIndexCache::default();
        cache.want(p, &key(1));
        cache.want(p, &key(2));
        assert!(matches!(cache.apply(&outcome(p, key(1))), Stored::Stale));
        assert!(cache.fresh(p, &key(1)).is_none());
    }

    #[test]
    fn a_failed_key_is_not_retried_until_the_key_moves() {
        let p = ProjectId::test_id(1);
        let mut cache = NetIndexCache::default();
        cache.want(p, &key(1));
        assert!(matches!(
            cache.apply(&failure(p, key(1))),
            Stored::Failed(_)
        ));
        assert!(!cache.want(p, &key(1)));
        assert!(cache.want(p, &key(2)));
    }
}
