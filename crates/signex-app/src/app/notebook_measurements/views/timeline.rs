//! The timeline in the notebook's margin lane (mockup row 199, decided as
//! drawn): the target's tabs (net, part, pin), the chart of its readings
//! against the limit and the computed value, the list newest first with
//! revision, board, conditions and operator, and the counts with export
//! CSV. Opened from a reference card, a notebook badge or Properties ▸
//! Design notes, which the Notebook tab routes here.

use iced::widget::{Column, Row, canvas, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_notebook::Reference;
use signex_notebook::measurement::{Timeline, TimelineEntry, Verdict, quantity_text};
use signex_notebook::si;
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::chart::{ChartInk, TimelineChart, layout, limit_line};
use super::{GAP, PAD, TS, target_label};
use crate::styles::{DialogButtonKind, dialog_button, ghost_button, ti, tokens::resolve};

/// The chart's height, from the type scale.
const CHART_HEIGHT: f32 = TS.body * 96.0 / 12.0;

/// What the timeline shows and where it is drawn.
pub(crate) struct TimelineView<'a> {
    pub(crate) timeline: &'a Timeline,
    /// The tabs: the net, part and pin the timeline was opened for.
    pub(crate) targets: &'a [Reference],
    pub(crate) tokens: &'a ThemeTokens,
    pub(crate) i18n: &'a I18n,
}

/// What the timeline's controls publish.
pub(crate) struct TimelineActions<M> {
    /// A tab was picked.
    pub(crate) target_picked: fn(Reference) -> M,
    /// A reading was clicked: its file and line, to show its source.
    pub(crate) reading_clicked: fn(String, u32) -> M,
    /// Export CSV was pressed.
    pub(crate) export_pressed: M,
}

