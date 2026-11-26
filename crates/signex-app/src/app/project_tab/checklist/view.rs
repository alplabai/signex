//! The Design Checklist tab's view (mockup row 206). Pure: the project and
//! the tab's state in, widgets out. Started, it is the bar (name, summary,
//! filter, Run checks, Add item), the progress per stage, then the items
//! grouped by stage, with the selected item's pane at the right (`detail`).
//! Empty, it offers the templates; unreadable, it says so and offers nothing.

use iced::widget::{Column, Row, Space, column, container, mouse_area, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::check::CheckReport;
use signex_types::project::{
    CHECKLIST_TEMPLATES, ChecklistItem, ChecklistStage, ChecklistTemplate, DesignChecklist,
    ItemState, LinkResult, ProjectData, StateCounts,
};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, text_input};

use super::super::look::{ButtonKind, Glyph, pill, text_button, ui_font};
use super::super::table::{SP, TS, note, page_scroller, problem};
use super::words::{
    link_words, problem_words, result_words, stage_words, state_words, template_help,
    template_words,
};
use super::{ChecklistMsg, ChecklistTabState, message};
use crate::app::Message;
use crate::app::state::{LoadedProject, ProjectId};
use crate::styles::tokens::{muted_text, resolve};
use crate::styles::{HAIRLINE, PillTone, Radii, ti};

/// Width of the State column (92 px, as drawn).
const STATE_W: f32 = TS.body * 23.0 / 3.0;
/// Width of the Owner column (112 px).
const OWNER_W: f32 = TS.body * 28.0 / 3.0;
/// Width of the Linked to column (252 px).
const LINK_W: f32 = TS.body * 21.0;
/// Width of the Updated column (80 px, a whole date).
const UPDATED_W: f32 = TS.body * 20.0 / 3.0;
/// Height of a progress bar (6 px).
const BAR_H: f32 = TS.body / 2.0;
/// Width of a stage heading's bar (120 px).
const GROUP_BAR_W: f32 = TS.body * 10.0;
/// Width of the filter box, in `body`-size characters (200 px).
const SEARCH_EMS: f32 = 16.7;

/// What every part of the view reads.
pub(super) struct Ctx<'s> {
    pub(super) project_id: ProjectId,
    pub(super) data: &'s ProjectData,
    pub(super) state: Option<&'s ChecklistTabState>,
    pub(super) report: Option<&'s CheckReport>,
    pub(super) i18n: &'s I18n,
    pub(super) tokens: &'s ThemeTokens,
}

impl Ctx<'_> {
    pub(super) fn send(&self, msg: ChecklistMsg) -> Message {
        message(self.project_id, msg)
    }

    /// What item `item`'s link says now; `None` without a link.
    pub(super) fn result_of(&self, item: &ChecklistItem) -> Option<LinkResult> {
        item.link
            .as_ref()
            .map(|link| link.result(self.data, self.report))
    }
}

pub(super) fn view<'a>(
    project: &LoadedProject,
    state: Option<&ChecklistTabState>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ctx = Ctx {
        project_id: project.id,
        data: &project.data,
        state,
        report: state.and_then(ChecklistTabState::report),
        i18n,
        tokens,
    };
    match project.data.design_checklist() {
        Err(_) => padded(problem(t!(i18n, ids::CHECKLIST_UNREADABLE), tokens), tokens),
        Ok(None) => padded(empty(&ctx), tokens),
        Ok(Some(checklist)) => started(&ctx, &checklist),
    }
}

/// `content` padded and scrolling like a Project tab page.
fn padded<'a>(content: Element<'a, Message>, tokens: &ThemeTokens) -> Element<'a, Message> {
    let inner = container(content)
        .width(Length::Fill)
        .padding([SP.xl + SP.s, SP.xl * 2.0]);
    page_scroller(inner.boxed(), tokens)
}

/// The empty tab: the templates, Start from template, and the note that
/// nothing is written until Save.
fn empty<'a>(ctx: &Ctx) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let picked = ctx
        .state
        .and_then(|state| state.template)
        .unwrap_or(ChecklistTemplate::ReleaseReview);
    let mut cards: Row<Element<'a, Message>> = Row::new().spacing(SP.l);
    for template in CHECKLIST_TEMPLATES {
        let kind = if template == picked {
            ButtonKind::Primary
        } else {
            ButtonKind::Outline
        };
        let count = template.items().len();
        let card = column![
            text_button(
                None,
                t!(i18n, template_words(template)),
                Some(ctx.send(ChecklistMsg::TemplatePicked(template))),
                kind,
                tokens,
            ),
            note(t!(i18n, template_help(template), "count" => count), tokens),
        ]
        .spacing(SP.xs);
        cards = cards.push(card.boxed());
    }
    let start = row![
        text_button(
            None,
            t!(i18n, ids::CHECKLIST_START),
            Some(ctx.send(ChecklistMsg::StartPressed)),
            ButtonKind::PrimaryLarge,
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
        start,
    ]
    .spacing(SP.l);
    if let Some(refused) = ctx.state.and_then(|state| state.problem.as_ref()) {
        body = body.push(problem(problem_words(refused, i18n), tokens));
    }
    body.boxed()
}

