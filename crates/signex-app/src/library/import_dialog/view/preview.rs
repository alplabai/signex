//! Step 2, preview (mockup row 211): where the library goes, the dry run's
//! counts, the severity chips and filter over the report's items, the
//! selected item's details, and the amber strip for what will be skipped.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, RenderTranslatable, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, text_input};

use super::{ImportDialogContext, SP, TS, caption, chip, count_tile, mono, tag};
use crate::library::import_dialog::preview::{MAX_ROWS, detail_lines};
use crate::library::import_dialog::{ImportDialogMsg, ImportDialogState, PreviewState, msg};
use crate::library::messages::LibraryMessage;
use crate::library::modal_chrome::{
    footer_strip, key_value_box, notice_banner, primary_btn_if, secondary_btn,
};
use crate::library_import::RunOutcome;
use crate::library_import::report::{ReportItem, Severity};
use crate::styles::{HAIRLINE, PillTone, Radii, ti, tokens};

/// Height of the item list box.
const LIST_H: f32 = 200.0;
const ROW_H: f32 = 26.0;

pub(super) fn severity_tone(severity: Severity) -> PillTone {
    match severity {
        Severity::Error => PillTone::Err,
        Severity::Dropped => PillTone::Warn,
        Severity::Approximated => PillTone::Info,
        Severity::Info | Severity::Other => PillTone::Neutral,
    }
}

pub(super) fn severity_label(severity: Severity, i18n: &I18n) -> Localized {
    match severity {
        Severity::Error => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_SEVERITY_ERROR),
        Severity::Dropped => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_SEVERITY_DROPPED),
        Severity::Approximated => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_SEVERITY_APPROXIMATED),
        Severity::Info => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_SEVERITY_INFO),
        Severity::Other => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_SEVERITY_OTHER),
    }
}

pub(super) fn body<'a>(
    state: &'a ImportDialogState,
    ctx: ImportDialogContext<'a>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mut col: Column<Element<'a, LibraryMessage>> =
        column![route(state, tokens, i18n)].spacing(SP.l);
    match &state.preview {
        None | Some(PreviewState::Running) => {
            col = col.push(caption(
                t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PREVIEW_RUNNING),
                tokens,
            ));
            col = col.push(super::progress::current_line(state, ctx, tokens, i18n));
        }
        Some(PreviewState::Failed(reason)) => {
            let words = t!(
                i18n,
                ids::LIBRARY_IMPORT_DIALOG_PREVIEW_FAILED,
                "error" => reason.localized(i18n).into_string()
            );
            let words = text(words).size(TS.body).boxed();
            col = col.push(notice_banner(PillTone::Err, words, Vec::new(), tokens));
        }
        Some(PreviewState::Ready(outcome)) => col = ready(col, state, outcome, tokens, i18n),
    }
    col.boxed()
}

/// The kind chips, the source and where it goes.
fn route<'a>(
    state: &'a ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mut line = row![].spacing(SP.m).align_y(Alignment::Center);
    for kind in state.detected_kinds() {
        line = line.push(tag(
            UserText::new(kind.label.clone()),
            PillTone::Accent,
            tokens,
        ));
    }
    if let Some(first) = state.sources.first() {
        line = line.push(mono(first.display().to_string(), tokens));
    }
    line = line.push(caption(
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_ROUTE_ARROW),
        tokens,
    ));
    line = line.push(mono(state.location.clone(), tokens));
    line.boxed()
}

fn ready<'a>(
    mut col: Column<Element<'a, LibraryMessage>>,
    state: &'a ImportDialogState,
    outcome: &'a RunOutcome,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Column<Element<'a, LibraryMessage>> {
    let counts = outcome.summary.counts;
    let tiles = row![
        count_tile(
            counts.parts,
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COUNT_PARTS),
            PillTone::Neutral,
            tokens
        ),
        count_tile(
            counts.symbols,
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COUNT_SYMBOLS),
            PillTone::Neutral,
            tokens
        ),
        count_tile(
            counts.footprints,
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COUNT_FOOTPRINTS),
            PillTone::Neutral,
            tokens
        ),
        count_tile(
            counts.models,
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COUNT_MODELS),
            PillTone::Info,
            tokens
        ),
        count_tile(
            counts.skipped,
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COUNT_SKIPPED),
            PillTone::Warn,
            tokens
        ),
    ]
    .spacing(SP.m);
    col = col.push(tiles.boxed());
    if let Some(why) = &outcome.report_problem {
        col = col.push(caption(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_REPORT_MISSING, "why" => why.as_str()),
            tokens,
        ));
    }
    col = col.push(filter_row(state, tokens, i18n));
    col = col.push(item_list(state, tokens, i18n));
    col = col.push(selected_detail(state, tokens, i18n));
    if counts.skipped > 0 {
        let words = t!(
            i18n,
            ids::LIBRARY_IMPORT_DIALOG_SKIPPED_BANNER,
            "count" => counts.skipped
        );
        let words = text(words).size(TS.body).boxed();
        col = col.push(notice_banner(PillTone::Warn, words, Vec::new(), tokens));
    }
    col
}

