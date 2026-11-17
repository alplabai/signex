//! The String tool (#2373): each click places a board string on the active
//! overlay, mechanical or documentation layer (`PcbLayer::takes_text`), as
//! one undo step, with the text, height and turn set in Properties (#2393),
//! else the catalog's new-string text at the default size.

use signex_i18n::{ids, t};
use signex_types::pcb::{BoardText, PcbBoard, Point};
use uuid::Uuid;

use crate::app::Signex;

impl Signex {
    /// The string a click at `at` places on the board in view, named `id`;
    /// `None` without a board in view or on a layer that takes no text.
    fn string_at(&self, id: Uuid, at: Point) -> Option<BoardText> {
        let path = self.active_tab_path()?;
        let layer = self.pcb_layer_view(&path).active();
        let fallback = t!(self.ui_state.i18n, ids::PCB_TOOL_STRING_TEXT).into_string();
        self.interaction_state
            .pcb_tool_defaults
            .text(id, at, layer, fallback)
    }

    /// A String click at `at`: place a string there, or say in the Messages
    /// panel which layers take one.
    pub(super) fn string_clicked(&mut self, at: Point) {
        let Some(text) = self.string_at(Uuid::now_v7(), at) else {
            let note = t!(self.ui_state.i18n, ids::PCB_TOOL_STRING_NEEDS_TEXT_LAYER);
            crate::diagnostics::log_warning(note.into_string());
            return;
        };
        let name = t!(self.ui_state.i18n, ids::PCB_TOOL_STRING).into_string();
        self.run_placement(&name, Ok(vec![signex_engine::pcb::draw::place_text(text)]));
    }

    /// The string the next click places, added to `preview`.
    pub(super) fn string_preview(&self, cursor: Point, preview: &mut PcbBoard) {
        preview.texts.extend(self.string_at(Uuid::nil(), cursor));
    }
}
