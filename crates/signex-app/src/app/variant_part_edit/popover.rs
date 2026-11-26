//! The Edit value popover (mockup row 209), in place by the part: the
//! parameter (Value by default; its list holds the part's Base parameters,
//! DECISIONS 2026-11-26), the Base value read-only, the variant's value
//! being typed, the scope on a channel tab, then Reset to base, Cancel and
//! OK. An empty field is not a value: OK stays greyed and Reset to base goes
//! back to Base (DECISIONS 2026-11-26). A pure view.

use iced::widget::{Column, Row, column, container, row};
use iced::{Alignment, Color, Element, Length, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::project::{VALUE_PARAMETER, VariantSettings};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, text, text_input};

use super::VariantPartEditMsg;
use super::open::VARIANT_VALUE_INPUT_ID;
use super::picker::scope_choice;
use super::picker_parts::{SP, TS, fill, with_tip};
use super::state::ValuePopover;
use crate::app::Message;
use crate::styles::tokens::resolve;
use crate::styles::{DialogButtonKind, chrome_px, dialog_button, medium_font, ti};

/// Card width (mockup: 300 px).
const CARD_W: f32 = chrome_px(300.0 * 11.0 / 12.0);
/// Width of the label column (mockup: 64 px).
const LABEL_W: f32 = chrome_px(64.0 * 11.0 / 12.0);

/// A parameter's name as shown: Value in the catalog's words, a field as
/// the part names it.
fn parameter_name<'a>(parameter: &str, i18n: &I18n) -> Label<'a> {
    if parameter.eq_ignore_ascii_case(VALUE_PARAMETER) {
        t!(i18n, ids::VARIANT_MANAGER_PARAMETER_VALUE).into()
    } else {
        UserText::new(parameter.to_string()).into()
    }
}

/// A row of the form: its label, then the field.
fn form_row<'a>(
    label: Label<'a>,
    field: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    row![
        text(label)
            .size(TS.label)
            .color(resolve(tokens).ink3)
            .width(Length::Fixed(LABEL_W)),
        field,
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// The popover card.
pub(super) fn popover_card<'a>(
    popover: &'a ValuePopover,
    settings: &VariantSettings,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let part = &popover.part;
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let where_ = match &part.channel {
        Some(channel) => t!(
            i18n,
            ids::VARIANT_EDIT_VALUE_WHERE_CHANNEL,
            "part" => part.this_part.designator.as_str(),
            "variant" => part.variant.as_str(),
            "channel" => channel.as_str()
        ),
        None => t!(
            i18n,
            ids::VARIANT_EDIT_VALUE_WHERE,
            "part" => part.this_part.designator.as_str(),
            "variant" => part.variant.as_str()
        ),
    };
    let head = row![
        text(t!(i18n, ids::VARIANT_EDIT_VALUE_TITLE))
            .size(TS.label)
            .font(medium_font())
            .color(ink),
        text(where_).size(TS.caption).color(r.ink4),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    let parameter = button(
        text(parameter_name(&popover.parameter, i18n))
            .size(TS.body)
            .color(ink),
    )
    .width(Length::Fill)
    .padding([SP.xs, SP.s])
    .on_press(Message::from(VariantPartEditMsg::ParameterListToggled))
    .style(crate::styles::outline_button(tokens));
    let base_value = part.base.base_value(&popover.parameter).to_string();
    let base = with_tip(
        container(
            text(UserText::new(base_value))
                .size(TS.body)
                .font(crate::library::picker::mono())
                .color(r.ink3),
        )
        .width(Length::Fill)
        .padding([SP.xs, SP.s])
        .style(crate::styles::read_only_field(tokens))
        .boxed(),
        t!(i18n, ids::VARIANT_EDIT_BASE_TIP),
        tokens,
    );
    let submit = popover
        .can_apply()
        .then(|| Message::from(VariantPartEditMsg::ApplyPressed));
    let value = text_input(UserText::new(String::new()), &popover.text)
        .id(VARIANT_VALUE_INPUT_ID.clone())
        .on_input(|text| Message::from(VariantPartEditMsg::ValueEdited(text)))
        .on_submit_maybe(submit.clone())
        .size(TS.body)
        .font(crate::library::picker::mono())
        .width(Length::Fill);
    let mut body: Vec<Element<'a, Message>> = vec![
        head.boxed(),
        form_row(
            t!(i18n, ids::VARIANT_EDIT_PARAMETER).into(),
            parameter.boxed(),
            tokens,
        ),
    ];
    if popover.parameters_open {
        body.push(parameter_list(popover, settings, i18n, tokens));
    }
    body.push(form_row(
        t!(i18n, ids::VARIANT_EDIT_BASE).into(),
        base,
        tokens,
    ));
    body.push(form_row(
        UserText::new(part.variant.clone()).into(),
        value.boxed(),
        tokens,
    ));
    if let Some(scope) = scope_choice(part, popover.scope, i18n) {
        body.push(scope);
    }
    let note = match (&popover.problem, popover.can_apply()) {
        (Some(problem), _) => crate::styles::dialog_inline_error(
            text(UserText::new(problem.clone()))
                .size(TS.caption)
                .boxed(),
            tokens,
        ),
        (None, false) => text(t!(i18n, ids::VARIANT_EDIT_EMPTY))
            .size(TS.caption)
            .color(ti(tokens.warning))
            .boxed(),
        (None, true) => text(t!(i18n, ids::VARIANT_EDIT_KEYS))
            .size(TS.caption)
            .color(r.ink4)
            .boxed(),
    };
    body.push(note);
    body.push(footer(popover, settings, i18n, tokens, submit));
    container(
        Column::with_children(body)
            .spacing(SP.m)
            .width(Length::Fixed(CARD_W)),
    )
    .padding(SP.l)
    .style(crate::styles::context_menu(tokens))
    .boxed()
}

