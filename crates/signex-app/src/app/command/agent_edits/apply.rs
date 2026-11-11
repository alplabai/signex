//! `apply_edits document=.. expect_revision=N edits=[..] [summary=..]
//! [expect_revisions={..}]` (#2178): an agent's list of edit commands, applied
//! as one labelled undo step, all or nothing. The list acts on the call's
//! sheet, or on other sheets of the same project that its edits name, and is
//! then one group on all of them (`group_run.rs`). See `docs/AGENT_EDITS.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::json;

use super::group_run::run_group;
use super::wire::{EditSpec, edit_refusal, parse_edit_list, parse_expect_revisions};
use crate::app::command::document_operands::named_sheet;
use crate::app::command::operand_handlers::{OperandHandler, operand_handler};
use crate::app::document::DocumentKey;
use crate::app::document_ids::DocumentTarget;
use crate::app::{Message, Signex};
use crate::keymap::{AppCommandId, metadata_for};

use super::super::{CommandArgs, CommandOutcome, OperandValue};

pub(super) type Ran = (CommandOutcome, Task<Message>);

/// The longest summary the undo label keeps.
const LABEL_CHARS: usize = 80;

pub(super) fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

/// One edit, checked and ready to run on `path`.
pub(super) struct Prepared {
    pub(super) id: AppCommandId,
    pub(super) args: CommandArgs,
    pub(super) handler: OperandHandler,
    pub(super) path: PathBuf,
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text),
        _ => None,
    }
}

/// The file name of `path`, for a message: an answer carries no folder.
pub(super) fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// The loaded sheet `value` (an id or a path) names, and the project file it
/// belongs to.
pub(super) fn sheet_named(app: &Signex, value: &str) -> Result<(PathBuf, Option<PathBuf>), String> {
    let target = DocumentTarget::parse(value).map_err(|e| e.to_string())?;
    let entry = app.resolve_document_target(&target)?;
    match entry.key {
        Some(DocumentKey::Schematic(path)) => Ok((path, entry.project.map(|p| p.path))),
        Some(_) => Err(format!(
            "{} is a {}, not a sheet",
            entry.name,
            entry.kind.as_str()
        )),
        None => Err(format!(
            "{} is referenced by a project but not loaded; open it first",
            entry.name
        )),
    }
}

/// Every edit parsed against its descriptor, before any of them runs, so a
/// typo in the last one changes nothing. An edit acts on the call's document,
/// which is filled in where the command takes one, or on the sheet its own
/// `document` names: one of the same project as the call's, never another.
fn prepare(
    app: &Signex,
    specs: &[EditSpec],
    document: &str,
    call_path: &Path,
    call_project: Option<&Path>,
) -> Result<Vec<Prepared>, String> {
    specs
        .iter()
        .enumerate()
        .map(|(index, spec)| {
            let at = |why: String| format!("edit {} (`{}`): {why}", index + 1, spec.id);
            let id = AppCommandId::new(spec.id.as_str()).map_err(|e| at(e.to_string()))?;
            if let Some(reason) = edit_refusal(&id) {
                return Err(at(reason));
            }
            let named = spec
                .operands
                .iter()
                .find(|(name, _)| name == "document")
                .map(|(_, value)| value.as_str());
            let (path, document) = match named {
                None => (call_path.to_path_buf(), document),
                Some(value) => {
                    let (path, project) = sheet_named(app, value).map_err(&at)?;
                    if path != call_path
                        && (project.is_none() || project.as_deref() != call_project)
                    {
                        return Err(at(
                            "names a sheet of another project than the list's document; \
                             one call edits one project"
                                .into(),
                        ));
                    }
                    (path, value)
                }
            };
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
            Ok(Prepared {
                id,
                args,
                handler,
                path,
            })
        })
        .collect()
}

/// The project file of the call's document, if it is in a project.
fn project_of(app: &Signex, target: &DocumentTarget) -> Option<PathBuf> {
    app.resolve_document_target(target)
        .ok()
        .and_then(|entry| entry.project)
        .map(|project| project.path)
}

