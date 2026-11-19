//! The colour card of a layer (#2363, #103): a click on the layer's dot
//! opens it under the row, indented to the name column. It lists the board
//! palette, takes a hex colour that recolours the layer as it is typed, and
//! offers the palette colour again while the layer has its own.

use iced::widget::{Column, Row, Space, container, row};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{ids, t};
use signex_types::pcb_layer::PcbLayer;
use signex_widgets::{button, text, text_input};

use super::metrics::{
    CARD, CARD_INDENT, CARD_PAD, EYE, HEX_FIELD, ICON, ROW_GAP, SWATCH, SWATCH_COLUMNS, SWATCH_GAP,
    SWATCH_RADIUS,
};
use super::rows::{dot_face, medium};
use super::{LayersPanelMsg, LayersPanelUi, PcbLayersPanel, SP, TS, row_button};
use crate::app::PcbViewMsg;
use crate::panels::form_look;
use crate::panels::{PanelContext, PanelMsg};
use crate::pcb_canvas::layer_style;
use crate::styles::tokens::resolve;
use crate::styles::{Radii, board_canvas_colors, floating_shadow, ti};

/// The board palette in its swatch order: the board layer colours, then
/// the top-side technical colours, then the Multi-Layer silver.
pub(super) fn palette() -> [Color; 24] {
    let board = board_canvas_colors().board_layer_colors();
    let cycle = board.layer_cycle;
    let tech = board.technical;
    [
        board.top_copper,
        board.bottom_copper,
        cycle[0],
        cycle[1],
        cycle[2],
        cycle[3],
        cycle[4],
        cycle[5],
        board.margin,
        board.silkscreen,
        board.solder_mask,
        board.paste,
        tech.adhesive.top,
        tech.fab.top,
        tech.assembly.top,
        tech.courtyard.top,
        tech.body_3d.top,
        tech.fiducial.top,
        tech.designator.top,
        tech.dimensions,
        tech.notes,
        tech.drill_drawing,
        tech.eco_1,
        board.multi_layer,
    ]
    .map(ti)
}

/// The red, green and blue of `colour`.
fn rgb(colour: Color) -> [u8; 3] {
    let [r, g, b, _] = colour.into_rgba8();
    [r, g, b]
}

/// The card of `layer` while its dot is open.
pub(super) fn view<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    layer: PcbLayer,
    ctx: &PanelContext,
) -> Option<Element<'a, PanelMsg>> {
    if ui.picker != Some(layer) {
        return None;
    }
    let i18n = &panel.i18n;
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let current = panel.colour(layer);
    let caption = |id| text(t!(i18n, id)).size(TS.caption).color(r.ink4).boxed();
    let gap = |height: f32| Space::new().height(height).boxed();
    let mut body = Column::<Element<'a, PanelMsg>>::new()
        .width(Length::Fill)
        .push(header(panel, layer, current, ctx))
        .push(gap(SP.m))
        .push(caption(ids::PCB_LAYERS_PALETTE))
        .push(gap(SP.s))
        .push(swatches(layer, current, ctx))
        .push(gap(SP.m))
        .push(caption(ids::PCB_LAYERS_COLOUR))
        .push(gap(SP.s))
        .push(hex_row(panel, ui, current, ctx));
    if panel.paint.colour(layer).is_some() {
        body = body.push(gap(SP.s)).push(palette_again(panel, layer, ctx));
    }
    let (fill, line, shadow) = (r.raised, ti(tokens.border), floating_shadow(tokens));
    let card = container(body)
        .width(CARD)
        .padding(CARD_PAD)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            shadow,
            ..container::Style::default()
        });
    Some(
        container(card)
            .padding(Padding {
                top: SP.xs,
                bottom: SP.s,
                left: CARD_INDENT,
                right: SP.m,
            })
            .boxed(),
    )
}

