//! Floating overlay builders — the symbol hover tooltip, the chrome-strip
//! command-palette dropdown, and the shared click-outside-to-dismiss
//! layer.
//!
//! Extracted verbatim from `view/mod.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files. The overlay-assembly
//! entry point `collect_overlays` stays in `view/mod.rs` alongside the
//! other composition-core methods.

use super::*;

impl Signex {
    /// Hover tooltip card showing the placed symbol's designator,
    /// value, footprint, and library id. Only paints after the cursor
    /// has dwelled on a Symbol hit for >= 250 ms — gates impulsive
    /// motion from popping the card. Returns None when the gate
    /// hasn't tripped, when no schematic is active, or when the
    /// uuid no longer resolves (e.g. the symbol was deleted while
    /// the dwell timer was running).
    pub(super) fn view_hover_tooltip(&self) -> Option<Element<'_, Message>> {
        use iced::widget::{column, container, text};
        use iced::{Background, Border, Color};

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
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let border_c = crate::styles::ti(tokens.border);
        // Match the schematic's own font (Iosevka by default; whatever
        // the user picked under Preferences ▸ Canvas Font) so the
        // tooltip reads as "this is data from the canvas" rather than
        // floating Roboto chrome. We reuse the app's `IOSEVKA` font
        // constant for canvas text so the lookup
        // hits the embedded TTF (not whatever the system fontconfig
        // falls back to for `Family::Name`).
        let canvas_font_name: &str = &self.document_state.panel_ctx.canvas_font_name;
        let canvas_font = if canvas_font_name == crate::fonts::DEFAULT_CANVAS_FONT
            || canvas_font_name.is_empty()
        {
            crate::render_config::IOSEVKA
        } else {
            crate::fonts::iced_font_for_family(canvas_font_name)
        };

        // Single row style — label in a fixed gutter, value fills the
        // remaining card width and wraps onto further lines on its
        // own when the payload is long. Keeps the rhythm uniform
        // across short (Designator / Value) and long (Footprint /
        // Library) fields without an inline-vs-stacked split.
        const CARD_W: f32 = 260.0;
        const LABEL_W: f32 = 60.0;
        let field = |label: &'static str, value: String| -> Element<'_, Message> {
            iced::widget::row![
                text(label)
                    .font(canvas_font)
                    .size(11)
                    .color(muted_c)
                    .width(LABEL_W),
                text(value)
                    .font(canvas_font)
                    .size(12)
                    .color(text_c)
                    .width(Length::Fill),
            ]
            .spacing(4)
            .width(Length::Fill)
            .align_y(iced::Alignment::Start)
            .into()
        };

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(4);
        rows.push(field("Designator", symbol.reference.clone()));
        if !symbol.value.is_empty() {
            rows.push(field("Value", symbol.value.clone()));
        }
        if !symbol.footprint.is_empty() {
            rows.push(field("Footprint", symbol.footprint.clone()));
        }
        if !symbol.lib_id.is_empty() {
            rows.push(field("Library", symbol.lib_id.clone()));
        }

