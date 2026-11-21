//! Pieces the Start screen's pages and the project hint share: buttons in
//! the mock's three looks and sizes, an icon glyph, the filter field, the
//! page heading, the count pill, list cards and a filtered list's empty
//! state.

use iced::widget::{Space, column, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::{button, text, text_input};

use crate::app::Message;
use crate::styles::{
    self, ChromeMetrics, PillTone, Radii, Spacing, TypeScale, ti,
    tokens::{composite_over, hover_overlay, resolve},
};

/// A button style chosen at run time from one of the looks.
type ButtonStyleFn = Box<dyn Fn(&iced::Theme, button::Status) -> button::Style>;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Edge of an icon in a row or a button (14 px at the default base).
pub(super) const ICON: f32 = TS.body * 7.0 / 6.0;
/// Theme the glyphs are fetched for. Every glyph is tinted with an ink
/// token, which hides the theme's accent swap, so any theme gives the same.
pub(super) const GLYPH_THEME: ThemeId = ThemeId::GraphiteDark;
/// Widest a page's filter field gets (340 px at the default base).
const FILTER_MAX_W: f32 = TS.body * 85.0 / 3.0;
/// Narrowest a page's filter field gets.
const FILTER_MIN_W: f32 = TS.body * 10.0;
/// Size of a page title ("Projects", "Libraries").
pub(super) const PAGE_TITLE_SIZE: f32 = TS.display;

/// A monochrome glyph `side` px square, tinted `ink`.
pub(super) fn icon<'a>(handle: svg::Handle, side: f32, ink: Color) -> Element<'a, Message> {
    svg(handle)
        .width(side)
        .height(side)
        .style(move |_: &iced::Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// The UI face at a heavier weight.
pub(super) fn weighted(weight: iced::font::Weight) -> iced::Font {
    iced::Font {
        weight,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

pub(super) fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// The message of command `id` run as a menu row runs it, `None` when the
/// catalog does not know the id (the button is then greyed).
pub(super) fn command(id: &'static str) -> Option<Message> {
    crate::keymap::AppCommandId::new(id)
        .ok()
        .map(|id| Message::Menu(crate::menu_bar::MenuMessage::Command(id)))
}

/// The mock's button looks: bordered, filled primary, and a borderless
/// link in the accent ink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Look {
    Outline,
    Primary,
    Ghost,
}

/// The mock's button sizes: `.btn.sm` (22 px) and `.btn` (26 px). The
/// navigation's two `.btn.lg` buttons are built in `graphite.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Size {
    Small,
    Regular,
}

impl Size {
    fn height(self) -> f32 {
        match self {
            Size::Small => TS.body * 11.0 / 6.0,
            Size::Regular => ChromeMetrics::DEFAULT.field,
        }
    }

    fn pad_x(self) -> f32 {
        match self {
            Size::Small => SP.m,
            Size::Regular => SP.m + SP.xs,
        }
    }

    fn text(self) -> f32 {
        match self {
            Size::Small => TS.caption,
            Size::Regular => TS.body,
        }
    }

    fn radius(self) -> f32 {
        match self {
            Size::Small => Radii::GRAPHITE.small,
            Size::Regular => Radii::GRAPHITE.button,
        }
    }
}

/// Quiet bordered button ("Open…"): raised fill, hairline border.
pub(super) fn outline_button(
    tokens: &ThemeTokens,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style + 'static {
    let (raised, hover, line, ink) = (
        resolve(tokens).raised,
        hover_overlay(tokens),
        ti(tokens.border),
        ti(tokens.text),
    );
    move |_, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => composite_over(hover, raised),
            _ => raised,
        })),
        text_color: ink,
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Borderless link button in the accent ink ("Show all", "Clear filter").
pub(super) fn ghost_link(
    tokens: &ThemeTokens,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style + 'static {
    let (ink, hover, muted) = (
        resolve(tokens).accent_text,
        hover_overlay(tokens),
        resolve(tokens).ink4,
    );
    move |_, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(hover)),
        text_color: if matches!(status, button::Status::Disabled) {
            muted
        } else {
            ink
        },
        border: Border {
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// A button in `look` and `size`, with an optional leading glyph; greyed
/// and inert without `on_press`.
pub(super) fn action_button<'a>(
    tokens: &ThemeTokens,
    look_size: (Look, Size),
    glyph: Option<svg::Handle>,
    label: Localized,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    sized_button(tokens, look_size, glyph, label, on_press, true)
}

/// The room [`action_button`] takes, drawn as nothing: a row reserves it
/// for a button that only shows on hover, so the row's text never runs
/// under the button.
pub(super) fn reserved_button<'a>(
    tokens: &ThemeTokens,
    look_size: (Look, Size),
    glyph: Option<svg::Handle>,
    label: Localized,
) -> Element<'a, Message> {
    sized_button(tokens, look_size, glyph, label, None, false)
}

