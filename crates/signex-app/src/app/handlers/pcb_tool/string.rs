//! The String tool (#2373): each click places a board string on the active
//! layer, as one undo step. It reads the catalog's new-string text until the
//! Properties panel edits it; the board renderer draws it (#2387).

use signex_engine::pcb::draw;
use signex_i18n::{ids, t};
use signex_types::pcb::{BoardText, Point};
use uuid::Uuid;

use crate::app::Signex;

impl Signex {
    /// The string a click at `at` places on the board in view, named `id`;
    /// `None` without a board in view.
    fn string_at(&self, id: Uuid, at: Point) -> Option<BoardText> {
        let path = self.active_tab_path()?;
        let layer = self.pcb_layer_view(&path).active();
        let text = t!(self.ui_state.i18n, ids::PCB_TOOL_STRING_TEXT).into_string();
        Some(draw::new_text(id, at, layer, text))
    }

    /// A String click at `at`: place a string there.
    pub(super) fn string_clicked(&mut self, at: Point) {
        let Some(text) = self.string_at(Uuid::now_v7(), at) else {
            return;
        };
        let name = t!(self.ui_state.i18n, ids::PCB_TOOL_STRING).into_string();
        self.run_placement(&name, Ok(vec![draw::place_text(text)]));
    }

    /// The string the next click places, drawn into `preview`.
    pub(super) fn string_preview(&self, cursor: Point, preview: &mut signex_types::pcb::PcbBoard) {
        preview.texts.extend(self.string_at(Uuid::nil(), cursor));
    }
}
