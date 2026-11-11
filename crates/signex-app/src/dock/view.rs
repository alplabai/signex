//! Dock rendering: region tab strips, collapsed rails, and floating panels.

use iced::widget::{Column, Space, Widget, canvas, column, container, mouse_area, row, svg};
use iced::{Color, Element, Length, Rectangle, Renderer, Theme};
use signex_i18n::{ids, t};
use signex_widgets::{button, raw, text};

use super::graphite;
use super::measure;
use super::tab_fit;
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
            return container(text(raw(""))).width(0).boxed();
        }

        if region.collapsed {
            return self.view_rail(position, region, ctx);
        }

        // ── Altium-style flat tabs with accent underline ──
        // Every tab is shown: when the natural widths do not fit the strip,
        // the wider tabs shrink and their labels end in an ellipsis
        // (`tab_fit::fit_widths`), so no tab is cut at the strip edge.
        let tab_strip = iced::widget::responsive(move |size| {
            let font = crate::fonts::iced_font_for_family(&ctx.ui_font_name);
            let natural: Vec<f32> = region
                .panels
                .iter()
                .map(|panel| tab_natural_width(*panel, ctx, font))
                .collect();
            let widths = tab_fit::fit_widths(&natural, size.width);
            let mut tab_row = row![]
                .spacing(0.0)
                .align_y(iced::Alignment::End)
                .height(Length::Fill);

            // Last visible panel index drives the `is_last` flag so
            // adjacent tabs share their L/R borders and the rightmost
            // tab actually closes off with a right edge.
            let last_panel_idx = region.panels.len().saturating_sub(1);
            let (tab_size, tab_pad) = (TS.body, graphite::TAB_PAD);
            for (i, panel) in region.panels.iter().enumerate() {
                let squeezed = (widths[i] < natural[i]).then_some(widths[i]);
                let label = t!(ctx.i18n, panel.message());
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
                let accent = styles::ti(ctx.tokens.accent);
                let fill = if is_dragging_this {
                    iced::Color { a: 0.22, ..accent }
                } else {
                    // The active tab merges into the panel fill.
                    let panel = styles::ti(ctx.tokens.panel_bg);
                    if is_active {
                        panel
                    } else if is_hovered {
                        styles::tokens::hover_overlay(&ctx.tokens)
                    } else {
                        iced::Color { a: 0.0, ..panel }
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
                let mut label_text = text(label)
                    .size(tab_size)
                    .color(text_c)
                    .wrapping(iced::widget::text::Wrapping::None);
                if squeezed.is_some() {
                    label_text = label_text
                        .width(Length::Fill)
                        .ellipsis(iced::widget::text::Ellipsis::End);
                }
                let tab_content: Element<'_, DockMessage> = match graphite::tab_badge(*panel, ctx) {
                    Some((count, tone)) => row![
                        label_text.boxed(),
                        container(
                            text(t!(ctx.i18n, ids::PANELS_DOCK_TAB_COUNT, "count" => count))
                                .size(TS.caption)
                                .boxed(),
                        )
                        .padding([0.0, SP.s])
                        .style(styles::pill(&ctx.tokens, tone))
                        .boxed(),
                    ]
                    .spacing(SP.s)
                    .align_y(iced::Alignment::Center)
                    .boxed(),
                    None => label_text.boxed(),
                };
                let mut inner = container(tab_content)
                    .padding([SP.s, tab_pad])
                    .height(Length::Fill)
                    .align_y(iced::Alignment::Center);
                if let Some(width) = squeezed {
                    inner = inner.width(width).clip(true);
                }
                let tab = mouse_area(
                    signex_widgets::tab_pill::TabPill::new(inner.boxed(), pill_style).square(true),
                )
                .on_enter(DockMessage::TabHoverEnter(position, i))
                .on_exit(DockMessage::TabHoverExit(position, i))
                .on_press(DockMessage::TabDragStart(position, i))
                .on_release(DockMessage::TabClick(position, i))
                .on_right_press(DockMessage::TabContextMenuRequested(position, i));

                tab_row = tab_row.push(tab.boxed());
            }
            tab_row.boxed()
        })
        .boxed();

        // Collapse button (SVG icon)
        let tid = ctx.theme_id;
        let collapse_icon = match position {
            PanelPosition::Left => svg_icon(icons::icon_collapse_left(tid)),
            PanelPosition::Right => svg_icon(icons::icon_collapse_right(tid)),
            PanelPosition::Bottom => svg_icon(icons::icon_collapse_down(tid)),
        };
        let close_btn = icon_button(icons::icon_close(tid), &ctx.tokens)
            .on_press(DockMessage::ClosePanel(position, region.active))
            .boxed();
        let collapse_btn = icon_button_of(collapse_icon, &ctx.tokens)
            .on_press(DockMessage::ToggleCollapse(position))
            .boxed();

        // Title of the currently active panel — drawn in the top bar
        // so the user can see the panel name without scanning the tab
        // row.
        let active_title: signex_widgets::Label<'_> = match region.panels.get(region.active) {
            Some(p) => t!(ctx.i18n, p.message()).into(),
            None => signex_i18n::UserText::new(String::new()).into(),
        };
        let title_text = text(active_title)
            .size(graphite::TITLE_SIZE)
            .font(graphite::title_font())
            .color(styles::ti(ctx.tokens.text));
        let title_bar = row![
            container(title_text).padding([0, 10]),
            Space::new().width(Length::Fill),
            collapse_btn,
            close_btn,
        ]
        .spacing(0)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        // The tab strip (built above) holds just the tab buttons. Collapse /
        // close moved up to the title bar so they're always visible
        // alongside the active panel name.

        // Panel content
        let content: Element<'_, DockMessage> =
            if let Some(panel) = region.panels.get(region.active) {
                match *panel {
                    PanelKind::Library => {
                        // v0.9 Library panel — content lives in the
                        // library subsystem, not in `panels::view_panel`.
                        // Wrap the LibraryMessage in DockMessage::Library
                        // so the dispatcher can route it back out.
                        crate::library::panel::view(library, &ctx.tokens, &ctx.i18n)
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
                text(raw("")).boxed()
            };

        // Title + collapse/close on top, tabs on the bottom. Content grows
        // between them.
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
        body.push(
            container(tab_strip)
                .width(Length::Fill)
                .height(graphite::TAB_STRIP_H)
                .clip(true)
                .style(graphite::tab_strip(&ctx.tokens))
                .boxed(),
        )
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

        if is_vertical {
            let mut rail: Column<Element<'a, DockMessage>> = Column::new().spacing(SP.xs);

            // Expand button with SVG arrow
            let tid = ctx.theme_id;
            let expand_icon = match position {
                PanelPosition::Left => svg_icon(icons::icon_expand_left(tid)),
                PanelPosition::Right => svg_icon(icons::icon_expand_right(tid)),
                _ => svg_icon(icons::icon_expand_left(tid)),
            };
            rail = rail.push(
                icon_button_of(expand_icon, &ctx.tokens)
                    .on_press(DockMessage::ToggleCollapse(position))
                    .boxed(),
            );

            for (i, panel) in region.panels.iter().enumerate() {
                let label = panel.label().to_string();
                let is_active = i == region.active;
                let text_c = if is_active {
                    styles::ti(ctx.tokens.text)
                } else {
                    styles::ti(ctx.tokens.text_secondary)
                };

                // Canvas height = text pixel width (measured with the real
                // font) + 1× font size padding.
                let label_w = measure::text_width(&label, RAIL_FONT_SIZE);
                let tab_h = label_w + RAIL_FONT_SIZE;
                let tab_style: TabStyleFn =
                    Box::new(graphite::rail_tab(&ctx.tokens, position, is_active));

                rail = rail.push(
                    button(
                        canvas(RotatedLabel {
                            label,
                            color: text_c,
                        })
                        .width(Length::Fill)
                        .height(tab_h),
                    )
                    .padding(0)
                    .width(Length::Fill)
                    .on_press(DockMessage::SelectTab(position, i))
                    .style(tab_style)
                    .boxed(),
                );
            }

            container(rail)
                .width(graphite::RAIL_W)
                .height(Length::Fill)
                .padding([SP.s, SP.xs])
                .style(graphite::rail(&ctx.tokens, position))
                .boxed()
        } else {
            // Bottom panel collapsed: horizontal thin strip with tab labels
            let mut rail = row![].spacing(SP.xs);

            for (i, panel) in region.panels.iter().enumerate() {
                let label = t!(ctx.i18n, panel.message());
                let is_active = i == region.active;
                let text_c = if is_active {
                    styles::ti(ctx.tokens.text)
                } else {
                    styles::ti(ctx.tokens.text_secondary)
                };

                let tab_style: TabStyleFn =
                    Box::new(graphite::rail_tab(&ctx.tokens, position, is_active));
                rail = rail.push(
                    button(text(label).size(TS.caption).color(text_c))
                        .padding([SP.xs, SP.m])
                        .on_press(DockMessage::SelectTab(position, i))
                        .style(tab_style)
                        .boxed(),
                );
            }

            container(rail)
                .width(Length::Fill)
                .height(graphite::TAB_STRIP_H)
                .padding([0.0, SP.s])
                .center_y(graphite::TAB_STRIP_H)
                .style(graphite::rail(&ctx.tokens, position))
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
        let label = t!(ctx.i18n, kind.message());

        let content: Element<'a, DockMessage> = match kind {
            PanelKind::Library => crate::library::panel::view(library, &ctx.tokens, &ctx.i18n)
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
        Some(card.boxed())
    }
}

/// Width a dock tab takes when nothing squeezes it: label, count chip and
/// side padding, measured in the UI font. One `SP.xs` of slack covers the
/// shaping difference between this measurement and the drawn text.
fn tab_natural_width(panel: PanelKind, ctx: &panels::PanelContext, font: iced::Font) -> f32 {
    let label = t!(ctx.i18n, panel.message()).to_string();
    let mut width = measure::text_width_in(&label, TS.body, font) + 2.0 * graphite::TAB_PAD;
    if let Some((count, _)) = graphite::tab_badge(panel, ctx) {
        let chip = t!(ctx.i18n, ids::PANELS_DOCK_TAB_COUNT, "count" => count).to_string();
        width += SP.s + measure::text_width_in(&chip, TS.caption, font) + 2.0 * SP.s;
    }
    width + SP.xs
}

/// Font size for collapsed rail labels — every other dimension derives from this.
const RAIL_FONT_SIZE: f32 = TS.label;

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
