//! Layer tabs under the PCB canvas (#2357, mockup row 104): a tab with a colour
//! swatch for every layer the Layers panel shows, in the panel's order,
//! then the Multi-Layer while it is shown, in a 28 px strip that scrolls
//! sideways when the tabs do not fit. Hiding a layer (its eye, its tab's
//! Hide row) takes its tab away; showing it brings the tab back. A click on
//! a tab makes that layer the active one, the same message a Layers panel
//! row sends, so the tab, the panel row and the status bar pill always name
//! one layer; the active tab has the accent bar along its top. A
//! right-click opens the tab's menu (#2379, #2394). The layer set button at
//! the left end shows the set the strip lists and opens the set menu
//! (#2394); the tabs name their layers in the user's style.

use iced::border::Side;
use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::scrollable::Direction;
use iced::widget::{Row, Space, container, mouse_area, row, scrollable};
use iced::{Background, Border, Color, Element, Font, Length, Theme, Widget as _};
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
use crate::styles::{HAIRLINE, chrome_px, ti, tokens::resolve};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Height of the strip.
const STRIP_HEIGHT: f32 = chrome_px(28.0 * 11.0 / 12.0);
/// Side padding of the layer set button.
const SET_PAD_X: f32 = chrome_px(8.0 * 11.0 / 12.0);
/// Side padding of a tab.
const TAB_PAD_X: f32 = chrome_px(10.0 * 11.0 / 12.0);
/// Gap between a tab's swatch and its name.
const TAB_GAP: f32 = chrome_px(6.0 * 11.0 / 12.0);
/// Edge of a tab's colour swatch.
const SWATCH: f32 = chrome_px(9.0 * 11.0 / 12.0);
/// Corner radius of a tab's colour swatch.
const SWATCH_RADIUS: f32 = chrome_px(2.0 * 11.0 / 12.0);
/// Thickness of the accent bar along the top of the active tab.
const ACCENT_BAR: f32 = 2.0 * HAIRLINE;

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
    /// The layer set menu is open for this strip: its button is drawn
    /// pressed (mockup row 104).
    pub set_menu_open: bool,
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
/// its eye is the Layers panel's Multi-Layer row. Nor does the tab of a
/// layer token this build does not know.
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
        set_menu_open,
    } = strip;
    if board.layers.is_empty() {
        return None;
    }
    let (panel, line) = (ti(tokens.panel_bg), ti(tokens.border));
    let active = view.shown_active();
    let copper = board_copper_layers(&board.layers);
    let paint = view.paint_of(board);
    let mut tabs: Row<Element<'a, Message>> = Row::new().height(Length::Fill);
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
    // A long strip scrolls inside the window instead of widening it.
    let tabs = scrollable(tabs)
        .direction(Direction::Horizontal(crate::styles::thin_scrollbar()))
        .style(crate::styles::scrollbar_style(tokens))
        .width(Length::Fill)
        .height(Length::Fill);
    let content = row![
        layer_set_button(i18n, view, window, set_menu_open, tokens),
        tabs.boxed(),
    ]
    .height(Length::Fill);
    Some(
        container(content)
            .height(STRIP_HEIGHT)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(panel)),
                border: Border {
                    width: HAIRLINE,
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

/// Fill and ink of the layer set button: muted ink at rest, the accent-soft
/// pair while its menu is open.
fn set_button_look(tokens: &ThemeTokens, open: bool) -> (Option<Color>, Color) {
    let r = resolve(tokens);
    if open {
        (Some(r.accent_soft), r.accent_soft_ink)
    } else {
        (None, r.ink3)
    }
}

/// The layer set button at the left end of the strip (#2394, mockup row 104): the
/// set's name and a chevron, a line on its right; a click opens the set
/// menu.
fn layer_set_button<'a>(
    i18n: &I18n,
    view: &PcbLayerView,
    window: Option<iced::window::Id>,
    open: bool,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let (fill, ink) = set_button_look(tokens, open);
    let (wash, line) = (
        crate::styles::tokens::hover_overlay(tokens),
        ti(tokens.border),
    );
    let content = row![
        text(layer_set_label(i18n, view))
            .size(TS.body)
            .color(ink)
            .wrapping(text::Wrapping::None)
            .boxed(),
        signex_widgets::chevron::chevron(true, TS.caption, ink).boxed(),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    let opened = Message::Pcb(PcbMsg::LayerSetMenu(LayerSetMenuMsg::Opened { window }));
    let set_button = button(container(content).center_y(Length::Fill))
        .padding([0.0, SET_PAD_X])
        .height(Length::Fill)
        .style(move |_: &Theme, status| ButtonStyle {
            background: fill
                .or_else(|| matches!(status, Status::Hovered | Status::Pressed).then_some(wash))
                .map(Background::Color),
            border: Border {
                right: Side::default().color(line).width(HAIRLINE),
                ..Border::default()
            },
            ..ButtonStyle::default()
        })
        .on_press(opened);
    let tip = container(
        text(t!(i18n, ids::PCB_LAYER_SET_TOOLTIP))
            .size(TS.caption)
            .color(ti(tokens.text)),
    )
    .padding([SP.xs, SP.s])
    .style(signex_widgets::active_bar_dropdown::tooltip_style(
        r.raised,
        r.line_strong,
    ));
    signex_widgets::tooltip(set_button, tip, signex_widgets::tooltip::Position::Top).boxed()
}

/// One tab: swatch and name with a line on its right; the active layer's
/// tab is filled accent-soft, in medium weight, with the accent bar along
/// its top.
fn layer_tab<'a>(
    label: Label<'static>,
    swatch: iced::Color,
    tokens: &ThemeTokens,
    selected: bool,
    messages: TabMessages,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let ring = r.shadow;
    let ink = if selected {
        ti(tokens.text)
    } else {
        ti(tokens.text_secondary)
    };
    let mut name = text(label)
        .size(TS.body)
        .color(ink)
        .wrapping(text::Wrapping::None);
    if selected {
        name = name.font(Font {
            weight: iced::font::Weight::Medium,
            ..Font::DEFAULT
        });
    }
    let content = row![
        container(Space::new())
            .width(SWATCH)
            .height(SWATCH)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(swatch)),
                border: Border {
                    width: HAIRLINE,
                    color: ring,
                    radius: SWATCH_RADIUS.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
        name.boxed(),
    ]
    .spacing(TAB_GAP)
    .align_y(iced::Alignment::Center);
    let (fill, wash, accent, line) = (
        r.accent_soft,
        crate::styles::tokens::hover_overlay(tokens),
        ti(tokens.accent),
        r.line_soft,
    );
    let TabMessages { press, menu } = messages;
    let tab = button(container(content).center_y(Length::Fill))
        .padding([0.0, TAB_PAD_X])
        .height(Length::Fill)
        .style(move |_: &Theme, status| {
            let background = if selected {
                Some(fill)
            } else {
                matches!(status, Status::Hovered | Status::Pressed).then_some(wash)
            };
            let mut border = Border {
                right: Side::default().color(line).width(HAIRLINE),
                ..Border::default()
            };
            if selected {
                border.top = Side::default().color(accent).width(ACCENT_BAR);
            }
            ButtonStyle {
                background: background.map(Background::Color),
                border,
                ..ButtonStyle::default()
            }
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

#[cfg(test)]
#[path = "pcb_strip_look_tests.rs"]
mod look_tests;
