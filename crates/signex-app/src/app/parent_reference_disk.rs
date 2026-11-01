//! Re-pointing sheet-symbol filenames in parent sheets on disk (#1849).
//!
//! A sheet rename (or a Save As that re-points the project) changes which
//! file the project names. The parent sheets that place the child by filename
//! have to follow in the same job: otherwise Discard All, a quit without
//! saving or a crash leaves a parent naming a file that is gone, and the
//! reopened hierarchy shows a missing child.
//!
//! The job re-reads the saved parent, changes only the child-sheet filename
//! field, and writes it back atomically, so the other unsaved edits of an open
//! parent are never written: they stay in memory and Discard All still
//! reverts them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::app::path_ops::ReferenceRewrite;

/// What re-pointing one parent on disk did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParentDisk {
    /// The saved parent was rewritten with the new filename.
    Written,
    /// The saved parent has no reference to change; nothing was written.
    NothingToChange,
    /// The saved parent could not be changed (unreadable, written by a newer
    /// Signex, or the write failed); it is left untouched and the reason is
    /// kept. The reference then waits for a save of the in-memory parent.
    Skipped(String),
}

/// Parent path to its disk outcome.
pub type ParentDiskOutcomes = HashMap<PathBuf, ParentDisk>;

/// Re-point every reference in `rewrites`, one read and one write per parent.
pub(crate) fn patch_parents(rewrites: &[ReferenceRewrite]) -> ParentDiskOutcomes {
    let mut by_parent: HashMap<&Path, Vec<&ReferenceRewrite>> = HashMap::new();
    for rewrite in rewrites {
        by_parent.entry(&rewrite.parent).or_default().push(rewrite);
    }
    by_parent
        .into_iter()
        .map(|(parent, rewrites)| (parent.to_path_buf(), patch_parent(parent, &rewrites)))
        .collect()
}

fn patch_parent(parent: &Path, rewrites: &[&ReferenceRewrite]) -> ParentDisk {
    let text = match std::fs::read_to_string(parent) {
        Ok(text) => text,
        Err(e) => return ParentDisk::Skipped(format!("could not be read: {e}")),
    };
    let mut parsed = match signex_types::format::SnxSchematic::parse(&text) {
        Ok(parsed) => parsed,
        Err(e) => return ParentDisk::Skipped(format!("could not be parsed: {e}")),
    };
    if parsed.is_from_newer_minor() {
        return ParentDisk::Skipped("was written by a newer Signex and is read-only".to_string());
    }
    let mut changed = false;
    for child in parsed.sheet.child_sheets.iter_mut() {
        if let Some(rewrite) = rewrites.iter().find(|r| r.from == child.filename) {
            child.filename = rewrite.to.clone();
            changed = true;
        }
    }
    if !changed {
        return ParentDisk::NothingToChange;
    }
    let bytes = match parsed.write_string() {
        Ok(text) => text.into_bytes(),
        Err(e) => return ParentDisk::Skipped(format!("could not be encoded: {e}")),
    };
    match signex_types::atomic_io::atomic_write(parent, &bytes) {
        Ok(()) => ParentDisk::Written,
        Err(e) => ParentDisk::Skipped(format!("could not be written: {e}")),
    }
}
