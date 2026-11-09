//! Symbol Pins panel: one table row per pin of the open symbol with the
//! number, name, electrical type, part, visibility, length and orientation.
//! The number cell selects the pin, the name cell edits it, the hidden cell
//! toggles it. All three reuse the Properties panel's pin messages
//! (`SymEditorSelectPin`, `SymEditorSetPinName`, `SymEditorTogglePinHidden`),
//! which already route to the `SymbolEditorMsg` pin edits.

use iced::widget::{Column, Row, container, row};
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text, text_input};

use crate::panels::{PanelContext, PanelMsg, SymbolEditorSelection, SymbolPinSummary};
use crate::styles::{Radii, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Column weights: number, name, electrical type, part, hidden, length,
/// orientation.
const COLUMNS: [(&str, u16); 7] = [
    ("#", 1),
    ("Name", 4),
    ("Electrical type", 4),
    ("Part", 1),
    ("Hidden", 2),
    ("Length (mm)", 2),
    ("Orientation", 3),
];

/// Render the table for the active symbol editor.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let muted = ti(tokens.text_secondary);
    let Some(sym) = ctx.symbol_editor.as_ref() else {
        return container(
            text(raw("Open a `.snxsym` to see its pins here."))
                .size(TS.caption)
                .color(muted),
        )
        .padding([6.0, SP.m])
        .width(Length::Fill)
        .boxed();
    };
    let selected_idx = match &sym.selected {
        SymbolEditorSelection::Pin(pin) => Some(pin.idx),
        _ => None,
    };
    let hidden = sym.pins.iter().filter(|p| p.details.hidden).count();
    let summary = format!(
        "{} pins \u{00B7} {} hidden \u{00B7} {} parts",
        sym.pins.len(),
        hidden,
        sym.active_max_part,
    );

    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    col = col.push(
        container(text(raw(summary)).size(TS.body).color(muted))
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .boxed(),
    );
    col = col.push(header_row(tokens));
    for pin in &sym.pins {
        col = col.push(pin_row(pin, selected_idx == Some(pin.idx), tokens));
    }
    col.boxed()
}

/// The column headings, uppercase and muted.
fn header_row<'a>(tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let ink = resolve(tokens).ink3;
    let mut cells: Row<Element<'a, PanelMsg>> =
        Row::new().spacing(SP.m).align_y(iced::Alignment::Center);
    for (label, portion) in COLUMNS {
        cells = cells.push(cell(
            text(raw(label.to_uppercase()))
                .size(TS.section)
                .color(ink)
                .boxed(),
            portion,
        ));
    }
    container(cells)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(Color { a: 0.04, ..ink })),
            ..container::Style::default()
        })
        .boxed()
}

/// One pin.
fn pin_row<'a>(
    pin: &'a SymbolPinSummary,
    is_selected: bool,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let ink = ti(tokens.text);
    let muted = resolve(tokens).ink3;
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let idx = pin.idx;
    let fill = if is_selected {
        Some(Background::Color(resolve(tokens).accent_soft))
    } else {
        None
    };

    let number = button(
        text(signex_i18n::UserText::new(pin.number.as_str()))
            .size(TS.dense)
            .font(mono)
            .color(ink),
    )
    .padding([SP.xs, SP.s])
    .on_press(PanelMsg::SymEditorSelectPin(idx))
    .style(move |_: &Theme, status| button::Style {
        background: match status {
            button::Status::Hovered => Some(Background::Color(Color { a: 0.08, ..ink })),
            _ => None,
        },
        text_color: ink,
        border: Border::default(),
        ..button::Style::default()
    });
    let name = text_input(raw("name"), pin.name.as_str())
        .padding([SP.xs, SP.s])
        .size(TS.dense)
        .font(mono)
        .style(crate::panels::form_look::text_input_style)
        .on_input(move |value| PanelMsg::SymEditorSetPinName {
            pin_idx: idx,
            value,
        });
    let hidden = pin.details.hidden;
    let hidden_toggle = button(
        text(raw(if hidden { "Hidden" } else { "Shown" }))
            .size(TS.body)
            .color(if hidden { ink } else { muted }),
    )
    .padding([SP.xs, SP.m])
    .on_press(PanelMsg::SymEditorTogglePinHidden(idx))
    .style(move |_: &Theme, status| button::Style {
        background: Some(Background::Color(Color {
            a: if status == button::Status::Hovered {
                0.12
            } else {
                0.05
            },
            ..ink
        })),
        text_color: ink,
        border: Border {
            radius: Radii::GRAPHITE.pill.into(),
            ..Border::default()
        },
        ..button::Style::default()
    });

    let cells = row![
        cell(number.boxed(), COLUMNS[0].1),
        cell(name.boxed(), COLUMNS[1].1),
        cell(
            text(raw(electrical_label(&pin.electrical)))
                .size(TS.dense)
                .color(ink)
                .boxed(),
            COLUMNS[2].1,
        ),
        cell(
            text(raw(part_label(pin.details.part_number)))
                .size(TS.dense)
                .font(mono)
                .color(muted)
                .boxed(),
            COLUMNS[3].1,
        ),
        cell(hidden_toggle.boxed(), COLUMNS[4].1),
        cell(
            text(raw(format!("{:.2}", pin.length)))
                .size(TS.dense)
                .font(mono)
                .color(ink)
                .boxed(),
            COLUMNS[5].1,
        ),
        cell(
            text(raw(pin.orientation.as_str()))
                .size(TS.dense)
                .color(ink)
                .boxed(),
            COLUMNS[6].1,
        ),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);

    container(cells)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: fill,
            ..container::Style::default()
        })
        .boxed()
}

fn cell<'a>(content: Element<'a, PanelMsg>, portion: u16) -> Element<'a, PanelMsg> {
    container(content)
        .width(Length::FillPortion(portion))
        .boxed()
}

/// `PowerInput` -> `Power Input`: the debug name of a pin direction with a
/// space before each inner capital.
fn electrical_label(debug_name: &str) -> String {
    let mut out = String::with_capacity(debug_name.len() + 2);
    for (i, ch) in debug_name.chars().enumerate() {
        if i > 0 && ch.is_ascii_uppercase() {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

/// Part column: the letter of part 1..=26, `*` for the shared part 0.
fn part_label(part_number: u8) -> String {
    match part_number {
        0 => "*".to_string(),
        n @ 1..=26 => char::from(b'A' + n - 1).to_string(),
        n => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn electrical_label_splits_camel_case() {
        assert_eq!(electrical_label("PowerInput"), "Power Input");
        assert_eq!(electrical_label("Passive"), "Passive");
    }

    #[test]
    fn part_label_names_parts_by_letter() {
        assert_eq!(part_label(1), "A");
        assert_eq!(part_label(2), "B");
        assert_eq!(part_label(0), "*");
        assert_eq!(part_label(27), "27");
    }
}
