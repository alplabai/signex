//! Validate, Execute, Report... and the fix links of the Update PCB dialog,
//! plus what the board and the list let Execute do.

use std::path::PathBuf;

use iced::Task;
use signex_engine::pcb::eco::{EcoKey, check, lower, report_text};
use signex_i18n::{ids, t};
use signex_net::cross_probe::PartOccurrence;

use super::UpdatePcbMsg;
use super::state::{BoardStatus, Phase, file_name};
use crate::app::state::cross_select::{CrossSelection, PendingProbe};
use crate::app::{AnnotateMsg, CheckedStep, Message, Signex};

/// The file name Report... suggests.
const REPORT_FILE: &str = "update-pcb-report.txt";

impl Signex {
    /// What the dialog's board lets Execute do; `None` with no board loaded.
    pub(crate) fn update_pcb_board_status(&self) -> Option<BoardStatus> {
        let board = self.ui_state.update_pcb.as_ref()?.board.as_deref()?;
        let engine = self.document_state.board_engine(board)?;
        Some(if !self.document_state.has_pcb_tab(board) {
            BoardStatus::Parked
        } else if engine.board().unknown.newer_minor_source.is_some() {
            BoardStatus::ReadOnly
        } else {
            BoardStatus::Editable
        })
    }

    /// The list no longer describes the board or the design: the board moved
    /// past its revision, or a sheet it read was edited since.
    pub fn update_pcb_stale(&self) -> bool {
        let Some(state) = self.ui_state.update_pcb.as_ref() else {
            return false;
        };
        let (Some(list), Some(snapshot), Some(board)) = (
            state.list.as_ref(),
            state.snapshot.as_ref(),
            state.board.as_deref(),
        ) else {
            return false;
        };
        let board_moved = self
            .document_state
            .board_engine(board)
            .is_some_and(|engine| engine.revision() != list.revision);
        let sheet_edited = snapshot.sheet_revisions.iter().any(|(path, revision)| {
            self.document_state
                .engines
                .get(path)
                .is_some_and(|engine| engine.revision() != *revision)
        });
        board_moved || sheet_edited
    }

    /// Why Execute is off, when the list is ready but cannot run.
    pub fn update_pcb_execute_block(&self) -> Option<String> {
        let state = self.ui_state.update_pcb.as_ref()?;
        let board = file_name(state.board.as_deref()?);
        let i18n = &self.ui_state.i18n;
        match self.update_pcb_board_status() {
            Some(BoardStatus::Parked) => {
                return Some(t!(i18n, ids::UPDATE_PCB_PARKED, "board" => board).into_string());
            }
            Some(BoardStatus::ReadOnly) => {
                return Some(t!(i18n, ids::UPDATE_PCB_READ_ONLY, "board" => board).into_string());
            }
            Some(BoardStatus::Editable) => {}
            None => return Some(self.board_missing_text(&board)),
        }
        self.update_pcb_stale()
            .then(|| t!(i18n, ids::UPDATE_PCB_STALE).into_string())
    }

    fn board_missing_text(&self, board: &str) -> String {
        t!(self.ui_state.i18n, ids::UPDATE_PCB_BOARD_MISSING, "board" => board).into_string()
    }

    /// Validate: would the ticked rows apply to the board as one step? Runs
    /// them on a copy.
    pub(super) fn validate_update_pcb(&mut self) {
        let Some(state) = self.ui_state.update_pcb.as_ref() else {
            return;
        };
        let (Some(list), Some(board)) = (state.ready_list(), state.board.as_deref()) else {
            return;
        };
        let verdict = match self.document_state.board_engine(board) {
            Some(engine) => check(list, engine).map_err(|e| e.to_string()),
            None => Err(self.board_missing_text(&file_name(board))),
        };
        if let Some(state) = self.ui_state.update_pcb.as_mut() {
            state.validation = Some(verdict);
        }
    }

    /// Execute: the ticked rows as one labelled undo step at the list's
    /// revision. On success the dialog closes and the board comes forward.
    pub(super) fn execute_update_pcb(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.update_pcb.as_ref() else {
            return Task::none();
        };
        let (Some(list), Some(board)) = (state.ready_list().cloned(), state.board.clone()) else {
            return Task::none();
        };
        if let Some(why) = self.update_pcb_execute_block() {
            self.refuse_execute(why);
            return Task::none();
        }
        let Some(engine) = self.document_state.board_engine(&board) else {
            return Task::none();
        };
        let commands = lower(&list, engine.board());
        let count = list.rows.iter().filter(|r| list.applies(r)).count();
        let label = self.update_pcb_text(ids::UPDATE_PCB_TITLE);
        let step = CheckedStep {
            label: &label,
            expected_revision: list.revision,
        };
        match self.execute_board_commands(&board, commands, Some(step)) {
            Ok(_) => {
                self.ui_state.update_pcb = None;
                crate::diagnostics::log_info(
                    t!(
                        self.ui_state.i18n,
                        ids::UPDATE_PCB_DONE,
                        "count" => count,
                        "board" => file_name(&board)
                    )
                    .into_string(),
                );
                self.activate_tab_for_path(&board);
            }
            Err(error) => self.refuse_execute(error.to_string()),
        }
        Task::none()
    }