/// The layer's dot and name, and the close button.
fn header<'a>(
    panel: &PcbLayersPanel,
    layer: PcbLayer,
    current: Color,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let ink3 = resolve(&ctx.tokens).ink3;
    let close = iced::widget::svg(crate::icons::icon_close(ctx.theme_id))
        .width(ICON)
        .height(ICON)
        .style(move |_: &Theme, _| iced::widget::svg::Style {
            color: Some(ink3),
            ..Default::default()
        });
    let close = button(container(close).center_x(EYE).center_y(EYE))
        .padding(0)
        .style(row_button(&ctx.tokens))
        .on_press(PanelMsg::LayersPanel(LayersPanelMsg::ColourChipToggled(
            layer,
        )));
    row![
        dot_face(Some(current), false, ctx),
        text(layer_style::layer_name(&panel.i18n, layer))
            .size(TS.body)
            .font(medium())
            .color(ti(ctx.tokens.text))
            .width(Length::Fill),
        close,
    ]
    .spacing(ROW_GAP)
    .align_y(Alignment::Center)
    .boxed()
}

/// The palette swatches, eight to a line; the layer's colour is ringed in
/// the accent. A click gives the layer that colour and closes the card.
fn swatches<'a>(layer: PcbLayer, current: Color, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let (accent, ring) = (ti(ctx.tokens.accent), r.line_strong);
    let selected = rgb(current);
    let swatch = |colour: Color| {
        let on = rgb(colour) == selected;
        button(Space::new())
            .width(SWATCH)
            .height(SWATCH)
            .padding(0)
            .style(move |_: &Theme, status| {
                let hovered = matches!(status, iced::widget::button::Status::Hovered);
                let (width, edge) = match (on, hovered) {
                    (true, _) => (2.0, accent),
                    (false, true) => (2.0, ring),
                    (false, false) => (1.0, ring),
                };
                iced::widget::button::Style {
                    background: Some(Background::Color(colour)),
                    border: Border {
                        width,
                        color: edge,
                        radius: SWATCH_RADIUS.into(),
                        ..Border::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            })
            .on_press(PanelMsg::PcbLayers(PcbViewMsg::LayerColourSet {
                layer,
                colour: Some(rgb(colour)),
            }))
            .boxed()
    };
    palette()
        .chunks(SWATCH_COLUMNS)
        .fold(
            Column::<Element<'a, PanelMsg>>::new().spacing(SWATCH_GAP),
            |grid, line| {
                let line = line.iter().fold(
                    Row::<Element<'a, PanelMsg>>::new().spacing(SWATCH_GAP),
                    |cells, colour| cells.push(swatch(*colour)),
                );
                grid.push(line.boxed())
            },
        )
        .boxed()
}

/// The hex field and a swatch of what it holds (the layer's colour until
/// it reads as a colour).
fn hex_row<'a>(
    panel: &PcbLayersPanel,
    ui: &LayersPanelUi,
    current: Color,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let typed = ui
        .hex_colour()
        .map_or(current, |[r, g, b]| Color::from_rgb8(r, g, b));
    let line = resolve(&ctx.tokens).line_strong;
    let field = text_input(t!(panel.i18n, ids::PCB_LAYERS_HEX), ui.hex.clone())
        .on_input(|hex| PanelMsg::LayersPanel(LayersPanelMsg::HexEdited(hex)))
        .font(Font::MONOSPACE)
        .size(TS.body)
        .padding(SP.s)
        .style(form_look::text_input_style)
        .width(Length::Fill);
    let preview = container(Space::new())
        .width(HEX_FIELD)
        .height(HEX_FIELD)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(typed)),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    row![field, preview]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed()
}

/// Palette colour: the layer drops its own colour for the board palette's
/// (closes the card, as a swatch does).
fn palette_again<'a>(
    panel: &PcbLayersPanel,
    layer: PcbLayer,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let ink = resolve(&ctx.tokens).accent_text;
    button(
        text(t!(panel.i18n, ids::PCB_LAYERS_COLOUR_PALETTE))
            .size(TS.caption)
            .color(ink),
    )
    .padding([SP.s, SP.m])
    .style(row_button(&ctx.tokens))
    .on_press(PanelMsg::PcbLayers(PcbViewMsg::LayerColourSet {
        layer,
        colour: None,
    }))
    .boxed()
}
