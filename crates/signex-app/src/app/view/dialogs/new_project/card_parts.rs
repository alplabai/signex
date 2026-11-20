//! The small parts of a profile card (#2537, #2536): the layer glyph, the
//! check disc, the edited pill, the pencil and menu buttons, the card's
//! layer stepper and the menu surface its rows sit on.

use iced::widget::{Space, column, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, button::Status, text};

use super::{SP, TS};
use crate::styles::{Radii, ti, tokens};

const CHECK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"><path d="M5 12l5 5 9-10" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const PEN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"><path d="M4 20h4L19 9l-4-4L4 16z M13.5 6.5l4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/></svg>"#;
const MORE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"><circle cx="5" cy="12" r="1.8" fill="currentColor"/><circle cx="12" cy="12" r="1.8" fill="currentColor"/><circle cx="19" cy="12" r="1.8" fill="currentColor"/></svg>"#;
const SLIDERS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"><path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><circle cx="16" cy="6" r="2" fill="none" stroke="currentColor" stroke-width="1.8"/><circle cx="10" cy="12" r="2" fill="none" stroke="currentColor" stroke-width="1.8"/><circle cx="18" cy="18" r="2" fill="none" stroke="currentColor" stroke-width="1.8"/></svg>"#;

/// Side of the layer glyph: three body lines.
const GLYPH: f32 = TS.body * 3.0;
/// Side of the check disc.
const DISC: f32 = TS.body * 1.5;
/// Side of the card pencil button.
pub(super) const PENCIL: f32 = TS.body * 1.8;
/// Side of a saved profile's menu button.
pub(super) const MENU_BUTTON: f32 = TS.body * 1.65;
/// Height of the edited pill.
const PILL_H: f32 = TS.caption * 1.4;
/// The glyph shows at most this many copper bars.
const MAX_BARS: u8 = 10;
/// Above this many copper layers the glyph draws thinner bars.
const THIN_FROM: u8 = 6;

/// One icon, tinted `ink`.
pub(super) fn icon<'a, M: 'a>(bytes: &'static [u8], size: f32, ink: Color) -> Element<'a, M> {
    svg(svg::Handle::from_memory(bytes))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

pub(super) fn pen_icon<'a, M: 'a>(ink: Color) -> Element<'a, M> {
    icon(PEN, TS.label, ink)
}

pub(super) fn more_icon<'a, M: 'a>(ink: Color) -> Element<'a, M> {
    icon(MORE, TS.label, ink)
}

