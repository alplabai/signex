//! `update_pcb_from_schematic [document=<project, one of its sheets or its
//! board>] apply=true | dry_run=true` (#1312, #2470): the form without a
//! dialog. With no `document` it acts on the active project's board. The
//! design is read off the update thread; once the list lands,
//! `apply=true` executes the rows ticked by default through the dialog's
//! Execute (one labelled undo step) and `dry_run=true` changes nothing
//! (`headless.rs`). Either answers with the change list (`answer.rs`).

use iced::Task;
use signex_i18n::{Translatable, ids};

use super::state::UpdatePcbJob;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::NoProjectNamed;
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
    let task = match app.open_update_pcb_run(project, UpdatePcbJob { waiting, dry_run }) {
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
/// project of the sheet or board it names. With no `document`, the active
/// project (Caner, 2026-11-25; [`active_project_with_board`]).
fn named_project(app: &Signex, args: &CommandArgs) -> Result<ProjectId, Translatable> {
    let Some(OperandValue::Document(named)) = args.operand("document") else {
        return active_project_with_board(app);
    };
    match app.project_named(named) {
        Ok(project) => Ok(project.id),
        Err(NoProjectNamed::Unresolved(why)) => Err(did_not_run(why)),
        Err(NoProjectNamed::NotInProject(_)) => Err(ids::UPDATE_PCB_NOT_IN_PROJECT.into()),
    }
}

/// No `document`: the active project's board (Caner, 2026-11-25). The
/// active project is `DocumentState::active_loaded_project`, the workspace
/// pointer every project open or reopen sets (headless too) and a close moves
/// to a neighbour; with none set and one project open, that project. It never
/// reads the tab in view, and another project's board is never taken in its
/// place: with no board on the active project, or no active project among
/// several, the run is refused with the boards to name. No project open, or
/// no board to list, is refused as before.
fn active_project_with_board(app: &Signex) -> Result<ProjectId, Translatable> {
    let docs = &app.document_state;
    if docs.projects.is_empty() {
        return Err(ids::UPDATE_PCB_NOT_IN_PROJECT.into());
    }
    let active = docs
        .active_loaded_project()
        .or(match docs.projects.as_slice() {
            [only] => Some(only),
            _ => None,
        });
    if let Some(project) = active
        && Signex::project_board(project).is_some()
    {
        return Ok(project.id);
    }
    let case = if active.is_some() {
        "active-has-none"
    } else {
        "no-active"
    };
    let boards: Vec<String> = docs
        .projects
        .iter()
        .filter(|project| active.is_none_or(|active| active.id != project.id))
        .filter_map(Signex::project_board)
        .map(|board| board.display().to_string())
        .collect();
    if boards.is_empty() {
        return Err(ids::UPDATE_PCB_NO_BOARD.into());
    }
    Err(Translatable::new(ids::UPDATE_PCB_WHICH_BOARD)
        .arg("case", case)
        .arg("boards", boards.join(", ")))
}