/// A started checklist: the list column, and the selected item's pane.
fn started<'a>(ctx: &Ctx, checklist: &DesignChecklist) -> Element<'a, Message> {
    let mut list: Column<Element<'a, Message>> = Column::new()
        .spacing(SP.l)
        .push(bar(ctx, checklist))
        .push(stage_cards(ctx, checklist));
    if let Some(reason) = run_failed(ctx) {
        list = list.push(problem(reason, ctx.tokens));
    }
    let selected = ctx
        .state
        .and_then(|state| state.selected.as_deref())
        .and_then(|id| checklist.item(id));
    if selected.is_none()
        && let Some(refused) = ctx.state.and_then(|state| state.problem.as_ref())
    {
        list = list.push(problem(problem_words(refused, ctx.i18n), ctx.tokens));
    }
    list = list.push(table(ctx, checklist));
    let list = padded(list.boxed(), ctx.tokens);
    match selected {
        Some(item) => row![list, super::detail::pane(ctx, item)]
            .height(Length::Fill)
            .boxed(),
        None => list,
    }
}

/// Why the last run did not finish, when it did not.
fn run_failed(ctx: &Ctx) -> Option<Localized> {
    let run = ctx.state?.last_run.as_deref()?;
    let reason = run.report.as_ref().err()?;
    Some(t!(ctx.i18n, ids::CHECKLIST_RUN_FAILED, "reason" => reason.as_str()))
}

/// The bar over the list: the name, the summary, the state filter, the
/// filter box, Run checks and Add item.
fn bar<'a>(ctx: &Ctx, checklist: &DesignChecklist) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let name = match checklist
        .template
        .as_deref()
        .and_then(ChecklistTemplate::parse)
    {
        Some(template) => t!(i18n, template_words(template)),
        None => t!(i18n, ids::CHECKLIST_OWN_LIST),
    };
    let counts = checklist.counts(None);
    let summary = t!(
        i18n,
        ids::CHECKLIST_SUMMARY,
        "total" => counts.total(),
        "open" => counts.open,
        "waived" => counts.waived
    );
    let filter = ctx.state.and_then(|state| state.filter);
    let mut line: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .push(
            text(name)
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
    let chips = [
        (None, ids::CHECKLIST_FILTER_ALL),
        (Some(ItemState::Open), ids::CHECKLIST_STATE_OPEN),
        (Some(ItemState::Done), ids::CHECKLIST_STATE_DONE),
        (Some(ItemState::Waived), ids::CHECKLIST_STATE_WAIVED),
    ];
    for (state, words) in chips {
        let kind = if state == filter {
            ButtonKind::Small
        } else {
            ButtonKind::Ghost
        };
        let press = ctx.send(ChecklistMsg::FilterPicked(state));
        line = line.push(text_button(
            None,
            t!(i18n, words),
            Some(press),
            kind,
            tokens,
        ));
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
        .width(Length::Fill)
        .boxed()
}

/// One card per stage and the release total: closed of all, the bar, and
/// the counts.
fn stage_cards<'a>(ctx: &Ctx, checklist: &DesignChecklist) -> Element<'a, Message> {
    let mut cards: Row<Element<'a, Message>> = Row::new().spacing(SP.m);
    for stage in ChecklistStage::ALL {
        let counts = checklist.counts(Some(stage));
        cards = cards.push(stage_card(ctx, t!(ctx.i18n, stage_words(stage)), counts));
    }
    let total = checklist.counts(None);
    let release = t!(ctx.i18n, ids::CHECKLIST_STAGE_RELEASE);
    cards = cards.push(stage_card(ctx, release, total));
    cards.width(Length::Fill).boxed()
}

fn stage_card<'a>(ctx: &Ctx, name: Localized, counts: StateCounts) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let progress = t!(
        i18n,
        ids::CHECKLIST_STAGE_PROGRESS,
        "closed" => counts.closed(),
        "total" => counts.total()
    );
    let detail = t!(
        i18n,
        ids::CHECKLIST_STAGE_COUNTS,
        "done" => counts.done,
        "waived" => counts.waived,
        "open" => counts.open
    );
    let head = row![
        text(name)
            .size(TS.body)
            .font(ui_font(iced::font::Weight::Semibold))
            .color(ti(tokens.text)),
        Space::new().width(Length::Fill),
        text(progress)
            .size(TS.caption)
            .color(ti(tokens.text_secondary)),
    ]
    .align_y(Alignment::Center);
    let body = column![head, progress_bar(counts, tokens), note(detail, tokens)].spacing(SP.s);
    let (fill, line) = (ti(tokens.panel_bg), ti(tokens.border));
    container(body)
        .width(Length::FillPortion(1))
        .padding([SP.m, SP.l])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: HAIRLINE,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// Done solid, waived light, open the track: a waived item counts as closed