/// The parameter list: each of the part's Base parameters with its Base
/// value, or, orange, the value the variant already gives it.
fn parameter_list<'a>(
    popover: &ValuePopover,
    settings: &VariantSettings,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let part = &popover.part;
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let overridden = ti(tokens.warning);
    let mut list: Column<Element<'a, Message>> = column![
        text(t!(
            i18n,
            ids::VARIANT_EDIT_PARAMETERS_HEAD,
            "part" => part.this_part.designator.as_str()
        ))
        .size(TS.caption)
        .color(r.ink3)
    ];
    for (name, base) in &part.base.parameters {
        let own = part.own_value(settings, name);
        let (shown, color): (Label<'a>, Color) = match own {
            Some(own) => (
                t!(
                    i18n,
                    ids::VARIANT_EDIT_OVERRIDDEN_IN,
                    "value" => own,
                    "variant" => part.variant.as_str()
                )
                .into(),
                overridden,
            ),
            None => (UserText::new(base.clone()).into(), r.ink3),
        };
        let line: Row<Element<'a, Message>> = row![
            text(parameter_name(name, i18n)).size(TS.body).color(ink),
            fill(),
            text(shown).size(TS.caption).color(color),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center);
        let is_current = name.eq_ignore_ascii_case(&popover.parameter);
        list = list.push(
            button(line)
                .width(Length::Fill)
                .padding([SP.xs, SP.s])
                .on_press(Message::from(VariantPartEditMsg::ParameterPicked(
                    name.clone(),
                )))
                .style(crate::styles::hover_row(tokens, is_current))
                .boxed(),
        );
    }
    list = list.push(
        text(t!(
            i18n,
            ids::VARIANT_EDIT_PARAMETERS_NOTE,
            "variant" => part.variant.as_str()
        ))
        .size(TS.caption)
        .color(r.ink4)
        .boxed(),
    );
    container(list.spacing(SP.xs))
        .width(Length::Fill)
        .padding(SP.s)
        .style(crate::styles::read_only_field(tokens))
        .boxed()
}

/// Reset to base (live while the variant gives the parameter a value of its
/// own), Cancel and OK.
fn footer<'a>(
    popover: &ValuePopover,
    settings: &VariantSettings,
    i18n: &I18n,
    tokens: &ThemeTokens,
    submit: Option<Message>,
) -> Element<'a, Message> {
    let part = &popover.part;
    let is_overridden = part.parts(popover.scope).iter().any(|one| {
        settings
            .part(Some(part.variant.as_str()), one.placed())
            .is_some_and(|own| {
                own.parameters
                    .keys()
                    .any(|name| name.eq_ignore_ascii_case(&popover.parameter))
            })
    });
    let reset = dialog_button(
        t!(i18n, ids::VARIANT_EDIT_RESET),
        DialogButtonKind::Ghost,
        is_overridden.then(|| Message::from(VariantPartEditMsg::ResetPressed)),
        tokens,
    );
    let reset = with_tip(
        reset,
        t!(
            i18n,
            ids::VARIANT_EDIT_RESET_TIP,
            "variant" => part.variant.as_str(),
            "part" => part.this_part.designator.as_str(),
            "base" => part.base.base_value(&popover.parameter)
        ),
        tokens,
    );
    row![
        reset,
        fill(),
        dialog_button(
            t!(i18n, ids::DIALOG_BUTTON_CANCEL),
            DialogButtonKind::Cancel,
            Some(Message::from(VariantPartEditMsg::CancelPressed)),
            tokens,
        ),
        dialog_button(
            t!(i18n, ids::DIALOG_BUTTON_OK),
            DialogButtonKind::Primary,
            submit,
            tokens,
        ),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}
