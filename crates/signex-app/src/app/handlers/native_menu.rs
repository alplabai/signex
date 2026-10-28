//! The macOS system menu bar's two touch points with the Model: choosing a
//! row, and keeping the bar current after every `update`.
//! Design: docs/MENU_BAR.md, "Native macOS menu".

use crate::app::Message;
use crate::app::state::Signex;
use crate::menu_bar::native::resolve_activation;
use iced::Task;

impl Signex {
    /// A native row was chosen. The id is looked up in the map of the bar
    /// as it stands now, so an id that is stale or whose row is disabled
    /// does nothing, even if the event was queued before a rebuild.
    pub(crate) fn handle_native_menu_activated(&mut self, id: &str) -> Task<Message> {
        match resolve_activation(&self.native_menu.actions, id).cloned() {
            Some(action) => self.dispatch_update(Message::Menu(action)),
            None => {
                crate::diagnostics::log_info(format!(
                    "Main menu: ignored \"{id}\", which is disabled or no longer in the menu"
                ));
                Task::none()
            }
        }
    }

    /// Rebuild the native spec and apply it, only when what the menu reads
    /// (enablement facts, recent projects) moved since the last sync.
    #[cfg(target_os = "macos")]
    pub(crate) fn sync_native_menu(&mut self) {
        use crate::menu_bar::MenuContext;
        use crate::menu_bar::model::menu_bar_model;
        use crate::menu_bar::native::{MenuInputs, build};

        let inputs = MenuInputs::new(
            self.command_context(self.ui_state.main_window_id),
            self.ui_state.recent_projects.entries(),
        );
        if !self.native_menu.needs_sync(&inputs) {
            return;
        }
        let ctx = MenuContext {
            commands: inputs.commands(),
            recent_projects: inputs.recent(),
            ..MenuContext::default()
        };
        let built = build(&menu_bar_model(&ctx));
        self.native_menu.sync(inputs, built);
    }
}
