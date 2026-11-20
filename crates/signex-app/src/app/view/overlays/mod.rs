//! Floating overlay builders — the hover cards (`symbol_hover`,
//! `net_hover`), the chrome-strip command-palette dropdown, and the shared
//! click-outside-to-dismiss layer.
//!
//! Extracted verbatim from `view/mod.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files. The overlay-assembly
//! entry point `collect_overlays` stays in `view/mod.rs` alongside the
//! other composition-core methods.

use super::canvas_region;
use super::*;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of the command palette card in an undocked window.
const WINDOW_PALETTE_WIDTH: f32 = 480.0;
/// Width of the Graphite palette card under the chrome strip (the mock's
/// 640 px); independent of the search field's own width.
const GRAPHITE_PALETTE_WIDTH: f32 = 640.0;
/// Least gap between the chrome palette card and either window edge.
const PALETTE_EDGE_GAP: f32 = 16.0;

mod bars;
#[cfg(test)]
mod dismiss_tests;
mod modals;
mod net_hover;
mod palette;
#[cfg(test)]
mod palette_scrim_tests;
mod placement_paused;
pub(super) mod read_only_badge;
mod symbol_hover;
mod text_edit;
#[cfg(test)]
mod window_gate_tests;
mod zoom_pill;
#[cfg(test)]
pub(super) use zoom_pill::pill_origin as zoom_pill_origin;

impl Signex {
    /// The canvas rectangle in window coordinates, from the live dock sizes.
    pub(in crate::app) fn canvas_bounds(&self) -> iced::Rectangle {
        self.canvas_bounds_in(None)
    }

    /// [`Self::canvas_bounds`] in the undocked window `window`, laid out at
    /// that window's own size (`None`: the main window). In a tiled main
    /// window it is the focused pane (#2249).
    pub(in crate::app) fn canvas_bounds_in(
        &self,
        window: Option<iced::window::Id>,
    ) -> iced::Rectangle {
        let area = self.document_area_in(window);
        match (window, self.tiled_panes()) {
            (None, Some((layout, panes))) => {
                let active = self.document_state.active_tab;
                let index = panes.iter().position(|tab| *tab == active).unwrap_or(0);
                canvas_region::pane_bounds(area, panes.len(), index, layout)
            }
            _ => area,
        }
    }

    /// The region between the docks, chrome and status bar where the
    /// documents show, in `window` (`None`: the main window).
    pub(in crate::app) fn document_area_in(
        &self,
        window: Option<iced::window::Id>,
    ) -> iced::Rectangle {
        use crate::dock::PanelPosition::{Bottom, Left, Right};
        let dock = &self.document_state.dock;
        let extent = |pos, size| canvas_region::DockExtent {
            present: dock.has_panels(pos),
            collapsed: dock.is_collapsed(pos),
            size,
        };
        let ui = &self.ui_state;
        canvas_region::canvas_bounds(
            window.map_or(ui.window_size, |w| ui.size_of_window(w)),
            extent(Left, ui.left_width),
            extent(Right, ui.right_width),
            extent(Bottom, ui.bottom_height),
            &self.document_state.panel_ctx.tokens,
        )
    }

    /// Where a hover card may go in `window_id`: the canvas in the main
    /// window, the whole window elsewhere.
    pub(super) fn card_bounds(&self, window_id: iced::window::Id) -> iced::Rectangle {
        canvas_region::card_bounds(
            self.ui_state.main_window_id == Some(window_id),
            self.canvas_bounds(),
            self.ui_state.window_size,
        )
    }

    /// Result list for the chrome-strip command palette. Anchored
    /// below the chrome strip; scoring + ranking happens in the
    /// `command_palette` module so this view stays a thin renderer.
    pub(super) fn view_command_palette_dropdown(&self) -> Element<'_, Message> {
        use crate::app::command_palette::MAX_RESULTS;
        use iced::widget::{Space, column, container, scrollable};
        use iced::{Background, Border, Color};
        use signex_i18n::{ids, t};
        use signex_widgets::{text, text_input};

        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &*self.ui_state.i18n;
        let text_c = crate::styles::ti(tokens.text);
        let muted_c = crate::styles::ti(tokens.text_secondary);

