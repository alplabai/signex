use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_overlay_message(&mut self, message: OverlayMsg) -> Task<Message> {
        match message {
            OverlayMsg::TogglePanelList => {
                self.ui_state.panel_list_open = !self.ui_state.panel_list_open;
                // The two status-bar popovers never stand together.
                self.ui_state.grid_snap_popover = None;
                Task::none()
            }
            OverlayMsg::OpenPanel(kind) => {
                // The generic panel-list entry point. It used to send
                // *every* kind to the right column, so picking a panel
                // whose home is elsewhere — Projects, Components, ERC —
                // grew a second copy on the right (#641).
                self.ui_state.panel_list_open = false;
                let shown = self.show_panel(kind);
                // A panel opened into an empty dock gives the undocked
                // windows their panel contexts and History loads.
                Task::batch([shown, self.finish_update()])
            }
            OverlayMsg::PanelToggled(kind) => {
                self.ui_state.panel_list_open = false;
                let toggled = self.toggle_panel(kind);
                Task::batch([toggled, self.finish_update()])
            }
            OverlayMsg::OpenFind => self.handle_find_replace_open_requested(false),
            OverlayMsg::OpenReplace => self.handle_find_replace_open_requested(true),
            // Find shows in its own OS window whichever sheet it searches,
            // so a press in any window closes it (docs/DETACHED_MODALS.md).
            OverlayMsg::FindToggled if self.ui_state.find_replace.open => {
                self.handle_find_replace_message(crate::find_replace::FindReplaceMsg::Close)
            }
            OverlayMsg::FindToggled => self.handle_find_replace_open_requested(false),
            OverlayMsg::CloseKeyboardShortcuts => {
                self.ui_state.keyboard_shortcuts_open = false;
                Task::none()
            }
            OverlayMsg::CloseAbout => {
                self.ui_state.about_open = false;
                Task::none()
            }
            OverlayMsg::CopyAboutInfo => {
                iced::clipboard::write(crate::about::version_info_text()).discard()
            }
            OverlayMsg::OpenLink(link) => {
                crate::about::open_link(self.ui_state.io.opener, link, |detail| {
                    Message::Overlay(OverlayMsg::LinkOpenFailed(detail))
                })
            }
            OverlayMsg::LinkOpenFailed(detail) => {
                self.document_state.error_notice =
                    Some(crate::app::state::ErrorNotice::link(detail));
                Task::none()
            }
            // Tools > Passive Network Calculator. Deliberately an in-app
            // modal and not an OS window: Signex runs borderless
            // (`bootstrap/new.rs`; only the macOS native title bar
            // differs), so a default
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
            OverlayMsg::DiskConflictOverwriteChosen => self.handle_disk_conflict_overwrite(),
            OverlayMsg::DiskConflictReloadChosen => self.handle_disk_conflict_reload(),
            OverlayMsg::DiskConflictSaveAsChosen => self.handle_disk_conflict_save_as(),
            OverlayMsg::ErrorNoticeCopyPressed => match &self.document_state.error_notice {
                Some(notice) => {
                    iced::clipboard::write(notice.copy_text(&self.ui_state.i18n)).discard()
                }
                None => Task::none(),
            },
            OverlayMsg::DismissFirstRunTour => self.dismiss_first_run_tour(),
            OverlayMsg::ModalDragStart { modal, x, y } => self.handle_modal_drag_start(modal, x, y),
            OverlayMsg::ModalDragEnd => self.handle_modal_drag_end(),
            OverlayMsg::FocusAt {
                world_x,
                world_y,
                select,
            } => self.handle_focus_at(world_x, world_y, select),
            OverlayMsg::ToggleAutoFocus => self.handle_toggle_auto_focus(),
            OverlayMsg::GridSnapPopoverToggled(window) => {
                let open_here = self
                    .ui_state
                    .grid_snap_popover
                    .as_ref()
                    .is_some_and(|popover| popover.window == window);
                self.ui_state.grid_snap_popover =
                    (!open_here).then(|| Box::new(crate::status_bar::GridSnapPopover::new(window)));
                self.ui_state.panel_list_open = false;
                Task::none()
            }
            OverlayMsg::GridSnapPopoverDismissed => {
                self.ui_state.grid_snap_popover = None;
                Task::none()
            }
            OverlayMsg::GridSnapPreviewHovered(msg) => {
                if let Some(popover) = self.ui_state.grid_snap_popover.as_mut() {
                    popover.hover.update(msg);
                }
                Task::none()
            }
            // Help cards (#2262).
            OverlayMsg::ShortcutsFilterChanged(filter) => {
                self.ui_state.shortcut_sheet.filter = filter;
                Task::none()
            }
            OverlayMsg::ShortcutsFilterCleared => {
                self.ui_state.shortcut_sheet.filter.clear();
                Task::none()
            }
            OverlayMsg::ShortcutsScopePicked(scope) => {
                self.ui_state.shortcut_sheet.scope = scope;
                Task::none()
            }
            OverlayMsg::ShortcutsPreferencesPressed => {
                self.ui_state.keyboard_shortcuts_open = false;
                self.open_preferences_on(crate::preferences::PrefNav::Keyboard)
            }
            OverlayMsg::FirstRunTourWheelChangePressed => {
                self.open_preferences_on(crate::preferences::PrefNav::Appearance)
            }
        }
    }

    /// Project lifecycle family handler (namespaced, ADR-0001 D3).
    /// Covers the project-close / app-quit confirm modals, the Project
    /// tab and its `.snxprj` write completion, the Add-Existing / Add-New-Schematic file-picker
    /// completions, and the async git-commit completion.
    pub(crate) fn dispatch_project_message(&mut self, msg: ProjectMsg) -> Task<Message> {
        match msg {
            ProjectMsg::Setup(msg) => self.dispatch_new_project_message(msg),
            ProjectMsg::DocumentSetup(msg) => self.dispatch_document_setup_message(msg),
            ProjectMsg::SetupProfilesWritten { note, undo, result } => {
                self.setup_profiles_written(note, *undo, result);
                Task::none()
            }
            ProjectMsg::StartScreen(msg) => self.dispatch_start_screen_message(msg),
            ProjectMsg::Thumbnail(msg) => self.dispatch_project_thumbnail_message(msg),
            ProjectMsg::SheetNumbering(msg) => self.dispatch_sheet_numbering_message(msg),
            ProjectMsg::CloseConfirm(choice) => self.handle_project_close_confirm(choice),
            ProjectMsg::AssignFootprints(msg) => self.dispatch_assign_footprints_message(msg),
            ProjectMsg::CompiledDesign(msg) => self.dispatch_compiled_design_message(msg),
            ProjectMsg::CloseRefsResolved { project_id, result } => {
                self.handle_close_refs_resolved(project_id, result)
            }
            ProjectMsg::CaseProbed {
                project_id,
                outcome,
            } => self.handle_case_probed(project_id, outcome),
            ProjectMsg::DocumentHeadersRead { project_id, reads } => {
                self.handle_document_headers_read(project_id, reads)
            }
            ProjectMsg::FsFactsProbed(generation, facts) => {
                self.handle_project_fs_probed(generation, facts)
            }
            ProjectMsg::NavigatorAssembled(generation, assembly) => {
                self.handle_navigator_assembled(generation, assembly)
            }
            ProjectMsg::DescendRequested {
                window,
                from,
                symbol,
                path,
                placement,
            } => {
                let task = self.descend_requested(window, &from, symbol, path, placement);
                Task::batch([task, self.finish_update()])
            }
            ProjectMsg::PrimitiveOpenRequested(path) => {
                let task = self.handle_open_primitive(path);
                Task::batch([task, self.finish_update()])
            }
            ProjectMsg::SheetOpenRequested { path, open } => {
                let task = self.open_sheet(path, *open);
                Task::batch([task, self.finish_update()])
            }
            ProjectMsg::WindowHierarchyAssembled(root, generation, assembly) => {
                self.handle_window_hierarchy_assembled(&root, generation, assembly)
            }
            ProjectMsg::AppQuitConfirm(choice) => self.handle_app_quit_confirm(choice),
            ProjectMsg::Tab(msg) => self.dispatch_project_tab_message(msg),
            ProjectMsg::Rules(msg) => self.dispatch_rules_tab_message(msg),
            ProjectMsg::FileSaved(done) => self.handle_project_file_saved(*done),
            ProjectMsg::AddExistingFilePicked { project_id, paths } => {
                self.handle_add_existing_file_picked(project_id, paths)
            }
            ProjectMsg::AddExistingFilesCopied {
                project_id,
                project_dir,
                results,
            } => self.handle_add_existing_files_copied(project_id, &project_dir, results),
            ProjectMsg::AddNewSchematicWritten {
                project_id,
                path,
                result,
                designators,
            } => {
                self.handle_add_new_schematic_written(project_id, path, result, designators);
                Task::none()
            }
            ProjectMsg::CreateChildSheetPathChanged(text) => {
                self.handle_create_child_sheet_path_changed(text)
            }
            ProjectMsg::CreateChildSheetBrowse => self.handle_create_child_sheet_browse(),
            ProjectMsg::CreateChildSheetBrowsed(picked) => {
                self.handle_create_child_sheet_browsed(picked)
            }
            ProjectMsg::CreateChildSheetAnswered { create } => {
                self.handle_create_child_sheet_answered(create)
            }
            ProjectMsg::CreateChildSheetOpenExisting => {
                self.handle_create_child_sheet_open_existing()
            }
            ProjectMsg::ChildSheetCreated {
                project_id,
                parent,
                symbol,
                path,
                result,
            } => self.handle_child_sheet_created(project_id, &parent, symbol, path, result),
            ProjectMsg::AddNewSchematicPicked {
                project_id,
                path,
                choice,
            } => self.handle_add_new_schematic_picked(project_id, path, choice),
            ProjectMsg::GitCommitDone {
                project_root,
                rel_path,
                result,
            } => self.handle_project_git_commit_done(project_root, rel_path, result),
            ProjectMsg::ProjectFolderRenamed { job, result } => {
                self.handle_project_folder_renamed(&job, result)
            }
            ProjectMsg::SheetFilesListed { ticket, result } => {
                self.handle_sheet_files_listed(ticket, result)
            }
            ProjectMsg::ProjectFileRenamed { job, result } => {
                self.handle_project_file_renamed(&job, result)
            }
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

    /// Right-click on a sheet canvas: end an armed placement, else open the
    /// canvas context menu owned by `window` (`None` = the main window).
    fn show_canvas_context_menu(
        &mut self,
        x: f32,
        y: f32,
        window: Option<iced::window::Id>,
    ) -> Task<Message> {
        // Altium convention: right-click during placement terminates
        // the placement flow (tool-stuck OR ghost-armed OR pending
        // power/port OR paused preview OR net-colour pen armed)
        // instead of opening the context menu. On a board that is a
        // footprint Move in progress (#2240).
        // A Move left on a board this window no longer shows is forgotten,
        // so it cannot swallow a right-click on a sheet.
        self.drop_stale_pcb_carry();
        if self.has_own_pcb_carry() {
            self.end_pcb_carry();
            return Task::none();
        }
        // A board placement tool closes a Solid Region, or ends its shape and
        // then itself (#2373).
        if self.close_board_region() || self.step_back_board_tool() {
            return Task::none();
        }
        let canvas = self.interaction_state.active_canvas();
        let placement_active = self.interaction_state.current_tool != Tool::Select
            || canvas.ghost_label.is_some()
            || canvas.ghost_symbol.is_some()
            || canvas.ghost_text.is_some()
            || canvas.placement_paused
            || self.interaction_state.placement.is_some()
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
        // A dropdown another window opened is closed, not left invisible and
        // blocking this window's menu.
        if self.interaction_state.active_bar_menu.is_some()
            && self.interaction_state.active_bar_menu_window != window
        {
            self.interaction_state.active_bar_menu = None;
            self.interaction_state.active_bar_menu_window = None;
        }
        if self.interaction_state.active_bar_menu.is_none() {
            let world = self.cursor_world_now();
            self.close_all_context_menus();
            self.interaction_state.context_menu = Some(ContextMenuState {
                x,
                y,
                world,
                window,
            });
        }
        self.read_reuse_blocks_in_view()
    }

    /// Context-menu subsystem handler (canvas / project-tree / tab menus +
    /// submenu hover state machine), namespaced (ADR-0001 D3).
    pub(crate) fn dispatch_context_menu_message(&mut self, msg: ContextMenuMsg) -> Task<Message> {
        match msg {
            ContextMenuMsg::ScopedToWindow(window, inner) => match *inner {
                // Hover state is global, not per document: no re-entry.
                hover @ (ContextMenuMsg::SubmenuHover(_)
                | ContextMenuMsg::SubmenuLeave
                | ContextMenuMsg::SubmenuEnterPanel
                | ContextMenuMsg::SubmenuLeavePanel) => self.dispatch_context_menu_message(hover),
                ContextMenuMsg::Show(x, y) => self.run_in_window_document(window, |app| {
                    app.show_canvas_context_menu(x, y, app.overlay_owner(window))
                }),
                // A canvas menu's command row is gated and run against the
                // window the menu opened in, which the dispatch enters
                // itself (#2267).
                pick @ ContextMenuMsg::Action(ContextAction::Command { .. })
                    if self.interaction_state.context_menu.is_some() =>
                {
                    self.dispatch_context_menu_message(pick)
                }
                // Expanding or rebuilding the tree edits the window's own
                // tree; opening a tab, a dialog or a file is queued to the
                // main window like Edit Symbol (#2037).
                ContextMenuMsg::ProjectTreeAction(action)
                    if !action.changes_only_the_shown_tree() =>
                {
                    self.handle_project_tree_action(action)
                }
                other => self
                    .run_in_window_document(window, |app| app.dispatch_context_menu_message(other)),
            },
            ContextMenuMsg::Show(x, y) => self.show_canvas_context_menu(x, y, None),
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
                self.close_all_context_menus();
                let (x, y) = self.interaction_state.last_mouse_pos;
                self.interaction_state.project_tree_context_menu =
                    Some(crate::app::ProjectTreeContextMenuState {
                        x,
                        y,
                        key,
                        window: self.interaction_state.acting_window,
                    });
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
                self.close_all_context_menus();
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
                // The tab strip's "+N" menu shares the tab menu's Esc rung.
                self.ui_state.tab_overflow_menu_open = false;
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
                let menu = self.interaction_state.context_menu.take();
                let opened_at = menu.as_ref().and_then(|menu| menu.world);
                // The window the canvas menu opened in; `None` is the main one.
                let menu_window = menu.and_then(|menu| menu.window);
                // The project tree's menu closes with the canvas one: a row of
                // either runs a command, and neither may outlive the pick.
                self.interaction_state.project_tree_context_menu = None;
                self.interaction_state.context_submenu = None;
                self.interaction_state.pending_submenu = None;
                self.interaction_state.submenu_launcher_hovered = None;
                self.interaction_state.submenu_panel_hovered = false;
                self.interaction_state.submenu_unhovered_since = None;
                match action {
                    ContextAction::Command { id, project } => {
                        let args = crate::app::command::CommandArgs::in_context(
                            crate::app::command::InvocationContext {
                                origin: opened_at,
                                project,
                            },
                        );
                        // Gated and run in the menu's window, as a key or the
                        // undocked menu bar's `UiMsg::MenuInWindow` is.
                        self.dispatch_command(&id, args, menu_window)
                    }
                    ContextAction::OpenChildSheet => {
                        self.open_selected_child_sheet().unwrap_or_else(Task::none)
                    }
                    ContextAction::CreateChildSheet => match self.selected_child_sheet() {
                        Some((symbol, _)) => self.create_child_sheet_from_symbol(symbol),
                        None => Task::none(),
                    },
                    ContextAction::SyncSheetPins => {
                        self.sync_selected_sheet_pins();
                        // This dispatcher does not run `finish_update`; publish
                        // the outcome message to the Messages panel now.
                        self.sync_diagnostics_panel_ctx();
                        Task::none()
                    }
                    ContextAction::ActiveBar(active_bar_action) => {
                        self.handle_active_bar_action(active_bar_action)
                    }
                    ContextAction::ReuseBlock(event) => self.handle_reuse_block(event),
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
            AnnotateMsg::ReprefixPressed if self.annotate_preview_is_loading() => Task::none(),
            AnnotateMsg::ReprefixPressed => {
                let task = self.handle_reprefix_pressed();
                // Its refusals are Messages lines, on screen in this frame.
                self.sync_diagnostics_panel_ctx();
                task
            }
            AnnotateMsg::ReprefixRowsResolved { generation, result } => {
                let task = self.handle_reprefix_rows_resolved(generation, &result);
                self.sync_diagnostics_panel_ctx();
                task
            }
            AnnotateMsg::ReprefixSheetsRead { generation, result } => {
                let task = self.handle_reprefix_sheets_read(generation, &result);
                self.sync_diagnostics_panel_ctx();
                task
            }
            AnnotateMsg::ReprefixConfirmed => {
                let task = self.handle_reprefix_confirmed();
                self.sync_diagnostics_panel_ctx();
                task
            }
            AnnotateMsg::ReprefixCancelled => {
                let task = self.handle_reprefix_cancelled();
                self.sync_diagnostics_panel_ctx();
                task
            }
            // The preview reloads when the run ends.
            AnnotateMsg::Run(mode) => self.handle_annotate(mode),
            AnnotateMsg::PassSheetsRead { generation, result } => {
                let task = self.handle_annotate_sheets_read(generation, &result);
                self.sync_diagnostics_panel_ctx();
                task
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
            // The menu's Reset Schematic Designators is Reset & Renumber.
            AnnotateMsg::OpenResetConfirm => {
                self.handle_annotate_reset_pressed(signex_engine::AnnotateMode::ResetAndRenumber)
            }
            AnnotateMsg::ResetPressed(mode) => self.handle_annotate_reset_pressed(mode),
            AnnotateMsg::ListViewPicked(view) => {
                self.ui_state.annotate_session.list_view = view;
                Task::none()
            }
            AnnotateMsg::ListFilterEdited(filter) => {
                self.ui_state.annotate_session.filter = filter;
                Task::none()
            }
            AnnotateMsg::SheetTickToggled(path) => {
                self.handle_annotate_sheet_tick_toggled(path);
                Task::none()
            }
            AnnotateMsg::AllSheetsTicked(on) => {
                self.handle_annotate_all_sheets_ticked(on);
                Task::none()
            }
            AnnotateMsg::FixedOptionsToggled => {
                let session = &mut self.ui_state.annotate_session;
                session.fixed_options_open = !session.fixed_options_open;
                Task::none()
            }
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
            ErcMsg::StoreRulesInProject => self.handle_store_erc_rules_in_project(),
            ErcMsg::PinJunctionRepairRequested => {
                self.repair_pin_junctions();
                Task::none()
            }
            ErcMsg::ProfileSaveDialogOpened => self.handle_profile_save_dialog_opened(),
            ErcMsg::ProfileSaveNameChanged(name) => self.handle_profile_save_name_changed(name),
            ErcMsg::ProfileSaveVersionChanged(version) => {
                self.handle_profile_save_version_changed(version)
            }
            ErcMsg::ProfileSaveSubmitted => self.handle_profile_save_submitted(),
            ErcMsg::ProfileSaveOverwriteConfirmed => self.handle_profile_save_overwrite_confirmed(),
            ErcMsg::ProfileSaveCancelled => self.handle_profile_save_cancelled(),
            ErcMsg::UserDefaultLoaded(result) => self.handle_user_default_loaded(&result),
            ErcMsg::ProfileWriteFinished(result) => self.handle_profile_write_finished(&result),
            ErcMsg::ProfileWritesFlushed(result) => Self::handle_profile_writes_flushed(&result),
            ErcMsg::ProfileSaved { project, result } => self.handle_profile_saved(project, &result),
            ErcMsg::ProfileUpdateRequested => self.handle_profile_update_requested(),
            ErcMsg::ProfileLoadedForUpdate { project, result } => {
                self.handle_profile_loaded_for_update(project, &result)
            }
            ErcMsg::ProfileUpdateConfirmed => self.handle_profile_update_confirmed(),
            ErcMsg::ProfileUpdateCancelled => self.handle_profile_update_cancelled(),
            ErcMsg::ProfileDriftChecked { project, result } => {
                self.handle_profile_drift_checked(project, &result)
            }
            ErcMsg::DefaultProfileMigrated(result) => self.handle_default_profile_migrated(&result),
            ErcMsg::RulesUndone => {
                let changed = self.undo_project_rules();
                self.rerun_erc_after_rules_edit(changed)
            }
            ErcMsg::RulesRedone => {
                let changed = self.redo_project_rules();
                self.rerun_erc_after_rules_edit(changed)
            }
            ErcMsg::PinMatrixCellCycled { row, col } => {
                self.handle_pin_matrix_cell_cycled(row, col)
            }
            ErcMsg::RulesChangedOnlyToggled(on) => {
                self.ui_state.erc_rule_list.changed_only = on;
                Task::none()
            }
            ErcMsg::RuleFilterChanged(query) => {
                self.ui_state.erc_rule_list.query = query;
                Task::none()
            }
            ErcMsg::ProfileMenuToggled => self.handle_erc_profile_menu_toggled(),
            ErcMsg::ProfileMenuRowChosen(action) => {
                self.ui_state.erc_profile_menu_open = false;
                self.dispatch_erc_message(action.message())
            }
        }
    }

    /// Cycle one pin-matrix cell of the project's stored rules, or of the
    /// machine's rules when the project stores none.
    pub(crate) fn handle_pin_matrix_cell_cycled(&mut self, row: u8, col: u8) -> Task<Message> {
        if self.edit_project_erc_rules(|rules| {
            signex_erc::project_rules::cycle_pin_cell(rules, row, col)
        }) {
            return Task::none();
        }
        self.cycle_machine_pin_cell(row, col)
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
        // Same reason: undocked windows hold their own copy of the tokens.
        self.sync_window_panel_ctx_shared();
        task
    }

    /// Enable Version Control modal family handler (namespaced, ADR-0001 D3).
    pub(crate) fn dispatch_enable_version_control_message(
        &mut self,
        msg: EnableVersionControlMsg,
    ) -> Task<Message> {
        match msg {
            edit @ (EnableVersionControlMsg::ToggleLfs
            | EnableVersionControlMsg::ToggleItem(_)
            | EnableVersionControlMsg::AllTicked
            | EnableVersionControlMsg::CommitMessageChanged(_)
            | EnableVersionControlMsg::CommitEverySaveToggled) => {
                self.edit_enable_version_control(edit);
                Task::none()
            }
            EnableVersionControlMsg::Confirm => self.handle_enable_version_control_confirm(),
            EnableVersionControlMsg::ItemsScanned {
                project_dir,
                result,
            } => {
                self.handle_enable_version_control_items_scanned(&project_dir, result);
                Task::none()
            }
            EnableVersionControlMsg::Finished {
                scope,
                project_dir,
                outcome,
            } => {
                self.handle_enable_version_control_finished(scope, project_dir, outcome);
                Task::none()
            }
            EnableVersionControlMsg::Close => {
                self.close_enable_version_control_dialog();
                Task::none()
            }
        }
    }

    /// Rename modal family handler (namespaced, ADR-0001 D3).
    pub(crate) fn dispatch_rename_message(&mut self, msg: RenameMsg) -> Task<Message> {
        match msg {
            RenameMsg::BufferChanged(s) => {
                self.handle_rename_buffer_changed(s);
                Task::none()
            }
            RenameMsg::Submit => self.handle_rename_submit(),
            RenameMsg::Close => {
                // A folder move in flight reports its result into this
                // dialog; closing it would hide that outcome.
                if !self.document_state.project_folder_renames.is_empty() {
                    return Task::none();
                }
                self.ui_state.rename_dialog = None;
                Task::none()
            }
            RenameMsg::FileRenamed(done) => self.handle_file_renamed(*done),
            RenameMsg::LibraryRenamed(done) => self.handle_library_renamed(*done),
            RenameMsg::LibraryRenameRolledBack(done) => {
                self.handle_library_rename_rolled_back(*done)
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
            RemoveMsg::Deleted(done) => self.handle_path_deleted(*done),
            // Selecting a card removes nothing; the footer action does.
            RemoveMsg::ChoicePicked(choice) => {
                if let Some(dialog) = self.ui_state.remove_dialog.as_mut() {
                    dialog.choice = Some(choice);
                }
                Task::none()
            }
        }
    }

    /// The Remove or Close Project confirm when its card is in front of the
    /// main window: the overlay Esc would answer (#2262).
    pub(crate) fn confirm_in_front(&self) -> Option<crate::app::state::ModalId> {
        use crate::app::state::ModalId;
        match self.escape_overlay_message()? {
            Message::Remove(RemoveMsg::Close) => Some(ModalId::RemoveDialog),
            Message::Project(ProjectMsg::CloseConfirm(crate::app::ProjectCloseChoice::Cancel)) => {
                Some(ModalId::ProjectCloseConfirm)
            }
            _ => None,
        }
    }

    /// What Enter does in `modal`'s card: Close Project saves all. `None`:
    /// Enter does nothing there, as in Remove, where only a click confirms.
    pub(crate) fn confirm_enter_message(
        &self,
        modal: crate::app::state::ModalId,
    ) -> Option<Message> {
        use crate::app::state::ModalId;
        match modal {
            ModalId::ProjectCloseConfirm => {
                self.ui_state.project_close_confirm.is_some().then(|| {
                    Message::Project(ProjectMsg::CloseConfirm(
                        crate::app::ProjectCloseChoice::SaveAll,
                    ))
                })
            }
            _ => None,
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

/// Persist the first-run tour dismissal on a blocking worker, never in
/// `update` (#1469).
pub(in crate::app) fn dismiss_first_run_tour_task() -> Task<Message> {
    crate::fonts::background_pref_write("first_run_tour_dismissed", || {
        crate::fonts::write_first_run_tour_dismissed(true)
    })
}

impl Signex {
    /// Close the first-run tour card on a deliberate pointer gesture on a
    /// canvas ([`crate::canvas::CanvasEvent::is_user_gesture`]) and persist
    /// that, off the update thread. Nothing for a hover or a key's command.
    pub(super) fn dismiss_first_run_tour_on(
        &mut self,
        event: &crate::canvas::CanvasEvent,
    ) -> Task<Message> {
        if event.is_user_gesture() {
            self.dismiss_first_run_tour()
        } else {
            Task::none()
        }
    }

    /// Close the first-run tour card if it is open, and persist that.
    pub(super) fn dismiss_first_run_tour(&mut self) -> Task<Message> {
        if !self.ui_state.first_run_tour_open {
            return Task::none();
        }
        self.ui_state.first_run_tour_open = false;
        dismiss_first_run_tour_task()
    }

    /// Close every right-click menu (canvas, document tab, project tree,
    /// dock tab, Components row, Start screen row, board layer tab) and
    /// their submenu state. Each menu's open handler calls this first, so
    /// exactly one is ever open.
    pub(crate) fn close_all_context_menus(&mut self) {
        let interaction = &mut self.interaction_state;
        interaction.context_menu = None;
        interaction.tab_context_menu = None;
        interaction.start_row_menu = None;
        interaction.pcb_layer_tab_menu = None;
        interaction.pcb_layer_set_menu = None;
        interaction.project_tree_context_menu = None;
        interaction.context_submenu = None;
        interaction.pending_submenu = None;
        interaction.submenu_launcher_hovered = None;
        interaction.submenu_panel_hovered = false;
        interaction.submenu_unhovered_since = None;
        self.document_state.dock.tab_menu = None;
        self.library.components_panel.row_menu = None;
        self.ui_state.tab_overflow_menu_open = false;
    }
}

#[cfg(test)]
#[path = "context_menu_window_tests.rs"]
mod context_menu_window_tests;
