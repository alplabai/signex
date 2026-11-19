//! Component Layer Pairs (#2363, m103): Top | Layer | Bottom, each side
//! with its own eye and dot, the right side mirrored. A pair the board has
//! no layer for (Coating, Component Center, Tenting, Value) is not listed;
//! it shows once a layer exists for it.

use iced::widget::{container, row};
use iced::{Alignment, Element, Length, Widget};
use signex_i18n::{Localized, ids, t};
use signex_renderer::pcb::LayerPair;
use signex_types::pcb_layer::PcbLayer;
use signex_widgets::text;

use super::super::metrics::{PAIR_SIDE, SIDE_GAP};
use super::super::rows::{chip, eye, framed, with_editor};
use super::super::{LayersPanelUi, PcbLayersPanel, SP, TS, colour_card};
use crate::app::PcbViewMsg;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// The token a pair row's name is looked up by.
fn pair_token(pair: LayerPair) -> &'static str {
    match pair {
        LayerPair::Body3d => "body_3d",
        LayerPair::Assembly => "assembly",
        LayerPair::Coating => "coating",
        LayerPair::ComponentCenter => "component_center",
        LayerPair::ComponentOutline => "component_outline",
        LayerPair::Courtyard => "courtyard",
        LayerPair::Designator => "designator",
        LayerPair::Overlay => "overlay",
        LayerPair::Paste => "paste",
        LayerPair::Solder => "solder",
        LayerPair::Tenting => "tenting",
        LayerPair::Value => "value",
        LayerPair::Adhesive => "adhesive",
        LayerPair::Fiducial => "fiducial",
        LayerPair::Keepout => "keepout",
    }
}

/// The pairs the search and Used On list: each row's name and its sides.
pub(super) fn listed<'p>(
    panel: &'p PcbLayersPanel,
    ui: &'p LayersPanelUi,
) -> impl Iterator<Item = (Localized, (PcbLayer, PcbLayer))> + 'p {
    let i18n = &panel.i18n;
    LayerPair::ALL.into_iter().filter_map(move |pair| {
        let (top, bottom) = pair.layers()?;
        let name = t!(i18n, ids::PCB_LAYERS_PAIR, "pair" => pair_token(pair));
        let listed = ui.matches(i18n, &name) && (panel.lists(top) || panel.lists(bottom));
        listed.then_some((name, (top, bottom)))
    })
}

/// The pair rows the search and Used On list, without the table header.
pub(super) fn rows<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    ctx: &PanelContext,
) -> Vec<Element<'a, PanelMsg>> {
    listed(panel, ui)
        .map(|(name, sides)| pair_row(panel, ui, name, sides, ctx))
        .collect()
}

/// One side of a pair: the eye and the dot, mirrored on the right.
fn side<'a>(
    panel: &PcbLayersPanel,
    layer: PcbLayer,
    mirrored: bool,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let hidden = panel.is_hidden(layer);
    let toggled = PanelMsg::PcbLayers(PcbViewMsg::LayerVisibilityToggled(layer));
    let eye = eye(panel, hidden, Some(toggled), ctx);
    let dot = chip(panel, panel.colour(layer), hidden, Some(layer), ctx);
    let (first, second, align) = if mirrored {
        (dot, eye, iced::alignment::Horizontal::Right)
    } else {
        (eye, dot, iced::alignment::Horizontal::Left)
    };
    container(
        row![first, second]
            .spacing(SIDE_GAP)
            .align_y(Alignment::Center),
    )
    .width(PAIR_SIDE)
    .align_x(align)
    .boxed()
}

fn pair_row<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    name: Localized,
    (top, bottom): (PcbLayer, PcbLayer),
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let ink = if panel.is_hidden(top) && panel.is_hidden(bottom) {
        r.ink4
    } else {
        ti(ctx.tokens.text)
    };
    let line = row![
        side(panel, top, false, ctx),
        text(name)
            .size(TS.body)
            .color(ink)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
        side(panel, bottom, true, ctx),
    ];
    let line = with_editor(
        framed(line, false, ctx),
        colour_card::view(panel, ui, top, ctx),
    );
    with_editor(line, colour_card::view(panel, ui, bottom, ctx))
}

/// Top | Layer | Bottom over the pair rows.
pub(super) fn header<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let ink3 = resolve(&ctx.tokens).ink3;
    let i18n = &panel.i18n;
    let head = |id, align| {
        text(t!(i18n, id))
            .size(TS.caption)
            .color(ink3)
            .width(Length::Fill)
            .align_x(align)
    };
    container(
        row![
            container(head(
                ids::PCB_LAYERS_PAIR_TOP,
                iced::alignment::Horizontal::Left
            ))
            .width(PAIR_SIDE),
            head(
                ids::PCB_LAYERS_PAIR_LAYER,
                iced::alignment::Horizontal::Center
            ),
            container(head(
                ids::PCB_LAYERS_PAIR_BOTTOM,
                iced::alignment::Horizontal::Right
            ))
            .width(PAIR_SIDE),
        ]
        .align_y(Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}