        // Ranked in `update` (`refresh_command_palette_results`), read here.
        let results = &self.ui_state.command_palette.results;
        let total = self.ui_state.command_palette.total_matches;
        let selected = self
            .ui_state
            .command_palette
            .selected_index
            .min(total.saturating_sub(1));

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(MAX_RESULTS.min(total));
        for (display_idx, entry) in results.iter().enumerate() {
            let is_active = display_idx == selected;
            // `results` is already grouped (see `group_for_display`), so a
            // header goes wherever the source changes.
            if display_idx == 0 || results[display_idx - 1].source != entry.source {
                let count = results.iter().filter(|e| e.source == entry.source).count();
                rows.push(palette::group_header(
                    &self.ui_state.i18n,
                    tokens,
                    entry.source,
                    count,
                ));
            }
            rows.push(palette::result_row(
                &self.ui_state.i18n,
                tokens,
                entry,
                &self.ui_state.command_palette.query,
                is_active,
                Message::CommandPalette(CommandPaletteMsg::Select(display_idx)),
            ));
        }

        let body: Element<'_, Message> = if total == 0 {
            container(
                text(t!(i18n, ids::OVERLAY_PALETTE_NO_RESULTS))
                    .size(TS.label)
                    .color(muted_c),
            )
            .padding([SP.l, 14.0])
            .width(Length::Fill)
            .boxed()
        } else {
            let list = column(rows).spacing(SP.xs).padding(SP.s);
            // Bound the list so the 360 px card keeps room for the search
            // row above and the hint footer below.
            let list_cap = 360.0 - 2.0 * (TS.body * 1.6 + 2.0 * SP.l);
            scrollable(list)
                .height(Length::Fit.max(list_cap))
                .style(palette::scrollbar_style(tokens))
                .boxed()
        };

        // Footer with the hint line, and a note when there are more matches
        // than we render.
        let footer: Element<'_, Message> = if total > 0 {
            let more = (total > MAX_RESULTS).then(|| {
                t!(i18n, ids::OVERLAY_PALETTE_MORE_RESULTS, "count" => (total - MAX_RESULTS) as i64)
            });
            palette::hint_footer(i18n, tokens, results.get(selected), more)
        } else {
            Space::new().height(0).boxed()
        };

        // Card width and x come from the same geometry the chrome strip
        // lays the input out with, so the dropdown reads as an extension
        // of the input rather than a floating popup that happens to be
        // nearby — including after a resize shrinks the bar.
        let in_window = self.ui_state.command_palette.window.is_some();
        // An undocked window has no chrome bar: its palette carries the
        // text input at the top of the card.
        let (card_x, card_w) = if in_window {
            (16.0, WINDOW_PALETTE_WIDTH)
        } else {
            let (x, w) = self.chrome_search_bar_geometry();
            chrome_palette_card(x, w, self.ui_state.window_size.0)
        };
        let ink4 = crate::styles::tokens::resolve(tokens).ink4;
        let window_input: Option<Element<'_, Message>> =
            self.ui_state.command_palette.window.map(|window| {
                text_input(
                    t!(i18n, ids::OVERLAY_PALETTE_WINDOW_SEARCH),
                    &self.ui_state.command_palette.query,
                )
                .id(crate::app::command_palette::COMMAND_PALETTE_WINDOW_INPUT_ID.clone())
                .on_input(move |q| {
                    Message::CommandPalette(CommandPaletteMsg::QueryTypedInWindow(window, q))
                })
                .on_submit(Message::CommandPalette(CommandPaletteMsg::ExecuteSelected))
                .size(TS.body)
                .width(Length::Fill)
                .padding(0)
                .style(move |_: &iced::Theme, _| iced::widget::text_input::Style {
                    background: Background::Color(Color::TRANSPARENT),
                    border: Border::default(),
                    placeholder: ink4,
                    value: text_c,
                    selection: Color { a: 0.4, ..text_c },
                })
                .boxed()
            });
        let input = palette::search_row(
            i18n,
            tokens,
            &self.ui_state.command_palette.query,
            window_input,
        );
        let card = container(column![input, body, footer])
            .width(card_w)
            .height(Length::Fit.max(360.0))
            .padding(0)
            .style(crate::styles::modal_card(tokens));

        let y = if in_window {
            16.0
        } else {
            crate::menu_bar::MENU_BAR_HEIGHT + 4.0
        };
        signex_widgets::Translate::new(card.boxed(), (card_x, y)).boxed()
    }

    pub(super) fn dismiss_layer(on_press: Message) -> Element<'static, Message> {
        Self::dismiss_layer_tinted(
            on_press,
            crate::styles::tokens::scrim(crate::styles::tokens::BACKDROP_ALPHA),
        )
    }

    /// [`Self::dismiss_layer`] painted in `scrim`.
    pub(super) fn dismiss_layer_tinted(
        on_press: Message,
        scrim: iced::Color,
    ) -> Element<'static, Message> {
        // Opaque semi-transparent backdrop that blocks interaction with
        // underlying content. Left-click anywhere on it triggers the
        // dismiss message.
        //
        // Right-click menus use `MenuBackdrop` instead (#2349).
        //
        // We intentionally do *not* wire `on_right_press` — iced's
        // `mouse_area` would `capture_event()` the right-press and
        // prevent the underlying canvas from starting a pan. Instead
        // the canvas itself owns the right-press (its pan gesture) and
        // closes the context menu once the pan actually starts moving
        // (see `canvas/mod.rs`'s `CursorMoved` handler, which fires
        // `ContextMenuMsg::Close` the moment `pan_moved` flips on).
        //
        // The wheel is a separate matter, handled one level up:
        // `collect_overlays` wraps every overlay whose id
        // `overlay_id::swallows_scroll` marks, so scroll cannot reach the
        // canvas through an overlay regardless of whether that overlay
        // built a dismiss layer at all (#562).
        iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(scrim)),
                    ..container::Style::default()
                }),
        )
        .on_press(on_press)
        .boxed()
    }

    /// Active-bar dropdown menu (Place / Align / filter presets etc.).
    /// Absolute-positioned with `Translate` so the column can auto-size
    /// to its widest label. Pushes the dismiss layer then the dropdown.
    pub(in crate::app) fn active_bar_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let Some(ab_menu) = interaction.active_bar_menu else {
            return Vec::new();
        };
        // Only the window the dropdown was opened in paints it (#1910).
        if !self.overlay_belongs_to(interaction.active_bar_menu_window, window_id) {
            return Vec::new();
        }
        let has_selection = !interaction.canvas_for_window(window_id).selected.is_empty();
        let has_net_colors = !ui.net_colors.is_empty();
        let dropdown = crate::active_bar::view_dropdown(
            &self.ui_state.i18n,
            ab_menu,
            &document.panel_ctx.tokens,
            &interaction.selection_filters,
            &interaction.custom_filter_presets,
            self.ui_state.effective_theme_id(),
            has_selection,
            has_net_colors,
        )
        .map(Message::ActiveBar)
        .boxed();
        let metrics = signex_widgets::active_bar::Metrics::GRAPHITE;
        let x_off = crate::active_bar::dropdown_x_offset(ab_menu, &metrics);
        // Bar bottom: MENU_BAR_HEIGHT + tabs + top margin + bar height,
        // plus a small gap so the dropdown visually touches.
        let ab_y: f32 = canvas_region::chrome_top(&document.panel_ctx.tokens)
            + crate::active_bar::bar_top_margin()
            + crate::active_bar::bar_height_for(&metrics)
            + crate::active_bar::BAR_DROPDOWN_GAP_PX;
        let (ww, _) = ui.size_of_window(window_id);
        let adjusted_x =
            x_off + crate::active_bar::bar_left_in_window(interaction.current_tool, ww, &metrics);
        Self::dismiss_and_place(
            Message::ActiveBar(crate::active_bar::ActiveBarMsg::CloseMenus),
            dropdown,
            (adjusted_x, ab_y),
        )
    }

    /// Canvas right-click context menu (+ its Place/Align submenu): the
    /// card opens at the click and the open submenu's flyout sits beside it,
    /// level with its launcher row; both are kept inside the window. Pushes
    /// dismiss, then the placed menu.
    pub(in crate::app) fn context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let interaction = &self.interaction_state;
        let Some(ctx_menu) = interaction.context_menu.as_ref() else {
            return Vec::new();
        };
        // Only the window the menu was opened in paints it (#1910).
        if !self.overlay_belongs_to(ctx_menu.window, window_id) {
            return Vec::new();
        }
        let card = self.view_context_menu(window_id);
        let mut menu =
            super::context_menu::AnchoredMenu::new(card.element, (ctx_menu.x, ctx_menu.y));
        if let (Some(kind), Some(offset)) = (interaction.context_submenu, card.flyout_offset) {
            // A board's menu opens the board's flyouts (#2405).
            let submenu = if self.pcb_targets(Some(window_id)).shown {
                self.view_board_context_submenu(kind, window_id)
            } else {
                self.view_context_submenu(kind, window_id)
            };
            menu = menu.flyout(Self::submenu_panel(submenu), offset);
        }
        Self::anchored_layers(Message::ContextMenu(ContextMenuMsg::Close), menu)
    }

    /// Document-tab right-click menu. Rendered before the project-tree
    /// menu since the two are mutually exclusive. Pushes dismiss then the
    /// placed menu.
    pub(super) fn tab_context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let interaction = &self.interaction_state;
        let Some(tab_ctx) = interaction.tab_context_menu.as_ref() else {
            return self.dock_tab_context_menu_overlay();
        };
        // Only the main window's tab strip opens this menu, so only the main
        // window paints it (#2037).
        if !self.overlay_belongs_to(None, window_id) {
            return Vec::new();
        }
        Self::menu_overlay(
            Message::ContextMenu(ContextMenuMsg::CloseTab),
            self.view_tab_context_menu(tab_ctx),
            (tab_ctx.x, tab_ctx.y),
        )
    }

    /// Dock panel tab right-click menu: dismiss layer, then the placed menu.
    fn dock_tab_context_menu_overlay(&self) -> Vec<Element<'_, Message>> {
        let Some(menu_ctx) = self.document_state.dock.tab_menu else {
            return Vec::new();
        };
        Self::menu_overlay(
            Message::Dock(crate::dock::DockMessage::TabMenuDismissed),
            self.view_dock_tab_context_menu(),
            (menu_ctx.x, menu_ctx.y),
        )
    }

    /// Projects-panel tree right-click menu (+ its AddNewToProject
    /// submenu). Pushes dismiss, then the placed menu and its flyout.
    pub(super) fn project_tree_context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let interaction = &self.interaction_state;
        let Some(tree_ctx) = interaction.project_tree_context_menu.as_ref() else {
            return Vec::new();
        };
        // Only the window whose Projects panel opened the menu paints it.
        if !self.overlay_belongs_to(tree_ctx.window, window_id) {
            return Vec::new();
        }
        let card = self.view_project_tree_context_menu(tree_ctx);
        let mut menu =
            super::context_menu::AnchoredMenu::new(card.element, (tree_ctx.x, tree_ctx.y));
        // Only AddNewToProject opens from this menu.
        if let (Some(ContextSubmenu::AddNewToProject), Some(offset)) =
            (interaction.context_submenu, card.flyout_offset)
        {
            let submenu = self.view_context_submenu(ContextSubmenu::AddNewToProject, window_id);
            menu = menu.flyout(Self::submenu_panel(submenu), offset);
        }
        Self::anchored_layers(Message::ContextMenu(ContextMenuMsg::CloseProjectTree), menu)
    }

    /// v0.18.10 — Altium-style grid picker popup. Floats at the cursor
    /// when `G` is pressed in a footprint editor. Pushes dismiss then the
    /// placed menu.
    pub(super) fn grid_picker_overlay(&self) -> Vec<Element<'_, Message>> {
        let Some(picker) = self.interaction_state.grid_picker.as_ref() else {
            return Vec::new();
        };
        Self::menu_overlay(
            Message::Ui(UiMsg::GridPickerClose),
            self.view_grid_picker_menu(),
            (picker.x, picker.y),
        )
    }

    /// Dock drop preview — paints the region the dragged floating panel
    /// docks into if released now (`DockArea::drop_preview`).
    pub(super) fn dock_drag_zone_overlay(&self) -> Option<Element<'_, Message>> {
        use iced::widget::{column, container, row};

        let ui = &self.ui_state;
        let document = &self.document_state;
        let position = document.dock.drop_preview?;
        let geometry = document.dock.drop_geometry(
            ui.window_size,
            (ui.left_width, ui.right_width, ui.bottom_height),
        );
        let depth = geometry.preview_depth(position);
        let zone_style = crate::styles::dock_zone_highlight(&document.panel_ctx.tokens);
        let zone = |width: Length, height: Length| {
            container(iced::widget::Space::new())
                .width(width)
                .height(height)
                .style(zone_style)
        };
        Some(match position {
            crate::dock::PanelPosition::Left => zone(depth.into(), Length::Fill).boxed(),
            crate::dock::PanelPosition::Right => row![
                iced::widget::Space::new().width(Length::Fill),
                zone(depth.into(), Length::Fill),
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
            crate::dock::PanelPosition::Bottom => column![
                iced::widget::Space::new().height(Length::Fill),
                zone(Length::Fill, depth.into()),
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
        })
    }

    /// Floating panels — one `Translate`-positioned widget per floating
    /// panel. No clamp: panels follow Altium behaviour and may be
    /// dragged anywhere, even past the window edge (the OS clips).
    pub(super) fn floating_panels_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        let mut out: Vec<Element<'_, Message>> = Vec::new();
        for i in 0..document.dock.floating.len() {
            if let Some(panel_widget) = document.dock.view_floating_panel(
                i,
                self.panel_ctx_for(window_id),
                &self.library,
                self.sheet_grid(),
            ) {
                let fp = &document.dock.floating[i];
                out.push(
                    signex_widgets::Translate::new(
                        panel_widget
                            .map(move |msg| self.scope_dock_message(window_id, Message::Dock(msg)))
                            .boxed(),
                        (fp.x, fp.y),
                    )
                    .boxed(),
                );
            }
        }
        out
    }
}

