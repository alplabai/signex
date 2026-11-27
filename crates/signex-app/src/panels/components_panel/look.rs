//! Graphite look of the Components panel: the filter field, source chips,
//! the add menu, library and component rows, empty states and separators.

use iced::widget::{container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, text_input};

use crate::library::messages::LibraryMessage;
use crate::library::state::ComponentsMountSource;
use crate::styles::tokens::{hover_overlay, resolve};
use crate::styles::{TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// Height of a library or component row.
pub(super) const ROW_HEIGHT: f32 = 26.0;
/// Height of the filter field.
const FILTER_HEIGHT: f32 = 28.0;
/// Gap around the filter field and at the right edge of every row.
pub(super) const EDGE: f32 = 10.0;
/// Gap between the search glyph and the input.
const CHEVRON_GAP: f32 = 6.0;

const SEARCH_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><circle cx="6" cy="6" r="4.25" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M9.25 9.25L12.5 12.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;

/// The mono face part numbers are set in.
pub(super) fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// A monochrome glyph tinted `ink`.
fn glyph(bytes: &'static [u8], side: f32, ink: Color) -> iced::widget::Svg<'static> {
    svg(svg::Handle::from_memory(bytes))
        .width(side)
        .height(side)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
}

/// The filter field: a search glyph and a borderless input in a field frame.
pub(super) fn filter_field<'a>(
    value: &'a str,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let (ink, ink4, accent) = (ti(tokens.text), r.ink4, ti(tokens.accent));
    let input = text_input(t!(i18n, ids::PANELS_COMPONENTS_FILTER), value)
        .on_input(LibraryMessage::ComponentsPanelSetFilter)
        .padding(Padding::ZERO)
        .size(TS.body)
        .width(Length::Fill)
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..accent },
        });
    let field = container(
        row![glyph(SEARCH_GLYPH, 14.0, ink4).boxed(), input.boxed()]
            .spacing(CHEVRON_GAP)
            .align_y(Alignment::Center),
    )
    .padding([0.0, EDGE])
    .height(FILTER_HEIGHT)
    .center_y(FILTER_HEIGHT)
    .width(Length::Fill)
    .style(crate::styles::field(tokens, false));
    // The header row holds the side insets.
    container(field)
        .padding([8.0, 0.0])
        .width(Length::Fill)
        .boxed()
}

/// The info line under the header that says what each bare number of the
/// search matched, with part counts: `100  100 pF 8 · 100 nF 8 · name or
/// text 2` (components-cx decisions 6 and 9). `None` without one.
pub(super) fn match_summary<'a>(
    lines: &[super::query::MatchLine],
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Option<Element<'a, LibraryMessage>> {
    /// Values listed before "+N more".
    const SHOWN: usize = 5;
    let r = resolve(tokens);
    let ink = r.info;
    let item = |label: Localized| -> Element<'a, LibraryMessage> {
        text(label).size(TS.caption).color(ink).boxed()
    };
    let sep = || -> Element<'a, LibraryMessage> {
        text(t!(i18n, ids::PANELS_COMPONENTS_MATCH_SEPARATOR))
            .size(TS.caption)
            .color(Color { a: 0.55, ..ink })
            .boxed()
    };
    let mut col = iced::widget::Column::new().spacing(super::SP.xs);
    for line in lines {
        if line.values.is_empty() && line.by_name == 0 {
            continue;
        }
        let mut items: Vec<Element<'a, LibraryMessage>> = line
            .values
            .iter()
            .take(SHOWN)
            .map(|(value, count)| {
                item(t!(
                    i18n,
                    ids::PANELS_COMPONENTS_MATCH_VALUE,
                    "value" => value.as_str(),
                    "count" => *count
                ))
            })
            .collect();
        if line.values.len() > SHOWN {
            items.push(item(t!(
                i18n,
                ids::PANELS_COMPONENTS_MATCH_MORE,
                "count" => line.values.len() - SHOWN
            )));
        }
        if line.by_name > 0 {
            items.push(item(t!(
                i18n,
                ids::PANELS_COMPONENTS_MATCH_BY_NAME,
                "count" => line.by_name
            )));
        }
        let mut words = iced::widget::Row::new()
            .spacing(super::SP.xs)
            .align_y(Alignment::Center)
            .push(
                text(signex_i18n::UserText::new(line.word.clone()))
                    .size(TS.caption)
                    .color(ink)
                    .font(Font {
                        weight: iced::font::Weight::Semibold,
                        ..Font::DEFAULT
                    })
                    .boxed(),
            );
        for (i, entry) in items.into_iter().enumerate() {
            if i > 0 {
                words = words.push(sep());
            }
            words = words.push(entry);
        }
        col = col.push(words.wrap().boxed());
    }
    if lines.iter().all(|l| l.values.is_empty() && l.by_name == 0) {
        return None;
    }
    let fill = r.info_soft;
    Some(
        container(col)
            .padding([super::SP.xs, EDGE])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                ..container::Style::default()
            })
            .boxed(),
    )
}

/// Width of a row symbol thumbnail.
pub(super) const THUMB_WIDTH: f32 = TS.body * 3.0;
/// Height of a row symbol thumbnail.
pub(super) const THUMB_HEIGHT: f32 = TS.body * 2.5;
/// The menu "+ Add" opens: where to add the library.
pub(super) fn add_menu<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, LibraryMessage> {
    let ink = ti(tokens.text);
    let hover = hover_overlay(tokens);
    let mut items = iced::widget::Column::new().spacing(0);
    for source in ComponentsMountSource::ADDABLE {
        items = items.push(
            button(text(t!(i18n, source.message())).size(TS.body).color(ink))
                .padding([5.0, 10.0])
                .width(Length::Fill)
                .on_press(LibraryMessage::ComponentsPanelAddLibrary(*source))
                .style(move |_: &Theme, status: button::Status| button::Style {
                    background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                        .then_some(Background::Color(hover)),
                    text_color: ink,
                    border: Border {
                        radius: 6.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }),
        );
    }
    container(
        container(items)
            .padding(4.0)
            .width(Length::Fill)
            .style(crate::styles::card(tokens)),
    )
    .padding([0.0, EDGE])
    .width(Length::Fill)
    .boxed()
}

/// Colour of the separators between sections.
pub(super) fn separator(tokens: &ThemeTokens) -> Color {
    resolve(tokens).line_soft
}
