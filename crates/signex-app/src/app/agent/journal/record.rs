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
use crate::app::document_ids::DocumentTarget;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// What replaces a secret in a line before it is queued.
const REDACTED: &str = "[redacted]";

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

/// The rights of a client as one project may see them: every capability, and
/// the `allow-path:` roots that lie inside the project's folder. The roots
/// outside it are not recorded (they would name another project's place); how
/// many there were is.
pub(crate) fn rights_within(rights: &[String], project_dir: &Path) -> Vec<String> {
    let mut kept = Vec::new();
    let mut dropped = 0_usize;
    for right in rights {
        match right.strip_prefix("allow-path:") {
            Some(root) if !Path::new(root).starts_with(project_dir) => dropped += 1,
            _ => kept.push(right.clone()),
        }
    }
    if dropped > 0 {
        kept.push(format!(
            "allow-path:({dropped} outside this project not recorded)"
        ));
    }
    kept
}

/// `text` without the folder of any other open project: each is replaced, so
/// one project's journal never names another's place.
fn without_other_projects(text: &str, own_dir: &Path, all: &[PathBuf]) -> String {
    let mut out = text.to_string();
    for file in all {
        let Some(dir) = file.parent() else { continue };
        if dir == own_dir || own_dir.starts_with(dir) {
            continue;
        }
        out = out.replace(&dir.display().to_string(), "[another project]");
    }
    out
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

    /// The project files of every open project.
    pub(crate) fn open_project_files(&self) -> Vec<PathBuf> {
        self.document_state
            .projects
            .iter()
            .map(|project| project.path.clone())
            .collect()
    }

    /// The `.snxprj` of the project the `document` operand of a request names,
    /// if there is one and it is open.
    pub(crate) fn project_named_by(&self, operands: &[(String, String)]) -> Option<PathBuf> {
        let (_, text) = operands.iter().find(|(name, _)| name == "document")?;
        let target = DocumentTarget::parse(text).ok()?;
        let entry = self.resolve_document_target(&target).ok()?;
        entry.project.map(|project| project.path)
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
        // A Save As moves the sheet: its revision now is under the new path.
        let after_path = match (command.as_str(), args.operand("path")) {
            ("save_document_as", Some(OperandValue::Path(new))) => Some(PathBuf::from(new)),
            _ => None,
        };
        let revision_after = after_path
            .as_deref()
            .and_then(|path| self.engine_revision(path))
            .or_else(|| {
                started
                    .path
                    .as_deref()
                    .and_then(|path| self.engine_revision(path))
            });
        let entry = EditEntry {
            v: JOURNAL_VERSION,
            ts: started.ts,
            session: self.agent.session.clone(),
            pid,
            client,
            document: started.document,
            revision_after,
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
        let Some(project) = started.project else {
            self.warn_unplaced_journal_line(command);
            return Task::none();
        };
        let line = entry.to_line();
        self.queue_journal_line(&project, Journal::Edits, line)
    }

    /// Say that an edit line has no project to go to: it names no document
    /// and more than one project is open (or none).
    fn warn_unplaced_journal_line(&self, command: &AppCommandId) {
        let names: Vec<String> = self
            .document_state
            .projects
            .iter()
            .map(|project| {
                project.path.file_name().map_or_else(
                    || project.path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                )
            })
            .collect();
        let open = match names.as_slice() {
            [] => "no project is open".to_string(),
            _ => format!("{} projects are open: {}", names.len(), names.join(", ")),
        };
        crate::diagnostics::log_warning(format!(
            "Agent journal: `{}` was not recorded: it names no document, and {open}",
            command.as_str()
        ));
    }

    /// Record an access event in the access journal of each project in
    /// `projects`. `build` makes the line from the time and the project's
    /// folder; what it says about other projects is removed.
    pub(crate) fn journal_access_to(
        &mut self,
        projects: &[PathBuf],
        build: impl Fn(String, &Path) -> AccessEntry,
    ) -> Task<Message> {
        if !self.agent.journals_on {
            return Task::none();
        }
        let ts = now();
        let all = self.open_project_files();
        let tasks: Vec<_> = projects
            .iter()
            .filter_map(|project| {
                let folder = project.parent()?;
                let mut entry = build(ts.clone(), folder);
                entry.reason = entry
                    .reason
                    .map(|why| without_other_projects(&why, folder, &all));
                Some((project, entry.to_line()))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|(project, line)| self.queue_journal_line(project, Journal::Access, line))
            .collect();
        Task::batch(tasks)
    }

    /// The quit path (#2253): wait for the journal writes in flight and write
    /// the lines that had not started, then run `next`. A line queued at quit
    /// is on disk before the process goes.
    pub(crate) fn wait_for_journal_then(&mut self, next: Task<Message>) -> Task<Message> {
        let flush = self.agent.journal.take_for_quit();
        if flush.is_idle() {
            return next;
        }
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || flush.run())
                    .await
                    .unwrap_or_else(|error| {
                        vec![format!("the journal flush at quit did not finish: {error}")]
                    })
            },
            |failures| {
                Message::Agent(super::super::AgentMsg::JournalDrained {
                    project: PathBuf::new(),
                    failures,
                })
            },
        )
        .chain(next)
    }

    /// Put `line` on the queue of the project whose file is `project_file`,
    /// with that project's line cap. The session token is replaced wherever it
    /// stands in the encoded line, so no field of either journal can carry it.
    fn queue_journal_line(
        &mut self,
        project_file: &Path,
        journal: Journal,
        line: String,
    ) -> Task<Message> {
        let Some(folder) = project_file.parent() else {
            return Task::none();
        };
        let line = match self.agent.token_text() {
            Some(token) if !token.is_empty() => line.replace(token, REDACTED),
            _ => line,
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
