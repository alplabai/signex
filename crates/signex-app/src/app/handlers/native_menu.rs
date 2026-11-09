//! The macOS system menu bar's touch points with the Model: the menu it is
//! built from, choosing a row, and a failed build.
//! Design: docs/MENU_BAR.md, "Native macOS menu".

use crate::app::state::Signex;
use crate::app::{Message, UiMsg};
use crate::menu_bar::MenuMessage;
use iced::Task;

impl Signex {
    /// The window the system menu bar acts on: the focused one, else main.
    pub(crate) fn native_menu_window(&self) -> Option<iced::window::Id> {
        self.ui_state
            .focused_window
            .or(self.ui_state.main_window_id)
    }

    /// What the native menu is built from, read off the Model.
    pub(crate) fn native_menu_inputs(&self) -> crate::menu_bar::native::MenuInputs {
        use std::sync::Arc;

        crate::menu_bar::native::MenuInputs::new(
            self.command_context(self.native_menu_window()),
            self.ui_state.recent_projects.entries(),
            self.has_locking_confirm_modal(),
            self.open_panels(),
            Arc::clone(&self.ui_state.i18n),
        )
        .with_keymap(Arc::clone(&self.ui_state.active_keymap))
    }

    /// The menu bar as the Model stands now. The iced runtime calls it after
    /// every update on macOS and syncs the system menu to it; it never calls
    /// it elsewhere. The menu is rebuilt only when an input it reads changed.
    pub fn native_menu(&self) -> iced::menu::Menu<Message> {
        self.native_menu.menu(self.native_menu_inputs())
    }

    /// The message a native row sends, routed like the in-window row of the
    /// same window: rows that act on a document (Undo, Redo, Save, Save As,
    /// Print, Export ▸ PDF, Close Document, the tab stepper) go to a non-main
    /// window's own document.
    fn native_menu_message(&self, action: MenuMessage) -> Message {
        match (&action, self.native_menu_window()) {
            (row, Some(window))
                if row.window_command().is_some()
                    && self.ui_state.main_window_id != Some(window) =>
            {
                Message::Ui(UiMsg::MenuInWindow {
                    window,
                    msg: action,
                })
            }
            _ => Message::Menu(action),
        }
    }

    /// A model row of the native bar was chosen. While a confirm dialog is
    /// open it does nothing: the bar greys these rows, and this refuses one
    /// that slipped through before the bar was rebuilt. (The app menu's rows
    /// send their message directly and stay live.)
    pub(crate) fn handle_native_menu_row_chosen(&mut self, action: MenuMessage) -> Task<Message> {
        if self.has_locking_confirm_modal() {
            crate::diagnostics::log_info(format!(
                "Main menu: ignored {action:?} while a confirmation dialog is open"
            ));
            return Task::none();
        }
        let message = self.native_menu_message(action);
        self.dispatch_update(message)
    }

