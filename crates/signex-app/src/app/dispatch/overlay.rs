use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_overlay_message(&mut self, message: OverlayMsg) -> Task<Message> {
        match message {
            OverlayMsg::TogglePanelList => {
                self.ui_state.panel_list_open = !self.ui_state.panel_list_open;
                Task::none()
            }
            OverlayMsg::OpenPanel(kind) => {
                // The generic panel-list entry point. It used to send
                // *every* kind to the right column, so picking a panel
                // whose home is elsewhere — Projects, Signal, ERC —
                // grew a second copy on the right (#641).
                self.ui_state.panel_list_open = false;
                self.show_panel(kind)
            }
            OverlayMsg::OpenFind => self.handle_find_replace_open_requested(false),
            OverlayMsg::OpenReplace => self.handle_find_replace_open_requested(true),
            OverlayMsg::CloseKeyboardShortcuts => {
                self.ui_state.keyboard_shortcuts_open = false;
                Task::none()
            }
            OverlayMsg::CloseAbout => {
                self.ui_state.about_open = false;
                Task::none()
            }
            OverlayMsg::CopyAboutInfo => iced::clipboard::write(crate::about::version_info_text()),
            OverlayMsg::OpenLink(link) => crate::about::open_link(link, |detail| {
                Message::Overlay(OverlayMsg::LinkOpenFailed(detail))
            }),
            OverlayMsg::LinkOpenFailed(detail) => {
                self.document_state.error_notice =
                    Some(crate::app::state::ErrorNotice::link(detail));
                Task::none()
            }
            // Tools > Passive Network Calculator. Deliberately an in-app
            // modal and not an OS window: Signex runs borderless
            // (`bootstrap/new.rs` - `decorations: false`), so a default
            // `iced::window::open` would arrive with a native title bar
            // that appears nowhere else in the app. Setting the flag while
            // it is already open is a no-op, matching every other modal
            // here.
            OverlayMsg::OpenPassiveCalculator => {
                self.ui_state.passive_calculator_open = true;
                Task::none()
            }
            OverlayMsg::ClosePassiveCalculator => {
                self.ui_state.passive_calculator_open = false;
                Task::none()
            }
            OverlayMsg::DismissErrorNotice => {
                // Inlined rather than a handler in `menu/export`, which
                // is where it used to live: the card is not export-only
                // (#532), so neither is clearing it.
                self.document_state.error_notice = None;
                Task::none()
            }
            OverlayMsg::DismissFirstRunTour => {
                self.ui_state.first_run_tour_open = false;
                crate::fonts::write_first_run_tour_dismissed(true);
                Task::none()
            }
            OverlayMsg::ModalDragStart { modal, x, y } => self.handle_modal_drag_start(modal, x, y),
            OverlayMsg::ModalDragEnd => self.handle_modal_drag_end(),
            OverlayMsg::FocusAt {
                world_x,
                world_y,
                select,
            } => self.handle_focus_at(world_x, world_y, select),
            OverlayMsg::ToggleAutoFocus => self.handle_toggle_auto_focus(),
        }
    }

    /// Project lifecycle family handler (namespaced, ADR-0001 D3).
    /// Covers the project-close / app-quit confirm modals, the Project
    /// Options dismiss, the Add-Existing / Add-New-Schematic file-picker
    /// completions, and the async git-commit completion.
    pub(crate) fn dispatch_project_message(&mut self, msg: ProjectMsg) -> Task<Message> {
        match msg {
            ProjectMsg::CloseConfirm(choice) => self.handle_project_close_confirm(choice),
            ProjectMsg::CloseRefsResolved { project_id, result } => {
                self.handle_close_refs_resolved(project_id, result)
            }
            ProjectMsg::CaseProbed {
                project_id,
                outcome,
            } => self.handle_case_probed(project_id, outcome),
            ProjectMsg::FsFactsProbed(generation, facts) => {
                self.handle_project_fs_probed(generation, facts)
            }
            ProjectMsg::AppQuitConfirm(choice) => self.handle_app_quit_confirm(choice),
            ProjectMsg::CloseOptions => {
                self.ui_state.project_options = None;
                Task::none()
            }
            ProjectMsg::AddExistingFilePicked { project_id, paths } => {
                self.handle_add_existing_file_picked(project_id, paths)
            }
            ProjectMsg::AddExistingFilesCopied {
                project_id,
                results,
            } => {
                self.handle_add_existing_files_copied(project_id, results);
                Task::none()
            }
            ProjectMsg::AddNewSchematicWritten {
                project_id,
                path,
                result,
            } => {
                self.handle_add_new_schematic_written(project_id, path, result);
                Task::none()
            }
            ProjectMsg::AddNewSchematicPicked { project_id, path } => {
                self.handle_add_new_schematic_picked(project_id, path)
            }
            ProjectMsg::GitCommitDone {
                project_root,
                rel_path,
                result,
            } => self.handle_project_git_commit_done(project_root, rel_path, result),
        }
    }

    /// Move Selection dialog family handler (namespaced, ADR-0001 D3).
    /// Altium numeric ΔX / ΔY move on the current selection.
    pub(crate) fn dispatch_move_selection_message(
        &mut self,
        msg: MoveSelectionMsg,
    ) -> Task<Message> {
        match msg {
            MoveSelectionMsg::Open => self.handle_open_move_selection_dialog(),
            MoveSelectionMsg::Close => Task::batch([
                self.handle_close_move_selection_dialog(),
                self.close_detached_modal(super::state::ModalId::MoveSelection),
            ]),
            MoveSelectionMsg::DxChanged(s) => {
                self.ui_state.move_selection.dx = s;
                Task::none()
            }
            MoveSelectionMsg::DyChanged(s) => {
                self.ui_state.move_selection.dy = s;
                Task::none()
            }
            MoveSelectionMsg::Apply => self.handle_move_selection_apply(),
        }
    }

    /// Context-menu subsystem handler (canvas / project-tree / tab menus +
    /// submenu hover state machine), namespaced (ADR-0001 D3).
    pub(crate) fn dispatch_context_menu_message(&mut self, msg: ContextMenuMsg) -> Task<Message> {
        match msg {
            ContextMenuMsg::Show(x, y) => {
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
                    || self.ui_state.move_pick.is_some()
                    || self.ui_state.lasso_polygon.is_some();
                if placement_active {
                    self.clear_transient_schematic_tool_state();
                    self.interaction_state.current_tool = Tool::Select;
                    // Drop any app-level armed mode too.
                    self.ui_state.pending_net_color = None;
                    self.interaction_state.active_canvas_mut().pending_net_color = None;
                    self.ui_state.reorder_picker = None;
                    self.ui_state.move_pick = None;
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
            ContextMenuMsg::Close => {
                self.interaction_state.context_menu = None;
                self.interaction_state.context_submenu = None;
                self.interaction_state.pending_submenu = None;
                self.interaction_state.submenu_launcher_hovered = None;
                self.interaction_state.submenu_panel_hovered = false;
                self.interaction_state.submenu_unhovered_since = None;
                Task::none()
            }
            ContextMenuMsg::ShowProjectTree(key) => {
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
                    Some(crate::app::ProjectTreeContextMenuState { x, y, key });
                Task::none()
            }
            ContextMenuMsg::CloseProjectTree => {
                self.interaction_state.project_tree_context_menu = None;
                self.interaction_state.context_submenu = None;
                self.interaction_state.pending_submenu = None;
                self.interaction_state.submenu_launcher_hovered = None;
                self.interaction_state.submenu_panel_hovered = false;
                self.interaction_state.submenu_unhovered_since = None;
                Task::none()
            }
            ContextMenuMsg::ProjectTreeAction(action) => self.handle_project_tree_action(action),
            ContextMenuMsg::ShowTab(idx) => {
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
            ContextMenuMsg::CloseTab => {
                self.interaction_state.tab_context_menu = None;
                self.interaction_state.context_submenu = None;
                self.interaction_state.pending_submenu = None;
                self.interaction_state.submenu_launcher_hovered = None;
                self.interaction_state.submenu_panel_hovered = false;
                self.interaction_state.submenu_unhovered_since = None;
                Task::none()
            }
            ContextMenuMsg::TabAction(action) => self.handle_tab_context_action(action),
            ContextMenuMsg::SubmenuOpen(kind) => {
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
            ContextMenuMsg::SubmenuHover(kind) => {
                // Cursor entered a launcher row — arm the hover-open
                // timer and mark the launcher zone as hovered. The
                // close timer (if any) gets cancelled by the zone
                // refresh below.
                self.interaction_state.pending_submenu = Some((kind, std::time::Instant::now()));
                self.interaction_state.submenu_launcher_hovered = Some(kind);
                self.refresh_submenu_hover_state();
                Task::none()
            }
            ContextMenuMsg::SubmenuLeave => {
                // Cursor left a launcher row. Cancel the pending open
                // only if we're leaving the same launcher that armed
                // it (avoids a stale launcher cancelling a fresh open).
                self.interaction_state.submenu_launcher_hovered = None;
                self.interaction_state.pending_submenu = None;
                self.refresh_submenu_hover_state();
                Task::none()
            }
            ContextMenuMsg::SubmenuEnterPanel => {
                self.interaction_state.submenu_panel_hovered = true;
                self.refresh_submenu_hover_state();
                Task::none()
            }
            ContextMenuMsg::SubmenuLeavePanel => {
                self.interaction_state.submenu_panel_hovered = false;
                self.refresh_submenu_hover_state();
                Task::none()
            }
            ContextMenuMsg::SubmenuTickHover => {
                if let Some((kind, started)) = self.interaction_state.pending_submenu
                    && started.elapsed() >= std::time::Duration::from_millis(200)
                {
                    self.interaction_state.context_submenu = Some(kind);
                    self.interaction_state.pending_submenu = None;
                    self.interaction_state.submenu_unhovered_since = None;
                }
                if let Some(since) = self.interaction_state.submenu_unhovered_since
                    && since.elapsed() >= std::time::Duration::from_millis(150)
                {
                    self.interaction_state.context_submenu = None;
                    self.interaction_state.submenu_unhovered_since = None;
                }
                Task::none()
            }
            ContextMenuMsg::Action(action) => {
                self.interaction_state.context_menu = None;
                self.interaction_state.context_submenu = None;
                self.interaction_state.pending_submenu = None;
                self.interaction_state.submenu_launcher_hovered = None;
                self.interaction_state.submenu_panel_hovered = false;
                self.interaction_state.submenu_unhovered_since = None;
                match action {
                    ContextAction::Copy => self.dispatch_edit_message(EditMsg::Copy),
                    ContextAction::Cut => self.dispatch_edit_message(EditMsg::Cut),
                    ContextAction::Paste => self.dispatch_edit_message(EditMsg::Paste),
                    ContextAction::SmartPaste => self.dispatch_edit_message(EditMsg::SmartPaste),
                    ContextAction::OpenChildSheet => {
                        self.open_selected_child_sheet();
                        Task::none()
                    }
                    ContextAction::SyncSheetPins => {
                        self.sync_selected_sheet_pins();
                        // This dispatcher does not run `finish_update`; publish
                        // the outcome message to the Messages panel now.
                        self.sync_diagnostics_panel_ctx();
                        Task::none()
                    }
                    ContextAction::Delete => self.dispatch_edit_message(EditMsg::DeleteSelected),
                    ContextAction::SelectAll => self
                        .handle_selection_request(selection_request::SelectionRequest::SelectAll),
                    ContextAction::ZoomFit => {
                        self.handle_canvas_interaction_event(CanvasEvent::FitAll)
                    }
                    ContextAction::RotateSelected => {
                        self.dispatch_edit_message(EditMsg::RotateSelectedCounterclockwise)
                    }
                    ContextAction::MirrorX => self.dispatch_edit_message(EditMsg::MirrorSelectedY),
                    ContextAction::MirrorY => self.dispatch_edit_message(EditMsg::MirrorSelectedX),
                    ContextAction::ActiveBar(active_bar_action) => {
                        self.handle_active_bar_action(active_bar_action)
                    }
                }
            }
        }
    }

    /// Annotate dialog family handler (namespaced, ADR-0001 D3).
    pub(crate) fn dispatch_annotate_message(&mut self, msg: AnnotateMsg) -> Task<Message> {
        match msg {
            AnnotateMsg::Run(_) | AnnotateMsg::SetAllLocked(_)
                if self.annotate_preview_is_loading() =>
            {
                // The dialog's rows are being reloaded; acting now would use
                // a lock set built from a list that is out of date. The
                // buttons are disabled meanwhile.
                Task::none()
            }
            AnnotateMsg::Run(mode) => {
                // The action can rewrite unopened sheets on disk, which the
                // in-memory fingerprint cannot see: force a reload.
                let task = self.handle_annotate(mode);
                self.invalidate_annotate_preview();
                Task::batch([task, self.refresh_annotate_preview()])
            }
            AnnotateMsg::OpenDialog => {
                // A fresh session always re-reads the project from disk.
                self.invalidate_annotate_preview();
                let task = self.handle_open_annotate_dialog();
                Task::batch([task, self.refresh_annotate_preview()])
            }
            AnnotateMsg::PreviewLoaded { generation, result } => {
                self.handle_annotate_preview_loaded(generation, &result);
                Task::none()
            }
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
                self.recompute_annotate_entries();
                Task::none()
            }
            AnnotateMsg::SetAllLocked(lock) => {
                if lock {
                    let uuids = self.annotate_preview_uuids();
                    self.ui_state.annotate_locked.extend(uuids);
                } else {
                    self.ui_state.annotate_locked.clear();
                }
                self.recompute_annotate_entries();
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
            ErcMsg::RunFinished { generation, result } => {
                self.handle_erc_run_finished(generation, &result)
            }
            ErcMsg::PinMatrixCellCycled { row, col } => {
                self.handle_pin_matrix_cell_cycled(row, col)
            }
        }
    }

    /// Cycle one pin-matrix cell Error -> Warning -> Info -> Off -> Error and
    /// persist the result. A pair of pin types is unordered, so the mirrored
    /// cell is written with it: the dialog then never shows two different
    /// verdicts for the same pair. A value equal to the baseline drops the
    /// override instead of storing it.
    fn handle_pin_matrix_cell_cycled(&mut self, row: u8, col: u8) -> Task<Message> {
        use signex_erc::Severity;
        use signex_erc::pin_matrix::baseline_cell;
        let current = self
            .ui_state
            .pin_matrix_overrides
            .get(&(row, col))
            .copied()
            .unwrap_or_else(|| baseline_cell(row, col));
        let next = match current {
            Severity::Error => Severity::Warning,
            Severity::Warning => Severity::Info,
            Severity::Info => Severity::Off,
            Severity::Off => Severity::Error,
        };
        for key in [(row, col), (col, row)] {
            if next == baseline_cell(key.0, key.1) {
                self.ui_state.pin_matrix_overrides.remove(&key);
            } else {
                self.ui_state.pin_matrix_overrides.insert(key, next);
            }
        }
        crate::fonts::write_pin_matrix_overrides(&self.ui_state.pin_matrix_overrides);
        Task::none()
    }

    /// Preferences modal family handler (namespaced, ADR-0001 D3).
    pub(crate) fn dispatch_preferences_message(&mut self, msg: PreferencesMsg) -> Task<Message> {
        let task = match msg {
            PreferencesMsg::Open => self.handle_preferences_open_requested(),
            PreferencesMsg::Close => self.handle_preferences_close_requested(),
            PreferencesMsg::Nav(nav) => self.handle_preferences_navigation_requested(nav),
            PreferencesMsg::Inner(msg) => self.handle_preferences_message(msg),
        };
        // Preferences handlers report real failures — a refused write, a
        // recovery action that could not move the broken file aside — and
        // this dispatcher does not run `finish_update`, so without this
        // those records sit in the ring buffer until some unrelated later
        // message republishes them. A log window that shows a failure
        // several actions after it happened is worse than useless: it
        // attaches the message to the wrong action.
        self.sync_diagnostics_panel_ctx();
        // Same reason: the draft, discard and Save paths change the zoom
        // speed the PCB / symbol / footprint canvases hold a copy of.
        self.sync_zoom_step();
        task
    }

    /// Enable Version Control modal family handler (namespaced, ADR-0001 D3).
    pub(crate) fn dispatch_enable_version_control_message(
        &mut self,
        msg: EnableVersionControlMsg,
    ) -> Task<Message> {
        match msg {
            // A run in flight has already read the choices; edits are ignored.
            EnableVersionControlMsg::ToggleLfs => {
                if let Some(s) = self.ui_state.enable_version_control.as_mut()
                    && !s.working
                {
                    s.use_lfs = !s.use_lfs;
                }
                Task::none()
            }
            EnableVersionControlMsg::ToggleItem(idx) => {
                if let Some(s) = self.ui_state.enable_version_control.as_mut()
                    && !s.working
                    && let Some(item) = s.items.get_mut(idx)
                {
                    item.tracked = !item.tracked;
                }
                Task::none()
            }
            EnableVersionControlMsg::Confirm => self.handle_enable_version_control_confirm(),
            EnableVersionControlMsg::Finished {
                scope,
                project_dir,
                outcome,
            } => {
                self.handle_enable_version_control_finished(scope, project_dir, outcome);
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
            RemoveMsg::ExcludeDirty(choice) => self.handle_exclude_dirty_choice(choice),
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
