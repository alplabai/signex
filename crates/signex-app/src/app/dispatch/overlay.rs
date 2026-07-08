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
            Message::CloseKeyboardShortcuts => {
                self.ui_state.keyboard_shortcuts_open = false;
                Task::none()
            }
            Message::DismissFirstRunTour => {
                self.ui_state.first_run_tour_open = false;
                crate::fonts::write_first_run_tour_dismissed(true);
                Task::none()
            }
            Message::FindReplaceMsg(msg) => self.handle_find_replace_message(msg),
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
            Message::AppQuitConfirm(choice) => self.handle_app_quit_confirm(choice),
            Message::AddExistingFilePicked { project_idx, paths } => {
                self.handle_add_existing_file_picked(project_idx, paths);
                Task::none()
            }
            Message::AddNewSchematicPicked { project_idx, path } => {
                self.handle_add_new_schematic_picked(project_idx, path);
                Task::none()
            }
            Message::CloseProjectOptions => {
                self.ui_state.project_options = None;
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

    /// Annotate dialog family handler (namespaced, ADR-0001 D3).
    pub(crate) fn dispatch_annotate_message(&mut self, msg: AnnotateMsg) -> Task<Message> {
        match msg {
            AnnotateMsg::Run(mode) => self.handle_annotate(mode),
            AnnotateMsg::OpenDialog => self.handle_open_annotate_dialog(),
            AnnotateMsg::CloseDialog => self.handle_close_annotate_dialog(),
            AnnotateMsg::OrderChanged(order) => self.handle_annotate_order_changed(order),
            AnnotateMsg::OpenResetConfirm => self.handle_open_annotate_reset_confirm(),
            AnnotateMsg::CloseResetConfirm => self.handle_close_annotate_reset_confirm(),
            AnnotateMsg::ToggleLock(uuid) => {
                if self.ui_state.annotate_locked.contains(&uuid) {
                    self.ui_state.annotate_locked.remove(&uuid);
                } else {
                    self.ui_state.annotate_locked.insert(uuid);
                }
                Task::none()
            }
        }
    }

    /// ERC dialog family handler (namespaced, ADR-0001 D3).
    pub(crate) fn dispatch_erc_message(&mut self, msg: ErcMsg) -> Task<Message> {
        match msg {
            ErcMsg::Run => {
                let close_task = if self.ui_state.erc_dialog_open {
                    self.handle_close_erc_dialog()
                } else {
                    Task::none()
                };
                let task = self.handle_run_erc();
                let finish = self.finish_update();
                Task::batch([close_task, finish, task])
            }
            ErcMsg::OpenDialog => self.handle_open_erc_dialog(),
            ErcMsg::CloseDialog => self.handle_close_erc_dialog(),
            ErcMsg::SeverityChanged(rule, sev) => self.handle_erc_severity_changed(rule, sev),
        }
    }

    /// Preferences modal family handler (namespaced, ADR-0001 D3).
    pub(crate) fn dispatch_preferences_message(&mut self, msg: PreferencesMsg) -> Task<Message> {
        match msg {
            PreferencesMsg::Open => self.handle_preferences_open_requested(),
            PreferencesMsg::Close => self.handle_preferences_close_requested(),
            PreferencesMsg::Nav(nav) => self.handle_preferences_navigation_requested(nav),
            PreferencesMsg::Inner(msg) => self.handle_preferences_message(msg),
        }
    }

    /// Enable Version Control modal family handler (namespaced, ADR-0001 D3).
    pub(crate) fn dispatch_enable_version_control_message(
        &mut self,
        msg: EnableVersionControlMsg,
    ) -> Task<Message> {
        match msg {
            EnableVersionControlMsg::ToggleLfs => {
                if let Some(s) = self.ui_state.enable_version_control.as_mut() {
                    s.use_lfs = !s.use_lfs;
                }
                Task::none()
            }
            EnableVersionControlMsg::ToggleItem(idx) => {
                if let Some(s) = self.ui_state.enable_version_control.as_mut() {
                    if let Some(item) = s.items.get_mut(idx) {
                        item.tracked = !item.tracked;
                    }
                }
                Task::none()
            }
            EnableVersionControlMsg::Confirm => {
                self.handle_enable_version_control_confirm();
                Task::none()
            }
            EnableVersionControlMsg::Close => {
                self.ui_state.enable_version_control = None;
                Task::none()
            }
        }
    }

    /// Rename modal family handler (namespaced, ADR-0001 D3).
    pub(crate) fn dispatch_rename_message(&mut self, msg: RenameMsg) -> Task<Message> {
        match msg {
            RenameMsg::BufferChanged(s) => {
                if let Some(d) = self.ui_state.rename_dialog.as_mut() {
                    d.buffer = s;
                    d.error = None;
                }
                Task::none()
            }
            RenameMsg::Submit => self.handle_rename_submit(),
            RenameMsg::Close => {
                self.ui_state.rename_dialog = None;
                Task::none()
            }
        }
    }

    /// Remove-from-project modal family handler (namespaced, ADR-0001 D3).
    pub(crate) fn dispatch_remove_message(&mut self, msg: RemoveMsg) -> Task<Message> {
        match msg {
            RemoveMsg::Confirm(choice) => self.handle_remove_confirm(choice),
            RemoveMsg::Close => {
                self.ui_state.remove_dialog = None;
                Task::none()
            }
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
