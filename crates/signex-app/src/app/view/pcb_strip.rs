//! Layer tabs under the PCB canvas (#2357): a tab with a colour swatch for
//! every layer the Layers panel shows, in the panel's order, then the
//! Multi-Layer while it is shown. Hiding a layer (its eye, its tab's Hide
//! row) takes its tab away; showing it brings the tab back. The tabs scroll
//! sideways when they do not fit. A click on a tab makes that layer the
//! active one, the same message a Layers panel row sends, so the tab, the
//! panel row and the status bar pill always name one layer. A right-click
//! opens the tab's menu (#2379, #2394). The layer set button at the left
//! end shows the set the strip lists and opens the set menu (#2394); the
//! tabs name their layers in the user's style.

use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{Row, Space, container, mouse_area, row, scrollable};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_renderer::pcb::{
    LayerSetId, PcbLayerView, board_copper_layers, panel_order_layers, used_layers,
};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::PcbLayer;
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, text};

use super::*;
use crate::app::{LayerSetMenuMsg, LayerTabMenuMsg, PcbMsg, PcbViewMsg};
use crate::layer_strip_prefs::LayerStripPrefs;
use crate::pcb_canvas::layer_style;
use crate::styles::{Radii, ti, tokens::resolve};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The number in front of a tab's name: a copper layer's place in the
/// board's stack (1 is the top), a mechanical layer's index.
fn layer_number(layer: PcbLayer, copper: &[signex_types::pcb_layer::CopperLayer]) -> Option<u32> {
    match layer {
        PcbLayer::Copper(c) => copper.iter().position(|&l| l == c).map(|i| i as u32 + 1),
        PcbLayer::Mechanical(m) => Some(u32::from(m.index())),
        PcbLayer::Technical(_) => None,
    }
}

/// The name a tab gives `layer` under the user's settings, with its number
/// in front when they ask for numbers.
fn tab_name(
    i18n: &I18n,
    layer: PcbLayer,
    prefs: &LayerStripPrefs,
    copper: &[signex_types::pcb_layer::CopperLayer],
) -> Localized {
    let name = layer_style::styled_layer_name(i18n, layer, prefs.name_style);
    match layer_number(layer, copper).filter(|_| prefs.show_numbers) {
        Some(number) => t!(
            i18n,
            ids::PCB_LAYER_TAB_NUMBERED,
            "number" => number,
            "name" => name.into_string()
        ),
        None => name,
    }
}

/// The layers the strip gives a tab: every layer the Layers panel shows,
/// in the panel's order, that is not hidden. A layer set lists its own
/// layers alone (#2394); with Used On the panel and the strip list only the
/// layers the board uses (#2363).
pub(crate) fn strip_layers(board: &PcbBoard, view: &PcbLayerView) -> Vec<PcbLayer> {
    let used = view.used_only().then(|| used_layers(board));
    panel_order_layers(&board.layers)
        .into_iter()
        .filter(|&layer| !view.is_hidden(layer) && view.lists_in_strip(layer))
        .filter(|layer| used.as_ref().is_none_or(|used| used.contains(layer)))
        .collect()
}

/// Display name of a board layer: the name the user's tab style gives it;
/// a mechanical layer the user named keeps that name.
fn describe(
    i18n: &I18n,
    board: &PcbBoard,
    layer: PcbLayer,
    names: (&LayerStripPrefs, &[signex_types::pcb_layer::CopperLayer]),
) -> Label<'static> {
    let user_name = board
        .layers
        .iter()
        .filter(|def| matches!(layer, PcbLayer::Mechanical(_)) && !def.display_name.is_empty())
        .find(|def| parse_pcb_layer(&def.name) == Some(layer));
    match user_name {
        Some(def) => signex_i18n::UserText::new(def.display_name.clone()).into(),
        None => tab_name(i18n, layer, names.0, names.1).into(),
    }
}

/// The board the strip belongs to and how its view stands.
pub(super) struct StripBoard<'b> {
    pub board: &'b PcbBoard,
    pub view: &'b PcbLayerView,
    /// The undocked window the strip is drawn in; `None` for the main
    /// window. A tab click acts on that window's board.
    pub window: Option<iced::window::Id>,
    /// The user's tab names (#2394).
    pub prefs: &'b LayerStripPrefs,
}

/// What a click on the tab of `layer` sends.
fn picked(window: Option<iced::window::Id>, layer: PcbLayer) -> Message {
    let event = PcbViewMsg::ActiveLayerPicked(layer);
    match window {
        None => Message::Pcb(PcbMsg::View(event)),
        Some(window) => Message::Pcb(PcbMsg::ViewInWindow { window, event }),
    }
}

/// What a right-click on the tab of `layer` sends: its menu opens (#2379).
fn menu_opened(window: Option<iced::window::Id>, layer: PcbLayer) -> Message {
    Message::Pcb(PcbMsg::LayerTabMenu(LayerTabMenuMsg::Opened {
        window,
        layer,
    }))
}

