//! Debounced, change-detected persistence of the dock layout (#692).
//!
//! `update` used to rewrite `prefs.json` (read + parse + fsync + rename)
//! after every `DockMessage`, hover and per-tick floating-panel moves
//! included. This module is the pure half of the fix: it decides *whether*
//! a write is needed and *when*, and hands back a [`DockLayoutWrite`] job.
//! It performs no IO itself. The app turns the job into a `Task`, so the
//! write runs off the UI thread, and the [`DockLayoutStore`] seam lets a
//! test count writes without touching a disk.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use super::{DockArea, PanelPosition};
use crate::panels::PanelKind;

/// How long the layout must stay still before it is written. A drag or a
/// burst of tab clicks coalesces into one write after this quiet period.
pub const DOCK_LAYOUT_SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

/// The persisted part of one dock region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionLayout {
    pub panels: Vec<PanelKind>,
    pub collapsed: bool,
}

/// Exactly what `prefs.json` stores about the dock. Transient state
/// (hover, drag arming, tab scroll) is deliberately absent, so a message
/// that only touches that state compares equal and never schedules a write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockLayoutSnapshot {
    pub left: RegionLayout,
    pub right: RegionLayout,
    pub bottom: RegionLayout,
}

impl DockLayoutSnapshot {
    pub fn capture(dock: &DockArea) -> Self {
        let region = |pos: PanelPosition| RegionLayout {
            panels: dock.panel_kinds(pos).to_vec(),
            collapsed: dock.is_collapsed(pos),
        };
        Self {
            left: region(PanelPosition::Left),
            right: region(PanelPosition::Right),
            bottom: region(PanelPosition::Bottom),
        }
    }
}

/// Where a layout is written. The production implementation is
/// `fonts::PrefsDockLayoutStore`; tests substitute a recorder.
pub trait DockLayoutStore: Send + Sync {
    fn write(&self, layout: &DockLayoutSnapshot) -> Result<(), String>;
}

/// Messages of the dock-layout persistence loop. Namespaced under
/// `Message::DockLayout`.
#[derive(Debug, Clone)]
pub enum DockLayoutMsg {
    /// The quiet period for generation `.0` ran out. Stale generations
    /// (the layout changed again meanwhile) are ignored.
    SaveDebounceElapsed(u64),
    /// A background write finished; the error text is what the Messages
    /// panel shows.
    Saved(Result<(), String>),
}

/// What every write of one [`DockLayoutSync`] shares, so two writes can
/// never land out of order and quit can wait for the one in flight.
#[derive(Default)]
struct WriteLedger {
    /// Generation of the newest layout written. Held for the whole write,
    /// which serialises dock writes and makes "is a newer one already on
    /// disk?" atomic with the write itself.
    newest_written: Mutex<u64>,
    /// Writes handed out and not yet finished (or dropped unrun).
    in_flight: Mutex<usize>,
    idle: Condvar,
}

fn relock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // The guarded values are plain counters; a panicked writer cannot
    // leave them half-updated.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// [`DockLayoutBarrier::wait`] blocks until every write handed out before
/// the barrier was created has finished.
#[derive(Clone)]
pub struct DockLayoutBarrier(Arc<WriteLedger>);

impl DockLayoutBarrier {
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
pub struct DockLayoutWrite {
    store: Arc<dyn DockLayoutStore>,
    layout: DockLayoutSnapshot,
    generation: u64,
    ledger: Arc<WriteLedger>,
}

impl DockLayoutWrite {
    fn new(
        store: Arc<dyn DockLayoutStore>,
        layout: DockLayoutSnapshot,
        generation: u64,
        ledger: Arc<WriteLedger>,
    ) -> Self {
        *relock(&ledger.in_flight) += 1;
        Self {
            store,
            layout,
            generation,
            ledger,
        }
    }

    /// Write the layout unless a newer generation already reached the
    /// store; a stale write is a successful no-op, since the disk already
    /// holds something newer.
    pub fn run(self) -> Result<(), String> {
        let mut newest = relock(&self.ledger.newest_written);
        if self.generation <= *newest {
            return Ok(());
        }
        self.store.write(&self.layout)?;
        *newest = self.generation;
        Ok(())
    }
}

impl Drop for DockLayoutWrite {
    fn drop(&mut self) {
        let mut in_flight = relock(&self.ledger.in_flight);
        *in_flight -= 1;
        if *in_flight == 0 {
            self.ledger.idle.notify_all();
        }
    }
}

/// Change detection + debounce bookkeeping.
pub struct DockLayoutSync {
    store: Arc<dyn DockLayoutStore>,
    /// The layout believed to be on disk. `None` after a failed write,
    /// so the next observation always schedules a retry.
    saved: Option<DockLayoutSnapshot>,
    /// A changed layout waiting for its debounce to elapse.
    pending: Option<DockLayoutSnapshot>,
    generation: u64,
    ledger: Arc<WriteLedger>,
}

impl DockLayoutSync {
    pub fn new(store: Arc<dyn DockLayoutStore>, on_disk: DockLayoutSnapshot) -> Self {
        Self {
            store,
            saved: Some(on_disk),
            pending: None,
            generation: 0,
            ledger: Arc::default(),
        }
    }

