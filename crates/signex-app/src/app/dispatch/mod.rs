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
            Message::MainWindowOpened(id) => {
                self.ui_state.main_window_id = Some(id);
                Task::none()
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
            Message::Noop => Task::none(),
        }
    }
}
