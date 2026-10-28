use iced::Task;

use super::*;

mod command_palette;
mod document;
mod escape;
pub(crate) mod input;
mod keyboard;
mod keymap;
pub(crate) mod library;
#[cfg(test)]
mod net_color_tests;
mod overlay;
#[cfg(test)]
mod prefs_async_tests;
mod text_edit;
mod tool;
mod ui;

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
            // many emit sites). Both dismiss the first-run tour card on
            // the first gesture (UX §4.3) before running the handler.
            Message::CanvasEvent(event) => {
                if self.ui_state.first_run_tour_open {
                    self.ui_state.first_run_tour_open = false;
                    let save = overlay::dismiss_first_run_tour_task();
                    return Task::batch([save, self.handle_canvas_interaction_event(event)]);
                }
                self.handle_canvas_interaction_event(event)
            }
            Message::CanvasEventInWindow { window_id, event } => {
                if self.ui_state.first_run_tour_open {
                    self.ui_state.first_run_tour_open = false;
                    let save = overlay::dismiss_first_run_tour_task();
                    return Task::batch([
                        save,
                        self.handle_canvas_event_in_window(window_id, event),
                    ]);
                }
                self.handle_canvas_event_in_window(window_id, event)
            }
            Message::GridProperties(msg) => self.dispatch_grid_properties_message(msg),
            Message::SelectionFilter(msg) => self.dispatch_selection_filter_message(msg),
            Message::TextEdit(msg) => self.dispatch_text_edit_message(msg),
            Message::Tool(msg) => self.dispatch_tool_message(msg),
            Message::Edit(msg) => self.dispatch_edit_message(msg),
            Message::File(msg) => self.dispatch_file_message(msg),
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
            Message::ContextMenu(msg) => self.dispatch_context_menu_message(msg),
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
                // Only main-window resizes drive layout math. Detached
                // modal + undocked-tab windows have their own sizes
                // that would otherwise clobber the main-window state.
                if self.ui_state.main_window_id == Some(id) {
                    self.ui_state.window_size = (w, h);
                    // Windows fires a resize event whenever DWM moves
                    // the window to a monitor with a different DPI, so
                    // re-querying the scale factor here keeps the
                    // wordmark-PNG tier picker in sync after a
                    // cross-monitor drag.
                    let scale_task = iced::window::scale_factor(id)
                        .map(|s| Message::Window(WindowMsg::MainWindowScaleChanged(s)));
                    return Task::batch([scale_task, self.main_window_mode_task(id)]);
                }
                Task::none()
            }
            WindowMsg::MainWindowScaleChanged(scale) => {
                self.ui_state.main_window_scale = scale;
                Task::none()
            }
            WindowMsg::MainWindowOpened(id) => {
                self.ui_state.main_window_id = Some(id);
                self.ui_state.main_window_opened = true;
                // winit's default menu exists now; ours replaces it on the
                // sync that ends this update.
                #[cfg(target_os = "macos")]
                self.native_menu.arm();
                // Pull the real initial size from winit — opening the
                // window at Settings.size doesn't always land at
                // exactly that size (OS DPI scaling, display clamps).
                // Without this, Active-Bar dropdown positions are off
                // until the user physically resizes the window.
                let size_task = iced::window::size(id).map(move |size| {
                    Message::Window(WindowMsg::WindowResizedFor(id, size.width, size.height))
                });
                // Stash the scale factor so the wordmark PNG picker
                // can render at native device-pixel count. Re-queried
                // on every resize to track monitor moves.
                let scale_task = iced::window::scale_factor(id)
                    .map(|s| Message::Window(WindowMsg::MainWindowScaleChanged(s)));
                // Re-add Windows 11 DWM rounded corners + drop shadow
                // (silently no-ops on Windows 10 and non-Windows). Has
                // to run after the HWND is alive, hence here rather than
                // in bootstrap.
                let corners_task = crate::chrome::apply_rounded_corners::<Message>(id);
                iced::Task::batch([size_task, scale_task, corners_task])
            }
            WindowMsg::SecondaryWindowClosed(id) => {
                // Main window closed → terminate the process.
                if self.ui_state.main_window_id == Some(id) {
                    return self.flush_quit_writes_then(iced::exit());
                }
                if self.ui_state.focused_window == Some(id) {
                    self.ui_state.focused_window = None;
                }
                let mut layout_persist = Task::none();
                // Drop the entry and dismiss the backing modal state so
                // closing the OS window fully exits the modal instead of
                // reattaching a phantom copy to the main window on the
                // next view frame. Phase 3 will add undocked-tab cleanup
                // here too.
                let owner = self.ui_state.library_modal_owners.remove(&id);
                if let Some(kind) = self.ui_state.windows.remove(&id) {
                    use super::state::{ModalId, WindowKind};
                    match kind {
                        WindowKind::DetachedModal(modal) => match modal {
                            ModalId::AnnotateDialog => self.ui_state.annotate_dialog_open = false,
                            ModalId::AnnotateResetConfirm => {
                                self.ui_state.annotate_reset_confirm = false
                            }
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
                                self.ui_state.parameter_manager_open = false
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
                            ModalId::AppQuitConfirm => self.ui_state.app_quit_confirm = None,
                            // A window that vanished without a close request
                            // is a Cancel: nothing is written or deleted.
                            ModalId::LibraryPicker => self.library.picker = None,
                            ModalId::LibraryPrimitivePicker => self.library.primitive_picker = None,
                            ModalId::LibraryDocumentOptions => self.library.document_options = None,
                            ModalId::LibraryCreateOptions => self.library.create_options = None,
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
                        WindowKind::UndockedTab { .. } => {
                            self.interaction_state.canvases.remove(&id);
                            self.interaction_state.pcb_canvases.remove(&id);
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
                layout_persist
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
                    crate::chrome::apply_rounded_corners::<Message>(id),
                    iced::window::gain_focus(id),
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
                let mut per_window = crate::canvas::CanvasSlot::new();
                if let Some(engine) = self.document_state.engines.get(&path) {
                    per_window.set_render_cache(Some(
                        crate::schematic_runtime::SchematicRenderCache::from_sheet_at(
                            engine.document(),
                            engine.revision(),
                        ),
                    ));
                }
                // #631 — theme, snap, grid size/style and grid visibility
                // used to be mirrored here so the new window would not
                // flash the defaults. They are read from `UiState` in
                // `view` now, so a fresh window is correct on its first
                // frame with nothing copied. Paper size stays: it belongs
                // to *this* window's document, not to the app.
                per_window.paper_width_mm = self.interaction_state.canvas.paper_width_mm;
                per_window.paper_height_mm = self.interaction_state.canvas.paper_height_mm;
                // The sheet frame belongs to this window's document too;
                // without it the undocked window draws no frame until the
                // next paper change.
                per_window.sheet_frame = self.sheet_frame_for_path(
                    &path,
                    per_window.paper_width_mm,
                    per_window.paper_height_mm,
                );
                per_window.designators = self.designators_for_path(&path);
                per_window.fit_to_content();
                self.interaction_state.canvases.insert(id, per_window);
                // A PCB tab gets its own PCB canvas too, so panning one
                // window no longer moves the other (#494).
                if let Some(per_window_pcb) = self.new_pcb_canvas_for(&path) {
                    self.interaction_state
                        .pcb_canvases
                        .insert(id, per_window_pcb);
                }
                // Raise + focus the undocked tab window — like the
                // detached modal, it opens at `Level::Normal` and can
                // land behind the main window otherwise.
                iced::window::gain_focus(id)
            }
            WindowMsg::ReattachTab(id) => {
                // Pre-remove so the tab bar shows the reattached tab on
                // the next view frame even before the OS-level close
                // fires `SecondaryWindowClosed`. Clear the per-window
                // canvas here too — otherwise `SecondaryWindowClosed`
                // short-circuits on `windows.remove -> None` and the
                // canvas cache + selection leak.
                self.ui_state.windows.remove(&id);
                self.interaction_state.canvases.remove(&id);
                self.interaction_state.pcb_canvases.remove(&id);
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
                iced::window::gain_focus(id)
            }
            WindowMsg::StartMainWindowDrag => match self.ui_state.main_window_id {
                Some(id) => crate::chrome::start_window_drag(id),
                None => Task::none(),
            },
            WindowMsg::StartMainWindowResize(direction) => match self.ui_state.main_window_id {
                Some(id) => crate::chrome::start_window_resize(id, direction),
                None => Task::none(),
            },
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
                Task::none()
            }
            WindowMsg::NativeMenuItemActivated(id) => self.handle_native_menu_activated(&id),
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

    /// Per-net colour override + F5-palette handler (namespaced family,
    /// ADR-0001 D3).
    pub(crate) fn dispatch_net_color_message(&mut self, msg: NetColorMsg) -> Task<Message> {
        match msg {
            NetColorMsg::Open => {
                self.ui_state.net_color_palette_open = true;
                self.handle_detach_modal(super::state::ModalId::NetColorPalette)
            }
            NetColorMsg::Close => {
                self.ui_state.net_color_palette_open = false;
                self.close_detached_modal(super::state::ModalId::NetColorPalette)
            }
            NetColorMsg::Set { net, color } => {
                if let Some(c) = color {
                    self.ui_state.net_colors.insert(net, c);
                } else {
                    self.ui_state.net_colors.remove(&net);
                }
                for canvas in self.interaction_state.all_schematic_canvases_mut() {
                    canvas.clear_content_cache();
                }
                Task::none()
            }
            NetColorMsg::CustomShow(show) => {
                self.ui_state.net_color_custom.show = show;
                Task::none()
            }
            NetColorMsg::CustomDraft(c) => {
                self.ui_state.net_color_custom.draft = c;
                Task::none()
            }
            NetColorMsg::CustomSubmit(c) => {
                self.ui_state.net_color_custom.show = false;
                self.ui_state.net_color_custom.draft = c;
                let color = signex_types::theme::Color {
                    r: (c.r * 255.0).round() as u8,
                    g: (c.g * 255.0).round() as u8,
                    b: (c.b * 255.0).round() as u8,
                    a: 255,
                };
                self.ui_state.pending_net_color = Some(color);
                self.interaction_state.active_canvas_mut().pending_net_color = Some(color);
                Task::none()
            }
            NetColorMsg::CustomChannel(chan, s) => {
                // Parse as u8: values above 255 (however large) clamp to
                // 255. Input that does not parse (e.g. the empty string
                // while the user types) keeps the channel's previous value,
                // so the text_input never rejects an intermediate value and
                // a half-typed edit never zeroes the channel.
                let Ok(parsed) = s.trim().parse::<u64>() else {
                    return Task::none();
                };
                let parsed = parsed.min(255) as u8;
                let draft = &mut self.ui_state.net_color_custom.draft;
                let v = parsed as f32 / 255.0;
                match chan {
                    super::contracts::Channel::R => draft.r = v,
                    super::contracts::Channel::G => draft.g = v,
                    super::contracts::Channel::B => draft.b = v,
                }
                Task::none()
            }
        }
    }

    /// Parameter Manager dialog handler (namespaced family, ADR-0001 D3).
    pub(crate) fn dispatch_parameter_manager_message(
        &mut self,
        msg: ParameterManagerMsg,
    ) -> Task<Message> {
        match msg {
            ParameterManagerMsg::Open => {
                self.ui_state.parameter_manager_open = true;
                self.handle_detach_modal(super::state::ModalId::ParameterManager)
            }
            ParameterManagerMsg::Close => {
                self.ui_state.parameter_manager_open = false;
                self.close_detached_modal(super::state::ModalId::ParameterManager)
            }
            ParameterManagerMsg::Edit {
                symbol_uuid,
                key,
                value,
            } => self.handle_parameter_manager_edit(symbol_uuid, key, value),
        }
    }
}
