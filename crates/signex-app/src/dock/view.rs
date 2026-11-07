//! Dock rendering: region tab strips, collapsed rails, and floating panels.

use iced::widget::{
    Column, Space, Widget, button, canvas, column, container, mouse_area, row, svg, text,
};
use iced::{Color, Element, Length, Rectangle, Renderer, Theme};

use super::graphite;
use super::measure;
use super::types::*;
use crate::icons;
use crate::panels::components_panel::details::symbol_canvas::SheetGrid;
use crate::panels::{self, PanelKind};
use crate::styles::{self, Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Wrap a themed SVG handle into a 14×14 `Svg` widget. Kept as a free
/// function so every call site reads `svg_icon(icons::icon_close(tid))`
/// without extra boilerplate.
fn svg_icon(handle: svg::Handle) -> iced::widget::Svg<'static> {
    svg(handle).width(14).height(14)
}

/// Graphite 24x24 icon button around an already-built icon.
fn icon_button_of(
    icon: iced::widget::Svg<'static>,
    tokens: &signex_types::theme::ThemeTokens,
) -> iced::widget::Button<
    'static,
    DockMessage,
    iced::widget::Container<'static, iced::widget::Svg<'static>>,
> {
    button(container(icon).center(Length::Fill))
        .width(graphite::ICON_BUTTON)
        .height(graphite::ICON_BUTTON)
        .padding(0)
        .style(graphite::icon_button(tokens))
}

fn icon_button(
    handle: svg::Handle,
    tokens: &signex_types::theme::ThemeTokens,
) -> iced::widget::Button<
    'static,
    DockMessage,
    iced::widget::Container<'static, iced::widget::Svg<'static>>,
> {
    icon_button_of(svg_icon(handle), tokens)
}

/// Boxed button style of a rail tab.
type TabStyleFn = Box<dyn Fn(&Theme, button::Status) -> button::Style>;

