//! Opening the dialog, its edits and Cancel. Apply is in `apply.rs`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use super::SheetNumberingMsg;
use super::read::read_numbering;
use super::state::SheetNumberingState;
use crate::app::{Message, Signex};

impl Signex {
    pub(crate) fn dispatch_sheet_numbering_message(
        &mut self,
        msg: SheetNumberingMsg,
    ) -> Task<Message> {
        use SheetNumberingMsg as M;
        match msg {
            M::Requested => self.open_sheet_numbering(),
            M::Read { generation, result } => self.sheet_numbering_read(generation, &result),
            M::NumberEdited { row, text } => {
                if let Some(state) = self.ui_state.sheet_numbering.as_mut() {
                    state.edit(row, text);
                }
                Task::none()
            }
            M::AutoNumberRequested => {
                if let Some(state) = self.ui_state.sheet_numbering.as_mut() {
                    state.auto_number();
                }
                Task::none()
            }
            M::ApplyRequested => self.apply_sheet_numbers(),
            M::CancelRequested => {
                self.ui_state.sheet_numbering = None;
                Task::none()
            }
        }
    }

    /// What the dialog numbers: the sheet it reads the project from (the
    /// active document, else the active project's root sheet), the project's
    /// name and folder. `None` for a sheet outside a project, which has no
    /// project numbering; the command is greyed then.
    pub(crate) fn sheet_numbering_project(&self) -> Option<(PathBuf, String, PathBuf)> {
        let document = &self.document_state;
        let source = document.active_path.clone().or_else(|| {
            document
                .active_project
                .and_then(|id| document.project_by_id(id))
                .and_then(crate::app::project_sheets::project_root_sheet_path)
        })?;
        let project = document.document_project(&source)?;
        let (name, dir) = (project.data.name.clone(), project.dir().to_path_buf());
        Some((source, name, dir))
    }

    /// Open the dialog and start reading the project's pages.
    fn open_sheet_numbering(&mut self) -> Task<Message> {
        if self.ui_state.sheet_numbering.is_some() {
            return Task::none();
        }
        let Some(target) = self.sheet_numbering_project() else {
            crate::diagnostics::log_warning(
                "Number Schematic Sheets: open a sheet of a project first; a sheet outside a \
                 project has no project sheet numbers.",
            );
            return Task::none();
        };
        self.interaction_state.editing_text = None;
        self.interaction_state.context_menu = None;
        self.start_sheet_numbering_read(target)
    }

    /// (Re)start the read of `target`'s project with a fresh generation.
    fn start_sheet_numbering_read(
        &mut self,
        (source, project_name, project_dir): (PathBuf, String, PathBuf),
    ) -> Task<Message> {
        let plan =
            crate::app::project_sheets::plan_project_sheets_for(&self.document_state, &source);
        let open: HashSet<PathBuf> = self.document_state.engines.keys().cloned().collect();
        self.ui_state.sheet_numbering_reads += 1;
        let generation = self.ui_state.sheet_numbering_reads;
        self.ui_state.sheet_numbering = Some(SheetNumberingState::reading(
            generation,
            self.document_state.documents_epoch,
            self.child_sheet_signature(),
            project_name.clone(),
        ));
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                read_numbering(&plan, Some(&project_dir), &project_name, &open)
            }),
            move |result| {
                Message::from(SheetNumberingMsg::Read {
                    generation,
                    result: result.and_then(|read| read).map(Arc::new),
                })
            },
        )
    }

    /// The read finished. A read that an opened or closed sheet made stale
    /// is started again; an answer for a closed or newer dialog is dropped.
    fn sheet_numbering_read(
        &mut self,
        generation: u64,
        result: &Result<Arc<super::NumberingRead>, String>,
    ) -> Task<Message> {
        let epoch = self.document_state.documents_epoch;
        let signature = self.child_sheet_signature();
        let Some(state) = self.ui_state.sheet_numbering.as_mut() else {
            return Task::none();
        };
        if state.generation != generation {
            return Task::none();
        }
        if state.epoch != epoch || state.child_sheets != signature {
            self.ui_state.sheet_numbering = None;
            return match self.sheet_numbering_project() {
                Some(target) => self.start_sheet_numbering_read(target),
                None => Task::none(),
            };
        }
        match result {
            Ok(read) => {
                for (path, why) in &read.unreadable {
                    crate::diagnostics::log_warning(format!(
                        "Number Schematic Sheets: sheet '{}' {why}",
                        path.display()
                    ));
                }
                state.show(Arc::clone(read));
            }
            Err(why) => {
                crate::diagnostics::log_warning(format!("Number Schematic Sheets: {why}"));
                state.error = Some(why.clone());
            }
        }
        Task::none()
    }
}
