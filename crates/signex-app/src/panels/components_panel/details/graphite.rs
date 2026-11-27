//! Graphite look of the Components panel details: the header card with the
//! lifecycle pill, the section labels and the parameter table.

use iced::widget::{Column, column, container, row};
use iced::{Alignment, Border, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, theme_ext};

use super::ComponentDetails;
use crate::library::editor::kit::{
    TableColumn, lifecycle_label, lifecycle_tone, table_header, table_row, toned_pill,
};
use crate::library::messages::LibraryMessage;
use crate::styles::tokens::resolve;
use crate::styles::{HAIRLINE, Radii, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The parameter table's columns.
const PARAMETER_COLUMNS: [TableColumn; 3] = [
    // The pin star (mockup row 52): no title.
    (None, Length::Fixed(PIN_WIDTH)),
    (
        Some(ids::EDITORS_COMPONENT_COLUMN_NAME),
        Length::FillPortion(2),
    ),
    (
        Some(ids::EDITORS_COMPONENT_COLUMN_VALUE),
        Length::FillPortion(3),
    ),
];
/// Width of the pin star column.
const PIN_WIDTH: f32 = 14.0;
const VALUE_TIP_CHARS: usize = 16;

fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// The header card: mono part number with its lifecycle pill and the
/// primary Place, then "description · library" (`subtitle`, else the bare
/// description), the class and the maker.
pub(super) fn header_card<'a>(
    d: &'a ComponentDetails,
    maker: String,
    subtitle: Option<signex_widgets::Label<'a>>,
    place: Element<'a, LibraryMessage>,
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
        place,
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let mut card: Column<Element<'a, LibraryMessage>> = column![title].spacing(4);
    match subtitle {
        Some(line) => {
            card = card.push(text(line).size(TS.caption).color(r.ink3).boxed());
        }
        None if !d.description.trim().is_empty() => {
            card = card.push(
                text(signex_i18n::UserText::new(d.description.as_str()))
                    .size(TS.caption)
                    .color(r.ink3)
                    .boxed(),
            );
        }
        None => {}
    }
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

/// The parameters as a bordered table: a Name / Value header, then one
/// separated row each, or the empty line. Long cells end in an ellipsis and
/// a long value shows in full in a tooltip. Each row starts with its pin
/// star; a value that is not set is drawn muted.
pub(super) fn parameter_table<'a>(
    parameters: Vec<crate::panels::components_panel::table::DetailLine>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mut table: Column<Element<'a, LibraryMessage>> =
        column![table_header(&PARAMETER_COLUMNS, i18n, tokens)].width(Length::Fill);
    if parameters.is_empty() {
        table = table.push(
            container(
                text(t!(i18n, ids::PANELS_COMPONENTS_DETAILS_NO_PARAMETERS))
                    .size(TS.caption)
                    .color(resolve(tokens).ink4),
            )
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .boxed(),
        );
    }
    let r = resolve(tokens);
    for (index, line) in parameters.into_iter().enumerate() {
        let star = if line.pinned { "\u{2605}" } else { "\u{2606}" };
        let star_ink = if line.pinned {
            r.accent_soft_ink
        } else {
            r.ink4
        };
        let pin = signex_widgets::button(
            text(signex_i18n::UserText::new(star))
                .size(TS.caption)
                .color(star_ink),
        )
        .padding(0)
        .on_press(LibraryMessage::ComponentsTable(
            crate::library::messages::ComponentsTableMsg::PinToggled(line.name.clone()),
        ))
        .style(crate::panels::form_look::ghost_button)
        .boxed();
        let name_cell = one_line(line.name)
            .color(r.ink3)
            .width(Length::Fill)
            .boxed();
        let value = if line.is_set {
            value_cell(line.value, tokens)
        } else {
            one_line(line.value)
                .color(r.ink4)
                .width(Length::Fill)
                .boxed()
        };
        let cells = vec![
            (pin, PARAMETER_COLUMNS[0].1),
            (name_cell, PARAMETER_COLUMNS[1].1),
            (value, PARAMETER_COLUMNS[2].1),
        ];
        table = table.push(table_row(cells, index, None, tokens));
    }
    let line = theme_ext::border_color(tokens);
    container(table)
        .width(Length::Fill)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: HAIRLINE,
                radius: Radii::GRAPHITE.control.into(),
                color: line,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A caption-size cell text on one line, cut with an ellipsis.
fn one_line<'a>(content: String) -> iced::widget::Text<'a> {
    text(signex_i18n::UserText::new(content))
        .size(TS.caption)
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
}

/// The mono value cell, with a tooltip when it may not fit.
fn value_cell<'a>(value: String, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let long = value.chars().count() > VALUE_TIP_CHARS;
    let tip = long.then(|| value.clone());
    let cell: Element<'a, LibraryMessage> = one_line(value)
        .font(mono())
        .color(ti(tokens.text))
        .width(Length::Fill)
        .boxed();
    let Some(tip) = tip else {
        return cell;
    };
    signex_widgets::tooltip(
        cell,
        container(text(signex_i18n::UserText::new(tip)).size(TS.caption))
            .padding([SP.xs, SP.s])
            .style(crate::styles::context_menu(tokens)),
        iced::widget::tooltip::Position::Bottom,
    )
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::{find, lay_out_texts};
    use signex_types::theme::{ThemeId, theme_tokens};

    const WINDOW: iced::Size = iced::Size::new(320.0, 400.0);

    #[test]
    fn the_table_has_a_header_and_one_aligned_row_per_parameter() {
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let i18n = I18n::en_us();
        let parameters = vec![
            ("value".to_string(), "74HC14".to_string()),
            ("tolerance".to_string(), "1%".to_string()),
        ];
        let lines = parameters
            .iter()
            .map(
                |(name, value)| crate::panels::components_panel::table::DetailLine {
                    name: name.clone(),
                    value: value.clone(),
                    is_set: true,
                    pinned: false,
                },
            )
            .collect();
        let texts = lay_out_texts(parameter_table(lines, &tokens, &i18n), WINDOW);
        let (name_head, value_head) = (find(&texts, "NAME"), find(&texts, "VALUE"));
        assert!((name_head.bounds.y - value_head.bounds.y).abs() < 1.0);
        let mut above = name_head.bounds.y;
        for (name, value) in &parameters {
            assert_eq!(texts.iter().filter(|t| t.text == *name).count(), 1);
            let (name, value) = (find(&texts, name), find(&texts, value));
            assert!(name.bounds.y > above, "rows run down under the header");
            assert!((name.bounds.y - value.bounds.y).abs() < 1.0);
            assert!((name.bounds.x - name_head.bounds.x).abs() < 1.0);
            assert!((value.bounds.x - value_head.bounds.x).abs() < 1.0);
            above = name.bounds.y;
        }
    }

    #[test]
    fn an_empty_table_keeps_its_header_and_says_so() {
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let i18n = I18n::en_us();
        let texts = lay_out_texts(parameter_table(Vec::new(), &tokens, &i18n), WINDOW);
        let head = find(&texts, "NAME");
        assert!(find(&texts, "No parameters.").bounds.y > head.bounds.y);
    }
}
