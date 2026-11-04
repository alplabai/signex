//! What an undocked window opens besides panels: the Find dialog, the command
//! palette and the Projects menu each belong to the window they were opened
//! in, and go away with it (#2037).

use super::super::*;

impl Signex {
    /// Close the Find dialog, command palette and Projects menu that
    /// `window` opened, when it closes or reattaches: nothing else would clear
    /// them, and they would then act on a sheet that window no longer shows.
    pub(crate) fn close_surfaces_owned_by(&mut self, window: iced::window::Id) {
        if self.ui_state.find_replace.window == Some(window) {
            let find = &mut self.ui_state.find_replace;
            find.open = false;
            find.window = None;
            find.matches.clear();
            find.selected_index = None;
            self.cancel_replace_job("the window it searched was closed");
        }
        if self.ui_state.command_palette.window == Some(window) {
            self.ui_state.command_palette.open = false;
            self.ui_state.command_palette.window = None;
        }
        let interaction = &mut self.interaction_state;
        if interaction
            .project_tree_context_menu
            .as_ref()
            .is_some_and(|menu| menu.window == Some(window))
        {
            interaction.project_tree_context_menu = None;
            interaction.context_submenu = None;
        }
    }
}