/// The edits of an `apply_edits` call, parsed as the call parses them, each
/// with the sheet it acts on, for planning their reads (#2199); `None` when
/// the call is refused anyway (it then runs at once and says why).
pub(in crate::app::command) fn listed_edits(
    app: &Signex,
    args: &CommandArgs,
) -> Option<Vec<(AppCommandId, CommandArgs, PathBuf)>> {
    let path = named_sheet(app, args, "`apply_edits`").ok()?;
    let Some(OperandValue::Document(target)) = args.operand("document") else {
        return None;
    };
    let specs = parse_edit_list(text(args, "edits").unwrap_or_default()).ok()?;
    let project = project_of(app, target);
    let edits = prepare(app, &specs, &target.to_string(), &path, project.as_deref()).ok()?;
    Some(edits.into_iter().map(|e| (e.id, e.args, e.path)).collect())
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

/// The revision each sheet of the call must be at: `expect_revision` for the
/// call's own sheet, `expect_revisions` for the others. Every sheet the edits
/// touch needs one, and none may name a sheet no edit touches (a typo would
/// otherwise go unnoticed).
fn expected_revisions(
    app: &Signex,
    args: &CommandArgs,
    call_path: &Path,
    call_revision: u64,
    sheets: &[PathBuf],
) -> Result<HashMap<PathBuf, u64>, String> {
    let mut expected = HashMap::from([(call_path.to_path_buf(), call_revision)]);
    if let Some(list) = text(args, "expect_revisions") {
        for (document, revision) in parse_expect_revisions(list)? {
            let (path, _) = sheet_named(app, &document)
                .map_err(|why| format!("`expect_revisions` key `{document}`: {why}"))?;
            if path == call_path {
                return Err(format!(
                    "`expect_revisions` names {}, the call's own document: its revision is `expect_revision`",
                    file_name(&path)
                ));
            }
            if !sheets.contains(&path) {
                return Err(format!(
                    "`expect_revisions` names {}, which no edit touches",
                    file_name(&path)
                ));
            }
            if expected.insert(path.clone(), revision).is_some() {
                return Err(format!(
                    "`expect_revisions` names {} twice",
                    file_name(&path)
                ));
            }
        }
    }
    if let Some(missing) = sheets.iter().find(|path| !expected.contains_key(*path)) {
        return Err(format!(
            "an edit names {}: `expect_revisions` must hold the revision `read_document` reports for it",
            file_name(missing)
        ));
    }
    Ok(expected)
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
    let project = project_of(app, target);
    let edits = match prepare(app, &specs, &target.to_string(), &path, project.as_deref()) {
        Ok(edits) => edits,
        Err(reason) => return refused(reason),
    };
    // The sheets the edits touch, in order of first use; an empty list touches
    // the call's own.
    let mut sheets: Vec<PathBuf> = Vec::new();
    for edit in &edits {
        if !sheets.contains(&edit.path) {
            sheets.push(edit.path.clone());
        }
    }
    if sheets.is_empty() {
        sheets.push(path.clone());
    }
    let expected = match expected_revisions(app, args, &path, expected, &sheets) {
        Ok(expected) => expected,
        Err(reason) => return refused(reason),
    };
    // The call's own sheet is always checked; the others when an edit touches them.
    let mut checked = vec![path.clone()];
    checked.extend(sheets.iter().filter(|sheet| **sheet != path).cloned());
    for sheet in &checked {
        if let Some(reason) = app.refuse_if_read_only_at(sheet) {
            return refused(reason);
        }
        let Some(engine) = app.document_state.engines.get(sheet) else {
            return refused(format!("{} is not loaded", sheet.display()));
        };
        if engine.revision() != expected[sheet] {
            let which = if *sheet == path {
                String::new()
            } else {
                format!("{}: ", file_name(sheet))
            };
            return refused(format!(
                "{which}the document changed since revision {} (it is at {}); read it again and retry. Nothing was applied",
                expected[sheet],
                engine.revision()
            ));
        }
    }
    if edits.is_empty() {
        let revision = app
            .document_state
            .engines
            .get(&path)
            .map_or(0, signex_engine::Engine::revision);
        return (
            CommandOutcome::Answered(
                json!({"changed": false, "revision": revision, "group": null, "edits": 0, "results": []}),
            ),
            Task::none(),
        );
    }
    let label = label_for(text(args, "summary"), &specs);
    run_group(app, &path, &sheets, &edits, label)
}
