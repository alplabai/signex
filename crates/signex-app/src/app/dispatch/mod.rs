use iced::Task;

use super::*;

mod document;
mod overlay;
mod routed;
mod text_edit;
mod tool;
mod ui;

impl Signex {
    pub(crate) fn dispatch_update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Menu(_) | Message::Tab(_) | Message::Dock(_) | Message::Selection(_) => {
                self.dispatch_routed_message(message)
            }
            Message::ThemeChanged(_)
            | Message::UnitCycled
            | Message::GridToggle
            | Message::CanvasEvent(_)
            | Message::DragStart(_)
            | Message::DragMove(_, _)
            | Message::WindowResized(_, _)
            | Message::DragEnd
            | Message::GridCycle
            | Message::StatusBar(_) => self.dispatch_ui_message(message),
            Message::TextEditChanged(_) | Message::TextEditSubmit => {
                self.dispatch_text_edit_message(message)
            }
            Message::PrePlacementTab
            | Message::ResumePlacement
            | Message::CycleDrawMode
            | Message::CancelDrawing
            | Message::Tool(_) => self.dispatch_tool_message(message),
            Message::FileOpened(_)
            | Message::DeleteSelected
            | Message::Undo
            | Message::Redo
            | Message::RotateSelected
            | Message::MirrorSelectedX
            | Message::MirrorSelectedY
            | Message::Cut
            | Message::Copy
            | Message::Paste
            | Message::SmartPaste
            | Message::Duplicate
            | Message::SaveFile
            | Message::SaveFileAs(_)
            | Message::SchematicLoaded(_) => self.dispatch_document_message(message),
            Message::TogglePanelList
            | Message::OpenPanel(_)
            | Message::OpenFind
            | Message::OpenReplace
            | Message::OpenPreferences
            | Message::ClosePreferences
            | Message::PreferencesNav(_)
            | Message::PreferencesMsg(_)
            | Message::FindReplaceMsg(_)
            | Message::ActiveBar(_)
            | Message::ShowContextMenu(_, _)
            | Message::CloseContextMenu
            | Message::ContextAction(_)
            | Message::CloseTabConfirm(_)
            | Message::RunErc
            | Message::Annotate(_)
            | Message::OpenAnnotateDialog
            | Message::CloseAnnotateDialog
            | Message::AnnotateOrderChanged(_)
            | Message::OpenErcDialog
            | Message::CloseErcDialog
            | Message::ErcSeverityChanged(_, _)
            | Message::OpenAnnotateResetConfirm
            | Message::CloseAnnotateResetConfirm
            | Message::ModalDragStart { .. }
            | Message::ModalDragEnd
            | Message::FocusAt { .. }
            | Message::ToggleAutoFocus => self.dispatch_overlay_message(message),
            Message::WindowResizedFor(id, w, h) => {
                // Only main-window resizes drive layout math. Detached
                // modal + undocked-tab windows have their own sizes
                // that would otherwise clobber the main-window state.
                if self.ui_state.main_window_id == Some(id) {
                    self.ui_state.window_size = (w, h);
                }
                Task::none()
            }
            Message::MainWindowOpened(id) => {
                self.ui_state.main_window_id = Some(id);
                // Pull the real initial size from winit — opening the
                // window at Settings.size doesn't always land at
                // exactly that size (OS DPI scaling, display clamps).
                // Without this, Active-Bar dropdown positions are off
                // until the user physically resizes the window.
                iced::window::size(id)
                    .map(move |size| Message::WindowResizedFor(id, size.width, size.height))
            }
            Message::SecondaryWindowClosed(id) => {
                // Drop the entry and dismiss the backing modal state so
                // closing the OS window fully exits the modal instead of
                // reattaching a phantom copy to the main window on the
                // next view frame. Phase 3 will add undocked-tab cleanup
                // here too.
                if let Some(kind) = self.ui_state.windows.remove(&id) {
                    use super::state::{ModalId, WindowKind};
                    match kind {
                        WindowKind::DetachedModal(modal) => match modal {
                            ModalId::AnnotateDialog => {
                                self.ui_state.annotate_dialog_open = false
                            }
                            ModalId::AnnotateResetConfirm => {
                                self.ui_state.annotate_reset_confirm = false
                            }
                            ModalId::ErcDialog => self.ui_state.erc_dialog_open = false,
                            ModalId::Preferences => {
                                self.ui_state.preferences_open = false
                            }
                            ModalId::FindReplace => {
                                self.ui_state.find_replace.open = false
                            }
                            ModalId::CloseTabConfirm => {
                                self.ui_state.close_tab_confirm = None
                            }
                            ModalId::MoveSelection => {
                                self.ui_state.move_selection.open = false
                            }
                            ModalId::NetColorPalette => {
                                self.ui_state.net_color_palette_open = false
                            }
                            ModalId::ParameterManager => {
                                self.ui_state.parameter_manager_open = false
                            }
                        },
                        // Closing an undocked-tab window is the reattach
                        // gesture — no additional state to reset since the
                        // tab itself stays in document_state.tabs.
                        WindowKind::UndockedTab { .. } => {}
                        // Closing a detached panel reattaches it as a
                        // docked panel in the right column so the user
                        // doesn't lose access to the panel kind.
                        WindowKind::DetachedPanel(kind) => {
                            self.document_state.dock.add_panel(
                                crate::dock::PanelPosition::Right,
                                kind,
                            );
                        }
                    }
                }
                Task::none()
            }
            Message::DetachModal(modal) => self.handle_detach_modal(modal),
            Message::DetachedModalOpened { modal, id } => {
                self.ui_state
                    .windows
                    .insert(id, super::state::WindowKind::DetachedModal(modal));
                // Any lingering drag state belongs to the main window —
                // once the modal is popped out, the OS handles window
                // drags directly.
                self.ui_state.modal_dragging = None;
                Task::none()
            }
            Message::UndockTab(idx) => self.handle_undock_tab(idx),
            Message::UndockedTabOpened { path, id } => {
                let title = self
                    .document_state
                    .tabs
                    .iter()
                    .find(|t| t.path == path)
                    .map(|t| t.title.clone())
                    .unwrap_or_default();
                self.ui_state
                    .windows
                    .insert(id, super::state::WindowKind::UndockedTab { path, title });
                Task::none()
            }
            Message::ReattachTab(id) => {
                self.ui_state.windows.remove(&id);
                iced::window::close(id)
            }
            Message::DetachFloatingPanel(idx) => self.handle_detach_floating_panel(idx),
            Message::DetachedPanelOpened { kind, id } => {
                self.ui_state
                    .windows
                    .insert(id, super::state::WindowKind::DetachedPanel(kind));
                Task::none()
            }
            Message::StartDetachedWindowDrag(modal) => {
                self.handle_start_detached_window_drag(modal)
            }
            Message::OpenMoveSelectionDialog => self.handle_open_move_selection_dialog(),
            Message::CloseMoveSelectionDialog => {
                let _ = self.handle_close_move_selection_dialog();
                self.close_detached_modal(super::state::ModalId::MoveSelection)
            }
            Message::MoveSelectionDxChanged(s) => {
                self.ui_state.move_selection.dx = s;
                Task::none()
            }
            Message::MoveSelectionDyChanged(s) => {
                self.ui_state.move_selection.dy = s;
                Task::none()
            }
            Message::MoveSelectionApply => self.handle_move_selection_apply(),
            Message::OpenNetColorPalette => {
                self.ui_state.net_color_palette_open = true;
                self.handle_detach_modal(super::state::ModalId::NetColorPalette)
            }
            Message::CloseNetColorPalette => {
                self.ui_state.net_color_palette_open = false;
                self.close_detached_modal(super::state::ModalId::NetColorPalette)
            }
            Message::NetColorSet { net, color } => {
                if let Some(c) = color {
                    self.ui_state.net_colors.insert(net, c);
                } else {
                    self.ui_state.net_colors.remove(&net);
                }
                self.interaction_state.canvas.clear_content_cache();
                Task::none()
            }
            Message::OpenParameterManager => {
                self.ui_state.parameter_manager_open = true;
                self.handle_detach_modal(super::state::ModalId::ParameterManager)
            }
            Message::CloseParameterManager => {
                self.ui_state.parameter_manager_open = false;
                self.close_detached_modal(super::state::ModalId::ParameterManager)
            }
            Message::ParameterManagerEdit {
                symbol_uuid,
                key,
                value,
            } => self.handle_parameter_manager_edit(symbol_uuid, key, value),
            Message::AnnotateToggleLock(uuid) => {
                if self.ui_state.annotate_locked.contains(&uuid) {
                    self.ui_state.annotate_locked.remove(&uuid);
                } else {
                    self.ui_state.annotate_locked.insert(uuid);
                }
                Task::none()
            }
            Message::NetColorCustomShow(show) => {
                self.ui_state.net_color_custom.show = show;
                Task::none()
            }
            Message::NetColorCustomDraft(c) => {
                self.ui_state.net_color_custom.draft = c;
                Task::none()
            }
            Message::NetColorCustomSubmit(c) => {
                self.ui_state.net_color_custom.show = false;
                self.ui_state.net_color_custom.draft = c;
                let color = signex_types::theme::Color {
                    r: (c.r * 255.0).round() as u8,
                    g: (c.g * 255.0).round() as u8,
                    b: (c.b * 255.0).round() as u8,
                    a: 255,
                };
                self.ui_state.pending_net_color = Some(color);
                self.interaction_state.canvas.pending_net_color = Some(color);
                Task::none()
            }
            Message::NetColorCustomChannel(chan, s) => {
                // Parse as u8; silently ignore invalid input so the
                // text_input doesn't reject intermediate values like
                // the empty string while the user types.
                let parsed = s.trim().parse::<u16>().unwrap_or(0).min(255) as u8;
                let draft = &mut self.ui_state.net_color_custom.draft;
                let v = parsed as f32 / 255.0;
                match chan {
                    super::contracts::Channel::R => draft.r = v,
                    super::contracts::Channel::G => draft.g = v,
                    super::contracts::Channel::B => draft.b = v,
                }
                Task::none()
            }
            Message::LassoCommit => {
                if let Some(pts) = self.ui_state.lasso_polygon.take() {
                    if pts.len() >= 3
                        && let Some(snapshot) = self.active_render_snapshot()
                    {
                        let poly: Vec<(f64, f64)> =
                            pts.iter().map(|p| (p.x, p.y)).collect();
                        let filters = self.interaction_state.selection_filters.clone();
                        self.interaction_state.canvas.selected =
                            signex_render::schematic::hit_test::hit_test_polygon(
                                snapshot, &poly,
                            )
                            .into_iter()
                            .filter(|h| {
                                super::handlers::selection_workflow::passes_filter(
                                    h, snapshot, &filters,
                                )
                            })
                            .collect();
                        self.update_selection_info();
                    }
                }
                self.interaction_state.canvas.lasso_polygon = None;
                self.interaction_state.canvas.clear_overlay_cache();
                Task::none()
            }
            Message::CycleSelectionMode => {
                use signex_render::schematic::hit_test::SelectionMode;
                self.ui_state.selection_mode = match self.ui_state.selection_mode {
                    SelectionMode::Inside => SelectionMode::Touching,
                    SelectionMode::Touching => SelectionMode::Inside,
                };
                crate::diagnostics::log_info(&format!(
                    "Selection mode: {:?}",
                    self.ui_state.selection_mode
                ));
                Task::none()
            }
            Message::PinMatrixCellCycled { row, col } => {
                use signex_erc::Severity;
                // Baseline defaults must match the `MATRIX` constant in
                // `pin_matrix_view` so "clearing" an override drops back
                // to the same severity the user sees in the UI.
                const BASELINE: [[Severity; 6]; 6] = [
                    [Severity::Off, Severity::Off, Severity::Off, Severity::Off, Severity::Off, Severity::Off],
                    [Severity::Off, Severity::Error, Severity::Off, Severity::Off, Severity::Error, Severity::Error],
                    [Severity::Off, Severity::Off, Severity::Off, Severity::Off, Severity::Off, Severity::Warning],
                    [Severity::Off, Severity::Off, Severity::Off, Severity::Off, Severity::Off, Severity::Error],
                    [Severity::Off, Severity::Error, Severity::Off, Severity::Off, Severity::Error, Severity::Error],
                    [Severity::Off, Severity::Error, Severity::Warning, Severity::Error, Severity::Error, Severity::Off],
                ];
                let key = (row, col);
                let baseline = BASELINE
                    .get(row as usize)
                    .and_then(|r| r.get(col as usize))
                    .copied()
                    .unwrap_or(Severity::Off);
                let current = self
                    .ui_state
                    .pin_matrix_overrides
                    .get(&key)
                    .copied()
                    .unwrap_or(baseline);
                let next = match current {
                    Severity::Error => Severity::Warning,
                    Severity::Warning => Severity::Info,
                    Severity::Info => Severity::Off,
                    Severity::Off => Severity::Error,
                };
                if next == baseline {
                    self.ui_state.pin_matrix_overrides.remove(&key);
                } else {
                    self.ui_state.pin_matrix_overrides.insert(key, next);
                }
                crate::fonts::write_pin_matrix_overrides(
                    &self.ui_state.pin_matrix_overrides,
                );
                Task::none()
            }
            Message::Noop => Task::none(),
        }
    }
}
