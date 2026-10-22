use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_menu_panel_command(&mut self, msg: &MenuMessage) -> Option<Task<Message>> {
        match msg {
            // Each of these used to name its own dock region, and the
            // regions disagreed with both the boot layout and the
            // status-bar panel list — which is how one kind ended up
            // docked twice. `show_panel` owns the placement now (#641).
            MenuMessage::OpenProjectsPanel => {
                Some(self.show_panel(crate::panels::PanelKind::Projects))
            }
            MenuMessage::OpenComponentsPanel => {
                Some(self.show_panel(crate::panels::PanelKind::Components))
            }
            MenuMessage::OpenNavigatorPanel => {
                Some(self.show_panel(crate::panels::PanelKind::Navigator))
            }
            MenuMessage::OpenPropertiesPanel => {
                let task = self.show_panel(crate::panels::PanelKind::Properties);
                self.interaction_state.context_menu = None;
                Some(task)
            }
            MenuMessage::OpenErcPanel => Some(self.show_panel(crate::panels::PanelKind::Erc)),
            MenuMessage::OpenMessagesPanel => {
                Some(self.show_panel(crate::panels::PanelKind::Messages))
            }
            MenuMessage::OpenSignalPanel => Some(self.show_panel(crate::panels::PanelKind::Signal)),
            MenuMessage::OpenPreferences => {
                Some(self.update(Message::Preferences(PreferencesMsg::Open)))
            }
            MenuMessage::OpenPassiveCalculator => {
                Some(self.update(Message::Overlay(OverlayMsg::OpenPassiveCalculator)))
            }
            MenuMessage::OpenGerberViewer => Some(self.update(Message::OpenGerberViewer)),
            MenuMessage::OpenKeyboardShortcuts => {
                // Single-flag toggle — opening the modal is enough; the
                // close path goes through
                // `Message::Overlay(OverlayMsg::CloseKeyboardShortcuts)`.
                self.ui_state.keyboard_shortcuts_open = true;
                self.interaction_state.context_menu = None;
                self.ui_state.panel_list_open = false;
                Some(Task::none())
            }
            _ => None,
        }
    }
}
