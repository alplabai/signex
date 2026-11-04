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

mod bars;
mod modals;
mod net_hover;
pub(super) mod read_only_badge;
#[cfg(test)]
mod window_gate_tests;

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
        use iced::widget::{column, container, text};
        use iced::{Background, Border};

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
                    .size(TS.body)
                    .color(muted_c)
                    .width(LABEL_W),
                text(value)
                    .font(canvas_font)
                    .size(TS.label)
                    .color(text_c)
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                    .width(Length::Fill),
            ]
            .spacing(SP.s)
            .width(Length::Fill)
            .align_y(iced::Alignment::Start)
            .boxed()
        };

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(4);
        rows.push(field(
            "Designator",
            self.shown_designator(symbol).to_string(),
        ));
        rows.push(field(
            "Value",
            crate::library::part_label::card_value(&symbol.value).to_owned(),
        ));
        if !symbol.footprint.is_empty() {
            rows.push(field("Footprint", symbol.footprint.clone()));
        }
        if !symbol.lib_id.is_empty() {
            rows.push(field(
                "Library",
                self.library
                    .drift
                    .display_name(&symbol.lib_id, &self.library.open_libraries),
            ));
        }

        let rows_len = rows.len();
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
                    ..Border::default()
                },
                ..container::Style::default()
            });

        // Below-right of the cursor, flipped or clamped to stay on the
        // canvas (not the Properties panel beside it). Rows wrap, so the
        // height budgets a few lines each plus the card padding; iced lays
        // the card out at its true size.
        const ESTIMATED_ROW_H: f32 = 28.0;
        let estimated_h = 16.0 + ESTIMATED_ROW_H * rows_len as f32;
        let (x, y) =
            canvas_region::place_card((sx, sy), (CARD_W, estimated_h), self.card_bounds(window_id));
        Some(super::view::translate::Translate::new(card.boxed(), (x, y)).boxed())
    }

    /// Result list for the chrome-strip command palette. Anchored
    /// below the chrome strip; scoring + ranking happens in the
    /// `command_palette` module so this view stays a thin renderer.
    pub(super) fn view_command_palette_dropdown(&self) -> Element<'_, Message> {
        use crate::app::command_palette::{CommandSource, MAX_RESULTS};
        use iced::widget::{Space, button, column, container, row, scrollable, text};
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

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(MAX_RESULTS.min(total));
        for (display_idx, entry) in results.iter().enumerate() {
            let is_active = display_idx == selected;
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
            scrollable(list).height(Length::Shrink).boxed()
        };

        // Footer when there are more matches than we render.
        let footer: Element<'_, Message> = if total > MAX_RESULTS {
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
        let (card_x, card_w) = self.chrome_search_bar_geometry();
        let card = container(column![body, footer])
            .width(card_w)
            .height(Length::Fit.max(360.0))
            .padding(0)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(panel_bg)),
                border: Border {
                    color: border_c,
                    width: 1.0,
                    radius: 6.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });

        let y = crate::menu_bar::MENU_BAR_HEIGHT + 4.0;
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
        let x_off = crate::active_bar::dropdown_x_offset(ab_menu);
        // Bar bottom: MENU_BAR_HEIGHT + tabs + top margin + bar height,
        // plus a small gap so the dropdown visually touches.
        let ab_y: f32 = canvas_region::CHROME_TOP
            + crate::active_bar::BAR_TOP_MARGIN_PX
            + crate::active_bar::BAR_HEIGHT_PX
            + crate::active_bar::BAR_DROPDOWN_GAP_PX;
        let (ww, _) = ui.size_of_window(window_id);
        let adjusted_x =
            x_off + crate::active_bar::bar_left_in_window(interaction.current_tool, ww);
        vec![
            Self::dismiss_layer(Message::ActiveBar(
                crate::active_bar::ActiveBarMsg::CloseMenus,
            )),
            super::translate::Translate::new(dropdown, (adjusted_x, ab_y)).boxed(),
        ]
    }

    /// Canvas right-click context menu (+ its Place/Align submenu).
    /// Clamps the menu inside the window; the submenu pops to the right
    /// (or left on overflow) aligned to its launcher row. Pushes dismiss,
    /// menu, then the optional submenu.
    pub(in crate::app) fn context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        use iced::widget::{column, row};

        let interaction = &self.interaction_state;
        let Some(ctx_menu) = interaction.context_menu.as_ref() else {
            return Vec::new();
        };
        // Only the window the menu was opened in paints it (#1910).
        if !self.overlay_belongs_to(ctx_menu.window, window_id) {
            return Vec::new();
        }
        let menu = self.view_context_menu(window_id);
        // Clamp the menu inside the window so a click near the
        // right/bottom edge doesn't push it off-screen. Estimate
        // the menu's footprint conservatively from the maximum
        // possible row count (22 rows) and
        // CONTEXT_MENU_WIDTH; flip-up / flip-left when the click
        // lands too close to an edge.
        let window_size = self.ui_state.size_of_window(window_id);
        let (x, y) = super::context_menu::clamp_menu_origin(
            (ctx_menu.x, ctx_menu.y),
            (
                Self::CONTEXT_MENU_WIDTH,
                super::context_menu::menu_height(22),
            ),
            window_size,
        );
        let menu_w = Self::CONTEXT_MENU_WIDTH;
        let edge_margin = super::context_menu::EDGE_MARGIN;
        let win_w = window_size.0;
        let mut out: Vec<Element<'_, Message>> = Vec::new();
        out.push(Self::dismiss_layer(Message::ContextMenu(
            ContextMenuMsg::Close,
        )));
        out.push(
            column![
                iced::widget::Space::new().height(y),
                row![
                    iced::widget::Space::new().width(x),
                    menu,
                    iced::widget::Space::new().width(Length::Fill),
                ]
                .width(Length::Fill),
            ]
            .boxed(),
        );
        // Submenu (Place / Align) — pop to the right of the parent
        // menu (or left if the right edge would overflow), and
        // align its top to the launcher row's y-position so the
        // first submenu item sits next to the row that opened it.
        if let Some(submenu_kind) = interaction.context_submenu {
            let submenu = self.view_context_submenu(
                submenu_kind,
                interaction.canvas_for_window(window_id).selected.len(),
            );
            // Wrap in mouse_area so on_enter/on_exit on the panel
            // can extend the close timer when the cursor crosses
            // from the launcher into the submenu and back.
            let submenu = iced::widget::mouse_area(submenu)
                .on_enter(Message::ContextMenu(ContextMenuMsg::SubmenuEnterPanel))
                .on_exit(Message::ContextMenu(ContextMenuMsg::SubmenuLeavePanel));
            let submenu_w = menu_w;
            let sub_x = if x + menu_w + submenu_w + edge_margin > win_w {
                (x - submenu_w).max(0.0)
            } else {
                x + menu_w
            };
            // Approximate launcher-row y inside the parent menu.
            // Each ctx_menu_item_* row is ≈ 22 px tall (text + 4 px
            // top + 4 px bottom + a tiny line-height fudge); the
            // separator is rendered as a 1 px line. The numbers
            // below come from counting rows above each launcher in
            // `view_context_menu`.
            const ROW_H: f32 = 22.0;
            const SEP_H: f32 = 1.0;
            const TOP_PAD: f32 = 4.0;
            let launcher_y = match submenu_kind {
                // Above Place: 3 always-visible rows + 1 separator.
                ContextSubmenu::Place => TOP_PAD + 3.0 * ROW_H + SEP_H,
                // Align is only shown when something is selected;
                // above Align: the same 3 rows + 1 sep, then
                // Place / Part Actions / Sheet Actions / References.
                ContextSubmenu::Align => TOP_PAD + 7.0 * ROW_H + SEP_H,
                // Arrange sits right under Align, but only when a drawing is
                // selected (#1583).
                ContextSubmenu::Arrange => TOP_PAD + 8.0 * ROW_H + SEP_H,
                // AddNewToProject only fires from the project-tree
                // menu, never from the canvas menu — fall through
                // to a safe placeholder if the state somehow leaks
                // (no submenu rendered, just a 0-offset).
                ContextSubmenu::AddNewToProject => 0.0,
            };
            let sub_y = (y + launcher_y - 4.0).max(0.0);
            out.push(
                column![
                    iced::widget::Space::new().height(sub_y),
                    row![
                        iced::widget::Space::new().width(sub_x),
                        submenu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .boxed(),
            );
        }
        out
    }

    /// Document-tab right-click menu. Rendered before the project-tree
    /// menu since the two are mutually exclusive. Pushes dismiss then the
    /// clamped menu.
    pub(super) fn tab_context_menu_overlay(&self) -> Vec<Element<'_, Message>> {
        use iced::widget::{column, row};

        let ui = &self.ui_state;
        let interaction = &self.interaction_state;
        let Some(tab_ctx) = interaction.tab_context_menu.as_ref() else {
            return self.dock_tab_context_menu_overlay();
        };
        let menu = self.view_tab_context_menu(tab_ctx);
        // Conservative footprint matches the project-tree menu so
        // the two visually align.
        let (x, y) = super::context_menu::clamp_menu_origin(
            (tab_ctx.x, tab_ctx.y),
            (
                Self::CONTEXT_MENU_WIDTH,
                super::context_menu::menu_height(5),
            ),
            ui.window_size,
        );
        vec![
            Self::dismiss_layer(Message::ContextMenu(ContextMenuMsg::CloseTab)),
            column![
                iced::widget::Space::new().height(y),
                row![
                    iced::widget::Space::new().width(x),
                    menu,
                    iced::widget::Space::new().width(Length::Fill),
                ]
                .width(Length::Fill),
            ]
            .boxed(),
        ]
    }

    /// Dock panel tab right-click menu: dismiss layer, then the menu clamped
    /// into the window.
    fn dock_tab_context_menu_overlay(&self) -> Vec<Element<'_, Message>> {
        use iced::widget::{column, row};

        let Some(menu_ctx) = self.document_state.dock.tab_menu else {
            return Vec::new();
        };
        let menu = self.view_dock_tab_context_menu();
        let (x, y) = super::context_menu::clamp_menu_origin(
            (menu_ctx.x, menu_ctx.y),
            (
                Self::CONTEXT_MENU_WIDTH,
                super::context_menu::menu_height(5),
            ),
            self.ui_state.window_size,
        );
        vec![
            Self::dismiss_layer(Message::Dock(crate::dock::DockMessage::TabMenuDismissed)),
            column![
                iced::widget::Space::new().height(y),
                row![
                    iced::widget::Space::new().width(x),
                    menu,
                    iced::widget::Space::new().width(Length::Fill),
                ]
                .width(Length::Fill),
            ]
            .boxed(),
        ]
    }

    /// Projects-panel tree right-click menu (+ its AddNewToProject
    /// submenu). Pushes dismiss, menu, then the optional submenu.
    pub(super) fn project_tree_context_menu_overlay(&self) -> Vec<Element<'_, Message>> {
        use iced::widget::{column, row};

        let ui = &self.ui_state;
        let interaction = &self.interaction_state;
        let Some(tree_ctx) = interaction.project_tree_context_menu.as_ref() else {
            return Vec::new();
        };
        let menu = self.view_project_tree_context_menu(tree_ctx);
        // Conservative footprint: at most 6 rows × 22 px + 8 px
        // padding. Width matches the canvas menu so the two look
        // consistent.
        let (x, y) = super::context_menu::clamp_menu_origin(
            (tree_ctx.x, tree_ctx.y),
            (
                Self::CONTEXT_MENU_WIDTH,
                super::context_menu::menu_height(6),
            ),
            ui.window_size,
        );
        let menu_w = Self::CONTEXT_MENU_WIDTH;
        let edge_margin = super::context_menu::EDGE_MARGIN;
        let win_w = ui.window_size.0;
        let mut out: Vec<Element<'_, Message>> = Vec::new();
        out.push(Self::dismiss_layer(Message::ContextMenu(
            ContextMenuMsg::CloseProjectTree,
        )));
        out.push(
            column![
                iced::widget::Space::new().height(y),
                row![
                    iced::widget::Space::new().width(x),
                    menu,
                    iced::widget::Space::new().width(Length::Fill),
                ]
                .width(Length::Fill),
            ]
            .boxed(),
        );
        // Adjacent submenu (currently only AddNewToProject opens
        // from this menu). Mirrors the canvas-menu submenu logic
        // above — pop to the right of the parent (or left if the
        // right edge would overflow), align top to the launcher
        // row's y inside the parent menu.
        if let Some(ContextSubmenu::AddNewToProject) = interaction.context_submenu {
            let submenu = self.view_context_submenu(
                ContextSubmenu::AddNewToProject,
                interaction.canvas.selected.len(),
            );
            let submenu = iced::widget::mouse_area(submenu)
                .on_enter(Message::ContextMenu(ContextMenuMsg::SubmenuEnterPanel))
                .on_exit(Message::ContextMenu(ContextMenuMsg::SubmenuLeavePanel));
            let submenu_w = menu_w;
            let sub_x = if x + menu_w + submenu_w + edge_margin > win_w {
                (x - submenu_w).max(0.0)
            } else {
                x + menu_w
            };
            // Launcher position inside the project-tree menu:
            // `Make Project Available Online...` (row 0)
            // `Validate Project`                 (row 1)
            // `Add New to Project ›`             (row 2) ← target
            // → top + 2 rows, no separator above the launcher.
            const ROW_H: f32 = 22.0;
            const TOP_PAD: f32 = 4.0;
            let launcher_y = TOP_PAD + 2.0 * ROW_H;
            let sub_y = (y + launcher_y - 4.0).max(0.0);
            out.push(
                column![
                    iced::widget::Space::new().height(sub_y),
                    row![
                        iced::widget::Space::new().width(sub_x),
                        submenu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .boxed(),
            );
        }
        out
    }

    /// v0.18.10 — Altium-style grid picker popup. Floats at the cursor
    /// when `G` is pressed in a footprint editor. Pushes dismiss then the
    /// clamped menu.
    pub(super) fn grid_picker_overlay(&self) -> Vec<Element<'_, Message>> {
        use iced::widget::{column, row};

        let ui = &self.ui_state;
        let interaction = &self.interaction_state;
        let Some(picker) = interaction.grid_picker.as_ref() else {
            return Vec::new();
        };
        let menu = self.view_grid_picker_menu();
        let (x, y) = super::context_menu::clamp_menu_origin(
            (picker.x, picker.y),
            (200.0, super::context_menu::menu_height(13)),
            ui.window_size,
        );
        vec![
            Self::dismiss_layer(Message::Ui(UiMsg::GridPickerClose)),
            column![
                iced::widget::Space::new().height(y),
                row![
                    iced::widget::Space::new().width(x),
                    menu,
                    iced::widget::Space::new().width(Length::Fill),
                ]
                .width(Length::Fill),
            ]
            .boxed(),
        ]
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
