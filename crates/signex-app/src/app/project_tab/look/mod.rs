//! The Graphite look of the Project tab pages (mockups m30-m38): the card,
//! pills, status lines, label rows, buttons and glyphs every section builds
//! from. Pure view helpers; sizes from the type and spacing scale, colours
//! from the theme tokens.

mod glyphs;

#[cfg(test)]
mod tests;

pub(in crate::app::project_tab) use glyphs::{Glyph, glyph};

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Column, Row, column, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::Localized;
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::table::{SP, TS};
use crate::app::Message;
use crate::styles::tokens::{Overlay, danger, overlay, resolve};
use crate::styles::{HAIRLINE, PillTone, Radii, dimmed, pill_colors, ti};

/// Side of a line glyph next to text (the mockup's 14 px icon).
pub(in crate::app::project_tab) const ICON: f32 = TS.body * 7.0 / 6.0;
/// Side of an icon button (24 px).
pub(in crate::app::project_tab) const ICON_BUTTON: f32 = TS.body * 2.0;
/// Height of a pill (16 px).
const PILL_H: f32 = TS.body * 4.0 / 3.0;
/// Glyph in a pill (11 px).
const PILL_GLYPH: f32 = TS.caption;
/// Width of a [`kv_row`] caption column (150 px).
const KV_LABEL_W: f32 = TS.body * 12.5;
/// Padding of an info, warn or error line (about 7 x 9 px).
const LINE_PAD: [f32; 2] = [SP.s + SP.xs, SP.m];

/// The monospace face of paths, file names and numbers.
pub(in crate::app::project_tab) fn mono_font() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// The UI face at `weight`.
pub(in crate::app::project_tab) fn ui_font(weight: iced::font::Weight) -> iced::Font {
    iced::Font {
        weight,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// The header of a [`card`]: its bold title, then what sits at its right
/// (a caption, a button).
pub(in crate::app::project_tab) struct CardHeader<'a> {
    pub title: Localized,
    pub trailing: Vec<Element<'a, Message>>,
}

/// A bordered box (`.box`): an optional header over a hairline, then `body`.
pub(in crate::app::project_tab) fn card<'a>(
    header: Option<CardHeader<'a>>,
    body: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let line_soft = resolve(tokens).line_soft;
    let mut parts: Column<Element<'a, Message>> = Column::new().width(Length::Fill);
    if let Some(header) = header {
        let mut line: Row<Element<'a, Message>> =
            Row::new().spacing(SP.m).align_y(Alignment::Center).push(
                text(header.title)
                    .size(TS.body)
                    .font(ui_font(iced::font::Weight::Semibold))
                    .color(ti(tokens.text))
                    .width(Length::Fill)
                    .boxed(),
            );
        for part in header.trailing {
            line = line.push(part);
        }
        parts = parts.push(
            container(line)
                .width(Length::Fill)
                .padding([SP.m + SP.xs, SP.l])
                .style(move |_: &Theme| container::Style {
                    border: Border {
                        color: line_soft,
                        ..Border::default()
                    }
                    .bottom(iced::border::Side::default().width(HAIRLINE)),
                    ..container::Style::default()
                })
                .boxed(),
        );
    }
    parts = parts.push(
        container(body)
            .width(Length::Fill)
            .padding([SP.l, SP.l])
            .boxed(),
    );
    let (fill, line) = (ti(tokens.panel_bg), ti(tokens.border));
    container(parts)
        .width(Length::Fill)
        .padding(HAIRLINE)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: HAIRLINE,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A status pill: caption words in the tone's ink on its soft fill, with an
/// optional glyph before them.
pub(in crate::app::project_tab) fn pill<'a>(
    words: Localized,
    tone: PillTone,
    glyph: Option<Glyph>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink = pill_colors(tokens, tone).1;
    let mut line: Row<Element<'a, Message>> = Row::new().spacing(SP.xs).align_y(Alignment::Center);
    if let Some(g) = glyph {
        line = line.push(glyphs::glyph(g, PILL_GLYPH, ink));
    }
    line = line.push(
        text(words)
            .size(TS.caption)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
    );
    container(line)
        .height(Length::Fixed(PILL_H))
        .padding([0.0, SP.s + SP.xs])
        .align_y(Vertical::Center)
        .style(crate::styles::pill(tokens, tone))
        .boxed()
}

