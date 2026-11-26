//! The selected item's pane (mockup row 206, Item): its title, state
//! (Open, Done, Waived… which asks for the reason and will not save without
//! one), stage, owner, link with what it says now, notes, the last refusal
//! and Remove. A text box commits on Enter.

use iced::widget::{Column, Space, column, container, pick_list, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, ids, t};
use signex_types::project::{ChecklistField, ChecklistItem, ChecklistStage, ItemState};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{text, text_input};

use super::super::look::{ButtonKind, Glyph, field_label, pill, text_button, ui_font};
use super::super::table::{SP, TS, note, problem};
use super::ChecklistMsg;
use super::view::{Ctx, result_tone};
use super::words::{problem_words, result_words, stage_words, state_words};
use crate::app::Message;
use crate::styles::{HAIRLINE, ti};

/// Width of the pane (320 px).
const PANE_W: f32 = TS.body * 80.0 / 3.0;

/// A stage in the Stage picker: the stage and its words.
#[derive(Debug, Clone, PartialEq)]
struct StageChoice {
    stage: ChecklistStage,
    label: String,
}

impl std::fmt::Display for StageChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// The text a field shows when nothing is typed: the stored value.
fn stored_text(item: &ChecklistItem, field: ChecklistField) -> String {
    match field {
        ChecklistField::Title => item.title.clone(),
        ChecklistField::Stage => item.stage.key().to_string(),
        ChecklistField::Owner => item.owner.clone().unwrap_or_default(),
        ChecklistField::Link => item
            .link
            .as_ref()
            .map(|link| link.to_text())
            .unwrap_or_default(),
        ChecklistField::Notes => item.notes.clone().unwrap_or_default(),
    }
}

/// A field's caption over its box: what was typed, else the stored value.
fn field<'a>(
    ctx: &Ctx,
    item: &ChecklistItem,
    which: ChecklistField,
    caption: Localized,
    hint: Localized,
) -> Element<'a, Message> {
    let draft = ctx
        .state
        .and_then(|state| state.drafts.get(&(item.id.clone(), which)))
        .cloned();
    let shown = draft.unwrap_or_else(|| stored_text(item, which));
    let project_id = ctx.project_id;
    let (typed, submitted) = (item.id.clone(), item.id.clone());
    let input = text_input(hint, shown)
        .on_input(move |text| {
            super::message(
                project_id,
                ChecklistMsg::FieldEdited {
                    item: typed.clone(),
                    field: which,
                    text,
                },
            )
        })
        .on_submit(ctx.send(ChecklistMsg::FieldSubmitted {
            item: submitted,
            field: which,
        }))
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding(SP.xs)
        .width(Length::Fill);
    column![field_label(caption, ctx.tokens), input]
        .spacing(SP.xs)
        .boxed()
}

/// Open, Done and Waived…: the item's state is the filled one.
fn state_buttons<'a>(ctx: &Ctx, item: &ChecklistItem) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let kind = |state: ItemState| {
        if item.state == state {
            ButtonKind::Primary
        } else {
            ButtonKind::Outline
        }
    };
    let pick = |state: ItemState| {
        ctx.send(ChecklistMsg::StatePicked {
            item: item.id.clone(),
            state,
        })
    };
    let waive = ctx.send(ChecklistMsg::WaiveStarted {
        item: item.id.clone(),
    });
    row![
        text_button(
            None,
            t!(i18n, state_words(ItemState::Open)),
            Some(pick(ItemState::Open)),
            kind(ItemState::Open),
            tokens,
        ),
        text_button(
            None,
            t!(i18n, state_words(ItemState::Done)),
            Some(pick(ItemState::Done)),
            kind(ItemState::Done),
            tokens,
        ),
        text_button(
            None,
            t!(i18n, ids::CHECKLIST_WAIVE),
            Some(waive),
            kind(ItemState::Waived),
            tokens,
        ),
    ]
    .spacing(SP.s)
    .boxed()
}

/// The reason box Waived… opens, with Waive; or why the item was waived.
fn reason<'a>(ctx: &Ctx, item: &ChecklistItem) -> Option<Element<'a, Message>> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let typed = ctx
        .state
        .and_then(|state| state.waiving.as_ref())
        .filter(|(waiving, _)| *waiving == item.id)
        .map(|(_, reason)| reason.clone());
    let Some(typed) = typed else {
        let reason = item
            .reason
            .as_deref()
            .filter(|_| item.state == ItemState::Waived)?;
        let words = t!(i18n, ids::CHECKLIST_WAIVED_BECAUSE, "reason" => reason);
        return Some(note(words, tokens));
    };
    let project_id = ctx.project_id;
    let input = text_input(t!(i18n, ids::CHECKLIST_REASON_HINT), typed)
        .on_input(move |text| super::message(project_id, ChecklistMsg::ReasonEdited(text)))
        .on_submit(ctx.send(ChecklistMsg::WaiveSubmitted))
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding(SP.xs)
        .width(Length::Fill);
    let confirm = text_button(
        None,
        t!(i18n, ids::CHECKLIST_WAIVE_CONFIRM),
        Some(ctx.send(ChecklistMsg::WaiveSubmitted)),
        ButtonKind::Small,
        tokens,
    );
    let line = row![input, confirm]
        .spacing(SP.s)
        .align_y(Alignment::Center);
    let block = column![
        field_label(t!(i18n, ids::CHECKLIST_FIELD_REASON), tokens),
        line
    ]
    .spacing(SP.xs);
    Some(block.boxed())
}

