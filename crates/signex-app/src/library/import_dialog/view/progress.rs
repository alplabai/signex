//! Step 3, import (mockup row 211): the phase with its count and percent,
//! the bar, the current item, the phases so far, the silence rule, and
//! Cancel; or how the import ended when it wrote no library.

use iced::widget::{Column, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Font, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, RenderTranslatable, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::{ImportDialogContext, SP, TS, caption, mono, tag};
use crate::library::import_dialog::progress::PhaseLine;
use crate::library::import_dialog::{
    FolderAfter, ImportDialogMsg, ImportDialogState, ImportEnd, Step, msg,
};
use crate::library::messages::LibraryMessage;
use crate::library::modal_chrome::{cancel_btn, footer_strip, notice_banner, secondary_btn};
use crate::library_import::ProgressEvent;
use crate::library_import::run::SILENCE_LIMIT;
use crate::styles::{DialogButtonKind, HAIRLINE, PillTone, Radii, dialog_button, ti, tokens};

const BAR_H: f32 = 6.0;
/// The bar is drawn as two fill portions of this many parts.
const BAR_PARTS: u64 = 1000;

/// The words for a converter phase; a phase a newer converter adds shows
/// as it names itself.
pub(super) fn phase_label(phase: &str, i18n: &I18n) -> signex_widgets::Label<'static> {
    match phase {
        "read" => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PHASE_READ).into(),
        "convert" => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PHASE_CONVERT).into(),
        "write" => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PHASE_WRITE).into(),
        "models" => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PHASE_MODELS).into(),
        "" => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PHASE_STARTING).into(),
        other => UserText::new(other.to_string()).into(),
    }
}

fn done_of(line: &PhaseLine, i18n: &I18n) -> Option<Localized> {
    let total = line.total?;
    Some(t!(
        i18n,
        ids::LIBRARY_IMPORT_DIALOG_DONE_OF,
        "done" => line.done,
        "total" => total
    ))
}

fn bold() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..signex_widgets::ui_font::regular()
    }
}

/// The current phase, its count and percent, and the bar (also shown
/// under step 2 while the dry run reads the source).
pub(super) fn current_line<'a>(
    state: &'a ImportDialogState,
    ctx: ImportDialogContext<'a>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let current = state.current_phase();
    // Import of a staged preview runs no converter: it moves the folder.
    let moving = current.is_none() && state.step == Step::Import && state.staged.is_some();
    let title: signex_widgets::Label<'static> = if moving {
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MOVING).into()
    } else {
        phase_label(current.map_or("", |l| l.phase.as_str()), i18n)
    };
    let mut head = row![text(title).size(TS.body).font(bold())]
        .spacing(SP.m)
        .align_y(Alignment::Center);
    if let Some(count) = current.and_then(|l| done_of(l, i18n)) {
        head = head.push(caption(count, tokens));
    }
    head = head.push(Space::new().width(Length::Fill).boxed());
    let percent = current.and_then(PhaseLine::percent);
    if let Some(percent) = percent {
        head = head.push(caption(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PERCENT, "percent" => percent),
            tokens,
        ));
    }
    let mut col = column![head, bar(percent.unwrap_or(0), tokens)].spacing(SP.s);
    let item = ctx.running.and_then(|run| match &run.progress {
        Some(ProgressEvent::Progress {
            item: Some(item), ..
        }) => Some(item.clone()),
        _ => None,
    });
    if let Some(item) = item {
        col = col.push(
            row![
                caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_NOW), tokens),
                mono(item, tokens)
            ]
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    col.boxed()
}

/// The bar: an accent fill over the sunken track, `percent` of its width.
fn bar<'a>(percent: u64, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let r = tokens::resolve(tokens);
    let (track, fill) = (r.sunken, ti(tokens.accent));
    let done = (percent.min(100) * BAR_PARTS / 100) as u16;
    let rest = BAR_PARTS as u16 - done;
    let piece = |portion: u16, color: Option<iced::Color>| {
        container(Space::new())
            .width(Length::FillPortion(portion.max(1)))
            .height(Length::Fixed(BAR_H))
            .style(move |_: &Theme| container::Style {
                background: color.map(Background::Color),
                ..container::Style::default()
            })
    };
    let mut line = row![];
    if done > 0 {
        line = line.push(piece(done, Some(fill)));
    }
    if rest > 0 {
        line = line.push(piece(rest, None));
    }
    container(line)
        .width(Length::Fill)
        .height(Length::Fixed(BAR_H))
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(track)),
            border: Border {
                radius: (BAR_H / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

pub(super) fn body<'a>(
    state: &'a ImportDialogState,
    ctx: ImportDialogContext<'a>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    if let Some(end) = &state.import_end {
        return ended(end, state, tokens, i18n);
    }
    let mut col: Column<Element<'a, LibraryMessage>> =
        column![current_line(state, ctx, tokens, i18n)].spacing(SP.l);
    if !state.phases.is_empty() {
        col = col.push(phase_list(state, tokens, i18n));
    }
    let silence = if state.is_update() {
        ids::LIBRARY_IMPORT_DIALOG_SILENCE_UPDATE
    } else {
        ids::LIBRARY_IMPORT_DIALOG_SILENCE
    };
    let words = t!(
        i18n,
        silence,
        "seconds" => SILENCE_LIMIT.as_secs(),
        "path" => state.location.clone()
    );
    let words = text(words).size(TS.body).boxed();
    col = col.push(notice_banner(PillTone::Info, words, Vec::new(), tokens));
    col.boxed()
}

