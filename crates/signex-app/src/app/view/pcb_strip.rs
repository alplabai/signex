//! Layer tabs under the PCB canvas (#2357, #104): the board's visible layers
//! as tabs with a colour swatch, in a 28 px strip that scrolls sideways
//! when the tabs do not fit. A click on a tab makes that layer the active
//! one, the same message a Layers panel row sends, so the tab, the panel row
//! and the status bar pill always name one layer; the active tab has the
//! accent bar along its top. A right-click opens the tab's menu (#2379,
//! #2394). The layer set button at the left end shows the set the strip
//! lists and opens the set menu (#2394); the tabs name their layers in the
//! user's style.

use iced::border::Side;
use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::scrollable::Direction;
use iced::widget::{Row, Space, container, mouse_area, row, scrollable};
use iced::{Background, Border, Color, Element, Font, Length, Theme, Widget as _};
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
const SWATCH_RADIUS: f32 = 2.0;
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
    /// The layer set menu is open for this strip: its button is drawn
    /// pressed (#104).
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
/// A tab whose layer this build does not know sends neither.
struct TabMessages {
    press: Option<Message>,
    menu: Option<Message>,
}

/// Whether `layer` gets a tab: the set lists it (#2394) and it is not
/// hidden (#104). Show all layers brings a hidden layer's tab back.
fn has_tab(view: &PcbLayerView, layer: PcbLayer) -> bool {
    view.lists_in_strip(layer) && !view.is_hidden(layer)
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
    let mut tabs: Row<Element<'a, Message>> = Row::new().height(Length::Fill);
    for def in &board.layers {
        let layer = parse_pcb_layer(&def.name);
        // An unknown layer token keeps its tab.
        if layer.is_some_and(|l| !has_tab(view, l)) {
            continue;
        }
        let selected = layer.is_some() && layer == active;
        let messages = TabMessages {
            press: layer.map(|l| picked(window, l)),
            menu: layer.map(|l| menu_opened(window, l)),
        };
        let (label, swatch) = describe(i18n, def, tokens, (prefs, copper.as_slice()));
        tabs = tabs.push(layer_tab(label, swatch, tokens, selected, messages));
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

/// The layer set button at the left end of the strip (#2394, #104): the
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
mod tests {
    use super::*;
    use crate::layout_probe::{draw_pixels, lay_out_texts, root_size};
    use iced::Size;
    use signex_types::pcb_layer::CopperLayer;
    use signex_types::theme::{ThemeId, theme_tokens};

    const BOTTOM: PcbLayer = PcbLayer::Copper(CopperLayer::Bottom);

    /// A board that declares the layer tokens `names`, nothing on them.
    fn board(names: &[&str]) -> PcbBoard {
        let layers = names
            .iter()
            .zip(0u8..)
            .map(|(name, id)| LayerDef {
                id,
                name: (*name).to_owned(),
                layer_type: String::new(),
                display_name: String::new(),
            })
            .collect();
        PcbBoard {
            sketch: None,
            stackup: None,
            options: Default::default(),
            unknown: Default::default(),
            uuid: uuid::Uuid::now_v7(),
            version: 1,
            generator: "signex-test".into(),
            thickness: 1.6,
            outline: vec![],
            layers,
            setup: None,
            nets: vec![],
            footprints: vec![],
            segments: vec![],
            vias: vec![],
            zones: vec![],
            graphics: vec![],
            texts: vec![],
            free: Default::default(),
        }
    }

    fn strip<'a>(
        board: &PcbBoard,
        view: &PcbLayerView,
        set_menu_open: bool,
        tokens: &ThemeTokens,
    ) -> Element<'a, Message> {
        let prefs = LayerStripPrefs::default();
        let strip = StripBoard {
            board,
            view,
            window: None,
            prefs: &prefs,
            set_menu_open,
        };
        layer_strip(strip, &I18n::en_us(), tokens).expect("the board declares layers")
    }

    fn names(element: Element<'_, Message>, window: Size) -> Vec<String> {
        lay_out_texts(element, window)
            .into_iter()
            .map(|t| t.text)
            .collect()
    }

    /// #104: more tabs than the window is wide scroll inside the strip; every
    /// visible layer still has its tab, on one line, and the strip keeps to
    /// the window and its 28 px.
    #[test]
    fn the_strip_lays_out_every_visible_tab_inside_its_scrollable() {
        let board = board(&[
            "TopCopper",
            "BottomCopper",
            "TopSilkscreen",
            "BottomSilkscreen",
            "TopSolderMask",
            "BottomSolderMask",
            "TopPaste",
            "BottomPaste",
            "EdgeCuts",
            "TopCourtyard",
            "BottomCourtyard",
            "Dimensions",
            "Notes",
        ]);
        let view = PcbLayerView::default();
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let window = Size::new(400.0, 600.0);
        let laid_out = lay_out_texts(strip(&board, &view, false, &tokens), window);
        for name in [
            "Top Layer",
            "Bottom Layer",
            "Top Silkscreen",
            "Bottom Silkscreen",
            "Top Solder",
            "Bottom Solder",
            "Top Paste",
            "Bottom Paste",
            "Board Outline",
            "Top Courtyard",
            "Bottom Courtyard",
            "Dimensions",
            "Notes",
        ] {
            let tab = laid_out
                .iter()
                .find(|t| t.text == name)
                .unwrap_or_else(|| panic!("no tab {name:?}: {laid_out:#?}"));
            assert!(tab.bounds.height < 2.0 * TS.body, "{name} on one line");
        }
        let size = root_size(strip(&board, &view, false, &tokens), window);
        assert!(size.width <= window.width, "the strip keeps to the window");
        assert_eq!(size.height, STRIP_HEIGHT);
    }

    /// #104: a hidden layer has no tab; showing every layer brings it back.
    /// A layer token this build does not know keeps its tab.
    #[test]
    fn a_hidden_layer_has_no_tab_until_it_is_shown() {
        let board = board(&["TopCopper", "BottomCopper", "Nowhere"]);
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let window = Size::new(1200.0, 600.0);
        let mut view = PcbLayerView::default();
        view.hide(BOTTOM);
        let shown = names(strip(&board, &view, false, &tokens), window);
        assert!(!shown.iter().any(|t| t == "Bottom Layer"), "{shown:?}");
        assert!(shown.iter().any(|t| t == "Top Layer"), "{shown:?}");
        assert!(shown.iter().any(|t| t == "Nowhere"), "{shown:?}");
        view.show_all();
        let shown = names(strip(&board, &view, false, &tokens), window);
        assert!(shown.iter().any(|t| t == "Bottom Layer"), "{shown:?}");
    }

    /// #104: the layer set button is filled accent-soft while its menu is
    /// open, plain with muted ink otherwise, in both Graphite themes.
    #[test]
    fn the_set_button_draws_pressed_while_its_menu_is_open() {
        let board = board(&["TopCopper", "BottomCopper"]);
        let view = PcbLayerView::default();
        for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(theme);
            let r = resolve(&tokens);
            assert_eq!(set_button_look(&tokens, false), (None, r.ink3), "{theme:?}");
            assert_eq!(
                set_button_look(&tokens, true),
                (Some(r.accent_soft), r.accent_soft_ink),
                "{theme:?}"
            );
            let window = Size::new(320.0, STRIP_HEIGHT);
            let closed = draw_pixels(strip(&board, &view, false, &tokens), window);
            let open = draw_pixels(strip(&board, &view, true, &tokens), window);
            assert_ne!(closed, open, "{theme:?}: the open button looks pressed");
        }
    }
}
