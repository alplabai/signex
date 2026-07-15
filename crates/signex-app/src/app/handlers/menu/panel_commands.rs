use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_menu_panel_command(&mut self, msg: &MenuMessage) -> Option<Task<Message>> {
        match msg {
            MenuMessage::OpenProjectsPanel => {
                self.document_state.dock.add_panel(
                    crate::dock::PanelPosition::Left,
                    crate::panels::PanelKind::Projects,
                );
                Some(Task::none())
            }
            MenuMessage::OpenComponentsPanel => {
                self.document_state.dock.add_panel(
                    crate::dock::PanelPosition::Left,
                    crate::panels::PanelKind::Components,
                );
                Some(Task::none())
            }
            MenuMessage::OpenNavigatorPanel => {
                self.document_state.dock.add_panel(
                    crate::dock::PanelPosition::Right,
                    crate::panels::PanelKind::Navigator,
                );
                Some(Task::none())
            }
            MenuMessage::OpenPropertiesPanel => {
                self.document_state.dock.add_panel(
                    crate::dock::PanelPosition::Right,
                    crate::panels::PanelKind::Properties,
                );
                self.interaction_state.context_menu = None;
                Some(Task::none())
            }
            MenuMessage::OpenErcPanel => {
                self.document_state.dock.add_panel(
                    crate::dock::PanelPosition::Bottom,
                    crate::panels::PanelKind::Erc,
                );
                Some(Task::none())
            }
            MenuMessage::OpenMessagesPanel => {
                self.document_state.dock.add_panel(
                    crate::dock::PanelPosition::Bottom,
                    crate::panels::PanelKind::Messages,
                );
                Some(Task::none())
            }
            MenuMessage::OpenSignalPanel => {
                self.document_state.dock.add_panel(
                    crate::dock::PanelPosition::Bottom,
                    crate::panels::PanelKind::Signal,
                );
                Some(Task::none())
            }
            MenuMessage::OpenPreferences => {
                Some(self.update(Message::Preferences(PreferencesMsg::Open)))
            }
            MenuMessage::OpenTransmissionLineCalculator => {
                Some(self.update(Message::Window(WindowMsg::OpenTransmissionLineCalculator)))
            }
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
