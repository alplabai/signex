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
