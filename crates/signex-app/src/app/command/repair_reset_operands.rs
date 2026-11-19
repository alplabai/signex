//! Repair Pin Junctions, Reset board origin and Reset All Layer Colours from
//! operands, so a CLI run or an agent can do what the menu rows do on a
//! document it names. Each runs the same commit as its menu row: one undo
//! step, nothing when there is nothing to change.

use iced::Task;
use serde_json::json;

use super::layer_strip_operands::on_named_board;
use super::placement_operands::run_edit;
use super::{CommandArgs, CommandOutcome};
use crate::app::mutation_gateway::MutationOutcome;
use crate::app::{Message, PcbViewMsg, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// `repair_pin_junctions [document=..]`: a dot on every pin end of the sheet
/// that lies inside a wire with none (`signex_engine::pin_junction_repairs`).
/// Answers with how many it added.
pub(super) fn repair_pin_junctions(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let engine = app
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?;
        let mut ids = signex_types::object_id::UuidV7Ids;
        let repairs = signex_engine::pin_junction_repairs(engine.document(), &mut ids);
        let added = repairs.len();
        if added > 0 {
            let (outcome, reason) = app.apply_engine_commands_reporting(repairs, true, true);
            if outcome == MutationOutcome::Failed {
                return Err(reason.unwrap_or_else(|| "the engine refused the repair".into()));
            }
        }
        Ok(CommandOutcome::Answered(json!({ "added": added })))
    })
}

/// `reset_board_origin document=..`: the origin to the bottom-left of the
/// board outline. Answers with the origin it left.
pub(super) fn reset_board_origin(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match app.named_output_board(args) {
        Ok(path) => path,
        Err(reason) => return (CommandOutcome::disabled(reason), Task::none()),
    };
    if let Err(reason) = app.reset_pcb_origin_at(&path) {
        return (CommandOutcome::disabled(reason), Task::none());
    }
    let origin = app
        .document_state
        .pcb_board(&path)
        .map(|board| board.options.origin.at);
    let answer = json!({
        "document": path.display().to_string(),
        "origin": origin.map(|at| [at.x, at.y]),
    });
    (CommandOutcome::Answered(answer), Task::none())
}

/// `reset_layer_colors document=..`: every layer the board palette colour
/// again, as the layer tab's row does.
pub(super) fn reset_layer_colors(app: &mut Signex, args: &CommandArgs) -> Ran {
    let read_only = app.named_output_board(args).is_ok_and(|path| {
        app.document_state
            .pcb_board(&path)
            .is_some_and(|board| board.unknown.newer_minor_source.is_some())
    });
    if read_only {
        let reason = "the board opens read-only".to_string();
        return (CommandOutcome::disabled(reason), Task::none());
    }
    on_named_board(app, args, Ok(PcbViewMsg::LayerColoursReset))
}