/// The Stage picker: a pick commits at once.
fn stage_picker<'a>(ctx: &Ctx, item: &ChecklistItem) -> Element<'a, Message> {
    let choices: Vec<StageChoice> = ChecklistStage::ALL
        .into_iter()
        .map(|stage| StageChoice {
            stage,
            label: t!(ctx.i18n, stage_words(stage)).to_string(),
        })
        .collect();
    let selected = choices.iter().find(|c| c.stage == item.stage).cloned();
    let project_id = ctx.project_id;
    let id = item.id.clone();
    let picker = pick_list(selected, choices, StageChoice::to_string)
        .graphite()
        .on_select(move |choice: StageChoice| {
            super::message(
                project_id,
                ChecklistMsg::StagePicked {
                    item: id.clone(),
                    stage: choice.stage,
                },
            )
        })
        .width(Length::Fill);
    column![
        field_label(t!(ctx.i18n, ids::CHECKLIST_FIELD_STAGE), ctx.tokens),
        picker
    ]
    .spacing(SP.xs)
    .boxed()
}

/// The pane of `item`.
pub(super) fn pane<'a>(ctx: &Ctx, item: &ChecklistItem) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let header = row![
        text(t!(i18n, ids::CHECKLIST_DETAIL_TITLE))
            .size(TS.body)
            .font(ui_font(iced::font::Weight::Semibold))
            .color(ti(tokens.text)),
        Space::new().width(Length::Fill),
        text_button(
            Some(Glyph::Close),
            t!(i18n, ids::CHECKLIST_CLOSE_DETAIL),
            Some(ctx.send(ChecklistMsg::DetailClosed)),
            ButtonKind::Ghost,
            tokens,
        ),
    ]
    .align_y(Alignment::Center);
    let mut body: Column<Element<'a, Message>> = Column::new()
        .spacing(SP.l)
        .push(header.boxed())
        .push(field(
            ctx,
            item,
            ChecklistField::Title,
            t!(i18n, ids::CHECKLIST_FIELD_TITLE),
            t!(i18n, ids::CHECKLIST_FIELD_TITLE),
        ))
        .push(state_buttons(ctx, item));
    if let Some(reason) = reason(ctx, item) {
        body = body.push(reason);
    }
    body = body
        .push(stage_picker(ctx, item))
        .push(field(
            ctx,
            item,
            ChecklistField::Owner,
            t!(i18n, ids::CHECKLIST_FIELD_OWNER),
            t!(i18n, ids::CHECKLIST_FIELD_OWNER),
        ))
        .push(field(
            ctx,
            item,
            ChecklistField::Link,
            t!(i18n, ids::CHECKLIST_FIELD_LINK),
            t!(i18n, ids::CHECKLIST_FIELD_LINK_HINT),
        ));
    if let Some(result) = ctx.result_of(item)
        && let Some(words) = result_words(result, i18n)
    {
        body = body.push(pill(words, result_tone(result), None, tokens));
    }
    body = body.push(field(
        ctx,
        item,
        ChecklistField::Notes,
        t!(i18n, ids::CHECKLIST_FIELD_NOTES),
        t!(i18n, ids::CHECKLIST_FIELD_NOTES),
    ));
    if let Some(refused) = ctx.state.and_then(|state| state.problem.as_ref()) {
        body = body.push(problem(problem_words(refused, i18n), tokens));
    }
    body = body.push(text_button(
        Some(Glyph::Trash),
        t!(i18n, ids::CHECKLIST_REMOVE),
        Some(ctx.send(ChecklistMsg::Removed {
            item: item.id.clone(),
        })),
        ButtonKind::Outline,
        tokens,
    ));
    let (fill, line) = (ti(tokens.panel_bg), ti(tokens.border));
    container(body)
        .width(Length::Fixed(PANE_W))
        .height(Length::Fill)
        .padding(SP.l)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border::default()
                .left(iced::border::Side::default().width(HAIRLINE).color(line)),
            ..container::Style::default()
        })
        .boxed()
}
