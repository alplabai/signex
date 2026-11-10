//! Turning what the app knows at an agent command into journal lines (#2253):
//! who, which document at which revision, what ran and how it came out. The
//! time is taken when the event happens, here in `update`, so the order of the
//! lines is the order of the events. Nothing here touches the disk; lines go on
//! the [`JournalQueue`](super::queue::JournalQueue). See
//! `docs/AGENT_JOURNALS.md`.

use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use iced::Task;
use signex_types::agent_journal::AgentJournalSettings;

use super::append::Journal;
use super::entry::{AccessEntry, EditEntry, JOURNAL_VERSION};
use super::queue::Queued;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// What is known about the command before it runs.
pub(crate) struct Started {
    ts: String,
    document: Option<String>,
    path: Option<PathBuf>,
    /// The project's `.snxprj`; its folder holds the journals.
    project: Option<PathBuf>,
    revision_before: Option<u64>,
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Commands that write or close a document without being `mutates_doc`; the
/// catalog has no flag for them, so they are named (as in `signex-cli`'s
/// capability groups, which a test there keeps in step with the catalog).
const ALSO_JOURNALED: [&str; 5] = [
    "save_document",
    "save_document_as",
    "new_document",
    "new_project_with_profile",
    "close_active_document",
];

/// Does `command` change a document, or what is on disk or open? Only those
/// are journaled; a selection step or a copy is not.
fn changes_documents(command: &AppCommandId) -> bool {
    ALSO_JOURNALED.contains(&command.as_str())
        || crate::keymap::metadata_for(command).is_some_and(|meta| meta.flags.mutates_doc)
}

impl Signex {
    /// Run an agent command and journal it: the command runs and answers as it
    /// would without a journal, whatever the journal comes to.
    pub(crate) fn journaled(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
        run: impl FnOnce(&mut Self) -> Ran,
    ) -> Ran {
        if !self.agent.journals_on || !self.agent.origin.is_agent() || !changes_documents(command) {
            return run(self);
        }
        let started = self.journal_started(args);
        self.document_state.agent_edits.last_group = None;
        let (outcome, task) = run(self);
        let write = self.journal_edit(started, command, args, &outcome);
        (outcome, Task::batch([task, write]))
    }

    fn journal_started(&self, args: &CommandArgs) -> Started {
        let entry = match args.operand("document") {
            Some(OperandValue::Document(target)) => self.resolve_document_target(target).ok(),
            _ => None,
        };
        let path = entry.as_ref().and_then(|entry| entry.path.clone());
        let project = entry
            .as_ref()
            .and_then(|entry| entry.project.as_ref())
            .map(|project| project.path.clone())
            .or_else(|| self.only_open_project());
        Started {
            ts: now(),
            document: entry
                .as_ref()
                .and_then(|entry| entry.id)
                .map(|id| id.to_string()),
            revision_before: path.as_deref().and_then(|path| self.engine_revision(path)),
            path,
            project,
        }
    }

    fn engine_revision(&self, path: &Path) -> Option<u64> {
        self.document_state
            .engines
            .get(path)
            .map(signex_engine::Engine::revision)
    }

    /// The project's `.snxprj` when exactly one is open: where a command that
    /// names no document is journaled.
    fn only_open_project(&self) -> Option<PathBuf> {
        match self.document_state.projects.as_slice() {
            [only] => Some(only.path.clone()),
            _ => None,
        }
    }

    fn journal_edit(
        &mut self,
        started: Started,
        command: &AppCommandId,
        args: &CommandArgs,
        outcome: &CommandOutcome,
    ) -> Task<Message> {
        let (pid, client) = match &self.agent.origin {
            super::super::Origin::Agent { pid, client } => (
                *pid,
                client.clone().or_else(|| self.agent.run_client.clone()),
            ),
            super::super::Origin::Person => (None, None),
        };
        let (outcome, reason) = match outcome {
            CommandOutcome::Ran | CommandOutcome::Answered(_) => ("ran", None),
            CommandOutcome::Disabled { reason } => ("refused", Some(reason.clone())),
            other => ("failed", Some(format!("{other:?}"))),
        };
        let mut entry = EditEntry {
            v: JOURNAL_VERSION,
            ts: started.ts,
            pid,
            client,
            document: started.document,
            revision_after: started
                .path
                .as_deref()
                .and_then(|path| self.engine_revision(path)),
            path: started.path.map(|path| path.display().to_string()),
            command: command.as_str().to_string(),
            operands: args.operand_texts().clone(),
            revision_before: started.revision_before,
            group: self
                .document_state
                .agent_edits
                .last_group
                .map(|group| group.get()),
            outcome: outcome.to_string(),
            reason,
            truncated: false,
            replayable: false,
        };
        if let Some(secret) = self.agent.token_text() {
            entry.redact(secret);
        }
        let Some(project) = started.project else {
            crate::diagnostics::log_warning(format!(
                "Agent journal: `{}` was not recorded: it names no project",
                command.as_str()
            ));
            return Task::none();
        };
        let line = entry.to_line();
        self.queue_journal_line(&project, Journal::Edits, line)
    }

    /// Record an access event in the access journal of every open project:
    /// a connection reaches all of them, so each keeps the record.
    pub(crate) fn journal_access(
        &mut self,
        build: impl Fn(String) -> AccessEntry,
    ) -> Task<Message> {
        if !self.agent.journals_on {
            return Task::none();
        }
        let ts = now();
        let projects: Vec<PathBuf> = self
            .document_state
            .projects
            .iter()
            .map(|project| project.path.clone())
            .collect();
        let tasks: Vec<_> = projects
            .iter()
            .map(|project| {
                let line = build(ts.clone()).to_line();
                self.queue_journal_line(project, Journal::Access, line)
            })
            .collect();
        Task::batch(tasks)
    }

    /// Put `line` on the queue of the project whose file is `project_file`,
    /// with that project's line cap.
    fn queue_journal_line(
        &mut self,
        project_file: &Path,
        journal: Journal,
        line: String,
    ) -> Task<Message> {
        let Some(folder) = project_file.parent() else {
            return Task::none();
        };
        let settings = self
            .document_state
            .projects
            .iter()
            .find(|project| project.path == project_file)
            .map_or_else(AgentJournalSettings::default, |project| {
                project.data.agent_journal
            });
        let item = Queued {
            journal,
            line,
            max_lines: settings.clamped().max_lines,
        };
        self.agent.journal.push(folder, item)
    }
}