/// `element` with the mouse wheel stopped at its own bounds, so it cannot
/// reach the canvas underneath (#562). Wrap the painted card, not a
/// window-sized positioner (`Translate`, a `Fill` column): `mouse_area`
/// captures wherever its bounds are, so a positioner would take the wheel
/// over the whole canvas.
pub(super) fn scroll_guard<'a>(element: Element<'a, Message>) -> Element<'a, Message> {
    // `Idle` makes the surface count as a hit for the `Stack`, which hands
    // the layers below a levitating cursor: no hover reaches the canvas.
    iced::widget::mouse_area(element)
        .on_scroll(|_| Message::noop())
        .interaction(iced::mouse::Interaction::Idle)
        .boxed()
}

/// The x and width of the palette card under the chrome strip, centred on
/// the search field (`search_x`, `search_w` from
/// `chrome_search_bar_geometry`) and clamped to stay [`PALETTE_EDGE_GAP`]
/// inside a `window_w` wide window.
fn chrome_palette_card(search_x: f32, search_w: f32, window_w: f32) -> (f32, f32) {
    let room = window_w - 2.0 * PALETTE_EDGE_GAP;
    let w = GRAPHITE_PALETTE_WIDTH.min(room.max(WINDOW_PALETTE_WIDTH));
    let centred = search_x + (search_w - w) / 2.0;
    let x = centred
        .min(window_w - w - PALETTE_EDGE_GAP)
        .max(PALETTE_EDGE_GAP);
    (x, w)
}

#[cfg(test)]
mod palette_width_tests {
    /// The palette card is 640 px wide (maintainer, 2026-11-10).
    #[test]
    fn the_chrome_palette_card_is_640_px_wide() {
        assert_eq!(super::chrome_palette_card(700.0, 380.0, 1600.0).1, 640.0);
    }

    /// The card is centred on the search field when the window has room.
    #[test]
    fn the_chrome_palette_card_centres_on_the_search_field() {
        let (x, w) = super::chrome_palette_card(600.0, 380.0, 1600.0);
        assert_eq!(x + w / 2.0, 600.0 + 380.0 / 2.0);
    }

    /// Near either window edge the card is clamped inside the window.
    #[test]
    fn the_chrome_palette_card_stays_inside_the_window() {
        let gap = super::PALETTE_EDGE_GAP;
        let (x, _) = super::chrome_palette_card(100.0, 380.0, 900.0);
        assert_eq!(x, gap);
        let (x, w) = super::chrome_palette_card(1150.0, 380.0, 1560.0);
        assert_eq!(x + w, 1560.0 - gap);
    }
}
