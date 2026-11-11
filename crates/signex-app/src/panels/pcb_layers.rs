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
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, text, tooltip};

use super::form_look::{FormLook, ghost_button};
use super::graphite::{active_fill, wash_fn};
use super::{PanelContext, PanelMsg, empty_state_card};
use crate::app::PcbViewMsg;
use crate::pcb_canvas::layer_style;
use crate::styles::{Radii, Spacing, TypeScale, ti, tokens::resolve};

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
    /// The Multi-Layer row (vias and through-hole pads) is hidden.
    pub multi_layer_hidden: bool,
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
            multi_layer_hidden: false,
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
            multi_layer_hidden: !view.multi_layer_visible(),
            board_open: true,
            i18n,
        }
    }

    pub fn is_hidden(&self, layer: PcbLayer) -> bool {
        self.hidden.contains(&layer)
    }

    /// Something is hidden, so Show all layers has work to do.
    pub fn has_hidden(&self) -> bool {
        !self.hidden.is_empty() || self.multi_layer_hidden
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
    if !panel.board_open {
        return empty_state_card(
            tokens,
            t!(i18n, ids::PCB_LAYERS_TITLE),
            t!(i18n, ids::PCB_LAYERS_EMPTY),
            None,
        );
    }
    let title = text(t!(i18n, ids::PCB_LAYERS_TITLE))
        .size(TS.label)
        .color(ti(tokens.text));
    let mut body = Column::<Element<'_, PanelMsg>>::new()
        .spacing(SP.xs)
        .width(Length::Fill);
    body = body.push(container(title).padding([SP.s, SP.m]).boxed());
    body = body.push(
        container(
            button(text(t!(i18n, ids::PCB_LAYERS_SHOW_ALL)).size(TS.body))
                .padding([SP.xs, SP.m])
                .style(show_all_button)
                .on_press_maybe(
                    panel
                        .has_hidden()
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
            ctx.collapsible(group_key(group), &header, &ctx.collapsed_sections, || {
                let col = rows.iter().fold(
                    Column::<Element<'_, PanelMsg>>::new().spacing(0),
                    |col, layer| col.push(layer_row(panel, layer, ctx)),
                );
                // Vias and through-hole pads list under the copper rows.
                if group == LayerGroup::Copper {
                    col.push(multi_layer_row(panel, ctx))
                } else {
                    col
                }
            })
            .boxed(),
        );
    }
    body.boxed()
}

/// What one row of the panel shows and does.
struct RowLook {
    label: Label<'static>,
    swatch: Color,
    hidden: bool,
    is_active: bool,
    /// What its eye sends.
    toggled: PcbViewMsg,
    /// What a click on its name sends: a copper row makes its layer active.
    picked: Option<PcbViewMsg>,
}

fn layer_row<'a>(
    panel: &PcbLayersPanel,
    layer: &LayerRow,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let look = RowLook {
        label: layer_style::row_label(&panel.i18n, layer),
        swatch: layer_style::layer_colour(layer.layer, &ctx.tokens),
        hidden: panel.is_hidden(layer.layer),
        is_active: panel.active.is_some() && layer.layer.copper() == panel.active,
        toggled: PcbViewMsg::LayerVisibilityToggled(layer.layer),
        picked: layer.layer.copper().map(PcbViewMsg::ActiveLayerPicked),
    };
    row_view(panel, look, ctx)
}

/// The Multi-Layer row: vias and through-hole pads, never the active layer.
fn multi_layer_row<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let look = RowLook {
        label: t!(panel.i18n, ids::PCB_LAYER_MULTI_LAYER).into(),
        swatch: layer_style::multi_layer_colour(),
        hidden: panel.multi_layer_hidden,
        is_active: false,
        toggled: PcbViewMsg::MultiLayerVisibilityToggled,
        picked: None,
    };
    row_view(panel, look, ctx)
}

fn row_view<'a>(
    panel: &PcbLayersPanel,
    look: RowLook,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let RowLook {
        label,
        swatch: swatch_c,
        hidden,
        is_active,
        toggled,
        picked,
    } = look;
    let text_c = if hidden {
        resolve(tokens).ink3
    } else {
        ti(tokens.text)
    };
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
                ..Default::default()
            }),
    )
    .padding([SP.xs, SP.s])
    .style(row_button(tokens))
    .on_press(PanelMsg::PcbLayers(toggled));
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
                width: 1.0,
                color: Color { a: 0.5, ..swatch_c },
                radius: 2.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let label = text(label).size(TS.body).color(text_c);
    let active_c = active_fill(tokens);
    let content = row![swatch, label]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
    let name: Element<'a, PanelMsg> = match picked {
        Some(picked) => button(content)
            .padding([SP.xs, SP.s])
            .width(Length::Fill)
            .style(row_button(tokens))
            .on_press(PanelMsg::PcbLayers(picked))
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
        background: is_active.then_some(Background::Color(active_c)),
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// A transparent row button with the Graphite hover wash.
fn row_button(tokens: &ThemeTokens) -> impl Fn(&Theme, Status) -> ButtonStyle + 'static {
    let wash = wash_fn(tokens);
    move |_: &Theme, status| ButtonStyle {
        background: matches!(status, Status::Hovered).then_some(Background::Color(wash(0.06))),
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..ButtonStyle::default()
    }
}

/// "Show all layers": the Graphite ghost button, muted while nothing is hidden.
fn show_all_button(theme: &Theme, status: Status) -> ButtonStyle {
    let mut style = ghost_button(theme, status);
    if matches!(status, Status::Disabled) {
        style.text_color = FormLook::of(theme).ink4;
    }
    style
}

#[cfg(test)]
#[path = "pcb_layers_tests.rs"]
mod tests;
