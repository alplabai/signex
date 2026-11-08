//! One headless run (#1923): the booted app, the policy, and one result line
//! per command. Everything reaches the app through `signex_app::headless`
//! and the app's own task driver.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_app::app::Signex;
use signex_app::diagnostics;
use signex_app::headless::{CommandArgs, CommandOutcome, OperandValue};
use signex_app::keymap::OperandKind;
use signex_app::task_driver::TaskDriver;
use signex_types::project::{ErcRules, RuleProfile};

use crate::capability::capability_of;
use crate::confine::canonical;
use crate::policy::{Approved, Policy};
use crate::protocol::{MessageLine, Request, ResultLine, Status};
use crate::roots::allowed_roots;

/// How much of a command line the audit log keeps.
const AUDIT_LINE_CHARS: usize = 400;

pub struct Session {
    app: Signex,
    driver: TaskDriver,
    policy: Policy,
    run_id: String,
    project_dir: PathBuf,
    /// Fixed at the start: what the project file lists later never widens it.
    roots: Vec<PathBuf>,
}

/// The state a command can change, per document: engine revision and unsaved
/// flag. A document that is gone or new differs too.
type Snapshot = BTreeMap<PathBuf, (u64, bool)>;

impl Session {
    /// Boot the app with `project` open (a `.snxprj`, or a loose sheet), with
    /// an explicit ERC profile, and wait until it is loaded.
    pub fn start(project: PathBuf, policy: Policy, run_id: String) -> Result<Self, String> {
        let project_dir = project
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| format!("{} has no folder", project.display()))?;
        let rules = RuleProfile::new("Signex headless default", "1", ErcRules::default());
        let (mut app, boot) = Signex::boot_headless(vec![project.clone()], &rules)
            .map_err(|error| error.to_string())?;
        let mut driver = TaskDriver::new();
        driver
            .settle(&mut app, boot)
            .map_err(|error| format!("opening {}: {error}", project.display()))?;
        if app.document_state.projects.is_empty() && app.document_state.engines.is_empty() {
            let detail = app
                .document_state
                .error_notice
                .take()
                .map_or_else(|| "nothing was loaded".to_string(), |n| n.detail);
            return Err(format!("could not open {}: {detail}", project.display()));
        }
        diagnostics::log_info(format!(
            "[run {run_id}] opened {}; ERC rules: {} v{}",
            project.display(),
            rules.name,
            rules.version
        ));
        let listed: Vec<PathBuf> = app
            .document_state
            .projects
            .iter()
            .flat_map(|loaded| &loaded.data.libraries)
            .map(|entry| entry.path.clone())
            .collect();
        let mounted: Vec<PathBuf> = app
            .library
            .open_libraries
            .iter()
            .map(|library| library.root.clone())
            .collect();
        let home = dirs::home_dir().and_then(|dir| canonical(&dir).ok());
        let roots = allowed_roots(&project_dir, &listed, &mounted, home.as_deref());
        Ok(Self {
            app,
            driver,
            policy,
            run_id,
            project_dir,
            roots,
        })
    }

    /// A line that is not a command (too long, not UTF-8, malformed, over the
    /// command limit): refused without the app being asked.
    pub fn reject(&self, seq: u64, raw: &str, reason: String) -> ResultLine {
        self.audit(seq, raw, Status::Refused, Some(&reason));
        ResultLine {
            reason: Some(reason),
            ..self.blank(seq, Status::Refused)
        }
    }

    /// Run one command and report what happened.
    pub fn execute(&mut self, seq: u64, request: &Request, raw: &str) -> ResultLine {
        let log_mark = diagnostics::newest_entry_id();
        let before = self.snapshot();
        let (status, reason, data) = self.run(request);
        let messages: Vec<MessageLine> = diagnostics::entries_since(log_mark)
            .entries
            .into_iter()
            .map(|entry| MessageLine {
                level: entry.level.label(),
                message: entry.message,
            })
            .collect();
        // A dialog the answers could not satisfy still reports `Ran`; its
        // refusal is only in the log.
        let (status, reason) = match (status, dialog_refusal(&messages)) {
            (Status::Ran, Some(why)) => (Status::Refused, Some(why)),
            (status, _) => (status, reason),
        };
        let changed = changed_between(&before, &self.snapshot());
        self.audit(seq, raw, status, reason.as_deref());
        ResultLine {
            id: Some(request.id.clone()),
            reason,
            data,
            messages,
            changed,
            ..self.blank(seq, status)
        }
    }

    /// End the session: flush, exit, and name the documents with changes
    /// that were not saved (nothing is saved at exit).
    pub fn finish(mut self) -> Result<Vec<PathBuf>, String> {
        let mut unsaved: Vec<PathBuf> = self
            .app
            .document_state
            .dirty_document_paths()
            .into_iter()
            .collect();
        unsaved.sort();
        let end = self.app.end_headless_session();
        self.driver
            .settle(&mut self.app, end)
            .map_err(|error| format!("ending the session: {error}"))?;
        Ok(unsaved)
    }

    fn run(&mut self, request: &Request) -> (Status, Option<String>, Option<Value>) {
        let approved = match self
            .policy
            .authorize(request, &self.project_dir, &self.roots)
        {
            Ok(approved) => approved,
            Err(reason) => return (Status::Refused, Some(reason), None),
        };
        let (args, answers) = match operands_for(&approved) {
            Ok(split) => split,
            Err(reason) => return (Status::Refused, Some(reason), None),
        };
        // The last check before the app gets the paths.
        if let Err(reason) = self.policy.recheck(&approved, &self.roots) {
            return (Status::Refused, Some(reason), None);
        }
        // A command that writes also writes files no operand names.
        if capability_of(approved.meta).writes_files()
            && let Err(reason) = self
                .policy
                .check_write_targets(&self.implicit_write_targets(), &self.roots)
        {
            return (Status::Refused, Some(reason), None);
        }
        let Some(dialogs) = self.app.headless_answers() else {
            return (Status::Failed, Some("the app is not headless".into()), None);
        };
        dialogs.replace_all(answers);
        let (outcome, task) = self.app.dispatch_command_headless(&approved.command, args);
        let settled = self.driver.settle(&mut self.app, task);
        let notice = self.app.document_state.error_notice.take();
        if let Err(error) = settled {
            return (Status::Failed, Some(error.to_string()), None);
        }
        verdict(
            approved.command.as_str(),
            outcome,
            notice.map(|n| format!("{}: {}", n.title, n.detail)),
        )
    }

    /// Where a save can land without an operand: the active document, every
    /// document with unsaved changes, and each open project's file.
    fn implicit_write_targets(&self) -> Vec<PathBuf> {
        let state = &self.app.document_state;
        let targets: BTreeSet<PathBuf> = state
            .active_path
            .iter()
            .cloned()
            .chain(state.dirty_document_paths())
            .chain(state.projects.iter().map(|project| project.path.clone()))
            .collect();
        targets.into_iter().collect()
    }

    fn snapshot(&self) -> Snapshot {
        let state = &self.app.document_state;
        let dirty = state.dirty_document_paths();
        let mut all: Snapshot = state
            .engines
            .iter()
            .map(|(path, engine)| (path.clone(), (engine.revision(), dirty.contains(path))))
            .collect();
        for path in dirty {
            all.entry(path).or_insert((0, true));
        }
        all
    }

    fn audit(&self, seq: u64, raw: &str, status: Status, reason: Option<&str>) {
        let line: String = raw.chars().take(AUDIT_LINE_CHARS).collect();
        let why = reason.map(|r| format!(": {r}")).unwrap_or_default();
        diagnostics::log_info(format!(
            "[run {}] #{seq} {line} -> {status:?}{why}",
            self.run_id
        ));
    }

    /// A result line with nothing in it but its place in the run.
    fn blank(&self, seq: u64, status: Status) -> ResultLine {
        ResultLine {
            run: self.run_id.clone(),
            seq,
            id: None,
            status,
            reason: None,
            data: None,
            messages: Vec::new(),
            changed: Vec::new(),
        }
    }
}

