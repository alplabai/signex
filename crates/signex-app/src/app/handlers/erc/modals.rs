//! ERC/annotate dialogs, modal drag, undock/detach handlers. Split from `handlers/erc.rs`.

use iced::Task;

use super::super::super::state::ModalId;
use super::super::super::*;

use crate::library::messages::LibraryMessage;

impl Signex {
    pub(crate) fn handle_open_erc_dialog(&mut self) -> Task<Message> {
        self.ui_state.erc_dialog_open = true;
        self.interaction_state.context_menu = None;
        let drift = self.check_profile_drift_task();
        let detach = self.handle_detach_modal(super::super::super::state::ModalId::ErcDialog);
        Task::batch([detach, drift])
    }

    pub(crate) fn handle_close_erc_dialog(&mut self) -> Task<Message> {
        self.ui_state.erc_dialog_open = false;
        self.ui_state.pending_profile_update = None;
        self.ui_state.save_profile_dialog = None;
        self.close_detached_modal(super::super::super::state::ModalId::ErcDialog)
    }

    pub(crate) fn handle_erc_severity_changed(
        &mut self,
        rule: signex_erc::RuleKind,
        severity: signex_erc::Severity,
    ) -> Task<Message> {
        // A project that stores its own rules is edited in place (undoable,
        // saved with the `.snxprj`); the machine's rules stay untouched.
        if self.edit_project_erc_rules(|rules| {
            signex_erc::project_rules::set_severity(rules, rule, severity)
        }) {
            return Task::none();
        }
        // No rules in the project: the edit goes to the machine's rules, the
        // `User Default` profile (#1608).
        self.set_machine_severity(rule, severity)
    }

    pub(crate) fn handle_open_annotate_reset_confirm(&mut self) -> Task<Message> {
        self.ui_state.annotate_reset_confirm = true;
        self.handle_detach_modal(super::super::super::state::ModalId::AnnotateResetConfirm)
    }

    pub(crate) fn handle_close_annotate_reset_confirm(&mut self) -> Task<Message> {
        self.ui_state.annotate_reset_confirm = false;
        self.close_detached_modal(super::super::super::state::ModalId::AnnotateResetConfirm)
    }

    pub(crate) fn handle_modal_drag_start(
        &mut self,
        modal: super::super::super::state::ModalId,
        x: f32,
        y: f32,
    ) -> Task<Message> {
        self.ui_state.modal_dragging = Some((modal, x, y));
        Task::none()
    }

    pub(crate) fn handle_modal_drag_end(&mut self) -> Task<Message> {
        self.ui_state.modal_dragging = None;
        self.ui_state.tab_dragging = None;
        Task::none()
    }

    /// Pop tab `idx` into its own OS window. The tab stays in
    /// `document_state.tabs` so reattach is a pure UI flip — closing the
    /// popped-out window via `SecondaryWindowClosed` just drops the entry
    /// from `ui_state.windows` and the tab re-appears in the tab bar.
    pub(crate) fn handle_undock_tab(&mut self, idx: usize) -> Task<Message> {
        let Some(tab) = self.document_state.tabs.get(idx) else {
            return Task::none();
        };
        // A Project tab stays docked (#1202): commands typed in an undocked
        // window act on the main window's active tab, so Ctrl+Z / Ctrl+S
        // there would reach another document.
        if tab.kind.as_project().is_some() {
            // End the tab drag so the next move past the edge does not ask
            // (and log the refusal) again.
            self.ui_state.tab_dragging = None;
            crate::diagnostics::log_warning(
                "The Project tab stays in the main window; it cannot be undocked yet",
            );
            return Task::none();
        }
        let path = tab.path.clone();
        // Component Preview tabs undock to a window with
        // `WindowKind::ComponentEditor` so the editor view dispatch
        // picks it up. Schematic / PCB tabs use
        // `WindowKind::UndockedTab` as before.
        let component_editor = tab.kind.as_component_editor().cloned();

        // Don't re-undock a tab that already has a window.
        let already_undocked = match component_editor.as_ref() {
            Some(ce) => self.ui_state.windows.values().any(|k| {
                matches!(
                    k,
                    super::super::super::state::WindowKind::ComponentEditor {
                        library_path,
                        table,
                        row_id,
                    } if library_path == &ce.library_path
                        && table == &ce.table
                        && row_id == &ce.row_id
                )
            }),
            None => self.ui_state.windows.values().any(
                |k| matches!(k, super::super::super::state::WindowKind::UndockedTab { path: p, .. } if p == &path),
            ),
        };
        if already_undocked {
            return Task::none();
        }
        let title = tab.title.clone();

        // Make the tab active so the duplicated view in the new window
        // lands on that tab's content. Main window's active_tab is
        // shared — if the user wants to keep editing a different tab in
        // main, they can switch after the window opens.
        if idx != self.document_state.active_tab {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }

        let size = iced::Size::new(1400.0, 900.0);
        let (id, open_task) = iced::window::open(iced::window::Settings {
            size,
            resizable: true,
            decorations: true,
            exit_on_close_request: false,
            ..Default::default()
        });
        // Stash immediately so the first frame in the new window has a
        // target; `UndockedTabOpened` refreshes the title afterwards.
        if let Some(ce) = component_editor {
            self.ui_state.windows.insert(
                id,
                super::super::super::state::WindowKind::ComponentEditor {
                    library_path: ce.library_path.clone(),
                    table: ce.table.clone(),
                    row_id: ce.row_id,
                },
            );
            // Component Editor windows don't need the
            // `UndockedTabOpened` follow-up (no per-window canvas to
            // wire); the editor view picks the entry up directly off
            // `library.editors` via the address it already has.
            return open_task.discard();
        }
        self.ui_state.windows.insert(
            id,
            super::super::super::state::WindowKind::UndockedTab {
                path: path.clone(),
                title,
            },
        );
        open_task.map(move |settled_id| {
            Message::Window(WindowMsg::UndockedTabOpened {
                path: path.clone(),
                id: settled_id,
            })
        })
    }

