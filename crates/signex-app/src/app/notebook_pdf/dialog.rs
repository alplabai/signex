//! Export Design Notebook PDF: open on the project in view, read and check
//! the notebook, change the settings, then make and write the PDF and show
//! the done card. The command's export ends here too.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_i18n::{Translatable, ids, t};
use signex_notebook::DEFAULT_NOTEBOOK_FILE;
use signex_notebook::export::{ExportOptions, ExportSummary, StaleRule};

use super::{
    BuildRequest, BuiltPdf, ExportDone, NotebookPdfDialog, NotebookPdfMsg, PageChoice, Stage, jobs,
};
use crate::app::command::CommandOutcome;
use crate::app::contracts::{ExportMsg, ExportWriteOutcome};
use crate::app::{Message, Signex};

impl Signex {
    /// Open the dialog on the notebook of the project in view and read it.
    /// `current_page` is the page the notebook tab shows.
    pub(super) fn open_notebook_pdf(&mut self, current_page: Option<usize>) -> Task<Message> {
        let Some(project) = self.document_state.active_document_project() else {
            let line = t!(self.ui_state.i18n, ids::NOTEBOOK_PDF_NO_PROJECT);
            crate::diagnostics::log_info(line.into_string());
            return Task::none();
        };
        let id = project.id;
        let project_dir = project.dir().to_path_buf();
        let project_name = project.data.name.clone();
        let variants = jobs::variants_of(project);
        let design = self.notebook_design_inputs(id);
        let state = &mut self.ui_state.notebook_pdf;
        state.generation += 1;
        let generation = state.generation;
        let notebook = DEFAULT_NOTEBOOK_FILE.to_string();
        state.dialog = Some(NotebookPdfDialog {
            project: id,
            project_dir: project_dir.clone(),
            project_name,
            notebook: notebook.clone(),
            variants,
            current_page: current_page.unwrap_or(1).max(1),
            choice: PageChoice::All,
            range_text: String::new(),
            options: ExportOptions::default(),
            path_text: String::new(),
            path_edited: false,
            stage: Stage::Reading,
            generation,
        });
        jobs::inspect_task(generation, project_dir, notebook, design)
    }

    /// Keep the inspection of this opening; the Save to field takes the
    /// default file name unless it was edited.
    pub(super) fn apply_notebook_inspection(
        &mut self,
        generation: u64,
        result: Arc<Result<signex_notebook::export::NotebookInspection, String>>,
    ) {
        let Some(dialog) = self.ui_state.notebook_pdf.dialog.as_mut() else {
            return;
        };
        if dialog.generation != generation || !matches!(dialog.stage, Stage::Reading) {
            return;
        }
        let result = Arc::try_unwrap(result).unwrap_or_else(|shared| (*shared).clone());
        match result {
            Ok(inspection) => {
                if !dialog.path_edited {
                    let path = jobs::default_path(
                        &dialog.project_name,
                        &dialog.notebook,
                        &inspection.title,
                    );
                    dialog.path_text = path.display().to_string();
                }
                dialog.stage = Stage::Ready(Box::new(inspection));
            }
            Err(why) => dialog.stage = Stage::Unreadable(why),
        }
    }

    /// Change the open dialog.
    pub(super) fn edit_notebook_pdf(&mut self, change: impl FnOnce(&mut NotebookPdfDialog)) {
        if let Some(dialog) = self.ui_state.notebook_pdf.dialog.as_mut() {
            change(dialog);
        }
    }

    /// Browse…: the save dialog, starting at the Save to file.
    pub(super) fn browse_notebook_pdf_path(&mut self) -> Task<Message> {
        let Some(dialog) = self.ui_state.notebook_pdf.dialog.as_ref() else {
            return Task::none();
        };
        let generation = dialog.generation;
        let start = jobs::resolve_path(&dialog.project_dir, &dialog.path_text);
        let file_name = start
            .as_deref()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let folder = start
            .as_deref()
            .and_then(Path::parent)
            .map_or_else(|| dialog.project_dir.clone(), Path::to_path_buf);
        let picker = self.ui_state.io.picker.clone();
        let i18n = self.ui_state.i18n.clone();
        Task::perform(
            async move {
                picker
                    .dialog()
                    .title_in(&i18n, ids::FILE_DIALOG_EXPORT_PDF)
                    .filter_in(&i18n, ids::FILE_FILTER_PDF, &["pdf"])
                    .set_directory(folder)
                    .set_file_name(file_name)
                    .save_file()
                    .await
            },
            move |path| match path {
                Some(path) => Message::from(NotebookPdfMsg::PathChosen { generation, path }),
                None => Message::noop(),
            },
        )
    }

