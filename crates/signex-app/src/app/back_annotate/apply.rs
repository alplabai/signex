//! Validate and Execute. Execute writes the ticked rows into every changed
//! sheet as ONE undo group (`app/sheet_write.rs`): a sheet that is not open
//! is loaded from the copy the read took and left unsaved, after its file is
//! stated again off the UI thread; nothing is written to disk. A list that
//! no longer describes the board or the sheets is refused (Refresh).

use std::collections::BTreeMap;
use std::path::PathBuf;

use iced::Task;
use signex_engine::pcb::eco::{BackChangeList, SheetEdit, sheet_commands};
use signex_i18n::{ids, t};

use super::{BackAnnotateMsg, BackRead};
use crate::app::sheet_write::{SheetWrite, SheetWriteRefusal, disk_changed};
use crate::app::{Message, Signex, UndoGroupKind};

const LABEL: &str = "Back Annotate";

/// The edits of one sheet file: whether it is placed once, and its edits.
type FileEdits = (bool, Vec<SheetEdit>);

impl Signex {
    /// The list no longer describes the board or the design: the board moved
    /// past its revision, or a sheet the read used was edited, closed or
    /// opened since.
    pub fn back_annotate_stale(&self) -> bool {
        let Some(state) = self.ui_state.back_annotate.as_ref() else {
            return false;
        };
        let (Some(list), Some(read), Some(board)) = (
            state.list.as_ref(),
            state.read.as_ref(),
            state.board.as_deref(),
        ) else {
            return false;
        };
        let board_moved = self
            .document_state
            .board_engine(board)
            .is_none_or(|engine| engine.revision() != list.revision);
        let sheet_edited = read.sheet_revisions.iter().any(|(path, then)| {
            let now = self.document_state.engines.get(path).map(|e| e.revision());
            now != *then
        });
        board_moved || sheet_edited
    }

    /// Validate: would the ticked rows apply to every sheet? Runs them on
    /// copies of the sheets.
    pub(super) fn validate_back_annotate(&mut self) {
        let verdict = self.back_annotate_plan().and_then(|(_, read, edits)| {
            for (file, (single, file_edits)) in &edits {
                let document = match self.document_state.engines.get(file) {
                    Some(engine) => engine.document().clone(),
                    None => read.loaded.get(file).cloned().ok_or_else(|| {
                        self.back_annotate_text_path(ids::BACK_ANNOTATE_NOT_LOADED, file)
                    })?,
                };
                if document.is_read_only() {
                    return Err(self.back_annotate_text_path(ids::BACK_ANNOTATE_READ_ONLY, file));
                }
                let commands = sheet_commands(&document, file_edits, *single);
                let mut engine = signex_engine::Engine::new(document)
                    .map_err(|e| self.back_annotate_refused(file, &e.to_string()))?;
                engine
                    .execute_batch(commands)
                    .map_err(|e| self.back_annotate_refused(file, &e.to_string()))?;
            }
            Ok(())
        });
        if let Some(state) = self.ui_state.back_annotate.as_mut() {
            state.validation = Some(verdict);
        }
    }