/// and stays visible.
fn progress_bar<'a>(counts: StateCounts, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = resolve(tokens);
    let parts = [
        (counts.done, ti(tokens.success)),
        (counts.waived, r.ok_soft),
        (counts.open, r.sunken),
    ];
    let mut line: Row<Element<'a, Message>> = Row::new().width(Length::Fill);
    if counts.total() == 0 {
        line = line.push(segment(1, r.sunken));
    }
    for (count, color) in parts.into_iter().filter(|(count, _)| *count > 0) {
        line = line.push(segment(count, color));
    }
    container(line)
        .width(Length::Fill)
        .height(Length::Fixed(BAR_H))
        .boxed()
}

fn segment<'a>(count: usize, color: Color) -> Element<'a, Message> {
    let portion = u16::try_from(count).unwrap_or(u16::MAX);
    container(Space::new().width(Length::Fill).height(Length::Fill))
        .width(Length::FillPortion(portion))
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

/// Does `item` pass the state filter and the filter text?
fn shown(ctx: &Ctx, item: &ChecklistItem) -> bool {
    let Some(state) = ctx.state else {
        return true;
    };
    if state.filter.is_some_and(|filter| filter != item.state) {
        return false;
    }
    let needle = state.search.trim().to_lowercase();
    if needle.is_empty() {
        return true;
    }
    let link = item.link.as_ref().map(|link| link.to_text());
    [Some(&item.title), item.owner.as_ref(), link.as_ref()]
        .into_iter()
        .flatten()
        .any(|hay| hay.to_lowercase().contains(&needle))
}

/// A column heading.
fn heading<'a>(words: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(words)
        .size(TS.caption)
        .color(resolve(tokens).ink3)
        .boxed()
}

/// The items grouped by stage under a heading row; a stage with no item
/// shown is left out.
fn table<'a>(ctx: &Ctx, checklist: &DesignChecklist) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let head = cells(
        heading(t!(i18n, ids::CHECKLIST_COLUMN_STATE), tokens),
        heading(t!(i18n, ids::CHECKLIST_COLUMN_ITEM), tokens),
        heading(t!(i18n, ids::CHECKLIST_COLUMN_OWNER), tokens),
        heading(t!(i18n, ids::CHECKLIST_COLUMN_LINK), tokens),
        heading(t!(i18n, ids::CHECKLIST_COLUMN_UPDATED), tokens),
    );
    let mut rows: Column<Element<'a, Message>> = Column::new().push(head);
    let mut any = false;
    for stage in ChecklistStage::ALL {
        let items: Vec<&ChecklistItem> = checklist
            .items
            .iter()
            .filter(|item| item.stage == stage && shown(ctx, item))
            .collect();
        if items.is_empty() {
            continue;
        }
        any = true;
        rows = rows.push(group_head(ctx, stage, checklist.counts(Some(stage))));
        for item in items {
            rows = rows.push(item_row(ctx, item));
        }
    }
    if !any {
        let none = container(note(t!(i18n, ids::CHECKLIST_NO_MATCH), tokens)).padding([SP.m, SP.l]);
        rows = rows.push(none.boxed());
    }
    let line = ti(tokens.border);
    container(rows)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: HAIRLINE,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// One row's five cells at the column widths.
fn cells<'a>(
    state: Element<'a, Message>,
    item: Element<'a, Message>,
    owner: Element<'a, Message>,
    link: Element<'a, Message>,
    updated: Element<'a, Message>,
) -> Element<'a, Message> {
    row![
        container(state).width(Length::Fixed(STATE_W)),
        container(item).width(Length::Fill),
        container(owner).width(Length::Fixed(OWNER_W)),
        container(link).width(Length::Fixed(LINK_W)),
        container(updated).width(Length::Fixed(UPDATED_W)),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .padding([SP.s, SP.l])
    .boxed()
}

