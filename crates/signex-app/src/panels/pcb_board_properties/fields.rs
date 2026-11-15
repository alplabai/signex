//! Editable cells the board option sections share (#2374): a length or
//! text field whose draft waits for Enter, a colour swatch, a dropdown and
//! a selectable table row.

use iced::widget::{button as button_style, container, pick_list};
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::Localized;
use signex_types::measure::length_field_text;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{PickListPlaceholder as _, button};

use super::chrome::Form;
use super::info::shown_unit;
use super::{BoardField, PanelMsg, PcbBoardMsg, PcbBoardSummary, SP};
use crate::styles::TypeScale;
use crate::styles::tokens::resolve;

const TS: TypeScale = TypeScale::DEFAULT;

fn msg(m: PcbBoardMsg) -> PanelMsg {
    PanelMsg::PcbBoard(m)
}

/// A field showing `mm` in the panel's unit, or its draft; Enter applies.
pub(super) fn length<'a>(
    form: &Form<'a>,
    board: &PcbBoardSummary,
    field: BoardField,
    label: Localized,
    mm: f64,
) -> Element<'a, PanelMsg> {
    let shown = || length_field_text(mm, shown_unit(form));
    text(form, board, field, label, shown())
}

/// A field showing `value`, or its draft; Enter applies.
pub(super) fn text<'a>(
    form: &Form<'a>,
    board: &PcbBoardSummary,
    field: BoardField,
    label: Localized,
    value: String,
) -> Element<'a, PanelMsg> {
    let value = board.view.drafts.get(&field).cloned().unwrap_or(value);
    form.input(
        label,
        value,
        move |s| msg(PcbBoardMsg::FieldTyped(field, s)),
        msg(PcbBoardMsg::FieldSubmitted(field)),
    )
}

/// A square of `color` that sends `on_press`: the next swatch colour.
pub(super) fn swatch<'a>(
    form: &Form<'a>,
    color: signex_types::theme::Color,
    on_press: PcbBoardMsg,
    tip: Localized,
) -> Element<'a, PanelMsg> {
    let fill = crate::render_config::to_iced(&color);
    let edge = resolve(form.tokens()).line_soft;
    let chip = container(iced::widget::Space::new())
        .width(Length::Fixed(TS.body))
        .height(Length::Fixed(TS.body))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: crate::styles::HAIRLINE,
                color: edge,
                radius: 2.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let press = button(chip)
        .padding(0)
        .on_press(msg(on_press))
        .style(|_: &Theme, _| button_style::Style::default());
    form.tip(press.boxed(), tip)
}

/// One dropdown choice: the value and how it reads.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Choice<T> {
    pub value: T,
    pub label: String,
}

/// A Graphite dropdown of `choices`; `current: None` shows `placeholder`.
pub(super) fn dropdown<'a, T: Clone + PartialEq + 'a>(
    choices: Vec<Choice<T>>,
    current: Option<T>,
    placeholder: Localized,
    picked: impl Fn(T) -> PcbBoardMsg + 'a,
) -> Element<'a, PanelMsg> {
    let selected = current.and_then(|v| choices.iter().find(|c| c.value == v).cloned());
    pick_list(selected, choices, |c: &Choice<T>| c.label.clone())
        .graphite()
        .placeholder_label(placeholder)
        .on_select(move |c: Choice<T>| msg(picked(c.value)))
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .boxed()
}

/// A table row that selects its object when clicked; `selected` tints it.
pub(super) fn selectable<'a>(
    form: &Form<'a>,
    row: Element<'a, PanelMsg>,
    selected: bool,
    on_press: PcbBoardMsg,
) -> Element<'a, PanelMsg> {
    let tint = resolve(form.tokens()).accent_text;
    button(row)
        .padding(0)
        .width(Length::Fill)
        .on_press(msg(on_press))
        .style(move |_: &Theme, _| button_style::Style {
            background: selected.then_some(Background::Color(Color { a: 0.12, ..tint })),
            ..button_style::Style::default()
        })
        .boxed()
}

/// `mm` in the panel's unit, as a read-only value shows it.
pub(super) fn measure(form: &Form<'_>, mm: f64) -> Localized {
    signex_i18n::t!(
        form.i18n,
        signex_i18n::ids::PCB_BOARD_MEASURE,
        "value" => length_field_text(mm, shown_unit(form))
    )
}
