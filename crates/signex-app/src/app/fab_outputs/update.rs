use std::path::PathBuf;

use iced::Task;
use signex_engine::ProjectGeneralCommand;
use signex_i18n::{ids, t};
use signex_renderer::board3d::StepSchema;
use signex_types::pcb::PcbBoard;

use super::documents::{DocumentsJob, assembly_pdf_options, step_file_name};
use super::write::{GenerateJob, run_generate};
use super::{FabOutputKinds, FabOutputsMsg, FabOutputsState, FabProblem};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

impl Signex {
    pub(crate) fn dispatch_fab_outputs_message(&mut self, msg: FabOutputsMsg) -> Task<Message> {
        use FabOutputsMsg as M;
        match msg {
            M::Requested => self.open_fab_outputs(),
            M::OutputToggled(kind) => self.edit_fab_kinds(|k| kind.toggle(k)),
            // The folder form is greyed and deaf while ODB++ is unticked;
            // this guard is the backstop.
            M::OdbFolderToggled => {
                let odb = self
                    .ui_state
                    .fab_outputs
                    .as_deref()
                    .is_some_and(|s| self.fab_output_kinds(s).odb);
                if odb {
                    self.edit_fab_kinds(|k| k.odb_folder = !k.odb_folder);
                }
            }
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
            M::CommandWritten { run, result } => return self.fab_command_written(*run, result),
            M::FolderOpenRequested => return self.open_fab_outputs_folder(),
            M::FolderOpenFailed { folder, why } => {
                let folder = folder.display().to_string();
                let line = t!(
                    self.ui_state.i18n,
                    ids::FAB_OUTPUTS_FOLDER_NOT_OPENED,
                    "folder" => folder.as_str(),
                    "why" => why.as_str()
                );
                crate::diagnostics::log_warning(line.into_string());
            }
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
            state.generated_at = None;
        }
    }

    /// The kinds ticked for the open dialog's board: its project's stored
    /// kinds (the defaults when it stores none), else the dialog's own for a
    /// board no project names.
    pub(crate) fn fab_output_kinds(&self, state: &FabOutputsState) -> FabOutputKinds {
        match self.document_state.board_project(&state.board, None) {
            Some(project) => project.data.fab_outputs.unwrap_or_default(),
            None => state.kinds,
        }
    }

    /// A tick changed. A board's project stores it as a project edit (one
    /// undo step, the project dirty, written on Save); nothing is written
    /// here.
    fn edit_fab_kinds(&mut self, edit: impl FnOnce(&mut FabOutputKinds)) {
        let Some(state) = self.ui_state.fab_outputs.as_deref() else {
            return;
        };
        if state.writing.is_some() {
            return;
        }
        let mut kinds = self.fab_output_kinds(state);
        edit(&mut kinds);
        let project = self
            .document_state
            .board_project(&state.board, None)
            .map(|project| project.id);
        self.with_fab_outputs(|s| s.kinds = kinds);
        if let Some(project_id) = project {
            self.store_fab_kinds(project_id, kinds);
        }
    }

    /// Store `kinds` in project `project_id` through its General history,
    /// as the Project tab's edits are.
    fn store_fab_kinds(&mut self, project_id: ProjectId, kinds: FabOutputKinds) {
        let document = &mut self.document_state;
        document.note_project_clean_state(project_id);
        let Some(project) = document.projects.iter_mut().find(|p| p.id == project_id) else {
            return;
        };
        let history = document.general_history.entry(project_id).or_default();
        let command = ProjectGeneralCommand::SetFabOutputs(kinds.stored());
        if history.apply(&mut project.data, command) {
            // Not a rule or a sheet: ERC results stay current.
            document.sync_project_history_dirty(project_id, false);
            self.refresh_panel_ctx();
        }
    }

    /// Open the dialog on the board in view with its project's kinds.
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
        state.kinds = self.fab_output_kinds(&state);
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

    /// What Generate makes of `kinds` from `board`, a copy of the dialog's
    /// board as it is in memory: the dialog's choices, the words the
    /// assembly PDF prints and where the parts' STEP models are read from.
    /// `fabrication_outputs` with operands builds its job here too.
    pub(super) fn generate_job(
        &self,
        state: &FabOutputsState,
        kinds: FabOutputKinds,
        board: PcbBoard,
    ) -> GenerateJob {
        let name = state.name.trim().to_string();
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
            designator_standard: self.document_state.designator_standard_for(&state.board),
            // The dialog writes the writer's own Gerber (#1299).
            gerber: Default::default(),
        }
    }

    /// Generate: copy the board as it is in memory and build and write its
    /// files on a worker. The dialog stays open with the outcome.
    fn generate_fab_outputs(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.fab_outputs.as_deref() else {
            return Task::none();
        };
        if !state.can_generate(&self.fab_output_kinds(state)) {
            return Task::none();
        }
        // The copy the files are written from: designators as the project's
        // standard shows them (#1416).
        let job = self
            .document_state
            .pcb_board_shown(&state.board)
            .map(std::borrow::Cow::into_owned)
            .map(|board| self.generate_job(state, self.fab_output_kinds(state), board));
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
        state.generated_at = None;
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
        self.log_fab_outcome(result.as_ref());
        if let Some(state) = self.ui_state.fab_outputs.as_mut()
            && state.writing == Some(generation)
        {
            state.writing = None;
            state.outcome = Some(result);
            // The clock is read here, when the outcome arrives, never in view.
            state.generated_at = Some(chrono::Local::now().time());
        }
    }

    /// The Messages line of a finished Generate, the dialog's or the
    /// command's: what was written with every note and stale file, a warning
    /// when there is any, or why nothing was.
    pub(super) fn log_fab_outcome(&self, result: Result<&super::FabWritten, &FabProblem>) {
        let (line, is_warning) = match result {
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
    }

    /// Open folder in the success banner: show the folder the last Generate
    /// wrote in the file manager, off the update thread. A failed launch is
    /// a Messages warning.
    fn open_fab_outputs_folder(&mut self) -> Task<Message> {
        let Some(Some(Ok(written))) = self.ui_state.fab_outputs.as_ref().map(|s| &s.outcome) else {
            return Task::none();
        };
        let folder = written.folder.clone();
        let opener = self.ui_state.io.opener;
        let target = folder.clone();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                opener.view(&target).map_err(|err| err.to_string())
            }),
            move |result| match result.and_then(|opened| opened) {
                Ok(()) => Message::noop(),
                Err(why) => Message::from(FabOutputsMsg::FolderOpenFailed { folder, why }),
            },
        )
    }
}
