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
use std::time::{Duration, Instant};

/// A probe still unanswered after this long is presumed hung (a dead network
/// mount) and abandoned so the next one can start.
const PROBE_DEADLINE: Duration = crate::library::external_poll::POLL_TIMEOUT;

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

/// Completions that write, create, delete or rename files the project tree
/// shows. Only these make the cache stale ahead of the poll tick; chatty UI
/// messages must not, or every keystroke would cost a disk probe.
pub fn message_touches_disk(message: &crate::app::Message) -> bool {
    use crate::app::{FileMsg, Message, ProjectMsg};
    use crate::library::LibraryMessage as L;
    match message {
        Message::File(m) => matches!(
            m,
            FileMsg::NewProject(_) | FileMsg::SampleProjectUnpacked(_) | FileMsg::DocumentSaved(_)
        ),
        Message::Project(m) => matches!(
            m,
            ProjectMsg::CloseRefsResolved { .. }
                | ProjectMsg::AddExistingFilePicked { .. }
                | ProjectMsg::AddExistingFilesCopied { .. }
                | ProjectMsg::AddNewSchematicWritten { .. }
        ),
        Message::Library(m) => matches!(
            m,
            L::CreateLibraryAt(_)
                | L::CreateLibraryAtPath { .. }
                | L::LibraryCreateOptionsConfirm
                | L::AddLibraryFileWritten { .. }
                | L::NewComponentSubmit
                | L::NewComponentConfirmCreateTable
                | L::BrowserConfirmAddTable { .. }
                | L::BrowserDeleteTable { .. }
                | L::BrowserConfirmRenameTable { .. }
                | L::BrowserConfirmAddClass { .. }
                | L::BrowserDeleteClass { .. }
                | L::BrowserConfirmRenameClass { .. }
                | L::BrowserDeleteRowConfirm { .. }
                | L::BrowserCellCommit { .. }
                | L::RecoveryLibraryMissing(_)
                | L::RecoveryLibraryMissingLocateResult(_)
                | L::ComponentsPanelPromoteToGlobal(_)
                | L::LibraryUpdatesApply
        ),
        _ => false,
    }
}

/// The facts, the request they answer, and the probe bookkeeping.
#[derive(Debug, Default)]
pub struct ProjectFsCache {
    pub facts: ProjectFsFacts,
    wanted: ProjectFsRequest,
    stale: bool,
    /// Generation and start time of the running probe; `None` while idle.
    in_flight: Option<(u64, Instant)>,
    /// Generation handed to the most recent probe.
    generation: u64,
}

impl ProjectFsCache {
    /// Record what the tree depends on now; a change makes the cache stale.
    pub fn want(&mut self, request: ProjectFsRequest) {
        if request != self.wanted {
            self.wanted = request;
            self.stale = true;
        }
    }

    /// The disk changed without the path set changing (a save, delete or
    /// rename the app did itself): re-probe on the next update.
    pub fn invalidate(&mut self) {
        self.stale = true;
    }

    /// Claim the next probe, if one may start: none while another runs, and
    /// `force` (the poll tick) or staleness is needed.
    pub fn begin_probe(&mut self, force: bool) -> Option<(u64, ProjectFsRequest)> {
        if self
            .in_flight
            .is_some_and(|(_, t)| t.elapsed() < PROBE_DEADLINE)
            || !(force || self.stale)
            || !self.is_watching()
        {
            return None;
        }
        self.generation += 1;
        self.in_flight = Some((self.generation, Instant::now()));
        self.stale = false;
        Some((self.generation, self.wanted.clone()))
    }

    /// A probe ended; `facts` is `None` when its worker died (the next poll
    /// tick retries, so a deterministic failure cannot spin). A result whose
    /// `generation` is not the running probe's (one reclaimed after the
    /// deadline) is dropped. True when the facts changed and the tree must be
    /// rebuilt.
    pub fn finish_probe(&mut self, generation: u64, facts: Option<ProjectFsFacts>) -> bool {
        if self.in_flight.map(|(g, _)| g) != Some(generation) {
            return false;
        }
        self.in_flight = None;
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

    #[cfg(test)]
    pub fn probe_running(&self) -> bool {
        self.in_flight.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watching() -> ProjectFsCache {
        let mut cache = ProjectFsCache::default();
        cache.want(ProjectFsRequest {
            exists: vec![PathBuf::from("a")],
            listings: Vec::new(),
        });
        cache
    }

    #[test]
    fn a_hung_probe_is_reclaimed_after_the_deadline() {
        let mut cache = watching();
        assert!(cache.begin_probe(true).is_some());
        assert!(cache.begin_probe(true).is_none(), "still within deadline");
        let (generation, _) = cache.in_flight.unwrap();
        cache.in_flight = Instant::now()
            .checked_sub(PROBE_DEADLINE + Duration::from_secs(1))
            .map(|t| (generation, t));
        assert!(cache.begin_probe(true).is_some(), "deadline passed");
    }

    #[test]
    fn a_dead_worker_retries_on_the_forced_tick_only() {
        let mut cache = watching();
        let (generation, _) = cache.begin_probe(false).unwrap();
        assert!(!cache.finish_probe(generation, None));
        assert!(cache.begin_probe(false).is_none(), "no hot retry loop");
        assert!(cache.begin_probe(true).is_some(), "the tick retries");
    }

    #[test]
    fn a_reclaimed_probes_late_result_is_dropped() {
        let mut cache = watching();
        let (old, _) = cache.begin_probe(false).unwrap();
        cache.in_flight = Instant::now()
            .checked_sub(PROBE_DEADLINE + Duration::from_secs(1))
            .map(|t| (old, t));
        let (new, _) = cache.begin_probe(true).unwrap();
        assert_ne!(old, new);
        let mut late = ProjectFsFacts::default();
        late.exists.insert(PathBuf::from("a"), false);
        assert!(!cache.finish_probe(old, Some(late)), "stale result ignored");
        assert!(!cache.facts.is_missing(Path::new("a")));
        assert!(cache.probe_running(), "the newer probe is still in flight");
        cache.finish_probe(new, Some(ProjectFsFacts::default()));
        assert!(!cache.probe_running());
    }

    #[test]
    fn invalidate_makes_the_next_probe_start() {
        let mut cache = watching();
        let (generation, _) = cache.begin_probe(false).unwrap();
        cache.finish_probe(generation, Some(ProjectFsFacts::default()));
        assert!(cache.begin_probe(false).is_none());
        cache.invalidate();
        assert!(cache.begin_probe(false).is_some());
    }
}
