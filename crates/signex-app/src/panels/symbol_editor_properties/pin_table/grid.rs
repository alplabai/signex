//! The Pin table's rows: a header with the tick-all box, then one row per
//! listed pin, its number, name and part typed in place, its electrical
//! type and orientation picked, its Hidden box, its length on the typing
//! draft, and a Check column naming what is wrong with it.

use iced::widget::{Column, Row, Space, container};
use iced::{Alignment, Background, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_library::{PinDirection, PinOrientation};
use signex_widgets::{checkbox, text, text_input};

use super::super::super::{
    NumericFieldKey, PanelMsg, SymbolEditorPanelContext, SymbolEditorSelection, SymbolPinSummary,
    form_look,
};
use super::super::pins_tab::PinIssue;
use super::super::rows::{Choice, Form, SP, TS, mono};
use super::table;
use crate::library::editor::pin_labels::{
    direction_by_name, direction_message, orientation_by_name, orientation_message,
};
use crate::library::editor::symbol::{PinTableMsg, PinTableUi};
use crate::styles::{HAIRLINE, PillTone};

/// Width of the tick column, in body sizes.
const TICK_EM: f32 = 28.0 / 12.0;

/// Column headings and weights after the tick box: number, name, electrical
/// type, part, hidden, length, orientation and check.
const COLUMNS: [(MessageId, u16); 8] = [
    (ids::EDITORS_PINS_COLUMN_NUMBER, 2),
    (ids::EDITORS_PINS_COLUMN_NAME, 4),
    (ids::EDITORS_PINS_COLUMN_ELECTRICAL, 4),
    (ids::EDITORS_PINS_COLUMN_PART, 2),
    (ids::EDITORS_PINS_COLUMN_HIDDEN, 2),
    (ids::EDITORS_PINS_COLUMN_LENGTH, 3),
    (ids::EDITORS_PINS_COLUMN_ORIENTATION, 3),
    (ids::EDITORS_PIN_TABLE_COLUMN_CHECK, 4),
];

/// The orientations in the order the dropdowns list them.
pub(super) const ORIENTATIONS: [PinOrientation; 4] = [
    PinOrientation::Left,
    PinOrientation::Right,
    PinOrientation::Up,
    PinOrientation::Down,
];

/// Every electrical type, labelled.
pub(super) fn electrical_choices(i18n: &I18n) -> Vec<Choice<PinDirection>> {
    PinDirection::ALL
        .into_iter()
        .map(|d| Choice::new(d, t!(i18n, direction_message(d))))
        .collect()
}

/// The four orientations, labelled.
pub(super) fn orientation_choices(i18n: &I18n) -> Vec<Choice<PinOrientation>> {
    ORIENTATIONS
        .into_iter()
        .map(|o| Choice::new(o, t!(i18n, orientation_message(o))))
        .collect()
}

/// The header and the rows of `listed`. A ticked row and the pin selected
/// in the editor are lit.
pub(super) fn view<'a>(
    sym: &'a SymbolEditorPanelContext,
    listed: &[&'a SymbolPinSummary],
    issues: &[PinIssue],
    ui: &PinTableUi,
    form: &Form<'_>,
) -> Element<'a, PanelMsg> {
    let selected = match &sym.selected {
        SymbolEditorSelection::Pin(pin) => Some(pin.idx),
        _ => None,
    };
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    col = col.push(header(listed, ui, form));
    for pin in listed {
        let issue = issues.get(pin.idx).copied().unwrap_or_default();
        let ticked = ui.checked.contains(&pin.idx);
        let lit = ticked || selected == Some(pin.idx);
        col = col.push(pin_row(pin, issue, ticked, lit, form));
        col = col.push(rule(form));
    }
    col.boxed()
}

/// The tick-all box and the column headings, on the sunken band.
fn header<'a>(
    listed: &[&SymbolPinSummary],
    ui: &PinTableUi,
    form: &Form<'_>,
) -> Element<'a, PanelMsg> {
    let rows: Vec<usize> = listed.iter().map(|p| p.idx).collect();
    let all = !rows.is_empty() && rows.iter().all(|idx| ui.checked.contains(idx));
    let tick = checkbox(all)
        .size(TS.heading)
        .style(form_look::checkbox_style)
        .on_toggle(move |on| table(PinTableMsg::AllChecked(rows.clone(), on)));
    let ink = form.look.ink3;
    let mut cells: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .push(container(tick).width(TS.body * TICK_EM).boxed());
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
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(band)),
            ..container::Style::default()
        })
        .boxed()
}