    /// Remove the floating panel at `idx` and open an OS window that
    /// renders that panel's content. Closing the OS window re-docks the
    /// panel to the right column — see `SecondaryWindowClosed` in
    /// dispatch/mod.rs.
    pub(crate) fn handle_detach_floating_panel(&mut self, idx: usize) -> Task<Message> {
        let Some(fp) = self.document_state.dock.floating.get(idx) else {
            return Task::none();
        };
        let kind = fp.kind;
        let size = iced::Size::new(fp.width.max(420.0), fp.height.max(360.0));
        self.document_state.dock.floating.remove(idx);

        let (id, open_task) = iced::window::open(iced::window::Settings {
            size,
            resizable: true,
            decorations: true,
            exit_on_close_request: false,
            ..Default::default()
        });
        self.ui_state.windows.insert(
            id,
            super::super::super::state::WindowKind::DetachedPanel(kind),
        );
        open_task.map(move |settled_id| {
            Message::Window(WindowMsg::DetachedPanelOpened {
                kind,
                id: settled_id,
            })
        })
    }

    /// The message the modal's own close control sends. The OS close button
    /// of a detached modal's native title bar goes through this, so it runs
    /// the same close logic (dirty guard, state reset, window close) as the
    /// in-body close did. `None` for modals without a dedicated close
    /// message, which fall back to closing the window directly.
    pub(crate) fn detached_modal_close_message(modal: ModalId) -> Option<Message> {
        match modal {
            ModalId::AnnotateDialog => Some(Message::Annotate(AnnotateMsg::CloseDialog)),
            ModalId::AnnotateResetConfirm => {
                Some(Message::Annotate(AnnotateMsg::CloseResetConfirm))
            }
            ModalId::ReprefixConfirm => Some(Message::Annotate(AnnotateMsg::ReprefixCancelled)),
            ModalId::ErcDialog => Some(Message::Erc(ErcMsg::CloseDialog)),
            ModalId::MoveSelection => Some(Message::MoveSelection(MoveSelectionMsg::Close)),
            ModalId::NetColorPalette => Some(Message::NetColor(NetColorMsg::Close)),
            ModalId::ParameterManager => {
                Some(Message::ParameterManager(ParameterManagerMsg::Close))
            }
            ModalId::Preferences => Some(Message::Preferences(PreferencesMsg::Close)),
            ModalId::PrintPreview => Some(Message::PrintPreview(PrintPreviewMsg::Close)),
            ModalId::BomPreview => Some(Message::BomPreview(BomPreviewMsg::Close)),
            ModalId::FindReplace => Some(Message::FindReplaceMsg(
                crate::find_replace::FindReplaceMsg::Close,
            )),
            ModalId::RenameDialog => Some(Message::Rename(RenameMsg::Close)),
            ModalId::RemoveDialog => Some(Message::Remove(RemoveMsg::Close)),
            ModalId::EnableVersionControl => Some(Message::EnableVersionControl(
                EnableVersionControlMsg::Close,
            )),
            ModalId::GridProperties => Some(Message::GridProperties(GridPropertiesMsg::Close)),
            ModalId::SelectionFilterCustom => {
                Some(Message::SelectionFilter(SelectionFilterMsg::CloseCustom))
            }
            ModalId::About => Some(Message::Overlay(OverlayMsg::CloseAbout)),
            ModalId::KeyboardShortcuts => {
                Some(Message::Overlay(OverlayMsg::CloseKeyboardShortcuts))
            }
            ModalId::PassiveCalculator => {
                Some(Message::Overlay(OverlayMsg::ClosePassiveCalculator))
            }
            ModalId::FirstRunTour => Some(Message::Overlay(OverlayMsg::DismissFirstRunTour)),
            ModalId::CreateChildSheet => {
                Some(Message::Project(ProjectMsg::CreateChildSheetAnswered {
                    create: false,
                }))
            }
            ModalId::ErrorNotice => Some(Message::Overlay(OverlayMsg::DismissErrorNotice)),
            ModalId::NetlistIncompletePrompt => {
                Some(Message::Export(ExportMsg::NetlistCancelIncomplete))
            }
            // The safe choice for each: the copy is kept, nothing is saved,
            // discarded or exited by closing the window.
            ModalId::AutosaveRecovery => Some(Message::Autosave(
                crate::autosave::AutosaveMsg::Recovery(crate::autosave::RecoveryChoice::Later),
            )),
            ModalId::ProjectCloseConfirm => Some(Message::Project(ProjectMsg::CloseConfirm(
                ProjectCloseChoice::Cancel,
            ))),
            ModalId::AppQuitConfirm => Some(Message::Project(ProjectMsg::AppQuitConfirm(
                ProjectCloseChoice::Cancel,
            ))),
            // Library dialogs: Cancel, never a write or a delete. The
            // per-browser ones and the recovery prompt (whose choice depends
            // on which recovery is open) are answered by
            // `window_close_message`, which knows the window.
            ModalId::LibraryPicker => Some(Message::Library(LibraryMessage::ClosePicker)),
            ModalId::LibraryPrimitivePicker => {
                Some(Message::Library(LibraryMessage::PrimitivePicker(
                    crate::library::messages::PrimitivePickerMsg::Cancel,
                )))
            }
            ModalId::LibraryDocumentOptions => {
                Some(Message::Library(LibraryMessage::DocumentOptionsCancel))
            }
            ModalId::LibraryCreateOptions => {
                Some(Message::Library(LibraryMessage::LibraryCreateOptionsCancel))
            }
            ModalId::LibraryUpdates => Some(Message::Library(LibraryMessage::LibraryUpdatesCancel)),
            ModalId::LibraryCloseConfirm => {
                Some(Message::Library(LibraryMessage::CloseLibraryConfirm(
                    crate::library::messages::CloseLibraryChoice::Cancel,
                )))
            }
            ModalId::NetColorCustom => Some(Message::NetColor(NetColorMsg::CustomShow(false))),
            ModalId::LibraryEditRow | ModalId::LibraryDeleteConfirm | ModalId::LibraryRecovery => {
                None
            }
        }
    }

