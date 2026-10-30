//! Locks for state the whole `regression` binary shares. The tests run on
//! threads of one process, so a process-wide resource that a test changes or
//! asserts on needs one lock that every such test names.

use std::sync::{Mutex, MutexGuard};

static NEW_ROW_NOTES: Mutex<()> = Mutex::new(());
static GLOBAL_LIST_FILE: Mutex<()> = Mutex::new(());

fn take(lock: &'static Mutex<()>) -> MutexGuard<'static, ()> {
    lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The Messages-panel ring is process-wide and the unbound-row note names no
/// library, so another test's New Component reads as a second report. Every
/// test that creates a row through the app (New Component, or the browser's
/// Add Component) holds this while it runs.
pub(crate) fn new_row_notes() -> MutexGuard<'static, ()> {
    take(&NEW_ROW_NOTES)
}

/// Who may rely on the shared `global_libraries.toml` being a readable file:
/// the components-panel test turns it into a directory, and the stock-copy
/// tests restart an app that must read it back. Held by both for their whole
/// body.
pub(crate) fn global_list_file() -> MutexGuard<'static, ()> {
    take(&GLOBAL_LIST_FILE)
}
