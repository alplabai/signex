use iced::Task;

use super::*;

mod board_status;
pub(crate) mod chord;
mod command_palette;
mod document;
mod escape;
#[cfg(test)]
mod first_run_tour_tests;
#[cfg(test)]
mod grid_properties_tests;
pub(crate) mod input;
mod keyboard;
mod keymap;
pub(crate) mod library;
mod main_canvas_work;
#[cfg(test)]
mod net_color_tests;
mod overlay;
mod pcb;
pub(in crate::app) use overlay::dismiss_first_run_tour_task;
#[cfg(test)]
mod prefs_async_tests;
mod text_edit;
mod tool;
mod ui;
pub(in crate::app) mod window_action;
mod window_surfaces;

impl Signex {
    pub(crate) fn dispatch_update(&mut self, message: Message) -> Task<Message> {
        self.apply_pcb_renderer_dirty_hint(&message);

        match message {
            Message::PassiveCalculator(message) => {
                self.ui_state.passive_calculator.update(message);
                Task::none()
            }
            Message::Menu(msg) => self.handle_menu_message(msg),
            Message::Tab { window_id, msg } => {
                let task = self.handle_document_tab_message(window_id, msg);
                self.close_find_similar_if_sheet_changed();
                self.close_paste_array_if_sheet_changed();
                Task::batch([self.finish_update(), task])
            }
            Message::Dock(msg) => self.handle_dock_message(msg),
            Message::Input(msg) => self.dispatch_input_message(msg),
            Message::Library(msg) => self.dispatch_library_message(msg),
            Message::CommandPalette(msg) => self.dispatch_command_palette_message(msg),
            Message::Selection(request) => self.handle_selection_request(request),
            // Idle cursor move (the subscription forwards every one): only
            // the cached position changes, so skip the drag handlers and
            // `finish_update`'s per-event work (#111).
            // Dispatchers that skip `finish_update` leave the panel_ctx
            // mirrors / History dirty bit stale; the cheap (IO-free)
            // repair stays here, along with the drain + diagnostics
            // republish safety net (no-ops when nothing is queued).
            Message::Ui(UiMsg::DragMove(x, y)) if !self.is_layout_drag_active() => {
                self.interaction_state.last_mouse_pos = (x, y);
                self.sync_panel_ctx_mirrors();
                self.sync_diagnostics_panel_ctx();
                let history = self.refresh_history_panel_cheap();
                Task::batch([history, self.drain_pending_git_commits()])
            }
            Message::Ui(msg) => self.dispatch_ui_message(msg),
            // Canvas events keep their own top-level variants (hot path,
            // many emit sites). A deliberate pointer gesture dismisses the
            // first-run tour card (UX §4.3); a hover, and the fit and centre
            // commands a key or the menu sends, do not (#1994).
            Message::CanvasEvent(event) => {
                let dismissal = self.dismiss_first_run_tour_on(&event);
                Task::batch([dismissal, self.handle_canvas_interaction_event(event)])
            }
            Message::CanvasEventInWindow { window_id, event } => {
                let dismissal = self.dismiss_first_run_tour_on(&event);
                Task::batch([
                    dismissal,
                    self.handle_canvas_event_in_window(window_id, event),
                ])
            }
            Message::GridProperties(msg) => self.dispatch_grid_properties_message(msg),
            Message::SelectionFilter(msg) => self.dispatch_selection_filter_message(msg),
            Message::TextEdit(msg) => self.dispatch_text_edit_message(msg),
            Message::Tool(msg) => self.dispatch_tool_message(msg),
            Message::Edit(msg) => self.dispatch_edit_message(msg),
            Message::File(msg) => self.dispatch_file_message(msg),
            Message::Agent(msg) => self.dispatch_agent_message(msg),
            Message::Export(msg) => self.dispatch_export_message(msg),
            Message::PrintPreview(msg) => self.dispatch_print_preview_message(msg),
            Message::BomPreview(msg) => self.dispatch_bom_preview_message(msg),
            Message::Overlay(msg) => self.dispatch_overlay_message(msg),
            Message::FindReplaceMsg(msg) => self.handle_find_replace_message(msg),
            Message::ActiveBar(msg) => self.handle_active_bar_message(msg),
            Message::Project(msg) => {
                let task = self.dispatch_project_message(msg);
                Task::batch([task, self.finish_update()])
            }
            Message::ContextMenu(msg) => {
                // A right-click on the canvas is a gesture too (#1994).
                let dismissal = match msg {
                    ContextMenuMsg::Show(..) => self.dismiss_first_run_tour(),
                    _ => Task::none(),
                };
                // The menu takes the cursor: a card the canvas showed must go.
                if matches!(
                    msg,
                    ContextMenuMsg::Show(..)
                        | ContextMenuMsg::ShowProjectTree(_)
                        | ContextMenuMsg::ShowTab(_)
                ) {
                    self.clear_canvas_hover();
                }
                Task::batch([dismissal, self.dispatch_context_menu_message(msg)])
            }
            Message::Annotate(msg) => self.dispatch_annotate_message(msg),
            Message::Erc(msg) => self.dispatch_erc_message(msg),
            Message::Preferences(msg) => self.dispatch_preferences_message(msg),
            Message::EnableVersionControl(msg) => self.dispatch_enable_version_control_message(msg),
            Message::Rename(msg) => self.dispatch_rename_message(msg),
            Message::Remove(msg) => self.dispatch_remove_message(msg),
            Message::Window(msg) => self.dispatch_window_message(msg),
            Message::DockLayout(msg) => self.dispatch_dock_layout_message(msg),
            Message::SearchPrefs(msg) => self.dispatch_search_prefs_message(msg),
            Message::Autosave(msg) => self.dispatch_autosave_message(msg),
            Message::MoveSelection(msg) => self.dispatch_move_selection_message(msg),
            Message::NetColor(msg) => self.dispatch_net_color_message(msg),
            Message::ParameterManager(msg) => self.dispatch_parameter_manager_message(msg),
            Message::PcbCanvas(msg) => self.dispatch_pcb_canvas_message(msg),
            Message::Pcb(msg) => self.dispatch_pcb_message(msg),
            Message::CrossProbe(msg) => self.dispatch_cross_probe_message(msg),
            Message::UpdatePcb(msg) => self.dispatch_update_pcb_message(msg),
            Message::PanZoomBench(msg) => self.dispatch_pan_zoom_bench_message(msg),
            Message::DiffPairClasses(msg) => self.dispatch_diff_pair_classes_message(msg),
            Message::GerberViewer(msg) => self.dispatch_gerber_viewer_message(msg),
            Message::NotebookGenerate(msg) => self.dispatch_notebook_generate_message(msg),
        }
    }

