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
}

/// The answer to a [`ProjectFsRequest`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectFsFacts {
    exists: HashMap<PathBuf, bool>,
}

impl ProjectFsFacts {
    /// A path not probed yet counts as present, so a freshly opened project
    /// does not flash `(missing)` before its first probe lands.
    pub fn is_missing(&self, path: &Path) -> bool {
        self.exists.get(path) == Some(&false)
    }

    /// True once a probe saw that `dir` has no `.git`. Unprobed reads as
    /// "has one", so nothing offers Enable Version Control before the answer.
    pub fn lacks_git(&self, dir: &Path) -> bool {
        self.is_missing(&git_marker(dir))
    }

    /// Record a fact the app itself just made true (a repository it created),
    /// so the UI does not wait for the next probe to see it.
    pub fn set_exists(&mut self, path: PathBuf, exists: bool) {
        self.exists.insert(path, exists);
    }
}

/// The one definition of "this directory is a Git working tree" for the app.
pub fn git_marker(dir: &Path) -> PathBuf {
    dir.join(".git")
}

/// Blocking `stat`: never call it from `view`. UI-thread update code reads
/// [`ProjectFsFacts::lacks_git`] instead; `runtime/history.rs`
/// (`resolve_history_target`) is a known update-path caller, tracked as a
/// follow-up.
pub fn dir_has_git(dir: &Path) -> bool {
    git_marker(dir).exists()
}

/// Run the requested `stat` / `read_dir` calls. Blocking: call it off the
/// UI thread.
pub fn probe(request: &ProjectFsRequest) -> ProjectFsFacts {
    let mut facts = ProjectFsFacts::default();
    for path in request.exists.iter().collect::<HashSet<_>>() {
        facts.exists.insert(path.clone(), path.exists());
    }
    facts
}

