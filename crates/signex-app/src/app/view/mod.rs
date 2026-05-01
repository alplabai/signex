use iced::Element;

mod dialogs;
mod detached;
mod layout;
mod menus;
mod overlays;
mod print_preview;
mod translate;

use super::*;

impl Signex {
    pub fn view(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        // Secondary windows (detached modals, future undocked tabs) render
        // just their own content — no menu / dock / canvas. The main
        // window's view_main drops any overlay whose modal is currently
        // detached so we don't double-render.
        if let Some(kind) = self.ui_state.windows.get(&window_id) {
            return match kind {
                super::state::WindowKind::DetachedModal(modal) => self.view_detached_modal(*modal),
                // Undocked tab = full duplicate of the main app view.
                // Shared Signex state means edits sync automatically; the
                // only difference between main and undocked is the OS
                // window id they render into.
                super::state::WindowKind::UndockedTab { .. } => self.view_main_for(window_id),
                super::state::WindowKind::DetachedPanel(kind) => {
                    let panel = crate::panels::view_panel(*kind, &self.document_state.panel_ctx)
                        .map(crate::dock::DockMessage::Panel)
                        .map(Message::Dock);
                    iced::widget::container(iced::widget::scrollable(panel))
                        .padding(8)
                        .into()
                }
            };
        }
        self.view_main_for(window_id)
    }

}
