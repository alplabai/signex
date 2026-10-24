//! Generic change-detected, generation-debounced, ordered write loop.
//!
//! The pure half of "persist `T` soon": it decides *whether* a write is
//! needed and *when*, and hands back a [`PendingWrite`] job. It performs
//! no IO itself. The app turns the job into a `Task`, so the write runs
//! off the UI thread, and the [`DebouncedStore`] seam lets a test count
//! writes without touching a disk.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};

/// Where a value is written. Production implementations go through
/// `fonts::try_update_prefs_json`; tests substitute a recorder.
pub trait DebouncedStore<T>: Send + Sync {
    fn write(&self, value: &T) -> Result<(), String>;
}

/// What every write of one [`DebouncedSync`] shares, so two writes can
/// never land out of order and quit can wait for the one in flight.
#[derive(Default)]
struct WriteLedger {
    /// Generation of the newest value written. Held for the whole write,
    /// which serialises writes and makes "is a newer one already on
    /// disk?" atomic with the write itself.
    newest_written: Mutex<u64>,
    /// Writes handed out and not yet finished (or dropped unrun).
    in_flight: Mutex<usize>,
    idle: Condvar,
}

fn relock<G>(mutex: &Mutex<G>) -> MutexGuard<'_, G> {
    // The guarded values are plain counters; a panicked writer cannot
    // leave them half-updated.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// [`WriteBarrier::wait`] blocks until every write handed out before the
/// barrier was created has finished.
#[derive(Clone)]
pub struct WriteBarrier(Arc<WriteLedger>);

impl WriteBarrier {
    pub fn wait(&self) {
        let mut in_flight = relock(&self.0.in_flight);
        while *in_flight > 0 {
            in_flight = self
                .0
                .idle
                .wait(in_flight)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }
}

/// One write to run off the UI thread.
pub struct PendingWrite<T> {
    store: Arc<dyn DebouncedStore<T>>,
    value: T,
    generation: u64,
    ledger: Arc<WriteLedger>,
}

impl<T> PendingWrite<T> {
    fn new(
        store: Arc<dyn DebouncedStore<T>>,
        value: T,
        generation: u64,
        ledger: Arc<WriteLedger>,
    ) -> Self {
        *relock(&ledger.in_flight) += 1;
        Self {
            store,
            value,
            generation,
            ledger,
        }
    }

    /// Write the value unless a newer generation already reached the
    /// store; a stale write is a successful no-op, since the disk already
    /// holds something newer.
    pub fn run(self) -> Result<(), String> {
        let mut newest = relock(&self.ledger.newest_written);
        if self.generation <= *newest {
            return Ok(());
        }
        self.store.write(&self.value)?;
        *newest = self.generation;
        Ok(())
    }
}

impl<T> Drop for PendingWrite<T> {
    fn drop(&mut self) {
        let mut in_flight = relock(&self.ledger.in_flight);
        *in_flight -= 1;
        if *in_flight == 0 {
            self.ledger.idle.notify_all();
        }
    }
}

/// Change detection + debounce bookkeeping.
pub struct DebouncedSync<T> {
    store: Arc<dyn DebouncedStore<T>>,
    /// The value believed to be on disk. `None` after a failed write,
    /// so the next observation always schedules a retry.
    saved: Option<T>,
    /// A changed value waiting for its debounce to elapse.
    pending: Option<T>,
    generation: u64,
    ledger: Arc<WriteLedger>,
}

impl<T: Clone + PartialEq> DebouncedSync<T> {
    pub fn new(store: Arc<dyn DebouncedStore<T>>, on_disk: T) -> Self {
        Self {
            store,
            saved: Some(on_disk),
            pending: None,
            generation: 0,
            ledger: Arc::default(),
        }
    }