/// Completions that write, create, delete or rename files the project tree
/// shows. Only these make the cache stale ahead of the poll tick; chatty UI
/// messages must not, or every keystroke would cost a disk probe.
pub fn message_touches_disk(message: &crate::app::Message) -> bool {
    use crate::app::{FileMsg, Message, ProjectMsg, RemoveMsg, RenameMsg};
    use crate::library::LibraryMessage as L;
    match message {
        Message::File(m) => matches!(
            m,
            FileMsg::NewProject(_)
                | FileMsg::SampleProjectUnpacked(_)
                | FileMsg::AltiumImportFinished(_)
                | FileMsg::DocumentSaved(_)
                | FileMsg::PrimitiveSaved(_)
                | FileMsg::SavePrimitiveAs { .. }
        ),
        Message::Rename(m) => matches!(
            m,
            RenameMsg::Submit
                | RenameMsg::FileRenamed(_)
                | RenameMsg::LibraryRenamed(_)
                | RenameMsg::LibraryRenameRolledBack(_)
        ),
        Message::EnableVersionControl(m) => {
            matches!(m, crate::app::EnableVersionControlMsg::Finished { .. })
        }
        Message::Remove(m) => matches!(m, RemoveMsg::Confirm(_) | RemoveMsg::Deleted(_)),
        Message::Project(m) => matches!(
            m,
            ProjectMsg::CloseRefsResolved { .. }
                | ProjectMsg::AddExistingFilePicked { .. }
                | ProjectMsg::AddExistingFilesCopied { .. }
                | ProjectMsg::AddNewSchematicWritten { .. }
                | ProjectMsg::FileSaved(_)
        ),
        Message::Library(m) => matches!(
            m,
            L::CreateLibraryAt(_)
                | L::CreateLibraryAtPath { .. }
                | L::LibraryCreateOptionsConfirm
                | L::AddLibraryFileWritten { .. }
                | L::NewComponent
                | L::BrowserConfirmAddTable { .. }
                | L::BrowserDeleteTable { .. }
                | L::BrowserConfirmRenameTable { .. }
                | L::BrowserDeleteRowConfirm { .. }
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
    /// The last finished probe's worker died; warn once per streak.
    failing: bool,
}

/// What a finished probe means for the caller.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ProbeFinish {
    /// The facts changed: rebuild the tree.
    pub changed: bool,
    /// The worker died and this is the first failure of a streak: report it.
    pub warning: Option<String>,
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

    /// A probe ended; `facts` is `Err` when its worker died (the next poll
    /// tick retries, so a deterministic failure cannot spin, and only the
    /// first failure of a streak is reported). A result whose `generation`
    /// is not the running probe's (one reclaimed after the deadline) is
    /// dropped.
    pub fn finish_probe(
        &mut self,
        generation: u64,
        facts: Result<ProjectFsFacts, String>,
    ) -> ProbeFinish {
        if self.in_flight.map(|(g, _)| g) != Some(generation) {
            return ProbeFinish::default();
        }
        self.in_flight = None;
        match facts {
            Err(reason) => {
                let first = !std::mem::replace(&mut self.failing, true);
                ProbeFinish {
                    changed: false,
                    warning: first.then_some(reason),
                }
            }
            Ok(f) => {
                self.failing = false;
                let changed = f != self.facts;
                if changed {
                    self.facts = f;
                }
                ProbeFinish {
                    changed,
                    warning: None,
                }
            }
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
        let finish = cache.finish_probe(generation, Err("panicked".into()));
        assert!(!finish.changed);
        assert!(cache.begin_probe(false).is_none(), "no hot retry loop");
        assert!(cache.begin_probe(true).is_some(), "the tick retries");
    }

    #[test]
    fn a_dead_worker_is_reported_once_per_failure_streak() {
        let mut cache = watching();
        let fail = |cache: &mut ProjectFsCache| {
            let (generation, _) = cache.begin_probe(true).unwrap();
            cache.finish_probe(generation, Err("worker panicked".into()))
        };
        assert_eq!(fail(&mut cache).warning.as_deref(), Some("worker panicked"));
        assert_eq!(fail(&mut cache).warning, None, "same streak: quiet");
        let (generation, _) = cache.begin_probe(true).unwrap();
        assert_eq!(
            cache.finish_probe(generation, Ok(ProjectFsFacts::default())),
            ProbeFinish::default()
        );
        assert!(
            fail(&mut cache).warning.is_some(),
            "a new streak reports again"
        );
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
        assert!(
            !cache.finish_probe(old, Ok(late)).changed,
            "stale result ignored"
        );
        assert!(!cache.facts.is_missing(Path::new("a")));
        assert!(cache.probe_running(), "the newer probe is still in flight");
        cache.finish_probe(new, Ok(ProjectFsFacts::default()));
        assert!(!cache.probe_running());
    }

    #[test]
    fn invalidate_makes_the_next_probe_start() {
        let mut cache = watching();
        let (generation, _) = cache.begin_probe(false).unwrap();
        cache.finish_probe(generation, Ok(ProjectFsFacts::default()));
        assert!(cache.begin_probe(false).is_none());
        cache.invalidate();
        assert!(cache.begin_probe(false).is_some());
    }

    #[test]
    fn disk_writes_that_keep_the_path_set_mark_the_cache_stale() {
        use crate::app::{FileMsg, Message, RemoveChoice, RemoveMsg, RenameMsg};
        let writes = [
            Message::File(FileMsg::SavePrimitiveAs {
                from_path: PathBuf::from("a.snxsym"),
                to_path: PathBuf::from("b.snxsym"),
            }),
            Message::Rename(RenameMsg::Submit),
            Message::Remove(RemoveMsg::Confirm(RemoveChoice::DeleteFile)),
        ];
        for message in &writes {
            assert!(message_touches_disk(message), "{message:?}");
        }
        assert!(!message_touches_disk(&Message::Rename(RenameMsg::Close)));
        assert!(!message_touches_disk(&Message::Remove(RemoveMsg::Close)));
    }
}