/// The operands of an approved command, read against its catalog descriptor,
/// split into what is dispatched and the dialog answers. A command with a
/// required path (open, save as, new) is a dialog in the GUI, so its path and
/// `overwrite` go to the dialog and the command runs with none.
fn operands_for(approved: &Approved) -> Result<(CommandArgs, Vec<(String, String)>), String> {
    let texts: Vec<String> = approved
        .operands
        .iter()
        .map(|(name, text)| format!("{name}={text}"))
        .collect();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&approved.command, &refs).map_err(|e| e.to_string())?;
    let is_dialog = approved
        .meta
        .operands
        .iter()
        .any(|operand| operand.required && operand.kind == OperandKind::Path);
    if !is_dialog {
        return Ok((args, Vec::new()));
    }
    let mut answers = Vec::new();
    if let Some(OperandValue::Path(path)) = args.operand("path") {
        answers.push(("path".to_string(), path.clone()));
    }
    if let Some(OperandValue::Bool(replace)) = args.operand("overwrite") {
        answers.push(("overwrite".to_string(), replace.to_string()));
    }
    Ok((CommandArgs::none(), answers))
}

/// What a dispatch outcome means for the result line. A `Ran` that left an
/// error notice failed; nothing unknown is ever reported as ran.
fn verdict(
    id: &str,
    outcome: CommandOutcome,
    notice: Option<String>,
) -> (Status, Option<String>, Option<Value>) {
    match outcome {
        CommandOutcome::Ran => match notice {
            Some(why) => (Status::Failed, Some(why), None),
            None => (Status::Ran, None, None),
        },
        CommandOutcome::UnknownId => (
            Status::Refused,
            Some(format!("unknown command `{id}`")),
            None,
        ),
        CommandOutcome::NoBridgeArm => (
            Status::Refused,
            Some(format!(
                "not runnable headless: `{id}` has no dispatch arm yet"
            )),
            None,
        ),
        CommandOutcome::Disabled { reason } => (Status::Refused, Some(reason), None),
        other => (
            Status::Failed,
            Some(format!("unexpected outcome {other:?}")),
            None,
        ),
    }
}

