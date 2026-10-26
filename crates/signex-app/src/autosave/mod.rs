//! Opt-in autosave of dirty schematics to a recovery sidecar (#104).
//!
//! Off by default (Preferences > Editing). When on, a timer subscription
//! asks every [`AUTOSAVE_INTERVAL`] for a recovery copy of each dirty
//! schematic. The copy goes to a [`sidecar`] file; the user's real file is
//! never touched, and nothing runs while the preference is off.
//!
//! ADR-0003 checklist for this deferred work:
//!
//! 1. Owed/in-flight state is [`AutosaveState`] in `DocumentState`: per
//!    path, the [`DocStamp`] last written and whether a write is in flight.
//! 2. No boundary needs a flush: a normal save supersedes any in-flight
//!    write and removes the sidecar (see `handlers/autosave.rs`).
//! 3. A sidecar is a serialised snapshot taken in `update`, never an undo
//!    snapshot.
//! 4. A completion removes its own sidecar when the document was saved or
//!    closed meanwhile, so a late write cannot resurrect stale content.
//! 5. A panic or dropped task maps to `Err` in [`AutosaveDone`], is
//!    reported once to the Messages panel, and frees the in-flight slot.
//! 6. Every disk write goes through `atomic_write`.

pub mod sidecar;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use iced::Subscription;
use serde::Serialize;

/// How often the autosave timer fires while the preference is on.
pub const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);

/// The revision of a document a sidecar was taken at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DocStamp {
    pub revision: u64,
    pub net_color_revision: u64,
}

/// Autosave messages. Namespaced under `Message::Autosave`.
#[derive(Debug, Clone, Serialize)]
pub enum AutosaveMsg {
    /// The autosave timer fired.
    Ticked,
    /// A background sidecar write finished.
    Written(AutosaveDone),
    /// A background sidecar removal finished; the error text is what the
    /// Messages panel shows.
    Discarded(Result<(), String>),
}

/// Completion of one sidecar write.
#[derive(Debug, Clone, Serialize)]
pub struct AutosaveDone {
    pub path: PathBuf,
    pub stamp: DocStamp,
    pub result: Result<(), String>,
}

/// What autosave owes and has done, per document path.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AutosaveState {
    dir: Option<PathBuf>,
    /// Documents with a sidecar on disk, and the stamp it holds.
    written: HashMap<PathBuf, DocStamp>,
    /// Documents with a write running; the flag is "a normal save landed
    /// meanwhile, so the result must not be kept".
    in_flight: HashMap<PathBuf, bool>,
    /// The last failure shown, so a failing disk is reported once rather
    /// than every tick.
    last_error: Option<String>,
}

/// The recovery directory under the per-user config root, if the platform
/// has one.
pub fn default_recovery_dir() -> Option<PathBuf> {
    crate::config_root::config_root().map(|root| root.join("recovery"))
}

impl AutosaveState {
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self {
            dir,
            ..Self::default()
        }
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// Point autosave at another recovery directory.
    pub fn set_dir(&mut self, dir: Option<PathBuf>) {
        self.dir = dir;
    }

    /// Does `path` at `stamp` need a new sidecar?
    pub fn needs_write(&self, path: &Path, stamp: DocStamp) -> bool {
        !self.in_flight.contains_key(path) && self.written.get(path) != Some(&stamp)
    }

    pub fn begin(&mut self, path: &Path) {
        self.in_flight.insert(path.to_path_buf(), false);
    }

    /// A write finished. Returns true when it was superseded by a save.
    pub fn finish(&mut self, path: &Path, stamp: DocStamp, ok: bool) -> bool {
        let superseded = self.in_flight.remove(path).unwrap_or(false);
        if ok && !superseded {
            self.written.insert(path.to_path_buf(), stamp);
        }
        superseded
    }

    /// A normal save landed: any write running now is stale. Returns true
    /// when a sidecar is on disk and must be removed.
    pub fn saved(&mut self, path: &Path) -> bool {
        if let Some(superseded) = self.in_flight.get_mut(path) {
            *superseded = true;
        }
        self.written.remove(path).is_some()
    }

    /// Stop tracking `path`'s sidecar. Returns true if one was on disk.
    pub fn forget(&mut self, path: &Path) -> bool {
        self.written.remove(path).is_some()
    }

    /// Paths with a sidecar for which `is_live` says there is nothing left
    /// to recover (saved, closed or discarded).
    pub fn stale(&self, is_live: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
        self.written
            .keys()
            .filter(|p| !is_live(p))
            .cloned()
            .collect()
    }

    /// Record a failure; true when it differs from the last one shown.
    pub fn should_report(&mut self, error: &str) -> bool {
        if self.last_error.as_deref() == Some(error) {
            return false;
        }
        self.last_error = Some(error.to_string());
        true
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
    }
}

/// The timer; empty while autosave is off.
pub fn subscription(enabled: bool) -> Subscription<AutosaveMsg> {
    if enabled {
        iced::time::every(AUTOSAVE_INTERVAL).map(|_| AutosaveMsg::Ticked)
    } else {
        Subscription::none()
    }
}
