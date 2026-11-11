//! `revert document=.. group=N [documents=[..]]` (#2178): take back an agent's
//! step, only while it is the newest step of that document. The agent has no
//! undo or redo, so a person's step is never one it can move. A step that
//! spans sheets (a cross-sheet `apply_edits`) is taken back on all of them or
//! on none, and only when the call names every one of them.

use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_engine::UndoGroupId;

use super::apply::{file_name, sheet_named};
use super::wire::parse_documents;
use crate::app::command::document_operands::named_sheet;
use crate::app::{Message, Signex, UndoGroupKind};

use super::super::{CommandArgs, CommandOutcome, OperandValue};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

/// The sheets of `group`: those its cross-sheet record holds, else `path` alone.
fn members_of(app: &Signex, group: UndoGroupId, path: &Path) -> Vec<PathBuf> {
    match app.document_state.undo_groups.get(&group) {
        Some(record) if record.kind == UndoGroupKind::AgentEdit => record.live.clone(),
        _ => vec![path.to_path_buf()],
    }
}

/// Why `group` cannot be taken back on the sheet at `path` right now, or
/// `None` when it can: it must be that sheet's newest step, with nothing the
/// person undid to redo and nothing the person is in the middle of.
fn blocked_on(app: &Signex, path: &Path, group: UndoGroupId) -> Option<String> {
    let name = file_name(path);
    let Some(engine) = app.document_state.engines.get(path) else {
        return Some(format!("{name} is not loaded"));
    };
    if engine.next_undo_group() != Some(group) {
        return Some(if engine.has_undo_group(group) {
            format!(
                "group {} is not the newest step of {name}: a later step sits on top; nothing was changed",
                group.get()
            )
        } else {
            format!(
                "group {} is no longer in the history of {name} (reverted, undone or trimmed); nothing was changed",
                group.get()
            )
        });
    }
    // A step the person undid sits on the redo stack as a snapshot of the sheet
    // with this step in it: redoing it after the revert would bring it back.
    if engine.can_redo() {
        return Some(format!(
            "{name} has a step the person undid and can redo, which would bring group {} back; nothing was changed",
            group.get()
        ));
    }
    // What the person is drawing, typing into or moving is anchored to items
    // of the sheet as it is now.
    app.person_busy_on(path).map(str::to_string)
}

/// The sheets the call names: `document`, and each of the `documents` list.
fn named_sheets(app: &Signex, args: &CommandArgs, path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut named = vec![path.to_path_buf()];
    if let Some(OperandValue::Text(list)) = args.operand("documents") {
        for document in parse_documents(list)? {
            let (sheet, _) = sheet_named(app, &document)
                .map_err(|why| format!("`documents` entry `{document}`: {why}"))?;
            if !named.contains(&sheet) {
                named.push(sheet);
            }
        }
    }
    Ok(named)
}

pub(in crate::app::command) fn revert(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match named_sheet(app, args, "`revert`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let group = match args.operand("group") {
        Some(OperandValue::Text(text)) => text.parse::<u64>().map(UndoGroupId::new),
        _ => return refused("the operand `group` is needed"),
    };
    let Ok(group) = group else {
        return refused("`group` must be the whole number `apply_edits` answered");
    };
    let Some(label) = app.document_state.agent_edits.labels.get(&group).cloned() else {
        return refused(format!(
            "group {} is not a step an agent made in this session; only those can be reverted",
            group.get()
        ));
    };
    let members = members_of(app, group, &path);
    // The caller must name every sheet of the group itself, so a revert never
    // moves, nor names, a sheet the caller did not name (and so did not have
    // confined): a refusal says how many are missing, never which.
    let named = match named_sheets(app, args, &path) {
        Ok(named) => named,
        Err(reason) => return refused(reason),
    };
    let missing = members.iter().filter(|m| !named.contains(m)).count();
    let stray = named.iter().filter(|n| !members.contains(n)).count();
    if missing + stray > 0 {
        return refused(format!(
            "group {} does not cover exactly the sheets named: name every sheet of the group in `documents` ({missing} not named, {stray} named that are not in it); nothing was changed",
            group.get()
        ));
    }
    // Every sheet is judged before any is touched: all of them or none.
    if let Some(reason) = members
        .iter()
        .find_map(|member| blocked_on(app, member, group))
    {
        return refused(reason);
    }
    let mut taken: Vec<(PathBuf, u64, signex_engine::DocumentPatch)> = Vec::new();
    for member in &members {
        let Some(engine) = app.document_state.engines.get_mut(member) else {
            continue;
        };
        match engine.discard_undo_group_top(group) {
            Ok(Some(patch)) => taken.push((member.clone(), engine.revision(), patch.document)),
            Ok(None) => return refused("nothing was changed"),
            Err(error) => return refused(error.to_string()),
        }
    }
    app.document_state.agent_edits.forget_step(group);
    app.document_state.undo_groups.remove(&group);
    // Undo can bring back an older embedded library symbol.
    app.library.drift.check_pending = true;
    for (member, _, patch) in &taken {
        app.after_sheet_change(member, Signex::render_invalidation_for_patch(*patch));
    }
    let names: Vec<String> = members.iter().map(|member| file_name(member)).collect();
    // A single command's label already says its document.
    let on = if label.ends_with(&format!(" on {}", names.join(", "))) {
        String::new()
    } else {
        format!(" on {}", names.join(", "))
    };
    crate::diagnostics::log_info(format!("Reverted {label}{on} (undo group {})", group.get()));
    let revision = taken
        .iter()
        .find(|(member, _, _)| *member == path)
        .map_or(0, |(_, revision, _)| *revision);
    let mut answer = json!({"reverted": true, "group": group.get(), "revision": revision});
    if taken.len() > 1
        && let Some(object) = answer.as_object_mut()
    {
        let sheets: Vec<Value> = taken
            .iter()
            .map(|(member, revision, _)| json!({"document": file_name(member), "revision": revision}))
            .collect();
        object.insert("sheets".to_string(), Value::Array(sheets));
    }
    (CommandOutcome::Answered(answer), Task::none())
}
