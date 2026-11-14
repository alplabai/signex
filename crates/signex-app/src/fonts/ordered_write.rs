//! Off-thread, last-writer-wins `prefs.json` writes for toggles flipped from
//! `update` (#1469).
//!
//! `update` must never touch the disk, so a toggle hands its write to a
//! `spawn_blocking` worker. Two workers started back to back can reach the
//! prefs lock in either order, and the older value would then land last. Each
//! write therefore takes a ticket on the UI thread, and a worker that finds a
//! newer ticket already applied for the same key skips its write.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use iced::Task;

use crate::app::{Message, UiMsg};

static NEXT_TICKET: AtomicU64 = AtomicU64::new(1);

/// Writes handed out by `background_pref_write` that have not finished (or
/// been dropped unrun). The quit path waits for this to reach zero so the
/// last preference change is never lost to `iced::exit()` (#1351).
static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

// Test hook: the next quit-path wait built on this thread sets its probe when
// it starts waiting. Per thread, so parallel tests cannot see each other's.
#[cfg(test)]
thread_local! {
    static WAIT_PROBE: std::cell::RefCell<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn install_wait_probe(probe: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    WAIT_PROBE.with(|slot| *slot.borrow_mut() = Some(probe));
}

#[cfg(test)]
pub(crate) fn take_wait_probe() -> Option<std::sync::Arc<std::sync::atomic::AtomicBool>> {
    WAIT_PROBE.with(|slot| slot.borrow_mut().take())
}

/// Holds one slot of `IN_FLIGHT`; releases it when the write completes or its
/// task is dropped without running. The second case may lose the setting, so it
/// is reported (#1932).
struct InFlightSlot {
    key: &'static str,
    /// Set by the worker as it starts the write. A blocking task the runtime
    /// never started (it may skip them at shutdown) leaves it unset, so the
    /// warning says the write may not have been saved: a spurious warning
    /// beats a silent loss.
    has_run: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl InFlightSlot {
    fn take(key: &'static str) -> Self {
        IN_FLIGHT.fetch_add(1, Ordering::SeqCst);
        #[cfg(test)]
        SLOTS_TAKEN_HERE.with(|n| n.set(n.get() + 1));
        Self {
            key,
            has_run: std::sync::Arc::default(),
        }
    }
}

impl Drop for InFlightSlot {
    fn drop(&mut self) {
        IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        if !self.has_run.load(Ordering::SeqCst) {
            crate::diagnostics::log_warning(format!(
                "The {} preference may not have been saved: its write was dropped before it ran",
                self.key
            ));
        }
    }
}

// Test hook: how many slots this thread has taken. Per thread, so parallel
// tests cannot move each other's count the way they move `IN_FLIGHT`.
#[cfg(test)]
thread_local! {
    static SLOTS_TAKEN_HERE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many in-flight slots the current thread has taken, for tests that guard
/// a path from taking one before its task runs.
#[cfg(test)]
pub(crate) fn pref_slots_taken_on_this_thread() -> usize {
    SLOTS_TAKEN_HERE.with(std::cell::Cell::get)
}

/// Block until every ordered write started so far has finished, or `limit`
/// passes. Returns whether none are left. Call from a blocking worker.
pub fn wait_for_pref_writes(limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while IN_FLIGHT.load(Ordering::SeqCst) != 0 {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    true
}

/// Newest ticket applied per key. Guards ordering only, holds no app state.
/// Held while a write runs, which is already serialised by the prefs lock.
static APPLIED: Mutex<Option<HashMap<&'static str, u64>>> = Mutex::new(None);

fn applied() -> MutexGuard<'static, Option<HashMap<&'static str, u64>>> {
    APPLIED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One pending write of the preference `key`, ordered against every other
/// write of that key by the moment it was created.
#[derive(Debug, Clone, Copy)]
pub struct OrderedPrefWrite {
    key: &'static str,
    ticket: u64,
}

impl OrderedPrefWrite {
    /// Take the next ticket for `key`. Call on the UI thread, in the order
    /// the user acted.
    pub fn new(key: &'static str) -> Self {
        Self {
            key,
            ticket: NEXT_TICKET.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// Run `write` unless a newer write of the same key already ran.
    /// Returns whether `write` ran.
    pub fn run(self, write: impl FnOnce()) -> bool {
        let mut guard = applied();
        let newest = guard
            .get_or_insert_with(HashMap::new)
            .entry(self.key)
            .or_insert(0);
        if self.ticket < *newest {
            return false;
        }
        *newest = self.ticket;
        write();
        true
    }
}

/// [`background_pref_write`] for an edit merged into what the file holds
/// rather than a whole value: no write is skipped as stale (each carries a
/// different edit), the edits of one key run in the order they were made
/// (`merge_queue`), the quit path still waits for them, and each outcome
/// comes back through `done` (#2232).
pub fn background_pref_merge<T: Send + 'static>(
    key: &'static str,
    write: impl FnOnce() -> Result<T, String> + Send + 'static,
    done: impl Fn(Result<T, String>) -> Message + Send + 'static,
) -> Task<Message> {
    let slot = InFlightSlot::take(key);
    let has_run = slot.has_run.clone();
    // Taken here, on the UI thread, so the order is the user's.
    let ticket = super::merge_queue::Ticket::take(key);
    Task::perform(
        async move {
            let result = tokio::task::spawn_blocking(move || {
                has_run.store(true, Ordering::SeqCst);
                if !ticket.wait_turn() {
                    tracing::warn!(
                        target: "signex::prefs",
                        key = key,
                        "an earlier edit of this preference did not finish in time; this one runs anyway"
                    );
                }
                let result = write();
                drop(ticket);
                result
            })
            .await
            .unwrap_or_else(|e| Err(format!("the preference write task failed: {e}")));
            drop(slot);
            result
        },
        done,
    )
}

/// The `Task` that performs `write` on a blocking worker, never in `update`.
/// `write` reports its own failure through the prefs refusal log; the
/// completion message only republishes that log to the Messages panel.
pub fn background_pref_write(
    key: &'static str,
    write: impl FnOnce() + Send + 'static,
) -> Task<Message> {
    let ordered = OrderedPrefWrite::new(key);
    // Taken here, on the UI thread, so a quit that follows sees the write
    // even before its worker has started.
    let slot = InFlightSlot::take(key);
    let has_run = slot.has_run.clone();
    Task::perform(
        async move {
            let result = tokio::task::spawn_blocking(move || {
                has_run.store(true, Ordering::SeqCst);
                ordered.run(write);
            })
            .await;
            drop(slot);
            if let Err(join_error) = result {
                tracing::error!(
                    target: "signex::prefs",
                    key = key,
                    error = %join_error,
                    "the preference write task failed; the setting was not saved"
                );
            }
        },
        |()| Message::Ui(UiMsg::PrefWriteFinished),
    )
}

#[cfg(test)]
mod dropped_write_tests {
    use super::*;

    /// #1932: a write whose task is dropped unrun is reported, not lost
    /// silently.
    #[test]
    fn a_pref_write_dropped_before_it_ran_is_reported() {
        let needle = "The zq-dropped-write preference may not have been saved";
        let before = crate::diagnostics::logged_count(needle);
        drop(background_pref_write("zq-dropped-write", || {}));
        assert_eq!(crate::diagnostics::logged_count(needle), before + 1);
    }

    /// Dropping the task while its write already runs loses nothing: the
    /// write finishes on its worker, so there is nothing to report.
    #[test]
    fn a_pref_write_dropped_while_it_runs_is_not_reported() {
        use futures::StreamExt as _;
        let needle = "The zq-running-write preference may not have been saved";
        let before = crate::diagnostics::logged_count(needle);
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let write = background_pref_write("zq-running-write", move || {
            started_tx.send(()).ok();
            release_rx.recv().ok();
        });
        let mut stream = iced_runtime::task::into_stream(write).expect("a task");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            let started = tokio::task::spawn_blocking(move || started_rx.recv());
            tokio::select! {
                _ = stream.next() => panic!("the write was released early"),
                _ = started => {}
            }
        });
        drop(stream);
        release_tx.send(()).ok();
        drop(runtime);
        assert_eq!(crate::diagnostics::logged_count(needle), before);
    }

    #[test]
    fn a_pref_write_that_ran_reports_nothing() {
        let needle = "The zq-finished-write preference may not have been saved";
        let before = crate::diagnostics::logged_count(needle);
        let write = background_pref_write("zq-finished-write", || {});
        crate::task_driver::TaskDriver::new()
            .outputs(write)
            .expect("ran");
        assert_eq!(crate::diagnostics::logged_count(needle), before);
    }
}
