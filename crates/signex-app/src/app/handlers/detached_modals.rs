//! Keeping each tool or confirm dialog in step with its own OS window.
//! Design and the lock policy: docs/DETACHED_MODALS.md.

use iced::Task;

use crate::app::Message;
use crate::app::state::{ModalId, Signex, WindowKind};

/// Dialogs whose window follows their open state, whatever code flips it.
const FOLLOWED: [ModalId; 24] = [
    ModalId::FindReplace,
    ModalId::RenameDialog,
    ModalId::RemoveDialog,
    ModalId::EnableVersionControl,
    ModalId::GridProperties,
    ModalId::SelectionFilterCustom,
    ModalId::About,
    ModalId::KeyboardShortcuts,
    ModalId::PassiveCalculator,
    ModalId::CreateChildSheet,
    ModalId::ErrorNotice,
    ModalId::NetlistIncompletePrompt,
    ModalId::AutosaveRecovery,
    ModalId::ProjectCloseConfirm,
    ModalId::AppQuitConfirm,
    ModalId::LibraryPicker,
    ModalId::LibraryPrimitivePicker,
    ModalId::LibraryDocumentOptions,
    ModalId::LibraryCreateOptions,
    ModalId::LibraryStockCopy,
    ModalId::LibraryUpdates,
    ModalId::LibraryCloseConfirm,
    ModalId::LibraryRecovery,
    ModalId::NetColorCustom,
];

/// Confirm-class dialogs among the detached ones: while one is open the
/// main window takes no input.
const LOCKING: [ModalId; 13] = [
    ModalId::RenameDialog,
    ModalId::RemoveDialog,
    ModalId::AnnotateResetConfirm,
    ModalId::ReprefixConfirm,
    ModalId::CreateChildSheet,
    ModalId::ErrorNotice,
    ModalId::NetlistIncompletePrompt,
    ModalId::AutosaveRecovery,
    ModalId::ProjectCloseConfirm,
    ModalId::AppQuitConfirm,
    ModalId::LibraryCloseConfirm,
    ModalId::LibraryDeleteConfirm,
    ModalId::LibraryRecovery,
];

/// Take keyboard focus away from whatever text input holds it, in every window.
fn unfocus_text_inputs() -> Task<Message> {
    iced_runtime::task::effect(iced_runtime::Action::widget(
        iced::advanced::widget::operation::focusable::unfocus::<()>(),
    ))
}

impl Signex {
    /// Whether a followed dialog's state says it is open; `None` for a
    /// dialog that manages its own window.
    fn followed_modal_open(&self, modal: ModalId) -> Option<bool> {
        let ui = &self.ui_state;
        Some(match modal {
            ModalId::FindReplace => ui.find_replace.open,
            ModalId::RenameDialog => ui.rename_dialog.is_some(),
            ModalId::RemoveDialog => ui.remove_dialog.is_some(),
            ModalId::EnableVersionControl => ui.enable_version_control.is_some(),
            ModalId::GridProperties => ui.grid_properties.is_some(),
            ModalId::SelectionFilterCustom => ui.selection_filter_custom.is_some(),
            ModalId::About => ui.about_open,
            ModalId::KeyboardShortcuts => ui.keyboard_shortcuts_open,
            ModalId::PassiveCalculator => ui.passive_calculator_open,
            ModalId::FirstRunTour => ui.first_run_tour_open,
            ModalId::CreateChildSheet => ui.create_child_sheet_confirm.is_some(),
            ModalId::ErrorNotice => self.document_state.error_notice.is_some(),
            ModalId::NetlistIncompletePrompt => {
                self.document_state.netlist_incomplete_prompt.is_some()
            }
            ModalId::AutosaveRecovery => self.document_state.autosave.current_offer().is_some(),
            ModalId::ProjectCloseConfirm => ui.project_close_confirm.is_some(),
            ModalId::AppQuitConfirm => ui.app_quit_confirm.is_some(),
            ModalId::LibraryPicker => self.library.picker.is_some(),
            ModalId::LibraryPrimitivePicker => self.library.primitive_picker.is_some(),
            ModalId::LibraryDocumentOptions => self.library.document_options.is_some(),
            ModalId::LibraryCreateOptions => self.library.create_options.is_some(),
            ModalId::LibraryStockCopy => self.library.stock_copy.is_some(),
            ModalId::LibraryUpdates => self.library.library_updates.is_some(),
            ModalId::LibraryCloseConfirm => self.library.close_library_confirm.is_some(),
            ModalId::LibraryRecovery => self.library.recovery.is_some(),
            ModalId::NetColorCustom => ui.net_color_custom.show,
            // One window per browser, followed by `sync_browser_modals`.
            ModalId::LibraryEditRow | ModalId::LibraryDeleteConfirm => return None,
            ModalId::AnnotateDialog
            | ModalId::AnnotateResetConfirm
            | ModalId::ReprefixConfirm
            | ModalId::ErcDialog
            | ModalId::MoveSelection
            | ModalId::NetColorPalette
            | ModalId::ParameterManager
            | ModalId::Preferences
            | ModalId::PrintPreview
            | ModalId::BomPreview => return None,
        })
    }

