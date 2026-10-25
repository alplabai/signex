//! A route-independent fingerprint of one open document's in-memory content
//! (#859). A History restore records it when it starts and compares it when
//! it finishes: any difference means the user edited the document meanwhile,
//! whichever route the edit took, and the restore must not replace it.

use std::hash::{Hash, Hasher};

use super::super::super::*;
use crate::app::restore_job::EditStamp;

fn feed<T: serde::Serialize>(hasher: &mut impl Hasher, value: &T) {
    // A value that fails to serialize hashes as empty and then compares equal
    // to itself; the engine revision and editor undo depth hashed alongside
    // still move on an edit.
    serde_json::to_vec(value).unwrap_or_default().hash(hasher);
}

impl Signex {
    /// Hash of everything an edit to `path` can change in memory: the engine
    /// document and its colours, the open editor's container and undo depth,
    /// and a PCB tab's board. Dirty flags are left out: saving unchanged
    /// content clears them and is not an edit.
    pub(crate) fn edit_stamp(&self, path: &std::path::Path) -> EditStamp {
        let ds = &self.document_state;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        if let Some(engine) = ds.engines.get(path) {
            feed(&mut hasher, engine.document());
            engine.revision().hash(&mut hasher);
            self.net_color_revision_of(path).hash(&mut hasher);
            let mut colours: Vec<_> = self.ui_state.wire_color_overrides.iter().collect();
            colours.sort_by_key(|(id, _)| **id);
            feed(&mut hasher, &colours);
        }
        if let Some(editor) = ds.symbol_editors.get(path) {
            feed(&mut hasher, &editor.file);
            editor.undo_snapshots.len().hash(&mut hasher);
        }
        if let Some(editor) = ds.footprint_editors.get(path) {
            feed(&mut hasher, &editor.file);
            editor.history.len().hash(&mut hasher);
        }
        for tab in ds.tabs.iter().filter(|t| t.path == path) {
            if let Some(crate::app::TabDocument::Pcb(board)) = &tab.cached_document {
                feed(&mut hasher, board);
            }
        }
        EditStamp(hasher.finish())
    }
}
