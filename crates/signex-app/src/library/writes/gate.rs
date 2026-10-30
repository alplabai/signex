//! The per-library write order (#893).
//!
//! Every library mutation (and every refresh read that follows one) takes a
//! numbered [`Turn`] on its library's [`WriteGate`] while `update` still has
//! the message in hand, so the numbers follow the order the user acted in.
//! The blocking worker then waits for its number before it touches the
//! library. Two quick edits of one library therefore land in the order they
//! were made, whichever worker thread the runtime wakes first, and a read
//! queued behind a write sees that write.
//!
//! A turn is released on drop, so a worker that panics, or a task the
//! runtime drops before it ran, cannot stall the writes behind it (ADR-0003
//! D5): a turn dropped before its number is up is remembered and skipped.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The revision each row got from this app's own latest write to it.
///
/// A queued edit is built from the row the user saw. When an earlier write
/// of this app moved that row on, the edit is rebased onto the row that
/// write produced instead of being refused as stale; a row that changed
/// since for any other reason (another process, another user) is not in
/// step with this ledger and is still refused by the adapter.
#[derive(Default)]
pub struct OwnRevisions(Mutex<HashMap<signex_library::RowId, Option<u64>>>);

impl OwnRevisions {
    fn map(&self) -> MutexGuard<'_, HashMap<signex_library::RowId, Option<u64>>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The revision this app's last write to `row` produced, if it made one.
    pub fn of(&self, row: signex_library::RowId) -> Option<Option<u64>> {
        self.map().get(&row).copied()
    }

    /// This app just wrote `row`, which now stands at `revision`.
    pub fn record(&self, row: signex_library::RowId, revision: Option<u64>) {
        self.map().insert(row, revision);
    }

    /// `row` was deleted.
    pub fn forget(&self, row: signex_library::RowId) {
        self.map().remove(&row);
    }
}

#[derive(Default)]
struct GateState {
    /// The number the next [`Turn`] gets.
    next: u64,
    /// The number whose worker may run now.
    serving: u64,
    /// Numbers released before their turn came.
    skipped: BTreeSet<u64>,
    /// How many of the turns handed out and not yet released are reads.
    reads: u64,
}

/// The write order of one library.
#[derive(Default)]
pub struct WriteGate {
    own: OwnRevisions,
    state: Mutex<GateState>,
    changed: Condvar,
}

impl WriteGate {
    fn lock(&self) -> MutexGuard<'_, GateState> {
        // The state is plain counters; a panic elsewhere cannot leave it
        // half-updated, so a poisoned lock is recovered.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Writes (and reads queued behind them) that have not finished.
    pub fn pending(&self) -> u64 {
        let state = self.lock();
        state.next - state.serving
    }

    /// Writes that have not finished; reads queued behind them do not count.
    pub fn pending_writes(&self) -> u64 {
        let state = self.lock();
        state.next - state.serving - state.reads
    }

    /// Wait until every turn handed out so far is released, at most `limit`.
    /// `true` when the gate is idle.
    pub fn wait_idle(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        let mut state = self.lock();
        while state.serving != state.next {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            state = self
                .changed
                .wait_timeout(state, left)
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
        true
    }
}

/// One place in a library's write order. Call [`Self::wait`] on the worker
/// before touching the library; dropping it hands the library to the next.
#[must_use = "a turn that is dropped gives its place to the next writer"]
pub struct Turn {
    gate: Arc<WriteGate>,
    ticket: u64,
    read: bool,
}

impl Turn {
    fn take(gate: &Arc<WriteGate>, read: bool) -> Self {
        let mut state = gate.lock();
        let ticket = state.next;
        state.next += 1;
        state.reads += u64::from(read);
        drop(state);
        Self {
            gate: Arc::clone(gate),
            ticket,
            read,
        }
    }

    /// What this app's own writes to the library did to its rows.
    pub fn own_revisions(&self) -> &OwnRevisions {
        &self.gate.own
    }

    /// Block until every earlier turn is released.
    pub fn wait(&self) {
        let mut state = self.gate.lock();
        while state.serving != self.ticket {
            state = self
                .gate
                .changed
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        let mut state = self.gate.lock();
        state.reads -= u64::from(self.read);
        if state.serving == self.ticket {
            state.serving += 1;
            loop {
                let serving = state.serving;
                if !state.skipped.remove(&serving) {
                    break;
                }
                state.serving += 1;
            }
        } else {
            state.skipped.insert(self.ticket);
        }
        drop(state);
        self.gate.changed.notify_all();
    }
}

type Gates = Arc<Mutex<HashMap<PathBuf, Arc<WriteGate>>>>;

/// The write gate of every library, keyed by its `.snxlib` path.
#[derive(Default)]
pub struct LibraryWrites {
    gates: Gates,
}

/// Waits for every library's writes, including the ones that start after
/// the waiter was made. Holds the gates, not the Model, so a worker can use it.
#[derive(Clone)]
pub struct IdleWaiter {
    gates: Gates,
}

fn lock_gates(gates: &Gates) -> MutexGuard<'_, HashMap<PathBuf, Arc<WriteGate>>> {
    gates
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl IdleWaiter {
    /// Wait until no library has a write or read pending, at most `limit`.
    /// `true` when everything is idle.
    pub fn wait_all_idle(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        loop {
            let busy: Vec<Arc<WriteGate>> = lock_gates(&self.gates)
                .values()
                .filter(|gate| gate.pending() > 0)
                .cloned()
                .collect();
            if busy.is_empty() {
                return true;
            }
            for gate in busy {
                let left = deadline.saturating_duration_since(Instant::now());
                if !gate.wait_idle(left) {
                    return false;
                }
            }
        }
    }
}

impl LibraryWrites {
    /// The next place in the write order of the library at `root`.
    pub fn turn(&mut self, root: &Path) -> Turn {
        let mut gates = lock_gates(&self.gates);
        let gate = gates.entry(root.to_path_buf()).or_default();
        Turn::take(gate, false)
    }

