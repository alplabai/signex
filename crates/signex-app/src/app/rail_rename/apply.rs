//! The rename's steps: read the project, state the closed sheets again, then
//! change every sheet of the rail as one undo group. Sheets that changed under
//! a read are read again; anything else that stops it changes nothing.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_i18n::{ids, t};
use signex_types::schematic::SchematicSheet;

use super::{
    MAX_READS, RailProblem, RailRead, RailRename, RailRenameMsg, RunningRename, holds_rail,
    rail_commands, read_rail,
};
use crate::app::path_op_jobs::run_blocking;
use crate::app::project_sheets::plan_project_sheets_for;
use crate::app::sheet_write::{SheetWrite, disk_changed};
use crate::app::{Message, Signex, UndoGroupKind};
use crate::library::closed_sheets::FileStamp;

const LABEL: &str = "Rename power rail";

impl Signex {
    /// Rename the rail `rename` names: at once on a loose sheet, else queued
    /// for [`Self::start_rail_rename`].
    pub(crate) fn rename_rail(&mut self, rename: RailRename) {
        if rename.new == rename.old {
            return;
        }
        if rename.new.trim().is_empty() {
            self.refuse_rail(&rename, &RailProblem::EmptyName);
            return;
        }
        if self
            .document_state
            .document_project(&rename.sheet)
            .is_some()
        {
            self.document_state.rail_rename.queue.push_back(rename);
            return;
        }
        let edits = BTreeMap::from([(rename.sheet.clone(), ())]);
        self.write_rail(&rename, edits, &HashMap::new());
    }

    /// Start the next queued rename's read, unless one is running. Run by the
    /// update loop after every message.
    pub(crate) fn start_rail_rename(&mut self) -> Task<Message> {
        let state = &mut self.document_state.rail_rename;
        if state.running.is_some() {
            return Task::none();
        }
        let Some(rename) = state.queue.pop_front() else {
            return Task::none();
        };
        let owner = self.document_state.document_project(&rename.sheet);
        let Some(project) = owner.map(|p| p.id) else {
            self.refuse_rail(&rename, &RailProblem::Stale);
            return Task::none();
        };
        let plan = plan_project_sheets_for(&self.document_state, &rename.sheet);
        let open: HashSet<PathBuf> = (self.document_state.engines.iter())
            .map(|(path, _)| path.clone())
            .collect();
        let old = rename.old.clone();
        let epoch = self.document_state.documents_epoch;
        let state = &mut self.document_state.rail_rename;
        state.next_job += 1;
        let job = state.next_job;
        state.running = Some(RunningRename {
            job,
            rename,
            epoch,
            project,
            read: None,
        });
        Task::perform(
            run_blocking(move || read_rail(&plan, &open, &old)),
            move |result| {
                Message::from(RailRenameMsg::Read {
                    job,
                    result: result.map(Arc::new),
                })
            },
        )
    }

    pub(crate) fn dispatch_rail_rename(&mut self, msg: RailRenameMsg) -> Task<Message> {
        match msg {
            RailRenameMsg::Read { job, result } => self.rail_read_finished(job, result),
            RailRenameMsg::DiskChecked { job, result } => self.rail_disk_checked(job, result),
        }
    }

    fn rail_read_finished(
        &mut self,
        job: u64,
        result: Result<Arc<RailRead>, String>,
    ) -> Task<Message> {
        if !self.document_state.rail_rename.is_running(job) {
            return Task::none();
        }
        let read = match result {
            Ok(read) => read,
            Err(why) => {
                self.stop_rail(&RailProblem::ReadFailed(why));
                return Task::none();
            }
        };
        if self.rail_read_is_stale() {
            self.read_rail_again();
            return Task::none();
        }
        if let Some((path, _)) = read.unreadable.first() {
            self.stop_rail(&RailProblem::Unreadable(path.clone()));
            return Task::none();
        }
        let to_check: Vec<(PathBuf, Option<FileStamp>)> =
            read.stamps.iter().map(|(p, s)| (p.clone(), *s)).collect();
        if to_check.is_empty() {
            self.write_running_rail(&read);
            return Task::none();
        }
        if let Some(running) = self.document_state.rail_rename.running.as_mut() {
            running.read = Some(read);
        }
        Task::perform(
            run_blocking(move || disk_changed(&to_check)),
            move |result| Message::from(RailRenameMsg::DiskChecked { job, result }),
        )
    }