    /// Find any OS window that currently hosts `modal` and request the
    /// OS to close it. Used by the in-body Close button so pressing Close
    /// inside a detached modal both dismisses the modal state and cleans
    /// up the popped-out window — without this, the window would stay
    /// open rendering an orphaned modal body.
    pub(crate) fn close_detached_modal(
        &mut self,
        modal: super::super::super::state::ModalId,
    ) -> Task<Message> {
        use super::super::super::state::WindowKind;
        let maybe_id = self.ui_state.windows.iter().find_map(|(id, kind)| {
            if matches!(kind, WindowKind::DetachedModal(m) if *m == modal) {
                Some(*id)
            } else {
                None
            }
        });
        if let Some(id) = maybe_id {
            self.ui_state.windows.remove(&id);
            self.ui_state.modal_window_sizes.remove(&id);
            iced::window::close(id)
        } else {
            Task::none()
        }
    }

    /// Pop `modal` out of the main window into its own OS window. The
    /// window's initial size matches the modal's in-app dimensions so the
    /// user sees continuity; position falls back to default (centered on
    /// the OS) since we don't know where to anchor absent monitor query.
    pub(crate) fn handle_detach_modal(
        &mut self,
        modal: super::super::super::state::ModalId,
    ) -> Task<Message> {
        // Don't open a second window for the same modal — treat detach
        // on an already-detached modal as a no-op.
        if self.ui_state.windows.values().any(
            |kind| matches!(kind, super::super::super::state::WindowKind::DetachedModal(m) if *m == modal),
        ) {
            return Task::none();
        }

        let (id, open_task) =
            iced::window::open(modal_window_settings(modal, self.modal_window_size(modal)));
        // Stash the mapping right away — view(id) for the new window
        // fires before open_task resolves on some platforms, and without
        // the entry the detached window would render empty.
        self.ui_state.windows.insert(
            id,
            super::super::super::state::WindowKind::DetachedModal(modal),
        );
        // When the OS finishes opening the window, forward the id so the
        // update can double-check and clear any leftover drag state.
        open_task.map(move |settled_id| {
            Message::Window(WindowMsg::DetachedModalOpened {
                modal,
                id: settled_id,
            })
        })
    }

