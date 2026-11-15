//! Layer tabs under the PCB canvas (#2357): the board's own layers as tabs
//! with a colour swatch. A click on a tab makes that layer the active one,
//! the same message a Layers panel row sends, so the tab, the panel row and
//! the status bar pill always name one layer.

use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::{Row, Space, container, row};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_renderer::pcb::PcbLayerView;
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{LayerDef, PcbBoard};
use signex_types::pcb_layer::PcbLayer;
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, text};

use super::*;
use crate::app::{PcbMsg, PcbViewMsg};
use crate::pcb_canvas::layer_style;
use crate::styles::{Radii, ti, tokens::resolve};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Display name and swatch of a board layer: the same name and colour the
/// Layers panel gives it (a mechanical layer the user named keeps that
/// name), so the strip and the status-bar layer chip agree; a layer token
/// the format does not know keeps its own name.
fn describe(i18n: &I18n, def: &LayerDef, tokens: &ThemeTokens) -> (Label<'static>, iced::Color) {
    let Some(layer) = parse_pcb_layer(&def.name) else {
        return (
            signex_i18n::UserText::new(def.name.clone()).into(),
            resolve(tokens).ink4,
        );
    };
    let user_name = matches!(layer, PcbLayer::Mechanical(_)) && !def.display_name.is_empty();
    let label = if user_name {
        signex_i18n::UserText::new(def.display_name.clone()).into()
    } else {
        layer_style::layer_name(i18n, layer).into()
    };
    (label, layer_style::layer_colour(layer, tokens))
}

/// The board the strip belongs to and how its view stands.
pub(super) struct StripBoard<'b> {
    pub board: &'b PcbBoard,
    pub view: &'b PcbLayerView,
    /// The undocked window the strip is drawn in; `None` for the main
    /// window. A tab click acts on that window's board.
    pub window: Option<iced::window::Id>,
}

/// What a click on the tab of `layer` sends.
fn picked(window: Option<iced::window::Id>, layer: PcbLayer) -> Message {
    let event = PcbViewMsg::ActiveLayerPicked(layer);
    match window {
        None => Message::Pcb(PcbMsg::View(event)),
        Some(window) => Message::Pcb(PcbMsg::ViewInWindow { window, event }),
    }
}

/// The strip for the board; `None` when the board declares no layers.
pub(super) fn layer_strip<'a>(
    strip: StripBoard<'_>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Option<Element<'a, Message>> {
    let StripBoard {
        board,
        view,
        window,
    } = strip;
    if board.layers.is_empty() {
        return None;
    }
    let r = resolve(tokens);
    let (muted, line) = (r.ink3, r.line_soft);
    let active = view.shown_active();
    let mut tabs: Row<Element<'a, Message>> = row![
        text(t!(i18n, ids::PCB_LAYERS_TITLE))
            .size(TS.caption)
            .color(muted)
            .boxed(),
        Space::new().width(SP.s).boxed(),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center);
    for def in &board.layers {
        let layer = parse_pcb_layer(&def.name);
        let selected = layer.is_some() && layer == active;
        let hidden = layer.is_some_and(|l| view.is_hidden(l));
        let look = TabLook { selected, hidden };
        let on_press = layer.map(|l| picked(window, l));
        tabs = tabs.push(layer_tab(i18n, def, tokens, look, on_press));
    }
    Some(
        container(tabs)
            .padding([SP.xs, SP.l])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                border: Border {
                    width: 1.0,
                    color: line,
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
    )
}

/// How a tab stands: the active layer, a hidden layer.
#[derive(Clone, Copy)]
struct TabLook {
    selected: bool,
    hidden: bool,
}

/// One tab: swatch and name, filled while it is the active layer, faint
/// while the layer is hidden.
fn layer_tab<'a>(
    i18n: &I18n,
    def: &LayerDef,
    tokens: &ThemeTokens,
    look: TabLook,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let TabLook { selected, hidden } = look;
    let (label, swatch) = describe(i18n, def, tokens);
    let r = resolve(tokens);
    let ink = match (selected, hidden) {
        (true, _) => r.accent_soft_ink,
        (false, true) => r.ink4,
        (false, false) => ti(tokens.text),
    };
    let content = row![
        container(Space::new())
            .width(TS.caption)
            .height(TS.caption)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(swatch)),
                border: Border {
                    radius: 2.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
        text(label).size(TS.body).color(ink).boxed(),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    let (fill, wash) = (r.accent_soft, crate::styles::tokens::hover_overlay(tokens));
    button(content)
        .padding([SP.xs, SP.s])
        .style(move |_: &Theme, status| ButtonStyle {
            background: if selected {
                Some(Background::Color(fill))
            } else {
                matches!(status, Status::Hovered | Status::Pressed)
                    .then_some(Background::Color(wash))
            },
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..ButtonStyle::default()
        })
        .on_press_maybe(on_press)
        .boxed()
}

/// `view` with `strip` (when there is one) stacked under it.
pub(super) fn with_strip<'a>(
    view: Element<'a, Message>,
    strip: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    match strip {
        Some(strip) => iced::widget::column![view, strip].boxed(),
        None => view,
    }
}