    /// Open the window of every followed dialog that has just opened and
    /// close the window of every one that has just closed. Runs after each
    /// `update`, so the many places that flip these flags need no wiring.
    pub(crate) fn sync_detached_modals(&mut self) -> Option<Task<Message>> {
        if !self.ui_state.main_window_opened {
            return None;
        }
        let mut tasks = Vec::new();
        for modal in FOLLOWED {
            let Some(open) = self.followed_modal_open(modal) else {
                continue;
            };
            let recovery = matches!(modal, ModalId::AutosaveRecovery | ModalId::LibraryRecovery);
            if recovery
                && !open
                && self.followed_modal_open(ModalId::AutosaveRecovery) != Some(true)
                && self.followed_modal_open(ModalId::LibraryRecovery) != Some(true)
            {
                self.ui_state.recovery_window_vanished = 0;
            }
            // A recovery prompt whose window keeps vanishing stays in the window.
            if recovery && open && self.ui_state.recovery_window_vanished >= 2 {
                continue;
            }
            match (open, self.modal_detached(modal)) {
                (true, false) => tasks.push(self.handle_detach_modal(modal)),
                (false, true) => tasks.push(self.close_detached_modal(modal)),
                (true, true) | (false, false) => {}
            }
        }
        tasks.extend(self.sync_browser_modals());
        tasks.extend(self.resize_content_sized_modals());
        // A text field that had keyboard focus keeps taking typing under the
        // lock, so release it once when a confirm dialog locks the windows.
        let locked = self.locking_confirm_window().is_some();
        if locked && !self.ui_state.lock_unfocused {
            self.close_transient_ui_for_lock();
            // First in the batch: a dialog's own autofocus, started by its
            // window opening, must not be undone by this.
            tasks.insert(0, unfocus_text_inputs());
        }
        self.ui_state.lock_unfocused = locked;
        (!tasks.is_empty()).then(|| Task::batch(tasks))
    }

    /// Open and close the windows of the per-browser dialogs (edit row, delete
    /// confirm): one window per browser that has one open, bound to that
    /// browser, closed when the browser's dialog (or the browser) is gone.
    fn sync_browser_modals(&mut self) -> Vec<Task<Message>> {
        let mut tasks = Vec::new();
        for modal in [ModalId::LibraryEditRow, ModalId::LibraryDeleteConfirm] {
            let mut open: Vec<std::path::PathBuf> = self
                .library
                .library_browsers
                .iter()
                .filter(|(_, b)| match modal {
                    ModalId::LibraryEditRow => b.edit_modal.is_some(),
                    _ => b.delete_confirm.is_some(),
                })
                .map(|(path, _)| path.clone())
                .collect();
            open.sort();
            if modal == ModalId::LibraryEditRow {
                // A browser whose edit form is closed starts counting afresh.
                self.ui_state
                    .edit_row_window_vanished
                    .retain(|path, _| open.contains(path));
            }
            for path in &open {
                let stays_in_window = modal == ModalId::LibraryEditRow
                    && self
                        .ui_state
                        .edit_row_window_vanished
                        .get(path)
                        .is_some_and(|n| *n >= 2);
                if !stays_in_window && self.browser_modal_window(modal, path).is_none() {
                    tasks.push(self.detach_browser_modal(modal, path.clone()));
                }
            }
            let stale: Vec<iced::window::Id> = self
                .ui_state
                .windows
                .iter()
                .filter_map(|(id, kind)| match kind {
                    WindowKind::DetachedModal(m) if *m == modal => Some(*id),
                    _ => None,
                })
                .filter(|id| {
                    self.ui_state
                        .library_modal_owners
                        .get(id)
                        .is_none_or(|owner| !open.contains(owner))
                })
                .collect();
            for id in stale {
                self.ui_state.library_modal_owners.remove(&id);
                self.ui_state.modal_window_sizes.remove(&id);
                self.ui_state.windows.remove(&id);
                tasks.push(iced::window::close(id));
            }
        }
        tasks
    }

    /// The window showing `modal` for the browser at `path`.
    pub(crate) fn browser_modal_window(
        &self,
        modal: ModalId,
        path: &std::path::Path,
    ) -> Option<iced::window::Id> {
        self.ui_state.windows.iter().find_map(|(id, kind)| {
            (matches!(kind, WindowKind::DetachedModal(m) if *m == modal)
                && self
                    .ui_state
                    .library_modal_owners
                    .get(id)
                    .map(|p| p.as_path())
                    == Some(path))
            .then_some(*id)
        })
    }

