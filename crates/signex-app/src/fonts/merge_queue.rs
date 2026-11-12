//! Submit-order queue for merged preference edits (#2232): an edit merged
//! into the file (not a whole value) must land in the order the user made
//! it, so each takes a ticket on the UI thread and its worker waits until
//! every earlier ticket of the same key has finished or been dropped.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Longest a worker waits for an earlier edit before it runs anyway (and
/// says so), so a stuck earlier write cannot hold every later one forever.
const TURN_LIMIT: Duration = Duration::from_secs(30);

#[derive(Default)]
struct Queues {
    /// Next ticket to hand out, per key.
    next: HashMap<&'static str, u64>,
    /// Every ticket up to this one has finished, per key.
    done_upto: HashMap<&'static str, u64>,
    /// Finished tickets above `done_upto`, waiting for a gap to close.
    finished: HashMap<&'static str, BTreeSet<u64>>,
}

static QUEUES: Mutex<Option<Queues>> = Mutex::new(None);
static TURN: Condvar = Condvar::new();

fn queues() -> MutexGuard<'static, Option<Queues>> {
    QUEUES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One place in the queue of `key`. Dropping it, run or not, lets the next
/// ticket go.
pub(super) struct Ticket {
    key: &'static str,
    number: u64,
}

impl Ticket {
    /// The next place for `key`. Take it on the UI thread, in the order the
    /// user acted.
    pub(super) fn take(key: &'static str) -> Self {
        let mut guard = queues();
        let queues = guard.get_or_insert_with(Queues::default);
        let next = queues.next.entry(key).or_insert(1);
        let number = *next;
        *next += 1;
        Self { key, number }
    }

    /// Block until every earlier ticket of this key is done. `false` when
    /// the wait gave up after [`TURN_LIMIT`].
    pub(super) fn wait_turn(&self) -> bool {
        let deadline = Instant::now() + TURN_LIMIT;
        let mut guard = queues();
        loop {
            let done = guard
                .get_or_insert_with(Queues::default)
                .done_upto
                .get(self.key)
                .copied()
                .unwrap_or(0);
            if done + 1 >= self.number {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            guard = TURN
                .wait_timeout(guard, deadline - now)
                .map(|(g, _)| g)
                .unwrap_or_else(|poisoned| poisoned.into_inner().0);
        }
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        let mut guard = queues();
        let queues = guard.get_or_insert_with(Queues::default);
        let finished = queues.finished.entry(self.key).or_default();
        finished.insert(self.number);
        let done = queues.done_upto.entry(self.key).or_insert(0);
        while finished.remove(&(*done + 1)) {
            *done += 1;
        }
        drop(guard);
        TURN.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Two queued edits apply in the order they were submitted, even when
    /// the later one's worker starts first.
    #[test]
    fn queued_edits_run_in_submit_order() {
        let key = "zq-merge-order";
        let first = Ticket::take(key);
        let second = Ticket::take(key);
        let log = Arc::new(Mutex::new(Vec::new()));
        let later = {
            let log = log.clone();
            std::thread::spawn(move || {
                assert!(second.wait_turn());
                log.lock().unwrap().push(2);
                drop(second);
            })
        };
        std::thread::sleep(Duration::from_millis(50));
        assert!(log.lock().unwrap().is_empty(), "the later edit waits");
        assert!(first.wait_turn());
        log.lock().unwrap().push(1);
        drop(first);
        later.join().expect("joined");
        assert_eq!(*log.lock().unwrap(), [1, 2]);
    }

    /// A ticket dropped without running does not hold the queue.
    #[test]
    fn a_dropped_ticket_lets_the_next_one_run() {
        let key = "zq-merge-dropped";
        let first = Ticket::take(key);
        let second = Ticket::take(key);
        drop(first);
        assert!(second.wait_turn());
    }
}
