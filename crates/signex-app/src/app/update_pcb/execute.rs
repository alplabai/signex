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

fn board_missing_text(i18n: &signex_i18n::I18n, board: &str) -> String {
    t!(i18n, ids::UPDATE_PCB_BOARD_MISSING, "board" => board).into_string()
}

impl Signex {
    /// What the dialog's board lets Execute do; `None` with no board loaded.
    pub(crate) fn update_pcb_board_status(&self) -> Option<BoardStatus> {
        let board = self.ui_state.update_pcb.as_ref()?.board.as_deref()?;
        self.board_status(board)
    }

    /// What `board` lets a change list do; `None` while it is not loaded.
    /// Update PCB and Compare Board with Schematic share it.
    pub(crate) fn board_status(&self, board: &std::path::Path) -> Option<BoardStatus> {
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
    /// past its revision, or a sheet it read was edited, closed or opened
    /// since (revisions are unique, so a reopened sheet reads as changed).
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
        let sheet_edited = snapshot.sheet_revisions.iter().any(|(path, read)| {
            let now = self.document_state.engines.get(path).map(|e| e.revision());
            now != *read
        });
        board_moved || sheet_edited
    }

    /// Why Execute is off, when the list is ready but cannot run.
    pub fn update_pcb_execute_block(&self) -> Option<String> {
        self.update_pcb_execute_block_in(&self.ui_state.i18n)
    }

    /// [`Self::update_pcb_execute_block`] in a given language (Messages
    /// takes en-US).
    fn update_pcb_execute_block_in(&self, i18n: &signex_i18n::I18n) -> Option<String> {
        let state = self.ui_state.update_pcb.as_ref()?;
        if let Some(why) = self.board_block_in(state.board.as_deref()?, i18n) {
            return Some(why);
        }
        self.update_pcb_stale()
            .then(|| t!(i18n, ids::UPDATE_PCB_STALE).into_string())
    }

    /// Why `board` cannot take a change list's step, in `i18n`: parked,
    /// read-only or not loaded. `None` when it is editable.
    pub(crate) fn board_block_in(
        &self,
        board: &std::path::Path,
        i18n: &signex_i18n::I18n,
    ) -> Option<String> {
        let name = file_name(board);
        match self.board_status(board) {
            Some(BoardStatus::Parked) => {
                Some(t!(i18n, ids::UPDATE_PCB_PARKED, "board" => name).into_string())
            }
            Some(BoardStatus::ReadOnly) => {
                Some(t!(i18n, ids::UPDATE_PCB_READ_ONLY, "board" => name).into_string())
            }
            Some(BoardStatus::Editable) => None,
            None => Some(board_missing_text(i18n, &name)),
        }
    }