    /// Window lifecycle, docking, and native-chrome message family
    /// (namespaced, ADR-0001 D3). Main + secondary window open / close /
    /// resize / scale, detach + reattach of modals, undocked tabs and
    /// floating panels, and the borderless-chrome drag / resize /
    /// minimize / maximize buttons. Routed from `dispatch_update`.
    pub(crate) fn dispatch_window_message(&mut self, message: WindowMsg) -> Task<Message> {
        match message {
            WindowMsg::WindowResizedFor(id, w, h) => {
                // A minimised window reports 0x0: keep the last real size,
                // so nothing is fitted or laid out against it, and the
                // restore arrives as a resize to the size already stored.
                if !UiState::is_real_window_size((w, h)) {
                    return Task::none();
                }
                let settle = self.note_window_geometry_changed(id);
                // Only main-window resizes drive layout math. Detached
                // modal + undocked-tab windows have their own sizes
                // that would otherwise clobber the main-window state.
                if self.ui_state.main_window_id == Some(id) {
                    let before = self.canvas_bounds();
                    self.ui_state.window_size = (w, h);
                    self.follow_main_canvas_resize(before);
                    return Task::batch([settle, self.main_window_mode_task(id)]);
                }
                // An undocked tab window's own size places its overlays.
                self.follow_undocked_window_resize(id, (w, h));
                self.ui_state.set_undocked_window_size(id, (w, h));
                settle
            }
            WindowMsg::WindowMoved(id) => self.note_window_geometry_changed(id),
            WindowMsg::WindowGeometrySettled(id) => self.window_geometry_settled(id),
            WindowMsg::WindowGeometryRead(readout) => self.window_geometry_read(*readout),
            WindowMsg::WindowGeometrySaved(result) => self.window_geometry_saved(result),
            WindowMsg::TileRequested(layout) => self.tile(layout),
            WindowMsg::UntileRequested => self.untile_documents(),
            WindowMsg::TilePanePressed(tab) => self.focus_tile_pane(tab),
            WindowMsg::WindowScaleChanged(id, scale) => {
                // The CPU canvas caches its geometry, which was drawn with the
                // old scale; a move to another display keeps the logical
                // bounds, so nothing else would redraw it (#1887).
                // A window without a canvas (a detached modal or panel) has
                // nothing to redraw, and must not clear the main canvas.
                let has_canvas = self.ui_state.main_window_id == Some(id)
                    || self.interaction_state.canvases.contains_key(&id);
                let changed = self.ui_state.set_window_os_scale(id, scale);
                if changed && self.ui_state.main_window_id == Some(id) {
                    self.document_state.panel_ctx.device_px_per_logical_px =
                        self.ui_state.main_device_px_per_logical_px();
                }
                if changed && has_canvas {
                    let slot = self.interaction_state.canvas_for_window_mut(id);
                    slot.clear_bg_cache();
                    slot.clear_content_cache();
                }
                Task::none()
            }
            WindowMsg::MainWindowOpened(id) => {
                self.ui_state.main_window_id = Some(id);
                self.ui_state.main_window_opened = true;
                // Pull the real initial size from winit — opening the
                // window at Settings.size doesn't always land at
                // exactly that size (OS DPI scaling, display clamps).
                // Without this, Active-Bar dropdown positions are off
                // until the user physically resizes the window.
                let size_task = iced::window::size(id).map(move |size| {
                    Message::Window(WindowMsg::WindowResizedFor(id, size.width, size.height))
                });
                // Read the scale factor: it picks the wordmark PNG tier and
                // sets the stroke floor of the canvases. Later changes (a
                // move to another screen) arrive as `Rescaled` events.
                let scale_task = window_scale_task(id);
                // Re-add Windows 11 DWM rounded corners + drop shadow
                // (silently no-ops on Windows 10 and non-Windows). Has
                // to run after the HWND is alive, hence here rather than
                // in bootstrap.
                let corners_task = crate::chrome::apply_rounded_corners::<Message>(id);
                let settle = self.note_window_geometry_changed(id);
                iced::Task::batch([size_task, scale_task, corners_task, settle])
            }
            WindowMsg::SecondaryWindowClosed(id) => {
                // Main window closed → terminate the process.
                if self.ui_state.main_window_id == Some(id) {
                    return self.flush_quit_writes_then(iced::exit());
                }
                self.ui_state.forget_window_os_scale(id);
                self.forget_window_geometry(id);
                self.ui_state.forget_undocked_window_size(id);
                self.close_overlays_owned_by(id);
                if self.ui_state.focused_window == Some(id) {
                    self.ui_state.focused_window = None;
                }
                let mut layout_persist = Task::none();
                let mut quit_answer = Task::none();
                let mut annotate_close = Task::none();
                // Drop the entry and dismiss the backing modal state so
                // closing the OS window fully exits the modal instead of
                // reattaching a phantom copy to the main window on the
                // next view frame. Phase 3 will add undocked-tab cleanup
                // here too.
                let owner = self.ui_state.library_modal_owners.remove(&id);
                self.ui_state.modal_window_sizes.remove(&id);
                if let Some(kind) = self.ui_state.windows.remove(&id) {
                    use super::state::{ModalId, WindowKind};
                    match kind {
                        WindowKind::DetachedModal(modal) => match modal {
                            // The dialog's own close: its confirms go with it.
                            ModalId::AnnotateDialog => {
                                annotate_close = self.handle_close_annotate_dialog();
                            }
                            ModalId::AnnotateResetConfirm => {
                                self.ui_state.annotate_reset_confirm = false
                            }
                            ModalId::ReprefixConfirm => self.ui_state.reprefix_confirm = None,
                            ModalId::ErcDialog => self.ui_state.erc_dialog_open = false,
                            ModalId::Preferences => {
                                // Backstop for a Preferences window destroyed
                                // outside the guarded close paths (which all
                                // refuse to close while dirty): if unsaved
                                // live previews are still active, revert them
                                // so e.g. the experimental PCB GPU render
                                // toggle can't linger enabled with the
                                // checkbox showing unchecked. Gated on dirty
                                // so a clean close doesn't pay the full
                                // revert (keymap-profile rebuild + canvas
                                // cache clears — a visible re-tessellation
                                // hitch on a large board) for a no-op.
                                if self.ui_state.preferences_dirty {
                                    self.revert_preferences_drafts();
                                }
                                self.ui_state.preferences_open = false;
                            }
                            ModalId::FindReplace => self.ui_state.find_replace.open = false,
                            ModalId::MoveSelection => self.ui_state.move_selection.open = false,
                            ModalId::NetColorPalette => {
                                self.ui_state.net_color_palette_open = false
                            }
                            ModalId::ParameterManager => {
                                self.end_parameter_edit_runs();
                                self.ui_state.parameter_manager_open = false;
                                *self.ui_state.parameter_table = Default::default();
                            }
                            ModalId::RenameDialog => self.ui_state.rename_dialog = None,
                            ModalId::RemoveDialog => self.ui_state.remove_dialog = None,
                            ModalId::PrintPreview => self.dismiss_print_preview(),
                            ModalId::BomPreview => self.document_state.bom_preview = None,
                            ModalId::EnableVersionControl => {
                                self.close_enable_version_control_dialog();
                            }
                            ModalId::GridProperties => {
                                self.ui_state.grid_properties = None;
                            }
                            ModalId::SelectionFilterCustom => {
                                self.ui_state.selection_filter_custom = None;
                            }
                            // A Cancel, unless Create is writing: then the
                            // draft stays and the sync reopens the window.
                            ModalId::NewProject => {
                                if self
                                    .ui_state
                                    .new_project
                                    .as_ref()
                                    .is_some_and(|s| s.creating.is_none())
                                {
                                    self.ui_state.new_project = None;
                                }
                            }
                            ModalId::About => self.ui_state.about_open = false,
                            ModalId::KeyboardShortcuts => {
                                self.ui_state.keyboard_shortcuts_open = false
                            }
                            ModalId::PassiveCalculator => {
                                self.ui_state.passive_calculator_open = false
                            }
                            ModalId::FirstRunTour => self.ui_state.first_run_tour_open = false,
                            ModalId::CreateChildSheet => {
                                self.ui_state.create_child_sheet_confirm = None
                            }
                            // A window that went away without a close
                            // request is a Cancel, never a Discard.
                            ModalId::ErrorNotice => self.document_state.error_notice = None,
                            ModalId::NetlistIncompletePrompt => {
                                self.document_state.netlist_incomplete_prompt = None
                            }
                            ModalId::AutosaveRecovery => self.note_recovery_window_vanished(),
                            ModalId::ProjectCloseConfirm => {
                                self.ui_state.project_close_confirm = None
                            }
                            // A prompt window that vanished is a Cancel: the
                            // pending system quit request is answered too.
                            ModalId::AppQuitConfirm => {
                                if self.ui_state.app_quit_confirm.take().is_some() {
                                    quit_answer = Self::decline_quit();
                                }
                            }
                            // A window that vanished without a close request
                            // is a Cancel: nothing is written or deleted.
                            ModalId::LibraryPicker => self.library.picker = None,
                            ModalId::LibraryPrimitivePicker => self.library.primitive_picker = None,
                            ModalId::LibraryDocumentOptions => self.library.document_options = None,
                            ModalId::LibraryCreateOptions => self.library.create_options = None,
                            ModalId::LibraryStockCopy => self.library.stock_copy = None,
                            ModalId::LibraryUpdates => self.library.library_updates = None,
                            ModalId::LibraryCloseConfirm => {
                                self.library.close_library_confirm = None
                            }
                            ModalId::NetColorCustom => self.ui_state.net_color_custom.show = false,
                            // Never auto-decided: the sync shows it again.
                            ModalId::LibraryRecovery => self.note_recovery_window_vanished(),
                            // The edit form keeps its typed text: the window comes
                            // back, twice at most, then it stays in the window.
                            ModalId::LibraryEditRow => {
                                if let Some(path) = owner {
                                    let count = self
                                        .ui_state
                                        .edit_row_window_vanished
                                        .entry(path)
                                        .or_insert(0);
                                    *count += 1;
                                    if *count == 2 {
                                        crate::diagnostics::log_warning(
                                            "An Edit Component Details window closed twice \
                                             unanswered; it now stays in the main window",
                                        );
                                    }
                                }
                            }
                            // A confirm that vanished is a Cancel.
                            ModalId::LibraryDeleteConfirm => {
                                if let Some(path) = owner
                                    && let Some(browser) =
                                        self.library.library_browsers.get_mut(&path)
                                {
                                    browser.delete_confirm = None;
                                }
                            }
                        },
                        // Closing an undocked-tab window is the reattach
                        // gesture — the tab itself stays in
                        // document_state.tabs. Drop the per-window
                        // canvas so we don't leak caches for a window
                        // that's gone.
                        WindowKind::UndockedTab { path, .. } => {
                            self.end_wire_of_window(id);
                            self.end_pause_of_window(id);
                            self.interaction_state.canvases.remove(&id);
                            self.interaction_state.pcb_canvases.remove(&id);
                            self.interaction_state.pcb_selections.remove(&id);
                            self.forget_pcb_carry_of_window(id);
                            self.show_redocked_tab(&path);
                            self.settle_document_tiling();
                        }
                        // Closing a detached panel reattaches it to the
                        // dock so the user doesn't lose access to the
                        // panel kind. It goes back to its home region
                        // rather than always to the right column, and
                        // `show_panel` makes it the active tab there.
                        // The `windows.remove` above already dropped
                        // this window, so the dock call is the whole
                        // job — no detached copy is left to focus.
                        WindowKind::DetachedPanel(kind) => {
                            self.document_state.dock.show_panel(kind);
                            layout_persist = self.persist_dock_layout_soon();
                        }
                        // The Component Preview lives as a tab in the
                        // main window; its state outlasts the
                        // detached OS window. Closing the OS window
                        // re-docks the editor to the main-window tab
                        // bar — `library.editors` keeps the in-flight
                        // edits keyed by `(library_path, table,
                        // row_id)`, and the main-window tab
                        // already exists, so there's nothing to do
                        // here beyond letting the window-id mapping
                        // drop above.
                        WindowKind::ComponentEditor { .. } => {}
                    }
                }
                Task::batch([layout_persist, quit_answer, annotate_close])
            }
            WindowMsg::DetachModal(modal) => self.handle_detach_modal(modal),
            WindowMsg::DetachedModalOpened { modal, id } => {
                // handle_detach_modal already inserted the entry when the
                // open was requested.  Re-inserting here is fine when the
                // window is still tracked, but if close_detached_modal
                // already removed it before this callback arrived (a race
                // where the user closes the dialog before the OS confirms
                // the window open), re-inserting would leave a stale entry
                // that blocks every subsequent open attempt.
                if self.ui_state.windows.contains_key(&id) {
                    self.ui_state
                        .windows
                        .insert(id, super::state::WindowKind::DetachedModal(modal));
                }
                // Any lingering drag state belongs to the main window —
                // once the modal is popped out, the OS handles window
                // drags directly.
                self.ui_state.modal_dragging = None;
                // Win11 DWM rounded corners on the detached window so
                // its edges visually match the modal_card's 8 px
                // radius. Silent no-op on Win10 / non-Windows.
                // Without this the OS paints the window with hard
                // corners and the modal_card's rounded border is
                // hidden inside a square OS frame.
                //
                // Focus the window as it settles. A freshly-opened
                // detached window inherits `Level::Normal` and is not
                // raised by the OS, so it can appear *behind* the main
                // window — the user would have to move the main window
                // to find the dialog. Focusing here is permitted:
                // signex owns the foreground at the moment it opens the
                // modal, so the OS honours the request (unlike a
                // foreground grab from a background process).
                Task::batch([
                    self.focus_new_project_name_once_open(modal),
                    self.focus_move_selection_dx_once_open(modal),
                    self.focus_rename_field_once_open(modal),
                    crate::chrome::apply_rounded_corners::<Message>(id),
                    iced::window::gain_focus(id),
                    window_scale_task(id),
                ])
            }
            WindowMsg::UndockTab(idx) => self.handle_undock_tab(idx),
            WindowMsg::UndockedTabOpened { path, id } => {
                // The entry was inserted when the open was requested; if
                // the window closed before the OS confirmed the open, do
                // not resurrect it (stale window + canvas leak).
                if !self.ui_state.windows.contains_key(&id) {
                    return Task::none();
                }
                let title = self
                    .document_state
                    .tabs
                    .iter()
                    .find(|t| t.path == path)
                    .map(|t| t.title.clone())
                    .unwrap_or_default();
                self.ui_state.windows.insert(
                    id,
                    super::state::WindowKind::UndockedTab {
                        path: path.clone(),
                        title,
                    },
                );
                // Spin up a fresh canvas for this window, seeded from
                // the engine that the tab points at so the new window
                // renders the correct schematic from its first frame.
                // Pan/zoom/selection start at CanvasSlot::new
                // defaults — independent of the main canvas.
                // The tab need not be main's active one: its sheet names its
                // paper. A tab with no sheet keeps the main canvas's.
                let main = &self.interaction_state.canvas;
                let paper = self.document_state.engines.get(&path).map_or(
                    (main.paper_width_mm, main.paper_height_mm),
                    |engine| {
                        let size =
                            super::projector::effective_paper_size(&engine.document().paper_size);
                        crate::panels::paper_dimensions(&size)
                    },
                );
                let per_window = self.seeded_canvas_slot(&path, paper);
                self.interaction_state.canvases.insert(id, per_window);
                // A PCB tab gets its own PCB canvas too, so panning one
                // window no longer moves the other (#494).
                if let Some(per_window_pcb) = self.new_pcb_canvas_for(&path) {
                    self.interaction_state
                        .pcb_canvases
                        .insert(id, per_window_pcb);
                }
                // The main window no longer shows it: main's commands must
                // not keep editing the document that moved away.
                self.leave_undocked_active_tab(&path);
                self.refresh_window_panel_ctxs(None);
                // One pane fewer in the main window (#2249).
                self.settle_document_tiling();
                // Raise + focus the undocked tab window — like the
                // detached modal, it opens at `Level::Normal` and can
                // land behind the main window otherwise.
                Task::batch([iced::window::gain_focus(id), window_scale_task(id)])
            }
            WindowMsg::ReattachTab(id) => {
                // Pre-remove so the tab bar shows the reattached tab on
                // the next view frame even before the OS-level close
                // fires `SecondaryWindowClosed`. Clear the per-window
                // canvas here too — otherwise `SecondaryWindowClosed`
                // short-circuits on `windows.remove -> None` and the
                // canvas cache + selection leak.
                self.close_overlays_owned_by(id);
                self.ui_state.forget_undocked_window_size(id);
                let reattached = self.ui_state.windows.remove(&id);
                self.end_wire_of_window(id);
                self.end_pause_of_window(id);
                self.interaction_state.canvases.remove(&id);
                self.interaction_state.pcb_canvases.remove(&id);
                self.interaction_state.pcb_selections.remove(&id);
                self.forget_pcb_carry_of_window(id);
                if let Some(super::state::WindowKind::UndockedTab { path, .. }) = reattached {
                    self.show_redocked_tab(&path);
                }
                // The tab is back in the main window, as a pane when tiled.
                self.settle_document_tiling();
                iced::window::close(id)
            }
            WindowMsg::DetachFloatingPanel(idx) => self.handle_detach_floating_panel(idx),
            WindowMsg::DetachedPanelOpened { kind, id } => {
                // The entry was inserted when the open was requested; if
                // the window closed before the OS confirmed the open, do
                // not resurrect it (stale window + canvas leak).
                if !self.ui_state.windows.contains_key(&id) {
                    return Task::none();
                }
                self.ui_state
                    .windows
                    .insert(id, super::state::WindowKind::DetachedPanel(kind));
                // Raise + focus the detached panel window — same
                // `Level::Normal` open-behind issue as the modal/tab.
                Task::batch([iced::window::gain_focus(id), window_scale_task(id)])
            }
            // Dock back closes the panel's window; `SecondaryWindowClosed`
            // then re-docks the panel in its home region, as the close does.
            WindowMsg::DetachedPanelDockBackPressed(id) => {
                if matches!(
                    self.ui_state.windows.get(&id),
                    Some(super::state::WindowKind::DetachedPanel(_))
                ) {
                    iced::window::close(id)
                } else {
                    Task::none()
                }
            }
            WindowMsg::StartMainWindowDrag => {
                // Title-bar drag diagnosis: the press reached a drag zone.
                let window = self.ui_state.main_window_id;
                tracing::debug!(?window, "title bar: drag zone pressed");
                match window {
                    Some(id) => crate::chrome::start_window_drag(id),
                    None => Task::none(),
                }
            }
            WindowMsg::StartMainWindowResize(direction) => match self.ui_state.main_window_id {
                Some(id) => crate::chrome::start_window_resize(id, direction),
                None => Task::none(),
            },
            WindowMsg::DialogTitleBarPressed(id) => crate::chrome::start_window_drag(id),
            WindowMsg::DialogEdgePressed(id, direction) => {
                crate::chrome::start_window_resize(id, direction)
            }
            WindowMsg::DialogCloseClicked(id) => {
                self.update(Message::Window(WindowMsg::WindowCloseRequested(id)))
            }
            WindowMsg::MinimizeMainWindow => match self.ui_state.main_window_id {
                Some(id) => iced::window::minimize(id, true),
                None => Task::none(),
            },
            WindowMsg::ToggleMaximizeMainWindow => match self.ui_state.main_window_id {
                Some(id) => iced::window::toggle_maximize(id),
                None => Task::none(),
            },
            WindowMsg::CloseMainWindow => self.handle_app_quit_requested(),
            WindowMsg::LockedWindowClicked => match self.locking_confirm_window() {
                Some(id) => iced::window::gain_focus(id),
                None => Task::none(),
            },
            WindowMsg::WindowFocused(id) => {
                self.ui_state.focused_window = Some(id);
                Task::none()
            }
            WindowMsg::WindowUnfocused(id) => {
                if self.ui_state.focused_window == Some(id) {
                    self.ui_state.focused_window = None;
                }
                // No edge pan behind Alt+Tab (#2872).
                self.forget_edge_pan_pointers();
                self.end_pcb_drag_on_focus_loss(id)
            }
            WindowMsg::NativeMenuRowChosen(action) => self.handle_native_menu_row_chosen(action),
            WindowMsg::NativeMenuFailed(reason) => self.handle_native_menu_failed(&reason),
            WindowMsg::QuitRequested => {
                crate::diagnostics::log_info("Quit: the system asked Signex to quit");
                self.handle_app_quit_requested()
            }
            WindowMsg::ControlsLayoutDetected(layout) => {
                self.handle_controls_layout_detected(layout)
            }
            WindowMsg::MainWindowMonitorSizeRead(size) => {
                self.ui_state.main_window_monitor_size = size;
                Task::none()
            }
            WindowMsg::MainWindowModeRead(mode) => {
                self.ui_state.main_window_mode = mode;
                Task::none()
            }
            WindowMsg::WindowCloseRequested(id) => {
                // OS close request (Alt+F4 / native close). Windows are
                // opened with `exit_on_close_request: false`, so route the main window through
                // the unsaved-changes guard, a detached modal through its
                // own close message, and close any other window directly.
                if self.ui_state.main_window_id == Some(id) {
                    // The window is gone before another read could finish.
                    self.persist_main_geometry_on_close();
                    self.handle_app_quit_requested()
                } else if let Some(super::state::WindowKind::DetachedModal(modal)) =
                    self.ui_state.windows.get(&id).cloned()
                    && let Some(close) = self.window_close_message(id, modal)
                {
                    // The native close button of a detached modal runs
                    // the modal's own close message, so the dirty guard
                    // (Preferences keeps the window open until the user
                    // saves or discards), the state reset and the window
                    // close are the same as for the in-body close.
                    self.update(close)
                } else {
                    iced::window::close(id)
                }
            }
        }
    }
}