/// A tinted status line: `glyph` in `ink`, then `content`, then `action`.
fn status_line<'a>(
    g: Glyph,
    content: Element<'a, Message>,
    action: Option<Element<'a, Message>>,
    fill: Color,
    ink: Color,
) -> Element<'a, Message> {
    let mut line: Row<Element<'a, Message>> = row![
        container(glyphs::glyph(g, ICON, ink)).padding([HAIRLINE, 0.0]),
        container(content).width(Length::Fill),
    ]
    .spacing(SP.s + SP.xs * 0.5)
    .align_y(Alignment::Start);
    if let Some(action) = action {
        line = line.push(action);
    }
    container(line)
        .width(Length::Fill)
        .padding(LINE_PAD)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            text_color: Some(ink),
            border: Border {
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A quiet note (`.info-line`): sunken fill, ink3, an info glyph. Text in
/// `content` without its own colour takes the line's ink.
pub(in crate::app::project_tab) fn info_line<'a>(
    content: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    status_line(Glyph::Info, content, None, r.sunken, r.ink3)
}

/// A warning (`.warn-line`): warn_soft fill, warning ink, `action` at the
/// right.
pub(in crate::app::project_tab) fn warn_line<'a>(
    glyph: Glyph,
    content: Element<'a, Message>,
    action: Option<Element<'a, Message>>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    status_line(
        glyph,
        content,
        action,
        resolve(tokens).warn_soft,
        ti(tokens.warning),
    )
}

/// An error (`.err-box`): err_soft fill, error ink, a warning glyph.
pub(in crate::app::project_tab) fn err_box<'a>(
    content: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    status_line(
        Glyph::Warn,
        content,
        None,
        resolve(tokens).err_soft,
        danger(tokens),
    )
}

/// A key / value row (`.kv`): the caption in ink3 in a fixed column, then
/// `value`.
pub(in crate::app::project_tab) fn kv_row<'a>(
    caption: Localized,
    value: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    row![
        container(text(caption).size(TS.body).color(resolve(tokens).ink3))
            .width(Length::Fixed(KV_LABEL_W)),
        container(value).width(Length::Fill),
    ]
    .spacing(SP.xl)
    .align_y(Alignment::Center)
    .boxed()
}

/// An option row (`.optrow`): the title over its help, the control at the
/// right.
pub(in crate::app::project_tab) fn option_row<'a>(
    title: Localized,
    help: Localized,
    control: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let words = column![
        text(title)
            .size(TS.body)
            .font(ui_font(iced::font::Weight::Medium))
            .color(ti(tokens.text)),
        text(help).size(TS.caption).color(resolve(tokens).ink3),
    ]
    .spacing(SP.xs)
    .width(Length::Fill);
    row![words, control]
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed()
}

/// A field label (`.flabel`): caption size, ink3, weight 500. The words are
/// shown as given: the catalog writes a field label in capitals, as it does
/// for the library editor's section headers.
pub(in crate::app::project_tab) fn field_label<'a>(
    words: Localized,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    text(words)
        .size(TS.caption)
        .font(ui_font(iced::font::Weight::Medium))
        .color(resolve(tokens).ink3)
        .boxed()
}

/// `content` with `tip` floating above it in the shared tooltip card.
fn with_tip<'a>(
    content: Element<'a, Message>,
    tip: Localized,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let tip = container(text(tip).size(TS.caption).color(ti(tokens.text)))
        .padding([SP.xs, SP.s])
        .style(crate::styles::context_menu(tokens));
    signex_widgets::tooltip(content, tip, iced::widget::tooltip::Position::Top)
        .gap(SP.xs)
        .boxed()
}