fn changed_between(before: &Snapshot, after: &Snapshot) -> Vec<String> {
    let paths: BTreeSet<&PathBuf> = before.keys().chain(after.keys()).collect();
    paths
        .into_iter()
        .filter(|path| before.get(*path) != after.get(*path))
        .map(|path| path.display().to_string())
        .collect()
}

/// What the app's file dialog logs when a headless answer is missing, not
/// absolute, or would replace a file without `overwrite=true`
/// (`io_ports::picker`). Pinned by the end-to-end test.
const DIALOG_REFUSED: &str = "refused in a headless run";

/// The first message that says a dialog was refused.
fn dialog_refusal(messages: &[MessageLine]) -> Option<String> {
    messages
        .iter()
        .find(|line| line.message.contains(DIALOG_REFUSED))
        .map(|line| line.message.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(entries: &[(&str, u64, bool)]) -> Snapshot {
        entries
            .iter()
            .map(|(path, rev, dirty)| (PathBuf::from(path), (*rev, *dirty)))
            .collect()
    }

    #[test]
    fn a_changed_revision_a_flipped_flag_a_new_or_a_gone_document_is_changed() {
        let before = snap(&[
            ("a", 1, false),
            ("b", 1, false),
            ("c", 1, false),
            ("d", 1, true),
        ]);
        let after = snap(&[
            ("a", 2, false),
            ("b", 1, true),
            ("c", 1, false),
            ("e", 1, false),
        ]);
        assert_eq!(changed_between(&before, &after), ["a", "b", "d", "e"]);
        assert!(changed_between(&before, &before).is_empty());
    }

    #[test]
    fn a_refused_dialog_in_the_log_is_found_and_other_messages_are_not() {
        let line = |message: &str| MessageLine {
            level: "WARN",
            message: message.to_string(),
        };
        let quiet = [line("Opened a.snxsch"), line("Skipped 1 locked object(s)")];
        assert_eq!(dialog_refusal(&quiet), None);
        let refused = [
            line("fine"),
            line("File dialog: refused in a headless run; give the `path` argument"),
        ];
        assert_eq!(
            dialog_refusal(&refused).as_deref(),
            Some("File dialog: refused in a headless run; give the `path` argument")
        );
    }

    #[test]
    fn an_error_notice_makes_a_ran_command_failed_and_unknown_outcomes_never_ran() {
        let ran = verdict("x", CommandOutcome::Ran, None);
        assert_eq!(ran.0, Status::Ran);
        let failed = verdict(
            "x",
            CommandOutcome::Ran,
            Some("Open failed: no file".into()),
        );
        assert_eq!(
            (failed.0, failed.1.as_deref()),
            (Status::Failed, Some("Open failed: no file"))
        );
        let unknown = verdict("x", CommandOutcome::UnknownId, None);
        assert_eq!(unknown.0, Status::Refused);
        let arm = verdict("x", CommandOutcome::NoBridgeArm, None);
        assert!(arm.1.unwrap().contains("no dispatch arm"));
        let disabled = verdict(
            "x",
            CommandOutcome::Disabled {
                reason: "why".into(),
            },
            None,
        );
        assert_eq!(
            (disabled.0, disabled.1.as_deref()),
            (Status::Refused, Some("why"))
        );
        let not_headless = verdict("x", CommandOutcome::NotHeadless, None);
        assert_eq!(not_headless.0, Status::Failed);
    }
}
