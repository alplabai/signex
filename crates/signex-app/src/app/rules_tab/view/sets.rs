//! The Rule sets panel (#2398): a form to create a set and to give one new
//! limits or apply it to a scope, then one line per set with where it is
//! applied.

use std::collections::BTreeMap;

use iced::widget::{Column, Row, column, row};
use iced::{Element, Length, Widget as _};
use signex_drc_dsl::table::sets::values_text;
use signex_i18n::{MessageId, ids, t};
use signex_types::project::NamedRuleSet;
use signex_widgets::{button, text, text_input};

use super::{Frame, SP, TS, up, user};
use crate::app::Message;
use crate::app::rules_tab::{RulesTabMsg, SetFormField};
use crate::styles::tokens;

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

pub(super) fn view<'a>(
    frame: &Frame<'a>,
    sets: &BTreeMap<String, NamedRuleSet>,
) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let form = &frame.state.set_form;
    let mut body: Column<Element<'a, Message>> = Column::new().spacing(SP.s);
    body = body.push(text(t!(i18n, ids::RULES_SETS_TITLE)).size(TS.label).boxed());
    body = body.push(
        text(t!(i18n, ids::RULES_SETS_HINT))
            .size(TS.caption)
            .color(tokens::muted_text(tokens))
            .boxed(),
    );
    body = body.push(input(
        frame,
        SetFormField::Name,
        ids::RULES_SETS_NAME,
        &form.name,
    ));
    body = body.push(input(
        frame,
        SetFormField::Values,
        ids::RULES_SETS_VALUES,
        &form.values,
    ));
    body = body.push(input(
        frame,
        SetFormField::Severity,
        ids::RULES_SETS_SEVERITY,
        &form.severity,
    ));
    body = body.push(
        button(text(t!(i18n, ids::RULES_SETS_CREATE)).size(TS.label))
            .padding([SP.xs, SP.m])
            .style(crate::styles::accent_button(tokens))
            .on_press(up(RulesTabMsg::SetCreated { project_id: id }))
            .boxed(),
    );
    body = body.push(input(
        frame,
        SetFormField::Scope,
        ids::RULES_SETS_SCOPE,
        &form.scope,
    ));
    if sets.is_empty() {
        body = body.push(
            text(t!(i18n, ids::RULES_SETS_EMPTY))
                .size(TS.body)
                .color(tokens::muted_text(tokens))
                .boxed(),
        );
    }
    for (name, set) in sets {
        body = body.push(set_line(frame, name, set));
    }
    body.width(Length::FillPortion(2)).boxed()
}

fn set_line<'a>(frame: &Frame<'a>, name: &str, set: &NamedRuleSet) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let severity = set.severity.clone().unwrap_or_else(|| "error".to_string());
    let head = format!("{name}: {} ({severity})", values_text(set));
    let uses = if set.uses.is_empty() {
        t!(i18n, ids::RULES_SETS_UNUSED)
    } else {
        let scopes: Vec<&str> = set.uses.iter().map(|u| u.scope.as_str()).collect();
        t!(i18n, ids::RULES_SETS_USES, "scopes" => scopes.join("; "))
    };
    let apply = button(text(t!(i18n, ids::RULES_SETS_APPLY)).size(TS.caption))
        .padding([0.0, SP.xs])
        .style(crate::styles::outline_button(tokens))
        .on_press(up(RulesTabMsg::SetApplied {
            project_id: id,
            set: name.to_string(),
        }));
    let redefine = button(text(t!(i18n, ids::RULES_SETS_REDEFINE)).size(TS.caption))
        .padding([0.0, SP.xs])
        .style(crate::styles::outline_button(tokens))
        .on_press(up(RulesTabMsg::SetRedefined {
            project_id: id,
            set: name.to_string(),
        }));
    let actions: Row<Element<'a, Message>> = row![apply, redefine].spacing(SP.xs);
    column![
        text(user(head)).size(TS.body),
        text(uses)
            .size(TS.caption)
            .color(tokens::muted_text(tokens)),
        actions,
    ]
    .spacing(SP.xs)
    .boxed()
}