fn sized_button<'a>(
    tokens: &ThemeTokens,
    (look, size): (Look, Size),
    glyph: Option<svg::Handle>,
    label: Localized,
    on_press: Option<Message>,
    visible: bool,
) -> Element<'a, Message> {
    let ink = match look {
        _ if !visible => Color::TRANSPARENT,
        Look::Outline => ti(tokens.text),
        Look::Primary => resolve(tokens).primary_ink,
        Look::Ghost => resolve(tokens).accent_text,
    };
    let mut content = row![]
        .spacing(SP.s + SP.xs / 2.0)
        .align_y(Alignment::Center);
    if let Some(glyph) = glyph {
        content = content.push(icon(glyph, size.text() * 7.0 / 6.0, ink));
    }
    content = content.push(
        text(label)
            .size(size.text())
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
    );
    let radius = size.radius();
    let style: ButtonStyleFn = match look {
        Look::Outline => Box::new(outline_button(tokens)),
        Look::Primary => Box::new(styles::primary_button(tokens)),
        Look::Ghost => Box::new(ghost_link(tokens)),
    };
    let mut b = button(container(content).center_y(Length::Fill))
        .height(size.height())
        .padding([0.0, size.pad_x()])
        .style(move |theme: &iced::Theme, status| {
            if !visible {
                return button::Style {
                    text_color: Color::TRANSPARENT,
                    ..button::Style::default()
                };
            }
            let mut s = style(theme, status);
            s.border.radius = radius.into();
            s
        });
    if let Some(message) = on_press {
        b = b.on_press(message);
    }
    b.boxed()
}

/// A section heading; the catalog writes it in capitals. Semibold as in the
/// mock; iced has no letter spacing, so the mock's tracking is not drawn.
pub(super) fn section_header<'a>(tokens: &ThemeTokens, title: Localized) -> Element<'a, Message> {
    text(title)
        .size(TS.section)
        .font(weighted(iced::font::Weight::Semibold))
        .style(styles::section_label(tokens))
        .boxed()
}

/// A section heading with its count pill and, at the right, `link`.
pub(super) fn section_header_with<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    title: Localized,
    count: usize,
    link: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut line = row![section_header(tokens, title)]
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .width(Length::Fill);
    if count > 0 {
        line = line.push(count_pill(tokens, i18n, count));
    }
    if let Some(link) = link {
        line = line
            .push(Space::new().width(Length::Fill).boxed())
            .push(link);
    }
    line.boxed()
}

/// A page's title and the line under it.
pub(super) fn page_heading<'a>(
    tokens: &ThemeTokens,
    title: Localized,
    body: Localized,
) -> Element<'a, Message> {
    column![
        text(title)
            .size(PAGE_TITLE_SIZE)
            .font(weighted(iced::font::Weight::Semibold))
            .color(ti(tokens.text)),
        text(body).size(TS.label).color(resolve(tokens).ink3),
    ]
    .spacing(SP.s + SP.xs / 2.0)
    .boxed()
}

