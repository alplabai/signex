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
        let tokens = &self.document_view.panel_ctx.tokens;
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
                    &self.document_view.panel_ctx.canvas_colors,
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
                        // raw: a fallback of the component editor, which
                        // moves with the editors (#2060).
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
            // Only the active tab of the main window edge-pans (#2872).
            let active = self
                .document_state
                .tabs
                .get(self.document_state.active_tab)
                .is_some_and(|shown| shown.path == tab.path);
            let edge_pan = (in_main && active).then_some(&self.interaction_state.symbol_edge_pan);
            return Some(
                crate::library::editor::standalone::view_symbol(
                    editor,
                    &self.document_view.panel_ctx,
                    display,
                    path,
                    self.ui_state.device_px_per_logical_px(window_id),
                    edge_pan,
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
                    &self.document_view.panel_ctx.ui_font_name,
                    self.library.mirror_cards.get(path),
                )
                .map(Message::Library)
                .boxed(),
            );
        }
        let path = tab.kind.as_library_browser()?;
        Some(match self.library.library_browsers.get(path) {
            Some(browser) => {
                let look = crate::library::browser::PreviewLook {
                    canvas_colors: self.document_view.panel_ctx.canvas_colors,
                    grid: self.sheet_grid(),
                    device_px_per_logical_px: self.ui_state.device_px_per_logical_px(window_id),
                };
                let (library, i18n) = (&self.library, &self.ui_state.i18n);
                // A narrow window scrolls the tab sideways inside its frame,
                // so the header, grid and preview stay whole (mockup row 118).
                crate::library::editor::kit::sideways_frame(
                    crate::library::browser::MIN_BROWSER_W,
                    tokens,
                    move || {
                        crate::library::browser::view(path, library, browser, tokens, i18n, look)
                    },
                )
                .map(Message::Library)
                .boxed()
            }
            // The browser-state map out of step with the tabs: keep the tab
            // renderable rather than crashing.
            None => container(
                // raw: a fallback of the Library Browser, which moves
                // with the library views (#2060).
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
