//! Filesystem facts the project tree shows (#111): which registered files
//! are missing and what a library's `symbols/` / `footprints/` hold.
//!
//! `refresh_panel_ctx` runs from ~180 call sites and must not touch the
//! disk, so it only reads [`ProjectFsFacts`] from [`ProjectFsCache`]. The
//! facts are refilled off-thread by [`probe`], started when the set of paths
//! the tree depends on changes and on the external-edit poll tick, and
//! swapped in when the answer lands. Until then the last known state shows.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// What the project tree needs to know about the disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectFsRequest {
    /// Paths whose existence marks a `(missing)` row.
    pub exists: Vec<PathBuf>,
    /// Directories to list, with the extension (no dot) whose file stems
    /// are wanted.
    pub listings: Vec<(PathBuf, &'static str)>,
}

/// The answer to a [`ProjectFsRequest`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectFsFacts {
    exists: HashMap<PathBuf, bool>,
    listings: HashMap<PathBuf, Vec<String>>,
}

impl ProjectFsFacts {
    /// A path not probed yet counts as present, so a freshly opened project
    /// does not flash `(missing)` before its first probe lands.
    pub fn is_missing(&self, path: &Path) -> bool {
        self.exists.get(path) == Some(&false)
    }

    /// Sorted file stems of the last listing of `dir`; empty before one lands.
    pub fn names_in(&self, dir: &Path) -> Vec<String> {
        self.listings.get(dir).cloned().unwrap_or_default()
    }
}

/// Run the requested `stat` / `read_dir` calls. Blocking: call it off the
/// UI thread.
pub fn probe(request: &ProjectFsRequest) -> ProjectFsFacts {
    let mut facts = ProjectFsFacts::default();
    for path in request.exists.iter().collect::<HashSet<_>>() {
        facts.exists.insert(path.clone(), path.exists());
    }
    for (dir, ext) in &request.listings {
        let context = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("library");
        let mut names: Vec<String> = crate::app::dir_listing::list_dir_or_report(dir, context)
            .into_iter()
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(*ext))
            .filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(str::to_string))
            .collect();
        names.sort();
        facts.listings.insert(dir.clone(), names);
    }
    facts
}

/// The facts, the request they answer, and the probe bookkeeping.
#[derive(Debug, Default)]
pub struct ProjectFsCache {
    pub facts: ProjectFsFacts,
    wanted: ProjectFsRequest,
    stale: bool,
    in_flight: bool,
}

impl ProjectFsCache {
    /// Record what the tree depends on now; a change makes the cache stale.
    pub fn want(&mut self, request: ProjectFsRequest) {
        if request != self.wanted {
            self.wanted = request;
            self.stale = true;
        }
    }

    /// Claim the next probe, if one may start: none while another runs, and
    /// `force` (the poll tick) or staleness is needed.
    pub fn begin_probe(&mut self, force: bool) -> Option<ProjectFsRequest> {
        if self.in_flight || !(force || self.stale) || !self.is_watching() {
            return None;
        }
        self.in_flight = true;
        self.stale = false;
        Some(self.wanted.clone())
    }

    /// A probe ended; `facts` is `None` when its worker died. True when the
    /// facts changed and the tree must be rebuilt.
    pub fn finish_probe(&mut self, facts: Option<ProjectFsFacts>) -> bool {
        self.in_flight = false;
        match facts {
            Some(f) if f != self.facts => {
                self.facts = f;
                true
            }
            _ => false,
        }
    }

    /// True while there is something to probe, i.e. while the poll should
    /// keep ticking.
    pub fn is_watching(&self) -> bool {
        self.wanted != ProjectFsRequest::default()
    }
}
