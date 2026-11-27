//! A board's Length Matching as a page tab (#2797, mockup row 107): one per
//! board, after its PCB tab, kept by the board page rules of
//! `app/layer_stack_tab`. Length Matching... and the dock panel's Open as
//! tab open it (`LengthMatchingMsg::TabOpened`); a second open focuses it.
//! The tab holds no document: it shows the board's groups and saves with
//! the board.

use iced::{Element, Widget as _};

use crate::app::{Message, Signex};
use crate::dock::DockMessage;

impl Signex {
    /// Whether the main window shows a Length Matching tab.
    pub(crate) fn length_matching_tab_in_window(&self, is_main: bool) -> bool {
        let docs = &self.document_state;
        is_main
            && docs
                .tabs
                .get(docs.active_tab)
                .is_some_and(|tab| tab.kind.is_length_matching())
    }

    pub(crate) fn view_length_matching_tab(&self) -> Element<'_, Message> {
        crate::panels::length_matching::tab_view(&self.document_view.panel_ctx)
            .map(|msg| Message::Dock(DockMessage::Panel(msg)))
            .boxed()
    }
}
