//! The Re-sync review (mockup row 210): the side sheet a Re-sync opens when
//! a block would become unlinked. Ticking and unticking is view state;
//! Apply selected unlinks the checked blocks as one undo step
//! (`signex_types::block_diagram::unlink_blocks`) and says so in Messages,
//! with Undo. An unchecked row is not remembered: the next Re-sync lists it
//! again, checked. Cancel leaves the diagram as it is.

use std::path::Path;

use iced::Task;
use signex_i18n::{ids, t};
use signex_types::block_diagram::unlink_blocks;
use uuid::Uuid;

use crate::app::{Message, Signex};

/// What happened in the review side sheet.
#[derive(Debug, Clone, PartialEq)]
pub enum ReviewEvent {
    /// A row ticked (`true`) or unticked.
    RowToggled(Uuid, bool),
    /// The Unlinked group's box: every row on or off.
    AllToggled(bool),
    /// The pointer entered a row (its block) or left it (`None`): the
    /// block is outlined on the diagram while it is over the row.
    RowHovered(Option<Uuid>),
    /// Cancel, the close button or Close.
    Closed,
    ApplyPressed,
}

impl Signex {
    pub(super) fn block_diagram_review_event(
        &mut self,
        path: &Path,
        event: ReviewEvent,
    ) -> Task<Message> {
        if event == ReviewEvent::ApplyPressed {
            self.apply_review(path);
            return Task::none();
        }
        let Some(tab) = self.open_diagram_view_mut(path) else {
            return Task::none();
        };
        let Some(review) = tab.review.as_mut() else {
            return Task::none();
        };
        match event {
            ReviewEvent::RowToggled(block, on) => {
                if let Some(row) = review.unlink.iter_mut().find(|row| row.block == block) {
                    row.checked = on;
                }
            }
            ReviewEvent::AllToggled(on) => {
                review.unlink.iter_mut().for_each(|row| row.checked = on);
            }
            ReviewEvent::RowHovered(block) => review.hovered = block,
            ReviewEvent::Closed | ReviewEvent::ApplyPressed => tab.review = None,
        }
        Task::none()
    }

    /// Unlink the checked blocks as one undo step and close the review.
    fn apply_review(&mut self, path: &Path) {
        let depth = self.document_state.undo_depth;
        let Some(editor) = self.document_state.block_diagrams.editors.get_mut(path) else {
            return;
        };
        let tab = super::view_of(&mut self.document_view.block_diagrams, path);
        let Some(review) = tab.review.take() else {
            return;
        };
        if editor.is_read_only() {
            return;
        }
        let checked = review.checked();
        let applied = unlink_blocks(&editor.diagram, &checked);
        if applied.diagram != editor.diagram {
            tab.camera.hold(&editor.diagram);
            let before = std::mem::replace(&mut editor.diagram, applied.diagram);
            editor.record(before, depth);
            tab.prune(&editor.diagram);
        }
        self.document_state.sync_block_diagram_dirty(path);
        if checked.is_empty() {
            return;
        }
        let line = t!(
            self.ui_state.i18n,
            ids::BLOCK_DIAGRAM_REVIEW_APPLIED,
            "blocks" => checked.len(),
            "file" => super::file_name(path)
        );
        crate::diagnostics::log_info(line.into_string());
    }
}
