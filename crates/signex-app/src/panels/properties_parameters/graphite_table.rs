//! Graphite look of the Parameters table: a hairline card, a muted sunken
//! name column and the value cells beside it.

use super::super::*;
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale, ti};
use iced::Widget;
use signex_widgets::Label;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the name column in `dense` font sizes, so it follows the font.
const NAME_COLUMN_EMS: f32 = 10.0;
/// Width of the name column. A longer name wraps inside it, by word and
/// then by glyph, and never reaches the value cell.
const NAME_COLUMN_WIDTH: f32 = TS.dense * NAME_COLUMN_EMS;
/// Corner radius of the table card.
const TABLE_RADIUS: f32 = 8.0;

/// One row of the Parameters table: its name and its value cell.
pub(super) struct ParameterRow<'a> {
    pub(super) name: Label<'a>,
    pub(super) value: Element<'a, PanelMsg>,
}

/// A value cell that cannot be edited: mono text, wrapped rather than cut.
pub(super) fn read_only_value<'a>(tokens: &ThemeTokens, value: String) -> Element<'a, PanelMsg> {
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    container(
        text(signex_i18n::UserText::new(value))
            .size(TS.dense)
            .color(ti(tokens.text))
            .font(mono)
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
    )
    .padding([5.0, 8.0])
    .width(Length::Fill)
    .clip(true)
    .boxed()
}

/// An editable value cell that reads inline (mockup row 40): no box until
/// hover or focus, and while it is empty it shows `placeholder`, in muted
/// italic when `is_unset` says no value stands behind it ("Not set").
pub(super) fn inline_value_input<'a>(
    placeholder: Label<'a>,
    value: String,
    is_unset: bool,
) -> iced::widget::TextInput<'a, PanelMsg> {
    let reads_not_set = is_unset && value.is_empty();
    let input = signex_widgets::text_input(placeholder, value)
        .size(TS.body)
        .padding(SP.xs)
        .style(crate::panels::form_look::inline_value_input_style)
        .width(Length::Fill);
    if !reads_not_set {
        return input;
    }
    input.font(iced::Font {
        style: iced::font::Style::Italic,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    })
}

/// The header of a Parameters group: chevron, name, row count and an
/// optional note at the right ("read-only"); a press folds the group.
pub(super) fn group_header<'a>(
    ctx: &PanelContext,
    key: &'static str,
    title: signex_i18n::Localized,
    count: usize,
    note: Option<signex_i18n::Localized>,
) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let is_open = !ctx.collapsed_sections.contains(key);
    let count = u32::try_from(count).unwrap_or(u32::MAX);
    let count = signex_widgets::chips::count_text(&ctx.i18n, count);
    let mut line = row![
        signex_widgets::chevron::chevron(is_open, TS.caption, r.ink4),
        text(title)
            .size(TS.body)
            .color(ti(ctx.tokens.text_secondary)),
        text(signex_i18n::UserText::new(count))
            .size(TS.caption)
            .color(r.ink4)
            .font(crate::fonts::iced_font_for_family(
                crate::fonts::CHROME_MONO_FONT
            )),
        Space::new().width(Length::Fill),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    if let Some(note) = note {
        line = line.push(text(note).size(TS.caption).color(r.ink4).boxed());
    }
    signex_widgets::button(container(line).padding([SP.s, SP.m]).width(Length::Fill))
        .padding(0)
        .width(Length::Fill)
        .on_press(PanelMsg::ToggleSection(key.to_string()))
        .style(|_: &Theme, _| iced::widget::button::Style::default())
        .boxed()
}

/// The name / value rows in a rounded hairline card.
pub(super) fn parameter_table<'a>(
    tokens: &ThemeTokens,
    rows: Vec<ParameterRow<'a>>,
) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let (line, line_soft, sunken, ink3) = (ti(tokens.border), r.line_soft, r.sunken, r.ink3);
    let mut table: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    for ParameterRow { name, value } in rows {
        let name_cell = container(
            text(name)
                .size(TS.dense)
                .color(ink3)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
        )
        .padding([5.0, 8.0])
        .width(NAME_COLUMN_WIDTH)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            ..container::Style::default()
        });
        table = table.push(
            row![name_cell.boxed(), value]
                .width(Length::Fill)
                .align_y(iced::Alignment::Center)
                .boxed(),
        );
        table = table.push(
            container(Space::new())
                .height(1.0)
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(line_soft)),
                    ..container::Style::default()
                })
                .boxed(),
        );
    }
    let card = container(table)
        .width(Length::Fill)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: 1.0,
                color: line,
                radius: TABLE_RADIUS.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    container(card.boxed())
        .padding([0.0, SP.m])
        .width(Length::Fill)
        .boxed()
}