        let card = container(column(rows).spacing(3))
            .padding(iced::Padding {
                top: 8.0,
                right: 14.0,
                bottom: 8.0,
                left: 10.0,
            })
            .width(Length::Fixed(CARD_W))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(panel_bg)),
                border: Border {
                    color: border_c,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            });

        // Offset to bottom-right so the card never sits directly under
        // the cursor — keeps the underlying symbol visible and avoids
        // hover flicker when the tooltip itself enters the cursor's
        // hover rectangle.
        const OFFSET: f32 = 16.0;
        let (ww, wh) = self.ui_state.window_size;
        // Conservative card-size estimate for edge clamping. The
        // actual size depends on font metrics so this is a guess
        // intended to keep the card on-screen near the right/bottom
        // edges; the iced layout will still render at its true size.
        const ESTIMATED_W: f32 = CARD_W;
        const ESTIMATED_H: f32 = 110.0;
        let mut x = sx + OFFSET;
        let mut y = sy + OFFSET;
        if x + ESTIMATED_W > ww {
            x = (sx - OFFSET - ESTIMATED_W).max(0.0);
        }
        if y + ESTIMATED_H > wh {
            y = (sy - OFFSET - ESTIMATED_H).max(0.0);
        }
        Some(super::view::translate::Translate::new(card, (x, y)).into())
    }

    /// Result list for the chrome-strip command palette. Anchored
    /// below the chrome strip; scoring + ranking happens in the
    /// `command_palette` module so this view stays a thin renderer.
    pub(super) fn view_command_palette_dropdown(&self) -> Element<'_, Message> {
        use crate::app::command_palette::{
            CommandSource, MAX_RESULTS, build_catalog, rank_results,
        };
        use iced::widget::{Space, button, column, container, row, scrollable, text};
        use iced::{Alignment, Background, Border, Color};

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let muted_c = crate::styles::ti(tokens.text_secondary);
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let border_c = crate::styles::ti(tokens.border);
        let accent_c = crate::styles::ti(tokens.accent);
        let hover_c = crate::styles::ti(tokens.hover);

        let catalog = build_catalog(self);
        let ranked = rank_results(&catalog, &self.ui_state.command_palette.query);
        let total = ranked.len();
        let selected = self
            .ui_state
            .command_palette
            .selected_index
            .min(total.saturating_sub(1));

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(MAX_RESULTS.min(total));
        for (display_idx, &(catalog_idx, _score)) in ranked.iter().take(MAX_RESULTS).enumerate() {
            let entry = &catalog[catalog_idx];
            let is_active = display_idx == selected;
            let row_bg = if is_active {
                Some(Background::Color(hover_c))
            } else {
                None
            };
            let source_label = match entry.source {
                CommandSource::Command => "Command",
                CommandSource::Symbol => "Symbol",
                CommandSource::File => "File",
            };
            let label_col = column![
                text(entry.label.clone()).size(12).color(text_c),
                text(if entry.detail.is_empty() {
                    String::new()
                } else {
                    entry.detail.clone()
                })
                .size(10)
                .color(muted_c),
            ]
            .spacing(2)
            .width(Length::Fill);
            let row_inner = row![label_col, text(source_label).size(10).color(muted_c),]
                .spacing(10)
                .align_y(Alignment::Center);
            let btn = button(row_inner)
                .width(Length::Fill)
                .padding([6, 12])
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
                        },
                        text_color: text_c,
                        ..button::Style::default()
                    }
                });
            rows.push(btn.into());
        }

        let body: Element<'_, Message> = if total == 0 {
            container(text("No results").size(12).color(muted_c))
                .padding([12, 14])
                .width(Length::Fill)
                .into()
        } else {
            let list = column(rows).spacing(2).padding(4);
            scrollable(list).height(Length::Shrink).into()
        };

        // Footer when there are more matches than we render.
        let footer: Element<'_, Message> = if total > MAX_RESULTS {
            container(
                text(format!(
                    "{} more results — refine query",
                    total - MAX_RESULTS
                ))
                .size(10)
                .color(muted_c),
            )
            .padding([4, 14])
            .width(Length::Fill)
            .into()
        } else {
            Space::new().height(0).into()
        };

        // Card width matches the chrome search bar exactly so the
        // dropdown reads as an extension of the input rather than a
        // floating popup that happens to be nearby.
        let card_w = CHROME_SEARCH_BAR_WIDTH;
        let card = container(column![body, footer])
            .width(card_w)
            .max_height(360.0)
            .padding(0)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(panel_bg)),
                border: Border {
                    color: border_c,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..container::Style::default()
            });

        let (ww, _wh) = self.ui_state.window_size;
        // Track the chrome search bar's actual layout position so the
        // dropdown lines up with the input. The chrome row is
        // `[menu, drag_fill, search, drag_fill, controls]`; the two
        // Fill drag zones split the leftover evenly, so the search
        // bar starts at `menu_w + leftover/2`.
        let menu_w = crate::menu_bar::approx_menu_bar_width();
        let leftover = (ww - menu_w - card_w - CHROME_CONTROLS_W).max(0.0);
        let x = (menu_w + leftover / 2.0).max(8.0);
        let y = crate::menu_bar::MENU_BAR_HEIGHT + 4.0;
        super::view::translate::Translate::new(card, (x, y)).into()
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
        const BACKDROP_OPACITY: f32 = 0.55;
        iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        0.0,
                        0.0,
                        0.0,
                        BACKDROP_OPACITY,
                    ))),
                    ..container::Style::default()
                }),
        )
        .on_press(on_press)
        .into()
    }
}
