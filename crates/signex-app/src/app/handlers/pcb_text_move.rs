//! A footprint's designator or comment moved on its own (#2368): a press on
//! the text drags it, the release moves it as one board undo step. The text
//! follows the pointer exactly (no snap): it is placed by eye, clear of the
//! copper and other text. The command comes from
//! `signex_engine::pcb::footprint_text_edit`, which refuses a locked
//! footprint or one that locks its strings; a refusal changes nothing and
//! says why in Messages.

use std::collections::BTreeSet;
use std::path::Path;

use pcb_geom::BoardTextField;
use signex_engine::pcb::footprint_text_edit::{
    FootprintTextEdit, FootprintTextRefusal, footprint_text_command, text_movable,
};
use signex_types::pcb::Point;
use uuid::Uuid;

use crate::app::Signex;
use crate::pcb_canvas::{CarryKind, PcbCarry};

impl Signex {
    /// A press at `from` on a designator or comment starts a drag of that
    /// text alone. Whether it did. A locked footprint refuses at once, and
    /// the press then carries nothing.
    pub(crate) fn start_text_drag(&mut self, path: &Path, from: Point, reach: f64) -> bool {
        let Some(text) = self
            .with_pcb_selection(|state, board| state.grab_text_for_drag(board, from, reach))
            .flatten()
        else {
            return false;
        };
        let refused = self
            .active_pcb()
            .and_then(|board| text_movable(board, text.0).err());
        let window = self.interaction_state.acting_window;
        let mut carry = PcbCarry::new(CarryKind::Drag, path.to_path_buf(), window, BTreeSet::new());
        carry.anchor = Some(from);
        match refused {
            Some(refusal) => {
                crate::diagnostics::log_warning(format!("Move: {refusal}; nothing changed"));
            }
            None => carry.text = Some(text),
        }
        self.interaction_state.pcb_carry = Some(carry);
        self.refresh_pcb_highlight();
        true
    }

    /// Release a text drag: move the text by the carry's offset.
    pub(crate) fn drop_text_carry(&mut self, carry: &PcbCarry) {
        let Some((footprint, field)) = carry.text else {
            return;
        };
        let Some(path) = self.active_tab_path() else {
            return;
        };
        let edit = FootprintTextEdit::MoveBy(carry.delta);
        if let Err(refusal) = self.edit_footprint_text(&path, footprint, field, edit) {
            crate::diagnostics::log_warning(format!("Move: {refusal}; nothing changed"));
        }
    }

    /// Run `edit` on `field` of `footprint` on the board at `path` as one
    /// undo step. Whether the board changed.
    pub(crate) fn edit_footprint_text(
        &mut self,
        path: &Path,
        footprint: Uuid,
        field: BoardTextField,
        edit: FootprintTextEdit,
    ) -> Result<bool, String> {
        let engine = self
            .document_state
            .board_engine(path)
            .ok_or("the board is not open")?;
        let command = footprint_text_command(engine.board(), footprint, field, edit)
            .map_err(|refusal: FootprintTextRefusal| refusal.to_string())?;
        let Some(command) = command else {
            return Ok(false);
        };
        let patches = self
            .execute_board_commands(path, vec![command], None)
            .map_err(|error| error.to_string())?;
        self.refresh_pcb_highlight();
        Ok(!patches.is_empty())
    }
}
