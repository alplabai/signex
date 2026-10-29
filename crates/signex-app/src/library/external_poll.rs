//! External-edit poll for mounted `.snxlib` files (#531).
//!
//! `view` reads only the caches on `OpenLibrary`, so a file edited by
//! another process stays invisible until something in `update` refreshes
//! them. This module is that trigger, split the MVU way:
//!
//! * [`subscription`] — a slow timer; the only thing that runs while idle.
//! * [`changed_libraries`] — the `stat` of every mounted file, off the UI
//!   thread (`Task::perform`), compared with the stamp each library's
//!   caches were filled under.
//! * the handlers in `app/dispatch/library/external_edits.rs` — refresh the
//!   libraries the poll named, on the update side.
//!
//! The comparison is stat-only (mtime + length). A same-length edit inside
//! one mtime tick is not seen here; it is caught when the user next saves,
//! which compares content (see `signex-library`'s `disk_sync`).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use iced::Subscription;
use signex_library::DiskStamp;

use super::messages::LibraryMessage;
use super::state::LibraryState;

/// How often mounted libraries are checked for external edits.
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// How long one `stat` pass may take before the poll reports it stalled
/// (a dead network mount can block `stat` indefinitely, #827).
pub const POLL_TIMEOUT: Duration = Duration::from_secs(10);

/// A mounted library and the stamp its caches were filled under.
pub type WatchedLibrary = (PathBuf, Option<DiskStamp>);

/// A library whose file changed, with the stamp the file has now.
pub type ChangedLibrary = (PathBuf, DiskStamp);

/// The result of one poll pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollOutcome {
    /// The pass finished; the libraries whose file no longer matches.
    Changed(Vec<ChangedLibrary>),
    /// The pass did not finish within [`POLL_TIMEOUT`]. Its `stat` thread
    /// keeps running and holds the `stat_running` flag until it returns.
    TimedOut,
    /// The pass panicked; the message says why. The flag is cleared anyway.
    Failed(String),
}

/// Tick while at least one library is mounted or `also_watching` (the
/// project tree has disk facts to re-check); no timer otherwise.
pub fn subscription(state: &LibraryState, also_watching: bool) -> Subscription<LibraryMessage> {
    if state.open_libraries.is_empty() && !also_watching {
        return Subscription::none();
    }
    iced::time::every(POLL_INTERVAL).map(|_| LibraryMessage::ExternalEditsPollTicked)
}

/// The watched libraries whose file no longer matches the stamp their
/// caches were filled under, each with its current stamp. A file that cannot
/// be `stat`ed is not reported: the adapter already warns (once) about a
/// missing library, and refreshing would only repeat the failure.
pub fn libraries_changed(watched: &[WatchedLibrary]) -> Vec<ChangedLibrary> {
    watched
        .iter()
        .filter_map(|(path, observed)| match DiskStamp::of(path) {
            Ok(now) if *observed != Some(now) => Some((path.clone(), now)),
            _ => None,
        })
        .collect()
}

/// [`libraries_changed`] on the blocking pool, for `Task::perform`, bounded
/// by [`POLL_TIMEOUT`]. `stat_running` is set for as long as the blocking
/// pass runs, including after a timeout, so the caller can refuse to start
/// a second pass behind a hung one.
pub async fn changed_libraries(
    watched: Vec<WatchedLibrary>,
    stat_running: Arc<AtomicBool>,
) -> PollOutcome {
    run_bounded(
        move || libraries_changed(&watched),
        POLL_TIMEOUT,
        stat_running,
    )
    .await
}

/// Clears the "stat running" flag when the blocking pass ends, however it
/// ends: returning, or unwinding from a panic.
struct StatRunning(Arc<AtomicBool>);

impl Drop for StatRunning {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Run `pass` on the blocking pool, bounded by `timeout`. The flag stays set
/// until `pass` actually returns, even past the timeout.
pub(crate) async fn run_bounded(
    pass: impl FnOnce() -> Vec<ChangedLibrary> + Send + 'static,
    timeout: Duration,
    stat_running: Arc<AtomicBool>,
) -> PollOutcome {
    stat_running.store(true, Ordering::SeqCst);
    let guard = StatRunning(stat_running);
    let task = tokio::task::spawn_blocking(move || {
        let _guard = guard;
        pass()
    });
    match tokio::time::timeout(timeout, task).await {
        Ok(Ok(changed)) => PollOutcome::Changed(changed),
        Ok(Err(join_error)) => PollOutcome::Failed(join_error.to_string()),
        Err(_) => PollOutcome::TimedOut,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #869 review: a pass that outlives the timeout reports `TimedOut`, and
    /// the flag stays set until the pass really returns.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_slow_pass_times_out_and_keeps_the_flag_until_it_returns() {
        let flag = Arc::new(AtomicBool::new(false));
        let outcome = run_bounded(
            || {
                std::thread::sleep(Duration::from_millis(300));
                Vec::new()
            },
            Duration::from_millis(20),
            Arc::clone(&flag),
        )
        .await;
        assert_eq!(outcome, PollOutcome::TimedOut);
        assert!(flag.load(Ordering::SeqCst), "the pass is still running");
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(
            !flag.load(Ordering::SeqCst),
            "cleared once the pass returned"
        );
    }

    /// A pass that panics clears the flag and reports the failure, instead
    /// of leaving every later tick dropped behind a flag nobody clears.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_panicking_pass_clears_the_flag_and_reports_failed() {
        let flag = Arc::new(AtomicBool::new(false));
        let outcome = run_bounded(
            || panic!("stat exploded"),
            Duration::from_secs(5),
            Arc::clone(&flag),
        )
        .await;
        assert!(matches!(outcome, PollOutcome::Failed(_)), "{outcome:?}");
        assert!(!flag.load(Ordering::SeqCst));
    }
}
