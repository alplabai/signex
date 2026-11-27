//! The end of step 3 (mockup row 211, "Done: result" and "Done: loss
//! report"): the result card, the loss report grouped by severity and code,
//! and the footer with Save report…, Open report and Open in Library
//! Browser. A stored report opened from the Library Browser shows only the
//! loss report.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, column, container, row};
use iced::{Alignment, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, RenderTranslatable, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{segmented, text};

use super::preview::{filter_row, severity_label, severity_tone};
use super::{ImportDialogContext, SP, TS, caption, count_tile, mono, tag};
use crate::library::import_dialog::preview::MAX_ROWS;
use crate::library::import_dialog::result::{
    ImportResult, Mounted, ReportNote, RequiredLibrary, ResultTab, RunFacts, SeverityGroup,
};
use crate::library::import_dialog::{ImportDialogMsg, ImportDialogState, msg};
use crate::library::messages::LibraryMessage;
use crate::library::modal_chrome::{
    cancel_btn, footer_strip, key_value_box, notice_banner, primary_btn_if,
};
use crate::library_import::report::Severity;
use crate::library_import::{ExitKind, RunMode};
use crate::styles::{DialogButtonKind, HAIRLINE, PillTone, Radii, dialog_button, ti};

/// How much of a commit id the History row shows.
const COMMIT_CHARS: usize = 8;

pub(super) fn body<'a>(
    state: &'a ImportDialogState,
    result: &'a ImportResult,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mut col: Column<Element<'a, LibraryMessage>> = column![].spacing(SP.l);
    match &result.run {
        Some(facts) => {
            col = col.push(tabs(state.result_tab, tokens, i18n));
            col = match state.result_tab {
                ResultTab::Result => col.push(result_card(result, facts, tokens, i18n)),
                ResultTab::Report => col.push(loss_report(state, result, tokens, i18n)),
            };
        }
        None => col = col.push(loss_report(state, result, tokens, i18n)),
    }
    if let Some(note) = &state.report_note {
        col = col.push(report_note(note, tokens, i18n));
    }
    col.boxed()
}

/// Result | Loss report.
fn tabs<'a>(current: ResultTab, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, LibraryMessage> {
    let style = crate::styles::tokens::resolve(tokens).segmented(TS.body);
    segmented(
        [
            (
                ResultTab::Result,
                t!(i18n, ids::LIBRARY_IMPORT_DIALOG_TAB_RESULT).into(),
            ),
            (
                ResultTab::Report,
                t!(i18n, ids::LIBRARY_IMPORT_DIALOG_TAB_REPORT).into(),
            ),
        ],
        current,
        style,
        |tab| msg(ImportDialogMsg::ResultTabPicked(tab)),
    )
}

/// The result card: what was imported, where it went, and the way to the
/// loss report.
fn result_card<'a>(
    result: &'a ImportResult,
    facts: &'a RunFacts,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let name = result.library_name();
    let title = match facts.mode {
        RunMode::Update => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_RESULT_UPDATED, "name" => name),
        RunMode::Import | RunMode::Preview | RunMode::Stage => {
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_RESULT_IMPORTED, "name" => name)
        }
    };
    let warn = |words: Localized| -> Element<'a, LibraryMessage> {
        notice_banner(
            PillTone::Warn,
            text(words).size(TS.body).boxed(),
            Vec::new(),
            tokens,
        )
    };
    let mut col = column![text(title).size(TS.title).color(ti(tokens.text))].spacing(SP.l);
    // Exit 2 (contract §3.3): written, with items dropped or not imported.
    if facts.exit == ExitKind::Partial {
        col = col.push(warn(t!(
            i18n,
            ids::LIBRARY_IMPORT_DIALOG_PARTIAL,
            "count" => result.lost_items()
        )));
    }
    col = col
        .push(result_tiles(result, facts, tokens, i18n))
        .push(key_value_box(
            result_rows(result, facts, tokens, i18n),
            tokens,
        ));
    if facts.mode == RunMode::Update && facts.local_commits > 0 {
        col = col.push(warn(t!(
            i18n,
            ids::LIBRARY_IMPORT_LOCAL_COMMITS,
            "count" => facts.local_commits
        )));
    }
    let summary = t!(
        i18n,
        ids::LIBRARY_IMPORT_DIALOG_REPORT_SUMMARY,
        "approximated" => result.count(Severity::Approximated),
        "dropped" => result.count(Severity::Dropped),
        "skipped" => facts.counts.skipped
    );
    let view_report = row![
        crate::library::modal_chrome::secondary_btn(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_VIEW_REPORT),
            msg(ImportDialogMsg::ResultTabPicked(ResultTab::Report)),
            tokens,
        ),
        caption(summary, tokens),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    col.push(view_report.boxed()).boxed()
}

