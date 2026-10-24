//! Debounced, change-detected persistence of the dock layout (#692).
//!
//! `update` used to rewrite `prefs.json` (read + parse + fsync + rename)
//! after every `DockMessage`, hover and per-tick floating-panel moves
//! included. This module is the pure half of the fix: it decides *whether*
//! a write is needed and *when*, and hands back a [`DockLayoutWrite`] job.
//! It performs no IO itself. The app turns the job into a `Task`, so the
//! write runs off the UI thread, and the [`DockLayoutStore`] seam lets a
//! test count writes without touching a disk.

use std::time::Duration;

use super::{DockArea, PanelPosition};
use crate::panels::PanelKind;
use crate::persist::{DebouncedStore, DebouncedSync, PendingWrite, WriteBarrier};

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

impl<S: DockLayoutStore> DebouncedStore<DockLayoutSnapshot> for S {
    fn write(&self, layout: &DockLayoutSnapshot) -> Result<(), String> {
        DockLayoutStore::write(self, layout)
    }
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

/// Quit waits on this for the dock write that is already running.
pub type DockLayoutBarrier = WriteBarrier;

/// One dock-layout write to run off the UI thread.
pub type DockLayoutWrite = PendingWrite<DockLayoutSnapshot>;

/// Change detection + debounce bookkeeping for the dock layout.
pub type DockLayoutSync = DebouncedSync<DockLayoutSnapshot>;

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Default)]
    struct Recorder {
        written: Mutex<Vec<DockLayoutSnapshot>>,
    }

    impl DockLayoutStore for Recorder {
        fn write(&self, layout: &DockLayoutSnapshot) -> Result<(), String> {
            self.written
                .lock()
                .expect("recorder lock")
                .push(layout.clone());
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

    /// The dock store trait feeds the generic loop: an older write landing
    /// last is skipped, so the newer layout is what the store holds.
    #[test]
    fn an_older_dock_write_landing_last_is_skipped() {
        let recorder = Arc::new(Recorder::default());
        let mut sync = DockLayoutSync::new(recorder.clone(), layout(false, false));
        let older = sync.observe(layout(true, false)).expect("a change");
        let older_write = sync.take_if_current(older).expect("the older write");
        let newer = sync.observe(layout(true, true)).expect("a second change");
        let newer_write = sync.take_if_current(newer).expect("the newer write");

        newer_write.run().expect("newer write");
        older_write.run().expect("older write");

        assert_eq!(
            *recorder.written.lock().expect("recorder lock"),
            vec![layout(true, true)]
        );
    }
}