impl DockArea {
    pub(crate) fn view_region<'a>(
        &'a self,
        position: PanelPosition,
        ctx: &'a panels::PanelContext,
        library: &'a crate::library::LibraryState,
        grid: SheetGrid,
    ) -> Element<'a, DockMessage> {
        let region = match position {
            PanelPosition::Left => &self.left,
            PanelPosition::Right => &self.right,
            PanelPosition::Bottom => &self.bottom,
        };

        if region.panels.is_empty() {
            return container(text("")).width(0).boxed();
        }

        if region.collapsed {
            return self.view_rail(position, region, ctx);
        }

        // ── Altium-style flat tabs with accent underline + overflow scroll ──
        let total_tabs = region.panels.len();
        let graphite = styles::is_graphite(&ctx.tokens);
        // Graphite scrolls the strip itself (wheel or thin bar), so it has
        // no arrows and never starts mid-strip.
        let offset = if graphite {
            0
        } else {
            region.tab_offset.min(total_tabs.saturating_sub(1))
        };
        let has_overflow = !graphite && total_tabs > 3;
        let can_scroll_left = offset > 0;
        let can_scroll_right = offset + 3 < total_tabs;

        let mut tab_row = row![].spacing(0.0).align_y(iced::Alignment::End);
        if graphite {
            tab_row = tab_row.height(Length::Fill);
        }

        // Left scroll arrow (only when tabs are scrolled)
        if has_overflow {
            let arrow = button(text("<").size(TS.caption))
                .padding([SP.s, SP.s])
                .style(button::text);
            tab_row = tab_row.push(
                if can_scroll_left {
                    arrow.on_press(DockMessage::TabScroll(position, -1))
                } else {
                    arrow
                }
                .boxed(),
            );
        }

        // Last visible panel index drives the `is_last` flag so
        // adjacent tabs share their L/R borders and the rightmost
        // tab actually closes off with a right edge.
        let last_panel_idx = region.panels.len().saturating_sub(1);
        // Graphite keeps the label size and padding; a long strip scrolls.
        let (tab_size, tab_pad) = (TS.body, graphite::TAB_PAD);
        for (i, panel) in region.panels.iter().enumerate().skip(offset) {
            let label = panel.label();
            let is_active = i == region.active;
            let is_last = i == last_panel_idx;

            let text_c = if is_active {
                styles::ti(ctx.tokens.text)
            } else {
                styles::ti(ctx.tokens.text_secondary)
            };
            // Visual feedback while dragging: give the source tab a
            // brighter border + tinted background so the user sees
            // which tab they grabbed.
            let is_dragging_this =
                matches!(self.tab_drag, Some((p, src)) if p == position && src == i);
            let is_hovered =
                matches!(self.hovered_tab, Some((p, idx)) if p == position && idx == i);
            // No manual width — Iced's layout engine measures the text.
            // The accent underline is done via bottom-padding on an outer
            // container whose background is the accent color, avoiding
            // Length::Fill which would expand the tab to the panel width.
            // Shared `signex_widgets::TabPill` custom widget — same
            // rounded-top + 3-sided border + accent underline that the
            // document tab bar uses. Panel tabs and document tabs stay
            // visually in lockstep — the accent line is drawn by
            // `TabPill` itself from `accent` + `is_active` below.
            let tab_active = styles::ti(ctx.tokens.hover);
            let accent = styles::ti(ctx.tokens.accent);
            let fill = if is_dragging_this {
                iced::Color { a: 0.22, ..accent }
            } else if graphite {
                // Graphite: the active tab merges into the panel fill.
                let panel = styles::ti(ctx.tokens.panel_bg);
                if is_active {
                    panel
                } else if is_hovered {
                    styles::tokens::hover_overlay(&ctx.tokens)
                } else {
                    iced::Color { a: 0.0, ..panel }
                }
            } else if is_active {
                tab_active
            } else if is_hovered {
                iced::Color {
                    a: tab_active.a * 0.70,
                    ..tab_active
                }
            } else {
                iced::Color {
                    a: tab_active.a * 0.35,
                    ..tab_active
                }
            };
            let pill_style = signex_widgets::tab_pill::TabPillStyle {
                fill,
                border: styles::ti(ctx.tokens.border),
                accent,
                is_active,
                is_last,
                // Panel tabs sit at the top of each dock region but
                // visually hang from the strip baseline that runs
                // ABOVE the panel content — so the accent stripe
                // belongs at the top of the pill and the rounded
                // corners flip to the bottom. Inverse of doc tabs.
                accent_position: signex_widgets::tab_pill::AccentPosition::Top,
            };
            // F27 — pin the panel-tab label to a single line. Without
            // `Wrapping::None` iced word-wraps "SCH Library" onto two
            // lines whenever the dock area squeezes the tab — which
            // happens any time the panel column is in its default
            // ~240 px width with several panels in the strip.
            let label_text = text(label)
                .size(tab_size)
                .color(text_c)
                .wrapping(iced::widget::text::Wrapping::None);
            let tab_content: Element<'_, DockMessage> = match graphite
                .then(|| graphite::tab_badge(*panel, ctx))
                .flatten()
            {
                Some((count, tone)) => row![
                    label_text.boxed(),
                    container(text(count.to_string()).size(TS.caption).boxed())
                        .padding([0.0, SP.s])
                        .style(styles::pill(&ctx.tokens, tone))
                        .boxed(),
                ]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center)
                .boxed(),
                None => label_text.boxed(),
            };
            let mut inner = container(tab_content).padding([SP.s, tab_pad]);
            if graphite {
                inner = inner.height(Length::Fill).align_y(iced::Alignment::Center);
            }
            let tab = mouse_area(
                signex_widgets::tab_pill::TabPill::new(inner.boxed(), pill_style).square(graphite),
            )
            .on_enter(DockMessage::TabHoverEnter(position, i))
            .on_exit(DockMessage::TabHoverExit(position, i))
            .on_press(DockMessage::TabDragStart(position, i))
            .on_release(DockMessage::TabClick(position, i))
            .on_right_press(DockMessage::TabContextMenuRequested(position, i));

            tab_row = tab_row.push(tab.boxed());
        }

        // Right scroll arrow (only when more tabs are hidden)
        if has_overflow {
            let arrow = button(text(">").size(TS.caption))
                .padding([SP.s, SP.s])
                .style(button::text);
            tab_row = tab_row.push(
                if can_scroll_right {
                    arrow.on_press(DockMessage::TabScroll(position, 1))
                } else {
                    arrow
                }
                .boxed(),
            );
        }

        // Collapse button (SVG icon)
        let tid = ctx.theme_id;
        let collapse_icon = match position {
            PanelPosition::Left => svg_icon(icons::icon_collapse_left(tid)),
            PanelPosition::Right => svg_icon(icons::icon_collapse_right(tid)),
            PanelPosition::Bottom => svg_icon(icons::icon_collapse_down(tid)),
        };
        let (close_btn, collapse_btn) = if graphite {
            (
                icon_button(icons::icon_close(tid), &ctx.tokens)
                    .on_press(DockMessage::ClosePanel(position, region.active))
                    .boxed(),
                icon_button_of(collapse_icon, &ctx.tokens)
                    .on_press(DockMessage::ToggleCollapse(position))
                    .boxed(),
            )
        } else {
            (
                button(svg_icon(icons::icon_close(tid)))
                    .padding([5.0, SP.s])
                    .style(button::text)
                    .on_press(DockMessage::ClosePanel(position, region.active))
                    .boxed(),
                button(collapse_icon)
                    .padding([5.0, SP.s])
                    .style(button::text)
                    .on_press(DockMessage::ToggleCollapse(position))
                    .boxed(),
            )
        };

        // Title of the currently active panel — drawn in the top bar
        // so the user can see the panel name without scanning the tab
        // row.
        let active_title = region
            .panels
            .get(region.active)
            .map(|p| p.label().to_string())
            .unwrap_or_default();
        let title_text = if graphite {
            text(active_title)
                .size(graphite::TITLE_SIZE)
                .font(graphite::title_font())
        } else {
            text(active_title).size(TS.body)
        }
        .color(styles::ti(ctx.tokens.text));
        let title_bar = row![
            container(title_text).padding(if graphite { [0, 10] } else { [5, 10] }),
            Space::new().width(Length::Fill),
            collapse_btn,
            close_btn,
        ]
        .spacing(0)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        // Bottom tab strip — just the tab buttons + overflow arrows.
        // Collapse / close moved up to the title bar so they're always
        // visible alongside the active panel name.
        let tab_row: Element<'_, DockMessage> = if graphite {
            iced::widget::scrollable(tab_row)
                .direction(iced::widget::scrollable::Direction::Horizontal(
                    iced::widget::scrollable::Scrollbar::new()
                        .width(3)
                        .scroller_width(3),
                ))
                .width(Length::Shrink)
                .height(Length::Fill)
                .boxed()
        } else {
            tab_row.boxed()
        };
        let tab_strip = row![tab_row, Space::new().width(Length::Fill),]
            .spacing(0)
            .align_y(iced::Alignment::End)
            .width(Length::Fill)
            .height(if graphite {
                Length::Fill
            } else {
                Length::Shrink
            });

        // Panel content
        let content: Element<'_, DockMessage> =
            if let Some(panel) = region.panels.get(region.active) {
                match *panel {
                    PanelKind::Library => {
                        // v0.9 Library panel — content lives in the
                        // library subsystem, not in `panels::view_panel`.
                        // Wrap the LibraryMessage in DockMessage::Library
                        // so the dispatcher can route it back out.
                        crate::library::panel::view(library, &ctx.tokens)
                            .map(DockMessage::Library)
                            .boxed()
                    }
                    PanelKind::Components => {
                        // v0.9 Stage 9 Components Panel — three mount
                        // sources (Project / Installed / Global) read
                        // through `library_state` directly, not
                        // through the legacy `PanelContext`. Project
                        // library paths are derived from
                        // `ctx.projects[].libraries[].root` so the
                        // dock signature stays narrow.
                        panels::components_panel::view(library, ctx, &ctx.tokens, grid)
                            .map(DockMessage::Library)
                            .boxed()
                    }
                    _ => panels::view_panel(*panel, ctx)
                        .map(DockMessage::Panel)
                        .boxed(),
                }
            } else {
                text("").boxed()
            };

        // Altium parity: title + collapse/close on top, tabs on the
        // bottom. Content grows between them.
        if graphite {
            // Projects panel footer: the ERC / DRC / last-commit summary.
            let footer = (position == PanelPosition::Left
                && region.panels.get(region.active) == Some(&PanelKind::Projects))
            .then(|| panels::graphite::summary_card(ctx))
            .flatten()
            .map(|card| {
                container(card.map(DockMessage::Panel))
                    .padding(SP.s)
                    .width(Length::Fill)
                    .boxed()
            });
            let mut body = column![
                container(title_bar)
                    .width(Length::Fill)
                    .center_y(graphite::TITLE_H)
                    .padding([0.0, SP.s])
                    .style(graphite::title_bar(&ctx.tokens)),
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(0)
                    .clip(true),
            ];
            if let Some(footer) = footer {
                body = body.push(footer);
            }
            return body
                .push(
                    container(tab_strip)
                        .width(Length::Fill)
                        .height(graphite::TAB_STRIP_H)
                        .clip(true)
                        .style(graphite::tab_strip(&ctx.tokens))
                        .boxed(),
                )
                .boxed();
        }
        column![
            container(title_bar)
                .width(Length::Fill)
                .padding([0.0, SP.s])
                .style(styles::tab_bar_strip(&ctx.tokens)),
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(0),
            // F32 — `clip(true)` so tab labels can't bleed past the
            // panel boundary into the adjacent canvas / dock region.
            // Combined with F27's `Wrapping::None` on the inner text:
            // labels stay one line and get hard-cut at the panel
            // edge instead of either wrapping or spilling.
            container(tab_strip)
                .width(Length::Fill)
                .padding([0.0, SP.s])
                .clip(true)
                .style(styles::tab_bar_strip(&ctx.tokens)),
        ]
        .boxed()
    }

    fn view_rail<'a>(
        &'a self,
        position: PanelPosition,
        region: &'a DockRegion,
        ctx: &'a panels::PanelContext,
    ) -> Element<'a, DockMessage> {
        // Altium-style collapsed panel: vertical tabs with full panel names
        // Each tab is a button with the panel name, stacked vertically
        let is_vertical = matches!(position, PanelPosition::Left | PanelPosition::Right);
        let graphite = styles::is_graphite(&ctx.tokens);

        if is_vertical {
            let mut rail: Column<Element<'a, DockMessage>> = Column::new().spacing(SP.xs);

            // Expand button with SVG arrow
            let tid = ctx.theme_id;
            let expand_icon = match position {
                PanelPosition::Left => svg_icon(icons::icon_expand_left(tid)),
                PanelPosition::Right => svg_icon(icons::icon_expand_right(tid)),
                _ => svg_icon(icons::icon_expand_left(tid)),
            };
            rail = rail.push(if graphite {
                icon_button_of(expand_icon, &ctx.tokens)
                    .on_press(DockMessage::ToggleCollapse(position))
                    .boxed()
            } else {
                button(expand_icon)
                    .padding(SP.s)
                    .width(RAIL_CANVAS_W)
                    .style(button::text)
                    .on_press(DockMessage::ToggleCollapse(position))
                    .boxed()
            });

            for (i, panel) in region.panels.iter().enumerate() {
                let label = panel.label().to_string();
                let is_active = i == region.active;
                let text_c = if is_active {
                    styles::ti(ctx.tokens.text)
                } else {
                    styles::ti(ctx.tokens.text_secondary)
                };

                // Canvas height = text pixel width + 1× font size padding.
                // Graphite measures with the real font; legacy keeps the estimate.
                let label_w = if graphite {
                    measure::text_width(&label, RAIL_FONT_SIZE)
                } else {
                    estimate_text_width(&label, RAIL_FONT_SIZE)
                };
                let tab_h = label_w + RAIL_FONT_SIZE;
                let tab_style: TabStyleFn = if graphite {
                    Box::new(graphite::rail_tab(&ctx.tokens, position, is_active))
                } else {
                    Box::new(styles::rail_tab(&ctx.tokens, is_active))
                };

                rail = rail.push(
                    button(
                        canvas(RotatedLabel {
                            label,
                            color: text_c,
                        })
                        .width(if graphite {
                            Length::Fill
                        } else {
                            Length::Fixed(RAIL_CANVAS_W)
                        })
                        .height(tab_h),
                    )
                    .padding(0)
                    .width(if graphite {
                        Length::Fill
                    } else {
                        Length::Shrink
                    })
                    .on_press(DockMessage::SelectTab(position, i))
                    .style(tab_style)
                    .boxed(),
                );
            }

            if graphite {
                return container(rail)
                    .width(graphite::RAIL_W)
                    .height(Length::Fill)
                    .padding([SP.s, SP.xs])
                    .style(graphite::rail(&ctx.tokens, position))
                    .boxed();
            }
            container(rail)
                .height(Length::Fill)
                .padding([3.0, SP.xs])
                .style(styles::collapsed_rail(&ctx.tokens))
                .boxed()
        } else {
            // Bottom panel collapsed: horizontal thin strip with tab labels
            let mut rail = row![].spacing(SP.xs);

            for (i, panel) in region.panels.iter().enumerate() {
                let label = panel.label();
                let is_active = i == region.active;
                let text_c = if is_active {
                    styles::ti(ctx.tokens.text)
                } else {
                    styles::ti(ctx.tokens.text_secondary)
                };

                let tab_style: TabStyleFn = if graphite {
                    Box::new(graphite::rail_tab(&ctx.tokens, position, is_active))
                } else {
                    Box::new(button::text)
                };
                rail = rail.push(
                    button(text(label).size(TS.caption).color(text_c))
                        .padding([SP.xs, SP.m])
                        .on_press(DockMessage::SelectTab(position, i))
                        .style(tab_style)
                        .boxed(),
                );
            }

            if graphite {
                return container(rail)
                    .width(Length::Fill)
                    .height(graphite::TAB_STRIP_H)
                    .padding([0.0, SP.s])
                    .center_y(graphite::TAB_STRIP_H)
                    .style(graphite::rail(&ctx.tokens, position))
                    .boxed();
            }

            container(rail)
                .width(Length::Fill)
                .height(28)
                .padding([SP.xs, SP.s])
                .style(styles::collapsed_rail(&ctx.tokens))
                .boxed()
        }
    }

    /// Render a single floating panel as an overlay element.
    pub(crate) fn view_floating_panel<'a>(
        &'a self,
        idx: usize,
        ctx: &'a panels::PanelContext,
        library: &'a crate::library::LibraryState,
        grid: SheetGrid,
    ) -> Option<Element<'a, DockMessage>> {
        let fp = self.floating.get(idx)?;
        let kind = fp.kind;
        let label = kind.label();

        // Title bar with drag handle + close button
        let title_bar_content = container(
            row![
                container(text(label).size(TS.body).color(styles::ti(ctx.tokens.text)))
                    .padding([SP.s, SP.m])
                    .width(Length::Fill),
                button(svg_icon(icons::icon_close(ctx.theme_id)))
                    .padding([SP.s, SP.s])
                    .style(button::text)
                    .on_press(DockMessage::DockFloating(idx)),
            ]
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center),
        )
        .width(fp.width)
        .style(styles::floating_title_bar(&ctx.tokens));

        let title_bar = mouse_area(title_bar_content)
            .on_press(DockMessage::StartDragFloating(idx))
            .on_release(DockMessage::FloatingDragEnd(idx));

        let content: Element<'a, DockMessage> = match kind {
            PanelKind::Library => crate::library::panel::view(library, &ctx.tokens)
                .map(DockMessage::Library)
                .boxed(),
            PanelKind::Components => {
                panels::components_panel::view(library, ctx, &ctx.tokens, grid)
                    .map(DockMessage::Library)
                    .boxed()
            }
            _ => panels::view_panel(kind, ctx)
                .map(DockMessage::Panel)
                .boxed(),
        };

        if styles::is_graphite(&ctx.tokens) {
            // One radius-10 card: title bar, body, themed shadow.
            let bar = container(
                row![
                    container(
                        text(label)
                            .size(graphite::TITLE_SIZE)
                            .font(graphite::title_font())
                            .color(styles::ti(ctx.tokens.text)),
                    )
                    .padding([0, 10])
                    .width(Length::Fill),
                    icon_button(icons::icon_close(ctx.theme_id), &ctx.tokens)
                        .on_press(DockMessage::DockFloating(idx))
                        .boxed(),
                ]
                .align_y(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .center_y(graphite::TITLE_H)
            .padding([0.0, SP.s])
            .style(graphite::title_bar(&ctx.tokens));
            let bar = mouse_area(bar)
                .on_press(DockMessage::StartDragFloating(idx))
                .on_release(DockMessage::FloatingDragEnd(idx));
            let card = container(column![
                bar,
                container(iced::widget::scrollable(content).width(Length::Fill))
                    .width(Length::Fill)
                    .height(Length::Fill),
            ])
            .width(fp.width)
            .height(fp.height + graphite::TITLE_H)
            .clip(true)
            .style(graphite::floating_card(&ctx.tokens));
            return Some(card.boxed());
        }

        let panel_widget = container(
            column![
                title_bar,
                container(iced::widget::scrollable(content).width(Length::Fill))
                    .width(fp.width)
                    .height(fp.height)
                    .style(styles::floating_panel_body(&ctx.tokens)),
            ]
            .spacing(0),
        )
        .style(styles::floating_panel_shadow(&ctx.tokens));

        Some(panel_widget.boxed())
    }
}

