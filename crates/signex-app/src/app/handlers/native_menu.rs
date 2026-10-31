//! The macOS system menu bar's two touch points with the Model: choosing a
//! row, and keeping the bar current after every `update`.
//! Design: docs/MENU_BAR.md, "Native macOS menu".

use crate::app::state::Signex;
use crate::app::{Message, UiMsg};
use crate::menu_bar::MenuMessage;
use crate::menu_bar::native::is_app_menu_id;
use iced::Task;

impl Signex {
    /// The window the system menu bar acts on: the focused one, else main.
    pub(crate) fn native_menu_window(&self) -> Option<iced::window::Id> {
        self.ui_state
            .focused_window
            .or(self.ui_state.main_window_id)
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

    /// A native row was chosen. The id is looked up in the map of the bar
    /// as it stands now, so an id that is stale or whose row is disabled
    /// does nothing, even if the event was queued before a rebuild. While a
    /// confirm dialog is open only the app menu acts: the bar greys the
    /// model rows, and this refuses one that slipped through.
    pub(crate) fn handle_native_menu_activated(&mut self, id: &str) -> Task<Message> {
        if !is_app_menu_id(id) && self.has_locking_confirm_modal() {
            crate::diagnostics::log_info(format!(
                "Main menu: ignored \"{id}\" while a confirmation dialog is open"
            ));
            return Task::none();
        }
        match self.native_menu.actions.get(id).cloned() {
            Some(action) => {
                if action.is_panel_row() {
                    self.native_menu.invalidate();
                }
                let message = self.native_menu_message(action);
                self.dispatch_update(message)
            }
            None => {
                crate::diagnostics::log_info(format!(
                    "Main menu: ignored \"{id}\", which is disabled or no longer in the menu"
                ));
                Task::none()
            }
        }
    }

    /// Rebuild the native spec and apply it, only when what it reads
    /// (enablement facts of the focused window, recent projects, the
    /// confirm-dialog lock, the keymap) moved since the last sync.
    #[cfg(target_os = "macos")]
    pub(crate) fn sync_native_menu(&mut self) {
        use crate::menu_bar::MenuContext;
        use crate::menu_bar::model::menu_bar_model;
        use crate::menu_bar::native::{MenuInputs, build};
        use std::sync::Arc;

        let inputs = MenuInputs::new(
            self.command_context(self.native_menu_window()),
            self.ui_state.recent_projects.entries(),
            self.has_locking_confirm_modal(),
            self.open_panels(),
        )
        .with_keymap(Arc::clone(&self.ui_state.active_keymap));
        if !self.native_menu.needs_sync(&inputs) {
            return;
        }
        let ctx = MenuContext {
            commands: inputs.commands(),
            recent_projects: inputs.recent(),
            open_panels: inputs.open_panels().to_vec(),
            active_keymap: inputs.keymap(),
            ..MenuContext::default()
        };
        let built = build(&menu_bar_model(&ctx), inputs.locked());
        self.native_menu.sync(inputs, built);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppQuitConfirmState;
    use crate::menu_bar::MenuContext;
    use crate::menu_bar::model::menu_bar_model;
    use crate::menu_bar::native::build;

    /// An app whose native map is seeded as the macOS sync would, so the
    /// activation path runs on every OS.
    fn seeded(locked: bool) -> Signex {
        let (mut app, _task) = Signex::new();
        app.native_menu.actions = build(&menu_bar_model(&MenuContext::default()), locked).actions;
        app
    }

    fn open_quit_confirm(app: &mut Signex, dirty: &[&str]) {
        app.ui_state.app_quit_confirm = Some(AppQuitConfirmState {
            dirty_paths: dirty.iter().map(Into::into).collect(),
            dirty_rows: Vec::new(),
        });
    }

    #[test]
    fn choosing_a_panel_row_drops_the_applied_menu_so_marks_redraw() {
        let mut app = seeded(false);
        app.native_menu.seed_applied_for_test();
        assert!(app.native_menu.is_applied_for_test());

        let _ = app.handle_native_menu_activated("View/Properties");

        assert!(!app.native_menu.is_applied_for_test());
    }

    const SHORTCUTS: &str = "Help/Keyboard Shortcuts";

    #[test]
    fn a_live_id_dispatches_its_message() {
        let mut app = seeded(false);
        assert!(!app.ui_state.keyboard_shortcuts_open);
        let _ = app.handle_native_menu_activated(SHORTCUTS);
        assert!(app.ui_state.keyboard_shortcuts_open);
    }

    #[test]
    fn a_missing_id_does_nothing() {
        let mut app = seeded(false);
        let _ = app.handle_native_menu_activated("Help/No Such Row");
        assert!(!app.ui_state.keyboard_shortcuts_open);
    }

    #[test]
    fn a_confirm_dialog_refuses_model_rows_even_from_a_stale_map() {
        // The map still says the row is live: the backstop must refuse.
        let mut app = seeded(false);
        open_quit_confirm(&mut app, &["/p/a.snxsch"]);
        let _ = app.handle_native_menu_activated(SHORTCUTS);
        assert!(!app.ui_state.keyboard_shortcuts_open);
        let confirm = app.ui_state.app_quit_confirm.as_ref().expect("still open");
        assert_eq!(confirm.dirty_paths.len(), 1, "the snapshot is untouched");
    }

    #[test]
    fn a_confirm_dialog_keeps_the_app_menu_live() {
        let mut app = seeded(true);
        open_quit_confirm(&mut app, &["/p/a.snxsch"]);
        assert!(app.native_menu.actions.contains_key("Signex/About Signex"));
        let _ = app.handle_native_menu_activated("Signex/About Signex");
        assert!(app.ui_state.about_open);
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
            app.native_menu_message(MenuMessage::Undo),
            Message::Ui(UiMsg::MenuInWindow { window, .. }) if window == other
        ));
        assert!(matches!(
            app.native_menu_message(MenuMessage::Save),
            Message::Ui(UiMsg::MenuInWindow { window, .. }) if window == other
        ));
        assert!(matches!(
            app.native_menu_message(MenuMessage::OpenAbout),
            Message::Menu(MenuMessage::OpenAbout)
        ));
        app.ui_state.focused_window = Some(main);
        assert!(matches!(
            app.native_menu_message(MenuMessage::Undo),
            Message::Menu(MenuMessage::Undo)
        ));
        app.ui_state.focused_window = None;
        assert_eq!(app.native_menu_window(), Some(main));
    }
}
