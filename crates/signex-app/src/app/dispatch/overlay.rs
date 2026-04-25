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
                crate::fonts::write_dock_layout(&self.document_state.dock);
                Task::none()
            }
            Message::OpenFind => self.handle_find_replace_open_requested(false),
            Message::OpenReplace => self.handle_find_replace_open_requested(true),
            Message::OpenPreferences => self.handle_preferences_open_requested(),
            Message::ClosePreferences => self.handle_preferences_close_requested(),
            Message::PreferencesNav(nav) => self.handle_preferences_navigation_requested(nav),
            Message::PreferencesMsg(msg) => self.handle_preferences_message(msg),
            Message::FindReplaceMsg(msg) => self.handle_find_replace_message(msg),
            Message::RunErc => {
                let close_task = if self.ui_state.erc_dialog_open {
                    self.handle_close_erc_dialog()
                } else {
                    Task::none()
                };
                let task = self.handle_run_erc();
                let finish = self.finish_update();
                Task::batch([close_task, finish, task])
            }
            Message::Annotate(mode) => self.handle_annotate(mode),
            Message::OpenAnnotateDialog => self.handle_open_annotate_dialog(),
            Message::CloseAnnotateDialog => self.handle_close_annotate_dialog(),
            Message::AnnotateOrderChanged(order) => self.handle_annotate_order_changed(order),
            Message::OpenErcDialog => self.handle_open_erc_dialog(),
            Message::CloseErcDialog => self.handle_close_erc_dialog(),
            Message::ErcSeverityChanged(rule, sev) => self.handle_erc_severity_changed(rule, sev),
            Message::OpenAnnotateResetConfirm => self.handle_open_annotate_reset_confirm(),
            Message::CloseAnnotateResetConfirm => self.handle_close_annotate_reset_confirm(),
            Message::ModalDragStart { modal, x, y } => self.handle_modal_drag_start(modal, x, y),
            Message::ModalDragEnd => self.handle_modal_drag_end(),
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
                // power/port OR paused preview OR net-colour pen armed)
                // instead of opening the context menu.
                let canvas = self.interaction_state.active_canvas();
                let placement_active = self.interaction_state.current_tool != Tool::Select
                    || canvas.ghost_label.is_some()
                    || canvas.ghost_symbol.is_some()
                    || canvas.ghost_text.is_some()
                    || canvas.placement_paused
                    || self.interaction_state.pending_power.is_some()
                    || self.interaction_state.pending_port.is_some()
                    || self.ui_state.pending_net_color.is_some()
                    || self.ui_state.reorder_picker.is_some()
                    || self.ui_state.lasso_polygon.is_some();
                if placement_active {
                    self.clear_transient_schematic_tool_state();
                    self.interaction_state.current_tool = Tool::Select;
                    // Drop any app-level armed mode too.
                    self.ui_state.pending_net_color = None;
                    self.interaction_state.active_canvas_mut().pending_net_color = None;
                    self.ui_state.reorder_picker = None;
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                    return Task::none();
                }
                if self.interaction_state.active_bar_menu.is_none() {
                    self.interaction_state.context_menu = Some(ContextMenuState { x, y });
                }
                Task::none()
            }
            Message::CloseContextMenu => {
                self.interaction_state.context_menu = None;
                self.interaction_state.context_submenu = None;
                self.interaction_state.pending_submenu = None;
                self.interaction_state.submenu_launcher_hovered = None;
                self.interaction_state.submenu_panel_hovered = false;
                self.interaction_state.submenu_unhovered_since = None;
                Task::none()
            }
            Message::ShowProjectTreeContextMenu(path) => {
                // Close any canvas context menu so the two menus never
                // overlap, then anchor the new menu to `last_mouse_pos`
                // (iced 0.14 mouse_area does not forward cursor coords
                // with on_right_press, so we use the last tracked pos
                // from the global mouse-move subscription). Also clear
                // any submenu state from a *previous* right-click —
                // otherwise opening the project root, hovering "Add
                // New to Project ›", dismissing the menu, then right-
                // clicking a leaf row would still render the stale
                // submenu next to the new (leaf) menu.
                self.interaction_state.context_menu = None;
                self.interaction_state.context_submenu = None;
                self.interaction_state.pending_submenu = None;
                self.interaction_state.submenu_launcher_hovered = None;
                self.interaction_state.submenu_panel_hovered = false;
                self.interaction_state.submenu_unhovered_since = None;
                let (x, y) = self.interaction_state.last_mouse_pos;
                self.interaction_state.project_tree_context_menu =
                    Some(crate::app::ProjectTreeContextMenuState { x, y, path });
                Task::none()
            }
            Message::CloseProjectTreeContextMenu => {
                self.interaction_state.project_tree_context_menu = None;
                self.interaction_state.context_submenu = None;
                self.interaction_state.pending_submenu = None;
                self.interaction_state.submenu_launcher_hovered = None;
                self.interaction_state.submenu_panel_hovered = false;
                self.interaction_state.submenu_unhovered_since = None;
                Task::none()
            }
            Message::ProjectTreeAction(action) => self.handle_project_tree_action(action),
            Message::ShowTabContextMenu(idx) => {
                // Mutually exclusive with the canvas + project-tree
                // menus — close them and any submenu state from a
                // previous right-click before anchoring the tab menu
                // at `last_mouse_pos`.
                self.interaction_state.context_menu = None;
                self.interaction_state.project_tree_context_menu = None;
                self.interaction_state.context_submenu = None;
                self.interaction_state.pending_submenu = None;
                self.interaction_state.submenu_launcher_hovered = None;
                self.interaction_state.submenu_panel_hovered = false;
                self.interaction_state.submenu_unhovered_since = None;
                let (x, y) = self.interaction_state.last_mouse_pos;
                self.interaction_state.tab_context_menu =
                    Some(crate::app::TabContextMenuState { x, y, tab_idx: idx });
                Task::none()
            }
            Message::CloseTabContextMenu => {
                self.interaction_state.tab_context_menu = None;
                self.interaction_state.context_submenu = None;
                self.interaction_state.pending_submenu = None;
                self.interaction_state.submenu_launcher_hovered = None;
                self.interaction_state.submenu_panel_hovered = false;
                self.interaction_state.submenu_unhovered_since = None;
                Task::none()
            }
            Message::TabContextAction(action) => self.handle_tab_context_action(action),
            Message::ProjectCloseConfirm(choice) => self.handle_project_close_confirm(choice),
            Message::RenameBufferChanged(s) => {
                if let Some(d) = self.ui_state.rename_dialog.as_mut() {
                    d.buffer = s;
                    d.error = None;
                }
                Task::none()
            }
            Message::RenameSubmit => self.handle_rename_submit(),
            Message::CloseRenameDialog => {
                self.ui_state.rename_dialog = None;
                Task::none()
            }
            Message::RemoveConfirm(choice) => self.handle_remove_confirm(choice),
            Message::CloseRemoveDialog => {
                self.ui_state.remove_dialog = None;
                Task::none()
            }
            Message::OpenContextSubmenu(kind) => {
                // Click-to-open. Toggles off if the same kind is fired
                // again so the header row works as a collapse handle.
                if self.interaction_state.context_submenu == Some(kind) {
                    self.interaction_state.context_submenu = None;
                } else {
                    self.interaction_state.context_submenu = Some(kind);
                }
                self.interaction_state.pending_submenu = None;
                self.interaction_state.submenu_unhovered_since = None;
                Task::none()
            }
            Message::HoverContextSubmenu(kind) => {
                // Cursor entered a launcher row — arm the hover-open
                // timer and mark the launcher zone as hovered. The
                // close timer (if any) gets cancelled by the zone
                // refresh below.
                self.interaction_state.pending_submenu = Some((kind, std::time::Instant::now()));
                self.interaction_state.submenu_launcher_hovered = Some(kind);
                self.refresh_submenu_hover_state();
                Task::none()
            }
            Message::LeaveContextSubmenu => {
                // Cursor left a launcher row. Cancel the pending open
                // only if we're leaving the same launcher that armed
                // it (avoids a stale launcher cancelling a fresh open).
                self.interaction_state.submenu_launcher_hovered = None;
                self.interaction_state.pending_submenu = None;
                self.refresh_submenu_hover_state();
                Task::none()
            }
            Message::EnterContextSubmenuPanel => {
                self.interaction_state.submenu_panel_hovered = true;
                self.refresh_submenu_hover_state();
                Task::none()
            }
            Message::LeaveContextSubmenuPanel => {
                self.interaction_state.submenu_panel_hovered = false;
                self.refresh_submenu_hover_state();
                Task::none()
            }
            Message::TickContextSubmenuHover => {
                if let Some((kind, started)) = self.interaction_state.pending_submenu {
                    if started.elapsed() >= std::time::Duration::from_millis(200) {
                        self.interaction_state.context_submenu = Some(kind);
                        self.interaction_state.pending_submenu = None;
                        self.interaction_state.submenu_unhovered_since = None;
                    }
                }
                if let Some(since) = self.interaction_state.submenu_unhovered_since {
                    if since.elapsed() >= std::time::Duration::from_millis(150) {
                        self.interaction_state.context_submenu = None;
                        self.interaction_state.submenu_unhovered_since = None;
                    }
                }
                Task::none()
            }
            Message::ContextAction(action) => {
                self.interaction_state.context_menu = None;
                self.interaction_state.context_submenu = None;
                self.interaction_state.pending_submenu = None;
                self.interaction_state.submenu_launcher_hovered = None;
                self.interaction_state.submenu_panel_hovered = false;
                self.interaction_state.submenu_unhovered_since = None;
                match action {
                    ContextAction::Copy => self.dispatch_document_message(Message::Copy),
                    ContextAction::Cut => self.dispatch_document_message(Message::Cut),
                    ContextAction::Paste => self.dispatch_document_message(Message::Paste),
                    ContextAction::SmartPaste => {
                        self.dispatch_document_message(Message::SmartPaste)
                    }
                    ContextAction::OpenChildSheet => {
                        self.open_selected_child_sheet();
                        Task::none()
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
                    ContextAction::ActiveBar(active_bar_action) => {
                        self.handle_active_bar_action(active_bar_action)
                    }
                }
            }
            _ => unreachable!("dispatch_overlay_message received non-overlay message"),
        }
    }

    /// Recompute the submenu close timer from the current hover zone
    /// booleans. If either the launcher row or the submenu panel is
    /// hovered the close timer is cancelled; once *both* are clear and
    /// a submenu is actually open, we arm the 150 ms delay. Called
    /// after every hover-zone change so the close timer state never
    /// contradicts the live hover flags.
    fn refresh_submenu_hover_state(&mut self) {
        let any_hovered = self.interaction_state.submenu_launcher_hovered.is_some()
            || self.interaction_state.submenu_panel_hovered;
        if any_hovered {
            self.interaction_state.submenu_unhovered_since = None;
        } else if self.interaction_state.context_submenu.is_some()
            && self.interaction_state.submenu_unhovered_since.is_none()
        {
            self.interaction_state.submenu_unhovered_since = Some(std::time::Instant::now());
        }
    }
}
