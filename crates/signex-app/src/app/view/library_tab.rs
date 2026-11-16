//! The library tabs' surfaces (Component Preview, symbol and footprint
//! editors, Library Browser) for one tab, so the main window's active tab,
//! an undocked window and an unfocused tile pane (#2249) draw them alike.

use iced::widget::{column, container};
use iced::{Element, Length, Widget as _};

use crate::app::{Message, Signex, TabInfo};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    /// The surface of `tab` in `window_id` when it is a library tab, `None`
    /// for any other kind. A Component Preview draws here in the main window
    /// only (`in_main`); its own window draws it through
    /// `WindowKind::ComponentEditor`.
    pub(in crate::app::view) fn view_library_tab<'a>(
        &'a self,
        tab: &'a TabInfo,
        window_id: iced::window::Id,
        in_main: bool,
    ) -> Option<Element<'a, Message>> {
        let tokens = &self.document_state.panel_ctx.tokens;
        if in_main && let Some(editor_id) = tab.kind.as_component_editor() {
            let address = crate::library::state::EditorAddress::new(
                editor_id.library_path.clone(),
                editor_id.table.clone(),
                editor_id.row_id,
            );
            return Some(if let Some(editor) = self.library.editors.get(&address) {
                crate::library::editor::view(
                    editor,
                    &self.library,
                    tokens,
                    &self.document_state.panel_ctx.canvas_colors,
                    self.sheet_grid(),
                    self.ui_state.device_px_per_logical_px(window_id),
                    address,
                    &self.ui_state.i18n,
                )
                .map(Message::Library)
                .boxed()
            } else {
                container(
                    column![
                        signex_widgets::text(signex_widgets::raw(
                            "Component Editor — state not yet loaded"
                        ))
                        .size(TS.title)
                        .color(crate::styles::ti(tokens.text_secondary)),
                    ]
                    .spacing(SP.s)
                    .align_x(iced::Alignment::Center),
                )
                .center(Length::Fill)
                .style(crate::styles::panel_region(tokens))
                .boxed()
            });
        }
        // Standalone primitive editor and Library Browser tabs (#1818).
        // Lookup is path-keyed via `DocumentState.symbol_editors` /
        // `footprint_editors` and `LibraryState.library_browsers`.
        if let Some(path) = tab.kind.as_symbol_editor()
            && let Some(editor) = self.document_state.symbol_editors.get(path)
        {
            // Every editor opened from one library shares its grid, unit and
            // background, as the status bar does.
            let display = self.symbol_display(path);
            return Some(
                crate::library::editor::standalone::view_symbol(
                    editor,
                    &self.document_state.panel_ctx,
                    display,
                    path,
                    self.ui_state.device_px_per_logical_px(window_id),
                )
                .map(Message::Library)
                .boxed(),
            );
        }
        if let Some(path) = tab.kind.as_footprint_editor()
            && let Some(editor) = self.document_state.footprint_editors.get(path)
        {
            return Some(
                crate::library::editor::standalone::view_footprint(
                    editor,
                    tokens,
                    self.ui_state.effective_theme_id(),
                    &self.ui_state.i18n,
                    self.shows_footprint_layer_strip(window_id),
                )
                .map(Message::Library)
                .boxed(),
            );
        }
        let path = tab.kind.as_library_browser()?;
        Some(match self.library.library_browsers.get(path) {
            Some(browser) => crate::library::browser::view(
                path,
                &self.library,
                browser,
                tokens,
                &self.ui_state.i18n,
            )
            .map(Message::Library)
            .boxed(),
            // The browser-state map out of step with the tabs: keep the tab
            // renderable rather than crashing.
            None => container(
                signex_widgets::text(signex_widgets::raw(
                    "Library Browser — state not yet loaded",
                ))
                .size(TS.title)
                .color(crate::styles::ti(tokens.text_secondary)),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(tokens))
            .boxed(),
        })
    }
}
