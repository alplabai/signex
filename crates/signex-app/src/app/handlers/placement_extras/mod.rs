//! Placement extras on a sheet (#1667): Autoplace Fields, Copy Attributes,
//! Place Design Block and Import Graphics.
//!
//! A design block and imported graphics land on the cursor the way a paste
//! does: the file is read off the UI thread, the content follows the cursor,
//! turns with Space and mirrors with X / Y, and one click places it as one
//! undo step. A read that comes back after Esc, a tool change or a sheet
//! switch is dropped and said in the Messages panel.

mod autoplace;
mod copy_attributes;
mod design_block;
mod import_graphics;

pub(crate) use import_graphics::read_import_text;

use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::CopiedSelection;
use signex_types::schematic::Point;

use crate::app::state::PlacementKind;
use crate::app::{Message, PlacementExtrasMsg, Signex};

impl Signex {
    pub(crate) fn dispatch_placement_extras(&mut self, msg: PlacementExtrasMsg) -> Task<Message> {
        let task = match msg {
            PlacementExtrasMsg::AutoplaceFieldsRequested => {
                self.autoplace_selected_fields();
                Task::none()
            }
            PlacementExtrasMsg::CopyAttributesRequested { at } => self.copy_attributes_at(at),
            PlacementExtrasMsg::DesignBlockRequested => self.design_block_dialog(),
            PlacementExtrasMsg::DesignBlockPicked(path) => self.read_design_block(path),
            PlacementExtrasMsg::DesignBlockRead {
                path,
                target,
                epoch,
                sheet,
            } => {
                self.design_block_read(&path, &target, epoch, sheet);
                Task::none()
            }
            PlacementExtrasMsg::ImportGraphicsRequested => self.import_graphics_dialog(),
            PlacementExtrasMsg::ImportGraphicsPicked(path) => self.read_graphics(path),
            PlacementExtrasMsg::GraphicsRead {
                path,
                target,
                epoch,
                graphics,
            } => {
                self.graphics_read(&path, &target, epoch, graphics);
                Task::none()
            }
            // #1283: snippets and templates.
            PlacementExtrasMsg::SnippetRequested => self.snippet_dialog(),
            PlacementExtrasMsg::SnippetSaveRequested => self.save_selection_as_snippet(),
            PlacementExtrasMsg::SnippetWritten {
                saved,
                result,
                waiting,
            } => self.snippet_written(saved, result, waiting),
            PlacementExtrasMsg::ReuseListed {
                waiting,
                answer,
                listing,
            } => self.reuse_listed(waiting, answer, &listing),
        };
        Task::batch([task, self.finish_update()])
    }

    /// The sheet content from a file would land on: the active schematic,
    /// when it takes an edit. Says why not in the Messages panel.
    fn fragment_target(&mut self, what: &str) -> Option<PathBuf> {
        if !self.has_active_schematic() {
            crate::diagnostics::log_warning(format!("{what} works on a schematic sheet."));
            return None;
        }
        if let Some(reason) = self.refuse_if_active_read_only() {
            crate::diagnostics::log_warning(format!("{what}: {reason}"));
            return None;
        }
        self.document_state.active_path.clone()
    }

    /// Whether a read asked for at `epoch` for `target` still has a cursor
    /// to land on; said once in the Messages panel when it has not.
    fn fragment_read_is_current(&self, target: &Path, epoch: u64, what: &str) -> bool {
        let current = epoch == self.interaction_state.placement_epoch
            && self.document_state.active_path.as_deref() == Some(target);
        if !current {
            crate::diagnostics::log_info(format!(
                "{what} cancelled: the tool or the sheet changed before the file was read."
            ));
        }
        current
    }

    /// The grid a fragment's anchor snaps to.
    fn fragment_grid_mm(&self) -> f64 {
        crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm)
    }

    /// Put `copied` on the cursor of `target`: the next click places it as
    /// one undo step, numbered as a paste is.
    fn arm_fragment(&mut self, copied: CopiedSelection, target: &Path) {
        self.interaction_state.placement_epoch += 1;
        self.arm_placement_session(copied.payload, target, PlacementKind::Paste);
        self.refresh_paste_ghost();
    }

    /// Place `copied` with its anchor at `at` on the sheet the edit acts on,
    /// as one undo step: the operand form of a click on the cursor content.
    pub(crate) fn place_fragment_at(
        &mut self,
        copied: &CopiedSelection,
        at: Point,
    ) -> Result<(), String> {
        let target = self
            .document_state
            .edit_path()
            .map(Path::to_path_buf)
            .ok_or_else(|| "no schematic is open".to_string())?;
        let payload = self.paste_numbering(&target).apply(&copied.payload);
        self.place_part_at(&payload, at).map(|_| ())
    }
}
