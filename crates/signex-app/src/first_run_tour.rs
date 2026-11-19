//! First-run card: one dismissible card pinned to the bottom-right of the
//! main window on first launch. Four rows (pan, zoom, the command palette,
//! the shortcut sheet), each an icon tile, a verb and the gesture or key as
//! a cap; a footer naming what the mouse wheel does, with a link to change
//! it, and "Got it". It closes only on ✕, "Got it", Esc or a pointer
//! gesture on a canvas (docs/DECISIONS.md, 2026-11-03 item 9).

use iced::widget::{Column, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, raw, text};

use crate::about_modal::{GLYPH_THEME, px};
use crate::app::{Message, OverlayMsg};
use crate::canvas::WheelMode;
use crate::icons;
use crate::keyboard_shortcuts_modal::key_cap;
use crate::keymap::{CompiledKeymap, Modifiers, ModifiersExt, Platform};
use crate::styles::{self, DialogButtonKind, Radii, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;

const CARD_W: f32 = px(320.0);
const CARD_PAD_RIGHT: f32 = 24.0;
const CARD_PAD_BOTTOM: f32 = 36.0;
const HEADER_H: f32 = px(40.0);
const TILE: f32 = px(30.0);
const ROW_GAP: f32 = px(10.0);

/// What the card reads from the app: the wheel setting and the keys the
/// keymap binds to the palette and the shortcut sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TourFacts {
    pub wheel_mode: WheelMode,
    /// The palette's key as the keymap shows it; empty when unbound.
    pub palette_key: String,
    /// The key that opens the shortcut sheet.
    pub sheet_key: String,
}

impl TourFacts {
    /// The facts for `wheel_mode` and the active `keymap`. The palette cap
    /// names the real binding, the same rule as the title-bar cap.
    pub fn read(wheel_mode: WheelMode, keymap: &CompiledKeymap, i18n: &I18n) -> Self {
        Self {
            wheel_mode,
            palette_key: keymap.shortcut_label_preferring("open_command_palette", "Ctrl+K", i18n),
            sheet_key: crate::keyboard_shortcuts_modal::sheet_key(keymap, i18n),
        }
    }
}

/// The card pinned to the main window's corner.
pub fn view<'a>(i18n: &I18n, tokens: &'a ThemeTokens, facts: &TourFacts) -> Element<'a, Message> {
    build(i18n, tokens, facts, false)
}

/// The card as the content of its own OS window: no header, filling it.
pub fn window_view<'a>(
    i18n: &I18n,
    tokens: &'a ThemeTokens,
    facts: &TourFacts,
) -> Element<'a, Message> {
    build(i18n, tokens, facts, true)
}

/// The zoom gesture under `mode`: the wheel, or with Scroll the primary
/// modifier held with the wheel (Ctrl, or Cmd on macOS).
pub fn zoom_gesture(i18n: &I18n, mode: WheelMode) -> Localized {
    match mode {
        WheelMode::Zoom => t!(i18n, ids::DIALOG_TOUR_ZOOM_WHEEL),
        WheelMode::Scroll => {
            let primary = Modifiers {
                primary: true,
                ..Modifiers::default()
            };
            let modifier = primary.display_in(Platform::current(), i18n);
            t!(i18n, ids::DIALOG_TOUR_ZOOM_MODIFIED_WHEEL, "modifier" => modifier)
        }
    }
}

fn build<'a>(
    i18n: &I18n,
    tokens: &'a ThemeTokens,
    facts: &TourFacts,
    windowed: bool,
) -> Element<'a, Message> {
    let rows: [(iced::widget::svg::Handle, MessageId, Option<Label<'a>>); 4] = [
        (
            icons::icon_chrome_hand(GLYPH_THEME),
            ids::DIALOG_TOUR_PAN,
            Some(t!(i18n, ids::DIALOG_TOUR_PAN_GESTURE).into()),
        ),
        (
            icons::icon_chrome_mouse(GLYPH_THEME),
            ids::DIALOG_TOUR_ZOOM,
            Some(zoom_gesture(i18n, facts.wheel_mode).into()),
        ),
        (
            icons::icon_chrome_search(GLYPH_THEME),
            ids::DIALOG_TOUR_FIND_COMMAND,
            // raw: the key chord, already rendered for the locale by the keymap.
            (!facts.palette_key.is_empty()).then(|| raw(facts.palette_key.clone())),
        ),
        (
            icons::icon_chrome_keyboard(GLYPH_THEME),
            ids::DIALOG_TOUR_ALL_SHORTCUTS,
            // raw: the key chord, already rendered for the locale by the keymap.
            Some(raw(facts.sheet_key.clone())),
        ),
    ];
    let mut list: Column<Element<'a, Message>> = Column::new().spacing(ROW_GAP);
    for (icon, label, cap) in rows {
        list = list.push(tour_row(i18n, tokens, icon, label, cap));
    }
    let body = container(list)
        .padding([px(12.0), px(16.0)])
        .width(Length::Fill);
    let footer = footer(i18n, tokens, facts.wheel_mode);

    if windowed {
        return container(column![body, Space::new().height(Length::Fill), footer])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(styles::modal_card(tokens))
            .boxed();
    }
    let header = crate::app::view::dialogs::graphite_header(
        t!(i18n, ids::DIALOG_TITLE_FIRST_RUN_TOUR).into_string(),
        None,
        None,
        Message::Overlay(OverlayMsg::DismissFirstRunTour),
        HEADER_H,
        tokens,
    );
    let card = container(column![header, body, footer])
        .width(Length::Fixed(CARD_W))
        .clip(true)
        .style(styles::modal_card(tokens));

    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(
            iced::Padding::default()
                .right(CARD_PAD_RIGHT)
                .bottom(CARD_PAD_BOTTOM),
        )
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Bottom)
        .boxed()
}

