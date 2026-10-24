//! Command palette dispatch — open/close, query updates, execute.

use iced::Task;
use iced::widget::operation;

use super::super::*;
use crate::app::command_palette::{
    COMMAND_PALETTE_INPUT_ID, CommandAction, MAX_RESULTS, build_catalog, rank_results,
};

impl Signex {
    pub(super) fn dispatch_command_palette_message(
        &mut self,
        message: CommandPaletteMsg,
    ) -> Task<Message> {
        match message {
            CommandPaletteMsg::Open => self.open_command_palette(),
            CommandPaletteMsg::Close => {
                self.ui_state.command_palette.open = false;
                Task::none()
            }
            CommandPaletteMsg::QueryChanged(q) => {
                self.ui_state.command_palette.query = q;
                self.ui_state.command_palette.selected_index = 0;
                // First keystroke promotes a passive (placeholder)
                // chrome bar into a live palette.
                self.ui_state.command_palette.open = true;
                self.refresh_command_palette_results();
                Task::none()
            }
            CommandPaletteMsg::MoveSelection(delta) => {
                self.move_command_palette_selection(delta);
                Task::none()
            }
            CommandPaletteMsg::Select(idx) => {
                self.ui_state.command_palette.selected_index = idx;
                self.execute_command_palette_selected()
            }
            CommandPaletteMsg::ExecuteSelected => self.execute_command_palette_selected(),
        }
    }

    fn open_command_palette(&mut self) -> Task<Message> {
        self.ui_state.command_palette.open = true;
        self.ui_state.command_palette.selected_index = 0;
        self.refresh_command_palette_results();
        // Focus + select-all so a fresh `Ctrl+Shift+P` can be typed
        // over without the user clearing the previous query manually.
        Task::batch([
            operation::focus(COMMAND_PALETTE_INPUT_ID.clone()),
            operation::select_all(COMMAND_PALETTE_INPUT_ID.clone()),
        ])
    }

    /// Rebuild the catalog and rank it against the live query, keeping the
    /// top [`MAX_RESULTS`] for the dropdown to render.
    fn refresh_command_palette_results(&mut self) {
        let catalog = build_catalog(self);
        let ranked = rank_results(&catalog, &self.ui_state.command_palette.query);
        let palette = &mut self.ui_state.command_palette;
        palette.total_matches = ranked.len();
        palette.results = ranked
            .iter()
            .take(MAX_RESULTS)
            .map(|&(catalog_idx, _)| catalog[catalog_idx].clone())
            .collect();
    }

    fn move_command_palette_selection(&mut self, delta: i32) {
        let total = self.ui_state.command_palette.total_matches.min(MAX_RESULTS);
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
            // Through the same bridge the keyboard uses. The palette
            // carries an id and never learns which `Message` it becomes
            // — Command ≠ Message (#278). An id that stopped resolving
            // between `build_catalog` and here is a no-op rather than a
            // panic; `core_to_message` logs which one and why.
            CommandAction::Command(command) => {
                // `bridge::` rather than the `command::core_to_message`
                // re-export — #367 (PR #504) makes that re-export private.
                // Once #504 lands this becomes
                // `self.dispatch_command(&command, CommandArgs::none(), None)`,
                // which is the single funnel the epic is aiming at.
                match crate::app::command::bridge::core_to_message(&command) {
                    Some(message) => self.dispatch_update(message),
                    None => Task::none(),
                }
            }
            CommandAction::Panel(panel) => {
                Task::done(Message::Overlay(OverlayMsg::OpenPanel(panel)))
            }
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
        Task::done(Message::Overlay(OverlayMsg::FocusAt {
            world_x: symbol.position.x,
            world_y: symbol.position.y,
            select: Some(signex_types::schematic::SelectedItem {
                uuid: symbol.uuid,
                kind: signex_types::schematic::SelectedKind::Symbol,
            }),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::handlers::menu::export::tests::app_workspace;

    #[test]
    fn the_query_ranks_into_the_cache_the_view_reads() {
        let dir = std::env::temp_dir().join(format!("signex-palette-{}", uuid::Uuid::new_v4()));
        let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);

        let _ =
            app.dispatch_command_palette_message(CommandPaletteMsg::QueryChanged(String::new()));
        let everything = app.ui_state.command_palette.total_matches;
        assert!(everything > 0);
        assert_eq!(
            app.ui_state.command_palette.results.len(),
            everything.min(MAX_RESULTS)
        );

        let _ = app.dispatch_command_palette_message(CommandPaletteMsg::QueryChanged(
            "zzzzqqqqxxxx".to_string(),
        ));
        assert_eq!(app.ui_state.command_palette.total_matches, 0);
        assert!(app.ui_state.command_palette.results.is_empty());
    }
}