/// Imported parts, approximated and dropped items, skipped items.
fn result_tiles<'a>(
    result: &ImportResult,
    facts: &RunFacts,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let tile = |count: u64, label: Localized, tone: PillTone| -> Element<'a, LibraryMessage> {
        count_tile(count, label, tone, tokens)
    };
    row![
        tile(
            facts.counts.parts,
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COUNT_IMPORTED),
            PillTone::Ok
        ),
        tile(
            result.count(Severity::Approximated) as u64,
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_SEVERITY_APPROXIMATED),
            PillTone::Info
        ),
        tile(
            result.count(Severity::Dropped) as u64,
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_SEVERITY_DROPPED),
            PillTone::Warn
        ),
        tile(
            facts.counts.skipped,
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COUNT_SKIPPED),
            PillTone::Err
        ),
    ]
    .spacing(SP.m)
    .boxed()
}

/// Location, Mounted, History and, when the manifest names any, Requires.
fn result_rows<'a>(
    result: &ImportResult,
    facts: &RunFacts,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Vec<(Localized, Element<'a, LibraryMessage>)> {
    let line =
        |words: Localized| -> Element<'a, LibraryMessage> { text(words).size(TS.body).boxed() };
    let mut rows = vec![
        (
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_LOCATION),
            mono(result.library_dir.display().to_string(), tokens),
        ),
        (
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MOUNTED),
            line(mounted_words(facts.mounted.as_ref(), i18n)),
        ),
        (
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_HISTORY),
            line(history_words(facts, i18n)),
        ),
    ];
    if !facts.required.is_empty() {
        let mut names = column![].spacing(SP.xs);
        for required in &facts.required {
            names = names.push(line(required_words(required, i18n)));
        }
        rows.push((t!(i18n, ids::LIBRARY_IMPORT_DIALOG_REQUIRES), names.boxed()));
    }
    rows
}

/// Where the library was mounted; "Mounting…" until that is settled.
fn mounted_words(mounted: Option<&Mounted>, i18n: &I18n) -> Localized {
    match mounted {
        None => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MOUNTING),
        Some(Mounted::AllProjects) => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MOUNTED_ALL),
        Some(Mounted::Project(name)) => t!(
            i18n,
            ids::LIBRARY_IMPORT_DIALOG_MOUNTED_PROJECT,
            "project" => name.as_str()
        ),
        Some(Mounted::Unrecorded) => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MOUNTED_UNRECORDED),
        Some(Mounted::Clash(other)) => t!(
            i18n,
            ids::LIBRARY_IMPORT_DIALOG_MOUNTED_CLASH,
            "other" => other.display().to_string()
        ),
    }
}

/// The commit the run made, or that an update changed nothing.
fn history_words(facts: &RunFacts, i18n: &I18n) -> Localized {
    match &facts.commit {
        Some(commit) => t!(
            i18n,
            ids::LIBRARY_IMPORT_DIALOG_HISTORY_COMMIT,
            "commit" => commit.chars().take(COMMIT_CHARS).collect::<String>()
        ),
        None => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_HISTORY_UNCHANGED),
    }
}

/// A required library: its name when open, else its id.
fn required_words(required: &RequiredLibrary, i18n: &I18n) -> Localized {
    match &required.open_as {
        Some(name) => t!(
            i18n,
            ids::LIBRARY_IMPORT_DIALOG_REQUIRES_OPEN,
            "name" => name.as_str()
        ),
        None => t!(
            i18n,
            ids::LIBRARY_IMPORT_DIALOG_REQUIRES_MISSING,
            "id" => required.id.to_string()
        ),
    }
}

/// What a severity means for an item, under its heading.
fn severity_meaning(severity: Severity, i18n: &I18n) -> Localized {
    match severity {
        Severity::Error => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MEANING_ERROR),
        Severity::Dropped => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MEANING_DROPPED),
        Severity::Approximated => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MEANING_APPROXIMATED),
        Severity::Info => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MEANING_INFO),
        Severity::Other => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MEANING_OTHER),
    }
}