fn pin_row<'a>(
    pin: &'a SymbolPinSummary,
    issue: PinIssue,
    ticked: bool,
    lit: bool,
    form: &Form<'_>,
) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let look = form.look;
    let pin_idx = pin.idx;
    let tick = checkbox(ticked)
        .size(TS.heading)
        .style(form_look::checkbox_style)
        .on_toggle(move |on| table(PinTableMsg::RowChecked(pin_idx, on)));
    let number = text_input(
        t!(i18n, ids::EDITORS_PIN_TABLE_FIELD_NUMBER),
        pin.number.as_str(),
    )
    .size(TS.body)
    .padding([SP.xs, SP.s])
    .font(mono())
    .style(form_look::refusable_input_style(
        issue.bad_number.then_some(look.err),
    ))
    .on_input(move |value| PanelMsg::SymEditorSetPinNumber { pin_idx, value })
    .width(Length::Fill);
    let name = text_input(
        t!(i18n, ids::EDITORS_PINS_NAME_PLACEHOLDER),
        pin.name.as_str(),
    )
    .size(TS.body)
    .padding([SP.xs, SP.s])
    .font(mono())
    .style(form_look::refusable_input_style(
        issue.no_name.then_some(look.warn),
    ))
    .on_input(move |value| PanelMsg::SymEditorSetPinName { pin_idx, value })
    .width(Length::Fill);
    let electrical = form.dropdown(
        electrical_choices(i18n),
        direction_by_name(&pin.electrical),
        move |value| PanelMsg::SymEditorSetPinElectrical { pin_idx, value },
    );
    let part_now = pin.details.part_number;
    let part = form.input(
        t!(i18n, ids::EDITORS_PIN_TABLE_FIELD_PART),
        part_now.to_string(),
        true,
        move |s| PanelMsg::SymEditorSetPinPartNumber {
            pin_idx,
            value: s.trim().parse::<u8>().unwrap_or(part_now),
        },
    );
    let hidden = checkbox(pin.details.hidden)
        .size(TS.heading)
        .style(form_look::checkbox_style)
        .on_toggle(move |_| PanelMsg::SymEditorTogglePinHidden(pin_idx));
    let length = form.number(NumericFieldKey::PinLength(pin_idx), pin.length, None);
    let orientation = form.dropdown(
        orientation_choices(i18n),
        orientation_by_name(&pin.orientation),
        move |value| PanelMsg::SymEditorSetPinOrientation { pin_idx, value },
    );
    let cells: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .push(container(tick).width(TS.body * TICK_EM).boxed())
        .push(cell(number.boxed(), COLUMNS[0].1))
        .push(cell(name.boxed(), COLUMNS[1].1))
        .push(cell(electrical, COLUMNS[2].1))
        .push(cell(part, COLUMNS[3].1))
        .push(cell(hidden.boxed(), COLUMNS[4].1))
        .push(cell(length, COLUMNS[5].1))
        .push(cell(orientation, COLUMNS[6].1))
        .push(cell(check(pin, issue, form), COLUMNS[7].1));
    let fill = lit.then_some(look.accent_soft);
    container(cells)
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            ..container::Style::default()
        })
        .boxed()
}

/// The Check cell: a missing or duplicate number in the error tone, a
/// missing name in the warning tone, else OK.
fn check<'a>(pin: &SymbolPinSummary, issue: PinIssue, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let mut pills: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.xs);
    if issue.bad_number {
        let number = pin.number.trim();
        let label = if number.is_empty() {
            t!(i18n, ids::EDITORS_PIN_TABLE_NO_NUMBER)
        } else {
            t!(i18n, ids::EDITORS_PIN_TABLE_DUPLICATE, "number" => number)
        };
        pills = pills.push(form.pill(label.into(), PillTone::Err));
    }
    if issue.no_name {
        pills = pills.push(form.pill(
            t!(i18n, ids::EDITORS_PIN_TABLE_NO_NAME).into(),
            PillTone::Warn,
        ));
    }
    if !issue.any() {
        pills = pills.push(form.pill(t!(i18n, ids::EDITORS_PIN_TABLE_OK).into(), PillTone::Ok));
    }
    pills.boxed()
}

fn cell<'a>(content: Element<'a, PanelMsg>, portion: u16) -> Element<'a, PanelMsg> {
    container(content)
        .width(Length::FillPortion(portion))
        .clip(true)
        .boxed()
}

fn rule<'a>(form: &Form<'_>) -> Element<'a, PanelMsg> {
    let line = form.look.line_soft;
    container(Space::new())
        .height(HAIRLINE)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}