/// What a tab sends: a click picks its layer, a right-click opens its menu.
/// The Multi-Layer tab sends neither: it is never the active layer, and
/// its eye is the Layers panel's Multi-Layer row.
struct TabMessages {
    press: Option<Message>,
    menu: Option<Message>,
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
        prefs,
    } = strip;
    if board.layers.is_empty() {
        return None;
    }
    let line = resolve(tokens).line_soft;
    let active = view.shown_active();
    let copper = board_copper_layers(&board.layers);
    let paint = view.paint_of(board);
    let mut tabs: Row<Element<'a, Message>> =
        Row::new().spacing(SP.xs).align_y(iced::Alignment::Center);
    for layer in strip_layers(board, view) {
        let messages = TabMessages {
            press: Some(picked(window, layer)),
            menu: Some(menu_opened(window, layer)),
        };
        let label = describe(i18n, board, layer, (prefs, copper.as_slice()));
        // The colour the view gave the layer (Layer colour..., #2363).
        let swatch = layer_style::painted_layer_colour(&paint, layer);
        let selected = active == Some(layer);
        tabs = tabs.push(layer_tab(label, swatch, tokens, selected, messages));
    }
    // Vias and through-hole pads: shown while the Multi-Layer row is, and
    // never in a layer set.
    if view.multi_layer_visible() && view.layer_set().is_none() {
        let label: Label<'static> = t!(i18n, ids::PCB_LAYER_MULTI_LAYER).into();
        let messages = TabMessages {
            press: None,
            menu: None,
        };
        let swatch = layer_style::multi_layer_colour();
        tabs = tabs.push(layer_tab(label, swatch, tokens, false, messages));
    }
    let tabs = scrollable(tabs)
        .direction(Direction::Horizontal(Scrollbar::new().spacing(SP.xs)))
        .width(Length::Fill);
    let content = row![
        layer_set_button(i18n, view, window, tokens),
        Space::new().width(SP.s).boxed(),
        tabs.boxed(),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center);
    Some(
        container(content)
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

/// The name the layer set button shows: the set the strip lists, All
/// without one.
fn layer_set_label(i18n: &I18n, view: &PcbLayerView) -> Label<'static> {
    match view.layer_set().map(|set| &set.id) {
        Some(LayerSetId::Saved(name)) => signex_i18n::UserText::new(name.clone()).into(),
        Some(LayerSetId::Builtin(set)) => {
            t!(i18n, ids::PCB_LAYER_SET_SHORT, "set" => set.token()).into()
        }
        None => t!(i18n, ids::PCB_LAYER_SET_SHORT, "set" => "all").into(),
    }
}

/// The layer set button at the left end of the strip (#2394): the set's
/// name and a chevron; a click opens the set menu.
fn layer_set_button<'a>(
    i18n: &I18n,
    view: &PcbLayerView,
    window: Option<iced::window::Id>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let (ink, wash) = (
        ti(tokens.text),
        crate::styles::tokens::hover_overlay(tokens),
    );
    let content = row![
        text(layer_set_label(i18n, view))
            .size(TS.body)
            .color(ink)
            .boxed(),
        signex_widgets::chevron::chevron(true, TS.caption, r.ink3).boxed(),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center);
    let opened = Message::Pcb(PcbMsg::LayerSetMenu(LayerSetMenuMsg::Opened { window }));
    let set_button = button(content)
        .padding([SP.xs, SP.s])
        .style(move |_: &Theme, status| ButtonStyle {
            background: matches!(status, Status::Hovered | Status::Pressed)
                .then_some(Background::Color(wash)),
            border: Border {
                width: 1.0,
                color: r.line_soft,
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..ButtonStyle::default()
        })
        .on_press(opened);
    let tip = container(
        text(t!(i18n, ids::PCB_LAYER_SET_TOOLTIP))
            .size(TS.caption)
            .color(ink),
    )
    .padding([SP.xs, SP.s])
    .style(signex_widgets::active_bar_dropdown::tooltip_style(
        r.raised,
        r.line_strong,
    ));
    signex_widgets::tooltip(set_button, tip, signex_widgets::tooltip::Position::Top).boxed()
}

/// One tab: swatch and name, filled while it is the active layer.
fn layer_tab<'a>(
    label: Label<'static>,
    swatch: iced::Color,
    tokens: &ThemeTokens,
    selected: bool,
    messages: TabMessages,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let ink = if selected {
        r.accent_soft_ink
    } else {
        ti(tokens.text)
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
    let TabMessages { press, menu } = messages;
    let tab = button(content)
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
        .on_press_maybe(press)
        .boxed();
    match menu {
        Some(menu) => mouse_area(tab).on_right_press(menu).boxed(),
        None => tab,
    }
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

#[cfg(test)]
#[path = "pcb_strip_tests.rs"]
mod tests;
