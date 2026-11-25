//! Block diagram export (#2796, mockup row 171): a `.snxblk` written as an
//! SVG or PDF file. In a window `export_block_diagram` takes the diagram in
//! view (a `.snxblk` tab) or asks for one, then asks for the file; with
//! operands it writes `document` to `path` and answers once written. The
//! drawing is `signex_output::block_diagram_drawing`, the files
//! `block_diagram_svg` and `block_diagram_pdf`. The saved file is what is
//! exported. Nothing in the diagram changes, so there is no undo step.
//! Rules: `docs/BLOCK_DIAGRAM_EXPORT.md`.

mod command;
mod job;

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{Translatable, ids, t};
use signex_output::{
    BlockDiagramFormat, BlockDiagramLook, BlockDiagramPdfOptions, BlockDiagramPdfText,
};
use signex_types::block_diagram::BLOCK_DIAGRAM_EXTENSION;

use crate::app::agent::WaitingCommand;
use crate::app::command::CommandOutcome;
use crate::app::contracts::ExportWriteOutcome;
use crate::app::handlers::menu::export::uncovered_chars_warning;
use crate::app::state::ErrorNotice;
use crate::app::{ExportMsg, Message, Signex};

pub(crate) use command::export_block_diagram;
use job::is_block_diagram;
pub use job::{ExportJob, Exported};

impl From<BlockDiagramExportMsg> for Message {
    fn from(msg: BlockDiagramExportMsg) -> Self {
        Self::Export(ExportMsg::BlockDiagram(msg))
    }
}

/// What happened in a block diagram export, namespaced under
/// `ExportMsg::BlockDiagram`.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockDiagramExportMsg {
    /// `export_block_diagram` without operands in a window.
    Requested,
    /// The open dialog answered with the diagram to export.
    DiagramPicked(PathBuf),
    /// The save dialog answered with `path` for `diagram`.
    PathChosen { diagram: PathBuf, path: PathBuf },
    /// An export ran off the update thread. Boxed: the largest variant.
    Written(Box<ExportWritten>),
}

/// What an export that ran off the update thread brings back.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportWritten {
    pub job: ExportJob,
    /// What it wrote, or why nothing.
    pub result: Result<Exported, String>,
    /// The command run to answer; `None` from the dialogs.
    pub waiting: Option<WaitingCommand>,
}

/// The file name of `path`, else the whole path.
fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

impl Signex {
    pub(crate) fn handle_block_diagram_export(
        &mut self,
        msg: BlockDiagramExportMsg,
    ) -> Task<Message> {
        match msg {
            BlockDiagramExportMsg::Requested => self.request_block_diagram_export(),
            BlockDiagramExportMsg::DiagramPicked(diagram) => self.ask_block_diagram_file(diagram),
            BlockDiagramExportMsg::PathChosen { diagram, path } => {
                self.export_block_diagram_to(diagram, path)
            }
            BlockDiagramExportMsg::Written(written) => {
                let ExportWritten {
                    job,
                    result,
                    waiting,
                } = *written;
                self.block_diagram_written(job, result, waiting)
            }
        }
    }

