//! The ordered per-project queue of journal lines (#2253). `update` only puts
//! a line on it; one task per project at a time appends what is waiting, off
//! the update thread, in the order the events happened. What the task reports
//! comes back as `AgentMsg::JournalDrained` and every failure reaches
//! Messages. The quit path takes what has not started and waits for what has
//! (`take_for_quit`). See `docs/AGENT_JOURNALS.md`, "Writing".

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use iced::Task;

use super::append::{Journal, append_line};
use crate::app::Message;
use crate::app::agent::AgentMsg;

/// How long the quit path waits for a write in flight.
pub(crate) const QUIT_WAIT: Duration = Duration::from_secs(10);

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

/// How many drain tasks are running or about to, and the way to wait for none.
#[derive(Default)]
struct Gate {
    running: Mutex<usize>,
    idle: Condvar,
}

/// Counts a drain task for as long as it lives.
struct Running(Arc<Gate>);

impl Running {
    fn enter(gate: &Arc<Gate>) -> Self {
        *gate.running.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        Self(Arc::clone(gate))
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let mut running = self
            .0
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *running -= 1;
        self.0.idle.notify_all();
    }
}

/// The lines waiting to be written, by the project folder they belong to.
#[derive(Default)]
pub(crate) struct JournalQueue {
    projects: BTreeMap<PathBuf, ProjectQueue>,
    gate: Arc<Gate>,
}

/// What the quit path writes: the lines that had not started, after the
/// writes in flight are done.
pub(crate) struct QuitFlush {
    gate: Arc<Gate>,
    lines: Vec<(PathBuf, Vec<Queued>)>,
}

impl QuitFlush {
    /// Is there nothing to wait for and nothing to write?
    pub(crate) fn is_idle(&self) -> bool {
        self.lines.is_empty()
            && *self
                .gate
                .running
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                == 0
    }

    /// Wait (bounded) for the writes in flight, then write the rest in order.
    /// What could not be written, each said once.
    pub(crate) fn run(self) -> Vec<String> {
        let mut failures = Vec::new();
        let running = self
            .gate
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (running, wait) = self
            .gate
            .idle
            .wait_timeout_while(running, QUIT_WAIT, |running| *running > 0)
            .unwrap_or_else(PoisonError::into_inner);
        if wait.timed_out() {
            failures.push(format!(
                "a journal write was still running after {} s at quit; its lines may be missing",
                QUIT_WAIT.as_secs()
            ));
        }
        drop(running);
        for (dir, batch) in &self.lines {
            failures.extend(write_batch(dir, batch));
        }
        failures
    }
}

impl JournalQueue {
    /// Queue `item` for `project_dir`. The task to start when none is running
    /// for it already; its lines are then appended in order.
    pub(crate) fn push(&mut self, project_dir: &Path, item: Queued) -> Task<Message> {
        let queue = self.projects.entry(project_dir.to_path_buf()).or_default();
        queue.pending.push_back(item);
        Self::start_if_idle(&self.gate, project_dir, queue)
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
        Self::start_if_idle(&self.gate, project_dir, queue)
    }

    /// The quit path: take every line that has not started (so no task starts
    /// for them), to be written after the writes in flight.
    pub(crate) fn take_for_quit(&mut self) -> QuitFlush {
        let lines = self
            .projects
            .iter_mut()
            .filter(|(_, queue)| !queue.pending.is_empty())
            .map(|(dir, queue)| (dir.clone(), queue.pending.drain(..).collect()))
            .collect();
        QuitFlush {
            gate: Arc::clone(&self.gate),
            lines,
        }
    }

    /// How many lines wait to be written, for tests.
    #[cfg(test)]
    pub(crate) fn outstanding(&self) -> usize {
        self.projects
            .values()
            .map(|queue| queue.pending.len())
            .sum()
    }

    fn start_if_idle(
        gate: &Arc<Gate>,
        project_dir: &Path,
        queue: &mut ProjectQueue,
    ) -> Task<Message> {
        if queue.draining || queue.pending.is_empty() {
            return Task::none();
        }
        queue.draining = true;
        let batch: Vec<Queued> = queue.pending.drain(..).collect();
        drain_task(project_dir.to_path_buf(), batch, Running::enter(gate))
    }
}

/// Append `batch` to the project's journals, in order, on a worker.
fn drain_task(project_dir: PathBuf, batch: Vec<Queued>, running: Running) -> Task<Message> {
    let reported = project_dir.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let failures = write_batch(&project_dir, &batch);
                drop(running);
                failures
            })
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
