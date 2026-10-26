//! Command palette dispatch — open/close, query updates, execute.

use iced::Task;
use iced::widget::operation;

use super::super::*;
use crate::app::command_palette::{
    COMMAND_PALETTE_INPUT_ID, CommandAction, MAX_RESULTS, build_catalog, catalog_stamp,
    rank_results,
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
        let stamp = catalog_stamp(self);
        let palette = &mut self.ui_state.command_palette;
        palette.catalog_stamp = stamp;
        palette.total_matches = ranked.len();
        palette.results = ranked
            .iter()
            .take(MAX_RESULTS)
            .map(|&(catalog_idx, _)| catalog[catalog_idx].clone())
            .collect();
    }

    /// Re-rank the open palette when the symbols or files behind its rows
    /// changed since the list was built, so Enter never runs a row the user
    /// is not looking at.
    pub(in crate::app) fn refresh_command_palette_if_stale(&mut self) {
        if !self.ui_state.command_palette.open
            || self.ui_state.command_palette.catalog_stamp == catalog_stamp(self)
        {
            return;
        }
        self.refresh_command_palette_results();
        let palette = &mut self.ui_state.command_palette;
        palette.selected_index = palette
            .selected_index
            .min(palette.results.len().saturating_sub(1));
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
        // The row the user sees and selected: the cached results, not a
        // re-rank, which can order differently once the catalog has moved.
        let Some(action) = self
            .ui_state
            .command_palette
            .results
            .get(self.ui_state.command_palette.selected_index)
            .map(|entry| entry.action.clone())
        else {
            // Empty result list — close the palette quietly.
            self.ui_state.command_palette.open = false;
            return Task::none();
        };

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

    fn symbol_row(reference: &str) -> (String, String, String, String) {
        (
            reference.into(),
            String::new(),
            String::new(),
            String::new(),
        )
    }

    #[test]
    fn enter_runs_the_row_the_user_selected_not_a_fresh_ranking() {
        let (mut app, _task) = Signex::new();
        // A cached row the live catalog does not contain: a re-rank against
        // the query would find nothing and drop the action.
        let palette = &mut app.ui_state.command_palette;
        palette.open = true;
        palette.query = "qzxjk-no-live-match".into();
        palette.results = vec![crate::app::command_palette::CommandEntry {
            source: crate::app::command_palette::CommandSource::Command,
            label: "Open Panel: ERC".into(),
            detail: String::new(),
            action: CommandAction::Panel(crate::panels::PanelKind::Erc),
        }];
        palette.selected_index = 0;

        let task = app.execute_command_palette_selected();

        assert_eq!(task.units(), 1, "the cached row's action must run");
        assert!(!app.ui_state.command_palette.open);
    }

    #[test]
    fn an_open_palette_follows_the_symbols_behind_its_rows() {
        let (mut app, _task) = Signex::new();
        app.document_state.panel_ctx.placed_symbols = vec![symbol_row("QZX1")];
        let _focus = app.open_command_palette();
        app.ui_state.command_palette.query = "QZX".into();
        app.refresh_command_palette_results();
        assert_eq!(app.ui_state.command_palette.total_matches, 1);

        // A symbol is placed while the palette is open.
        app.document_state.panel_ctx.placed_symbols = vec![symbol_row("QZX1"), symbol_row("QZX2")];
        app.refresh_command_palette_if_stale();

        assert_eq!(app.ui_state.command_palette.total_matches, 2);
    }
}
