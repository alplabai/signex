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
}

/// Tick while at least one library is mounted; no timer otherwise.
pub fn subscription(state: &LibraryState) -> Subscription<LibraryMessage> {
    if state.open_libraries.is_empty() {
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
    stat_running.store(true, Ordering::SeqCst);
    let flag = Arc::clone(&stat_running);
    let pass = tokio::task::spawn_blocking(move || {
        let changed = libraries_changed(&watched);
        flag.store(false, Ordering::SeqCst);
        changed
    });
    match tokio::time::timeout(POLL_TIMEOUT, pass).await {
        Ok(changed) => PollOutcome::Changed(changed.unwrap_or_default()),
        Err(_) => PollOutcome::TimedOut,
    }
}
