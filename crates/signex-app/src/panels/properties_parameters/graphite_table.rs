//! Graphite look of the Parameters table: a hairline card, a muted sunken
//! name column and mono values.

use super::super::*;
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale, ti};
use iced::Widget;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the name column.
const NAME_COLUMN_WIDTH: f32 = 110.0;
/// Corner radius of the table card.
const TABLE_RADIUS: f32 = 8.0;

/// The name / value rows in a rounded hairline card.
pub fn parameter_table<'a>(tokens: &ThemeTokens, rows: &[(&str, &str)]) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let (line, line_soft, sunken) = (ti(tokens.border), r.line_soft, r.sunken);
    let (ink, ink3) = (ti(tokens.text), r.ink3);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let mut table: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    for (name, value) in rows {
        let name_cell = container(
            text(signex_i18n::UserText::new(name.to_string()))
                .size(TS.dense)
                .color(ink3),
        )
        .padding([5.0, 8.0])
        .width(NAME_COLUMN_WIDTH)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            ..container::Style::default()
        });
        let value_cell = container(
            text(raw(value.to_string()))
                .size(TS.dense)
                .color(ink)
                .font(mono)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .padding([5.0, 8.0])
        .width(Length::Fill)
        .clip(true);
        table = table.push(
            row![name_cell.boxed(), value_cell.boxed()]
                .width(Length::Fill)
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