/// A stage's heading row: its name, the bar and closed of all.
fn group_head<'a>(ctx: &Ctx, stage: ChecklistStage, counts: StateCounts) -> Element<'a, Message> {
    let tokens = ctx.tokens;
    let progress = t!(
        ctx.i18n,
        ids::CHECKLIST_STAGE_PROGRESS,
        "closed" => counts.closed(),
        "total" => counts.total()
    );
    let line = row![
        text(t!(ctx.i18n, stage_words(stage)))
            .size(TS.body)
            .font(ui_font(iced::font::Weight::Semibold))
            .color(ti(tokens.text)),
        container(progress_bar(counts, tokens)).width(Length::Fixed(GROUP_BAR_W)),
        text(progress)
            .size(TS.caption)
            .color(ti(tokens.text_secondary)),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let fill = resolve(tokens).sunken;
    container(line)
        .width(Length::Fill)
        .padding([SP.s, SP.l])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            ..container::Style::default()
        })
        .boxed()
}

/// The tone of an item state's pill.
pub(super) fn state_tone(state: ItemState) -> PillTone {
    match state {
        ItemState::Open => PillTone::Neutral,
        ItemState::Done => PillTone::Ok,
        ItemState::Waived => PillTone::Info,
    }
}

/// The tone of a link result's pill.
pub(super) fn result_tone(result: LinkResult) -> PillTone {
    match result {
        LinkResult::Passes => PillTone::Ok,
        LinkResult::Fails(_) => PillTone::Err,
        LinkResult::NotRun | LinkResult::NoCheck => PillTone::Neutral,
    }
}

/// The link's words and what it says: a linked check that passes on an open
/// item says so and offers Mark done, which the owner presses; any other
/// result is a pill.
fn link_cell<'a>(ctx: &Ctx, item: &ChecklistItem) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let words = text(link_words(item.link.as_ref(), i18n))
        .size(TS.body)
        .color(ti(tokens.text_secondary));
    let mut link: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(words.boxed());
    match ctx.result_of(item) {
        Some(LinkResult::Passes) if item.state == ItemState::Open => {
            let done = ChecklistMsg::StatePicked {
                item: item.id.clone(),
                state: ItemState::Done,
            };
            link = link
                .push(pill(
                    t!(i18n, ids::CHECKLIST_CHECK_PASSES),
                    PillTone::Ok,
                    Some(Glyph::Check),
                    tokens,
                ))
                .push(text_button(
                    None,
                    t!(i18n, ids::CHECKLIST_MARK_DONE),
                    Some(ctx.send(done)),
                    ButtonKind::Small,
                    tokens,
                ));
        }
        Some(result) => {
            if let Some(words) = result_words(result, i18n) {
                link = link.push(pill(words, result_tone(result), None, tokens));
            }
        }
        None => {}
    }
    link.boxed()
}

/// One item: its state, title (and why it was waived), owner, link with the
/// link's result, and the day of its last edit. A click selects it.
fn item_row<'a>(ctx: &Ctx, item: &ChecklistItem) -> Element<'a, Message> {
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let state = pill(
        t!(i18n, state_words(item.state)),
        state_tone(item.state),
        None,
        tokens,
    );
    let mut title: Column<Element<'a, Message>> = Column::new().spacing(SP.xs).push(
        text(UserText::new(item.title.clone()))
            .size(TS.body)
            .color(ti(tokens.text))
            .boxed(),
    );
    if let Some(reason) = item
        .reason
        .as_deref()
        .filter(|_| item.state == ItemState::Waived)
    {
        let words = t!(i18n, ids::CHECKLIST_WAIVED_BECAUSE, "reason" => reason);
        title = title.push(note(words, tokens));
    }
    let owner = text(UserText::new(item.owner.clone().unwrap_or_default()))
        .size(TS.body)
        .color(ti(tokens.text_secondary))
        .boxed();
    let updated = text(UserText::new(item.updated.clone().unwrap_or_default()))
        .size(TS.caption)
        .color(muted_text(tokens))
        .boxed();
    let line = cells(state, title.boxed(), owner, link_cell(ctx, item), updated);
    let is_selected = ctx
        .state
        .and_then(|state| state.selected.as_deref())
        .is_some_and(|id| id == item.id);
    let (fill, rule) = (resolve(tokens).accent_soft, resolve(tokens).line_soft);
    let framed = container(line)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: is_selected.then_some(Background::Color(fill)),
            border: Border::default()
                .top(iced::border::Side::default().width(HAIRLINE).color(rule)),
            ..container::Style::default()
        });
    mouse_area(framed)
        .on_press(ctx.send(ChecklistMsg::ItemSelected(item.id.clone())))
        .boxed()
}
