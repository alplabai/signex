//! Command palette dispatch — open/close, query updates, execute.

use iced::Task;
use iced::widget::operation;

use super::super::*;
use crate::app::command_palette::{
    COMMAND_PALETTE_INPUT_ID, CommandAction, MAX_RESULTS, build_catalog, rank_results,
};

impl Signex {
    pub(super) fn dispatch_command_palette_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CommandPaletteOpen => self.open_command_palette(),
            Message::CommandPaletteClose => {
                self.ui_state.command_palette.open = false;
                Task::none()
            }
            Message::CommandPaletteQueryChanged(q) => {
                self.ui_state.command_palette.query = q;
                self.ui_state.command_palette.selected_index = 0;
                // First keystroke promotes a passive (placeholder)
                // chrome bar into a live palette.
                self.ui_state.command_palette.open = true;
                Task::none()
            }
            Message::CommandPaletteMoveSelection(delta) => {
                self.move_command_palette_selection(delta);
                Task::none()
            }
            Message::CommandPaletteSelect(idx) => {
                self.ui_state.command_palette.selected_index = idx;
                self.execute_command_palette_selected()
            }
            Message::CommandPaletteExecuteSelected => self.execute_command_palette_selected(),
            _ => unreachable!("dispatch_command_palette_message received non-palette message"),
        }
    }

    fn open_command_palette(&mut self) -> Task<Message> {
        self.ui_state.command_palette.open = true;
        self.ui_state.command_palette.selected_index = 0;
        // Focus + select-all so a fresh `Ctrl+Shift+P` can be typed
        // over without the user clearing the previous query manually.
        Task::batch([
            operation::focus(COMMAND_PALETTE_INPUT_ID.clone()),
            operation::select_all(COMMAND_PALETTE_INPUT_ID.clone()),
        ])
    }

    fn move_command_palette_selection(&mut self, delta: i32) {
        // Recompute the result count so wrapping respects the live
        // query — saves carrying a separate "results_len" cache.
        let catalog = build_catalog(self);
        let total = rank_results(&catalog, &self.ui_state.command_palette.query)
            .len()
            .min(MAX_RESULTS);
        if total == 0 {
            self.ui_state.command_palette.selected_index = 0;
            return;
        }
        let cur = self.ui_state.command_palette.selected_index as i32;
        // Wrap so ArrowDown past the bottom lands on the first row,
        // matching VS Code's palette.
        let next = (cur + delta).rem_euclid(total as i32);
        self.ui_state.command_palette.selected_index = next as usize;
    }

    fn execute_command_palette_selected(&mut self) -> Task<Message> {
        let catalog = build_catalog(self);
        let ranked = rank_results(&catalog, &self.ui_state.command_palette.query);
        let Some(&(catalog_idx, _)) = ranked.get(self.ui_state.command_palette.selected_index)
        else {
            // Empty result list — close the palette quietly.
            self.ui_state.command_palette.open = false;
            return Task::none();
        };
        let action = catalog[catalog_idx].action.clone();

        // Close palette before dispatching so re-entrant updates
        // (e.g. a menu that itself opens a modal) don't fight an
        // open dropdown.
        self.ui_state.command_palette.open = false;
        self.ui_state.command_palette.query.clear();
        self.ui_state.command_palette.selected_index = 0;

        match action {
            CommandAction::Menu(menu_msg) => Task::done(Message::Menu(menu_msg)),
            CommandAction::Panel(panel) => Task::done(Message::OpenPanel(panel)),
            CommandAction::OpenFile(path) => Task::done(Message::File(FileMsg::Opened(Some(path)))),
            CommandAction::FocusSymbol { reference } => self.focus_symbol_by_reference(&reference),
        }
    }

    /// Resolve a placed symbol's world position from the active
    /// engine and dispatch a `FocusAt` to centre + select it. No-op
    /// when nothing matches (e.g. the symbol was deleted between
    /// catalog build and Enter press).
    fn focus_symbol_by_reference(&self, reference: &str) -> Task<Message> {
        let Some(active_path) = self.document_state.active_path.clone() else {
            return Task::none();
        };
        let Some(engine) = self.document_state.engines.get(&active_path) else {
            return Task::none();
        };
        let Some(symbol) = engine
            .document()
            .symbols
            .iter()
            .find(|s| s.reference == reference)
        else {
            return Task::none();
        };
        Task::done(Message::FocusAt {
            world_x: symbol.position.x,
            world_y: symbol.position.y,
            select: Some(signex_types::schematic::SelectedItem {
                uuid: symbol.uuid,
                kind: signex_types::schematic::SelectedKind::Symbol,
            }),
        })
    }
}
