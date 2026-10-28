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

/// Holds one slot of `IN_FLIGHT`; releases it when the write completes or its
/// task is dropped without running.
struct InFlightSlot;

impl InFlightSlot {
    fn take() -> Self {
        IN_FLIGHT.fetch_add(1, Ordering::SeqCst);
        Self
    }
}

impl Drop for InFlightSlot {
    fn drop(&mut self) {
        IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
    }
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
    let slot = InFlightSlot::take();
    Task::perform(
        async move {
            let result = tokio::task::spawn_blocking(move || {
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
