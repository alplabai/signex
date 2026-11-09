//! Graphite look of the Components panel details: the header card with the
//! lifecycle pill, the section labels and the parameter rows.

use iced::widget::{Column, column, container, row};
use iced::{Alignment, Element, Font, Length, Padding, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::ComponentDetails;
use crate::library::editor::kit::{lifecycle_label, lifecycle_tone, toned_pill};
use crate::library::messages::LibraryMessage;
use crate::styles::tokens::resolve;
use crate::styles::{TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// Height of a parameter row.
const PARAMETER_ROW_HEIGHT: f32 = 24.0;

fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// The header card: mono part number with its lifecycle pill, then the
/// class, the maker and the description.
pub(super) fn header_card<'a>(
    d: &'a ComponentDetails,
    maker: String,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let title = row![
        text(signex_i18n::UserText::new(d.internal_pn.as_str()))
            .size(TS.body)
            .font(Font {
                weight: iced::font::Weight::Semibold,
                ..mono()
            })
            .color(ink)
            .width(Length::Fill)
            .boxed(),
        toned_pill(
            lifecycle_label(d.state).to_string(),
            lifecycle_tone(d.state),
            tokens
        ),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let mut card: Column<Element<'a, LibraryMessage>> = column![title].spacing(4);
    if !d.class.is_empty() {
        card = card.push(
            text(signex_i18n::UserText::new(d.class.as_str()))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    }
    if !maker.is_empty() {
        card = card.push(
            text(signex_i18n::UserText::new(maker))
                .size(TS.body)
                .color(ink)
                .boxed(),
        );
    }
    if !d.description.trim().is_empty() {
        card = card.push(
            text(signex_i18n::UserText::new(d.description.as_str()))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    }
    container(card)
        .padding(10)
        .width(Length::Fill)
        .style(crate::styles::card(tokens))
        .boxed()
}

/// A section label over a preview or the parameters; the catalog writes it
/// in capitals.
pub(super) fn section_label<'a>(
    label: signex_i18n::Localized,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    container(text(label).size(TS.section).color(resolve(tokens).ink4))
        .padding(Padding {
            top: 6.0,
            ..Padding::ZERO
        })
        .boxed()
}

/// One parameter: muted name, mono value.
pub(super) fn parameter_row<'a>(
    name: &'a str,
    value: &'a str,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    container(
        row![
            text(signex_i18n::UserText::new(name))
                .size(TS.caption)
                .color(resolve(tokens).ink3)
                .width(Length::FillPortion(2))
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
            text(signex_i18n::UserText::new(value))
                .size(TS.caption)
                .font(mono())
                .color(ti(tokens.text))
                .width(Length::FillPortion(3))
                .boxed(),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .height(PARAMETER_ROW_HEIGHT)
    .center_y(PARAMETER_ROW_HEIGHT)
    .width(Length::Fill)
    .boxed()
}