    /// The system menu bar could not be built: the in-window menu row takes
    /// over, and the reason reaches the Messages panel.
    pub(crate) fn handle_native_menu_failed(&mut self, reason: &str) -> Task<Message> {
        self.native_menu.fall_back(reason);
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppQuitConfirmState;

    fn open_quit_confirm(app: &mut Signex, dirty: &[&str]) {
        app.ui_state.app_quit_confirm = Some(AppQuitConfirmState {
            dirty_paths: dirty.iter().map(Into::into).collect(),
            dirty_rows: Vec::new(),
        });
    }

    fn shortcuts_row() -> MenuMessage {
        MenuMessage::command("show_current_command_hotkeys")
    }

    #[test]
    fn a_chosen_row_dispatches_its_message() {
        let (mut app, _task) = Signex::new();
        assert!(!app.ui_state.keyboard_shortcuts_open);
        let _ = app.handle_native_menu_row_chosen(shortcuts_row());
        assert!(app.ui_state.keyboard_shortcuts_open);
    }

    #[test]
    fn a_confirm_dialog_refuses_model_rows_even_from_a_stale_menu() {
        // The menu still says the row is live: the backstop must refuse.
        let (mut app, _task) = Signex::new();
        open_quit_confirm(&mut app, &["/p/a.snxsch"]);
        let _ = app.handle_native_menu_row_chosen(shortcuts_row());
        assert!(!app.ui_state.keyboard_shortcuts_open);
        let confirm = app.ui_state.app_quit_confirm.as_ref().expect("still open");
        assert_eq!(confirm.dirty_paths.len(), 1, "the snapshot is untouched");
    }

    #[test]
    fn tool_windows_do_not_lock_the_bar() {
        let (mut app, _task) = Signex::new();
        app.ui_state.preferences_open = true;
        app.ui_state.erc_dialog_open = true;
        app.ui_state.about_open = true;
        app.ui_state.passive_calculator_open = true;
        assert!(!app.has_locking_confirm_modal());
        open_quit_confirm(&mut app, &[]);
        assert!(app.has_locking_confirm_modal());
    }

    #[test]
    fn a_detached_annotate_reset_confirm_still_locks_the_bar() {
        use crate::app::state::{ModalId, WindowKind};
        let (mut app, _task) = Signex::new();
        app.ui_state.annotate_reset_confirm = true;
        app.ui_state.windows.insert(
            iced::window::Id::unique(),
            WindowKind::DetachedModal(ModalId::AnnotateResetConfirm),
        );
        assert!(app.has_locking_confirm_modal());
    }

    #[test]
    fn undo_goes_to_the_focused_non_main_window() {
        let (mut app, _task) = Signex::new();
        let main = iced::window::Id::unique();
        let other = iced::window::Id::unique();
        app.ui_state.main_window_id = Some(main);
        app.ui_state.focused_window = Some(other);
        assert!(matches!(
            app.native_menu_message(MenuMessage::command("undo")),
            Message::Ui(UiMsg::MenuInWindow { window, .. }) if window == other
        ));
        assert!(matches!(
            app.native_menu_message(MenuMessage::command("save_document")),
            Message::Ui(UiMsg::MenuInWindow { window, .. }) if window == other
        ));
        assert!(matches!(
            app.native_menu_message(MenuMessage::OpenAbout),
            Message::Menu(MenuMessage::OpenAbout)
        ));
        app.ui_state.focused_window = Some(main);
        assert!(matches!(
            app.native_menu_message(MenuMessage::command("undo")),
            Message::Menu(MenuMessage::Command(id)) if id.as_str() == "undo"
        ));
        app.ui_state.focused_window = None;
        assert_eq!(app.native_menu_window(), Some(main));
    }

    /// The runtime asks for the menu after every update batch (every canvas
    /// mouse move): only a changed input may rebuild it.
    #[test]
    fn the_native_menu_is_rebuilt_only_when_an_input_changed() {
        let (mut app, _task) = Signex::new();
        let _ = app.native_menu();
        let built = app.native_menu.builds();

        let _ = app.native_menu();
        let _ = app.native_menu();
        assert_eq!(app.native_menu.builds(), built, "same inputs rebuilt");

        open_quit_confirm(&mut app, &["/p/a.snxsch"]);
        let _ = app.native_menu();
        assert_eq!(app.native_menu.builds(), built + 1, "the lock was missed");

        let _ = app.update(Message::Ui(UiMsg::LanguageChanged(
            signex_i18n::LanguagePref::Locale("en-XA".to_owned()),
        )));
        let _ = app.native_menu();
        assert_eq!(
            app.native_menu.builds(),
            built + 2,
            "the language was missed"
        );
        let _ = app.native_menu();
        assert_eq!(app.native_menu.builds(), built + 2);
    }

    #[test]
    fn a_failed_system_menu_brings_the_in_window_row_back_and_is_logged() {
        let (mut app, _task) = Signex::new();
        let needle = "could not build the system menu bar (boom)";
        let logged_before = crate::diagnostics::logged_count(needle);
        let _ = app.update(Message::Window(crate::app::WindowMsg::NativeMenuFailed(
            "boom".into(),
        )));
        assert!(app.native_menu.has_failed());
        assert!(!app.native_menu.hides_in_window_menu());
        assert!(
            crate::diagnostics::logged_count(needle) > logged_before,
            "the failure did not reach the log"
        );
    }
}