/// One row: the icon tile, the verb, and the gesture or key as a cap.
fn tour_row<'a>(
    i18n: &I18n,
    tokens: &ThemeTokens,
    icon: iced::widget::svg::Handle,
    label: MessageId,
    cap: Option<Label<'a>>,
) -> Element<'a, Message> {
    let r = styles::tokens::resolve(tokens);
    let (sunken, line) = (r.sunken, r.line_soft);
    let tile = container(styles::tinted_glyph(icon, px(14.0), r.ink4))
        .width(TILE)
        .height(TILE)
        .center_x(TILE)
        .center_y(TILE)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let mut line_row = row![
        tile,
        text(t!(i18n, label))
            .size(TS.body)
            .color(ti(tokens.text))
            .width(Length::Fill),
    ]
    .spacing(px(10.0))
    .align_y(Alignment::Center);
    if let Some(cap) = cap {
        line_row = line_row.push(key_cap(cap, false, tokens));
    }
    line_row.boxed()
}

/// "Wheel set to Zoom · change" and "Got it".
fn footer<'a>(i18n: &I18n, tokens: &ThemeTokens, mode: WheelMode) -> Element<'a, Message> {
    let r = styles::tokens::resolve(tokens);
    let note = row![
        text(t!(i18n, ids::DIALOG_TOUR_WHEEL_SET, "mode" => mode.as_pref()))
            .size(TS.caption)
            .color(r.ink4),
        button(
            text(t!(i18n, ids::DIALOG_TOUR_WHEEL_CHANGE))
                .size(TS.caption)
                .color(r.accent_text)
        )
        .padding(0)
        .on_press(Message::Overlay(OverlayMsg::FirstRunTourWheelChangePressed))
        .style(styles::ghost_button(tokens)),
    ]
    .spacing(px(4.0))
    .align_y(Alignment::Center);
    styles::dialog_footer_band(
        vec![note.boxed()],
        vec![styles::dialog_button(
            t!(i18n, ids::DIALOG_TOUR_GOT_IT),
            DialogButtonKind::Primary,
            Some(Message::Overlay(OverlayMsg::DismissFirstRunTour)),
            tokens,
        )],
        Padding::from([px(10.0), px(16.0)]),
        tokens,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::{lay_out_texts, messages_on_click};
    use signex_types::theme::{ThemeId, theme_tokens};

    const WINDOW: iced::Size = iced::Size::new(1400.0, 900.0);

    fn facts(mode: WheelMode) -> TourFacts {
        TourFacts {
            wheel_mode: mode,
            palette_key: "Ctrl+K".into(),
            sheet_key: "F1".into(),
        }
    }

    fn texts(element: Element<'_, Message>) -> Vec<String> {
        lay_out_texts(element, WINDOW)
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    #[test]
    fn the_card_lists_four_rows_with_their_caps() {
        let i18n = I18n::en_us_ref();
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            let shown = texts(view(i18n, &tokens, &facts(WheelMode::Zoom)));
            for expected in [
                "Welcome to Signex",
                "Pan",
                "Right-drag",
                "Zoom",
                "Wheel",
                "Find any command",
                "Ctrl+K",
                "All keyboard shortcuts",
                "F1",
                "Wheel set to Zoom ·",
                "change",
                "Got it",
            ] {
                assert!(
                    shown.iter().any(|t| t == expected),
                    "{id:?}: {expected:?} not in {shown:?}"
                );
            }
        }
    }

    #[test]
    fn the_zoom_row_follows_the_wheel_setting() {
        let i18n = I18n::en_us_ref();
        assert_eq!(zoom_gesture(i18n, WheelMode::Zoom).into_string(), "Wheel");
        let modifier = Modifiers {
            primary: true,
            ..Modifiers::default()
        }
        .display_in(Platform::current(), i18n);
        let scroll = zoom_gesture(i18n, WheelMode::Scroll)
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(scroll, format!("{modifier}+Wheel"));

        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let shown = texts(view(i18n, &tokens, &facts(WheelMode::Scroll)));
        assert!(shown.iter().any(|t| *t == scroll), "{shown:?}");
        assert!(
            shown.iter().any(|t| t == "Wheel set to Scroll ·"),
            "{shown:?}"
        );
        assert!(!shown.iter().any(|t| t == "Wheel"), "{shown:?}");
    }

    #[test]
    fn an_unbound_palette_key_leaves_its_row_without_a_cap() {
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let mut unbound = facts(WheelMode::Zoom);
        unbound.palette_key.clear();
        let shown = texts(view(I18n::en_us_ref(), &tokens, &unbound));
        assert!(shown.iter().any(|t| t == "Find any command"));
        assert!(!shown.iter().any(|t| t == "Ctrl+K"));
    }

    #[test]
    fn got_it_and_the_change_link_send_their_messages() {
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let i18n = I18n::en_us_ref();
        let click = |label: &str| {
            messages_on_click(view(i18n, &tokens, &facts(WheelMode::Zoom)), WINDOW, label)
        };
        assert!(
            click("Got it")
                .iter()
                .any(|m| matches!(m, Message::Overlay(OverlayMsg::DismissFirstRunTour)))
        );
        assert!(click("change").iter().any(|m| matches!(
            m,
            Message::Overlay(OverlayMsg::FirstRunTourWheelChangePressed)
        )));
    }
}
