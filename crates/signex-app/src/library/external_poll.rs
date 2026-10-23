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
use std::time::Duration;

use iced::Subscription;
use signex_library::DiskStamp;

use super::messages::LibraryMessage;
use super::state::LibraryState;

/// How often mounted libraries are checked for external edits.
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// A mounted library and the stamp its caches were filled under.
pub type WatchedLibrary = (PathBuf, Option<DiskStamp>);

/// Tick while at least one library is mounted; no timer otherwise.
pub fn subscription(state: &LibraryState) -> Subscription<LibraryMessage> {
    if state.open_libraries.is_empty() {
        return Subscription::none();
    }
    iced::time::every(POLL_INTERVAL).map(|_| LibraryMessage::ExternalEditsPollTicked)
}

/// Paths of the watched libraries whose file no longer matches the stamp
/// their caches were filled under. A file that cannot be `stat`ed is not
/// reported: the adapter already warns (once) about a missing library, and
/// refreshing would only repeat the failure.
pub fn libraries_changed(watched: &[WatchedLibrary]) -> Vec<PathBuf> {
    watched
        .iter()
        .filter(|(path, observed)| match DiskStamp::of(path) {
            Ok(now) => *observed != Some(now),
            Err(_) => false,
        })
        .map(|(path, _)| path.clone())
        .collect()
}

/// [`libraries_changed`] on the blocking pool, for `Task::perform`.
pub async fn changed_libraries(watched: Vec<WatchedLibrary>) -> Vec<PathBuf> {
    tokio::task::spawn_blocking(move || libraries_changed(&watched))
        .await
        .unwrap_or_default()
}
