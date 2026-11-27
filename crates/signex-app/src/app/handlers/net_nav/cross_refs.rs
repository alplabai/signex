//! Keeping the off-sheet cross references (#1273) current: the index build
//! they read is asked for when the sheet in view has a Global label, and a
//! `query_cross_references` that waited for a build is answered when it
//! lands. The table itself is `app::cross_refs`.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_types::schematic::LabelType;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::state::ProjectId;
use crate::app::state::net_nav::{Stored, WaitingCrossRefs};
use crate::keymap::AppCommandId;

use super::super::super::*;
use super::index_job::IndexLookup;

impl Signex {
    /// Ask for the index of the active sheet's project when the sheet has a
    /// Global label, so its cross references follow edits. Plans only when
    /// connectivity or the active sheet changed since the last look.
    pub(super) fn warm_net_index_for_cross_refs(&mut self) -> Task<Message> {
        let active = self.document_state.active_path.clone();
        let stamp = (self.net_nav_stamp(), active.clone());
        if self.document_view.net_nav.cross_ref_stamp.as_ref() == Some(&stamp) {
            return Task::none();
        }
        self.document_view.net_nav.cross_ref_stamp = Some(stamp);
        let Some(path) = active else {
            return Task::none();
        };
        let has_global = self
            .document_state
            .engines
            .get(&path)
            .is_some_and(|engine| {
                engine
                    .document()
                    .labels
                    .iter()
                    .any(|label| label.label_type == LabelType::Global)
            });
        let Some(project) = self.project_of_sheet(&path).filter(|_| has_global) else {
            return Task::none();
        };
        match self.net_index_lookup(project) {
            IndexLookup::Building(task) => task,
            IndexLookup::Ready(_) | IndexLookup::Unavailable => Task::none(),
        }
    }

    /// Run `query_cross_references` on `document` of `project`: answered now
    /// from a current index, else answered when the build it waits for lands
    /// (`answer_cross_ref_queries`).
    pub(crate) fn start_cross_ref_query(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
        project: ProjectId,
        document: PathBuf,
    ) -> (CommandOutcome, Task<Message>) {
        match self.net_index_lookup_owned(project) {
            IndexLookup::Ready(data) => (
                CommandOutcome::Answered(self.cross_refs_answer(&document, &data)),
                Task::none(),
            ),
            IndexLookup::Building(build) => {
                let waiting = self.wait_for_command(command, args);
                let ticket = waiting.ticket;
                self.document_view
                    .net_nav
                    .cross_ref_queries
                    .push(WaitingCrossRefs {
                        project,
                        document,
                        waiting,
                    });
                (CommandOutcome::Deferred { ticket }, build)
            }
            IndexLookup::Unavailable => (
                CommandOutcome::disabled(
                    "the project's net index could not be built; see Messages".to_string(),
                ),
                Task::none(),
            ),
        }
    }

    /// Answer the cross-reference queries of `project` that waited for the
    /// build `stored` records: with the landed index, or refused when it
    /// failed. A stale build leaves them waiting for the next one.
    pub(super) fn answer_cross_ref_queries(
        &mut self,
        project: ProjectId,
        stored: &Stored,
    ) -> Task<Message> {
        let data = match stored {
            Stored::Fresh(data) => Ok(Arc::clone(data)),
            Stored::Failed(reason) => Err(reason.clone()),
            Stored::Stale => return Task::none(),
        };
        let queries = &mut self.document_view.net_nav.cross_ref_queries;
        let (mine, others): (Vec<_>, Vec<_>) = std::mem::take(queries)
            .into_iter()
            .partition(|q| q.project == project);
        *queries = others;
        let mut tasks = Vec::with_capacity(mine.len());
        for query in mine {
            let outcome = match &data {
                Ok(data) => CommandOutcome::Answered(self.cross_refs_answer(&query.document, data)),
                Err(reason) => CommandOutcome::disabled(format!(
                    "the project's net index could not be built: {reason}"
                )),
            };
            tasks.push(self.finish_waiting_command(query.waiting, &query.document, |_| outcome));
        }
        Task::batch(tasks)
    }
}