/// The timeline panel.
pub(crate) fn timeline_panel<'a, M: Clone + 'a>(
    view: TimelineView<'a>,
    actions: TimelineActions<M>,
) -> Element<'a, M> {
    let TimelineView {
        timeline,
        targets,
        tokens,
        i18n,
    } = view;
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let mut title = vec![timeline.reference.name()];
    if !timeline.quantity.is_empty() {
        title.push(timeline.quantity.clone());
    }
    let heading = row![
        text(t!(i18n, ids::NOTEBOOK_TIMELINE_TITLE))
            .size(TS.body)
            .color(ink),
        text(UserText::new(title.join(" · ")))
            .size(TS.body)
            .color(r.ink3),
    ]
    .spacing(GAP * 1.5)
    .align_y(Alignment::Center);
    let tabs = Row::with_children(targets.iter().map(|target| {
        let active = target == &timeline.reference;
        let kind = if active {
            DialogButtonKind::Primary
        } else {
            DialogButtonKind::Secondary
        };
        let picked = (actions.target_picked)(target.clone());
        dialog_button(target_label(i18n, target), kind, Some(picked), tokens)
    }))
    .spacing(GAP);
    let mut body = Column::<Element<'a, M>>::new()
        .push(heading.boxed())
        .push(tabs.boxed())
        .spacing(PAD);
    body = body.push(chart_block(timeline, tokens, i18n));
    if timeline.entries.is_empty() {
        let name = timeline.reference.name();
        body = body.push(
            text(t!(i18n, ids::NOTEBOOK_TIMELINE_EMPTY, "name" => name))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    }
    for entry in &timeline.entries {
        body = body.push(entry_row(
            entry,
            timeline,
            tokens,
            i18n,
            actions.reading_clicked,
        ));
    }
    let summary = t!(
        i18n, ids::NOTEBOOK_TIMELINE_SUMMARY,
        "readings" => timeline.readings,
        "fails" => timeline.fails,
        "stale" => timeline.stale
    );
    let export = button(
        text(t!(i18n, ids::NOTEBOOK_TIMELINE_EXPORT))
            .size(TS.caption)
            .color(r.accent_text),
    )
    .padding(0)
    .style(ghost_button(tokens))
    .on_press(actions.export_pressed);
    body = body.push(
        row![text(summary).size(TS.caption).color(r.ink3), export]
            .spacing(GAP * 2.0)
            .align_y(Alignment::Center)
            .boxed(),
    );
    let fill = r.raised;
    let line = r.line_soft;
    container(body)
        .padding(PAD)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: line,
                width: 1.0,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The chart with its limit and computed captions; nothing while no
/// dated reading has a value.
fn chart_block<'a, M: Clone + 'a>(
    timeline: &'a Timeline,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, M> {
    let r = resolve(tokens);
    let Some(laid) = layout(timeline) else {
        return Column::<Element<'a, M>>::new().boxed();
    };
    let ink = ChartInk {
        line: r.ink3,
        point: ti(tokens.accent),
        fail: ti(tokens.error),
        stale: ti(tokens.warning),
        limit: ti(tokens.error),
        computed: r.info,
    };
    let mut captions = Row::new().spacing(GAP * 3.0);
    if let Some(limit) = timeline.limit {
        let value = quantity_text(&si::number(limit_line(limit), 3), &timeline.unit);
        captions = captions.push(
            text(t!(i18n, ids::NOTEBOOK_TIMELINE_LIMIT, "value" => value))
                .size(TS.caption)
                .color(ti(tokens.error)),
        );
    }
    if let Some(computed) = timeline.computed {
        let value = si::number(computed, 3);
        captions = captions.push(
            text(t!(i18n, ids::NOTEBOOK_TIMELINE_COMPUTED, "value" => value))
                .size(TS.caption)
                .color(r.info),
        );
    }
    column![
        captions,
        canvas(TimelineChart { layout: laid, ink })
            .width(Length::Fill)
            .height(Length::Fixed(CHART_HEIGHT)),
    ]
    .spacing(GAP)
    .boxed()
}

/// One reading: id, revision and board; conditions and operator; a fail
/// or stale note; the value. A click shows its source.
fn entry_row<'a, M: Clone + 'a>(
    entry: &'a TimelineEntry,
    timeline: &'a Timeline,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    clicked: fn(String, u32) -> M,
) -> Element<'a, M> {
    let r = resolve(tokens);
    let mut head = Vec::new();
    if let Some(id) = &entry.id {
        head.push(id.to_string());
    }
    if !entry.rev.is_empty() {
        head.push(t!(i18n, ids::NOTEBOOK_TIMELINE_REV, "rev" => entry.rev.as_str()).into_string());
    }
    if !entry.board.is_empty() {
        let board = entry.board.as_str();
        head.push(t!(i18n, ids::NOTEBOOK_TIMELINE_BOARD, "board" => board).into_string());
    }
    let mut lines = Column::new().spacing(GAP / 2.0).width(Length::Fill).push(
        text(UserText::new(head.join(" · ")))
            .size(TS.caption)
            .color(ti(tokens.text)),
    );
    let detail: Vec<&str> = [entry.conditions.as_str(), entry.operator.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect();
    if !detail.is_empty() {
        lines = lines.push(
            text(UserText::new(detail.join(" · ")))
                .size(TS.caption)
                .color(r.ink3),
        );
    }
    let (note, note_ink) = match (entry.verdict, entry.changes.first()) {
        (Some(Verdict::Fail), _) => {
            let limit = timeline
                .limit
                .map(|l| quantity_text(&si::number(limit_line(l), 3), &timeline.unit))
                .unwrap_or_default();
            let note = t!(i18n, ids::NOTEBOOK_TIMELINE_FAIL, "limit" => limit);
            (Some(note), ti(tokens.error))
        }
        (_, Some(change)) => {
            let note = t!(
                i18n, ids::NOTEBOOK_TIMELINE_STALE,
                "part" => change.part.as_str(),
                "was" => change.was.as_str()
            );
            (Some(note), ti(tokens.warning))
        }
        _ => (None, r.ink3),
    };
    if let Some(note) = note {
        lines = lines.push(text(note).size(TS.caption).color(note_ink));
    }
    let value_ink = match (entry.verdict, entry.is_stale()) {
        (Some(Verdict::Fail), _) => ti(tokens.error),
        (_, true) => ti(tokens.warning),
        _ => ti(tokens.text),
    };
    let value = quantity_text(&entry.value_text, &entry.unit);
    let content = row![
        lines,
        text(UserText::new(value)).size(TS.caption).color(value_ink),
    ]
    .spacing(GAP * 2.0)
    .align_y(Alignment::Start);
    button(content)
        .padding(GAP)
        .width(Length::Fill)
        .style(ghost_button(tokens))
        .on_press(clicked(entry.file.clone(), entry.line))
        .boxed()
}
