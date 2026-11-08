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
/// typo in the last one changes nothing. An edit never names its document: it
/// acts on the call's, which is filled in where the command takes one.
fn prepare(specs: &[EditSpec], document: &str) -> Result<Vec<Prepared>, String> {
    specs
        .iter()
        .enumerate()
        .map(|(index, spec)| {
            let at = |why: String| format!("edit {} (`{}`): {why}", index + 1, spec.id);
            let id = AppCommandId::new(spec.id.as_str()).map_err(|e| at(e.to_string()))?;
            if let Some(reason) = edit_refusal(&id) {
                return Err(at(reason));
            }
            if spec.operands.iter().any(|(name, _)| name == "document") {
                return Err(at(
                    "an edit acts on the call's document; leave out its `document`".into(),
                ));
            }
            let takes_document = metadata_for(&id)
                .is_some_and(|meta| meta.operands.iter().any(|op| op.name == "document"));
            let mut texts: Vec<String> = spec
                .operands
                .iter()
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
    let edits = match prepare(&specs, &target.to_string()) {
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
                json!({"changed": false, "revision": engine.revision(), "group": null, "edits": 0}),
            ),
            Task::none(),
        );
    }
    let label = label_for(text(args, "summary"), &specs);
    run_all(app, &path, &edits, label)
}

/// Run the edits on `path` inside one engine transaction.
fn run_all(app: &mut Signex, path: &std::path::Path, edits: &[Prepared], label: String) -> Ran {
    let Some(engine) = app.document_state.engines.get_mut(path) else {
        return refused(format!("{} is not loaded", path.display()));
    };
    if let Err(error) = engine.begin_transaction() {
        return refused(error.to_string());
    }
    app.document_state.agent_edits.target = Some(path.to_path_buf());
    app.document_state.agent_edits.owed = RenderInvalidation::NONE;
    let mut tasks = Vec::new();
    let mut failure = None;
    for (index, edit) in edits.iter().enumerate() {
        match (edit.handler)(app, &edit.args) {
            (CommandOutcome::Ran, task) => tasks.push(task),
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
        json!({"changed": true, "revision": revision, "group": group.get(), "label": label, "edits": count})
    } else {
        json!({"changed": false, "revision": revision, "group": null, "edits": count})
    };
    (CommandOutcome::Answered(answer), Task::batch(tasks))
}