/// The severity chips (a press hides or shows that severity) and the
/// filter field.
fn filter_row<'a>(
    state: &'a ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mut line = row![].spacing(SP.s).align_y(Alignment::Center);
    for (severity, count) in state.severity_counts() {
        let label = t!(
            i18n,
            ids::LIBRARY_IMPORT_DIALOG_SEVERITY_CHIP,
            "label" => severity_label(severity, i18n).into_string(),
            "count" => count
        );
        line = line.push(chip(
            label,
            !state.hidden.contains(&severity),
            severity_tone(severity),
            msg(ImportDialogMsg::SeverityToggled(severity)),
            tokens,
        ));
    }
    let filter = text_input(
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_FILTER_PLACEHOLDER),
        &state.filter,
    )
    .style(crate::panels::form_look::text_input_style)
    .on_input(|v| msg(ImportDialogMsg::FilterEdited(v)))
    .size(TS.body)
    .padding(SP.s)
    .width(Length::Fill);
    line.push(filter.boxed()).boxed()
}

/// The report items the filter shows, at most [`MAX_ROWS`], in a bordered
/// scrolling box.
fn item_list<'a>(
    state: &'a ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let visible = state.visible_items();
    if state.report_items().is_empty() {
        return caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_LIST_EMPTY), tokens);
    }
    if visible.is_empty() {
        return caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_LIST_NONE_MATCH), tokens);
    }
    let rows = visible.iter().take(MAX_ROWS).map(|&(index, item)| {
        item_row(
            index,
            item,
            state.selected_item == Some(index),
            tokens,
            i18n,
        )
    });
    let line = ti(tokens.border);
    let list = container(
        scrollable(column(rows).width(Length::Fill))
            .height(Length::Fixed(LIST_H))
            .width(Length::Fill),
    )
    .padding(SP.xs)
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: HAIRLINE,
            color: line,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let mut col = column![list].spacing(SP.xs);
    if visible.len() > MAX_ROWS {
        col = col.push(caption(
            t!(
                i18n,
                ids::LIBRARY_IMPORT_DIALOG_LIST_MORE,
                "shown" => MAX_ROWS,
                "count" => visible.len()
            ),
            tokens,
        ));
    }
    col.boxed()
}

/// One item: its severity, its name in the source and the message.
fn item_row<'a>(
    index: usize,
    item: &'a ReportItem,
    selected: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = tokens::resolve(tokens);
    let ink = ti(tokens.text);
    let line = row![
        container(tag(
            severity_label(item.severity, i18n),
            severity_tone(item.severity),
            tokens
        ))
        .width(Length::Fixed(110.0)),
        container(mono(item.source.clone(), tokens)).width(Length::FillPortion(2)),
        text(UserText::new(item.message.clone()))
            .size(TS.body)
            .color(r.ink3)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
            .width(Length::FillPortion(3)),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let (hover, picked) = (tokens::hover_overlay(tokens), r.accent_soft);
    button(line)
        .height(Length::Fixed(ROW_H))
        .width(Length::Fill)
        .padding([0.0, SP.s])
        .on_press(msg(ImportDialogMsg::ItemSelected(index)))
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            let fill = if selected {
                Some(picked)
            } else {
                hovered.then_some(hover)
            };
            iced::widget::button::Style {
                background: fill.map(Background::Color),
                text_color: ink,
                border: Border {
                    radius: Radii::GRAPHITE.small.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .boxed()
}

/// The selected item's details: kind, source, code, what it was written
/// as, its message and detail fields. A dry run draws no symbol or
/// footprint; the caption under it says so.
fn selected_detail<'a>(
    state: &'a ImportDialogState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let no_drawing = caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_NO_DRAWING), tokens);
    let Some(item) = state.selected_report_item() else {
        return column![
            caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_DETAIL_HINT), tokens),
            no_drawing
        ]
        .spacing(SP.xs)
        .boxed();
    };
    let value =
        |s: String| -> Element<'a, LibraryMessage> { text(UserText::new(s)).size(TS.body).boxed() };
    let written: Element<'a, LibraryMessage> = match &item.id {
        Some(id) => mono(id.clone(), tokens),
        None => text(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_DETAIL_NOT_WRITTEN))
            .size(TS.body)
            .boxed(),
    };
    let mut rows = vec![
        (
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_DETAIL_KIND),
            value(item.kind.clone()),
        ),
        (
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_DETAIL_SOURCE),
            mono(item.source.clone(), tokens),
        ),
        (
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_DETAIL_CODE),
            mono(item.code.clone(), tokens),
        ),
        (t!(i18n, ids::LIBRARY_IMPORT_DIALOG_DETAIL_WRITTEN), written),
    ];
    let details: Vec<(String, String)> = detail_lines(item);
    let mut extra: Column<Element<'a, LibraryMessage>> = column![
        text(UserText::new(item.message.clone()))
            .size(TS.body)
            .wrapping(Wrapping::WordOrGlyph)
    ]
    .spacing(SP.xs);
    for (key, shown) in details {
        extra = extra.push(caption(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_DETAIL_LINE, "key" => key, "value" => shown),
            tokens,
        ));
    }
    rows.push((severity_label(item.severity, i18n), extra.boxed()));
    column![
        key_value_box(rows, tokens),
        no_drawing,
        Space::new().height(SP.xs)
    ]
    .spacing(SP.xs)
    .boxed()
}

pub(super) fn footer<'a>(
    state: &'a ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let parts = state.parts_to_import();
    let label = if state.checking_target {
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_CHECKING)
    } else {
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_IMPORT_PARTS, "count" => parts)
    };
    let ready = state.preview_outcome().is_some() && !state.checking_target;
    footer_strip(
        vec![caption(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_NOTHING_WRITTEN),
            tokens,
        )],
        vec![
            secondary_btn(
                t!(i18n, ids::DIALOG_BUTTON_BACK),
                msg(ImportDialogMsg::BackPressed),
                tokens,
            ),
            primary_btn_if(label, msg(ImportDialogMsg::ImportPressed), ready, tokens),
        ],
        tokens,
    )
}
