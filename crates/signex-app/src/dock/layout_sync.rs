//! Debounced, change-detected persistence of the dock layout (#692).
//!
//! `update` used to rewrite `prefs.json` (read + parse + fsync + rename)
//! after every `DockMessage`, hover and per-tick floating-panel moves
//! included. This module is the pure half of the fix: it decides *whether*
//! a write is needed and *when*, and hands back a [`DockLayoutWrite`] job.
//! It performs no IO itself. The app turns the job into a `Task`, so the
//! write runs off the UI thread, and the [`DockLayoutStore`] seam lets a
//! test count writes without touching a disk.

use std::sync::Arc;
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

/// One write to run off the UI thread.
pub struct DockLayoutWrite {
    store: Arc<dyn DockLayoutStore>,
    layout: DockLayoutSnapshot,
}

impl DockLayoutWrite {
    pub fn run(self) -> Result<(), String> {
        self.store.write(&self.layout)
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
}

impl DockLayoutSync {
    pub fn new(store: Arc<dyn DockLayoutStore>, on_disk: DockLayoutSnapshot) -> Self {
        Self {
            store,
            saved: Some(on_disk),
            pending: None,
            generation: 0,
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

    /// A write failed: forget what is on disk so the next change retries.
    pub fn write_failed(&mut self) {
        self.saved = None;
    }

    fn take_pending(&mut self) -> Option<DockLayoutWrite> {
        let layout = self.pending.take()?;
        self.saved = Some(layout.clone());
        Some(DockLayoutWrite {
            store: Arc::clone(&self.store),
            layout,
        })
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
