//! The pin table of the Pins tab: every listed pin in one row with its
//! number, name, electrical type, part, visibility, length and orientation.
//! The number selects the pin, the name is edited in place and the hidden
//! cell toggles it, through the same messages as the Pin properties
//! (`SymEditorSelectPin`, `SymEditorSetPinName`, `SymEditorTogglePinHidden`).
//! It keeps the old Pins panel's bulk edit reachable until the Pin table
//! document tab (#122b) lands.

use iced::widget::{Column, Row, container, scrollable};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_widgets::{Label, button, text, text_input};

use super::super::{PanelMsg, SymbolPinSummary, form_look};
use super::rows::{Form, SP, TS, mono};
use crate::library::editor::pin_labels::{
    electrical_message, orientation_name_message, part_label,
};
use crate::styles::{Radii, ti};

/// Column headings and weights: number, name, electrical type, part,
/// hidden, length, orientation.
const COLUMNS: [(MessageId, u16); 7] = [
    (ids::EDITORS_PINS_COLUMN_NUMBER, 1),
    (ids::EDITORS_PINS_COLUMN_NAME, 4),
    (ids::EDITORS_PINS_COLUMN_ELECTRICAL, 4),
    (ids::EDITORS_PINS_COLUMN_PART, 1),
    (ids::EDITORS_PINS_COLUMN_HIDDEN, 2),
    (ids::EDITORS_PINS_COLUMN_LENGTH, 2),
    (ids::EDITORS_PINS_COLUMN_ORIENTATION, 3),
];

/// The table's width as a multiple of the body size; a narrower dock
/// scrolls it sideways instead of squeezing the columns.
const TABLE_EM: f32 = 560.0 / 12.0;

/// The header and one row per pin in `pins`. The Pins tab shows only while
/// nothing is selected, so no row is highlighted.
pub(super) fn view<'a>(pins: &[&'a SymbolPinSummary], form: &Form<'_>) -> Element<'a, PanelMsg> {
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().width(TS.body * TABLE_EM);
    col = col.push(header(form));
    for pin in pins {
        col = col.push(pin_row(pin, form));
    }
    scrollable(col)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::default(),
        ))
        .width(Length::Fill)
        .style(form_look::scrollbar_style)
        .boxed()
}

fn header<'a>(form: &Form<'_>) -> Element<'a, PanelMsg> {
    let ink = form.look.ink3;
    let mut cells: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.m).align_y(Alignment::Center);
    for (label, portion) in COLUMNS {
        cells = cells.push(cell(
            text(t!(form.i18n, label))
                .size(TS.caption)
                .color(ink)
                .boxed(),
            portion,
        ));
    }
    let band = form.look.sunken;
    container(cells)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(band)),
            ..container::Style::default()
        })
        .boxed()
}

fn pin_row<'a>(pin: &'a SymbolPinSummary, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let look = form.look;
    let ink = look.ink;
    let muted = look.ink3;
    let idx = pin.idx;
    let hover = ti(form.tokens.hover);
    let chip_fill = look.sunken;

    let number = button(
        text(UserText::new(pin.number.as_str()))
            .size(TS.body)
            .font(mono())
            .color(ink),
    )
    .padding([SP.xs, SP.s])
    .on_press(PanelMsg::SymEditorSelectPin(idx))
    .style(move |_: &Theme, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(hover)),
        text_color: ink,
        border: Border::default(),
        ..button::Style::default()
    });
    let name = text_input(
        t!(i18n, ids::EDITORS_PINS_NAME_PLACEHOLDER),
        pin.name.as_str(),
    )
    .padding([SP.xs, SP.s])
    .size(TS.body)
    .font(mono())
    .style(form_look::text_input_style)
    .on_input(move |value| PanelMsg::SymEditorSetPinName {
        pin_idx: idx,
        value,
    });
    let hidden = pin.details.hidden;
    let hidden_toggle = button(
        text(t!(
            i18n,
            if hidden {
                ids::EDITORS_PINS_HIDDEN
            } else {
                ids::EDITORS_PINS_SHOWN
            }
        ))
        .size(TS.body)
        .color(if hidden { ink } else { muted }),
    )
    .padding([SP.xs, SP.m])
    .on_press(PanelMsg::SymEditorTogglePinHidden(idx))
    .style(move |_: &Theme, status| button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                hover
            } else {
                chip_fill
            },
        )),
        text_color: ink,
        border: Border {
            radius: Radii::GRAPHITE.pill.into(),
            ..Border::default()
        },
        ..button::Style::default()
    });
    let label = |id: Option<MessageId>, raw: &'a str| -> Element<'a, PanelMsg> {
        text(enum_label(i18n, id, raw))
            .size(TS.body)
            .color(ink)
            .boxed()
    };
    let small = |s: String, color: Color| -> Element<'a, PanelMsg> {
        text(UserText::new(s))
            .size(TS.body)
            .font(mono())
            .color(color)
            .boxed()
    };
    let cells: Row<Element<'a, PanelMsg>> = Row::with_children([
        cell(number.boxed(), COLUMNS[0].1),
        cell(name.boxed(), COLUMNS[1].1),
        cell(
            label(electrical_message(&pin.electrical), &pin.electrical),
            COLUMNS[2].1,
        ),
        cell(
            small(part_label(pin.details.part_number), muted),
            COLUMNS[3].1,
        ),
        cell(hidden_toggle.boxed(), COLUMNS[4].1),
        cell(small(format!("{:.2}", pin.length), ink), COLUMNS[5].1),
        cell(
            label(orientation_name_message(&pin.orientation), &pin.orientation),
            COLUMNS[6].1,
        ),
    ])
    .spacing(SP.m)
    .align_y(Alignment::Center);
    container(cells)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

fn cell<'a>(content: Element<'a, PanelMsg>, portion: u16) -> Element<'a, PanelMsg> {
    container(content)
        .width(Length::FillPortion(portion))
        .clip(true)
        .boxed()
}

/// The catalog label of an enum the pin summary carries by its debug name,
/// or the name itself for a variant without a label yet.
fn enum_label<'a>(i18n: &I18n, message: Option<MessageId>, debug_name: &'a str) -> Label<'a> {
    match message {
        Some(id) => t!(i18n, id).into(),
        None => UserText::new(debug_name).into(),
    }
}