    /// Record the value after a state change. Returns the generation to
    /// arm a debounce timer for, or `None` when nothing needs writing.
    /// Every call that changes the target invalidates earlier timers.
    pub fn observe(&mut self, current: T) -> Option<u64> {
        if self.pending.as_ref().or(self.saved.as_ref()) == Some(&current) {
            return None;
        }
        self.generation += 1;
        if self.saved.as_ref() == Some(&current) {
            // Changed back to what is already on disk: drop the pending
            // write and let the armed timer go stale.
            self.pending = None;
            return None;
        }
        self.pending = Some(current);
        Some(self.generation)
    }

    /// The timer for `generation` fired. Yields the write only when no
    /// newer change superseded it.
    pub fn take_if_current(&mut self, generation: u64) -> Option<PendingWrite<T>> {
        if generation != self.generation {
            return None;
        }
        self.take_pending()
    }

    /// Take whatever is pending now, ignoring the debounce (quit).
    pub fn flush(&mut self) -> Option<PendingWrite<T>> {
        self.take_pending()
    }

    /// A barrier over the writes handed out so far (quit waits on it for
    /// the write that is already running).
    pub fn barrier(&self) -> WriteBarrier {
        WriteBarrier(Arc::clone(&self.ledger))
    }

    /// A write failed: forget what is on disk so the next change retries.
    pub fn write_failed(&mut self) {
        self.saved = None;
    }

    /// [`Self::write_failed`] for values that must not be lost: the value
    /// is queued again (unless a newer one is already pending) so a later
    /// flush retries it.
    pub fn write_failed_retaining(&mut self, value: T) {
        self.saved = None;
        if self.pending.is_none() {
            self.pending = Some(value);
        }
    }

    fn take_pending(&mut self) -> Option<PendingWrite<T>> {
        let value = self.pending.take()?;
        self.saved = Some(value.clone());
        Some(PendingWrite::new(
            Arc::clone(&self.store),
            value,
            self.generation,
            Arc::clone(&self.ledger),
        ))
    }
}

impl<T> std::fmt::Debug for DebouncedSync<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DebouncedSync")
            .field("pending", &self.pending.is_some())
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    #[derive(Default)]
    struct Recorder {
        written: Mutex<Vec<u32>>,
    }

    impl DebouncedStore<u32> for Recorder {
        fn write(&self, value: &u32) -> Result<(), String> {
            relock(&self.written).push(*value);
            Ok(())
        }
    }

    fn sync_with(store: Arc<dyn DebouncedStore<u32>>) -> DebouncedSync<u32> {
        DebouncedSync::new(store, 0)
    }

    #[test]
    fn an_unchanged_value_arms_no_timer() {
        let mut sync = sync_with(Arc::new(Recorder::default()));
        assert_eq!(sync.observe(0), None);
    }

    #[test]
    fn a_burst_coalesces_into_the_newest_generation_only() {
        let recorder = Arc::new(Recorder::default());
        let mut sync = sync_with(recorder.clone());
        let g1 = sync.observe(1).expect("first change");
        let g2 = sync.observe(2).expect("second change");
        let g3 = sync.observe(3).expect("third change");

        assert!(sync.take_if_current(g1).is_none(), "stale timer");
        assert!(sync.take_if_current(g2).is_none(), "stale timer");
        sync.take_if_current(g3)
            .expect("the current timer")
            .run()
            .expect("write");

        assert_eq!(*relock(&recorder.written), vec![3]);
    }

    #[test]
    fn changing_back_to_the_saved_value_cancels_the_write() {
        let mut sync = sync_with(Arc::new(Recorder::default()));
        let armed = sync.observe(1).expect("a change");
        assert_eq!(sync.observe(0), None);
        assert!(sync.take_if_current(armed).is_none());
        assert!(sync.flush().is_none());
    }

    #[test]
    fn a_failed_write_makes_the_next_observation_retry() {
        let mut sync = sync_with(Arc::new(Recorder::default()));
        let armed = sync.observe(1).expect("a change");
        drop(sync.take_if_current(armed));
        sync.write_failed();
        assert!(sync.observe(1).is_some(), "the retry is scheduled");
    }

