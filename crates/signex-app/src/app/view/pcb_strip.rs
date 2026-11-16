//! Layer tabs under the PCB canvas (#2357): the board's own layers as tabs
//! with a colour swatch. A click on a tab makes that layer the active one,
//! the same message a Layers panel row sends, so the tab, the panel row and
//! the status bar pill always name one layer. A right-click opens the tab's
//! menu (#2379, #2394). The layer set button at the left end shows the set
//! the strip lists and opens the set menu (#2394); the tabs name their
//! layers in the user's style.

use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::{Row, Space, container, mouse_area, row};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_renderer::pcb::{LayerSetId, PcbLayerView, board_copper_layers};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{LayerDef, PcbBoard};
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

/// Display name and swatch of a board layer: the name the user's tab style
/// gives it (a mechanical layer the user named keeps that name), and the
/// colour the Layers panel gives it; a layer token the format does not know
/// keeps its own name.
fn describe(
    i18n: &I18n,
    def: &LayerDef,
    tokens: &ThemeTokens,
    names: (&LayerStripPrefs, &[signex_types::pcb_layer::CopperLayer]),
) -> (Label<'static>, iced::Color) {
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
        tab_name(i18n, layer, names.0, names.1).into()
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
/// A tab whose layer this build does not know sends neither.
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
    let mut tabs: Row<Element<'a, Message>> = row![
        layer_set_button(i18n, view, window, tokens),
        Space::new().width(SP.s).boxed(),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center);
    for def in &board.layers {
        let layer = parse_pcb_layer(&def.name);
        // A set lists its own layers alone (#2394).
        if layer.is_some_and(|l| !view.lists_in_strip(l)) {
            continue;
        }
        let selected = layer.is_some() && layer == active;
        let hidden = layer.is_some_and(|l| view.is_hidden(l));
        let look = TabLook { selected, hidden };
        let messages = TabMessages {
            press: layer.map(|l| picked(window, l)),
            menu: layer.map(|l| menu_opened(window, l)),
        };
        let (label, swatch) = describe(i18n, def, tokens, (prefs, copper.as_slice()));
        tabs = tabs.push(layer_tab(label, swatch, tokens, look, messages));
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

/// How a tab stands: the active layer, a hidden layer.
#[derive(Clone, Copy)]
struct TabLook {
    selected: bool,
    hidden: bool,
}

/// One tab: swatch and name, filled while it is the active layer, faint
/// while the layer is hidden.
fn layer_tab<'a>(
    label: Label<'static>,
    swatch: iced::Color,
    tokens: &ThemeTokens,
    look: TabLook,
    messages: TabMessages,
) -> Element<'a, Message> {
    let TabLook { selected, hidden } = look;
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
