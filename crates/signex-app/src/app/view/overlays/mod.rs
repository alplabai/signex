//! Floating overlay builders — the symbol hover tooltip, the chrome-strip
//! command-palette dropdown, and the shared click-outside-to-dismiss
//! layer.
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

mod bars;
mod modals;
mod net_hover;
mod palette;
pub(super) mod read_only_badge;
#[cfg(test)]
mod window_gate_tests;
mod zoom_pill;

impl Signex {
    /// The canvas rectangle in window coordinates, from the live dock sizes.
    pub(in crate::app) fn canvas_bounds(&self) -> iced::Rectangle {
        self.canvas_bounds_in(None)
    }

    /// [`Self::canvas_bounds`] in the undocked window `window`, laid out at
    /// that window's own size (`None`: the main window).
    pub(in crate::app) fn canvas_bounds_in(
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

    /// Hover tooltip card showing the placed symbol's designator,
    /// value, footprint, and library id. Only paints after the cursor
    /// has dwelled on a Symbol hit for >= 250 ms — gates impulsive
    /// motion from popping the card. Returns None when the gate
    /// hasn't tripped, when no schematic is active, or when the
    /// uuid no longer resolves (e.g. the symbol was deleted while
    /// the dwell timer was running).
    pub(super) fn view_hover_tooltip(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        use iced::widget::text;

        if self.interaction_state.editing_text.is_some() {
            return None;
        }
        // A net card (a wire, pin or label, #1270) takes the place of the
        // symbol card while the pointer rests on a pin.
        if let Some(card) = &self.document_state.net_nav.hover.card {
            return Some(self.view_net_hover_card(card, window_id));
        }
        let interaction = &self.interaction_state;
        let uuid = interaction.hover_symbol_uuid?;
        let started = interaction.hover_started_at?;
        if started.elapsed() < std::time::Duration::from_millis(700) {
            return None;
        }
        let (sx, sy) = interaction.hover_screen_pos?;
        let active_path = self.document_state.active_path.as_ref()?;
        let engine = self.document_state.engines.get(active_path)?;
        let symbol = engine.document().symbols.iter().find(|s| s.uuid == uuid)?;

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let muted_c = crate::styles::ti(tokens.text_secondary);
        // Match the schematic's own font (Iosevka by default; whatever
        // the user picked under Preferences ▸ Canvas Font) so the
        // tooltip reads as "this is data from the canvas" rather than
        // floating Roboto chrome. We reuse the app's `IOSEVKA` font
        // constant for canvas text so the lookup
        // hits the embedded TTF (not whatever the system fontconfig
        // falls back to for `Family::Name`).
        let canvas_font_name: &str = &self.document_state.panel_ctx.canvas_font_name;
        let canvas_font = crate::fonts::canvas_iced_font(canvas_font_name);

        // Single row style — label in a fixed gutter, value fills the
        // remaining card width and wraps onto further lines on its
        // own when the payload is long. Keeps the rhythm uniform
        // across short (Designator / Value) and long (Footprint /
        // Library) fields without an inline-vs-stacked split.
        let mut fields: Vec<(&'static str, String)> = Vec::with_capacity(4);
        fields.push(("Designator", self.shown_designator(symbol).to_string()));
        fields.push((
            "Value",
            crate::library::part_label::card_value(&symbol.value).to_owned(),
        ));
        if !symbol.footprint.is_empty() {
            fields.push(("Footprint", symbol.footprint.clone()));
        }
        if !symbol.lib_id.is_empty() {
            fields.push((
                "Library",
                self.library
                    .drift
                    .display_name(&symbol.lib_id, &self.library.open_libraries),
            ));
        }
        let (label_w, card_w) = symbol_card_widths(&fields, canvas_font_name);
        // Below the cap the card shrinks to its text; at the cap it holds
        // the cap and long values wrap.
        let at_cap = card_w >= SYMBOL_CARD_MAX_W;
        let fill_or_shrink = if at_cap { Length::Fill } else { Length::Shrink };
        let field = |label: &'static str, value: String| -> Element<'_, Message> {
            iced::widget::row![
                text(label)
                    .font(canvas_font)
                    .size(TS.body)
                    .color(muted_c)
                    .width(label_w),
                text(value)
                    .font(canvas_font)
                    .size(TS.label)
                    .color(text_c)
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                    .width(fill_or_shrink),
            ]
            .spacing(SP.s)
            .width(fill_or_shrink)
            .align_y(iced::Alignment::Start)
            .boxed()
        };

        let rows: Vec<Element<'_, Message>> = fields
            .into_iter()
            .map(|(label, value)| field(label, value))
            .collect();

        let rows_len = rows.len();
        let card = hover_card(
            rows,
            3.0,
            if at_cap {
                Length::Fixed(card_w)
            } else {
                Length::Shrink
            },
            &self.document_state.panel_ctx.tokens,
        );

        // Below-right of the cursor, flipped or clamped to stay on the
        // canvas (not the Properties panel beside it). Rows wrap, so the
        // height budgets a few lines each plus the card padding; iced lays
        // the card out at its true size.
        const ESTIMATED_ROW_H: f32 = 28.0;
        let estimated_h = 16.0 + ESTIMATED_ROW_H * rows_len as f32;
        let (x, y) =
            canvas_region::place_card((sx, sy), (card_w, estimated_h), self.card_bounds(window_id));
        Some(super::view::translate::Translate::new(card, (x, y)).boxed())
    }

    /// Result list for the chrome-strip command palette. Anchored
    /// below the chrome strip; scoring + ranking happens in the
    /// `command_palette` module so this view stays a thin renderer.
    pub(super) fn view_command_palette_dropdown(&self) -> Element<'_, Message> {
        use crate::app::command_palette::{CommandSource, MAX_RESULTS};
        use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
        use iced::{Alignment, Background, Border, Color};

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let muted_c = crate::styles::ti(tokens.text_secondary);
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let border_c = crate::styles::ti(tokens.border);
        let accent_c = crate::styles::ti(tokens.accent);
        let hover_c = crate::styles::ti(tokens.hover);

        // Ranked in `update` (`refresh_command_palette_results`), read here.
        let results = &self.ui_state.command_palette.results;
        let total = self.ui_state.command_palette.total_matches;
        let selected = self
            .ui_state
            .command_palette
            .selected_index
            .min(total.saturating_sub(1));

        let graphite = crate::styles::is_graphite(tokens);
        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(MAX_RESULTS.min(total));
        for (display_idx, entry) in results.iter().enumerate() {
            let is_active = display_idx == selected;
            if graphite {
                // `results` is already grouped (see `group_for_display`), so
                // a header goes wherever the source changes.
                if display_idx == 0 || results[display_idx - 1].source != entry.source {
                    let count = results.iter().filter(|e| e.source == entry.source).count();
                    rows.push(palette::group_header(tokens, entry.source, count));
                }
                rows.push(palette::result_row(
                    tokens,
                    entry,
                    &self.ui_state.command_palette.query,
                    is_active,
                    Message::CommandPalette(CommandPaletteMsg::Select(display_idx)),
                ));
                continue;
            }
            let row_bg = if is_active {
                Some(Background::Color(hover_c))
            } else {
                None
            };
            let source_label = match entry.source {
                CommandSource::Command => "Command",
                CommandSource::Symbol => "Symbol",
                CommandSource::Net => "Net",
                CommandSource::Sheet => "Sheet",
                CommandSource::File => "File",
            };
            let label_col = column![
                text(entry.label.clone()).size(TS.label).color(text_c),
                text(if entry.detail.is_empty() {
                    String::new()
                } else {
                    entry.detail.clone()
                })
                .size(TS.caption)
                .color(muted_c),
            ]
            .spacing(SP.xs)
            .width(Length::Fill);
            let row_inner = row![
                label_col,
                text(source_label).size(TS.caption).color(muted_c),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            let btn = button(row_inner)
                .width(Length::Fill)
                .padding([6.0, SP.l])
                .on_press(Message::CommandPalette(CommandPaletteMsg::Select(
                    display_idx,
                )))
                .style(move |_: &iced::Theme, status: button::Status| {
                    let bg = match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            Some(Background::Color(hover_c))
                        }
                        _ => row_bg,
                    };
                    button::Style {
                        background: bg,
                        border: Border {
                            width: if is_active { 1.0 } else { 0.0 },
                            radius: 3.0.into(),
                            color: if is_active {
                                accent_c
                            } else {
                                Color::TRANSPARENT
                            },
                            ..Border::default()
                        },
                        text_color: text_c,
                        ..button::Style::default()
                    }
                });
            rows.push(btn.boxed());
        }

        let body: Element<'_, Message> = if total == 0 {
            container(text("No results").size(TS.label).color(muted_c))
                .padding([SP.l, 14.0])
                .width(Length::Fill)
                .boxed()
        } else {
            let list = column(rows).spacing(SP.xs).padding(SP.s);
            // Graphite bounds the list so the 360 px card keeps room for the
            // search row above and the hint footer below.
            let list_cap = 360.0 - 2.0 * (TS.body * 1.6 + 2.0 * SP.l);
            let list = scrollable(list).height(if graphite {
                Length::Fit.max(list_cap)
            } else {
                Length::Shrink
            });
            if graphite {
                list.style(palette::scrollbar_style(tokens)).boxed()
            } else {
                list.boxed()
            }
        };

