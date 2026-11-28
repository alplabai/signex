//! The run summary and the history (mockup rows 114 and 193b): outputs
//! written, files, warnings, errors and time; where the files went (and the
//! zip); Open folder, View in Gerber Viewer, Copy path and Show the flow;
//! the Outputs card with a View link on the rows the viewer reads; and the
//! history kept in the `.snxout`, a row opening its summary.

use iced::Widget as _;
use iced::widget::{Column, column, row};
use iced::{Alignment, Element, Length};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_types::output_job::{RunRecord, RunResult, RunTrigger};

use super::{Frame, SP, kit, page};
use crate::app::Message;
use crate::app::output_job::tab::{JobTabMsg, TabPage};

/// The catalog name of what started a run.
pub(super) fn trigger_label(trigger: RunTrigger) -> MessageId {
    match trigger {
        RunTrigger::Manual => ids::OUTPUT_JOB_TRIGGER_MANUAL,
        RunTrigger::Selected => ids::OUTPUT_JOB_TRIGGER_SELECTED,
        RunTrigger::OnSave => ids::OUTPUT_JOB_TRIGGER_SAVED,
        RunTrigger::KeepUpToDate => ids::OUTPUT_JOB_TRIGGER_KEPT,
        RunTrigger::BeforeClose => ids::OUTPUT_JOB_TRIGGER_CLOSED,
        RunTrigger::Command => ids::OUTPUT_JOB_TRIGGER_COMMAND,
    }
}

/// What the summary shows: a run of this session, or a history row.
struct Shown {
    started: String,
    profile: String,
    variant: String,
    complete: bool,
    /// Each output with its file count, for a run of this session.
    outputs: Vec<(String, usize)>,
    output_count: usize,
    files: usize,
    warnings: Vec<String>,
    errors: usize,
    seconds: u32,
    folder: String,
    zip: Option<String>,
    viewable: usize,
}

fn from_record(record: &RunRecord) -> Shown {
    Shown {
        started: record.started.clone(),
        profile: record.profile.clone().unwrap_or_default(),
        variant: record.variant.clone(),
        complete: record.result == RunResult::Complete,
        outputs: Vec::new(),
        output_count: record.outputs as usize,
        files: record.files as usize,
        warnings: record.warning_lines.clone(),
        errors: record.errors as usize,
        seconds: record.seconds,
        folder: record.folder.clone(),
        zip: record.zip.clone(),
        viewable: record.viewable.len(),
    }
}

fn shown(frame: &Frame<'_>) -> Option<Shown> {
    if let Some(index) = frame.ui.history_pick {
        return frame.draft.history.get(index).map(from_record);
    }
    match frame.run.filter(|run| !run.is_running()) {
        Some(run) => Some(Shown {
            started: run.started.clone(),
            profile: run.profile.clone().unwrap_or_default(),
            variant: run.variant.clone(),
            complete: run.stopped.is_none(),
            outputs: run
                .report
                .outputs
                .iter()
                .map(|o| (o.name.clone(), o.files.len()))
                .collect(),
            output_count: run.report.outputs.len(),
            files: run.report.file_count(),
            warnings: run.report.warnings.clone(),
            errors: run.checks.as_ref().map_or(0, |c| c.errors),
            seconds: run.seconds,
            folder: run.report.folder.clone(),
            zip: run.report.zip.clone(),
            viewable: run.viewable.len(),
        }),
        None => frame.draft.history.first().map(from_record),
    }
}

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let look = frame.look;
    let job = frame.job_path();
    let back: Element<'a, Message> = row![
        kit::ghost(
            look.tokens,
            look.t(ids::OUTPUT_JOB_SHOW_FLOW),
            Some(Message::from(JobTabMsg::PagePicked {
                job: job.clone(),
                page: TabPage::Flow,
            }))
        ),
        kit::ghost(
            look.tokens,
            look.t(ids::OUTPUT_JOB_BACK_TO_SETTINGS),
            Some(Message::from(JobTabMsg::PagePicked {
                job: job.clone(),
                page: TabPage::Settings,
            }))
        ),
    ]
    .spacing(SP.s)
    .boxed();
    let mut parts = vec![back];
    match shown(frame) {
        Some(shown) => {
            parts.push(summary_card(frame, &shown));
            if !shown.outputs.is_empty() {
                parts.push(outputs_card(frame, &shown));
            }
        }
        None => parts.push(kit::line(look, look.t(ids::OUTPUT_JOB_NEVER_RUN))),
    }
    parts.push(history(frame));
    page(parts)
}

