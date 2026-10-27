//! Off-thread, last-writer-wins `prefs.json` writes for toggles flipped from
//! `update` (#1469).
//!
//! `update` must never touch the disk, so a toggle hands its write to a
//! `spawn_blocking` worker. Two workers started back to back can reach the
//! prefs lock in either order, and the older value would then land last. Each
//! write therefore takes a ticket on the UI thread, and a worker that finds a
//! newer ticket already applied for the same key skips its write.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use iced::Task;

use crate::app::{Message, UiMsg};

static NEXT_TICKET: AtomicU64 = AtomicU64::new(1);

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
/// It needs no completion handling: `write` reports its own failure through
/// the prefs refusal log.
pub fn background_pref_write(
    key: &'static str,
    write: impl FnOnce() + Send + 'static,
) -> Task<Message> {
    let ordered = OrderedPrefWrite::new(key);
    Task::perform(
        async move {
            if let Err(join_error) = tokio::task::spawn_blocking(move || {
                ordered.run(write);
            })
            .await
            {
                tracing::error!(
                    target: "signex::prefs",
                    key = key,
                    error = %join_error,
                    "the preference write task failed; the setting was not saved"
                );
            }
        },
        |()| Message::Ui(UiMsg::Noop),
    )
}
