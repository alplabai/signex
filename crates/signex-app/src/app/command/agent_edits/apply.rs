//! `apply_edits document=.. expect_revision=N edits=[..] [summary=..]`
//! (#2178): an agent's list of edit commands, applied to one sheet as one
//! labelled undo step, all or nothing. See `docs/AGENT_EDITS.md`.

use iced::Task;
use serde_json::json;

use super::wire::{EditSpec, edit_refusal, parse_edit_list};
use crate::app::command::document_operands::named_sheet;
use crate::app::command::operand_handlers::{OperandHandler, operand_handler};
use crate::app::{Message, Signex};
use crate::keymap::{AppCommandId, metadata_for};
use crate::schematic_runtime::RenderInvalidation;

use super::super::{CommandArgs, CommandOutcome, OperandValue};

type Ran = (CommandOutcome, Task<Message>);

/// The longest summary the undo label keeps.
const LABEL_CHARS: usize = 80;

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

/// One edit, checked and ready to run.
struct Prepared {
    id: AppCommandId,
    args: CommandArgs,
    handler: OperandHandler,
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text),
        _ => None,
    }
}

/// Every edit parsed against its descriptor, before any of them runs, so a
/// typo in the last one changes nothing. An edit acts on the call's document,
/// which is filled in where the command takes one; it may name that document
/// again (an id or a path, compared as the resolved document), never another.
fn prepare(
    app: &Signex,
    specs: &[EditSpec],
    document: &str,
    call_path: Option<&std::path::Path>,
) -> Result<Vec<Prepared>, String> {
    let names_the_call = |value: &str| {
        crate::app::document_ids::DocumentTarget::parse(value)
            .ok()
            .and_then(|target| app.resolve_document_target(&target).ok())
            .is_some_and(|entry| entry.path.as_deref() == call_path)
    };
    specs
        .iter()
        .enumerate()
        .map(|(index, spec)| {
            let at = |why: String| format!("edit {} (`{}`): {why}", index + 1, spec.id);
            let id = AppCommandId::new(spec.id.as_str()).map_err(|e| at(e.to_string()))?;
            if let Some(reason) = edit_refusal(&id) {
                return Err(at(reason));
            }
            // A list is one transaction on one sheet.
            if spec
                .operands
                .iter()
                .any(|(name, value)| name == "document" && !names_the_call(value))
            {
                return Err(at(
                    "an edit names another document than the list; leave out its `document`".into(),
                ));
            }
            let takes_document = metadata_for(&id)
                .is_some_and(|meta| meta.operands.iter().any(|op| op.name == "document"));
            let mut texts: Vec<String> = spec
                .operands
                .iter()
                .filter(|(name, _)| name != "document")
                .map(|(name, value)| format!("{name}={value}"))
                .collect();
            if takes_document {
                texts.push(format!("document={document}"));
            }
            let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
            let args = CommandArgs::parse_operands(&id, &refs).map_err(|e| at(e.to_string()))?;
            let handler = operand_handler(&id).ok_or_else(|| at("has no operand form".into()))?;
            Ok(Prepared { id, args, handler })
        })
        .collect()
}

/// The undo label: "Agent: " and the summary, or the ids of the edits.
fn label_for(summary: Option<&str>, specs: &[EditSpec]) -> String {
    let summary = summary.map_or_else(
        || {
            let ids: Vec<&str> = specs.iter().map(|spec| spec.id.as_str()).collect();
            ids.join(", ")
        },
        str::to_string,
    );
    let short: String = summary.chars().take(LABEL_CHARS).collect();
    format!("Agent: {short}")
}

pub(in crate::app::command) fn apply_edits(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match named_sheet(app, args, "`apply_edits`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let Some(OperandValue::Document(target)) = args.operand("document") else {
        return refused("the operand `document` is needed");
    };
    let expected = match text(args, "expect_revision").map(str::parse::<u64>) {
        Some(Ok(revision)) => revision,
        _ => {
            return refused(
                "`expect_revision` must be the whole number `read_document` reports as `revision`",
            );
        }
    };
    let specs = match parse_edit_list(text(args, "edits").unwrap_or_default()) {
        Ok(specs) => specs,
        Err(reason) => return refused(reason),
    };
    let edits = match prepare(app, &specs, &target.to_string(), Some(path.as_path())) {
        Ok(edits) => edits,
        Err(reason) => return refused(reason),
    };
    if let Some(reason) = app.refuse_if_read_only_at(&path) {
        return refused(reason);
    }
    let Some(engine) = app.document_state.engines.get(&path) else {
        return refused(format!("{} is not loaded", path.display()));
    };
    if engine.revision() != expected {
        return refused(format!(
            "the document changed since revision {expected} (it is at {}); read it again and retry. Nothing was applied",
            engine.revision()
        ));
    }
    if edits.is_empty() {
        return (
            CommandOutcome::Answered(
                json!({"changed": false, "revision": engine.revision(), "group": null, "edits": 0, "results": []}),
            ),
            Task::none(),
        );
    }
    let label = label_for(text(args, "summary"), &specs);
    run_all(app, &path, &edits, label)
}

