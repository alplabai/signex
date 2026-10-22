//! Child-sheet references for ownership checks, resolved from the open
//! projects' own sheet lists rather than only from the loaded engines.
//!
//! A sibling's dirty child sheet whose parent tab was closed has no parent
//! engine to point at it; reading the unloaded parents from disk keeps that
//! child attributable to its project (#741).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::DocumentState;
use super::scope::{CaseFold, path_key_with};

/// Child references per sheet path, plus one message per sheet that exists but
/// could not be read or parsed (its children are unknown).
pub(crate) struct SheetRefs {
    pub(crate) refs: HashMap<PathBuf, Vec<String>>,
    pub(crate) unreadable: Vec<String>,
}

impl DocumentState {
    /// [`child_sheet_refs`](Self::child_sheet_refs) extended with every sheet
    /// reachable from each open project's listed sheets. A loaded engine wins
    /// over disk, so unsaved edits are honoured.
    ///
    /// Reads only unloaded sheets, and only on a Close Project click — a few
    /// small files, not worth a cache that edits would have to invalidate.
    pub(crate) fn child_sheet_refs_from_projects(&self) -> SheetRefs {
        let fold = CaseFold::host_default();
        let mut refs = self.child_sheet_refs();
        let mut unreadable = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut pending: Vec<PathBuf> = self
            .projects
            .iter()
            .flat_map(|p| {
                let dir = p.dir();
                p.data.sheets.iter().map(move |s| dir.join(&s.filename))
            })
            .collect();
        while let Some(path) = pending.pop() {
            if !seen.insert(path_key_with(&path, fold)) {
                continue;
            }
            let children = match refs.get(&path) {
                Some(children) => children.clone(),
                None => {
                    let children = read_child_refs(&path, &mut unreadable);
                    refs.insert(path.clone(), children.clone());
                    children
                }
            };
            let dir = path.parent().unwrap_or_else(|| Path::new(""));
            pending.extend(children.iter().map(|c| dir.join(c)));
        }
        SheetRefs { refs, unreadable }
    }
}

/// `child_sheets` filenames of the sheet at `path`. A missing file has none;
/// an unreadable or unparsable one is reported and treated as having none.
fn read_child_refs(path: &Path, unreadable: &mut Vec<String>) -> Vec<String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            unreadable.push(format!("{} could not be read: {e}", path.display()));
            return Vec::new();
        }
    };
    match signex_types::format::SnxSchematic::parse(&text) {
        Ok(parsed) => parsed
            .sheet
            .child_sheets
            .into_iter()
            .map(|c| c.filename)
            .collect(),
        Err(e) => {
            unreadable.push(format!("{} could not be parsed: {e}", path.display()));
            Vec::new()
        }
    }
}