    /// Validate: would the ticked rows apply to the board as one step? Runs
    /// them on a copy. A stale list fails it, as Execute would.
    pub(super) fn validate_update_pcb(&mut self) {
        let Some(state) = self.ui_state.update_pcb.as_ref() else {
            return;
        };
        let (Some(list), Some(board)) = (state.ready_list(), state.board.as_deref()) else {
            return;
        };
        let verdict = match self.document_state.board_engine(board) {
            _ if self.update_pcb_stale() => {
                Err(t!(self.ui_state.i18n, ids::UPDATE_PCB_STALE).into_string())
            }
            Some(engine) => check(list, engine)
                .map_err(|e| e.to_string())
                .and_then(|()| {
                    let rows = self.update_pcb_rule_rows(state);
                    self.prepare_update_pcb_rules(state.project, &rows)
                        .map(|_| ())
                }),
            None => Err(board_missing_text(&self.ui_state.i18n, &file_name(board))),
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
        // #1489: the rules the schematic asks for go with the board's rows.
        let project = state.project;
        let rule_rows = self.update_pcb_rule_rows(state);
        let rules_pending = super::rules_from_schematic::pending(&rule_rows);
        // Exit criterion 3: the directive classes go with them.
        let snapshot = state.snapshot.clone();
        let classes_pending =
            (snapshot.as_deref()).is_some_and(|s| self.update_pcb_class_edit(&board, s).is_some());
        // The dialog's Execute is off with nothing ticked; Enter is too.
        if !state.apply && state.execute_count() == 0 && rules_pending == 0 && !classes_pending {
            return Task::none();
        }
        if state.execute_count() == 0 {
            let classes = |app: &mut Self| match snapshot.as_deref() {
                Some(snapshot) => app.apply_update_pcb_classes(&board, snapshot),
                None => Ok(()),
            };
            let applied = self
                .apply_update_pcb_rules(project, &rule_rows)
                .and_then(|()| classes(self));
            if let Err(why) = applied {
                self.refuse_execute(why);
                return Task::none();
            }
            self.ui_state.update_pcb = None;
            return Task::none();
        }
        if let Some(why) = self.update_pcb_execute_block() {
            let logged = self
                .update_pcb_execute_block_in(&signex_i18n::I18n::en_us())
                .unwrap_or_else(|| why.clone());
            self.refuse_execute_as(&logged, why);
            return Task::none();
        }
        if self.document_state.board_engine(&board).is_none() {
            return Task::none();
        }
        // #2734: a rule half that cannot land refuses before the board moves.
        let prepared = match self.prepare_update_pcb_rules(project, &rule_rows) {
            Ok(prepared) => prepared,
            Err(why) => {
                self.refuse_execute(why);
                return Task::none();
            }
        };
        let count = list.rows.iter().filter(|r| list.applies(r)).count();
        let label = self.update_pcb_text(ids::UPDATE_PCB_TITLE);
        match self.execute_eco_list(&board, &list, &label) {
            Ok(()) => {
                if let Err(why) = self.commit_update_pcb_rules(project, prepared) {
                    // The board step landed; say why the rules did not.
                    crate::diagnostics::log_warning(why);
                }
                if let Some(snapshot) = snapshot.as_deref()
                    && let Err(why) = self.apply_update_pcb_classes(&board, snapshot)
                {
                    crate::diagnostics::log_warning(why);
                }
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
            Err(why) => self.refuse_execute(why),
        }
        Task::none()
    }

    /// The rows of `list` that apply, run on `board` as one undo step
    /// labelled `label` at the list's revision: refused, with nothing
    /// changed, once the board moved past it. Update PCB's Execute and
    /// Compare Board with Schematic's Apply share it.
    pub(crate) fn execute_eco_list(
        &mut self,
        board: &std::path::Path,
        list: &signex_engine::pcb::eco::EcoChangeList,
        label: &str,
    ) -> Result<(), String> {
        let Some(engine) = self.document_state.board_engine(board) else {
            return Err(board_missing_text(&self.ui_state.i18n, &file_name(board)));
        };
        let commands = lower(list, engine.board());
        let step = CheckedStep {
            label,
            expected_revision: list.revision,
        };
        self.execute_board_commands(board, commands, Some(step))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Execute did not run: Validate's line says why (a headless run ends).
    fn refuse_execute(&mut self, why: String) {
        let logged = why.clone();
        self.refuse_execute_as(&logged, why);
    }

    /// [`Self::refuse_execute`] when Messages (en-US) and the dialog (UI
    /// language) word the reason differently.
    fn refuse_execute_as(&mut self, logged: &str, why: String) {
        self.log_update_pcb_refused(logged);
        let Some(state) = self.ui_state.update_pcb.as_mut() else {
            return;
        };
        if state.apply {
            self.ui_state.update_pcb = None;
        } else {
            state.validation = Some(Err(why));
        }
    }

    /// Report...: ask where to save the list as text. Only a ready list:
    /// while the net rows are recomputed they are out of date.
    pub(super) fn pick_update_pcb_report(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.update_pcb.as_ref() else {
            return Task::none();
        };
        if state.ready_list().is_none() {
            return Task::none();
        }
        let dir = state
            .board
            .as_deref()
            .and_then(std::path::Path::parent)
            .map(std::path::Path::to_path_buf);
        let i18n = &self.ui_state.i18n;
        let base = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::UPDATE_PCB_TITLE)
            .filter_in(i18n, ids::FILE_FILTER_TEXT, &["txt"]);
        Task::perform(
            async move {
                let mut dialog = base.set_file_name(REPORT_FILE);
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