fn summary_card<'a>(frame: &Frame<'a>, shown: &Shown) -> Element<'a, Message> {
    let look = frame.look;
    let job = frame.job_path();
    let head = match shown.complete {
        true => ids::OUTPUT_JOB_SUMMARY_COMPLETE,
        false => ids::OUTPUT_JOB_SUMMARY_STOPPED,
    };
    let mut col: Column<Element<'a, Message>> = column![
        kit::line(
            look,
            t!(
                look.i18n,
                head,
                "profile" => shown.profile.clone(),
                "variant" => shown.variant.clone(),
                "started" => shown.started.clone()
            )
        ),
        kit::quiet(
            look,
            t!(
                look.i18n,
                ids::OUTPUT_JOB_SUMMARY_COUNTS,
                "outputs" => shown.output_count,
                "files" => shown.files,
                "warnings" => shown.warnings.len(),
                "errors" => shown.errors,
                "seconds" => shown.seconds
            )
        ),
    ]
    .spacing(SP.s);
    for warning in shown.warnings.iter().take(10) {
        col = col.push(kit::quiet(look, UserText::new(warning.clone())));
    }
    if shown.complete {
        col = col
            .push(kit::head(look, ids::OUTPUT_JOB_WRITTEN_TO))
            .push(kit::user(look, &shown.folder));
        if let Some(zip) = &shown.zip {
            col = col.push(kit::quiet(
                look,
                t!(look.i18n, ids::OUTPUT_JOB_PLUS_ZIP, "zip" => zip.as_str()),
            ));
        }
        let view = (shown.viewable > 0)
            .then(|| Message::from(JobTabMsg::ViewInGerberPressed { job: job.clone() }));
        col = col
            .push(
                row![
                    kit::ghost(
                        look.tokens,
                        look.t(ids::OUTPUT_JOB_OPEN_FOLDER),
                        Some(Message::from(JobTabMsg::OpenFolderPressed {
                            job: job.clone()
                        }))
                    ),
                    kit::primary(look.tokens, look.t(ids::OUTPUT_JOB_VIEW_IN_GERBER), view),
                    kit::ghost(
                        look.tokens,
                        look.t(ids::OUTPUT_JOB_COPY_PATH),
                        Some(Message::from(JobTabMsg::CopyPathPressed { job }))
                    ),
                ]
                .spacing(SP.s)
                .boxed(),
            )
            .push(kit::quiet(
                look,
                look.t(ids::OUTPUT_JOB_VIEW_IN_GERBER_NOTE),
            ));
    }
    kit::card(look.tokens, col.boxed())
}

/// Every output with its file count; a View link on those with files the
/// Gerber Viewer reads.
fn outputs_card<'a>(frame: &Frame<'a>, shown: &Shown) -> Element<'a, Message> {
    let look = frame.look;
    let readable: Vec<&str> = frame
        .run
        .map(|run| {
            run.nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.kind,
                        signex_types::output_job::OutputKind::Gerber
                            | signex_types::output_job::OutputKind::NcDrill
                    )
                })
                .map(|node| node.name.as_str())
                .collect()
        })
        .unwrap_or_default();
    let mut col: Column<Element<'a, Message>> =
        column![kit::head(look, ids::OUTPUT_JOB_OUTPUTS)].spacing(SP.xs);
    for (name, files) in &shown.outputs {
        let mut line = row![
            iced::widget::container(kit::user(look, name)).width(Length::Fill),
            kit::quiet(
                look,
                t!(look.i18n, ids::OUTPUT_JOB_FILE_COUNT, "count" => *files)
            ),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center);
        if readable.contains(&name.as_str()) {
            line = line.push(kit::ghost(
                look.tokens,
                look.t(ids::OUTPUT_JOB_VIEW),
                Some(Message::from(JobTabMsg::ViewInGerberPressed {
                    job: frame.job_path(),
                })),
            ));
        }
        col = col.push_boxed(line);
    }
    kit::card(look.tokens, col.boxed())
}

/// The history kept in the job file, newest first; a row opens its summary.
fn history<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let look = frame.look;
    let mut col: Column<Element<'a, Message>> =
        column![kit::head(look, ids::OUTPUT_JOB_HISTORY)].spacing(SP.xs);
    if frame.draft.history.is_empty() {
        col = col.push(kit::quiet(look, look.t(ids::OUTPUT_JOB_NEVER_RUN)));
    }
    for (index, record) in frame.draft.history.iter().enumerate() {
        let trigger = look.t(trigger_label(record.trigger)).into_string();
        let result = match record.result {
            RunResult::Complete => t!(
                look.i18n,
                ids::OUTPUT_JOB_HISTORY_COMPLETE,
                "outputs" => record.outputs,
                "warnings" => record.warnings
            ),
            RunResult::Stopped => t!(
                look.i18n,
                ids::OUTPUT_JOB_HISTORY_STOPPED,
                "at" => record.stopped_at.clone().unwrap_or_default()
            ),
        };
        let words = t!(
            look.i18n,
            ids::OUTPUT_JOB_HISTORY_ROW,
            "started" => record.started.as_str(),
            "profile" => record.profile.clone().unwrap_or_default(),
            "variant" => record.variant.as_str(),
            "trigger" => trigger,
            "result" => result.into_string(),
            "seconds" => record.seconds
        );
        let msg = Message::from(JobTabMsg::HistoryPicked {
            job: frame.job_path(),
            index,
        });
        col = col.push(match frame.ui.history_pick == Some(index) {
            true => kit::primary(look.tokens, words, Some(msg)),
            false => kit::ghost(look.tokens, words, Some(msg)),
        });
    }
    col = col.push(kit::quiet(look, look.t(ids::OUTPUT_JOB_HISTORY_NOTE)));
    kit::card(look.tokens, col.boxed())
}
