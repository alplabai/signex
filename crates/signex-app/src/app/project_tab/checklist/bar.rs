//! The bar over the Design Checklist (mockup row 206): the template's name
//! and the summary, the filter chips with their counts (All, Open, Done,
//! Waived, Mine), the filter box, Run checks, Add item and the More menu
//! (Save as template…, Export CSV…, Reset from template). Save as template
//! opens a name box under the bar.

use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_types::project::{DesignChecklist, ItemState, TemplateRef};
use signex_widgets::{text, text_input};

use super::super::look::{ButtonKind, Glyph, field_label, text_button, ui_font};
use super::super::table::{SP, TS, note};
use super::view::Ctx;
use super::words::template_words;
use super::{ChecklistFilter, ChecklistMsg, message};
use crate::app::Message;
use crate::styles::tokens::muted_text;
use crate::styles::{HAIRLINE, Radii, ti};

/// Width of the filter box, in `body`-size characters (200 px).
const SEARCH_EMS: f32 = 16.7;
/// Width of the template name box (240 px).
const NAME_W: f32 = TS.body * 20.0;

/// The bar, and the Save as template name box while it is open.
pub(super) fn bar<'a>(ctx: &Ctx, checklist: &DesignChecklist) -> Element<'a, Message> {
    let mut block: Column<Element<'a, Message>> =
        Column::new().spacing(SP.s).push(line(ctx, checklist));
    if ctx.state.is_some_and(|state| state.more_open) {
        block = block.push(more_menu(ctx));
    }
    if let Some(name) = ctx.state.and_then(|state| state.template_name.as_ref()) {
        block = block.push(name_box(ctx, name));
    }
    block.width(Length::Fill).boxed()
}

/// The name of the template the checklist started from.
fn template_name(ctx: &Ctx, checklist: &DesignChecklist) -> signex_i18n::Localized {
    match checklist.template.as_deref().and_then(TemplateRef::parse) {
        Some(TemplateRef::Builtin(template)) => t!(ctx.i18n, template_words(template)),
        Some(TemplateRef::Saved(name)) => {
            t!(ctx.i18n, ids::CHECKLIST_SAVED_TEMPLATE, "name" => name.as_str())
        }
        None => t!(ctx.i18n, ids::CHECKLIST_OWN_LIST),
    }
}

/// The chips, each with how many items it shows.
fn chips(ctx: &Ctx, checklist: &DesignChecklist) -> Vec<(ChecklistFilter, MessageId, usize)> {
    let counts = checklist.counts(None);
    vec![
        (
            ChecklistFilter::All,
            ids::CHECKLIST_CHIP_ALL,
            counts.total(),
        ),
        (
            ChecklistFilter::State(ItemState::Open),
            ids::CHECKLIST_CHIP_OPEN,
            counts.open,
        ),
        (
            ChecklistFilter::State(ItemState::Done),
            ids::CHECKLIST_CHIP_DONE,
            counts.done,
        ),
        (
            ChecklistFilter::State(ItemState::Waived),
            ids::CHECKLIST_CHIP_WAIVED,
            counts.waived,
        ),
        (
            ChecklistFilter::Mine,
            ids::CHECKLIST_CHIP_MINE,
            checklist.owned_by(ctx.me),
        ),
    ]
}