/// Width of a page's filter field in a main column `main_w` wide.
pub(super) fn filter_width(main_w: f32) -> f32 {
    (main_w * 0.4).clamp(FILTER_MIN_W, FILTER_MAX_W)
}

/// The filter field of a list page: a search glyph and a borderless input
/// in a field frame.
pub(super) fn filter_field<'a>(
    tokens: &ThemeTokens,
    placeholder: Localized,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    width: f32,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let (ink, ink4, accent) = (ti(tokens.text), r.ink4, ti(tokens.accent));
    let input = text_input(placeholder, value)
        .on_input(on_input)
        .padding(Padding::ZERO)
        .size(TS.body)
        .width(Length::Fill)
        .style(move |_: &iced::Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..accent },
        });
    container(
        row![
            icon(crate::icons::icon_chrome_search(GLYPH_THEME), ICON, ink4),
            input.boxed()
        ]
        .spacing(SP.s + SP.xs / 2.0)
        .align_y(Alignment::Center),
    )
    .padding([0.0, SP.m])
    .width(width)
    .height(ChromeMetrics::DEFAULT.field)
    .center_y(ChromeMetrics::DEFAULT.field)
    .style(styles::field(tokens, false))
    .boxed()
}

/// A count in a small neutral pill (a section heading's count).
pub(super) fn count_pill<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    count: usize,
) -> Element<'a, Message> {
    pill(
        tokens,
        t!(i18n, ids::START_COUNT, "count" => count),
        PillTone::Neutral,
    )
}

/// `label` in a small pill of `tone`.
pub(super) fn pill<'a>(
    tokens: &ThemeTokens,
    label: Localized,
    tone: PillTone,
) -> Element<'a, Message> {
    container(
        text(label)
            .size(TS.caption)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([0.0, SP.s + SP.xs / 2.0])
    .height(TS.body * 4.0 / 3.0)
    .center_y(TS.body * 4.0 / 3.0)
    .style(styles::pill(tokens, tone))
    .boxed()
}

/// One card holding `rows`, with a hairline between rows.
pub(super) fn list_card<'a>(
    tokens: &ThemeTokens,
    rows: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let line = resolve(tokens).line_soft;
    let count = rows.len();
    let mut list = column![].width(Length::Fill);
    for (at, entry) in rows.into_iter().enumerate() {
        list = list.push(entry);
        if at + 1 < count {
            list = list.push(hairline(line));
        }
    }
    container(list)
        .width(Length::Fill)
        .padding(1)
        .style(styles::card(tokens))
        .boxed()
}

/// A full-width 1 px line in `line`.
pub(super) fn hairline<'a>(line: Color) -> Element<'a, Message> {
    container(Space::new().height(styles::HAIRLINE))
        .width(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}

/// The empty state of a list: a title, a line under it and, for a filter
/// that matches nothing, the Clear filter link.
pub(super) fn empty_state<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    (title, body): (Localized, Localized),
    clear: bool,
) -> Element<'a, Message> {
    let mut lines = column![
        text(title)
            .size(TS.body)
            .font(weighted(iced::font::Weight::Medium))
            .color(ti(tokens.text))
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
        text(body)
            .size(TS.body)
            .color(resolve(tokens).ink3)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
    ]
    .spacing(SP.xs)
    .align_x(Alignment::Center)
    .width(Length::Fill);
    if clear {
        lines = lines.push(action_button(
            tokens,
            (Look::Ghost, Size::Small),
            None,
            t!(i18n, ids::START_CLEAR_FILTER),
            Some(super::message(super::StartScreenMsg::FilterCleared)),
        ));
    }
    container(lines)
        .padding([SP.xl + SP.s, SP.l])
        .width(Length::Fill)
        .boxed()
}
