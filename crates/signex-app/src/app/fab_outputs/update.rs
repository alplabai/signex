use std::path::PathBuf;

use iced::Task;
use signex_i18n::{ids, t};

use super::{FabOutputsMsg, FabOutputsState, FabProblem, write_fab_outputs};
use crate::app::{Message, Signex};

impl Signex {
    pub(crate) fn dispatch_fab_outputs_message(&mut self, msg: FabOutputsMsg) -> Task<Message> {
        use FabOutputsMsg as M;
        match msg {
            M::Requested => self.open_fab_outputs(),
            M::OutputToggled(kind) => self.with_fab_outputs(|s| kind.toggle(&mut s.options)),
            M::NameEdited(name) => self.with_fab_outputs(|s| s.options.name = name),
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

    /// Open the dialog on the board in view. Nothing is read or written.
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
        self.interaction_state.context_menu = None;
        self.ui_state.fab_outputs = Some(FabOutputsState::new(path, board_name));
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

    /// Generate: copy the board as it is in memory and build and write its
    /// files on a worker. The dialog stays open with the outcome.
    fn generate_fab_outputs(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.fab_outputs.as_ref() else {
            return Task::none();
        };
        if !state.can_generate() {
            return Task::none();
        }
        let (board_path, options) = (state.board.clone(), state.options.clone());
        let folder = PathBuf::from(state.folder.trim());
        let board = self
            .document_state
            .board_engine(&board_path)
            .map(|engine| engine.board().clone());
        self.ui_state.fab_outputs_writes += 1;
        let generation = self.ui_state.fab_outputs_writes;
        let Some(state) = self.ui_state.fab_outputs.as_mut() else {
            return Task::none();
        };
        let Some(board) = board else {
            state.outcome = Some(Err(FabProblem::BoardGone));
            return Task::none();
        };
        state.writing = Some(generation);
        state.outcome = None;
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                write_fab_outputs(&board, &options, &folder)
            }),
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

    /// A write finished: the outcome shows in the dialog and in Messages.
    /// An answer for a closed or newer dialog only reaches Messages.
    fn fab_outputs_written(
        &mut self,
        generation: u64,
        result: Result<super::FabWritten, FabProblem>,
    ) {
        let line = match &result {
            Ok(written) => super::written_text(written, &self.ui_state.i18n),
            Err(problem) => super::problem_text(problem, &self.ui_state.i18n),
        };
        match &result {
            Ok(_) => crate::diagnostics::log_info(line.into_string()),
            Err(_) => crate::diagnostics::log_warning(line.into_string()),
        }
        if let Some(state) = self.ui_state.fab_outputs.as_mut()
            && state.writing == Some(generation)
        {
            state.writing = None;
            state.outcome = Some(result);
        }
    }
}