    /// Execute: state the closed sheets again, then write.
    pub(super) fn execute_back_annotate(&mut self) -> Task<Message> {
        let (read, edits) = match self.back_annotate_plan() {
            Ok((_, read, edits)) => (read, edits),
            Err(why) => return self.refuse_back_annotate_execute(why),
        };
        let to_check: Vec<_> = edits
            .keys()
            .filter(|file| !self.document_state.engines.contains_key(*file))
            .map(|file| (file.clone(), read.stamps.get(file).copied().flatten()))
            .collect();
        if to_check.is_empty() {
            return self.write_back_annotate();
        }
        let Some(state) = self.ui_state.back_annotate.as_mut() else {
            return Task::none();
        };
        state.checking = true;
        state.validation = None;
        let job = state.job;
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || disk_changed(&to_check)),
            move |result| Message::from(BackAnnotateMsg::DiskChecked { job, result }),
        )
    }

    /// The closed sheets were stated again: write, or refuse when one of
    /// them changed on disk since the read.
    pub(super) fn back_annotate_disk_checked(
        &mut self,
        job: u64,
        result: Result<Vec<PathBuf>, String>,
    ) -> Task<Message> {
        let Some(state) = self.ui_state.back_annotate.as_mut() else {
            return Task::none();
        };
        if state.job != job || !state.checking {
            return Task::none();
        }
        state.checking = false;
        match result {
            Err(why) => self.refuse_back_annotate_execute(why),
            Ok(changed) => match changed.first() {
                Some(path) => {
                    let why = self.back_annotate_text_path(ids::BACK_ANNOTATE_DISK_CHANGED, path);
                    self.refuse_back_annotate_execute(why)
                }
                None => self.write_back_annotate(),
            },
        }
    }

    /// Check again what can have moved meanwhile, then write every sheet as
    /// one undo group. On success the dialog closes.
    fn write_back_annotate(&mut self) -> Task<Message> {
        let (list, read, edits) = match self.back_annotate_plan() {
            Ok(plan) => plan,
            Err(why) => return self.refuse_back_annotate_execute(why),
        };
        let rows = list.rows.iter().filter(|r| r.applies()).count();
        let write = SheetWrite {
            label: LABEL,
            outcome: "back-annotated",
            kind: UndoGroupKind::BackAnnotate,
            loaded: &read.loaded,
        };
        let result = self.write_sheets_as_group(&write, edits, |document, (single, edits)| {
            sheet_commands(document, edits, *single)
        });
        match result {
            Ok(sheets) => {
                crate::diagnostics::log_info(
                    t!(
                        self.ui_state.i18n,
                        ids::BACK_ANNOTATE_DONE,
                        "count" => rows,
                        "sheets" => sheets
                    )
                    .into_string(),
                );
                self.ui_state.back_annotate = None;
                Task::none()
            }
            Err(refusal) => {
                let (id, path) = match refusal {
                    SheetWriteRefusal::Busy(p) => (ids::BACK_ANNOTATE_BUSY, p),
                    SheetWriteRefusal::ReadOnly(p) => (ids::BACK_ANNOTATE_READ_ONLY, p),
                    SheetWriteRefusal::NotLoaded(p) => (ids::BACK_ANNOTATE_NOT_LOADED, p),
                    SheetWriteRefusal::NotChanged(p) => (ids::BACK_ANNOTATE_NOT_CHANGED, p),
                };
                let why = self.back_annotate_text_path(id, &path);
                self.refuse_back_annotate_execute(why)
            }
        }
    }

    /// The ready list, its read and its edits per sheet file, or why Execute
    /// and Validate cannot run.
    #[allow(clippy::type_complexity)]
    fn back_annotate_plan(
        &self,
    ) -> Result<
        (
            BackChangeList,
            std::sync::Arc<BackRead>,
            BTreeMap<PathBuf, FileEdits>,
        ),
        String,
    > {
        let Some(state) = self.ui_state.back_annotate.as_ref() else {
            return Err(String::new());
        };
        let (Some(list), Some(read)) = (state.ready_list(), state.read.clone()) else {
            return Err(String::new());
        };
        if self.back_annotate_stale() {
            return Err(t!(self.ui_state.i18n, ids::BACK_ANNOTATE_STALE).into_string());
        }
        let mut by_file: BTreeMap<PathBuf, FileEdits> = BTreeMap::new();
        for edit in list.sheet_edits() {
            let key = (edit.placement.clone(), edit.units.clone());
            let Some(part) = read.files.get(&key) else {
                return Err(t!(self.ui_state.i18n, ids::BACK_ANNOTATE_NO_FILE).into_string());
            };
            // A sheet another open project reaches keeps its `reference`
            // for that project: write a per-placement entry instead (#1224).
            let single = part.single && !self.shared_with_another_open_project(&part.file);
            by_file
                .entry(part.file.clone())
                .or_insert_with(|| (single, Vec::new()))
                .1
                .push(edit);
        }
        Ok((list.clone(), read, by_file))
    }

    /// Keep the dialog open with `why` shown; the Messages panel gets it too.
    fn refuse_back_annotate_execute(&mut self, why: String) -> Task<Message> {
        if why.is_empty() {
            return Task::none();
        }
        crate::diagnostics::log_warning(format!("{LABEL}: nothing was changed: {why}"));
        if let Some(state) = self.ui_state.back_annotate.as_mut() {
            state.validation = Some(Err(why));
            state.checking = false;
        }
        Task::none()
    }

    fn back_annotate_text_path(
        &self,
        id: signex_i18n::MessageId,
        path: &std::path::Path,
    ) -> String {
        t!(self.ui_state.i18n, id, "path" => path.display().to_string()).into_string()
    }

    fn back_annotate_refused(&self, path: &std::path::Path, why: &str) -> String {
        t!(
            self.ui_state.i18n,
            ids::BACK_ANNOTATE_REFUSED,
            "path" => path.display().to_string(),
            "why" => why
        )
        .into_string()
    }
}
