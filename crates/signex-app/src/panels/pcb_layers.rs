//! The Layers panel (#2223): one row per board layer with an eye, a colour
//! swatch and the name; a click on a copper row makes it the active layer.
//! The rows are data (`PcbLayersPanel`); `view` only draws them.

use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::{Column, Space, container, row, svg};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_i18n::{I18n, ids, t};
use signex_renderer::pcb::{DIMMED_ALPHA, LayerGroup, LayerRow, PcbLayerView, layer_rows};
use signex_types::pcb::LayerDef;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_widgets::{button, text, theme_ext, tooltip};

use super::{PanelContext, PanelMsg, collapsible_section};
use crate::app::PcbViewMsg;
use crate::pcb_canvas::layer_style;
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

const EYE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><path d="M1 7c1.6-3 3.6-4.5 6-4.5S11.4 4 13 7c-1.6 3-3.6 4.5-6 4.5S2.6 10 1 7z" fill="none" stroke="currentColor" stroke-width="1.2"/><circle cx="7" cy="7" r="2" fill="currentColor"/></svg>"#;
const EYE_OFF: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><path d="M1 7c1.6-3 3.6-4.5 6-4.5S11.4 4 13 7c-1.6 3-3.6 4.5-6 4.5S2.6 10 1 7z" fill="none" stroke="currentColor" stroke-width="1.2" opacity="0.4"/><path d="M2 12L12 2" stroke="currentColor" stroke-width="1.4"/></svg>"#;

/// The eye icons, built once (the shared icon set will take them over).
fn eye_handle(hidden: bool) -> svg::Handle {
    static OPEN: OnceLock<svg::Handle> = OnceLock::new();
    static SHUT: OnceLock<svg::Handle> = OnceLock::new();
    let (cell, bytes) = if hidden {
        (&SHUT, EYE_OFF)
    } else {
        (&OPEN, EYE)
    };
    cell.get_or_init(|| svg::Handle::from_memory(bytes)).clone()
}

/// What the panel shows of the active board.
#[derive(Clone)]
pub struct PcbLayersPanel {
    pub rows: Vec<LayerRow>,
    pub hidden: HashSet<PcbLayer>,
    /// The active copper layer as drawn; `None` while every copper layer is
    /// hidden or no board is open.
    pub active: Option<CopperLayer>,
    /// A board is open; without one the panel shows its empty state.
    pub board_open: bool,
    pub i18n: Arc<I18n>,
}

impl Default for PcbLayersPanel {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            hidden: HashSet::new(),
            active: None,
            board_open: false,
            i18n: I18n::en_us(),
        }
    }
}

impl PcbLayersPanel {
    pub fn new(layers: &[LayerDef], view: &PcbLayerView, i18n: Arc<I18n>) -> Self {
        let rows = layer_rows(layers);
        let hidden = rows
            .iter()
            .map(|row| row.layer)
            .filter(|layer| !view.is_visible(*layer))
            .collect();
        Self {
            rows,
            hidden,
            active: view.shown_active(),
            board_open: true,
            i18n,
        }
    }

    pub fn is_hidden(&self, layer: PcbLayer) -> bool {
        self.hidden.contains(&layer)
    }
}

fn group_key(group: LayerGroup) -> &'static str {
    match group {
        LayerGroup::Copper => "pcb_layers_copper",
        LayerGroup::Technical => "pcb_layers_technical",
        LayerGroup::Mechanical => "pcb_layers_mechanical",
    }
}

