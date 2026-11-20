//! Constraint sets (#2398, #2530): one card per set under the toolbar (its
//! name, its limits, where it is applied, Use these limits and Apply to
//! scope), then the set form: name, limits, severity, scope and Create
//! set. A set is edited through the form, not in place on its card.

use iced::alignment::Vertical;
use iced::widget::{Column, Row, Space, column, container, pick_list, row, scrollable};
use iced::{Background, Border, Element, Length, Padding, Theme, Widget as _};
use signex_drc_dsl::table::sets::values_text;
use signex_i18n::{MessageId, ids, t};
use signex_types::project::NamedRuleSet;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{PickListPlaceholder as _, button, text, text_input};

use super::{Frame, SP, TS, up, user};
use crate::app::Message;
use crate::app::rules_tab::{RulesTabMsg, SetFormField};
use crate::styles::tokens::resolve;
use crate::styles::{Radii, ti, tokens};

/// A set card is 300 px wide at the 12 px body.
const CARD_WIDTH_EM: f32 = 300.0 / 12.0;
/// The severity words a set can carry, as the rule text writes them.
const SEVERITIES: [&str; 3] = ["error", "warning", "info"];

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let tokens = frame.tokens;
    let mut cards: Row<Element<'a, Message>> = Row::new().spacing(SP.s);
    if frame.sets.is_empty() {
        cards = cards.push(
            text(t!(frame.i18n, ids::RULES_SETS_EMPTY))
                .size(TS.body)
                .color(tokens::muted_text(tokens))
                .boxed(),
        );
    }
    for (name, set) in &frame.sets {
        cards = cards.push(card(frame, name, set));
    }
    column![
        scrollable(cards)
            .direction(iced::widget::scrollable::Direction::Horizontal(
                crate::styles::thin_scrollbar()
            ))
            .style(crate::styles::scrollbar_style(tokens)),
        form(frame),
    ]
    .spacing(SP.s)
    .boxed()
}

/// One set: name, applied-to scopes, its limits, and its two actions.
fn card<'a>(frame: &Frame<'a>, name: &str, set: &NamedRuleSet) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let r = resolve(tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let mut head: Row<Element<'a, Message>> =
        Row::new().spacing(SP.xs).align_y(Vertical::Center).push(
            text(user(name.to_string()))
                .size(TS.body)
                .font(mono)
                .color(ti(tokens.text))
                .boxed(),
        );
    if set.uses.is_empty() {
        head = head.push(
            text(t!(i18n, ids::RULES_SETS_UNUSED))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    } else {
        head = head.push(
            text(t!(i18n, ids::RULES_SETS_APPLIED_TO))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
        for used in &set.uses {
            head = head.push(scope_chip(frame, used.scope.clone()));
        }
    }
    let severity = set.severity.clone().unwrap_or_else(|| "error".to_string());
    let values = text(t!(
        i18n,
        ids::RULES_SETS_CARD_VALUES,
        "values" => values_text(set),
        "severity" => severity
    ))
    .size(TS.caption)
    .font(mono)
    .color(r.ink3);
    let action = |label: MessageId, msg: RulesTabMsg| {
        button(text(t!(i18n, label)).size(TS.caption))
            .padding([0.0, SP.s])
            .style(crate::styles::ghost_button(tokens))
            .on_press(up(msg))
    };
    let actions = row![
        action(
            ids::RULES_SETS_REDEFINE,
            RulesTabMsg::SetRedefined {
                project_id: id,
                set: name.to_string(),
            },
        ),
        action(
            ids::RULES_SETS_APPLY,
            RulesTabMsg::SetApplied {
                project_id: id,
                set: name.to_string(),
            },
        ),
    ]
    .spacing(SP.xs);
    container(column![head, values, actions].spacing(SP.xs))
        .padding(SP.s)
        .width(TS.body * CARD_WIDTH_EM)
        .style(crate::styles::card(tokens))
        .boxed()
}

/// A scope a set is applied to, as a small chip.
fn scope_chip<'a>(frame: &Frame<'a>, scope: String) -> Element<'a, Message> {
    let r = resolve(frame.tokens);
    container(text(user(scope)).size(TS.caption).color(r.accent_text))
        .padding(Padding::from([0.0, SP.xs]))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(r.accent_soft)),
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

fn input<'a>(
    frame: &Frame<'a>,
    field: SetFormField,
    hint: MessageId,
    value: &'a str,
) -> Element<'a, Message> {
    let id = frame.id;
    text_input(t!(frame.i18n, hint), value)
        .on_input(move |text| {
            up(RulesTabMsg::SetFormEdited {
                project_id: id,
                field,
                text,
            })
        })
        .size(TS.body)
        .padding(SP.xs)
        .width(Length::Fill)
        .style(crate::panels::form_look::text_input_style)
        .boxed()
}

/// The set form: name, limits, severity, Create set; then the scope Apply
/// to scope writes the set's rules for.
fn form<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let form = &frame.state.set_form;
    let picked = SEVERITIES
        .iter()
        .copied()
        .find(|s| *s == form.severity.trim());
    let severity = pick_list(picked, SEVERITIES.to_vec(), move |s: &&str| {
        t!(i18n, severity_label(s)).into_string()
    })
    .on_select(move |s: &str| {
        up(RulesTabMsg::SetFormEdited {
            project_id: id,
            field: SetFormField::Severity,
            text: s.to_string(),
        })
    })
    .graphite()
    .placeholder_label(t!(i18n, ids::RULES_SETS_SEVERITY))
    .text_size(TS.body)
    .width(Length::FillPortion(1));
    let create = button(text(t!(i18n, ids::RULES_SETS_CREATE)).size(TS.label))
        .padding([SP.xs, SP.m])
        .style(crate::styles::accent_button(tokens))
        .on_press(up(RulesTabMsg::SetCreated { project_id: id }));
    let first: Row<Element<'a, Message>> = row![
        container(input(
            frame,
            SetFormField::Name,
            ids::RULES_SETS_NAME,
            &form.name
        ))
        .width(Length::FillPortion(1)),
        container(input(
            frame,
            SetFormField::Values,
            ids::RULES_SETS_VALUES,
            &form.values
        ))
        .width(Length::FillPortion(3)),
        severity,
        create,
        container(input(
            frame,
            SetFormField::Scope,
            ids::RULES_SETS_SCOPE,
            &form.scope
        ))
        .width(Length::FillPortion(2)),
    ]
    .spacing(SP.s)
    .align_y(Vertical::Center);
    let hint = text(t!(i18n, ids::RULES_SETS_HINT))
        .size(TS.caption)
        .color(tokens::muted_text(tokens));
    let body: Column<Element<'a, Message>> = column![first, hint, Space::new().height(SP.xs)];
    body.spacing(SP.xs).boxed()
}

fn severity_label(word: &str) -> MessageId {
    match word {
        "warning" => ids::RULES_SEVERITY_WARNING,
        "info" => ids::RULES_SEVERITY_INFO,
        _ => ids::RULES_SEVERITY_ERROR,
    }
}
