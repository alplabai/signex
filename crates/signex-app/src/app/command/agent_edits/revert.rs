//! `revert document=.. group=N` (#2178): take back an agent's step, only while
//! it is the newest step of that document. The agent has no undo or redo, so a
//! person's step is never one it can move.

use iced::Task;
use serde_json::json;

use crate::app::command::document_operands::named_sheet;
use crate::app::{Message, Signex};

use super::super::{CommandArgs, CommandOutcome, OperandValue};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

pub(in crate::app::command) fn revert(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match named_sheet(app, args, "`revert`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let group = match args.operand("group") {
        Some(OperandValue::Text(text)) => text.parse::<u64>().map(signex_engine::UndoGroupId::new),
        _ => return refused("the operand `group` is needed"),
    };
    let Ok(group) = group else {
        return refused("`group` must be the whole number `apply_edits` answered");
    };
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let Some(label) = app.document_state.agent_edits.labels.get(&group).cloned() else {
        return refused(format!(
            "group {} is not a step an agent made in this session; only those can be reverted",
            group.get()
        ));
    };
    let Some(engine) = app.document_state.engines.get_mut(&path) else {
        return refused(format!("{name} is not loaded"));
    };
    if engine.next_undo_group() != Some(group) {
        return refused(if engine.has_undo_group(group) {
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
        return refused(format!(
            "{name} has a step the person undid and can redo, which would bring group {} back; nothing was changed",
            group.get()
        ));
    }
    // What the person is drawing, typing into or moving is anchored to items
    // of the sheet as it is now.
    if let Some(reason) = app.person_busy_on(&path) {
        return refused(reason);
    }
    let Some(engine) = app.document_state.engines.get_mut(&path) else {
        return refused(format!("{name} is not loaded"));
    };
    let patch = match engine.discard_undo_group_top(group) {
        Ok(Some(patch)) => patch,
        Ok(None) => return refused("nothing was changed"),
        Err(error) => return refused(error.to_string()),
    };
    let revision = engine.revision();
    app.document_state.agent_edits.forget_step(group);
    // Undo can bring back an older embedded library symbol.
    app.library.drift.check_pending = true;
    app.after_sheet_change(&path, Signex::render_invalidation_for_patch(patch.document));
    // A single command's label already says its document.
    let on = if label.ends_with(&format!(" on {name}")) {
        String::new()
    } else {
        format!(" on {name}")
    };
    crate::diagnostics::log_info(format!("Reverted {label}{on} (undo group {})", group.get()));
    (
        CommandOutcome::Answered(
            json!({"reverted": true, "group": group.get(), "revision": revision}),
        ),
        Task::none(),
    )
}
