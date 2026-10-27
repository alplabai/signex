//! Per-directory case answers for file IDENTITY (#1447).
//!
//! Whether `Lib.snxsch` and `lib.snxsch` are one file is a property of the
//! volume, so it comes from a probe. The probe is disk IO and runs off the
//! update thread; this holds what it reported, which probes are in flight, and
//! the opens and saves that wait for one. Until a directory is answered its
//! files are treated as folding: never two documents for one possible file.
//! `project_case` keeps the same split between key and identity.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::project_case::CaseProbeOutcome;

/// What the identity probes of loose directories reported, and what waits.
#[derive(Debug, Default)]
pub struct DirCases {
    /// The first answer per directory, decided or not.
    answers: HashMap<PathBuf, CaseProbeOutcome>,
    /// Directories whose probe has been started and not yet answered.
    inflight: HashSet<PathBuf>,
    /// Opens held back until their directory answers.
    deferred_opens: Vec<PathBuf>,
    /// Saves held back until their directory answers.
    deferred_saves: Vec<PathBuf>,
    /// Directories already told about a fold assumed for want of an answer.
    assumed_noted: HashSet<PathBuf>,
}

impl DirCases {
    pub(crate) fn answer(&self, dir: &Path) -> Option<&CaseProbeOutcome> {
        self.answers.get(dir)
    }

    /// True when `dir` is neither answered nor being probed, and is now marked
    /// in flight: the caller must start its probe.
    pub(crate) fn begin_probe(&mut self, dir: &Path) -> bool {
        !self.answers.contains_key(dir) && self.inflight.insert(dir.to_path_buf())
    }

    /// Store the first answer for `dir`; false when it already had one.
    pub(crate) fn record(&mut self, dir: &Path, outcome: CaseProbeOutcome) -> bool {
        self.inflight.remove(dir);
        if self.answers.contains_key(dir) {
            return false;
        }
        self.answers.insert(dir.to_path_buf(), outcome);
        true
    }

    pub(crate) fn defer_open(&mut self, path: PathBuf) {
        if !self.deferred_opens.contains(&path) {
            self.deferred_opens.push(path);
        }
    }

    /// Whether an open of `path` is parked waiting for its directory.
    pub(crate) fn is_deferred_open(&self, path: &Path) -> bool {
        self.deferred_opens.iter().any(|parked| parked == path)
    }

    /// Whether a probe of `dir` has been started and not answered.
    #[cfg(test)]
    pub(crate) fn is_probing(&self, dir: &Path) -> bool {
        self.inflight.contains(dir)
    }

    pub(crate) fn defer_save(&mut self, path: PathBuf) {
        if !self.deferred_saves.contains(&path) {
            self.deferred_saves.push(path);
        }
    }

    pub(crate) fn take_deferred_opens(&mut self, dir: &Path) -> Vec<PathBuf> {
        take_under(&mut self.deferred_opens, dir)
    }

    pub(crate) fn take_deferred_saves(&mut self, dir: &Path) -> Vec<PathBuf> {
        take_under(&mut self.deferred_saves, dir)
    }

    /// True the first time it is asked about `dir`.
    pub(crate) fn note_assumed_once(&mut self, dir: &Path) -> bool {
        self.assumed_noted.insert(dir.to_path_buf())
    }
}

/// Remove and return the paths whose parent directory is `dir`.
fn take_under(list: &mut Vec<PathBuf>, dir: &Path) -> Vec<PathBuf> {
    let (mine, rest) = std::mem::take(list)
        .into_iter()
        .partition(|path| path.parent() == Some(dir));
    *list = rest;
    mine
}
