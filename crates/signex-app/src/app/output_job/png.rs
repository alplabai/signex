//! `export_sheet_png`: the pages of a schematic's project as PNG files in a
//! folder, at the resolution asked (300 dpi from a key or the palette).

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{Translatable, ids};
use signex_types::output_job::DEFAULT_PNG_DPI;

use super::write::{JobReport, build_and_write};
use super::{JobWritten, OutputJobMsg};
use crate::app::agent::WaitingCommand;
use crate::app::command::CommandOutcome;
use crate::app::{Message, Signex};

impl Signex {
    /// `export_sheet_png` without operands: the folder picker for the pages
    /// of the schematic in view.
    pub(super) fn pick_sheet_png_folder(&mut self) -> Task<Message> {
        let Some(source) = self
            .document_state
            .active_path
            .clone()
            .filter(|path| self.document_state.engines.contains_key(path))
        else {
            return Task::none();
        };
        let i18n = &self.ui_state.i18n;
        let mut dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::FILE_DIALOG_EXPORT_SHEET_PNG);
        if let Some(folder) = source.parent() {
            dialog = dialog.set_directory(folder);
        }
        Task::perform(dialog.pick_folder(), move |folder| {
            Message::from(OutputJobMsg::PngFolderPicked { source, folder })
        })
    }

    /// The person picked `folder`: the drafts of the sheet's project are
    /// committed, then the pages are drawn at the default resolution.
    pub(super) fn start_picked_sheet_png(
        &mut self,
        source: &Path,
        folder: PathBuf,
    ) -> Task<Message> {
        self.commit_drafts_in_scope_of(Some(source));
        match self.start_sheet_png(source, folder, DEFAULT_PNG_DPI, None) {
            Ok(task) => task,
            Err(reason) => {
                self.refuse_sheet_png(&reason);
                Task::none()
            }
        }
    }

    /// Draw the pages of `source`'s scope at `dpi` and write them into
    /// `folder` off the UI thread; `waiting` is the call answered then.
    pub(super) fn start_sheet_png(
        &mut self,
        source: &Path,
        folder: PathBuf,
        dpi: u32,
        waiting: Option<WaitingCommand>,
    ) -> Result<Task<Message>, String> {
        let stem = self
            .document_state
            .document_project(source)
            .map(|project| project.path.as_path())
            .unwrap_or(source)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "sheet".to_string());
        let (work, variant) = self.sheet_png_work(source, &format!("{stem}.png"), dpi)?;
        self.refuse_while_project_moves(&folder, "export PNG pages to")
            .map_err(|e| e.to_string())?;
        self.begin_project_write(&folder);
        let mut report = JobReport {
            job: String::new(),
            folder: folder.display().to_string(),
            variant,
            outputs: Vec::new(),
            skipped: Vec::new(),
            warnings: Vec::new(),
        };
        let writing = folder.clone();
        Ok(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || build_and_write(&writing, None, vec![work]))
                    .await
                    .map_err(|e| format!("the export did not finish: {e}"))
                    .and_then(|written| written)
            },
            move |written| {
                let result = written.map(|(outputs, said)| {
                    report.outputs = outputs;
                    report.warnings = said;
                    report
                });
                Message::from(OutputJobMsg::PngWritten(Box::new(JobWritten {
                    target: folder,
                    waiting,
                    result,
                })))
            },
        ))
    }

    pub(super) fn handle_sheet_png_written(&mut self, written: JobWritten) -> Task<Message> {
        let JobWritten {
            target,
            waiting,
            result,
        } = written;
        self.end_project_write(&target);
        match &result {
            Ok(report) => super::run::log_report("PNG export", report),
            Err(reason) if waiting.is_none() => self.refuse_sheet_png(reason),
            Err(reason) => {
                crate::diagnostics::log_warning(format!("PNG export failed: {reason}"));
            }
        }
        let Some(waiting) = waiting else {
            return Task::none();
        };
        let outcome = match result {
            Ok(report) => CommandOutcome::Answered(
                serde_json::to_value(&report).unwrap_or(serde_json::Value::Null),
            ),
            Err(reason) => super::operands::png_refusal(reason),
        };
        self.finish_waiting_command(waiting, &target, move |_| outcome)
    }

    fn refuse_sheet_png(&mut self, reason: &str) {
        self.refuse_export(
            Translatable::new(ids::NOTICE_SHEET_PNG_FAILED).arg("error", reason.to_string()),
        );
    }
}
