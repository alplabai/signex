use std::path::PathBuf;

use iced::Task;
use signex_i18n::{ids, t};
use signex_renderer::board3d::StepSchema;
use signex_types::pcb::PcbBoard;

use super::documents::{DocumentsJob, assembly_pdf_options, step_file_name};
use super::write::{GenerateJob, run_generate};
use super::{FabKinds, FabOutputsMsg, FabOutputsState, FabProblem};
use crate::app::{Message, Signex};

impl Signex {
    pub(crate) fn dispatch_fab_outputs_message(&mut self, msg: FabOutputsMsg) -> Task<Message> {
        use FabOutputsMsg as M;
        match msg {
            M::Requested => self.open_fab_outputs(),
            M::OutputToggled(kind) => self.edit_fab_kinds(|k| kind.toggle(k)),
            M::OdbFolderToggled => self.edit_fab_kinds(|k| k.odb_folder = !k.odb_folder),
            M::WholePanelToggled => self.with_fab_outputs(|s| {
                s.whole_panel = s.panel.is_some() && !s.whole_panel;
            }),
            M::NameEdited(name) => self.with_fab_outputs(|s| s.name = name),
            M::FolderEdited(folder) => self.with_fab_outputs(|s| s.folder = folder),
            M::FolderPicked(Some(folder)) => {
                self.with_fab_outputs(|s| s.folder = folder.display().to_string());
            }
            M::FolderPicked(None) => {}
            M::BrowseRequested => return self.browse_fab_outputs_folder(),
            M::GenerateRequested => return self.generate_fab_outputs(),
            M::Written { generation, result } => self.fab_outputs_written(generation, result),
            M::CancelRequested => self.ui_state.fab_outputs = None,
        }
        Task::none()
    }

    /// An edit of the open dialog; it clears the last outcome, which no
    /// longer describes what Generate would write.
    fn with_fab_outputs(&mut self, edit: impl FnOnce(&mut FabOutputsState)) {
        if let Some(state) = self.ui_state.fab_outputs.as_mut()
            && state.writing.is_none()
        {
            edit(state);
            state.outcome = None;
        }
    }

    /// A change of the kinds of file; the next opening starts from them.
    fn edit_fab_kinds(&mut self, edit: impl FnOnce(&mut FabKinds)) {
        self.with_fab_outputs(|s| edit(&mut s.kinds));
        if let Some(state) = self.ui_state.fab_outputs.as_ref() {
            self.ui_state.fab_outputs_kinds = state.kinds;
        }
    }

    /// Open the dialog on the board in view with the kinds last used.
    /// Nothing is read or written.
    fn open_fab_outputs(&mut self) {
        if self.ui_state.fab_outputs.is_some() {
            return;
        }
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            let line = t!(self.ui_state.i18n, ids::FAB_OUTPUTS_NO_PCB);
            crate::diagnostics::log_info(line.into_string());
            return;
        };
        let board_name = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.title.clone())
            .unwrap_or_default();
        // A board with a panel writes the whole panel unless unticked (#2410).
        let panel = self
            .document_state
            .pcb_board(&path)
            .and_then(|board| board.options.panel)
            .map(|p| (p.columns, p.rows));
        let mut state = FabOutputsState::new(path, board_name);
        state.kinds = self.ui_state.fab_outputs_kinds;
        state.panel = panel;
        state.whole_panel = panel.is_some();
        self.interaction_state.context_menu = None;
        self.ui_state.fab_outputs = Some(Box::new(state));
    }

    fn browse_fab_outputs_folder(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.fab_outputs.as_ref() else {
            return Task::none();
        };
        let picker = self.ui_state.io.picker.clone();
        let start = PathBuf::from(state.folder.trim());
        let title = t!(self.ui_state.i18n, ids::FAB_OUTPUTS_PICK_FOLDER).into_string();
        Task::perform(
            async move {
                let mut dialog = picker.dialog().set_title(title);
                let existing = start.ancestors().find(|dir| dir.is_dir());
                if let Some(dir) = existing.filter(|dir| !dir.as_os_str().is_empty()) {
                    dialog = dialog.set_directory(dir);
                }
                dialog.pick_folder().await
            },
            |picked| Message::from(FabOutputsMsg::FolderPicked(picked)),
        )
    }

    /// What Generate makes from `board`, a copy of the dialog's board as it
    /// is in memory: the dialog's choices, the words the assembly PDF
    /// prints and where the parts' STEP models are read from.
    fn generate_job(&self, state: &FabOutputsState, board: PcbBoard) -> GenerateJob {
        let name = state.name.trim().to_string();
        let kinds = state.kinds;
        let pdf = kinds.assembly_pdf.then(|| {
            let options = assembly_pdf_options(&self.ui_state.board_output.print_options);
            let text = crate::app::board_output::print_text(&self.ui_state.i18n, &state.board_name);
            (options, text)
        });
        let step = kinds.step.then(|| {
            self.step_context(
                &board,
                step_file_name(&name),
                name.clone(),
                StepSchema::default(),
            )
        });
        GenerateJob {
            board,
            whole_panel: state.whole_panel,
            name,
            kinds,
            documents: DocumentsJob {
                pdf,
                svg: kinds.assembly_svg,
                step,
            },
            folder: PathBuf::from(state.folder.trim()),
        }
    }

    /// Generate: copy the board as it is in memory and build and write its
    /// files on a worker. The dialog stays open with the outcome.
    fn generate_fab_outputs(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.fab_outputs.as_deref() else {
            return Task::none();
        };
        if !state.can_generate() {
            return Task::none();
        }
        let job = self
            .document_state
            .board_engine(&state.board)
            .map(|engine| engine.board().clone())
            .map(|board| self.generate_job(state, board));
        self.ui_state.fab_outputs_writes += 1;
        let generation = self.ui_state.fab_outputs_writes;
        let Some(state) = self.ui_state.fab_outputs.as_mut() else {
            return Task::none();
        };
        let Some(job) = job else {
            state.outcome = Some(Err(FabProblem::BoardGone));
            return Task::none();
        };
        state.writing = Some(generation);
        state.outcome = None;
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || run_generate(job)),
            move |result| {
                Message::from(FabOutputsMsg::Written {
                    generation,
                    result: result
                        .map_err(FabProblem::WriteFailed)
                        .and_then(|written| written),
                })
            },
        )
    }

    /// A write finished: the outcome shows in the dialog and in Messages,
    /// with every note and stale file, as a warning when there is any. An
    /// answer for a closed or newer dialog only reaches Messages.
    fn fab_outputs_written(
        &mut self,
        generation: u64,
        result: Result<super::FabWritten, FabProblem>,
    ) {
        let (line, is_warning) = match &result {
            Ok(written) => super::written_message(written, &self.ui_state.i18n),
            Err(problem) => (
                super::problem_text(problem, &self.ui_state.i18n).into_string(),
                true,
            ),
        };
        if is_warning {
            crate::diagnostics::log_warning(line);
        } else {
            crate::diagnostics::log_info(line);
        }
        if let Some(state) = self.ui_state.fab_outputs.as_mut()
            && state.writing == Some(generation)
        {
            state.writing = None;
            state.outcome = Some(result);
        }
    }
}
