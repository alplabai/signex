//! The empty Design Checklist (mockup row 206, "Empty: Start from
//! template"): the three built-in templates with their item counts per
//! stage, the user's own saved templates, Start from template, Import from
//! another project… and the note that nothing is written until Save.

use iced::widget::{Column, Row, column, row};
use iced::{Alignment, Element, Widget as _};
use signex_i18n::{ids, t};
use signex_types::project::{
    CHECKLIST_TEMPLATES, ChecklistStage, ChecklistTemplate, SavedTemplate, TemplateRef,
};
use signex_widgets::text;

use super::super::look::{ButtonKind, Glyph, text_button, ui_font};
use super::super::table::{SP, TS, note, problem};
use super::view::{Ctx, notice_line};
use super::words::{problem_words, template_help, template_words};
use super::{ChecklistMsg, SavedTemplates};
use crate::app::Message;
use crate::styles::ti;

/// The items of each stage, as a template card's line under its name says.
fn stage_line<'a>(ctx: &Ctx, count: impl Fn(ChecklistStage) -> usize) -> Element<'a, Message> {
    let words = t!(
        ctx.i18n,
        ids::CHECKLIST_TEMPLATE_STAGES,
        "schematic" => count(ChecklistStage::Schematic),
        "layout" => count(ChecklistStage::Layout),
        "fabrication" => count(ChecklistStage::Fabrication)
    );
    note(words, ctx.tokens)
}

/// A built-in template's card.
fn builtin_card<'a>(ctx: &Ctx, template: ChecklistTemplate, picked: bool) -> Element<'a, Message> {
    let kind = if picked {
        ButtonKind::Primary
    } else {
        ButtonKind::Outline
    };
    let items = template.items();
    let pick = ChecklistMsg::TemplatePicked(TemplateRef::Builtin(template));
    column![
        text_button(
            None,
            t!(ctx.i18n, template_words(template)),
            Some(ctx.send(pick)),
            kind,
            ctx.tokens,
        ),
        note(
            t!(ctx.i18n, template_help(template), "count" => items.len()),
            ctx.tokens
        ),
        stage_line(ctx, |stage| items
            .iter()
            .filter(|i| i.stage == stage)
            .count()),
    ]
    .spacing(SP.xs)
    .boxed()
}

/// A saved template's card.
fn saved_card<'a>(ctx: &Ctx, template: &SavedTemplate, picked: bool) -> Element<'a, Message> {
    let kind = if picked {
        ButtonKind::Primary
    } else {
        ButtonKind::Outline
    };
    let help = match &template.saved_from {
        Some(project) => t!(
            ctx.i18n,
            ids::CHECKLIST_SAVED_TEMPLATE_FROM,
            "count" => template.items.len(),
            "project" => project.as_str()
        ),
        None => t!(
            ctx.i18n,
            ids::CHECKLIST_SAVED_TEMPLATE_HELP,
            "count" => template.items.len()
        ),
    };
    let pick = ChecklistMsg::TemplatePicked(TemplateRef::Saved(template.name.clone()));
    column![
        text_button(
            None,
            t!(ctx.i18n, ids::CHECKLIST_SAVED_TEMPLATE, "name" => template.name.as_str()),
            Some(ctx.send(pick)),
            kind,
            ctx.tokens,
        ),
        note(help, ctx.tokens),
        stage_line(ctx, |stage| template.count(stage)),
    ]
    .spacing(SP.xs)
    .boxed()
}

/// The user's templates, under their heading; nothing when there are none.
fn saved_cards<'a>(ctx: &Ctx, picked: &TemplateRef) -> Option<Element<'a, Message>> {
    let saved = &ctx.state?.saved;
    if let SavedTemplates::Unreadable(_) = saved {
        return None;
    }
    let list = saved.list();
    if list.is_empty() {
        return None;
    }
    let mut cards: Row<Element<'a, Message>> = Row::new().spacing(SP.l);
    for template in list {
        let is_picked = *picked == TemplateRef::Saved(template.name.clone());
        cards = cards.push(saved_card(ctx, template, is_picked));
    }
    let block = column![
        text(t!(ctx.i18n, ids::CHECKLIST_YOUR_TEMPLATES))
            .size(TS.body)
            .font(ui_font(iced::font::Weight::Semibold))
            .color(ti(ctx.tokens.text)),
        cards,
    ]
    .spacing(SP.s);
    Some(block.boxed())
}

/// The empty tab.
pub(super) fn empty<'a>(ctx: &Ctx) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let picked = ctx
        .state
        .and_then(|state| state.template.clone())
        .unwrap_or(TemplateRef::Builtin(ChecklistTemplate::ReleaseReview));
    let mut cards: Row<Element<'a, Message>> = Row::new().spacing(SP.l);
    for template in CHECKLIST_TEMPLATES {
        let is_picked = picked == TemplateRef::Builtin(template);
        cards = cards.push(builtin_card(ctx, template, is_picked));
    }
    let start = row![
        text_button(
            None,
            t!(i18n, ids::CHECKLIST_START),
            Some(ctx.send(ChecklistMsg::StartPressed)),
            ButtonKind::PrimaryLarge,
            tokens,
        ),
        text_button(
            Some(Glyph::Folder),
            t!(i18n, ids::CHECKLIST_IMPORT),
            Some(ctx.send(ChecklistMsg::ImportPressed)),
            ButtonKind::Outline,
            tokens,
        ),
        note(t!(i18n, ids::CHECKLIST_EMPTY_NOTE), tokens),
    ]
    .spacing(SP.l)
    .align_y(Alignment::Center);
    let mut body: Column<Element<'a, Message>> = column![
        text(t!(i18n, ids::CHECKLIST_EMPTY_TITLE))
            .size(TS.heading)
            .font(ui_font(iced::font::Weight::Semibold))
            .color(ti(tokens.text)),
        note(t!(i18n, ids::CHECKLIST_EMPTY_HELP), tokens),
        cards,
    ]
    .spacing(SP.l);
    if let Some(saved) = saved_cards(ctx, &picked) {
        body = body.push(saved);
    }
    body = body.push_boxed(start);
    if let Some(line) = notice_line(ctx) {
        body = body.push(line);
    }
    if let Some(refused) = ctx.state.and_then(|state| state.problem.as_ref()) {
        body = body.push(problem(problem_words(refused, i18n), tokens));
    }
    body.boxed()
}
