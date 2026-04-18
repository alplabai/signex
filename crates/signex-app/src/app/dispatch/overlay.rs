use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_overlay_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TogglePanelList => {
                self.ui_state.panel_list_open = !self.ui_state.panel_list_open;
                Task::none()
            }
            Message::OpenPanel(kind) => {
                self.ui_state.panel_list_open = false;
                self.document_state
                    .dock
                    .add_panel(crate::dock::PanelPosition::Right, kind);
                Task::none()
            }
            Message::OpenFind => self.handle_find_replace_open_requested(false),
            Message::OpenReplace => self.handle_find_replace_open_requested(true),
            Message::OpenPreferences => self.handle_preferences_open_requested(),
            Message::ClosePreferences => self.handle_preferences_close_requested(),
            Message::PreferencesNav(nav) => self.handle_preferences_navigation_requested(nav),
            Message::PreferencesMsg(msg) => self.handle_preferences_message(msg),
            Message::FindReplaceMsg(msg) => self.handle_find_replace_message(msg),
            Message::CloseTabConfirm(choice) => self.handle_close_tab_confirm(choice),
            Message::RunErc => self.handle_run_erc(),
            Message::FocusAt {
                world_x,
                world_y,
                select,
            } => self.handle_focus_at(world_x, world_y, select),
            Message::ToggleAutoFocus => self.handle_toggle_auto_focus(),
            Message::ActiveBar(msg) => self.handle_active_bar_message(msg),
            Message::ShowContextMenu(x, y) => {
                // Altium convention: right-click during placement terminates
                // the placement flow (tool-stuck OR ghost-armed OR pending
                // power/port OR paused preview) instead of opening the
                // context menu. Otherwise, open the menu.
                let canvas = &self.interaction_state.canvas;
                let placement_active = self.interaction_state.current_tool != Tool::Select
                    || canvas.ghost_label.is_some()
                    || canvas.ghost_symbol.is_some()
                    || canvas.ghost_text.is_some()
                    || canvas.placement_paused
                    || self.interaction_state.pending_power.is_some()
                    || self.interaction_state.pending_port.is_some();
                if placement_active {
                    self.clear_transient_schematic_tool_state();
                    self.interaction_state.current_tool = Tool::Select;
                    return Task::none();
                }
                if self.interaction_state.active_bar_menu.is_none() {
                    self.interaction_state.context_menu = Some(ContextMenuState { x, y });
                }
                Task::none()
            }
            Message::CloseContextMenu => {
                self.interaction_state.context_menu = None;
                Task::none()
            }
            Message::ContextAction(action) => {
                self.interaction_state.context_menu = None;
                match action {
                    ContextAction::Copy => self.dispatch_document_message(Message::Copy),
                    ContextAction::Cut => self.dispatch_document_message(Message::Cut),
                    ContextAction::Paste => self.dispatch_document_message(Message::Paste),
                    ContextAction::SmartPaste => {
                        self.dispatch_document_message(Message::SmartPaste)
                    }
                    ContextAction::Delete => {
                        self.dispatch_document_message(Message::DeleteSelected)
                    }
                    ContextAction::SelectAll => self.dispatch_routed_message(Message::Selection(
                        selection_request::SelectionRequest::SelectAll,
                    )),
                    ContextAction::ZoomFit => {
                        self.dispatch_ui_message(Message::CanvasEvent(CanvasEvent::FitAll))
                    }
                    ContextAction::RotateSelected => {
                        self.dispatch_document_message(Message::RotateSelected)
                    }
                    ContextAction::MirrorX => {
                        self.dispatch_document_message(Message::MirrorSelectedY)
                    }
                    ContextAction::MirrorY => {
                        self.dispatch_document_message(Message::MirrorSelectedX)
                    }
                }
            }
            _ => unreachable!("dispatch_overlay_message received non-overlay message"),
        }
    }
}
