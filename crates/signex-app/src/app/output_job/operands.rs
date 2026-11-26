//! `run_output_job path=<job.snxout> [name=<output>]` and
//! `export_sheet_png document=<sheet> path=<folder> [dpi=<n>]` from their
//! operands (#1299): a CLI or an agent names the job or the sheet and the
//! folder, and is answered with what was written.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{Translatable, ids};
use signex_types::output_job::{MAX_PNG_DPI, MIN_PNG_DPI, OUTPUT_JOB_EXTENSION};

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// A refusal of a run of `job`, with the reason in English.
pub(super) fn job_refusal(job: &Path, reason: impl Into<String>) -> CommandOutcome {
    let reason: String = reason.into();
    CommandOutcome::Disabled {
        reason: Translatable::new(ids::NOTICE_OUTPUT_JOB_FAILED)
            .arg("job", job.display().to_string())
            .arg("reason", reason),
    }
}

/// A refusal of a PNG export, with the reason in English.
pub(super) fn png_refusal(reason: impl Into<String>) -> CommandOutcome {
    let reason: String = reason.into();
    CommandOutcome::Disabled {
        reason: Translatable::new(ids::NOTICE_SHEET_PNG_FAILED).arg("error", reason),
    }
}

fn path_operand(args: &CommandArgs) -> Option<PathBuf> {
    match args.operand("path") {
        Some(OperandValue::Path(path)) => Some(PathBuf::from(path)),
        _ => None,
    }
}

fn text_operand<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// Whether `path` names an Export Job file (`.snxout`, any case).
pub(super) fn is_job_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(OUTPUT_JOB_EXTENSION))
}

/// `dpi` as written: whole dots per inch within the range a PNG takes.
pub(super) fn parse_dpi(text: Option<&str>) -> Result<u32, String> {
    let Some(text) = text else {
        return Ok(signex_types::output_job::DEFAULT_PNG_DPI);
    };
    text.trim()
        .parse::<u32>()
        .ok()
        .filter(|dpi| (MIN_PNG_DPI..=MAX_PNG_DPI).contains(dpi))
        .ok_or_else(|| {
            format!("`dpi` is a whole number from {MIN_PNG_DPI} to {MAX_PNG_DPI}, not `{text}`")
        })
}

/// `run_output_job path=<job> [name=<output>]`: checked now, answered once
/// the job file is read and its files are written
/// ([`CommandOutcome::Deferred`]).
pub(crate) fn run_output_job(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(job) = path_operand(args) else {
        return (
            job_refusal(Path::new(""), "needs the operand `path`"),
            Task::none(),
        );
    };
    if let Err(reason) = app.check_output_job_start(&job) {
        return (job_refusal(&job, reason), Task::none());
    }
    let command = match AppCommandId::new("run_output_job") {
        Ok(command) => command,
        Err(reason) => return (job_refusal(&job, reason), Task::none()),
    };
    let only = text_operand(args, "name").map(str::to_string);
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let task = super::run::read_job(job, only, Some(waiting));
    (CommandOutcome::Deferred { ticket }, task)
}

/// `export_sheet_png document=<sheet> path=<folder> [dpi=<n>]`: the pages
/// of the sheet's project (or the open loose sheets) as PNG files in the
/// folder, drawn and written off the UI thread, then answered with the
/// files ([`CommandOutcome::Deferred`]).
pub(crate) fn export_sheet_png(app: &mut Signex, args: &CommandArgs) -> Ran {
    let refused = |reason: String| (png_refusal(reason), Task::none());
    let source = match app.named_png_sheet(args) {
        Ok(source) => source,
        Err(reason) => return refused(reason),
    };
    let Some(folder) = path_operand(args) else {
        return refused("needs the operand `path`".to_string());
    };
    let dpi = match parse_dpi(text_operand(args, "dpi")) {
        Ok(dpi) => dpi,
        Err(reason) => return refused(reason),
    };
    let command = match AppCommandId::new("export_sheet_png") {
        Ok(command) => command,
        Err(reason) => return refused(reason),
    };
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    match app.start_sheet_png(&source, folder, dpi, Some(waiting)) {
        Ok(task) => (CommandOutcome::Deferred { ticket }, task),
        Err(reason) => refused(reason),
    }
}

impl Signex {
    /// The open schematic `document` names.
    fn named_png_sheet(&self, args: &CommandArgs) -> Result<PathBuf, String> {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            return Err("needs the operand `document`".to_string());
        };
        let entry = self.resolve_document_target(named)?;
        if entry.kind != DocumentKind::Schematic {
            return Err(format!(
                "{} is a {}, not a schematic",
                entry.name,
                entry.kind.as_str()
            ));
        }
        let path = entry
            .path
            .ok_or_else(|| format!("{} has no file", entry.name))?;
        if !self.document_state.engines.contains_key(&path) {
            return Err(format!("{} is not open: open it first", entry.name));
        }
        Ok(path)
    }

    /// Why a run of `job` cannot start now: not an Export Job file, not in
    /// an open project's folder with an open schematic, or in a project
    /// whose folder is being renamed.
    pub(super) fn check_output_job_start(&self, job: &Path) -> Result<(), String> {
        if !is_job_file(job) {
            return Err(format!(
                "{} is not an Export Job (.snxout) file",
                job.display()
            ));
        }
        self.output_job_scope(job)?;
        self.refuse_while_project_moves(job, "run the Export Job")
            .map_err(|e| e.to_string())
    }
}