    /// Record the layout after a state change. Returns the generation to
    /// arm a debounce timer for, or `None` when nothing needs writing.
    /// Every call that changes the target invalidates earlier timers.
    pub fn observe(&mut self, current: DockLayoutSnapshot) -> Option<u64> {
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
    pub fn take_if_current(&mut self, generation: u64) -> Option<DockLayoutWrite> {
        if generation != self.generation {
            return None;
        }
        self.take_pending()
    }

    /// Take whatever is pending now, ignoring the debounce (quit).
    pub fn flush(&mut self) -> Option<DockLayoutWrite> {
        self.take_pending()
    }

    /// A barrier over the writes handed out so far (quit waits on it for
    /// the write that is already running).
    pub fn barrier(&self) -> DockLayoutBarrier {
        DockLayoutBarrier(Arc::clone(&self.ledger))
    }

    /// A write failed: forget what is on disk so the next change retries.
    pub fn write_failed(&mut self) {
        self.saved = None;
    }

    fn take_pending(&mut self) -> Option<DockLayoutWrite> {
        let layout = self.pending.take()?;
        self.saved = Some(layout.clone());
        Some(DockLayoutWrite::new(
            Arc::clone(&self.store),
            layout,
            self.generation,
            Arc::clone(&self.ledger),
        ))
    }
}

impl std::fmt::Debug for DockLayoutSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockLayoutSync")
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
        written: Mutex<Vec<DockLayoutSnapshot>>,
    }

    impl DockLayoutStore for Recorder {
        fn write(&self, layout: &DockLayoutSnapshot) -> Result<(), String> {
            relock(&self.written).push(layout.clone());
            Ok(())
        }
    }

    fn layout(left_collapsed: bool, right_collapsed: bool) -> DockLayoutSnapshot {
        let region = |collapsed| RegionLayout {
            panels: vec![PanelKind::Projects],
            collapsed,
        };
        DockLayoutSnapshot {
            left: region(left_collapsed),
            right: region(right_collapsed),
            bottom: region(false),
        }
    }

    fn sync_with(store: Arc<dyn DockLayoutStore>) -> DockLayoutSync {
        DockLayoutSync::new(store, layout(false, false))
    }

    /// Two writes in flight, the older one scheduled last: the newer
    /// layout must be what the store ends up holding.
    #[test]
    fn an_older_write_landing_last_is_skipped() {
        // Arrange
        let recorder = Arc::new(Recorder::default());
        let mut sync = sync_with(recorder.clone());
        let older = sync.observe(layout(true, false)).expect("a change");
        let older_write = sync.take_if_current(older).expect("the older write");
        let newer = sync.observe(layout(true, true)).expect("a second change");
        let newer_write = sync.take_if_current(newer).expect("the newer write");

        // Act
        newer_write.run().expect("newer write");
        older_write.run().expect("older write");

        // Assert
        assert_eq!(*relock(&recorder.written), vec![layout(true, true)]);
    }

    /// In generation order both writes run, oldest first.
    #[test]
    fn writes_in_order_both_reach_the_store() {
        // Arrange
        let recorder = Arc::new(Recorder::default());
        let mut sync = sync_with(recorder.clone());
        let first = sync.observe(layout(true, false)).expect("a change");
        let first_write = sync.take_if_current(first).expect("first write");
        let second = sync.observe(layout(true, true)).expect("a second change");
        let second_write = sync.take_if_current(second).expect("second write");

        // Act
        first_write.run().expect("first write");
        second_write.run().expect("second write");

        // Assert
        assert_eq!(
            *relock(&recorder.written),
            vec![layout(true, false), layout(true, true)]
        );
    }

    /// Blocks inside `write` until let go, to hold a write "in flight".
    struct GatedStore {
        entered: Mutex<mpsc::Sender<()>>,
        proceed: Mutex<mpsc::Receiver<()>>,
    }

    impl DockLayoutStore for GatedStore {
        fn write(&self, _layout: &DockLayoutSnapshot) -> Result<(), String> {
            relock(&self.entered).send(()).expect("announce entry");
            relock(&self.proceed).recv().expect("wait to proceed");
            Ok(())
        }
    }

    /// Quit's barrier must not return while a write is still running.
    #[test]
    fn the_barrier_waits_for_the_write_in_flight() {
        // Arrange
        let (entered_tx, entered_rx) = mpsc::channel();
        let (proceed_tx, proceed_rx) = mpsc::channel();
        let store = Arc::new(GatedStore {
            entered: Mutex::new(entered_tx),
            proceed: Mutex::new(proceed_rx),
        });
        let mut sync = sync_with(store);
        let generation = sync.observe(layout(true, false)).expect("a change");
        let write = sync.take_if_current(generation).expect("the write");
        let running = std::thread::spawn(move || write.run());
        entered_rx.recv().expect("the write is inside the store");

        // Act
        let barrier = sync.barrier();
        let (waited_tx, waited_rx) = mpsc::channel();
        let waiter = std::thread::spawn(move || {
            barrier.wait();
            waited_tx.send(()).expect("announce the wait is over");
        });

        // Assert
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
