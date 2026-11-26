//! The tab's Re-sync lands: the regenerated diagram replaces the open one as
//! one undo step, so the person's unsaved edits stay in its history and Save
//! writes the result. Nothing is written by the run. When the diagram was
//! edited, closed or reopened while the run read the hierarchy, nothing is
//! applied and Messages says to run it again.

use std::path::PathBuf;

use signex_i18n::{Translatable, ids};
use signex_types::block_diagram::BlockDiagram;

use crate::app::Signex;

/// What a Re-sync merges into its open diagram.
#[derive(Debug, Clone)]
pub struct TabMerge {
    /// The open diagram's path.
    pub(crate) path: PathBuf,
    /// The diagram as the tab held it when the run started.
    pub(crate) base: BlockDiagram,
    /// The regenerated diagram; `None` when the run was refused.
    pub(crate) merged: Option<BlockDiagram>,
}

impl Signex {
    /// Apply `merge` to its open diagram, or say why not.
    pub(super) fn merge_into_tab(&mut self, merge: TabMerge) -> Result<(), Translatable> {
        let TabMerge { path, base, merged } = merge;
        let about = |id| Translatable::new(id).arg("path", path.display().to_string());
        let Some(merged) = merged else {
            return Ok(());
        };
        let depth = self.document_state.undo_depth;
        let editors = &mut self.document_state.block_diagrams.editors;
        let Some(editor) = editors.get_mut(&path) else {
            return Err(about(ids::PROJECT_BLOCK_DIAGRAM_RESYNC_CLOSED));
        };
        if editor.diagram != base {
            return Err(about(ids::PROJECT_BLOCK_DIAGRAM_RESYNC_CHANGED));
        }
        if merged != editor.diagram {
            editor.camera.hold(&editor.diagram);
            let before = std::mem::replace(&mut editor.diagram, merged);
            editor.record(before, depth);
            editor.prune_selection();
            self.document_state.sync_block_diagram_dirty(&path);
        }
        Ok(())
    }
}
