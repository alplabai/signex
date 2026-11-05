//! Command palette dispatch — open/close, query updates, execute.

use iced::Task;
use iced::widget::operation;

use super::super::*;
use crate::app::command_palette::{
    COMMAND_PALETTE_INPUT_ID, COMMAND_PALETTE_WINDOW_INPUT_ID, CommandAction, MAX_RESULTS,
    build_catalog, catalog_stamp, rank_results,
};
use crate::app::handlers::canvas::{AfterOpen, SheetOpen};

impl Signex {
    pub(super) fn dispatch_command_palette_message(
        &mut self,
        message: CommandPaletteMsg,
    ) -> Task<Message> {
        match message {
            CommandPaletteMsg::Open => {
                self.ui_state.command_palette.window = None;
                self.open_command_palette()
            }
            CommandPaletteMsg::OpenInWindow(window) => {
                self.ui_state.command_palette.window = self.overlay_owner(window);
                self.open_command_palette()
            }
            CommandPaletteMsg::Close => {
                self.ui_state.command_palette.open = false;
                Task::none()
            }
            CommandPaletteMsg::QueryChanged(q) => {
                // First keystroke promotes a passive (placeholder)
                // chrome bar into a live palette. The bar is the main
                // window's, so the typed query is the main window's.
                self.ui_state.command_palette.window = None;
                self.change_palette_query(q)
            }
            CommandPaletteMsg::QueryTypedInWindow(window, q) => {
                // A late keystroke for a palette that closed or moved to
                // another window is stale.
                if !self.ui_state.command_palette.open
                    || self.ui_state.command_palette.window != Some(window)
                {
                    return Task::none();
                }
                self.change_palette_query(q)
            }
            CommandPaletteMsg::IndexBuilt { generation, result } => {
                self.apply_palette_index(generation, &result);
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

    fn change_palette_query(&mut self, query: String) -> Task<Message> {
        self.ui_state.command_palette.query = query;
        self.ui_state.command_palette.selected_index = 0;
        self.ui_state.command_palette.open = true;
        self.refresh_command_palette_results();
        self.start_palette_index()
    }

    fn open_command_palette(&mut self) -> Task<Message> {
        self.ui_state.command_palette.open = true;
        self.ui_state.command_palette.selected_index = 0;
        self.refresh_command_palette_results();
        let input = if self.ui_state.command_palette.window.is_some() {
            COMMAND_PALETTE_WINDOW_INPUT_ID.clone()
        } else {
            COMMAND_PALETTE_INPUT_ID.clone()
        };
        // Focus + select-all so a fresh `Ctrl+Shift+P` can be typed
        // over without the user clearing the previous query manually.
        Task::batch([
            operation::focus(input.clone()),
            operation::text_input::select_all(input),
            // The project-wide rows come from an index built off-thread.
            self.start_palette_index(),
        ])
    }

    /// Rebuild the catalog and rank it against the live query, keeping the
    /// top [`MAX_RESULTS`] for the dropdown to render.
    pub(in crate::app) fn refresh_command_palette_results(&mut self) {
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
        let window = self.ui_state.command_palette.window;
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
                self.dispatch_command(&command, crate::app::command::CommandArgs::none(), window)
            }
            CommandAction::Panel(panel) => {
                Task::done(Message::Overlay(OverlayMsg::OpenPanel(panel)))
            }
            CommandAction::OpenFile(path) => Task::done(Message::File(FileMsg::Opened(Some(path)))),
            CommandAction::FocusSymbol { reference } => {
                self.focus_symbol_by_reference(&reference, window)
            }
            CommandAction::Reveal {
                path,
                placement,
                select,
            } => self.reveal_in_project(path, placement, select),
        }
    }

    /// Open `path` at `placement` and focus the item a search row names. A
    /// row can outlive what it pointed at (a sheet closed, renamed or
    /// deleted, a symbol removed): the file is looked up by path, so a stale
    /// row never opens a different file, and a missing item opens the sheet
    /// and says so. Nothing in any document is changed.
    ///
    /// A sheet nobody holds is read on a worker; the item is selected when it
    /// arrives.
    fn reveal_in_project(
        &mut self,
        path: std::path::PathBuf,
        placement: Option<signex_types::schematic::InstancePath>,
        select: Option<signex_types::schematic::SelectedItem>,
    ) -> Task<Message> {
        self.open_sheet(
            path,
            SheetOpen::at(placement).then(AfterOpen::Reveal { item: select }),
        )
    }

    /// Select `item` where it is now on the sheet at `path`, which is showing
    /// (not where it was when the index was built).
    pub(crate) fn reveal_item_on_active_sheet(
        &mut self,
        path: &std::path::Path,
        select: Option<signex_types::schematic::SelectedItem>,
    ) -> Task<Message> {
        let Some(item) = select else {
            return Task::none();
        };
        let position = self.document_state.active_engine().and_then(|engine| {
            engine
                .document()
                .symbols
                .iter()
                .find(|s| s.uuid == item.uuid)
                .map(|s| s.position)
        });
        let Some(position) = position else {
            crate::diagnostics::log_info(format!(
                "Search: that item is no longer on {}.",
                path.display()
            ));
            return Task::none();
        };
        self.handle_focus_at(position.x, position.y, Some(item))
    }

    /// Resolve a placed symbol's world position from the active
    /// engine and dispatch a `FocusAt` to centre + select it. No-op
    /// when nothing matches (e.g. the symbol was deleted between
    /// catalog build and Enter press).
    fn focus_symbol_by_reference(
        &mut self,
        reference: &str,
        window: Option<iced::window::Id>,
    ) -> Task<Message> {
        // A palette opened in an undocked window offers that window's
        // symbols: they are found and focused on its sheet.
        let Some(active_path) = self.palette_sheet_path(window) else {
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
        let (x, y) = (symbol.position.x, symbol.position.y);
        let item = signex_types::schematic::SelectedItem {
            uuid: symbol.uuid,
            kind: signex_types::schematic::SelectedKind::Symbol,
        };
        match window.filter(|w| self.is_document_window(*w)) {
            Some(window) => {
                self.run_in_window_document(window, |app| app.handle_focus_at(x, y, Some(item)))
            }
            None => Task::done(Message::Overlay(OverlayMsg::FocusAt {
                world_x: x,
                world_y: y,
                select: Some(item),
            })),
        }
    }

    /// The sheet a palette opened in `window` reads symbols from.
    fn palette_sheet_path(&self, window: Option<iced::window::Id>) -> Option<std::path::PathBuf> {
        match window.and_then(|w| self.ui_state.windows.get(&w)) {
            Some(crate::app::state::WindowKind::UndockedTab { path, .. }) => Some(path.clone()),
            _ => self.document_state.active_path.clone(),
        }
    }

    /// The panel context whose placed symbols the palette lists: the window's
    /// it was opened in, else the main window's.
    pub(crate) fn palette_panel_ctx(&self) -> &crate::panels::PanelContext {
        match self.ui_state.command_palette.window {
            Some(window) => self.panel_ctx_for(window),
            None => &self.document_state.panel_ctx,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::handlers::menu::export::tests::app_workspace;

    #[test]
    fn the_query_ranks_into_the_cache_the_view_reads() {
        let dir = std::env::temp_dir().join(format!("signex-palette-{}", uuid::Uuid::now_v7()));
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

    /// The messages `task` hands back to `update`, in order.
    fn output_messages(task: Task<Message>) -> Vec<Message> {
        use futures::StreamExt as _;
        let Some(mut stream) = iced_runtime::task::into_stream(task) else {
            return Vec::new();
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut out = Vec::new();
        runtime.block_on(async {
            while let Some(action) = stream.next().await {
                if let iced_runtime::Action::Output(message) = action {
                    out.push(message);
                }
            }
        });
        out
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

        // Exactly one message comes out, and it is the cached row's action.
        let messages = output_messages(task);
        assert!(
            matches!(
                messages.as_slice(),
                [Message::Overlay(OverlayMsg::OpenPanel(
                    crate::panels::PanelKind::Erc
                ))]
            ),
            "the cached row's action must run, got {messages:?}"
        );
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

    /// #1351 - a palette row goes through the same funnel as a keystroke,
    /// so a command with nothing to act on is dropped, not run.
    #[test]
    fn a_palette_command_obeys_the_enablement_gate() {
        use crate::app::command_palette::{CommandEntry, CommandSource};
        let (mut app, _task) = Signex::new();
        assert!(app.document_state.active_engine().is_none());
        let before = app.interaction_state.current_tool;
        let palette = &mut app.ui_state.command_palette;
        palette.open = true;
        palette.results = vec![CommandEntry {
            source: CommandSource::Command,
            label: "Place Wire".into(),
            detail: String::new(),
            action: CommandAction::Command(crate::keymap::AppCommandId::new("place_wire").unwrap()),
        }];
        palette.selected_index = 0;

        let _ = app.execute_command_palette_selected();

        assert_eq!(app.interaction_state.current_tool, before);
        assert!(!app.ui_state.command_palette.open);
    }
}