/// Ask winit for a window's OS scale; the answer lands as
/// `WindowMsg::WindowScaleChanged` (#1887).
fn window_scale_task(id: iced::window::Id) -> Task<Message> {
    iced::window::scale_factor(id)
        .map(move |s| Message::Window(WindowMsg::WindowScaleChanged(id, s)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dock::{DockMessage, PanelPosition, PanelSite};
    use crate::panels::PanelKind;

    /// A detached Projects panel that was last docked in the bottom region,
    /// not its home (left).
    fn detached_from_bottom(app: &mut Signex) -> iced::window::Id {
        let kind = PanelKind::Projects;
        let dock = &mut app.document_state.dock;
        dock.close_panel(kind);
        dock.add_panel(PanelPosition::Bottom, kind);
        let Some(PanelSite::Docked(pos, idx)) = dock.locate(kind) else {
            panic!("Projects is docked at the bottom");
        };
        dock.update(DockMessage::UndockPanel(pos, idx));
        let Some(PanelSite::Floating(idx)) = dock.locate(kind) else {
            panic!("Projects floats");
        };
        let _ = app.update(Message::Window(WindowMsg::DetachFloatingPanel(idx)));
        let detached = app.ui_state.windows.iter().find_map(|(id, window)| {
            matches!(
                window,
                super::super::state::WindowKind::DetachedPanel(PanelKind::Projects)
            )
            .then_some(*id)
        });
        let Some(id) = detached else {
            panic!("Projects has a detached window");
        };
        assert!(app.document_state.dock.locate(kind).is_none());
        id
    }

    fn home_region_holds_projects(app: &Signex) -> bool {
        matches!(
            app.document_state.dock.locate(PanelKind::Projects),
            Some(PanelSite::Docked(PanelPosition::Left, _))
        )
    }

    /// DECISIONS 2026-11-20 (h): Dock back re-docks a detached panel in its
    /// home region, not the last region it was in (mockup row 8).
    #[test]
    fn dock_back_re_docks_a_detached_panel_in_its_home_region() {
        let (mut app, _boot) = Signex::new();
        let id = detached_from_bottom(&mut app);

        let close = app.update(Message::Window(WindowMsg::DetachedPanelDockBackPressed(id)));
        assert_ne!(close.units(), 0, "Dock back closes the window");
        let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(id)));

        assert!(home_region_holds_projects(&app));
        assert!(
            !app.document_state
                .dock
                .panel_kinds(PanelPosition::Bottom)
                .contains(&PanelKind::Projects)
        );
    }

    /// The window's close re-docks the same way as Dock back.
    #[test]
    fn closing_a_detached_panel_window_re_docks_it_in_its_home_region() {
        let (mut app, _boot) = Signex::new();
        let id = detached_from_bottom(&mut app);

        let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(id)));

        assert!(!app.ui_state.windows.contains_key(&id));
        assert!(home_region_holds_projects(&app));
    }
}