/// The loss report: who wrote it, the chips and filter, and the items the
/// filter shows grouped by severity and code, at most [`MAX_ROWS`] rows.
fn loss_report<'a>(
    state: &'a ImportDialogState,
    result: &'a ImportResult,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let Some(report) = &result.report else {
        let why = result.report_problem.clone().unwrap_or_default();
        return caption(
            t!(
                i18n,
                ids::LIBRARY_IMPORT_DIALOG_REPORT_UNREADABLE,
                "path" => result.library_dir.display().to_string(),
                "why" => why
            ),
            tokens,
        );
    };
    let header = t!(
        i18n,
        ids::LIBRARY_IMPORT_DIALOG_REPORT_HEADER,
        "name" => result.library_name(),
        "converter" => report.converter.as_str(),
        "version" => report.converter_version.as_str()
    );
    let mut col: Column<Element<'a, LibraryMessage>> =
        column![caption(header, tokens), filter_row(state, tokens, i18n)].spacing(SP.m);
    if state.report_items().is_empty() {
        return col
            .push(caption(
                t!(i18n, ids::LIBRARY_IMPORT_DIALOG_LIST_EMPTY),
                tokens,
            ))
            .boxed();
    }
    let groups = state.report_groups();
    if groups.is_empty() {
        return col
            .push(caption(
                t!(i18n, ids::LIBRARY_IMPORT_DIALOG_LIST_NONE_MATCH),
                tokens,
            ))
            .boxed();
    }
    let visible: usize = groups.iter().map(|g| g.count).sum();
    let mut budget = MAX_ROWS;
    for group in groups {
        col = col.push(severity_block(group, &mut budget, tokens, i18n));
    }
    if visible > MAX_ROWS {
        col = col.push(caption(
            t!(
                i18n,
                ids::LIBRARY_IMPORT_DIALOG_LIST_MORE,
                "shown" => MAX_ROWS,
                "count" => visible
            ),
            tokens,
        ));
    }
    col.boxed()
}

/// One severity: its chip with the count and what it means, then each code
/// with its count and its items, while `budget` rows are left.
fn severity_block<'a>(
    group: SeverityGroup<'a>,
    budget: &mut usize,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let label = t!(
        i18n,
        ids::LIBRARY_IMPORT_DIALOG_SEVERITY_CHIP,
        "label" => severity_label(group.severity, i18n).into_string(),
        "count" => group.count
    );
    let mut col: Column<Element<'a, LibraryMessage>> = column![
        row![
            tag(label, severity_tone(group.severity), tokens),
            caption(severity_meaning(group.severity, i18n), tokens),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center)
    ]
    .spacing(SP.s);
    for code in group.codes {
        if *budget == 0 {
            break;
        }
        col = col.push(caption(
            t!(
                i18n,
                ids::LIBRARY_IMPORT_DIALOG_CODE_COUNT,
                "code" => code.code,
                "count" => code.items.len()
            ),
            tokens,
        ));
        for (_, item) in code.items.into_iter().take(*budget) {
            *budget -= 1;
            col = col.push(
                row![
                    container(mono(item.source.clone(), tokens)).width(Length::FillPortion(2)),
                    text(UserText::new(item.message.clone()))
                        .size(TS.body)
                        .color(ti(tokens.text_secondary))
                        .wrapping(Wrapping::None)
                        .ellipsis(Ellipsis::End)
                        .width(Length::FillPortion(3)),
                ]
                .spacing(SP.m)
                .align_y(Alignment::Center)
                .boxed(),
            );
        }
    }
    let line = ti(tokens.border);
    container(col)
        .padding(SP.m)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: HAIRLINE,
                color: line,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// What Save report… or Open report did.
fn report_note<'a>(
    note: &ReportNote,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let (tone, words) = match note {
        ReportNote::Saved(path) => (
            PillTone::Ok,
            t!(
                i18n,
                ids::LIBRARY_IMPORT_DIALOG_REPORT_SAVED,
                "path" => path.display().to_string()
            ),
        ),
        ReportNote::Failed(reason) => (PillTone::Err, reason.localized(i18n)),
    };
    notice_banner(tone, text(words).size(TS.body).boxed(), Vec::new(), tokens)
}

/// A secondary button, off when there is nothing to act on.
fn secondary_if<'a>(
    label: Localized,
    message: LibraryMessage,
    enabled: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    dialog_button(
        label,
        DialogButtonKind::Secondary,
        enabled.then_some(message),
        tokens,
    )
}

pub(super) fn footer<'a>(
    result: &'a ImportResult,
    ctx: ImportDialogContext<'a>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let has_report = result.report.is_some();
    let mut buttons = vec![
        secondary_if(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_SAVE_REPORT),
            msg(ImportDialogMsg::SaveReportPressed),
            has_report,
            tokens,
        ),
        secondary_if(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_OPEN_REPORT),
            msg(ImportDialogMsg::OpenReportPressed),
            has_report,
            tokens,
        ),
        cancel_btn(
            t!(i18n, ids::DIALOG_BUTTON_CLOSE),
            msg(ImportDialogMsg::Closed),
            tokens,
        ),
    ];
    if result.library_file.is_some() {
        buttons.push(primary_btn_if(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_OPEN_IN_BROWSER),
            msg(ImportDialogMsg::OpenInBrowserPressed),
            ctx.library_open,
            tokens,
        ));
    }
    footer_strip(Vec::new(), buttons, tokens)
}
