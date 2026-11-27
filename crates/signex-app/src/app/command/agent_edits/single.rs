//! One agent command that changes a document, as one labelled undo step
//! (#2253): the same group, label and repaint rules as `apply_edits`, for a
//! command the agent runs on its own. See `docs/AGENT_EDITS.md`, "Origin".

use std::path::PathBuf;

use iced::Task;
use signex_engine::{DocumentPatch, Engine};

use crate::app::command::document_operands::named_sheet;
use crate::app::command::operand_handlers::OperandHandler;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

use super::group_run::{commit_all, rollback_all};
use super::joined::{joined_paths, roll_back_joined, settle_joined, take_joined};
use super::wire::step_refusal;

use super::super::{CommandArgs, CommandOutcome};

type Ran = (CommandOutcome, Task<Message>);

/// Is `command` one an agent's run turns into a labelled undo step? Exactly
/// the commands that may be an edit of `apply_edits`, plus those a list does
/// not carry yet (`step_refusal`): so not `apply_edits` itself, which runs a
/// transaction of its own, nor the project-wide `assign_net_class`, which
/// leaves only a pick marker (no group).
pub(crate) fn runs_as_one_step(command: &AppCommandId) -> bool {
    step_refusal(command).is_none()
}

/// Run `handler` for `command` inside one engine transaction on the sheet its
/// `document` names. What it changed becomes one step stamped with a new
/// group and labelled "Agent: <command> on <file>"; a refusal rolls back. A
/// sheet the command brings in (`join_agent_step`, #2501) commits under the
/// same group, recorded as a cross-sheet agent step.
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
        // A read-only sheet: the handler's own check gives the person's
        // reason and the once-per-sheet Messages warning.
        if matches!(error, signex_engine::EngineError::ReadOnly) {
            return handler(app, args);
        }
        return (CommandOutcome::disabled(error.to_string()), Task::none());
    }
    let edits = &mut app.document_state.agent_edits;
    let previous = edits.target.replace(path.clone());
    edits.owed = DocumentPatch::NONE;
    let ran = handler(app, args);
    let edits = &mut app.document_state.agent_edits;
    let owed = std::mem::take(&mut edits.owed);
    edits.target = previous;
    // A sheet the command wrote besides its own (#2501) commits with it.
    let joined = take_joined(app);
    if !app.document_state.engines.contains_key(&path) {
        roll_back_joined(app, &joined);
        return (
            CommandOutcome::disabled("the sheet is not loaded"),
            Task::none(),
        );
    }
    let sheets: Vec<PathBuf> = std::iter::once(path.clone())
        .chain(joined_paths(&joined))
        .collect();
    if !matches!(ran.0, CommandOutcome::Ran | CommandOutcome::Answered(_)) {
        rollback_all(app, &sheets);
        roll_back_joined(app, &joined);
        return ran;
    }
    // With a joined sheet, no engine commits until every one can.
    let ready = sheets.len() < 2
        || (sheets.iter()).all(|sheet| {
            (app.document_state.engines.get(sheet)).is_some_and(Engine::transaction_ready)
        });
    if !ready {
        rollback_all(app, &sheets);
        roll_back_joined(app, &joined);
        let why = "a sheet's transaction cannot commit. Nothing was applied";
        return (CommandOutcome::disabled(why), Task::none());
    }
    let group = app.mint_undo_group();
    let landed = match commit_all(app, &sheets, group, |engine, _, group| {
        engine.commit_transaction(Some(group))
    }) {
        Ok(landed) => landed,
        Err(error) => {
            // Taken back or rolled back already; a sheet loaded for the step
            // goes too.
            roll_back_joined(app, &joined);
            let why = format!("{error}. Nothing was applied");
            return (CommandOutcome::disabled(why), Task::none());
        }
    };
    for (sheet, _) in &landed {
        if let Some(engine) = app.document_state.engines.get_mut(sheet) {
            Signex::report_engine_warnings(engine);
        }
    }
    let changed: Vec<PathBuf> = (landed.iter())
        .filter(|(_, changed)| *changed)
        .map(|(sheet, _)| sheet.clone())
        .collect();
    settle_joined(app, group, &changed, &joined);
    if !changed.is_empty() {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let label = format!("Agent: {} on {name}", command.as_str());
        app.document_state
            .agent_edits
            .record_step(group, label.clone(), &app.agent.origin);
        for sheet in &changed {
            // The refresh owed is the command's own sheet's; a joined one
            // is refreshed whole.
            let patch = if *sheet != path || owed == DocumentPatch::NONE {
                DocumentPatch::FULL
            } else {
                owed
            };
            app.after_sheet_change(sheet, patch);
        }
        crate::diagnostics::log_info(format!("{label} (undo group {})", group.get()));
    }
    ran
}
