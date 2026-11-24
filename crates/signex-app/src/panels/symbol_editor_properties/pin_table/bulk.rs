//! The bulk edit bar of the Pin table: Type, Part, Length, Orientation and
//! Hidden for every ticked row, a field left on Keep keeping each row's
//! value, and Apply to selected. Greyed and inert while no listed row is
//! ticked; a ticked row the filter hides is not counted.

use iced::widget::{Row, container, row};
use iced::{Alignment, Background, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_widgets::{button, text, text_input};

use super::super::super::{PanelMsg, form_look};
use super::super::rows::{Choice, Form, SP, TS, mono};
use super::grid::{electrical_choices, orientation_choices};
use super::table;
use crate::library::editor::symbol::{PinTableMsg, PinTableUi};

/// The width of a bulk dropdown and of a bulk number, in body sizes.
const SELECT_EM: f32 = 120.0 / 12.0;
const NUMBER_EM: f32 = 72.0 / 12.0;

/// `choices` after a first Keep entry, which leaves the field as it is.
fn with_keep<V>(i18n: &I18n, choices: Vec<Choice<V>>) -> Vec<Choice<Option<V>>> {
    std::iter::once(Choice::new(None, t!(i18n, ids::EDITORS_PIN_TABLE_KEEP)))
        .chain(choices.into_iter().map(|c| Choice {
            value: Some(c.value),
            label: c.label,
        }))
        .collect()
}

/// The bar for `count` ticked rows the table lists now: the rows Apply to
/// selected writes.
pub(super) fn view<'a>(ui: &'a PinTableUi, count: usize, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let on = count > 0;
    let ink = if on { form.look.ink3 } else { form.look.ink4 };
    let label = |id: MessageId| -> Element<'a, PanelMsg> {
        text(t!(i18n, id)).size(TS.caption).color(ink).boxed()
    };
    let keep = || -> Localized { t!(i18n, ids::EDITORS_PIN_TABLE_KEEP) };
    // A dropdown, or while no row is ticked its Keep shown read-only.
    let select = |field: Element<'a, PanelMsg>| -> Element<'a, PanelMsg> {
        container(if on {
            field
        } else {
            form.read_only(keep().into(), false)
        })
        .width(TS.body * SELECT_EM)
        .boxed()
    };
    let electrical = select(form.dropdown(
        with_keep(i18n, electrical_choices(i18n)),
        Some(ui.bulk.electrical),
        |v| table(PinTableMsg::BulkElectricalPicked(v)),
    ));
    let orientation = select(form.dropdown(
        with_keep(i18n, orientation_choices(i18n)),
        Some(ui.bulk.orientation),
        |v| table(PinTableMsg::BulkOrientationPicked(v)),
    ));
    let hidden = select(form.dropdown(
        with_keep(
            i18n,
            vec![
                Choice::new(true, t!(i18n, ids::EDITORS_PIN_TABLE_YES)),
                Choice::new(false, t!(i18n, ids::EDITORS_PIN_TABLE_NO)),
            ],
        ),
        Some(ui.bulk.hidden),
        |v| table(PinTableMsg::BulkHiddenPicked(v)),
    ));
    let part = number(
        keep(),
        ui.bulk.part.as_str(),
        on.then_some(|s: String| table(PinTableMsg::BulkPartChanged(s))),
    );
    let length = row![
        number(
            keep(),
            ui.bulk.length.as_str(),
            on.then_some(|s: String| table(PinTableMsg::BulkLengthChanged(s))),
        ),
        text(t!(i18n, ids::EDITORS_SYMPROPS_UNIT_MM))
            .size(TS.caption)
            .color(ink)
            .boxed(),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    let selected = if on {
        t!(i18n, ids::EDITORS_PIN_TABLE_SELECTED, "count" => count)
    } else {
        t!(i18n, ids::EDITORS_PIN_TABLE_NONE_SELECTED)
    };
    let apply = button(text(t!(i18n, ids::EDITORS_PIN_TABLE_APPLY)).size(TS.body))
        .padding([SP.xs, SP.m])
        .on_press_maybe(on.then_some(table(PinTableMsg::BulkApplied)))
        .style(crate::styles::primary_button(form.tokens));
    let bar: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(text(selected).size(TS.body).color(form.look.ink).boxed())
        .push(label(ids::EDITORS_PIN_TABLE_BULK_TYPE))
        .push(electrical)
        .push(label(ids::EDITORS_PIN_TABLE_BULK_PART))
        .push(part)
        .push(label(ids::EDITORS_PIN_TABLE_BULK_LENGTH))
        .push(length.boxed())
        .push(label(ids::EDITORS_PIN_TABLE_BULK_ORIENTATION))
        .push(orientation)
        .push(label(ids::EDITORS_PIN_TABLE_BULK_HIDDEN))
        .push(hidden)
        .push(apply.boxed());
    let band = form.look.sunken;
    container(bar.wrap().vertical_spacing(SP.xs))
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(band)),
            ..container::Style::default()
        })
        .boxed()
}

/// A small mono field with `placeholder`; `on_input: None` greys it.
fn number<'a>(
    placeholder: Localized,
    value: &'a str,
    on_input: Option<impl Fn(String) -> PanelMsg + 'a>,
) -> Element<'a, PanelMsg> {
    text_input(placeholder, value)
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .font(mono())
        .on_input_maybe(on_input)
        .style(form_look::greyable_input_style)
        .width(TS.body * NUMBER_EM)
        .boxed()
}