/// A filled bar of the glyph.
fn bar<'a, M: 'a>(height: f32, color: Color, radius: f32) -> Element<'a, M> {
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .style(move |_: &Theme| container::Style {
            background: Some(color.into()),
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The bar heights and gap of a `layers`-layer glyph: copper, dielectric,
/// gap. Thinner bars and a tighter gap as the stack grows.
pub(super) fn glyph_metrics(layers: u8) -> (f32, f32, f32) {
    let shown = layers.min(MAX_BARS);
    let (copper, dielectric) = if shown > THIN_FROM {
        (2.0, 1.0)
    } else {
        (3.0, 2.0)
    };
    let gap = match shown {
        0..=4 => 2.0,
        5..=6 => 1.0,
        _ => 0.0,
    };
    (copper, dielectric, gap)
}

/// The copper bars a `layers`-layer glyph draws: at most [`MAX_BARS`].
pub(super) fn glyph_bars(layers: u8) -> u8 {
    layers.clamp(1, MAX_BARS)
}

/// The card glyph: copper bars split by dielectric bars for a tier, the
/// sliders icon for Custom (`layers` `None`). Sunken; raised when picked.
pub(super) fn glyph<'a, M: 'a>(
    layers: Option<u8>,
    picked: bool,
    copper: Color,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let r = tokens::resolve(tokens);
    let fill = if picked { r.raised } else { r.sunken };
    let content: Element<'a, M> = match layers {
        None => icon(SLIDERS, TS.title, r.ink3),
        Some(layers) => {
            let (cu, diel, gap) = glyph_metrics(layers);
            let bars = glyph_bars(layers);
            let mut stack = column![].spacing(gap).width(Length::Fill);
            for i in 0..bars {
                stack = stack.push(bar(cu, copper, 2.0));
                if i + 1 < bars {
                    stack = stack.push(bar(diel, r.line_strong, 1.0));
                }
            }
            stack.boxed()
        }
    };
    container(content)
        .width(Length::Fixed(GLYPH))
        .height(Length::Fixed(GLYPH))
        .padding([0.0, SP.m - SP.xs])
        .center_x(Length::Fixed(GLYPH))
        .center_y(Length::Fixed(GLYPH))
        .style(move |_: &Theme| container::Style {
            background: Some(fill.into()),
            border: Border {
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The check disc at a card's top right: an empty ring, or the accent disc
/// with a white check when picked.
pub(super) fn check_disc<'a, M: 'a>(picked: bool, tokens: &ThemeTokens) -> Element<'a, M> {
    let r = tokens::resolve(tokens);
    let accent = ti(tokens.accent);
    let content: Element<'a, M> = if picked {
        icon(CHECK, TS.body, tokens::on_accent(tokens))
    } else {
        Space::new().boxed()
    };
    container(content)
        .width(Length::Fixed(DISC))
        .height(Length::Fixed(DISC))
        .center_x(Length::Fixed(DISC))
        .center_y(Length::Fixed(DISC))
        .style(move |_: &Theme| container::Style {
            background: picked.then_some(Background::Color(accent)),
            border: Border {
                width: 1.0,
                color: if picked { accent } else { r.line_strong },
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The accent "edited" pill beside an edited built-in's title.
pub(super) fn edited_pill<'a, M: 'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, M> {
    let r = tokens::resolve(tokens);
    container(
        text(t!(i18n, ids::DIALOG_NEW_PROJECT_CARD_EDITED))
            .size(TS.caption)
            .color(r.accent_text),
    )
    .height(Length::Fixed(PILL_H))
    .padding([0.0, SP.s + SP.xs])
    .center_y(Length::Fixed(PILL_H))
    .style(move |_: &Theme| container::Style {
        background: Some(r.accent_soft.into()),
        border: Border {
            radius: Radii::GRAPHITE.pill.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// A square icon button: quiet at rest, the hover fill on hover and while
/// its menu is open (`open`).
pub(super) fn icon_button<'a, M: Clone + 'a>(
    content: Element<'a, M>,
    side: f32,
    open: bool,
    on_press: M,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let hover = tokens::overlay(tokens, tokens::Overlay::Hover);
    let pressed = tokens::overlay(tokens, tokens::Overlay::Pressed);
    let ink = ti(tokens.text);
    button(
        container(content)
            .center_x(Length::Fixed(side))
            .center_y(Length::Fixed(side)),
    )
    .padding(0)
    .width(Length::Fixed(side))
    .height(Length::Fixed(side))
    .on_press(on_press)
    .style(move |_: &Theme, status: Status| button::Style {
        background: match status {
            Status::Pressed => Some(pressed.into()),
            Status::Hovered => Some(hover.into()),
            _ if open => Some(hover.into()),
            _ => None,
        },
        text_color: ink,
        border: Border {
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .boxed()
}

/// The small copper-layer stepper on a card: minus, the count, plus.
pub(super) fn layer_stepper<'a, M: Clone + 'a>(
    layers: u8,
    minus: M,
    plus: M,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let step = |glyph: &'static str, msg: M| {
        button(
            text(signex_i18n::UserText::new(glyph))
                .size(TS.body)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
        )
        .padding([0.0, SP.s])
        .width(Length::Fixed(TS.body * 1.6))
        .on_press(msg)
        .style(crate::styles::ghost_button(tokens))
        .boxed()
    };
    let line = ti(tokens.border);
    let fill = tokens::resolve(tokens).field;
    container(
        row![
            step("\u{2212}", minus),
            text(signex_i18n::UserText::new(layers.to_string()))
                .size(TS.caption)
                .font(super::super::graphite::mono())
                .color(ti(tokens.text))
                .width(Length::Fixed(TS.body * 1.6))
                .align_x(iced::alignment::Horizontal::Center),
            step("+", plus),
        ]
        .align_y(Alignment::Center),
    )
    .style(move |_: &Theme| container::Style {
        background: Some(fill.into()),
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// The floating menu surface: raised fill, hairline, card radius, shadow.
pub(super) fn menu_surface<'a, M: 'a>(
    rows: Vec<Element<'a, M>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let r = tokens::resolve(tokens);
    let line = ti(tokens.border);
    let shadow = crate::styles::floating_shadow(tokens);
    container(column(rows).spacing(SP.xs))
        .padding(SP.s)
        .width(Length::Fixed(TS.body * 20.0))
        .style(move |_: &Theme| container::Style {
            background: Some(r.raised.into()),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            shadow,
            ..container::Style::default()
        })
        .boxed()
}

/// One menu row: flat, the hover fill on hover.
pub(super) fn menu_row<'a, M: Clone + 'a>(
    label: Localized,
    on_press: M,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    button(text(label).size(TS.body).color(ti(tokens.text)))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .on_press(on_press)
        .style(crate::styles::ghost_button(tokens))
        .boxed()
}

/// A menu's bold header line.
pub(super) fn menu_header<'a, M: 'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, M> {
    container(
        text(label)
            .size(TS.caption)
            .color(tokens::resolve(tokens).ink3),
    )
    .padding([SP.xs, SP.m])
    .boxed()
}

/// A hairline between menu rows.
pub(super) fn menu_separator<'a, M: 'a>(tokens: &ThemeTokens) -> Element<'a, M> {
    let line = ti(tokens.border);
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(crate::styles::HAIRLINE))
        .style(move |_: &Theme| container::Style {
            background: Some(line.into()),
            ..container::Style::default()
        })
        .boxed()
}

/// A menu's wrapped caption.
pub(super) fn menu_caption<'a, M: 'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, M> {
    container(
        text(label)
            .size(TS.caption)
            .color(tokens::muted_text(tokens)),
    )
    .padding([SP.xs, SP.m])
    .boxed()
}
