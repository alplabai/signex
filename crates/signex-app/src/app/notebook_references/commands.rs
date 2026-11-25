//! The catalog commands of notebook references (mockup rows 196 and 197):
//! `notebook_references`, `notebook_reference_card`,
//! `notebook_reference_picker`, `notebook_badges` and
//! `notebook_fix_reference`. Each names the project by `document`, reads
//! the project's notebook and design off the update thread first
//! (`Deferred`), then answers from that read as the caller that sent it.
//! Shapes: `docs/NOTEBOOK_ANNOTATIONS.md`, "Commands".

use std::sync::Arc;

use iced::Task;
use serde_json::{Value, json};
use signex_notebook::{
    BadgeTarget, DEFAULT_NOTEBOOK_FILE, DesignIndex, Fix, PICKER_LIMIT, Reference,
    ResolvedReference, badge_of, picker_entries, plan_update_all, reference_card, resolve_one,
};

use super::{FileReferences, NotebookRerun, of_text};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::panels::annotations::NotebookScan;

type Ran = (CommandOutcome, Task<Message>);

/// A refusal with its reason: the operand handlers' one helper.
fn refused(reason: impl Into<String>) -> Ran {
    crate::app::command::operand_refused(reason)
}

fn answered(value: Value) -> Ran {
    (CommandOutcome::Answered(value), Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// What a command answers from: the read of the project it named.
struct Read {
    scan: Arc<NotebookScan>,
    design: Arc<DesignIndex>,
}

/// The read of the project `args` names. The first run reads it and waits
/// (`Err` holds the deferral); the run again after the read answers from
/// it.
fn read_first(app: &mut Signex, args: &CommandArgs, id: &str) -> Result<Read, Ran> {
    let project = app.rules_project_named(args).map_err(refused)?;
    let state = &app.ui_state.annotations;
    if state.answering {
        if state.target.as_ref().map(|t| t.0) != Some(project) {
            return Err(refused(
                "the notebook read moved to another project before it landed; ask again",
            ));
        }
        let Some(scan) = state.scan.clone() else {
            return Err(refused("the project's notebook could not be read"));
        };
        if !scan.folder_found {
            return Err(refused(
                "the project has no Design Notebook (no notebook/ folder)",
            ));
        }
        let Some(design) = scan.design.clone() else {
            return Err(refused(
                "the project's design could not be read whole, so no reference is checked",
            ));
        };
        return Ok(Read { scan, design });
    }
    let Some(loaded) = app.document_state.project_by_id(project) else {
        return Err(refused("the project is no longer open"));
    };
    let (dir, file) = (loaded.dir().to_path_buf(), loaded.path.clone());
    let command = AppCommandId::new(id).map_err(refused)?;
    let edits = &mut app.document_state.agent_edits;
    edits.tickets += 1;
    let ticket = edits.tickets;
    let rerun = NotebookRerun {
        ticket,
        command,
        args: args.clone(),
        project: file,
        origin: app.agent.origin.clone(),
        serving: app.agent.serving,
    };
    let task = app.start_annotations_scan(Some((project, dir)));
    app.ui_state.annotations.reruns.push(rerun);
    Err((CommandOutcome::Deferred { ticket }, task))
}

/// The references of every notebook file, the notebook tab's text in
/// place of the file where it holds one.
fn current_files(app: &Signex, read: &Read) -> Vec<FileReferences> {
    read.scan
        .references
        .iter()
        .map(|file| match app.notebook_buffer(&file.name) {
            Some(text) => of_text(&file.path, &text, &read.design),
            None => file.clone(),
        })
        .collect()
}

/// The reference `kind` and `name` name: `component R7`, `pin U3.3`,
/// `net +3V3`.
pub(crate) fn reference_operand(args: &CommandArgs) -> Result<Reference, String> {
    let kind = text(args, "kind").map(str::trim).unwrap_or_default();
    let Some(name) = text(args, "name").map(str::trim).filter(|n| !n.is_empty()) else {
        return Err("needs the operand `name`: a designator, a pin (`U3.3`) or a net".into());
    };
    match kind {
        "component" => Ok(Reference::Component(name.to_string())),
        "net" => Ok(Reference::Net(name.to_string())),
        "pin" => Reference::dotted_pin(name)
            .ok_or_else(|| format!("`{name}` is not a pin: write it as `U3.3`")),
        other => Err(format!(
            "`kind` is `component`, `pin` or `net`, not `{other}`"
        )),
    }
}

fn file_json(file: &FileReferences) -> Value {
    let references: Vec<Value> = file
        .report
        .references
        .iter()
        .map(|entry| {
            let mut value = serde_json::to_value(entry).unwrap_or(Value::Null);
            if let Value::Object(map) = &mut value {
                map.insert("uses".into(), json!(entry.uses()));
                map.insert("fixes".into(), json!(entry.offers()));
            }
            value
        })
        .collect();
    json!({
        "file": file.name,
        "references": references,
        "total": file.report.total_uses(),
        "stale": file.report.stale_count(),
        "header_unread": file.header_unread,
        "update_all": plan_update_all(&file.report),
    })
}

/// `notebook_references {document, file?}`: every reference of every
/// notebook file (or of `file`), where each stands, its fixes, and what
/// Update all would change.
pub(crate) fn notebook_references(app: &mut Signex, args: &CommandArgs) -> Ran {
    let read = match read_first(app, args, "notebook_references") {
        Ok(read) => read,
        Err(waiting) => return waiting,
    };
    let only = text(args, "file").map(str::trim);
    let files: Vec<Value> = current_files(app, &read)
        .iter()
        .filter(|file| only.is_none_or(|name| file.name == name))
        .map(file_json)
        .collect();
    answered(json!({ "files": files }))
}

/// The entry of `reference` in `file` (else the first file that uses it,
/// else the notebook's main file), resolved on its own when no file uses
/// it.
fn entry_of(
    files: &[FileReferences],
    file: Option<&str>,
    reference: &Reference,
    design: &DesignIndex,
) -> (ResolvedReference, usize) {
    let in_file = |f: &&FileReferences| file.is_none_or(|name| f.name == name);
    let found = files
        .iter()
        .filter(in_file)
        .find_map(|f| Some((f.report.get(reference)?.clone(), f.report.stale_count())));
    found.unwrap_or_else(|| {
        let (state, id) = resolve_one(reference, None, design);
        let stale = files
            .iter()
            .filter(in_file)
            .find(|f| f.name == file.unwrap_or(DEFAULT_NOTEBOOK_FILE))
            .map_or(0, |f| f.report.stale_count());
        let entry = ResolvedReference {
            reference: reference.clone(),
            state,
            id,
            lines: Vec::new(),
        };
        (entry, stale)
    })
}

/// `notebook_reference_card {document, kind, name, file?}`: the hover card
/// of one reference.
pub(crate) fn notebook_reference_card(app: &mut Signex, args: &CommandArgs) -> Ran {
    let reference = match reference_operand(args) {
        Ok(reference) => reference,
        Err(reason) => return refused(reason),
    };
    let read = match read_first(app, args, "notebook_reference_card") {
        Ok(read) => read,
        Err(waiting) => return waiting,
    };
    let files = current_files(app, &read);
    let (entry, stale) = entry_of(&files, text(args, "file"), &reference, &read.design);
    let badges = super::notebook_badges_of(&read.scan);
    let card = reference_card(&entry, &read.design, &badges, stale);
    answered(serde_json::to_value(&card).unwrap_or(Value::Null))
}

/// `notebook_reference_picker {document, text?, limit?}`: what typing `@`
/// and `text` lists.
pub(crate) fn notebook_reference_picker(app: &mut Signex, args: &CommandArgs) -> Ran {
    let limit = match text(args, "limit").map(str::trim) {
        None => PICKER_LIMIT,
        Some(limit) => match limit.parse::<usize>() {
            Ok(limit) if limit > 0 => limit,
            _ => return refused(format!("`limit` is a count above 0, not `{limit}`")),
        },
    };
    let read = match read_first(app, args, "notebook_reference_picker") {
        Ok(read) => read,
        Err(waiting) => return waiting,
    };
    let query = text(args, "text").unwrap_or_default();
    let entries = picker_entries(&read.design, query, limit);
    answered(json!({ "query": query, "entries": entries }))
}

/// `notebook_badges {document, part?, net?}`: the Notebook badges of the
/// project, or the one badge (Properties ▸ Design notes) of `part` or
/// `net`.
pub(crate) fn notebook_badges(app: &mut Signex, args: &CommandArgs) -> Ran {
    let target = match (text(args, "part"), text(args, "net")) {
        (Some(part), None) => Some(BadgeTarget::Part(part.trim().to_string())),
        (None, Some(net)) => Some(BadgeTarget::Net(net.trim().to_string())),
        (None, None) => None,
        (Some(_), Some(_)) => return refused("name a `part` or a `net`, not both"),
    };
    let read = match read_first(app, args, "notebook_badges") {
        Ok(read) => read,
        Err(waiting) => return waiting,
    };
    let badges = super::notebook_badges_of(&read.scan);
    let shown = !app.ui_state.annotations.badges_hidden;
    match target {
        Some(target) => {
            let badge = badge_of(&badges, &target);
            answered(json!({ "shown": shown, "target": target, "badge": badge }))
        }
        None => answered(json!({ "shown": shown, "badges": badges })),
    }
}

/// The fix `fix` names, with `to` or `pin` where it takes one; `update`
/// without `to` takes the new name the stored id found.
fn fix_operand(args: &CommandArgs, entry: &ResolvedReference) -> Result<Fix, String> {
    let token = text(args, "fix").map(str::trim).unwrap_or_default();
    match token {
        "update" => match (text(args, "to"), &entry.state) {
            (Some(to), _) => Ok(Fix::UpdateTo {
                to: match &entry.reference {
                    Reference::Pin { .. } => Reference::dotted_pin(to.trim())
                        .ok_or_else(|| format!("`{to}` is not a pin: write it as `U3.3`"))?,
                    Reference::Component(_) => Reference::Component(to.trim().to_string()),
                    Reference::Net(_) => Reference::Net(to.trim().to_string()),
                },
            }),
            (None, signex_notebook::ReferenceState::Renamed { now }) => {
                Ok(Fix::UpdateTo { to: now.clone() })
            }
            (None, _) => {
                Err("`update` needs the operand `to`: the reference is not renamed".into())
            }
        },
        "remove" => Ok(Fix::RemoveReference),
        "keep_text" => Ok(Fix::KeepAsPlainText),
        "pick_pin" => text(args, "pin")
            .map(|pin| Fix::PickAnotherPin {
                pin: pin.trim().to_string(),
            })
            .ok_or_else(|| "`pick_pin` needs the operand `pin`".to_string()),
        other => Err(format!(
            "`fix` is `update`, `remove`, `keep_text`, `pick_pin` or `update_all`, not `{other}`"
        )),
    }
}

/// `notebook_fix_reference {document, fix, kind?, name?, to?, pin?, file?}`:
/// apply a fix of a stale reference, or `fix=update_all`, to the notebook
/// tab's text of `file` (the notebook's main file by default). Written on
/// Save, like any notebook edit.
pub(crate) fn notebook_fix_reference(app: &mut Signex, args: &CommandArgs) -> Ran {
    let update_all = text(args, "fix").map(str::trim) == Some("update_all");
    let reference = if update_all {
        None
    } else {
        match reference_operand(args) {
            Ok(reference) => Some(reference),
            Err(reason) => return refused(reason),
        }
    };
    let read = match read_first(app, args, "notebook_fix_reference") {
        Ok(read) => read,
        Err(waiting) => return waiting,
    };
    let name = text(args, "file").map_or(DEFAULT_NOTEBOOK_FILE, str::trim);
    let files = current_files(app, &read);
    let Some(file) = files.iter().find(|f| f.name == name) else {
        return refused(format!("the notebook has no file `{name}`"));
    };
    let result = match &reference {
        None => app.apply_update_all(name, &file.report),
        Some(reference) => {
            let Some(entry) = file.report.get(reference) else {
                return refused(format!("`{name}` does not use {}", reference.name()));
            };
            match fix_operand(args, entry) {
                Ok(fix) => app.fix_reference_in_buffer(name, reference, &fix),
                Err(reason) => return refused(reason),
            }
        }
    };
    match result {
        Ok(changed) => answered(json!({ "file": name, "changed": changed })),
        Err(reason) => refused(reason),
    }
}
