//! Graphite look of the Components panel: the filter field, section headers,
//! library and component rows, empty states and the separators. Legacy
//! themes never reach these builders; each caller branches on
//! `is_graphite`.

use iced::widget::{Space, button, container, row, svg, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_types::theme::ThemeTokens;

use crate::library::messages::LibraryMessage;
use crate::styles::tokens::{hover_overlay, resolve};
use crate::styles::{PillTone, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// Height of a library or component row.
pub(super) const ROW_HEIGHT: f32 = 26.0;
/// Height of a section header.
const SECTION_HEIGHT: f32 = 30.0;
/// Height of the filter field.
const FILTER_HEIGHT: f32 = 28.0;
/// Gap around the filter field and at the right edge of every row.
pub(super) const EDGE: f32 = 10.0;
/// Side of the section chevron.
const CHEVRON: f32 = signex_widgets::chevron::side(TS.body);
/// Gap between the chevron, the label and the count pill.
const CHEVRON_GAP: f32 = 6.0;
/// Left inset of library headers and empty states: under the section label.
pub(super) const LIBRARY_INSET: f32 = EDGE + CHEVRON + CHEVRON_GAP;
/// Left inset of a component row: one step in from its library.
pub(super) const ROW_INSET: f32 = LIBRARY_INSET + 12.0;

const SEARCH_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><circle cx="6" cy="6" r="4.25" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M9.25 9.25L12.5 12.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;

/// The weight-600 face of the UI font.
fn semibold() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..Font::DEFAULT
    }
}

/// The mono face part numbers are set in.
pub(super) fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// A monochrome glyph tinted `ink`.
fn glyph(bytes: &'static [u8], side: f32, ink: Color) -> iced::widget::Svg<'static> {
    svg(svg::Handle::from_memory(bytes))
        .width(side)
        .height(side)
        .style(move |_: &Theme, _| svg::Style { color: Some(ink) })
}

/// The filter field: a search glyph and a borderless input in a field frame.
pub(super) fn filter_field<'a>(
    value: &'a str,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let (ink, ink4, accent) = (ti(tokens.text), r.ink4, ti(tokens.accent));
    let input = text_input("Filter components…", value)
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
    container(field)
        .padding([8.0, EDGE])
        .width(Length::Fill)
        .boxed()
}

/// A section header: stroke chevron, weight-600 label and a count pill as
/// one toggle, then the optional "+ Add Library…" ghost button.
pub(super) fn section_header<'a>(
    label: &'a str,
    count: usize,
    collapsed: bool,
    toggle: LibraryMessage,
    add: Option<LibraryMessage>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let hover = hover_overlay(tokens);
    let chevron = signex_widgets::chevron::chevron(!collapsed, TS.body, r.ink3);
    let title = row![
        chevron.boxed(),
        text(label)
            .size(TS.body)
            .font(semibold())
            .color(ink)
            .boxed(),
        crate::library::editor::kit::toned_pill(count.to_string(), PillTone::Neutral, tokens),
    ]
    .spacing(CHEVRON_GAP)
    .align_y(Alignment::Center);
    let toggle = button(container(title).center_y(Length::Fill))
        .padding([0.0, EDGE])
        .width(Length::Fill)
        .height(SECTION_HEIGHT)
        .on_press(toggle)
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(Background::Color(hover)),
            text_color: ink,
            ..button::Style::default()
        });
    let mut header: iced::widget::Row<Element<'a, LibraryMessage>> =
        row![toggle.boxed()].align_y(Alignment::Center);
    if let Some(add) = add {
        let ghost = button(text("+ Add Library…").size(TS.caption).color(ink))
            .padding([2.0, 8.0])
            .on_press(add)
            .style(|theme: &Theme, status| {
                crate::panels::form_look::ghost_button(theme, status)
                    .unwrap_or_else(|| button::text(theme, status))
            });
        header = header.push(
            container(ghost)
                .padding(Padding {
                    right: EDGE,
                    ..Padding::ZERO
                })
                .boxed(),
        );
    }
    header.boxed()
}

/// A secondary empty-state line under a section header.
pub(super) fn empty_line<'a>(
    message: &'a str,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    container(text(message).size(TS.caption).color(resolve(tokens).ink4))
        .padding(Padding {
            top: 0.0,
            right: EDGE,
            bottom: 8.0,
            left: LIBRARY_INSET,
        })
        .width(Length::Fill)
        .boxed()
}

/// A library header inside a section: its name and its row count.
pub(super) fn library_header<'a>(
    name: String,
    rows: usize,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    container(
        row![
            text(name)
                .size(TS.body)
                .color(r.ink3)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
            text(rows.to_string())
                .size(TS.caption)
                .font(mono())
                .color(r.ink4)
                .boxed(),
        ]
        .spacing(CHEVRON_GAP)
        .align_y(Alignment::Center),
    )
    .padding(Padding {
        top: 0.0,
        right: EDGE,
        bottom: 0.0,
        left: LIBRARY_INSET,
    })
    .height(ROW_HEIGHT)
    .center_y(ROW_HEIGHT)
    .width(Length::Fill)
    .boxed()
}

/// Colour of the separators between sections.
pub(super) fn separator(tokens: &ThemeTokens) -> Color {
    resolve(tokens).line_soft
}

/// A thin spacer row under the last line of a section body.
pub(super) fn section_gap<'a>() -> Element<'a, LibraryMessage> {
    Space::new().height(4.0).boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_chevron_box_is_the_shared_one_for_the_body_size() {
        assert_eq!(CHEVRON, signex_widgets::chevron::side(TS.body));
        assert_eq!(LIBRARY_INSET, EDGE + CHEVRON + CHEVRON_GAP);
    }
}
