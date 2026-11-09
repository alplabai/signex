//! One headless run (#1923): the booted app, the policy, and one result line
//! per command. Everything reaches the app through `signex_app::headless`
//! and the app's own task driver.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_app::app::Signex;
use signex_app::diagnostics;
use signex_app::headless::{
    ChangeSnapshot as Snapshot, CommandArgs, OperandValue, changed_between, edit_list_text,
    messages_since, outcome_verdict as verdict, parse_edit_list,
};
use signex_app::io_ports::HeadlessAnswers;
use signex_app::keymap::OperandKind;
use signex_app::task_driver::TaskDriver;
use signex_types::project::{ErcRules, RuleProfile};

use crate::capability::capability_of;
use crate::confine::canonical;
use crate::policy::{Approved, Policy};
use crate::protocol::{Request, ResultLine, Status};
use crate::roots::project_root;

/// How much of a command line the audit log keeps.
const AUDIT_LINE_CHARS: usize = 400;

pub struct Session {
    app: Signex,
    driver: TaskDriver,
    policy: Policy,
    run_id: String,
    project_dir: PathBuf,
    /// The project's folder, fixed at the start: nothing the project file
    /// lists later widens it.
    roots: Vec<PathBuf>,
}

impl Session {
    /// Boot the app with `project` open (a `.snxprj`, or a loose sheet), with
    /// an explicit ERC profile, and wait until it is loaded.
    pub fn start(project: PathBuf, policy: Policy, run_id: String) -> Result<Self, String> {
        let project_dir = project
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| format!("{} has no folder", project.display()))?;
        let home = dirs::home_dir().and_then(|dir| canonical(&dir).ok());
        let roots = project_root(&project_dir, home.as_deref())?;
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
        let windows_before = self.driver.dropped_window_actions();
        let dialogs_before = self.dialog_refusals();
        let (status, reason, data) = self.run(request);
        let windows_dropped = self.driver.dropped_window_actions() > windows_before;
        let dialog_refused = self.dialog_refusals() > dialogs_before;
        let messages = messages_since(log_mark);
        let (status, reason) =
            honest_status(status, reason, &request.id, windows_dropped, dialog_refused);
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
        let mut approved = match self
            .policy
            .authorize(request, &self.project_dir, &self.roots)
        {
            Ok(approved) => approved,
            Err(reason) => return (Status::Refused, Some(reason), None),
        };
        if let Err(reason) = self.authorize_edits(&mut approved) {
            return (Status::Refused, Some(reason), None);
        }
        let (args, answers) = match operands_for(&approved) {
            Ok(split) => split,
            Err(reason) => return (Status::Refused, Some(reason), None),
        };
        // The last check before the app gets the paths.
        if let Err(reason) = self.policy.recheck(&approved, &self.roots) {
            return (Status::Refused, Some(reason), None);
        }
        // An id names its file only through the app, so every `document`
        // operand, id or path, is resolved and checked as the file it names.
        let named = match self.named_document_paths(&approved) {
            Ok(named) => named,
            Err(reason) => return (Status::Refused, Some(reason), None),
        };
        // A command that writes also writes files no operand names. One that
        // names its document writes that document (not whatever is active)
        // and what saving it writes besides: its project file, the sheets a
        // Save As rewrites, the libraries a project save creates.
        if capability_of(approved.meta).writes_files() {
            let targets = if named.is_empty() {
                self.implicit_write_targets()
            } else {
                if let Err(reason) = self.check_side_writes(approved.command.as_str(), &named) {
                    return (Status::Refused, Some(reason), None);
                }
                named
            };
            if let Err(reason) = self.policy.check_write_targets(&targets, &self.roots) {
                return (Status::Refused, Some(reason), None);
            }
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

    /// The edits of an `apply_edits` call, each authorized as a command of its
    /// own (not `gui_only`, the `edit` capability held, every path operand
    /// confined, every `document` resolved and checked), then written back
    /// with the checked paths. Any other command has no edits.
    fn authorize_edits(&mut self, approved: &mut Approved) -> Result<(), String> {
        if approved.command.as_str() != "apply_edits" {
            return Ok(());
        }
        let Some(at) = approved
            .operands
            .iter()
            .position(|(name, _)| name == "edits")
        else {
            return Ok(());
        };
        let mut edits = parse_edit_list(&approved.operands[at].1)?;
        for (index, edit) in edits.iter_mut().enumerate() {
            let why = |reason: String| format!("edit {} (`{}`): {reason}", index + 1, edit.id);
            let request = Request {
                id: edit.id.clone(),
                operands: edit.operands.clone(),
            };
            let checked = self
                .policy
                .authorize(&request, &self.project_dir, &self.roots)
                .map_err(&why)?;
            self.policy.recheck(&checked, &self.roots).map_err(&why)?;
            self.named_document_paths(&checked).map_err(&why)?;
            edit.operands = checked.operands;
        }
        approved.operands[at].1 = edit_list_text(&edits);
        Ok(())
    }

    /// The files the `document` operands of `approved` name, resolved by the
    /// app and each checked like a path operand (inside the allowed folders,
    /// no link out, within the size cap). An operand that names no document
    /// is refused with the app's reason.
    fn named_document_paths(&mut self, approved: &Approved) -> Result<Vec<PathBuf>, String> {
        let writes = capability_of(approved.meta).writes_files();
        let mut paths = Vec::new();
        for (name, text) in &approved.operands {
            let is_document = approved
                .meta
                .operands
                .iter()
                .any(|operand| operand.name == *name && operand.kind == OperandKind::Document);
            if !is_document {
                continue;
            }
            let path = self
                .app
                .document_path(text)
                .map_err(|why| format!("operand `{name}`: {why}"))?;
            self.policy
                .check_resolved(&path, &self.roots, writes)
                .map_err(|why| format!("operand `{name}`: {why}"))?;
            paths.push(path);
        }
        Ok(paths)
    }

    /// The files saving the `named` documents writes besides them, each
    /// checked as a write target; a refusal names the file and its part.
    fn check_side_writes(&self, command: &str, named: &[PathBuf]) -> Result<(), String> {
        let as_copy = command == "save_document_as";
        for side in named
            .iter()
            .flat_map(|path| self.app.save_side_writes(path, as_copy))
        {
            let part = match side.extension().and_then(|e| e.to_str()) {
                Some("snxprj") => "project file",
                Some("snxsch") => "project sheet",
                _ => "library file",
            };
            self.policy
                .check_write_targets(std::slice::from_ref(&side), &self.roots)
                .map_err(|why| {
                    format!("{command} also writes the {part} {}: {why}", side.display())
                })?;
        }
        Ok(())
    }

    /// File dialogs refused so far in this session.
    fn dialog_refusals(&self) -> u64 {
        self.app
            .headless_answers()
            .map_or(0, HeadlessAnswers::refusals)
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
        self.app.change_snapshot()
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
    // A command that names its document takes its operands whole: `path` is
    // then the target of `save_document_as`, not a dialog answer.
    let names_document = approved
        .meta
        .operands
        .iter()
        .any(|operand| operand.kind == OperandKind::Document);
    let is_dialog = !names_document
        && approved
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

/// A command whose only effect was a window the task driver dropped, or a
/// file dialog the answers could not satisfy, still reports `Ran`. Both are
/// counted by the app (`TaskDriver::dropped_window_actions`,
/// `HeadlessAnswers::refusals`), so the decision never reads log text.
fn honest_status(
    status: Status,
    reason: Option<String>,
    id: &str,
    windows_dropped: bool,
    dialog_refused: bool,
) -> (Status, Option<String>) {
    match (status, windows_dropped, dialog_refused) {
        (Status::Ran, true, _) => (
            Status::Refused,
            Some(format!(
                "`{id}` needs a window, which a headless run does not have"
            )),
        ),
        (Status::Ran, false, true) => (
            Status::Refused,
            Some(
                "a file dialog was refused: it had no usable answer, or would replace a file \
                 without overwrite=true (see messages)"
                    .into(),
            ),
        ),
        (status, _, _) => (status, reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ran_command_with_a_dropped_window_or_a_refused_dialog_is_refused_and_nothing_else_changes()
    {
        let honest = |status, windows, dialogs| {
            honest_status(status, Some("why".into()), "x", windows, dialogs)
        };
        let (status, reason) = honest(Status::Ran, true, false);
        assert_eq!(status, Status::Refused);
        assert_eq!(
            reason.as_deref(),
            Some("`x` needs a window, which a headless run does not have")
        );
        let (status, reason) = honest(Status::Ran, false, true);
        assert_eq!(status, Status::Refused);
        assert!(reason.unwrap().contains("a file dialog was refused"));
        // A window wins over a dialog; neither signal leaves `ran` alone.
        assert!(
            honest(Status::Ran, true, true)
                .1
                .unwrap()
                .contains("needs a window")
        );
        assert_eq!(
            honest(Status::Ran, false, false),
            (Status::Ran, Some("why".into()))
        );
        // A command that was already refused or failed keeps its own reason.
        for status in [Status::Refused, Status::Failed] {
            assert_eq!(honest(status, true, true), (status, Some("why".into())));
        }
    }
}