/// The bar's one line.
fn line<'a>(ctx: &Ctx, checklist: &DesignChecklist) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let counts = checklist.counts(None);
    let summary = t!(
        i18n,
        ids::CHECKLIST_SUMMARY,
        "total" => counts.total(),
        "open" => counts.open,
        "waived" => counts.waived
    );
    let filter = ctx.state.map(|state| state.filter).unwrap_or_default();
    let mut line: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .push(
            text(template_name(ctx, checklist))
                .size(TS.body)
                .font(ui_font(iced::font::Weight::Semibold))
                .color(ti(tokens.text))
                .boxed(),
        )
        .push(
            text(summary)
                .size(TS.caption)
                .color(muted_text(tokens))
                .boxed(),
        )
        .push(Space::new().width(Length::Fill).boxed());
    for (chip, words, count) in chips(ctx, checklist) {
        let kind = if chip == filter {
            ButtonKind::Small
        } else {
            ButtonKind::Ghost
        };
        let press = ctx.send(ChecklistMsg::FilterPicked(chip));
        let words = t!(i18n, words, "count" => count);
        line = line.push(text_button(None, words, Some(press), kind, tokens));
    }
    let search = ctx
        .state
        .map(|state| state.search.clone())
        .unwrap_or_default();
    let project_id = ctx.project_id;
    let search = text_input(t!(i18n, ids::CHECKLIST_SEARCH), search)
        .on_input(move |text| message(project_id, ChecklistMsg::SearchEdited(text)))
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding(SP.xs)
        .width(Length::Fixed(TS.body * SEARCH_EMS));
    let running = ctx.state.is_some_and(|state| state.running);
    let run_words = if running {
        t!(i18n, ids::CHECKLIST_RUNNING)
    } else {
        t!(i18n, ids::CHECKLIST_RUN)
    };
    let run = (!running).then(|| ctx.send(ChecklistMsg::RunPressed));
    line.push(search.boxed())
        .push(text_button(
            Some(Glyph::Play),
            run_words,
            run,
            ButtonKind::Outline,
            tokens,
        ))
        .push(text_button(
            Some(Glyph::Plus),
            t!(i18n, ids::CHECKLIST_ADD),
            Some(ctx.send(ChecklistMsg::AddPressed)),
            ButtonKind::Primary,
            tokens,
        ))
        .push(text_button(
            Some(Glyph::ChevronDown),
            t!(i18n, ids::CHECKLIST_MORE),
            Some(ctx.send(ChecklistMsg::MoreToggled)),
            ButtonKind::Outline,
            tokens,
        ))
        .width(Length::Fill)
        .boxed()
}

/// The More menu's rows, under the bar at its right. Reset from template
/// is offered only for a checklist that started from one.
fn more_menu<'a>(ctx: &Ctx) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let has_template = ctx
        .data
        .design_checklist()
        .ok()
        .flatten()
        .and_then(|checklist| checklist.template)
        .and_then(|key| TemplateRef::parse(&key))
        .is_some();
    let reset = has_template.then(|| ctx.send(ChecklistMsg::ResetPressed));
    let rows = column![
        text_button(
            Some(Glyph::Save),
            t!(i18n, ids::CHECKLIST_SAVE_AS_TEMPLATE),
            Some(ctx.send(ChecklistMsg::SaveTemplateStarted)),
            ButtonKind::Ghost,
            tokens,
        ),
        text_button(
            Some(Glyph::File),
            t!(i18n, ids::CHECKLIST_EXPORT_CSV),
            Some(ctx.send(ChecklistMsg::ExportPressed)),
            ButtonKind::Ghost,
            tokens,
        ),
        text_button(
            Some(Glyph::Restore),
            t!(i18n, ids::CHECKLIST_RESET),
            reset,
            ButtonKind::Ghost,
            tokens,
        ),
    ]
    .spacing(SP.xs);
    let (fill, line) = (ti(tokens.panel_bg), ti(tokens.border));
    let menu = container(rows)
        .padding(SP.s)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: HAIRLINE,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    row![Space::new().width(Length::Fill), menu].boxed()
}

/// Save as template's name box, with Save and Cancel.
fn name_box<'a>(ctx: &Ctx, name: &str) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let project_id = ctx.project_id;
    let input = text_input(t!(i18n, ids::CHECKLIST_TEMPLATE_NAME), name.to_string())
        .on_input(move |text| message(project_id, ChecklistMsg::TemplateNameEdited(text)))
        .on_submit(ctx.send(ChecklistMsg::SaveTemplateSubmitted))
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding(SP.xs)
        .width(Length::Fixed(NAME_W));
    let line = row![
        input,
        text_button(
            None,
            t!(i18n, ids::CHECKLIST_TEMPLATE_SAVE),
            Some(ctx.send(ChecklistMsg::SaveTemplateSubmitted)),
            ButtonKind::Small,
            tokens,
        ),
        text_button(
            None,
            t!(i18n, ids::CHECKLIST_CANCEL),
            Some(ctx.send(ChecklistMsg::SaveTemplateCancelled)),
            ButtonKind::Ghost,
            tokens,
        ),
        note(t!(i18n, ids::CHECKLIST_TEMPLATE_NAME_HELP), tokens),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    column![
        field_label(t!(i18n, ids::CHECKLIST_TEMPLATE_NAME), tokens),
        line
    ]
    .spacing(SP.xs)
    .boxed()
}
