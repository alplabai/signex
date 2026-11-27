//! Live ERC (#1288, Caner 2026-11-25): on by default and quiet. After an
//! edit settles, the ERC runs in the background and the findings on the nets
//! the edit changed are replaced (`signex_erc::live`); the canvas markers and
//! the counts follow. Run ERC stays the full check, and the `live_erc`
//! preference turns this off. Rules: `docs/ERC_RULES.md`, "Live ERC".

mod job;
mod run;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use signex_types::schematic::SchematicSheet;

pub(crate) use job::LiveErcOutcome;

/// How long the sheets must stay unedited before a live check starts.
pub(crate) const LIVE_ERC_DEBOUNCE: Duration = Duration::from_millis(400);

/// A sheet as the last live check read it; `None` when it was first seen in
/// the update that edited it, so the next check takes all its findings.
pub(crate) type Baseline = Option<Arc<SchematicSheet>>;

/// Live ERC's messages, under `ErcMsg::Live`.
#[derive(Debug, Clone)]
pub enum LiveErcMsg {
    /// No edit for [`LIVE_ERC_DEBOUNCE`] since the wait `token` began.
    DebounceElapsed(u64),
    /// A live check finished; applied only if `generation` is still newest.
    Finished {
        generation: u64,
        result: Arc<Result<LiveErcOutcome, String>>,
    },
    /// The `toggle_live_erc` command from a key, the palette or a menu: the
    /// preference flips.
    Toggled,
}

/// Live ERC's state in the Model.
#[derive(Debug)]
pub(crate) struct LiveErc {
    /// The `live_erc` preference; on by default.
    pub enabled: bool,
    /// False in a headless run: it draws no markers, and `check_erc` is its
    /// check.
    pub runs_here: bool,
    /// The sheet edit epoch the last observed edit left.
    seen_epoch: u64,
    /// The newest debounce wait, while one is owed.
    pending: Option<u64>,
    /// Token of the newest debounce wait.
    debounce: u64,
    /// Token of the newest live check; a stale result is dropped.
    generation: u64,
    /// A live check is in flight.
    running: bool,
    /// Each open sheet as the last applied check read it.
    pub baselines: HashMap<PathBuf, Baseline>,
}

impl LiveErc {
    /// The state at launch, with the saved preference.
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            runs_here: true,
            seen_epoch: 0,
            pending: None,
            debounce: 0,
            generation: 0,
            running: false,
            baselines: HashMap::new(),
        }
    }

    /// Whether live checks run: the preference is on and there is a window.
    pub(crate) fn is_active(&self) -> bool {
        self.enabled && self.runs_here
    }

    /// Note the sheet edit epoch after an update: a new edit starts (or
    /// restarts) the wait. Returns whether `epoch` is a new edit.
    pub(crate) fn observe(&mut self, epoch: u64) -> bool {
        if epoch == self.seen_epoch {
            return false;
        }
        self.seen_epoch = epoch;
        if self.is_active() {
            self.debounce += 1;
            self.pending = Some(self.debounce);
        }
        true
    }

    /// The wait the subscription times, if any.
    pub(crate) fn pending(&self) -> Option<u64> {
        self.pending
    }

    /// The wait `token` ended: true when it is still the newest one, which
    /// it then settles.
    pub(crate) fn take_elapsed(&mut self, token: u64) -> bool {
        if self.pending != Some(token) {
            return false;
        }
        self.pending = None;
        true
    }

    /// Wait again: a full run was in flight when the wait ended.
    pub(crate) fn rearm(&mut self) {
        self.debounce += 1;
        self.pending = Some(self.debounce);
    }

    /// A live check starts; its token.
    pub(crate) fn begin(&mut self) -> u64 {
        self.generation += 1;
        self.running = true;
        self.generation
    }

    /// A full ERC run started: a live check still out is dropped.
    pub(crate) fn supersede(&mut self) {
        self.generation += 1;
        self.running = false;
    }

    /// A live check `generation` finished: true when it is the newest.
    pub(crate) fn finish(&mut self, generation: u64) -> bool {
        if generation != self.generation {
            return false;
        }
        self.running = false;
        true
    }

    /// A live check is in flight.
    pub(crate) fn is_running(&self) -> bool {
        self.running
    }

    /// Turned off: nothing is owed and a check still out is dropped.
    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.pending = None;
            self.supersede();
            self.baselines.clear();
        }
    }
}
