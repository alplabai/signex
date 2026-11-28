//! What the app keeps about a library import.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};

use super::events::ProgressEvent;
use super::run::{RunFailure, RunMode};
use super::snapshot::ImportJobOutcome;
use crate::ignore::IgnoreResult as _;

/// Where a finished import is mounted, settled when it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MountTarget {
    /// Recorded in `global_libraries.toml`.
    Global,
    /// Added to this project's (`.snxprj` path) libraries.
    Project(PathBuf),
}

/// The import in progress.
#[derive(Debug, Clone)]
pub struct ImportRun {
    pub output: PathBuf,
    pub mode: RunMode,
    pub target: MountTarget,
    pub cancel: Arc<AtomicBool>,
    /// The converter's process group while it runs.
    pub group: super::process::ProcessGroup,
    /// How far the run's worker has got, for the quit wait.
    pub stage: JobStage,
    /// The latest progress event, for the dialog's progress bar.
    pub progress: Option<ProgressEvent>,
}

/// A finished run, as it reaches `update`.
#[derive(Debug, Clone)]
pub struct ImportFinished {
    pub result: Result<ImportJobOutcome, RunFailure>,
    pub target: MountTarget,
}

/// The import part of the library state.
#[derive(Debug, Default)]
pub struct ImportState {
    pub running: Option<ImportRun>,
    /// The last finished run, kept for the dialog's report view.
    pub last: Option<Box<ImportFinished>>,
    /// The session's converter discovery, handed back to the next one so
    /// an unchanged PATH asks no converter again.
    pub discovered: Option<super::discovery::DiscoveryCache>,
    /// When this session started: a staging folder last changed before it
    /// is a crash's leftover (`staging::sweep_stale`).
    pub session_started: SessionStart,
}

/// The time the library state was made, at app start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionStart(pub std::time::SystemTime);

impl Default for SessionStart {
    fn default() -> Self {
        Self(std::time::SystemTime::now())
    }
}

const WORKING: u8 = 0;
const SETTLING: u8 = 1;
const DONE: u8 = 2;

/// How far a run's worker has got (#3123), shared with the app so quitting
/// can wait for it. Settling is the git work on the library after the
/// converter: a failed update's restore, or the import commit.
#[derive(Debug, Clone, Default)]
pub struct JobStage(Arc<AtomicU8>);

/// Marks the stage done when dropped, including on a panic.
pub struct DoneOnDrop(JobStage);

impl Drop for DoneOnDrop {
    fn drop(&mut self) {
        (self.0).0.store(DONE, Ordering::Release);
    }
}

impl JobStage {
    /// The worker has started writing the library through git.
    pub fn settling(&self) {
        self.0
            .compare_exchange(WORKING, SETTLING, Ordering::AcqRel, Ordering::Acquire)
            .ignore("a worker already done stays done");
    }

    /// A guard that marks the worker done when it goes out of scope.
    pub fn done_on_drop(&self) -> DoneOnDrop {
        DoneOnDrop(self.clone())
    }

    /// Block until the worker is done; true when it is. Gives up after
    /// `limit` while the worker is still working, but never while it is
    /// settling: a restore or a commit cut short leaves the library half
    /// written.
    pub fn wait_at_quit(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        loop {
            match self.0.load(Ordering::Acquire) {
                DONE => return true,
                WORKING if Instant::now() >= deadline => return false,
                _ => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_quit_wait_ends_when_the_worker_is_done() {
        let stage = JobStage::default();
        drop(stage.done_on_drop());
        assert!(stage.wait_at_quit(Duration::from_millis(10)));
    }

    #[test]
    fn the_quit_wait_gives_up_on_a_worker_still_working() {
        let stage = JobStage::default();
        let started = Instant::now();
        assert!(!stage.wait_at_quit(Duration::from_millis(30)));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    /// A restore is never cut short: past the limit, a settling worker is
    /// still waited for.
    #[test]
    fn the_quit_wait_outlasts_its_limit_while_the_worker_settles() {
        let stage = JobStage::default();
        stage.settling();
        let guard = stage.done_on_drop();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            drop(guard);
        });
        let started = Instant::now();
        assert!(stage.wait_at_quit(Duration::from_millis(10)));
        assert!(started.elapsed() >= Duration::from_millis(150));
        worker.join().unwrap();
    }
}