    pub(crate) fn handle_open_move_selection_dialog(&mut self) -> Task<Message> {
        self.ui_state.move_selection = super::super::super::state::MoveSelectionState {
            open: true,
            dx: "0".to_string(),
            dy: "0".to_string(),
        };
        self.handle_detach_modal(super::super::super::state::ModalId::MoveSelection)
    }

    pub(crate) fn handle_close_move_selection_dialog(&mut self) -> Task<Message> {
        self.ui_state.move_selection.open = false;
        Task::none()
    }

    pub(crate) fn handle_move_selection_apply(&mut self) -> Task<Message> {
        // Classify both axes before acting: a refused axis (`nan` / `inf` /
        // overflow) must not be read as "0 on that axis" while the other
        // axis still moves the selection. `move_offset` has already logged
        // the refusal, naming the axis; the dialog stays open so the user
        // can fix it.
        let dx = move_offset("Move Selection X", &self.ui_state.move_selection.dx);
        let dy = move_offset("Move Selection Y", &self.ui_state.move_selection.dy);
        let (Some(dx), Some(dy)) = (dx, dy) else {
            return Task::none();
        };
        if dx == 0.0 && dy == 0.0 {
            self.ui_state.move_selection.open = false;
            return Task::none();
        }
        let items = self.interaction_state.active_canvas_mut().selected.clone();
        if items.is_empty() {
            self.ui_state.move_selection.open = false;
            return Task::none();
        }
        self.ui_state.move_selection.open = false;
        // Through the mutation gateway, not `engine.execute` directly: the
        // gateway consumes the `CommandResult` this used to discard, so the
        // sheet reaches `dirty_paths` — without it the quit guard sees a
        // clean workspace and the move goes out with the process (#585) —
        // and it re-derives the project netlist and invalidates only what
        // the move touched instead of the whole canvas.
        self.apply_engine_command(
            signex_engine::Command::MoveSelection {
                items,
                dx,
                dy,
                keep_wires: self.ui_state.move_keeps_wires,
            },
            true,
            true,
        );
        Task::none()
    }

    pub(crate) fn handle_parameter_manager_edit(
        &mut self,
        symbol_uuid: uuid::Uuid,
        key: String,
        value: String,
    ) -> Task<Message> {
        // Same reasoning as `handle_move_selection_apply` (#585). The
        // gateway clears the content cache, syncs the canvas and refreshes
        // the panel context, so the hand-rolled versions of those are gone;
        // the overlay cache and the selection info were not touched before
        // and still are not.
        self.apply_engine_command(
            signex_engine::Command::SetSymbolField {
                symbol_id: symbol_uuid,
                key,
                value,
            },
            false,
            false,
        );
        Task::none()
    }
}

/// A Move Selection offset typed in the dialog. `None` means refused
/// (`nan` / `inf` / overflow: an `inf` offset would send every selected item
/// to infinity) and the whole apply is abandoned. Text that is not a number
/// at all (empty, `-`) is the user mid-edit and moves by nothing. `field`
/// names the box in the refusal line.
fn move_offset(field: &str, text: &str) -> Option<f64> {
    use crate::numeric_field::{FieldNumber, parse_named_field_number};
    match parse_named_field_number(field, text) {
        FieldNumber::Number(v) => Some(v),
        FieldNumber::NotANumber => Some(0.0),
        FieldNumber::Refused => None,
    }
}

