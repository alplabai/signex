//! One agent command that changes a document, as one labelled undo step
//! (#2253): the same group, label and repaint rules as `apply_edits`, for a
//! command the agent runs on its own. See `docs/AGENT_EDITS.md`, "Origin".

use iced::Task;

use crate::app::command::document_operands::named_sheet;
use crate::app::command::operand_handlers::OperandHandler;
use crate::app::{Message, Signex};
use crate::keymap::{AppCommandId, metadata_for};
use crate::schematic_runtime::RenderInvalidation;

use super::super::{CommandArgs, CommandOutcome};

type Ran = (CommandOutcome, Task<Message>);

/// Commands that change a document and are undoable but make their own step:
/// `apply_edits` runs a transaction of its own, and `assign_net_class` edits
/// the project's net classes, leaving only a pick marker (no group) on the
/// sheet's timeline.
const OWN_STEP: &[&str] = &["apply_edits", "assign_net_class"];

/// Is `command` one an agent's run turns into a labelled undo step?
pub(crate) fn runs_as_one_step(command: &AppCommandId) -> bool {
    metadata_for(command).is_some_and(|meta| meta.flags.mutates_doc && meta.flags.undoable)
        && !OWN_STEP.contains(&command.as_str())
}

/// Run `handler` for `command` inside one engine transaction on the sheet its
/// `document` names. What it changed becomes one step stamped with a new
/// group and labelled "Agent: <command> on <file>"; a refusal rolls back.
pub(crate) fn run_as_one_step(
    app: &mut Signex,
    command: &AppCommandId,
    args: &CommandArgs,
    handler: OperandHandler,
) -> Ran {
    // No sheet named or loaded: the handler says so itself.
    let Ok(path) = named_sheet(app, args, "the command") else {
        return handler(app, args);
    };
    let Some(engine) = app.document_state.engines.get_mut(&path) else {
        return handler(app, args);
    };
    if let Err(error) = engine.begin_transaction() {
        return (CommandOutcome::disabled(error.to_string()), Task::none());
    }
    let edits = &mut app.document_state.agent_edits;
    let previous = edits.target.replace(path.clone());
    edits.owed = RenderInvalidation::NONE;
    let ran = handler(app, args);
    let edits = &mut app.document_state.agent_edits;
    let owed = std::mem::take(&mut edits.owed);
    edits.target = previous;
    let Some(engine) = app.document_state.engines.get_mut(&path) else {
        return (
            CommandOutcome::disabled("the sheet is not loaded"),
            Task::none(),
        );
    };
    if !matches!(ran.0, CommandOutcome::Ran | CommandOutcome::Answered(_)) {
        engine.rollback_transaction();
        return ran;
    }
    let group = app.mint_undo_group();
    let Some(engine) = app.document_state.engines.get_mut(&path) else {
        return (
            CommandOutcome::disabled("the sheet is not loaded"),
            Task::none(),
        );
    };
    let committed = match engine.commit_transaction(Some(group)) {
        Ok(result) => result,
        Err(error) => {
            engine.rollback_transaction();
            let why = format!("{error}. Nothing was applied");
            return (CommandOutcome::disabled(why), Task::none());
        }
    };
    Signex::report_engine_warnings(engine);
    if committed.changed {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let label = format!("Agent: {} on {name}", command.as_str());
        let invalidation = if owed == RenderInvalidation::NONE {
            RenderInvalidation::FULL
        } else {
            owed
        };
        app.document_state
            .agent_edits
            .record_step(group, label.clone(), &app.agent.origin);
        app.after_sheet_change(&path, invalidation);
        crate::diagnostics::log_info(format!("{label} (undo group {})", group.get()));
    }
    ran
}
