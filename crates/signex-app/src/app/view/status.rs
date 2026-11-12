//! The status bar of one window, fed from the surface that window shows.

use iced::{Element, Widget as _};

use super::*;

impl Signex {
    /// The status bar for `window_id`. A symbol tab shows its own grid, unit,
    /// tool and segment and no schematic selection; its Grid and unit buttons
    /// drive the symbol's library.
    pub(in crate::app::view) fn view_status_bar(
        &self,
        window_id: iced::window::Id,
    ) -> Element<'_, Message> {
        let document = &self.document_state;
        let altium_import_name = document.altium_import_running.as_ref().map(|run| {
            run.input
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| run.input.display().to_string())
        });
        let (cursor_x, cursor_y) = self.status_cursor(window_id);
        let grid = self.status_grid(window_id);
        let symbol = self.status_symbol_path(window_id);
        let selected: &[signex_types::schematic::SelectedItem] = if symbol.is_some() {
            &[]
        } else {
            &self.interaction_state.canvas_for_window(window_id).selected
        };
        let activity = self.status_activity(window_id);
        let bar = crate::status_bar::view(
            cursor_x,
            cursor_y,
            grid.visible,
            self.ui_state.snap_enabled,
            self.status_zoom(window_id),
            grid.unit,
            self.status_tool(window_id),
            grid.size_mm,
            selected,
            &document.panel_ctx.tokens,
            document.inflight_git_commits.len(),
            altium_import_name.as_deref(),
            activity.as_ref(),
            self.status_active_layer(window_id),
            self.status_view_angle(window_id),
            &self.ui_state.i18n,
        );
        bar.map(move |req| Signex::status_bar_message(window_id, symbol.as_deref(), req))
            .boxed()
    }
}