    fn detach_browser_modal(&mut self, modal: ModalId, path: std::path::PathBuf) -> Task<Message> {
        let size = self.modal_window_size(modal);
        let (id, open_task) = iced::window::open(crate::app::handlers::erc::modal_window_settings(
            modal, size,
        ));
        self.ui_state
            .modal_window_sizes
            .insert(id, (size.width, size.height));
        self.ui_state
            .windows
            .insert(id, WindowKind::DetachedModal(modal));
        self.ui_state.library_modal_owners.insert(id, path);
        open_task.map(move |settled_id| {
            Message::Window(crate::app::WindowMsg::DetachedModalOpened {
                modal,
                id: settled_id,
            })
        })
    }

    /// What closing `window` (the OS close button, or Esc typed in it) sends:
    /// the dialog's own cancel, addressed to the browser or recovery it shows.
    pub(crate) fn window_close_message(
        &self,
        window: iced::window::Id,
        modal: ModalId,
    ) -> Option<Message> {
        use crate::library::messages::{BrowserEditMsg, LibraryMessage};
        let owner = self.ui_state.library_modal_owners.get(&window).cloned();
        match (modal, owner) {
            (ModalId::LibraryEditRow, Some(library_path)) => {
                Some(Message::Library(LibraryMessage::BrowserEdit {
                    library_path,
                    msg: BrowserEditMsg::Cancel,
                }))
            }
            (ModalId::LibraryDeleteConfirm, Some(library_path)) => {
                Some(Message::Library(LibraryMessage::BrowserDeleteRowCancel {
                    library_path,
                }))
            }
            (ModalId::LibraryRecovery, _) => self.recovery_cancel_message(),
            (modal, _) => Self::detached_modal_close_message(modal),
        }
    }

    /// The recovery dialog's Cancel / Decide-later choice, whichever kind is open.
    fn recovery_cancel_message(&self) -> Option<Message> {
        use crate::library::messages::LibraryMessage as L;
        use crate::library::recovery::{
            BrokenBindingChoice, GitMissingChoice, LibraryMissingChoice, RecoveryDialog,
        };
        Some(Message::Library(match self.library.recovery.as_ref()? {
            RecoveryDialog::LibraryMissing { .. } => {
                L::RecoveryLibraryMissing(LibraryMissingChoice::Cancel)
            }
            RecoveryDialog::GitMissing { .. } => L::RecoveryGitMissing(GitMissingChoice::Cancel),
            RecoveryDialog::BrokenPrimitiveBinding { .. } => {
                L::RecoveryBrokenBinding(BrokenBindingChoice::Cancel)
            }
        }))
    }

    /// The window `modal` lives in, if it is detached.
    pub(crate) fn detached_window_of(&self, modal: ModalId) -> Option<iced::window::Id> {
        self.ui_state.windows.iter().find_map(|(id, kind)| {
            matches!(kind, WindowKind::DetachedModal(m) if *m == modal).then_some(*id)
        })
    }

    /// A recovery prompt's window went away without an answer. The prompt is
    /// never decided for the user: it comes back, but only twice.
    pub(crate) fn note_recovery_window_vanished(&mut self) {
        self.ui_state.recovery_window_vanished += 1;
        if self.ui_state.recovery_window_vanished == 2 {
            crate::diagnostics::log_warning(
                "A recovery prompt's window closed twice without an answer; \
                 it now stays in the main window",
            );
        }
    }

    /// A lock starting closes what could still be acted on through it: the
    /// command palette, context menus, the Active Bar menu and an in-place
    /// text edit.
    fn close_transient_ui_for_lock(&mut self) {
        self.ui_state.command_palette.open = false;
        self.ui_state.panel_list_open = false;
        let interaction = &mut self.interaction_state;
        interaction.context_menu = None;
        interaction.tab_context_menu = None;
        interaction.project_tree_context_menu = None;
        interaction.active_bar_menu = None;
        interaction.grid_picker = None;
        interaction.editing_text = None;
    }

    /// The window of the open confirm dialog that locks the main window, if
    /// it lives in its own window.
    pub(crate) fn locking_confirm_window(&self) -> Option<iced::window::Id> {
        // Deterministic: the dialog that paints topmost, the one Esc acts on
        // (ties, e.g. two browsers' delete confirms, by window id text).
        let rank = |modal: ModalId| {
            crate::app::bootstrap::modal_overlay_id(modal).and_then(|overlay| {
                crate::app::view::overlay_id::PAINT_ORDER
                    .iter()
                    .position(|o| *o == overlay)
            })
        };
        self.ui_state
            .windows
            .iter()
            .filter_map(|(id, kind)| match kind {
                WindowKind::DetachedModal(modal) if LOCKING.contains(modal) => {
                    Some((rank(*modal), format!("{id:?}"), *id))
                }
                _ => None,
            })
            .max_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)))
            .map(|(_, _, id)| id)
    }
}

#[cfg(test)]
#[path = "detached_modals_tests.rs"]
mod tests;