/// The initial window size of `modal`: its in-window dimensions, so the user
/// sees continuity.
pub(crate) fn modal_size(modal: ModalId) -> iced::Size {
    match modal {
        ModalId::AnnotateDialog => iced::Size::new(1000.0, 680.0),
        ModalId::ErcDialog => iced::Size::new(1000.0, 600.0),
        ModalId::AnnotateResetConfirm => iced::Size::new(440.0, 200.0),
        ModalId::ReprefixConfirm => iced::Size::new(560.0, 520.0),
        ModalId::MoveSelection => iced::Size::new(420.0, 240.0),
        ModalId::NetColorPalette => iced::Size::new(520.0, 480.0),
        ModalId::ParameterManager => iced::Size::new(900.0, 560.0),
        ModalId::Preferences => iced::Size::new(900.0, 620.0),
        ModalId::FindReplace => iced::Size::new(440.0, 320.0),
        ModalId::RenameDialog => iced::Size::new(420.0, 160.0),
        ModalId::RemoveDialog => iced::Size::new(520.0, 240.0),
        ModalId::PrintPreview => iced::Size::new(1000.0, 700.0),
        ModalId::BomPreview => iced::Size::new(1080.0, 700.0),
        ModalId::EnableVersionControl => iced::Size::new(560.0, 480.0),
        ModalId::GridProperties => iced::Size::new(480.0, 290.0),
        ModalId::SelectionFilterCustom => iced::Size::new(440.0, 330.0),
        ModalId::About => iced::Size::new(400.0, 200.0),
        ModalId::KeyboardShortcuts => iced::Size::new(520.0, 600.0),
        ModalId::PassiveCalculator => iced::Size::new(640.0, 480.0),
        ModalId::FirstRunTour => iced::Size::new(360.0, 170.0),
        ModalId::CreateChildSheet => iced::Size::new(560.0, 215.0),
        ModalId::ErrorNotice => iced::Size::new(520.0, 260.0),
        ModalId::NetlistIncompletePrompt => iced::Size::new(580.0, 360.0),
        ModalId::AutosaveRecovery => iced::Size::new(540.0, 200.0),
        ModalId::ProjectCloseConfirm => iced::Size::new(540.0, 310.0),
        ModalId::AppQuitConfirm => iced::Size::new(540.0, 310.0),
        ModalId::LibraryPicker => iced::Size::new(560.0, 540.0),
        ModalId::LibraryEditRow => iced::Size::new(640.0, 600.0),
        ModalId::LibraryPrimitivePicker => iced::Size::new(760.0, 600.0),
        ModalId::LibraryDocumentOptions => iced::Size::new(580.0, 360.0),
        ModalId::LibraryCreateOptions => iced::Size::new(520.0, 310.0),
        ModalId::LibraryUpdates => iced::Size::new(680.0, 460.0),
        ModalId::LibraryCloseConfirm => iced::Size::new(560.0, 410.0),
        ModalId::LibraryDeleteConfirm => iced::Size::new(520.0, 240.0),
        ModalId::LibraryRecovery => iced::Size::new(600.0, 280.0),
        ModalId::NetColorCustom => iced::Size::new(600.0, 480.0),
    }
}

/// Dialogs that are fixed-size: the confirm and warning class, and the small
/// dialogs. They are not resizable (which disables the maximise / zoom button
/// in winit) and cannot be minimised. The large tool windows (Preferences, ERC,
/// BOM, Print Preview, Annotate, Parameter Manager, Find / Replace, shortcuts,
/// calculator, edit row) keep the normal window behaviour.
pub(crate) fn modal_is_fixed(modal: ModalId) -> bool {
    match modal {
        ModalId::AppQuitConfirm
        | ModalId::ProjectCloseConfirm
        | ModalId::ErrorNotice
        | ModalId::NetlistIncompletePrompt
        | ModalId::AutosaveRecovery
        | ModalId::LibraryRecovery
        | ModalId::RenameDialog
        | ModalId::RemoveDialog
        | ModalId::AnnotateResetConfirm
        | ModalId::ReprefixConfirm
        | ModalId::CreateChildSheet
        | ModalId::LibraryCloseConfirm
        | ModalId::LibraryDeleteConfirm
        | ModalId::About
        | ModalId::GridProperties
        | ModalId::SelectionFilterCustom
        | ModalId::EnableVersionControl
        | ModalId::NetColorCustom
        | ModalId::LibraryPicker
        | ModalId::LibraryPrimitivePicker
        | ModalId::LibraryDocumentOptions
        | ModalId::LibraryCreateOptions => true,
        ModalId::AnnotateDialog
        | ModalId::ErcDialog
        | ModalId::MoveSelection
        | ModalId::NetColorPalette
        | ModalId::ParameterManager
        | ModalId::Preferences
        | ModalId::FindReplace
        | ModalId::PrintPreview
        | ModalId::BomPreview
        | ModalId::KeyboardShortcuts
        | ModalId::PassiveCalculator
        | ModalId::FirstRunTour
        | ModalId::LibraryEditRow
        | ModalId::LibraryUpdates => false,
    }
}

