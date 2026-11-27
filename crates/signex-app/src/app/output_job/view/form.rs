//! The picked output's form (mockup row 114, middle): its kind, group and
//! source, the Enabled switch, the file (or the stem a board output's files
//! start with), the settings its kind takes (PNG resolution, the ODB++
//! folder form) and what the writer fixes, and Remove.

use iced::widget::{Column, column, row};
use iced::{Alignment, Element};
use signex_i18n::{MessageId, ids, t};
use signex_types::output_job::{DEFAULT_PNG_DPI, JobOutput, OutputKind};

use super::outputs::{group_label, kind_label};
use super::{Frame, SP, kit};
use crate::app::Message;
use crate::app::output_job::tab::JobTabMsg;

/// What the writer of `kind` fixes, as the form says it; `None` when the
/// kind's own fields say it all.
fn fixed_by_writer(kind: OutputKind) -> Option<MessageId> {
    match kind {
        OutputKind::Gerber => Some(ids::OUTPUT_JOB_FORM_GERBER_FIXED),
        OutputKind::NcDrill => Some(ids::OUTPUT_JOB_FORM_DRILL_FIXED),
        OutputKind::Bom => Some(ids::OUTPUT_JOB_FORM_BOM_FORMAT),
        OutputKind::SchematicPdf | OutputKind::SchematicPng => {
            Some(ids::OUTPUT_JOB_FORM_PAGES_FIXED)
        }
        OutputKind::Step => Some(ids::OUTPUT_JOB_FORM_STEP_FIXED),
        _ => None,
    }
}

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let look = frame.look;
    let picked = frame
        .ui
        .selected
        .as_deref()
        .and_then(|name| frame.draft.output(name));
    let Some(output) = picked else {
        return column![
            kit::line(look, look.t(ids::OUTPUT_JOB_FORM_NOTHING)),
            kit::quiet(look, look.t(ids::OUTPUT_JOB_FORM_NOTHING_BODY)),
        ]
        .spacing(SP.s)
        .into();
    };
    let Some(kind) = output.known_kind() else {
        return kit::quiet(
            look,
            t!(look.i18n, ids::OUTPUT_JOB_UNKNOWN_KIND, "name" => output.name.as_str(), "kind" => output.kind.as_str()),
        );
    };
    let job = frame.job_path();
    let name = output.name.clone();
    let source = match kind.is_board() {
        true => ids::OUTPUT_JOB_SOURCE_BOARD,
        false => ids::OUTPUT_JOB_SOURCE_SHEETS,
    };
    let mut form: Column<Element<'a, Message>> = column![
        row![
            kit::user(look, &output.name),
            kit::quiet(look, look.t(kind_label(kind))),
            kit::quiet(look, look.t(group_label(kind.group()))),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center),
        kit::quiet(look, look.t(source)),
        kit::tick(
            output.enabled,
            look.t(ids::OUTPUT_JOB_FORM_ENABLED),
            Some(Message::from(JobTabMsg::OutputEnabledToggled {
                job: job.clone(),
                name: name.clone(),
            })),
        ),
    ]
    .spacing(SP.m);
    form = form.push(file_field(frame, output, kind));
    if kind == OutputKind::SchematicPng {
        let dpi = output.dpi.unwrap_or(DEFAULT_PNG_DPI).to_string();
        let editing = (job.clone(), name.clone());
        form = form
            .push(kit::head(look, ids::OUTPUT_JOB_FORM_DPI))
            .push(kit::field(
                look.t(ids::OUTPUT_JOB_FORM_DPI),
                &dpi,
                move |text| {
                    Message::from(JobTabMsg::OutputDpiEdited {
                        job: editing.0.clone(),
                        name: editing.1.clone(),
                        text,
                    })
                },
            ))
            .push(kit::quiet(look, look.t(ids::OUTPUT_JOB_FORM_DPI_NOTE)));
    }
    if kind == OutputKind::Gerber {
        form = form.push(super::gerber_form::fields(frame, job.clone(), output));
    }
    if kind == OutputKind::Odb {
        form = form.push(kit::tick(
            output.odb_folder.unwrap_or(false),
            look.t(ids::OUTPUT_JOB_FORM_ODB_FOLDER),
            Some(Message::from(JobTabMsg::OdbFolderToggled {
                job: job.clone(),
                name: name.clone(),
            })),
        ));
    }
    if let Some(fixed) = fixed_by_writer(kind) {
        form = form.push(kit::quiet(look, look.t(fixed)));
    }
    form.push(kit::ghost(
        look.tokens,
        look.t(ids::OUTPUT_JOB_FORM_REMOVE),
        Some(Message::from(JobTabMsg::OutputRemoved { job, name })),
    ))
    .into()
}

/// The file the output writes, or the stem a board output's files start
/// with; empty takes the default.
fn file_field<'a>(frame: &Frame<'a>, output: &JobOutput, kind: OutputKind) -> Element<'a, Message> {
    let look = frame.look;
    let (head, note) = match kind.file_is_stem() {
        true => (ids::OUTPUT_JOB_FORM_STEM, ids::OUTPUT_JOB_FORM_STEM_NOTE),
        false => (ids::OUTPUT_JOB_FORM_FILE, ids::OUTPUT_JOB_FORM_FILE_NOTE),
    };
    let value = output.file.clone().unwrap_or_default();
    let job = frame.job_path();
    let name = output.name.clone();
    column![
        kit::head(look, head),
        kit::field(
            look.t(ids::OUTPUT_JOB_FORM_DEFAULT_FILE),
            &value,
            move |text| {
                Message::from(JobTabMsg::OutputFileEdited {
                    job: job.clone(),
                    name: name.clone(),
                    text,
                })
            }
        ),
        kit::quiet(look, look.t(note)),
    ]
    .spacing(SP.xs)
    .into()
}