/// Every phase so far: the ones before the current are done.
fn phase_list<'a>(
    state: &'a ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let last = state.phases.len().saturating_sub(1);
    let rows = state.phases.iter().enumerate().map(|(i, line)| {
        let tone = if i < last {
            PillTone::Ok
        } else {
            PillTone::Info
        };
        let mut entry = row![
            tag(phase_label(&line.phase, i18n), tone, tokens),
            Space::new().width(Length::Fill)
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center);
        if let Some(count) = done_of(line, i18n) {
            entry = entry.push(caption(count, tokens));
        }
        entry.boxed()
    });
    let line = ti(tokens.border);
    container(column(rows).spacing(SP.s))
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

/// A cancelled, failed or refused import: what happened and what became
/// of its folder.
fn ended<'a>(
    end: &ImportEnd,
    state: &ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let (tone, first, second) = end_words(end, state.location.clone(), i18n);
    let words = column![text(first).size(TS.body), text(second).size(TS.body)]
        .spacing(SP.xs)
        .boxed();
    let banner = notice_banner(tone, words, Vec::new(), tokens);
    let stderr = match end {
        ImportEnd::Failed { stderr_tail, .. } => super::stderr_box(stderr_tail, tokens, i18n),
        ImportEnd::Cancelled { .. } | ImportEnd::Refused { .. } => None,
    };
    match stderr {
        Some(stderr) => column![banner, stderr].spacing(SP.m).boxed(),
        None => banner,
    }
}

/// The tone and the two lines of how the import ended, for the folder
/// `path`.
pub(in crate::library::import_dialog) fn end_words(
    end: &ImportEnd,
    path: String,
    i18n: &I18n,
) -> (PillTone, Localized, Localized) {
    let failed = |reason: &signex_i18n::Translatable| {
        t!(
            i18n,
            ids::LIBRARY_IMPORT_DIALOG_FAILED,
            "error" => reason.localized(i18n).into_string()
        )
    };
    match end {
        ImportEnd::Cancelled { folder } => (
            PillTone::Warn,
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_CANCELLED),
            folder_after(folder, path, i18n),
        ),
        ImportEnd::Failed { reason, folder, .. } => (
            PillTone::Err,
            failed(reason),
            folder_after(folder, path, i18n),
        ),
        ImportEnd::Refused { reason } => (
            PillTone::Err,
            failed(reason),
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_UNCHANGED, "path" => path),
        ),
    }
}

/// What became of the folder the run wrote into.
fn folder_after(folder: &FolderAfter, path: String, i18n: &I18n) -> Localized {
    match folder {
        FolderAfter::Removed => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_REMOVED, "path" => path),
        FolderAfter::Kept => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_KEPT, "path" => path),
        FolderAfter::Restored => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_RESTORED, "path" => path),
        FolderAfter::NotRestored(why) => t!(
            i18n,
            ids::LIBRARY_IMPORT_DIALOG_NOT_RESTORED,
            "path" => path,
            "reason" => why.as_str()
        ),
    }
}

pub(super) fn footer<'a>(
    state: &'a ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    if state.import_end.is_some() {
        return footer_strip(
            Vec::new(),
            vec![
                secondary_btn(
                    t!(i18n, ids::DIALOG_BUTTON_BACK),
                    msg(ImportDialogMsg::BackPressed),
                    tokens,
                ),
                cancel_btn(
                    t!(i18n, ids::DIALOG_BUTTON_CLOSE),
                    msg(ImportDialogMsg::Closed),
                    tokens,
                ),
            ],
            tokens,
        );
    }
    let cancel = if state.stop_requested {
        dialog_button(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_CANCELLING),
            DialogButtonKind::Cancel,
            None,
            tokens,
        )
    } else if state.cancel_pending {
        // Waiting for the current item; a second press stops the run now.
        cancel_btn(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_STOP_NOW),
            msg(ImportDialogMsg::CancelPressed),
            tokens,
        )
    } else {
        cancel_btn(
            t!(i18n, ids::DIALOG_BUTTON_CANCEL),
            msg(ImportDialogMsg::CancelPressed),
            tokens,
        )
    };
    let rule = if state.is_update() {
        ids::LIBRARY_IMPORT_DIALOG_CANCEL_RULE_UPDATE
    } else {
        ids::LIBRARY_IMPORT_DIALOG_CANCEL_RULE
    };
    footer_strip(vec![caption(t!(i18n, rule), tokens)], vec![cancel], tokens)
}
