//! The ordered per-project queue of journal lines (#2253). `update` only puts
//! a line on it; one task per project at a time appends what is waiting, off
//! the update thread, in the order the events happened. What the task reports
//! comes back as `AgentMsg::JournalDrained` and every failure reaches
//! Messages. See `docs/AGENT_JOURNALS.md`, "Writing".

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use iced::Task;

use super::append::{Journal, append_line};
use crate::app::Message;
use crate::app::agent::AgentMsg;

/// One line waiting to be appended.
#[derive(Debug, Clone)]
pub(crate) struct Queued {
    pub(crate) journal: Journal,
    pub(crate) line: String,
    pub(crate) max_lines: u32,
}

#[derive(Default)]
struct ProjectQueue {
    pending: VecDeque<Queued>,
    /// A task is appending this project's lines now.
    draining: bool,
}

/// The lines waiting to be written, by the project folder they belong to.
#[derive(Default)]
pub(crate) struct JournalQueue {
    projects: BTreeMap<PathBuf, ProjectQueue>,
}

impl JournalQueue {
    /// Queue `item` for `project_dir`. The task to start when none is running
    /// for it already; its lines are then appended in order.
    pub(crate) fn push(&mut self, project_dir: &Path, item: Queued) -> Task<Message> {
        let queue = self.projects.entry(project_dir.to_path_buf()).or_default();
        queue.pending.push_back(item);
        Self::start_if_idle(project_dir, queue)
    }

    /// A task for `project_dir` ended: the task for what has waited since, if
    /// any; the project's queue is dropped when it is empty.
    pub(crate) fn drained(&mut self, project_dir: &Path) -> Task<Message> {
        let Some(queue) = self.projects.get_mut(project_dir) else {
            return Task::none();
        };
        queue.draining = false;
        if queue.pending.is_empty() {
            self.projects.remove(project_dir);
            return Task::none();
        }
        Self::start_if_idle(project_dir, queue)
    }

    /// How many lines wait or are being written, for tests.
    #[cfg(test)]
    pub(crate) fn outstanding(&self) -> usize {
        self.projects
            .values()
            .map(|queue| queue.pending.len())
            .sum()
    }

    fn start_if_idle(project_dir: &Path, queue: &mut ProjectQueue) -> Task<Message> {
        if queue.draining || queue.pending.is_empty() {
            return Task::none();
        }
        queue.draining = true;
        let batch: Vec<Queued> = queue.pending.drain(..).collect();
        drain_task(project_dir.to_path_buf(), batch)
    }
}

/// Append `batch` to the project's journals, in order, on a worker.
fn drain_task(project_dir: PathBuf, batch: Vec<Queued>) -> Task<Message> {
    let reported = project_dir.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || write_batch(&project_dir, &batch))
                .await
                .unwrap_or_else(|error| vec![format!("the journal write did not finish: {error}")])
        },
        move |failures| {
            Message::Agent(AgentMsg::JournalDrained {
                project: reported.clone(),
                failures,
            })
        },
    )
}

/// Append every line; what failed, said once each. A failure does not stop
/// the lines after it.
pub(crate) fn write_batch(project_dir: &Path, batch: &[Queued]) -> Vec<String> {
    batch
        .iter()
        .filter_map(|item| append_line(project_dir, item.journal, &item.line, item.max_lines).err())
        .collect()
}