    /// A failed write is queued again and the next flush retries it.
    #[test]
    fn a_retained_failure_is_written_by_the_next_flush() {
        let recorder = Arc::new(Recorder::default());
        let mut sync = sync_with(recorder.clone());
        let armed = sync.observe(1).expect("a change");
        drop(sync.take_if_current(armed));
        sync.write_failed_retaining(1);

        sync.flush().expect("the retry").run().expect("retry write");

        assert_eq!(*relock(&recorder.written), vec![1]);
    }

    /// A failure reported after a newer value was observed keeps the newer
    /// one pending, not the value that failed.
    #[test]
    fn a_retained_failure_keeps_a_newer_pending_value() {
        let recorder = Arc::new(Recorder::default());
        let mut sync = sync_with(recorder.clone());
        let first = sync.observe(1).expect("a change");
        drop(sync.take_if_current(first));
        sync.observe(2).expect("a newer change");
        sync.write_failed_retaining(1);

        sync.flush()
            .expect("the pending write")
            .run()
            .expect("write");

        assert_eq!(*relock(&recorder.written), vec![2]);
    }

    /// Two writes in flight, the older one scheduled last: the newer
    /// value must be what the store ends up holding.
    #[test]
    fn an_older_write_landing_last_is_skipped() {
        let recorder = Arc::new(Recorder::default());
        let mut sync = sync_with(recorder.clone());
        let older = sync.observe(1).expect("a change");
        let older_write = sync.take_if_current(older).expect("the older write");
        let newer = sync.observe(2).expect("a second change");
        let newer_write = sync.take_if_current(newer).expect("the newer write");

        newer_write.run().expect("newer write");
        older_write.run().expect("older write");

        assert_eq!(*relock(&recorder.written), vec![2]);
    }

    /// In generation order both writes run, oldest first.
    #[test]
    fn writes_in_order_both_reach_the_store() {
        let recorder = Arc::new(Recorder::default());
        let mut sync = sync_with(recorder.clone());
        let first = sync.observe(1).expect("a change");
        let first_write = sync.take_if_current(first).expect("first write");
        let second = sync.observe(2).expect("a second change");
        let second_write = sync.take_if_current(second).expect("second write");

        first_write.run().expect("first write");
        second_write.run().expect("second write");

        assert_eq!(*relock(&recorder.written), vec![1, 2]);
    }

    /// Blocks inside `write` until let go, to hold a write "in flight".
    struct GatedStore {
        entered: Mutex<mpsc::Sender<()>>,
        proceed: Mutex<mpsc::Receiver<()>>,
    }

    impl DebouncedStore<u32> for GatedStore {
        fn write(&self, _value: &u32) -> Result<(), String> {
            relock(&self.entered).send(()).expect("announce entry");
            relock(&self.proceed).recv().expect("wait to proceed");
            Ok(())
        }
    }

    /// Quit's barrier must not return while a write is still running.
    #[test]
    fn the_barrier_waits_for_the_write_in_flight() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (proceed_tx, proceed_rx) = mpsc::channel();
        let store = Arc::new(GatedStore {
            entered: Mutex::new(entered_tx),
            proceed: Mutex::new(proceed_rx),
        });
        let mut sync = sync_with(store);
        let generation = sync.observe(1).expect("a change");
        let write = sync.take_if_current(generation).expect("the write");
        let running = std::thread::spawn(move || write.run());
        entered_rx.recv().expect("the write is inside the store");

        let barrier = sync.barrier();
        let (waited_tx, waited_rx) = mpsc::channel();
        let waiter = std::thread::spawn(move || {
            barrier.wait();
            waited_tx.send(()).expect("announce the wait is over");
        });

        assert!(
            waited_rx.recv_timeout(Duration::from_millis(150)).is_err(),
            "the barrier returned while a write was still running"
        );
        proceed_tx.send(()).expect("let the write finish");
        waited_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the barrier returns once the write finished");
        running.join().expect("write thread").expect("write");
        waiter.join().expect("waiter thread");
    }

    #[test]
    fn the_barrier_returns_at_once_when_nothing_is_in_flight() {
        let sync = sync_with(Arc::new(Recorder::default()));
        sync.barrier().wait();
    }
}
