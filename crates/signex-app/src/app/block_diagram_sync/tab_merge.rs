//! The tab's Re-sync lands: the regenerated diagram replaces the open one as
//! one undo step, so the person's unsaved edits stay in its history and Save
//! writes the result. Nothing is written by the run. When the diagram was
//! edited, closed or reopened while the run read the hierarchy, nothing is
//! applied and Messages says to run it again.
//!
//! The run leaves unlinking out (mockup row 210): the added and the renamed
//! or moved blocks apply at once, and a block whose sheet left the project
//! opens the review side sheet instead (`block_diagram::review`).

use std::path::PathBuf;

use signex_i18n::{Translatable, ids};
use signex_types::block_diagram::{BlockDiagram, DiagramChange, LinkState, Regenerated};

use crate::app::Signex;
use crate::app::core::block_diagram_panes::{ResyncReview, UnlinkRow};

/// What a Re-sync merges into its open diagram.
#[derive(Debug, Clone)]
pub struct TabMerge {
    /// The open diagram's path.
    pub(crate) path: PathBuf,
    /// The diagram as the tab held it when the run started.
    pub(crate) base: BlockDiagram,
    /// The regenerated diagram and its changes; `None` when the run was
    /// refused.
    pub(crate) merged: Option<Regenerated>,
}

impl Signex {
    /// Apply `merge` to its open diagram, or say why not; opens the review
    /// when a block would become unlinked.
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
        let review = review_of(&merged);
        let had_review = editor.review.is_some();
        if merged.diagram != editor.diagram {
            editor.camera.hold(&editor.diagram);
            let before = std::mem::replace(&mut editor.diagram, merged.diagram);
            editor.record(before, depth);
            editor.prune_selection();
        }
        // The review opens only for an unlink; one already open shows
        // "Nothing to change" when the run found none.
        if !review.unlink.is_empty() || had_review {
            editor.review = Some(review);
        }
        self.document_state.sync_block_diagram_dirty(&path);
        Ok(())
    }
}

/// The review of a run: what it applied, and a checked row for each block
/// it left for unlinking.
fn review_of(run: &Regenerated) -> ResyncReview {
    let mut review = ResyncReview::default();
    for change in &run.changes {
        match change {
            DiagramChange::BlockAdded { block, .. } => review.added.push(*block),
            DiagramChange::SheetMoved {
                block, from, to, ..
            } if !review.moved.contains(block) => {
                review.moved.push(*block);
                // A changed file name is a rename; only the folder, a move.
                let file = |path: &str| path.rsplit(['/', '\\']).next().unwrap_or("").to_string();
                if file(from) != file(to) {
                    review.renamed.push(*block);
                }
            }
            _ => {}
        }
    }
    for change in &run.skipped {
        if let DiagramChange::SheetRemoved { block, path, .. } = change {
            let name = run
                .diagram
                .block(*block)
                .map_or_else(String::new, |b| b.name.clone());
            review.unlink.push(UnlinkRow {
                block: *block,
                name,
                path: path.clone(),
                checked: true,
            });
        }
    }
    review.unchanged = run
        .diagram
        .blocks
        .iter()
        .filter(|b| {
            b.sheet
                .as_ref()
                .is_some_and(|l| l.state == LinkState::Linked)
                && !review.added.contains(&b.id)
                && !review.moved.contains(&b.id)
                && !review.unlink.iter().any(|row| row.block == b.id)
        })
        .count();
    review
}