pub(super) fn view(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    let panel = &ctx.pcb_layers;
    let tokens = &ctx.tokens;
    let i18n = &panel.i18n;
    let title = text(t!(i18n, ids::PCB_LAYERS_TITLE))
        .size(TS.title)
        .color(theme_ext::text_primary(tokens));
    if !panel.board_open {
        return Column::<Element<'_, PanelMsg>>::new()
            .push(title.boxed())
            .push(
                text(t!(i18n, ids::PCB_LAYERS_EMPTY))
                    .size(TS.body)
                    .color(theme_ext::text_secondary(tokens))
                    .boxed(),
            )
            .spacing(SP.s)
            .padding(SP.m)
            .boxed();
    }
    let header_c = theme_ext::text_secondary(tokens);
    let border_c = theme_ext::border_color(tokens);
    let mut body = Column::<Element<'_, PanelMsg>>::new()
        .spacing(SP.xs)
        .width(Length::Fill);
    body = body.push(container(title).padding([SP.s, SP.m]).boxed());
    body = body.push(
        container(
            button(text(t!(i18n, ids::PCB_LAYERS_SHOW_ALL)).size(TS.body))
                .padding([SP.xs, SP.m])
                .style(flat_button(ctx))
                .on_press_maybe(
                    (!panel.hidden.is_empty())
                        .then_some(PanelMsg::PcbLayers(PcbViewMsg::AllLayersShown)),
                ),
        )
        .padding([0.0, SP.m])
        .boxed(),
    );
    for (group, header) in [
        (LayerGroup::Copper, ids::PCB_LAYERS_COPPER),
        (LayerGroup::Technical, ids::PCB_LAYERS_TECHNICAL),
        (LayerGroup::Mechanical, ids::PCB_LAYERS_MECHANICAL),
    ] {
        let rows: Vec<&LayerRow> = panel.rows.iter().filter(|r| r.group == group).collect();
        if rows.is_empty() {
            continue;
        }
        let header = t!(i18n, header).into_string();
        body = body.push(
            collapsible_section(
                group_key(group),
                &header,
                &ctx.collapsed_sections,
                header_c,
                border_c,
                || {
                    rows.iter().fold(
                        Column::<Element<'_, PanelMsg>>::new().spacing(0),
                        |col, layer| col.push(layer_row(panel, layer, ctx)),
                    )
                },
            )
            .boxed(),
        );
    }
    body.boxed()
}

fn layer_row<'a>(
    panel: &PcbLayersPanel,
    layer: &LayerRow,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let hidden = panel.is_hidden(layer.layer);
    let is_active = panel.active.is_some() && layer.layer.copper() == panel.active;
    let text_c = if hidden {
        theme_ext::text_secondary(tokens)
    } else {
        theme_ext::text_primary(tokens)
    };
    let swatch_c = layer_style::layer_colour(layer.layer, tokens);
    let i18n = &panel.i18n;
    let eye_tip = if hidden {
        t!(i18n, ids::PCB_LAYERS_SHOW)
    } else {
        t!(i18n, ids::PCB_LAYERS_HIDE)
    };
    let eye = button(
        svg(eye_handle(hidden))
            .width(14)
            .height(14)
            .style(move |_: &Theme, _| svg::Style {
                color: Some(text_c),
            }),
    )
    .padding([SP.xs, SP.s])
    .style(flat_button(ctx))
    .on_press(PanelMsg::PcbLayers(PcbViewMsg::LayerVisibilityToggled(
        layer.layer,
    )));
    let eye = tooltip(
        eye,
        text(eye_tip).size(TS.body),
        iced::widget::tooltip::Position::Bottom,
    );
    let swatch = container(Space::new())
        .width(12)
        .height(12)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(if hidden {
                Color {
                    a: DIMMED_ALPHA,
                    ..swatch_c
                }
            } else {
                swatch_c
            })),
            border: Border {
                radius: 2.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let label = text(layer_style::row_label(i18n, layer))
        .size(TS.body)
        .color(text_c);
    let selection_c = theme_ext::selection_color(tokens);
    let content = row![swatch, label]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
    let name: Element<'a, PanelMsg> = match layer.layer.copper() {
        Some(copper) => button(content)
            .padding([SP.xs, SP.s])
            .width(Length::Fill)
            .style(flat_button(ctx))
            .on_press(PanelMsg::PcbLayers(PcbViewMsg::ActiveLayerPicked(copper)))
            .boxed(),
        None => container(content)
            .padding([SP.xs, SP.s])
            .width(Length::Fill)
            .boxed(),
    };
    container(
        row![eye, name]
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding([0.0, SP.s])
    .style(move |_: &Theme| container::Style {
        background: is_active.then_some(Background::Color(selection_c)),
        ..container::Style::default()
    })
    .boxed()
}

/// A transparent button that tints on hover, from the theme tokens.
fn flat_button(ctx: &PanelContext) -> impl Fn(&Theme, Status) -> ButtonStyle + 'static {
    let hover = theme_ext::hover_color(&ctx.tokens);
    move |_: &Theme, status| ButtonStyle {
        background: matches!(status, Status::Hovered).then_some(Background::Color(hover)),
        ..ButtonStyle::default()
    }
}

#[cfg(test)]
#[path = "pcb_layers_tests.rs"]
mod tests;
