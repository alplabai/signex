//! Auto Place in Rooms from its operands (#2941): each room's unlocked
//! parts placed inside it by the weighted priorities
//! (`signex_engine::pcb::rooms::auto_place`), as one undo step, or only
//! reported with `dry_run=true`. Placement only: nothing is routed. See
//! `docs/CLI.md`, "Rooms and placement".

use iced::Task;
use serde_json::{Value, json};
use signex_engine::pcb::rooms::auto_place::{
    self as placer, AutoPlaceOptions, AutoPlacePlan, NotPlacedReason, Priority, Scores, Weights,
};
use signex_i18n::{Translatable, ids};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::answer::point;
use super::operands::{Ran, named_room, refused, refused_why, text};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};

/// The options the operands give, over the defaults.
fn options_of(board: &PcbBoard, args: &CommandArgs) -> Result<AutoPlaceOptions, Translatable> {
    let mut options = AutoPlaceOptions::default();
    if board.rooms.is_empty() {
        return Err(Translatable::new(ids::ROOMS_NO_ROOMS));
    }
    if let Some(list) = text(args, "rooms") {
        options.rooms = list
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(|name| named_room(board, name).map(|r| r.uuid))
            .collect::<Result<Vec<Uuid>, _>>()?;
    }
    if let Some(weights) = text(args, "weights") {
        options.weights = Weights::parse(weights)
            .map_err(|why| Translatable::new(ids::ROOMS_BAD_WEIGHTS).arg("why", why))?;
    }
    if let Some(OperandValue::Length(mm)) = args.operand("thermal_gap") {
        options.gaps.thermal_mm = *mm;
    }
    if let Some(OperandValue::Length(mm)) = args.operand("separation_gap") {
        options.gaps.separation_mm = *mm;
    }
    Ok(options)
}

fn per_priority(of: impl Fn(Priority) -> f64) -> Value {
    let map: serde_json::Map<String, Value> = Priority::ALL
        .into_iter()
        .map(|p| (p.token().to_string(), json!(of(p))))
        .collect();
    Value::Object(map)
}

fn scores(s: &Scores) -> Value {
    per_priority(|p| s.of(p))
}

/// The plan as the command answers it.
fn answer_of(plan: &AutoPlacePlan, options: &AutoPlaceOptions, dry_run: bool) -> Value {
    let moved: Vec<Value> = plan
        .moves
        .iter()
        .map(|m| {
            json!({
                "id": m.footprint.to_string(),
                "designator": m.designator,
                "room": m.room,
                "from": point(m.from),
                "to": point(m.to),
                "rotation": m.rotation,
                "side": if m.bottom { "bottom" } else { "top" },
            })
        })
        .collect();
    let not_placed: Vec<Value> = plan
        .not_placed
        .iter()
        .map(|n| {
            let reason = match n.reason {
                NotPlacedReason::NoSpace => "no_space",
                NotPlacedReason::Unreadable => "unreadable",
            };
            json!({
                "id": n.footprint.to_string(),
                "designator": n.designator,
                "room": n.room,
                "reason": reason,
            })
        })
        .collect();
    json!({
        "dry_run": dry_run,
        "rooms": plan.rooms,
        "weights": per_priority(|p| options.weights.of(p)),
        "gaps_mm": {
            "thermal": options.gaps.thermal_mm,
            "separation": options.gaps.separation_mm,
        },
        "moved": moved,
        "not_placed": not_placed,
        "score": {"before": scores(&plan.before), "after": scores(&plan.after)},
    })
}

/// `auto_place document=.. rooms=? weights=? thermal_gap=? separation_gap=?
/// dry_run=?`: the placement as one undo step, answered with each part
/// moved, the parts that stayed and the score per priority before and
/// after. With `dry_run=true` nothing changes.
pub(crate) fn auto_place_by_operands(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match app.named_output_board(args) {
        Ok(path) => path,
        Err(why) => return refused_why(why),
    };
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused_why("the board is not open");
    };
    let options = match options_of(board, args) {
        Ok(options) => options,
        Err(reason) => return refused(reason),
    };
    let dry_run = matches!(args.operand("dry_run"), Some(OperandValue::Bool(true)));
    let plan = placer::auto_place(board, &options);
    let mut answer = answer_of(&plan, &options, dry_run);
    answer["document"] = json!(path.display().to_string());
    if !dry_run
        && !plan.moves.is_empty()
        && let Err(why) = app.run_room_step(&path, plan.commands(), ids::ROOMS_UNDO_AUTO_PLACE)
    {
        return refused_why(why);
    }
    (CommandOutcome::Answered(answer), Task::none())
}