    /// Review…: each stale reference as a line in Messages.
    pub(super) fn review_notebook_stale(&self) {
        let Some(inspection) = self
            .ui_state
            .notebook_pdf
            .dialog
            .as_ref()
            .and_then(NotebookPdfDialog::inspection)
        else {
            return;
        };
        for reference in &inspection.stale.stale {
            let line = t!(
                self.ui_state.i18n,
                ids::NOTEBOOK_PDF_REVIEW_LINE,
                "name" => reference.name()
            );
            crate::diagnostics::log_warning(line.into_string());
        }
    }

    /// Export PDF (or Export anyway): make the PDF off the update thread.
    pub(super) fn export_notebook_pdf_from_dialog(&mut self) -> Task<Message> {
        let request = match self.dialog_build_request() {
            Ok(request) => request,
            Err(reason) => {
                if let Some(reason) = reason {
                    self.refuse_export(reason);
                }
                return Task::none();
            }
        };
        let Some(dialog) = self.ui_state.notebook_pdf.dialog.as_ref() else {
            return Task::none();
        };
        let design = self.notebook_design_inputs(dialog.project);
        self.edit_notebook_pdf(|dialog| {
            if let Stage::Ready(inspection) = std::mem::replace(&mut dialog.stage, Stage::Reading) {
                dialog.stage = Stage::Exporting(inspection);
            }
        });
        jobs::build_task(request, design)
    }

    /// What Export would make; `Err(None)` when the dialog is not ready,
    /// `Err(Some(reason))` when the settings refuse it.
    fn dialog_build_request(&self) -> Result<BuildRequest, Option<Translatable>> {
        let dialog = self.ui_state.notebook_pdf.dialog.as_ref().ok_or(None)?;
        let Stage::Ready(inspection) = &dialog.stage else {
            return Err(None);
        };
        let pages = dialog
            .page_selection()
            .ok_or_else(|| Some(Translatable::new(ids::NOTEBOOK_PDF_BAD_RANGE)))?;
        let stale = &inspection.stale.stale;
        if dialog.options.stale == StaleRule::Fail && !stale.is_empty() {
            let names: Vec<String> = stale.iter().map(|r| r.name()).collect();
            return Err(Some(
                Translatable::new(ids::NOTEBOOK_PDF_STALE_REFUSED)
                    .arg("count", stale.len())
                    .arg("names", names.join(", ")),
            ));
        }
        let path = jobs::resolve_path(&dialog.project_dir, &dialog.path_text)
            .ok_or_else(|| Some(Translatable::new(ids::NOTEBOOK_PDF_NO_PATH)))?;
        // The person picked the file, so replacing it is theirs to choose;
        // it is still a PDF and never a Signex document.
        let path =
            crate::app::notebook_project::export_target(&dialog.project_dir, &path, "pdf", true)
                .map_err(Some)?;
        let mut options = dialog.options.clone();
        options.pages = pages;
        Ok(BuildRequest {
            project_dir: dialog.project_dir.clone(),
            project_name: dialog.project_name.clone(),
            notebook: dialog.notebook.clone(),
            path: Some(path),
            overwrite: None,
            confined: false,
            options,
            generation: Some(dialog.generation),
            waiting: None,
        })
    }

    /// The PDF was made: write it through the export write path.
    pub(super) fn write_notebook_pdf(
        &mut self,
        request: BuildRequest,
        result: Arc<Result<BuiltPdf, String>>,
    ) -> Task<Message> {
        let built = match Arc::try_unwrap(result).unwrap_or_else(|shared| (*shared).clone()) {
            Ok(built) => built,
            Err(why) => return self.notebook_pdf_failed(request, why, true),
        };
        let BuiltPdf { path, export } = built;
        if let Err(why) = self.refuse_while_project_moves(&path, "export to") {
            return self.notebook_pdf_failed(request, why.to_string(), true);
        }
        let log = format!(
            "Wrote {} ({} page(s), {} bytes)",
            path.display(),
            export.summary.pages,
            export.pdf.len()
        );
        let warnings = export
            .summary
            .warnings
            .iter()
            .map(|d| format!("{}:{}:{}: {}", d.file, d.line, d.column, d.message))
            .collect();
        let request = Box::new(request);
        let summary = Box::new(export.summary);
        self.write_export(path, export.pdf, "PDF", log, None, warnings)
            .map(move |message| match message {
                Message::Export(ExportMsg::FileWritten(outcome)) => {
                    Message::from(NotebookPdfMsg::Written {
                        request: request.clone(),
                        summary: summary.clone(),
                        outcome,
                    })
                }
                other => other,
            })
    }