    /// The next place in the order of the library at `root` for a worker
    /// that only reads: it still waits for the writes before it, but it is
    /// not a write in [`Self::pending_writes`].
    pub fn read_turn(&mut self, root: &Path) -> Turn {
        let mut gates = lock_gates(&self.gates);
        let gate = gates.entry(root.to_path_buf()).or_default();
        Turn::take(gate, true)
    }

    fn gate(&self, root: &Path) -> Option<Arc<WriteGate>> {
        lock_gates(&self.gates).get(root).cloned()
    }

    /// The library roots with a write queued or running.
    pub fn busy_roots(&self) -> Vec<PathBuf> {
        lock_gates(&self.gates)
            .iter()
            .filter(|(_, gate)| gate.pending() > 0)
            .map(|(root, _)| root.clone())
            .collect()
    }

    /// Writes of the library at `root` that have not finished.
    pub fn pending(&self, root: &Path) -> u64 {
        self.gate(root).map_or(0, |gate| gate.pending())
    }

    /// Writes of the library at `root` that have not finished, not counting
    /// reads queued behind them.
    pub fn pending_writes(&self, root: &Path) -> u64 {
        self.gate(root).map_or(0, |gate| gate.pending_writes())
    }

    /// `row` of the library at `root` was deleted.
    pub fn forget(&self, root: &Path, row: signex_library::RowId) {
        if let Some(gate) = self.gate(root) {
            gate.own.forget(row);
        }
    }

    /// A waiter for every library's writes, for the quit path.
    pub fn idle_waiter(&self) -> IdleWaiter {
        IdleWaiter {
            gates: Arc::clone(&self.gates),
        }
    }

    /// Whether any library has unfinished work.
    pub fn any_pending(&self) -> bool {
        lock_gates(&self.gates)
            .values()
            .any(|gate| gate.pending() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_run_in_the_order_they_were_taken() {
        let mut writes = LibraryWrites::default();
        let root = Path::new("a.snxlib");
        let first = writes.turn(root);
        let second = writes.turn(root);
        let order = Arc::new(Mutex::new(Vec::new()));
        // The later turn's worker starts first; it still runs second.
        let late = {
            let order = Arc::clone(&order);
            std::thread::spawn(move || {
                second.wait();
                order.lock().unwrap().push(2);
            })
        };
        std::thread::sleep(Duration::from_millis(50));
        order.lock().unwrap().push(1);
        drop(first);
        late.join().unwrap();
        assert_eq!(*order.lock().unwrap(), vec![1, 2]);
        assert_eq!(writes.pending(root), 0);
    }

    #[test]
    fn a_turn_dropped_unused_does_not_stall_the_ones_behind_it() {
        let mut writes = LibraryWrites::default();
        let root = Path::new("a.snxlib");
        let first = writes.turn(root);
        let second = writes.turn(root);
        let third = writes.turn(root);
        // The second task is dropped before it ran, and before the first
        // finished.
        drop(second);
        drop(first);
        third.wait();
        assert_eq!(writes.pending(root), 1);
        drop(third);
        assert!(
            writes
                .idle_waiter()
                .wait_all_idle(Duration::from_millis(10))
        );
    }

    #[test]
    fn the_waiter_sees_writes_that_start_after_it_was_made() {
        let mut writes = LibraryWrites::default();
        let waiter = writes.idle_waiter();
        // A library that had no gate when the waiter was made.
        let late = writes.turn(Path::new("late.snxlib"));
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            drop(late);
        });
        let started = Instant::now();
        assert!(waiter.wait_all_idle(Duration::from_secs(5)));
        assert!(started.elapsed() >= Duration::from_millis(150));
        releaser.join().unwrap();
    }

    #[test]
    fn wait_idle_gives_up_at_its_limit() {
        let mut writes = LibraryWrites::default();
        let root = Path::new("a.snxlib");
        let held = writes.turn(root);
        assert!(
            !writes
                .idle_waiter()
                .wait_all_idle(Duration::from_millis(30))
        );
        drop(held);
        assert!(
            writes
                .idle_waiter()
                .wait_all_idle(Duration::from_millis(30))
        );
    }
}