    /// Execute did not run: Validate's line says why (a headless run ends).
    fn refuse_execute(&mut self, why: String) {
        self.log_update_pcb_refused(&why);
        let Some(state) = self.ui_state.update_pcb.as_mut() else {
            return;
        };
        if state.apply {
            self.ui_state.update_pcb = None;
        } else {
            state.validation = Some(Err(why));
        }
    }

    /// Report...: ask where to save the list as text.
    pub(super) fn pick_update_pcb_report(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.update_pcb.as_ref() else {
            return Task::none();
        };
        if state.list.is_none() {
            return Task::none();
        }
        let dir = state
            .board
            .as_deref()
            .and_then(std::path::Path::parent)
            .map(std::path::Path::to_path_buf);
        let title = self.update_pcb_text(ids::UPDATE_PCB_TITLE);
        let picker = self.ui_state.io.picker.clone();
        Task::perform(
            async move {
                let mut dialog = picker
                    .dialog()
                    .set_title(title)
                    .set_file_name(REPORT_FILE)
                    .add_filter("Text", &["txt"]);
                if let Some(dir) = dir {
                    dialog = dialog.set_directory(dir);
                }
                dialog.save_file().await
            },
            |path| Message::from(UpdatePcbMsg::ReportPathPicked(path)),
        )
    }

    /// The user picked where the report goes: write it there, off the update
    /// thread. The report is the list as it stands, ticks included.
    pub(super) fn write_update_pcb_report(&mut self, path: Option<PathBuf>) -> Task<Message> {
        let (Some(path), Some(list)) = (
            path,
            self.ui_state
                .update_pcb
                .as_ref()
                .and_then(|s| s.list.as_ref()),
        ) else {
            return Task::none();
        };
        let text = report_text(list);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    signex_types::atomic_io::atomic_write(&path, text.as_bytes())
                        .map(|()| path)
                        .map_err(|e| e.to_string())
                })
                .await
                .unwrap_or_else(|e| Err(format!("the worker stopped: {e}")))
            },
            |result| Message::from(UpdatePcbMsg::ReportWritten(result)),
        )
    }

    pub(super) fn update_pcb_report_written(&mut self, result: Result<PathBuf, String>) {
        let i18n = &self.ui_state.i18n;
        match result {
            Ok(path) => crate::diagnostics::log_info(
                t!(i18n, ids::UPDATE_PCB_REPORT_SAVED, "path" => path.display().to_string())
                    .into_string(),
            ),
            Err(why) => crate::diagnostics::log_warning(
                t!(i18n, ids::UPDATE_PCB_REPORT_FAILED, "why" => why).into_string(),
            ),
        }
    }

    /// Annotate...: the dialog closes and Annotate opens on the project's
    /// root sheet, brought forward when it is open.
    pub(super) fn annotate_from_update_pcb(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.update_pcb.take() else {
            return Task::none();
        };
        let root = self
            .document_state
            .project_by_id(state.project)
            .and_then(crate::app::project_sheets::project_root_sheet_path);
        if let Some(root) = root {
            self.activate_tab_for_path(&root);
        }
        self.update(Message::Annotate(AnnotateMsg::OpenDialog))
    }

    /// Cross Probe: the dialog closes and the part of an Add row is shown on
    /// its sheet, through the pending probe the pass after the update runs.
    pub(super) fn cross_probe_from_update_pcb(&mut self, key: &EcoKey) {
        let EcoKey::AddFootprint { sheet, part } = key else {
            return;
        };
        let Some(state) = self.ui_state.update_pcb.take() else {
            return;
        };
        let Some(from) = state.board.or_else(|| self.update_pcb_source()) else {
            return;
        };
        self.interaction_state.cross_select.pending = Some(PendingProbe {
            what: CrossSelection {
                parts: vec![PartOccurrence {
                    sheet: sheet.clone(),
                    symbol: *part,
                }],
                nets: Vec::new(),
            },
            to_board: false,
            from,
            opening: None,
        });
    }

    /// Reopen: the parked board gets its tab back, edits and history kept.
    pub(super) fn reopen_update_pcb_board(&mut self) -> Task<Message> {
        let Some(board) = self
            .ui_state
            .update_pcb
            .as_ref()
            .and_then(|s| s.board.clone())
        else {
            return Task::none();
        };
        match self.open_path_reported(board) {
            Ok(task) => task,
            Err(why) => {
                if let Some(state) = self.ui_state.update_pcb.as_mut() {
                    state.phase = Phase::Failed(why);
                }
                Task::none()
            }
        }
    }
}