/// A transparent square button (`.ib`): no fill at rest, the hover overlay
/// under the pointer, small radius.
fn icon_button_style(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let hover = overlay(tokens, Overlay::Hover);
    let pressed = overlay(tokens, Overlay::Pressed);
    move |_: &Theme, status: button::Status| button::Style {
        background: match status {
            button::Status::Hovered => Some(Background::Color(hover)),
            button::Status::Pressed => Some(Background::Color(pressed)),
            _ => None,
        },
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// A 24 px icon button with a tooltip; `None` dims it and leaves it inert.
pub(in crate::app::project_tab) fn icon_button<'a>(
    g: Glyph,
    tip: Localized,
    on_press: Option<Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink = ti(tokens.text_secondary);
    let ink = if on_press.is_some() { ink } else { dimmed(ink) };
    let face = container(glyphs::glyph(g, ICON, ink))
        .center_x(Length::Fill)
        .center_y(Length::Fill);
    let pressable = button(face)
        .width(Length::Fixed(ICON_BUTTON))
        .height(Length::Fixed(ICON_BUTTON))
        .padding(0.0)
        .style(icon_button_style(tokens))
        .on_press_maybe(on_press)
        .boxed();
    with_tip(pressable, tip, tokens)
}

/// The kind of a [`text_button`] (`.btn` and its variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app::project_tab) enum ButtonKind {
    /// The filled primary action.
    Primary,
    /// The primary action of a footer or an empty state, taller.
    PrimaryLarge,
    /// A neutral action with a hairline border.
    Outline,
    /// A small outlined action in a row or a line.
    Small,
    /// A borderless action.
    Ghost,
}

impl ButtonKind {
    /// `(height, horizontal padding, text size)`.
    pub(in crate::app::project_tab) const fn metrics(self) -> (f32, f32, f32) {
        match self {
            ButtonKind::Primary | ButtonKind::Outline => {
                (TS.body * 13.0 / 6.0, SP.m + SP.xs * 0.5, TS.body)
            }
            ButtonKind::PrimaryLarge => (TS.body * 2.5, SP.l + SP.xs * 0.5, TS.body),
            ButtonKind::Small | ButtonKind::Ghost => (TS.body * 11.0 / 6.0, SP.m, TS.caption),
        }
    }
}

/// A text button with an optional leading glyph; `None` greys it.
pub(in crate::app::project_tab) fn text_button<'a>(
    g: Option<Glyph>,
    words: Localized,
    on_press: Option<Message>,
    kind: ButtonKind,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (height, pad_x, size) = kind.metrics();
    let ink = match kind {
        ButtonKind::Primary | ButtonKind::PrimaryLarge => resolve(tokens).primary_ink,
        ButtonKind::Outline | ButtonKind::Small | ButtonKind::Ghost => ti(tokens.text),
    };
    let ink = if on_press.is_some() { ink } else { dimmed(ink) };
    let mut face: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.s + SP.xs * 0.5)
        .align_y(Alignment::Center);
    if let Some(g) = g {
        face = face.push(glyphs::glyph(g, size + SP.xs, ink));
    }
    face = face.push(
        text(words)
            .size(size)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
    );
    let pressable = button(
        container(face)
            .height(Length::Fill)
            .align_y(Vertical::Center)
            .align_x(Horizontal::Center),
    )
    .height(Length::Fixed(height))
    .padding([0.0, pad_x])
    .on_press_maybe(on_press);
    let styled = match kind {
        ButtonKind::Primary | ButtonKind::PrimaryLarge => {
            pressable.style(crate::styles::primary_button(tokens))
        }
        ButtonKind::Outline | ButtonKind::Small => {
            pressable.style(crate::styles::outline_button(tokens))
        }
        ButtonKind::Ghost => pressable.style(crate::styles::ghost_button(tokens)),
    };
    styled.boxed()
}