    /// The diagram in view, else the open dialog for one.
    fn request_block_diagram_export(&mut self) -> Task<Message> {
        if let Some(diagram) = self.active_tab_path().filter(|p| is_block_diagram(p)) {
            return self.ask_block_diagram_file(diagram);
        }
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::BLOCK_DIAGRAM_EXPORT_OPEN_TITLE)
            .filter_in(
                i18n,
                ids::BLOCK_DIAGRAM_EXPORT_FILTER_DIAGRAM,
                &[BLOCK_DIAGRAM_EXTENSION],
            );
        Task::perform(dialog.pick_file(), |picked| match picked {
            Some(diagram) => Message::from(BlockDiagramExportMsg::DiagramPicked(diagram)),
            None => Message::noop(),
        })
    }

    /// The save dialog for `diagram`, offering `<name>_blocks.svg`; the
    /// extension picked decides the format.
    fn ask_block_diagram_file(&mut self, diagram: PathBuf) -> Task<Message> {
        let target = self.save_target(Some(&diagram));
        let picker = self.ui_state.io.picker.clone();
        let i18n = self.ui_state.i18n.clone();
        let file_name = target.file_name("block-diagram", "_blocks", "svg");
        Task::perform(
            async move {
                let titled = picker
                    .dialog()
                    .title_in(&i18n, ids::BLOCK_DIAGRAM_EXPORT_SAVE_TITLE);
                target
                    .dialog(titled)
                    .await
                    .filter_in(&i18n, ids::BLOCK_DIAGRAM_EXPORT_FILTER_SVG, &["svg"])
                    .filter_in(&i18n, ids::BLOCK_DIAGRAM_EXPORT_FILTER_PDF, &["pdf"])
                    .set_file_name(file_name)
                    .save_file()
                    .await
            },
            move |path| match path {
                Some(path) => Message::from(BlockDiagramExportMsg::PathChosen { diagram, path }),
                None => Message::noop(),
            },
        )
    }

    /// The dialogs' export: refused with a notice for a file that is not
    /// `.svg` or `.pdf`, which the diagram itself never is.
    fn export_block_diagram_to(&mut self, diagram: PathBuf, path: PathBuf) -> Task<Message> {
        let Some(format) = BlockDiagramFormat::from_path(&path) else {
            self.refuse_export(
                Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_UNKNOWN_EXTENSION)
                    .arg("file", file_label(&path)),
            );
            return Task::none();
        };
        if self.refuse_with_notice_while_project_moves(&path, "export to", ErrorNotice::export) {
            return Task::none();
        }
        let job = self.block_diagram_job(diagram, path, format, BlockDiagramLook::default(), true);
        self.start_block_diagram_export(job, None)
    }

    /// The job for `diagram` to `path`; the PDF title block in the user's
    /// language.
    pub(super) fn block_diagram_job(
        &self,
        diagram: PathBuf,
        path: PathBuf,
        format: BlockDiagramFormat,
        look: BlockDiagramLook,
        title_block: bool,
    ) -> ExportJob {
        let label = t!(self.ui_state.i18n, ids::BLOCK_DIAGRAM_EXPORT_PAGE_LABEL).into_string();
        ExportJob {
            text: BlockDiagramPdfText {
                title: file_label(&diagram),
                label,
            },
            pdf: BlockDiagramPdfOptions {
                title_block,
                monochrome: look.monochrome,
                ..BlockDiagramPdfOptions::default()
            },
            diagram,
            path,
            format,
            look,
        }
    }

    /// Run `job` on the blocking pool, held as a project write so a folder
    /// rename waits for it. The caller checked the rename guard.
    pub(super) fn start_block_diagram_export(
        &mut self,
        job: ExportJob,
        waiting: Option<WaitingCommand>,
    ) -> Task<Message> {
        self.begin_project_write(&job.path);
        let work = job.clone();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || job::run(&work)),
            move |result| {
                Message::from(BlockDiagramExportMsg::Written(Box::new(ExportWritten {
                    job,
                    result: result.and_then(|written| written),
                    waiting,
                })))
            },
        )
    }

    /// The export's file is written, or why not: the Messages line, and
    /// for a command run its answer.
    fn block_diagram_written(
        &mut self,
        job: ExportJob,
        result: Result<Exported, String>,
        waiting: Option<WaitingCommand>,
    ) -> Task<Message> {
        let i18n = &self.ui_state.i18n;
        // The Messages line of a written file, or why nothing was written.
        let outcome = match result {
            Ok(done) => Ok((written_line(i18n, &job, &done), done)),
            Err(reason) => Err(Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_FAILED)
                .arg("document", file_label(&job.diagram))
                .arg("reason", reason)),
        };
        let Some(waiting) = waiting else {
            let write = ExportWriteOutcome {
                path: job.path.clone(),
                result: outcome.as_ref().map(|_| ()).map_err(|why| why.render(i18n)),
                success_log: outcome
                    .as_ref()
                    .map(|(line, _)| line.clone())
                    .unwrap_or_default(),
                advisory: None,
                warnings: outcome
                    .as_ref()
                    .ok()
                    .and_then(|(_, done)| uncovered_chars_warning(&done.uncovered_chars))
                    .into_iter()
                    .collect(),
                preview_source: None,
            };
            return self.handle_export_written(write);
        };
        match &outcome {
            Ok((line, _)) => crate::diagnostics::log_info(line),
            Err(why) => crate::diagnostics::log_warning(why.render(i18n)),
        }
        self.end_project_write(&job.path);
        let answer = match outcome {
            Ok((_, done)) => command::answered(&job, &done),
            Err(reason) => CommandOutcome::Disabled { reason },
        };
        self.finish_waiting_command(waiting, &job.diagram, move |_| answer)
    }
}

/// The Messages line of a written export.
fn written_line(i18n: &signex_i18n::I18n, job: &ExportJob, done: &Exported) -> String {
    t!(
        i18n,
        ids::BLOCK_DIAGRAM_EXPORT_WRITTEN,
        "file" => job.path.display().to_string(),
        "blocks" => done.blocks,
        "connections" => done.connections,
    )
    .into_string()
}

#[cfg(test)]
mod tests;
