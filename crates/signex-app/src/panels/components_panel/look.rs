//! Graphite look of the Components panel: the filter field, source chips,
//! the add menu, library and component rows, empty states and separators.

use iced::widget::{container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text, text_input};

use crate::library::messages::LibraryMessage;
use crate::library::state::ComponentsMountSource;
use crate::styles::tokens::{hover_overlay, resolve};
use crate::styles::{PillTone, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// Height of a library or component row.
pub(super) const ROW_HEIGHT: f32 = 26.0;
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

const PLUS_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12"><path d="M6 2v8M2 6h8" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;

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
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let (ink, ink4, accent) = (ti(tokens.text), r.ink4, ti(tokens.accent));
    let input = text_input(raw("Filter components…"), value)
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

/// Height of a source chip.
const CHIP_HEIGHT: f32 = 22.0;

/// The filter row: the search field and the "+ Add" ghost button.
pub(super) fn header_row<'a>(
    filter: Element<'a, LibraryMessage>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ink = ti(tokens.text);
    let add = button(
        row![
            glyph(PLUS_GLYPH, 12.0, ink),
            text(raw("Add")).size(TS.caption).color(ink)
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding([3.0, 8.0])
    .on_press(LibraryMessage::ComponentsPanelAddMenuToggled)
    .style(crate::panels::form_look::ghost_button);
    row![
        container(filter).width(Length::Fill),
        container(add).padding(Padding {
            right: EDGE,
            ..Padding::ZERO
        })
    ]
    .align_y(Alignment::Center)
    .boxed()
}

/// The menu "+ Add" opens: where to add the library.
pub(super) fn add_menu<'a>(tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let ink = ti(tokens.text);
    let hover = hover_overlay(tokens);
    let mut items = iced::widget::Column::new().spacing(0);
    for source in ComponentsMountSource::ADDABLE {
        items = items.push(
            button(text(raw(source.label())).size(TS.body).color(ink))
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

/// One source chip: its name and library count; the chosen one is lit.
pub(super) fn source_chip<'a>(
    label: &'static str,
    count: usize,
    selected: bool,
    on_press: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let ink = if selected { ti(tokens.text) } else { r.ink3 };
    let (fill, line) = if selected {
        (r.accent_soft, ti(tokens.accent))
    } else {
        (Color::TRANSPARENT, ti(tokens.border))
    };
    let hover = r.accent_soft;
    button(
        row![
            text(raw(label)).size(TS.caption).color(ink),
            text(raw(count.to_string()))
                .size(TS.caption)
                .font(mono())
                .color(r.ink4),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding([0.0, 8.0])
    .height(CHIP_HEIGHT)
    .on_press(on_press)
    .style(move |_: &Theme, status: button::Status| button::Style {
        background: if selected {
            Some(Background::Color(fill))
        } else {
            matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(Background::Color(hover))
        },
        text_color: ink,
        border: Border {
            width: 1.0,
            color: line,
            radius: 11.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .boxed()
}

/// The source chips under the search box, wrapped to the dock width.
pub(super) fn chip_row<'a>(chips: Vec<Element<'a, LibraryMessage>>) -> Element<'a, LibraryMessage> {
    let mut wrap = iced_aw::Wrap::new().spacing(6.0).line_spacing(6.0);
    for chip in chips {
        wrap = wrap.push(chip);
    }
    container(wrap)
        .padding([0.0, EDGE])
        .width(Length::Fill)
        .boxed()
}

/// A secondary empty-state line.
pub(super) fn empty_line<'a>(
    message: &'a str,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    container(
        text(raw(message))
            .size(TS.caption)
            .color(resolve(tokens).ink4),
    )
    .padding(Padding {
        top: 0.0,
        right: EDGE,
        bottom: 8.0,
        left: LIBRARY_INSET,
    })
    .width(Length::Fill)
    .boxed()
}

/// A library row: expand chevron, name, source badge and component count.
pub(super) fn library_header<'a>(
    root: std::path::PathBuf,
    name: String,
    source: ComponentsMountSource,
    rows: usize,
    collapsed: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let hover = hover_overlay(tokens);
    let ink = ti(tokens.text);
    let content = row![
        signex_widgets::chevron::chevron(!collapsed, TS.body, r.ink3),
        text(signex_i18n::UserText::new(name))
            .size(TS.body)
            .color(ink)
            .width(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::None),
        crate::library::editor::kit::toned_pill(
            // user-visible: migrate to the #2063 casemap
            source.label().to_uppercase(),
            PillTone::Neutral,
            tokens
        ),
        text(raw(rows.to_string()))
            .size(TS.caption)
            .font(mono())
            .color(r.ink4),
    ]
    .spacing(CHEVRON_GAP)
    .align_y(Alignment::Center);
    button(content)
        .padding(Padding {
            top: 0.0,
            right: EDGE,
            bottom: 0.0,
            left: EDGE,
        })
        .height(ROW_HEIGHT)
        .width(Length::Fill)
        .on_press(LibraryMessage::ComponentsPanelToggleLibrary(root))
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(Background::Color(hover)),
            text_color: ink,
            ..button::Style::default()
        })
        .boxed()
}

/// Colour of the separators between sections.
pub(super) fn separator(tokens: &ThemeTokens) -> Color {
    resolve(tokens).line_soft
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