/// Font size for collapsed rail labels — every other dimension derives from this.
const RAIL_FONT_SIZE: f32 = TS.label;
/// Canvas width = text line-height after 90° rotation ≈ 1.5 × font size.
const RAIL_CANVAS_W: f32 = RAIL_FONT_SIZE * 1.5;

/// Estimate the rendered pixel width of a string at `font_size`.
///
/// Ratios are calibrated for Segoe UI (Windows default / Iced default).
/// Grouped by measured glyph-width bands so every panel label gets
/// near-identical visual padding after center-alignment.
fn estimate_text_width(s: &str, font_size: f32) -> f32 {
    s.chars()
        .map(|c| {
            font_size
                * match c {
                    // ── narrowest glyphs ──
                    'i' | 'l' | '|' | '!' | '.' | ',' | ':' | ';' | '\'' => 0.28,
                    'I' | 'j' => 0.30,
                    'f' => 0.33,
                    'r' | 't' => 0.36,
                    ' ' => 0.28,
                    // ── medium-narrow ──
                    'c' | 's' | 'z' => 0.50,
                    'a' | 'e' | 'g' | 'k' | 'v' | 'x' | 'y' => 0.54,
                    // ── medium-wide ──
                    'b' | 'd' | 'h' | 'n' | 'o' | 'p' | 'q' | 'u' => 0.58,
                    // ── widest lowercase ──
                    'm' => 0.86,
                    'w' => 0.80,
                    // ── capitals ──
                    'M' | 'W' => 0.86,
                    'A'..='Z' => 0.62,
                    // ── digits & fallback ──
                    '0'..='9' => 0.58,
                    _ => 0.55,
                }
        })
        .sum()
}

/// Canvas program that draws rotated text (90° CW) for collapsed panel tabs.
struct RotatedLabel {
    label: String,
    color: Color,
}

impl canvas::Program<DockMessage> for RotatedLabel {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let cx = bounds.width / 2.0;
        let cy = bounds.height / 2.0;
        frame.translate(iced::Vector::new(cx, cy));
        frame.rotate(std::f32::consts::FRAC_PI_2); // 90° CW
        // After rotation, origin is at canvas center. Use center alignment
        // so the text is perfectly centered regardless of label length.
        frame.fill_text(canvas::Text {
            content: self.label.clone(),
            position: iced::Point::ORIGIN,
            color: self.color,
            size: iced::Pixels(RAIL_FONT_SIZE),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            ..canvas::Text::default()
        });
        vec![frame.into_geometry()]
    }
}
