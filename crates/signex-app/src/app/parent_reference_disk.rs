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
use crate::app::state::library_entry::with_new_leaf;
use crate::app::state::scope::{CaseFold, path_key_with};
use crate::persist::SharedBytes;

/// What re-pointing one parent on disk did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParentDisk {
    /// The saved parent was rewritten with the new filename; holds the exact
    /// bytes written, so the follow-up git commit records them.
    Written(SharedBytes),
    /// The saved parent has no reference to change; nothing was written.
    NothingToChange,
    /// The saved parent could not be changed (unreadable, written by a newer
    /// Signex, or the write failed); it is left untouched and the reason is
    /// kept. The reference then waits for a save of the in-memory parent.
    Skipped(String),
}

impl ParentDisk {
    pub fn is_written(&self) -> bool {
        matches!(self, Self::Written(_))
    }
}

/// Parent path to its disk outcome.
pub type ParentDiskOutcomes = HashMap<PathBuf, ParentDisk>;

/// The sheet file being renamed: every saved child entry of a candidate that
/// resolves to `old` is re-pointed at `new_name`, whether or not an open
/// engine still holds that symbol (a symbol deleted in memory but not yet
/// saved still names the file on disk).
pub(crate) struct Renamed<'a> {
    pub old: &'a Path,
    pub new_name: &'a str,
    pub fold: CaseFold,
}

/// Re-point the references of every candidate parent, one read and one write
/// per parent. `rewrites` are the planned string-level changes; a candidate is
/// also scanned for saved entries that resolve to the renamed file.
pub(crate) fn patch_parents(
    candidates: &[PathBuf],
    rewrites: &[ReferenceRewrite],
    renamed: &Renamed<'_>,
) -> ParentDiskOutcomes {
    candidates
        .iter()
        .map(|parent| {
            let planned: Vec<&ReferenceRewrite> =
                rewrites.iter().filter(|r| &r.parent == parent).collect();
            (parent.clone(), patch_parent(parent, &planned, renamed))
        })
        .collect()
}

fn patch_parent(parent: &Path, planned: &[&ReferenceRewrite], renamed: &Renamed<'_>) -> ParentDisk {
    let text = match std::fs::read_to_string(parent) {
        Ok(text) => text,
        Err(e) => return ParentDisk::Skipped(format!("could not be read: {e}")),
    };
    let mut parsed = match signex_types::format::SnxSchematic::parse(&text) {
        Ok(parsed) => parsed,
        Err(e) => return ParentDisk::Skipped(format!("could not be parsed: {e}")),
    };
    let parent_dir = parent.parent().unwrap_or_else(|| Path::new(""));
    let old_key = path_key_with(renamed.old, renamed.fold);
    let mut changed = false;
    for child in parsed.sheet.child_sheets.iter_mut() {
        let resolves = path_key_with(&parent_dir.join(&child.filename), renamed.fold) == old_key;
        let planned_to = planned
            .iter()
            .find(|r| r.from == child.filename)
            .map(|r| r.to.clone());
        let new = planned_to
            .or_else(|| resolves.then(|| with_new_leaf(&child.filename, renamed.new_name)));
        if let Some(new) = new {
            child.filename = new;
            changed = true;
        }
    }
    if !changed {
        return ParentDisk::NothingToChange;
    }
    if parsed.is_from_newer_minor() {
        return ParentDisk::Skipped("was written by a newer Signex and is read-only".to_string());
    }
    let bytes = match parsed.write_string() {
        Ok(text) => text.into_bytes(),
        Err(e) => return ParentDisk::Skipped(format!("could not be encoded: {e}")),
    };
    match signex_types::atomic_io::atomic_write(parent, &bytes) {
        Ok(()) => ParentDisk::Written(std::sync::Arc::new(bytes)),
        Err(e) => ParentDisk::Skipped(format!("could not be written: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_holds_the_exact_bytes_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("Root.snxsch");
        let child = |name: &str| signex_types::schematic::ChildSheet {
            uuid: uuid::Uuid::now_v7(),
            name: name.to_string(),
            filename: name.to_string(),
            position: signex_types::schematic::Point::new(0.0, 0.0),
            size: (10.0, 10.0),
            stroke_width: 0.0,
            fill: signex_types::schematic::FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        };
        let sheet = signex_types::schematic::SchematicSheet {
            child_sheets: vec![child("a.snxsch")],
            ..Default::default()
        };
        let text = signex_types::format::SnxSchematic::new(sheet)
            .write_string()
            .unwrap();
        std::fs::write(&parent, text).unwrap();
        let renamed = Renamed {
            old: &dir.path().join("a.snxsch"),
            new_name: "b.snxsch",
            fold: CaseFold::PRESERVE,
        };

        let out = patch_parents(std::slice::from_ref(&parent), &[], &renamed);

        let ParentDisk::Written(bytes) = &out[&parent] else {
            panic!("{out:?}");
        };
        assert_eq!(**bytes, std::fs::read(&parent).unwrap());
        assert!(String::from_utf8_lossy(bytes).contains("b.snxsch"));
    }
}