/// Run the edits on `path` inside one engine transaction. What their handlers
/// say in Messages is true only if the list lands, so those Info lines are held
/// and shown after the commit, or dropped with the rollback.
fn run_all(app: &mut Signex, path: &std::path::Path, edits: &[Prepared], label: String) -> Ran {
    crate::diagnostics::hold_info();
    let ran = run_listed(app, path, edits, label);
    crate::diagnostics::release_held_info(matches!(ran.0, CommandOutcome::Answered(_)));
    ran
}

fn run_listed(app: &mut Signex, path: &std::path::Path, edits: &[Prepared], label: String) -> Ran {
    let Some(engine) = app.document_state.engines.get_mut(path) else {
        return refused(format!("{} is not loaded", path.display()));
    };
    if let Err(error) = engine.begin_transaction() {
        return refused(error.to_string());
    }
    app.document_state.agent_edits.target = Some(path.to_path_buf());
    app.document_state.agent_edits.owed = RenderInvalidation::NONE;
    app.document_state.agent_edits.listing = true;
    let mut tasks = Vec::new();
    // What each edit answered, by index; `null` for one that only ran.
    let mut results = Vec::new();
    let mut failure = None;
    for (index, edit) in edits.iter().enumerate() {
        match (edit.handler)(app, &edit.args) {
            (CommandOutcome::Ran, task) => {
                tasks.push(task);
                results.push(serde_json::Value::Null);
            }
            (CommandOutcome::Answered(answer), task) => {
                tasks.push(task);
                results.push(answer);
            }
            (CommandOutcome::Disabled { reason }, _) => {
                failure = Some(format!(
                    "edit {} (`{}`): {reason}",
                    index + 1,
                    edit.id.as_str()
                ));
                break;
            }
            (other, _) => {
                failure = Some(format!(
                    "edit {} (`{}`) did not run: {other:?}",
                    index + 1,
                    edit.id.as_str()
                ));
                break;
            }
        }
    }
    let owed = std::mem::take(&mut app.document_state.agent_edits.owed);
    app.document_state.agent_edits.target = None;
    app.document_state.agent_edits.listing = false;
    app.document_state.agent_edits.project_set.take();
    let Some(engine) = app.document_state.engines.get_mut(path) else {
        return refused(format!("{} is not loaded", path.display()));
    };
    if let Some(reason) = failure {
        engine.rollback_transaction();
        return refused(format!("{reason}. Nothing was applied"));
    }
    let group = app.mint_undo_group();
    let Some(engine) = app.document_state.engines.get_mut(path) else {
        return refused(format!("{} is not loaded", path.display()));
    };
    let committed = match engine.commit_transaction(Some(group)) {
        Ok(result) => result,
        Err(error) => {
            engine.rollback_transaction();
            return refused(format!("{error}. Nothing was applied"));
        }
    };
    // The notices the edits raised were held until now: they are real.
    Signex::report_engine_warnings(engine);
    let revision = engine.revision();
    let count = edits.len();
    let answer = if committed.changed {
        let invalidation = if owed == RenderInvalidation::NONE {
            RenderInvalidation::FULL
        } else {
            owed
        };
        app.document_state
            .agent_edits
            .labels
            .insert(group, label.clone());
        app.after_sheet_change(path, invalidation);
        crate::diagnostics::log_info(format!(
            "{label}: {count} edit(s) applied to {} (undo group {})",
            path.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            group.get()
        ));
        json!({"changed": true, "revision": revision, "group": group.get(), "label": label, "edits": count, "results": results})
    } else {
        json!({"changed": false, "revision": revision, "group": null, "edits": count, "results": results})
    };
    (CommandOutcome::Answered(answer), Task::batch(tasks))
}