    fn rail_disk_checked(
        &mut self,
        job: u64,
        result: Result<Vec<PathBuf>, String>,
    ) -> Task<Message> {
        if !self.document_state.rail_rename.is_running(job) {
            return Task::none();
        }
        match result {
            Err(why) => self.stop_rail(&RailProblem::ReadFailed(why)),
            Ok(changed) if !changed.is_empty() || self.rail_read_is_stale() => {
                self.read_rail_again();
            }
            Ok(_) => {
                let running = self.document_state.rail_rename.running.as_ref();
                if let Some(read) = running.and_then(|r| r.read.clone()) {
                    self.write_running_rail(&read);
                }
            }
        }
        Task::none()
    }

    /// A document was opened, saved, closed or moved since the read.
    fn rail_read_is_stale(&self) -> bool {
        let epoch = self.document_state.documents_epoch;
        (self.document_state.rail_rename.running.as_ref()).is_some_and(|r| r.epoch != epoch)
    }

    /// Put the running rename back to be read again, a bounded number of times.
    fn read_rail_again(&mut self) {
        let Some(running) = self.document_state.rail_rename.running.take() else {
            return;
        };
        let mut rename = running.rename;
        rename.attempts += 1;
        if rename.attempts >= MAX_READS {
            self.refuse_rail(&rename, &RailProblem::Stale);
        } else {
            self.document_state.rail_rename.queue.push_front(rename);
        }
    }

    fn stop_rail(&mut self, problem: &RailProblem) {
        if let Some(running) = self.document_state.rail_rename.running.take() {
            self.refuse_rail(&running.rename, problem);
        }
    }

    /// Check the live sheets against the read, then change them.
    fn write_running_rail(&mut self, read: &RailRead) {
        let Some(running) = self.document_state.rail_rename.running.take() else {
            return;
        };
        let rename = running.rename;
        let edits = self.rail_members(&rename, read);
        match self.rail_problem(&rename, running.project, &edits) {
            Some(problem) => self.refuse_rail(&rename, &problem),
            None => self.write_rail(&rename, edits, &read.loaded),
        }
    }

    /// Every sheet holding the rail now: the open ones by their live
    /// document, the closed ones as read, and the sheet typed on.
    fn rail_members(&self, rename: &RailRename, read: &RailRead) -> BTreeMap<PathBuf, ()> {
        let engines = &self.document_state.engines;
        let open = (read.open.iter())
            .filter(|path| {
                (engines.get(*path)).is_some_and(|e| holds_rail(e.document(), &rename.old))
            })
            .cloned();
        open.chain(read.loaded.keys().cloned())
            .chain(std::iter::once(rename.sheet.clone()))
            .map(|path| (path, ()))
            .collect()
    }

    /// Why the rename cannot run: its port changed, or a sheet of the rail is
    /// also part of another open project, whose rail it would split.
    fn rail_problem(
        &self,
        rename: &RailRename,
        project: crate::app::state::ProjectId,
        members: &BTreeMap<PathBuf, ()>,
    ) -> Option<RailProblem> {
        let holds = (self.document_state.engines.get(&rename.sheet)).is_some_and(|engine| {
            let mut symbols = engine.document().symbols.iter();
            symbols.any(|s| s.uuid == rename.port && s.value == rename.old)
        });
        if !holds {
            return Some(RailProblem::PortGone);
        }
        (members.keys())
            .find(|path| self.shared_with_project_other_than(project, path))
            .map(|path| RailProblem::Shared(path.clone()))
    }

    /// One undo group over `edits`, all or nothing.
    fn write_rail(
        &mut self,
        rename: &RailRename,
        edits: BTreeMap<PathBuf, ()>,
        loaded: &HashMap<PathBuf, SchematicSheet>,
    ) {
        let write = SheetWrite {
            label: LABEL,
            outcome: "renamed",
            kind: UndoGroupKind::RailRename,
            loaded,
        };
        let (old, new) = (rename.old.as_str(), rename.new.as_str());
        match self.write_sheets_as_group(&write, edits, |doc, _| rail_commands(doc, old, new)) {
            Ok(sheets) if sheets > 1 => crate::diagnostics::log_info(
                t!(self.ui_state.i18n, ids::PROPERTIES_RAIL_RENAMED,
                    "old" => old,
                    "new" => new,
                    "sheets" => sheets)
                .into_string(),
            ),
            Ok(_) => {}
            Err(refusal) => self.refuse_rail(rename, &RailProblem::from(refusal)),
        }
    }

    /// Say in Messages that nothing changed and why, quoting the typed name.
    fn refuse_rail(&self, rename: &RailRename, problem: &RailProblem) {
        let (id, path) = problem.why();
        let why = t!(self.ui_state.i18n, id, "path" => path).into_string();
        crate::diagnostics::log_warning(
            t!(self.ui_state.i18n, ids::PROPERTIES_RAIL_RENAME_REFUSED,
                "old" => rename.old.as_str(),
                "new" => rename.new.as_str(),
                "why" => why.as_str())
            .into_string(),
        );
    }
}