/// The settings `modal`'s window opens with: its size, and fixed (not
/// resizable, not minimisable) when [`modal_is_fixed`].
pub(crate) fn modal_window_settings(modal: ModalId, size: iced::Size) -> iced::window::Settings {
    let fixed = modal_is_fixed(modal);
    iced::window::Settings {
        resizable: !fixed,
        minimizable: !fixed,
        ..detached_modal_settings(size)
    }
}

/// Window settings for a detached modal.
///
/// The level is `Normal` and the window keeps the OS decorations. A
/// detached modal is therefore a first-class window: it stacks like any
/// other window (never floats above other applications and never covers
/// the in-window modals painted in the main window), shows in the
/// taskbar / Dock, can be moved and resized from its native title bar, and
/// can be raised again after the main window covers it.
///
/// iced 0.14.0 offers no owner/parent relationship: `window::Settings` has
/// no `parent_window`, and `iced_winit` never calls winit's
/// `with_parent_window` / `with_transient_for`. "Stay above my owner only"
/// is therefore not requestable, which is why the window needs to be
/// recoverable on its own. The window title is not a `Settings` field in
/// iced; it comes from `Signex::title`, which uses `view::detached_title::detached_modal_title`.
pub(crate) fn detached_modal_settings(size: iced::Size) -> iced::window::Settings {
    iced::window::Settings {
        size,
        resizable: true,
        decorations: true,
        // The close request must reach `WindowCloseRequested`, which runs
        // the modal's own close message (the Preferences dirty guard).
        exit_on_close_request: false,
        level: iced::window::Level::Normal,
        // iced 0.14 can centre on the screen, not on the owner window (the
        // main window's position is not tracked); see docs/DETACHED_MODALS.md.
        position: iced::window::Position::Centered,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{detached_modal_settings, move_offset};

    /// #828 -- `inf` / `nan` / an overflow must not become a move offset.
    #[test]
    fn move_offset_refuses_non_finite_text() {
        for bad in ["nan", "NaN", "inf", "-inf", "1e999", "-1e999"] {
            assert_eq!(move_offset("Move Selection X", bad), None, "{bad:?}");
        }
    }

    #[test]
    fn move_offset_reads_ordinary_text() {
        assert_eq!(move_offset("Move Selection X", " 2.5 "), Some(2.5));
        assert_eq!(move_offset("Move Selection X", "-1"), Some(-1.0));
        assert_eq!(move_offset("Move Selection X", ""), Some(0.0));
    }

    #[test]
    fn detached_modal_is_not_always_on_top() {
        let s = detached_modal_settings(iced::Size::new(420.0, 180.0));
        assert_eq!(s.level, iced::window::Level::Normal);
    }

    #[test]
    fn confirms_and_small_dialogs_are_fixed_and_tool_windows_are_not() {
        use crate::app::state::ModalId;
        for fixed in [
            ModalId::AppQuitConfirm,
            ModalId::ErrorNotice,
            ModalId::CreateChildSheet,
            ModalId::About,
            ModalId::LibraryDeleteConfirm,
        ] {
            let s = super::modal_window_settings(fixed, super::modal_size(fixed));
            assert!(!s.resizable && !s.minimizable, "{fixed:?}");
        }
        for tool in [
            ModalId::Preferences,
            ModalId::ErcDialog,
            ModalId::BomPreview,
            ModalId::LibraryEditRow,
        ] {
            let s = super::modal_window_settings(tool, super::modal_size(tool));
            assert!(s.resizable && s.minimizable, "{tool:?}");
        }
    }

    #[test]
    fn detached_modal_keeps_requested_size_and_native_decorations() {
        let size = iced::Size::new(900.0, 620.0);
        let s = detached_modal_settings(size);
        assert_eq!(s.size, size);
        assert!(s.resizable);
        assert!(s.decorations);
        assert!(!s.exit_on_close_request);
    }
}