        // Footer when there are more matches than we render.
        let footer: Element<'_, Message> = if graphite && total > 0 {
            let more = (total > MAX_RESULTS)
                .then(|| format!("{} more results — refine query", total - MAX_RESULTS));
            palette::hint_footer(tokens, results.get(selected), more)
        } else if total > MAX_RESULTS {
            container(
                text(format!(
                    "{} more results — refine query",
                    total - MAX_RESULTS
                ))
                .size(TS.caption)
                .color(muted_c),
            )
            .padding([SP.s, 14.0])
            .width(Length::Fill)
            .boxed()
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
        } else if graphite {
            let (x, _) = self.chrome_search_bar_geometry();
            let window_w = self.ui_state.window_size.0;
            let w = GRAPHITE_PALETTE_WIDTH.min((window_w - 32.0).max(WINDOW_PALETTE_WIDTH));
            (x.min((window_w - w - 16.0).max(16.0)), w)
        } else {
            self.chrome_search_bar_geometry()
        };
        let ink4 = crate::styles::tokens::resolve(tokens).ink4;
        let window_input: Option<Element<'_, Message>> =
            self.ui_state.command_palette.window.map(|window| {
                let field = text_input(
                    "Search files, symbols, commands…",
                    &self.ui_state.command_palette.query,
                )
                .id(crate::app::command_palette::COMMAND_PALETTE_WINDOW_INPUT_ID.clone())
                .on_input(move |q| {
                    Message::CommandPalette(CommandPaletteMsg::QueryTypedInWindow(window, q))
                })
                .on_submit(Message::CommandPalette(CommandPaletteMsg::ExecuteSelected))
                .size(TS.body)
                .width(Length::Fill);
                if graphite {
                    field
                        .padding(0)
                        .style(move |_: &iced::Theme, _| iced::widget::text_input::Style {
                            background: Background::Color(Color::TRANSPARENT),
                            border: Border::default(),
                            placeholder: ink4,
                            value: text_c,
                            selection: Color { a: 0.4, ..text_c },
                        })
                        .boxed()
                } else {
                    field.padding(SP.l).boxed()
                }
            });
        let input: Element<'_, Message> = if graphite {
            palette::search_row(tokens, &self.ui_state.command_palette.query, window_input)
        } else {
            window_input.unwrap_or_else(|| Space::new().height(0).boxed())
        };
        let graphite_card = crate::styles::modal_card(tokens);
        let card = container(column![input, body, footer])
            .width(card_w)
            .height(Length::Fit.max(360.0))
            .padding(0)
            .style(move |theme: &iced::Theme| {
                if graphite {
                    return graphite_card(theme);
                }
                container::Style {
                    background: Some(Background::Color(panel_bg)),
                    border: Border {
                        color: border_c,
                        width: 1.0,
                        radius: 6.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }
            });

        let y = if in_window {
            16.0
        } else {
            crate::menu_bar::menu_bar_height(&self.document_state.panel_ctx.tokens) + 4.0
        };
        super::view::translate::Translate::new(card.boxed(), (card_x, y)).boxed()
    }

    pub(super) fn dismiss_layer(on_press: Message) -> Element<'static, Message> {
        // Opaque semi-transparent backdrop that blocks interaction with
        // underlying content. Left-click anywhere on it triggers the
        // dismiss message.
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
                    background: Some(iced::Background::Color(crate::styles::tokens::scrim(
                        crate::styles::tokens::BACKDROP_ALPHA,
                    ))),
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
        let metrics = signex_widgets::active_bar::Metrics::of(&document.panel_ctx.tokens);
        let x_off = crate::active_bar::dropdown_x_offset(ab_menu, &metrics);
        // Bar bottom: MENU_BAR_HEIGHT + tabs + top margin + bar height,
        // plus a small gap so the dropdown visually touches.
        let ab_y: f32 = canvas_region::chrome_top(&document.panel_ctx.tokens)
            + crate::active_bar::bar_top_margin(&document.panel_ctx.tokens)
            + crate::active_bar::bar_height_for(&metrics)
            + crate::active_bar::BAR_DROPDOWN_GAP_PX;
        let (ww, _) = ui.size_of_window(window_id);
        let adjusted_x =
            x_off + crate::active_bar::bar_left_in_window(interaction.current_tool, ww, &metrics);
        vec![
            Self::dismiss_layer(Message::ActiveBar(
                crate::active_bar::ActiveBarMsg::CloseMenus,
            )),
            super::translate::Translate::new(dropdown, (adjusted_x, ab_y)).boxed(),
        ]
    }

    /// Canvas right-click context menu (+ its Place/Align submenu).
    /// Clamps the menu inside the window; the submenu pops to the right
    /// (or left on overflow) level with its launcher row. Pushes dismiss,
    /// menu, then the optional submenu.
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
        let menu = self.view_context_menu(window_id);
        // The footprint is estimated from the largest possible row count,
        // so a click near an edge flips the card up or left.
        let menu_w = Self::CONTEXT_MENU_WIDTH;
        let origin = self.menu_origin(window_id, (ctx_menu.x, ctx_menu.y), menu_w, 22, true);
        let mut out = Self::menu_layers(Message::ContextMenu(ContextMenuMsg::Close), menu, origin);
        if let Some(submenu_kind) = interaction.context_submenu {
            let submenu = self.view_context_submenu(
                submenu_kind,
                interaction.canvas_for_window(window_id).selected.len(),
            );
            // Rows and separators above each launcher in `view_context_menu`.
            let (rows_above, separators_above) = match submenu_kind {
                // 3 always-visible rows + 1 separator.
                ContextSubmenu::Place => (3, 1),
                // Shown only with a selection: the same, then Place / Part
                // Actions / Sheet Actions / References.
                ContextSubmenu::Align => (7, 1),
                // Right under Align, only when a drawing is selected (#1583).
                ContextSubmenu::Arrange => (8, 1),
                // Only the project-tree menu opens this one.
                ContextSubmenu::AddNewToProject => (0, 0),
            };
            let sub_origin = super::context_menu::submenu_origin(
                origin,
                menu_w,
                menu_w,
                rows_above,
                separators_above,
                true,
                self.ui_state.size_of_window(window_id).0,
            );
            out.push(Self::submenu_flyout(submenu, sub_origin));
        }
        out
    }

    /// Document-tab right-click menu. Rendered before the project-tree
    /// menu since the two are mutually exclusive. Pushes dismiss then the
    /// clamped menu.
    pub(super) fn tab_context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let interaction = &self.interaction_state;
        let Some(tab_ctx) = interaction.tab_context_menu.as_ref() else {
            return self.dock_tab_context_menu_overlay(window_id);
        };
        // Only the main window's tab strip opens this menu, so only the main
        // window paints it (#2037).
        if !self.overlay_belongs_to(None, window_id) {
            return Vec::new();
        }
        self.menu_overlay(
            window_id,
            Message::ContextMenu(ContextMenuMsg::CloseTab),
            self.view_tab_context_menu(tab_ctx),
            (tab_ctx.x, tab_ctx.y),
            Self::CONTEXT_MENU_WIDTH,
            5,
        )
    }

    /// Dock panel tab right-click menu: dismiss layer, then the menu clamped
    /// into the window.
    fn dock_tab_context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let Some(menu_ctx) = self.document_state.dock.tab_menu else {
            return Vec::new();
        };
        self.menu_overlay(
            window_id,
            Message::Dock(crate::dock::DockMessage::TabMenuDismissed),
            self.view_dock_tab_context_menu(),
            (menu_ctx.x, menu_ctx.y),
            Self::CONTEXT_MENU_WIDTH,
            5,
        )
    }

    /// Projects-panel tree right-click menu (+ its AddNewToProject
    /// submenu). Pushes dismiss, menu, then the optional submenu.
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
        let (menu, menu_w, origin) = self.view_project_tree_context_menu(window_id, tree_ctx);
        let mut out = Self::menu_layers(
            Message::ContextMenu(ContextMenuMsg::CloseProjectTree),
            menu,
            origin,
        );
        // Currently only AddNewToProject opens from this menu: row 2, under
        // `Make Project Available Online...` and `Validate Project`.
        if let Some(ContextSubmenu::AddNewToProject) = interaction.context_submenu {
            let submenu = self.view_context_submenu(
                ContextSubmenu::AddNewToProject,
                interaction.canvas.selected.len(),
            );
            let sub_origin = super::context_menu::submenu_origin(
                origin,
                menu_w,
                // The flyout is built at the shared width (`submenu.rs`).
                Self::CONTEXT_MENU_WIDTH,
                2,
                0,
                false,
                self.ui_state.size_of_window(window_id).0,
            );
            out.push(Self::submenu_flyout(submenu, sub_origin));
        }
        out
    }

    /// v0.18.10 — Altium-style grid picker popup. Floats at the cursor
    /// when `G` is pressed in a footprint editor. Pushes dismiss then the
    /// clamped menu.
    pub(super) fn grid_picker_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let Some(picker) = self.interaction_state.grid_picker.as_ref() else {
            return Vec::new();
        };
        self.menu_overlay(
            window_id,
            Message::Ui(UiMsg::GridPickerClose),
            self.view_grid_picker_menu(),
            (picker.x, picker.y),
            200.0,
            13,
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
                    super::translate::Translate::new(
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

/// The card both hover cards draw: `rows` on the panel background with a 1 px
/// border, padded `8 14 8 10`. `width` is `Shrink` to fit the text, or fixed
/// at the cap.
fn hover_card<'a>(
    rows: Vec<Element<'a, Message>>,
    spacing: f32,
    width: Length,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    let panel_bg = crate::styles::ti(tokens.panel_bg);
    let border_c = crate::styles::ti(tokens.border);
    container(column(rows).spacing(spacing))
        .padding(iced::Padding {
            top: 8.0,
            right: 14.0,
            bottom: 8.0,
            left: 10.0,
        })
        .width(width)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(panel_bg)),
            border: iced::Border {
                color: border_c,
                width: 1.0,
                radius: 4.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// Widest the symbol hover card grows; below that it fits its text and the
/// values wrap (#2100).
const SYMBOL_CARD_MAX_W: f32 = 320.0;
/// Left (10) plus right (14) padding, plus the 1 px border on each side.
const SYMBOL_CARD_PAD_W: f32 = 26.0;

/// Label column and card width for the symbol hover card `fields`: the
/// longest label, the gap and the longest value, at most
/// [`SYMBOL_CARD_MAX_W`].
fn symbol_card_widths(fields: &[(&str, String)], family: &str) -> (f32, f32) {
    let family = signex_types::font::nonblank_family(family);
    let width =
        |text: &str, size: f32| signex_types::font::estimate_text_width_px(text, size, family);
    let label_w = fields
        .iter()
        .map(|(label, _)| width(label, TS.body))
        .fold(0.0, f32::max)
        .ceil();
    let value_w = fields
        .iter()
        .map(|(_, value)| width(value, TS.label))
        .fold(0.0, f32::max);
    let card_w = (label_w + SP.s + value_w + SYMBOL_CARD_PAD_W)
        .ceil()
        .min(SYMBOL_CARD_MAX_W);
    (label_w, card_w)
}
