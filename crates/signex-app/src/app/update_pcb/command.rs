//! `update_pcb_from_schematic document=<project, one of its sheets or its
//! board> apply=true | dry_run=true` (#1312, #2470): the form without a
//! dialog. The design is read off the update thread; once the list lands,
//! `apply=true` executes the rows ticked by default through the dialog's
//! Execute (one labelled undo step) and `dry_run=true` changes nothing
//! (`headless.rs`). Either answers with the change list (`answer.rs`).

use iced::Task;
use signex_i18n::{Translatable, ids};

use super::state::HeadlessRun;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

const COMMAND: &str = "update_pcb_from_schematic";

fn refused(reason: impl Into<Translatable>) -> Ran {
    let outcome = CommandOutcome::Disabled {
        reason: reason.into(),
    };
    (outcome, Task::none())
}

/// A reason that is already words (a lookup or a read that failed), under
/// the catalog's "Update PCB did not run" line.
pub(super) fn did_not_run(why: impl Into<String>) -> Translatable {
    Translatable::new(ids::UPDATE_PCB_REFUSED).arg("why", why.into())
}

pub(crate) fn update_pcb_from_schematic(app: &mut Signex, args: &CommandArgs) -> Ran {
    let given = |name: &str| matches!(args.operand(name), Some(OperandValue::Bool(true)));
    // `--dry-run` adds `dry_run=true` to a script's `apply=true`: the dry
    // run wins, so it never changes anything.
    let dry_run = given("dry_run");
    if !dry_run && !given("apply") {
        return refused(ids::UPDATE_PCB_COMMAND_MODE);
    }
    let project = match named_project(app, args) {
        Ok(project) => project,
        Err(why) => return refused(why),
    };
    // The person's dialog, or a run still waiting for its list.
    if app.ui_state.update_pcb.is_some() {
        return refused(ids::UPDATE_PCB_BUSY);
    }
    let command = match AppCommandId::new(COMMAND) {
        Ok(command) => command,
        Err(why) => return refused(did_not_run(why)),
    };
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let task = match app.open_update_pcb_run(project, HeadlessRun { waiting, dry_run }) {
        Ok(task) => task,
        Err(why) => return refused(why),
    };
    // The board could not be opened, or the project has no schematic: that
    // is known now, so the caller is answered now.
    if let Some(why) = app.take_failed_update_pcb_run() {
        let outcome = CommandOutcome::Disabled {
            reason: did_not_run(why),
        };
        return (outcome, task);
    }
    (CommandOutcome::Deferred { ticket }, task)
}

/// The open project `document` belongs to: the project itself, or the
/// project of the sheet or board it names.
fn named_project(app: &Signex, args: &CommandArgs) -> Result<ProjectId, Translatable> {
    let Some(OperandValue::Document(named)) = args.operand("document") else {
        return Err(ids::UPDATE_PCB_NOT_IN_PROJECT.into());
    };
    let entry = app.resolve_document_target(named).map_err(did_not_run)?;
    let path = match entry.kind {
        DocumentKind::Project => entry.path,
        _ => entry.project.map(|project| project.path),
    };
    path.and_then(|path| {
        app.document_state
            .projects
            .iter()
            .find(|project| project.path == path)
            .map(|project| project.id)
    })
    .ok_or_else(|| ids::UPDATE_PCB_NOT_IN_PROJECT.into())
}
