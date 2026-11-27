//! A board's Polygon Manager as a page tab (mockup row 178, DECISIONS
//! 2026-11-18): one per board, after its PCB tab, kept by the board page
//! rules of `app/layer_stack_tab`. Tools ▸ Polygon Manager..., a pour's
//! right-click and the PCB panel's Polygons mode open it
//! (`PolygonMsg::ManagerOpened`); a second open focuses it. The tab holds no
//! document: it shows the board's pours and saves with the board.

use iced::{Element, Widget as _};
use pcb_geom::ItemRef;

use crate::app::{Message, Signex};
use crate::dock::DockMessage;

impl Signex {
    /// Whether the main window shows a Polygon Manager tab.
    pub(crate) fn polygon_manager_tab_in_window(&self, is_main: bool) -> bool {
        let docs = &self.document_state;
        is_main
            && docs
                .tabs
                .get(docs.active_tab)
                .is_some_and(|tab| tab.kind.is_polygon_manager())
    }

    pub(crate) fn view_polygon_manager_tab(&self) -> Element<'_, Message> {
        crate::panels::polygons::view(&self.document_view.panel_ctx)
            .map(|msg| Message::Dock(DockMessage::Panel(msg)))
            .boxed()
    }

    /// Whether the board right-click in `window` is on a polygon pour: the
    /// board selection there, else what is under the pointer, holds one.
    /// The board menu then offers Polygon Manager.
    pub(crate) fn board_menu_on_pour(&self, window: Option<iced::window::Id>) -> bool {
        let undocked = window.filter(|id| self.ui_state.main_window_id != Some(*id));
        let (board, state) = match undocked {
            Some(id) => (
                self.pcb_for_window(id),
                self.interaction_state.pcb_selections.get(&id),
            ),
            None => (
                self.active_pcb(),
                Some(&self.interaction_state.pcb_selection),
            ),
        };
        let (Some(board), Some(state)) = (board, state) else {
            return false;
        };
        let is_pour = |item: ItemRef| match item {
            ItemRef::Zone { zone, .. } => board.zones.iter().any(|z| z.uuid == zone && z.is_pour()),
            _ => false,
        };
        state.selection().items().chain(state.hover()).any(is_pour)
    }
}