    /// The write landed: the done card, or the command's answer.
    pub(super) fn finish_notebook_pdf(
        &mut self,
        request: BuildRequest,
        summary: ExportSummary,
        outcome: ExportWriteOutcome,
    ) -> Task<Message> {
        let path = outcome.path.clone();
        let result = outcome.result.clone();
        let written = self.handle_export_written(outcome);
        if let Err(why) = result {
            // The write path has already said why.
            let failed = self.notebook_pdf_failed(request, why, false);
            return Task::batch([written, failed]);
        }
        let checksums = &mut self.ui_state.notebook_pdf.last_checksums;
        let same_as_last = checksums
            .insert(path.clone(), summary.sha256.clone())
            .map(|last| last == summary.sha256);
        if let Some(waiting) = request.waiting {
            let answer = answer_json(&request.project_dir, &request.notebook, &path, &summary);
            let answered =
                self.finish_waiting_command(waiting, &path, |_| CommandOutcome::Answered(answer));
            return Task::batch([written, answered]);
        }
        self.edit_notebook_pdf(|dialog| {
            if Some(dialog.generation) == request.generation {
                dialog.stage = Stage::Done(Box::new(ExportDone {
                    path,
                    summary,
                    same_as_last,
                }));
            }
        });
        written
    }

    /// The export failed with `why`: the command is refused, or the dialog
    /// is ready again and, when `report`, the notice says why.
    fn notebook_pdf_failed(
        &mut self,
        request: BuildRequest,
        why: String,
        report: bool,
    ) -> Task<Message> {
        if let Some(waiting) = request.waiting {
            let target = request.project_dir.clone();
            let reason = crate::app::notebook_project::because(why);
            return self
                .finish_waiting_command(waiting, &target, |_| CommandOutcome::Disabled { reason });
        }
        self.edit_notebook_pdf(|dialog| {
            if Some(dialog.generation) != request.generation {
                return;
            }
            if let Stage::Exporting(inspection) =
                std::mem::replace(&mut dialog.stage, Stage::Reading)
            {
                dialog.stage = Stage::Ready(inspection);
            }
        });
        if report {
            self.report_export_failure(Translatable::new(ids::NOTEBOOK_PDF_FAILED).arg("why", why));
        }
        Task::none()
    }

    /// Open PDF, or Show in folder when `reveal`, off the update thread.
    pub(super) fn open_exported_notebook_pdf(&self, reveal: bool) -> Task<Message> {
        let Some(Stage::Done(done)) = self.ui_state.notebook_pdf.dialog.as_ref().map(|d| &d.stage)
        else {
            return Task::none();
        };
        let path: PathBuf = done.path.clone();
        let opener = self.ui_state.io.opener;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let opened = if reveal {
                        opener.reveal(&path)
                    } else {
                        opener.view(&path)
                    };
                    opened.map_err(|e| format!("{}: {e}", path.display()))
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
            },
            |opened| match opened {
                Ok(()) => Message::noop(),
                Err(why) => Message::from(NotebookPdfMsg::OpenFailed(why)),
            },
        )
    }

    /// Open PDF or Show in folder did not launch.
    pub(super) fn report_notebook_pdf_open_failed(&self, why: &str) {
        let line = t!(self.ui_state.i18n, ids::NOTEBOOK_PDF_OPEN_FAILED, "why" => why);
        crate::diagnostics::log_warning(line.into_string());
    }
}

/// The answer of `export_notebook_pdf`: the summary, with the project, the
/// notebook and the file.
pub(super) fn answer_json(
    project_dir: &Path,
    notebook: &str,
    path: &Path,
    summary: &ExportSummary,
) -> serde_json::Value {
    let mut answer = serde_json::to_value(summary).unwrap_or(serde_json::Value::Null);
    if let Some(map) = answer.as_object_mut() {
        map.insert(
            "project_dir".into(),
            project_dir.display().to_string().into(),
        );
        map.insert(
            "notebook".into(),
            format!("{}/{notebook}", signex_notebook::NOTEBOOK_DIR).into(),
        );
        map.insert("path".into(), path.display().to_string().into());
    }
    answer
}
