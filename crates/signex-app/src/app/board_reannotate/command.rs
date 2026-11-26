//! `reannotate_board document=.. [order=..] [bottom=..] [dry_run=..]`
//! (#2470): Positional Re-Annotate of the named board without the dialog.
//! The plan is the dialog's (`plan_reannotate`) and the renames run through
//! the same labelled board step as its Execute, so one call is one undo
//! step. The answer lists every rename and every part kept as it was.

use iced::Task;
use serde_json::{Value, json};
use signex_engine::pcb::designators::{
    BottomSideOrder, KeptReason, ReannotateOptions, ReannotateOrder, ReannotatePlan,
    plan_reannotate,
};

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document::DocumentKey;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.trim()),
        _ => None,
    }
}

/// The options `order` and `bottom` name; the dialog's defaults when left out.
pub(crate) fn reannotate_options(args: &CommandArgs) -> Result<ReannotateOptions, String> {
    let order = match text(args, "order") {
        None | Some("x_then_y") => ReannotateOrder::XThenY,
        Some("y_then_x") => ReannotateOrder::YThenX,
        Some(other) => {
            return Err(format!(
                "`order` is `x_then_y` or `y_then_x`, not `{other}`"
            ));
        }
    };
    let bottom = match text(args, "bottom") {
        None | Some("after_top") => BottomSideOrder::AfterTop,
        Some("with_top") => BottomSideOrder::WithTop,
        Some(other) => {
            return Err(format!(
                "`bottom` is `after_top` or `with_top`, not `{other}`"
            ));
        }
    };
    Ok(ReannotateOptions { order, bottom })
}

fn kept_reason(reason: KeptReason) -> &'static str {
    match reason {
        KeptReason::Locked => "locked",
        KeptReason::NoPrefix => "no_prefix",
        KeptReason::Unlinked => "unlinked",
    }
}

/// The answer: each rename (`footprint`, `old`, `new`) in numbering order,
/// each part kept with why, and how many were already in order.
pub(crate) fn plan_answer(board: &str, plan: &ReannotatePlan, applied: bool) -> Value {
    let renames: Vec<Value> = plan
        .renames
        .iter()
        .map(|rename| {
            json!({
                "footprint": rename.footprint.to_string(),
                "old": rename.from,
                "new": rename.to,
            })
        })
        .collect();
    let kept: Vec<Value> = plan
        .kept
        .iter()
        .map(|kept| {
            json!({
                "footprint": kept.footprint.to_string(),
                "designator": kept.reference,
                "reason": kept_reason(kept.reason),
            })
        })
        .collect();
    json!({
        "board": board,
        "applied": applied,
        "renames": renames,
        "kept": kept,
        "unchanged": plan.unchanged,
    })
}

/// `reannotate_board` from its operands.
pub(crate) fn reannotate_board(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(OperandValue::Document(target)) = args.operand("document") else {
        return refused("`reannotate_board` needs the operand `document`");
    };
    let entry = match app.resolve_document_target(target) {
        Ok(entry) => entry,
        Err(why) => return refused(why),
    };
    let board = match entry.key {
        Some(DocumentKey::Pcb(path)) if app.document_state.board_engine(&path).is_some() => path,
        Some(DocumentKey::Pcb(_)) | None => {
            return refused(format!("{} is not open; open it first", entry.name));
        }
        Some(_) => {
            return refused(format!(
                "`reannotate_board` numbers a board, and {} is a {}",
                entry.name,
                entry.kind.as_str()
            ));
        }
    };
    let options = match reannotate_options(args) {
        Ok(options) => options,
        Err(why) => return refused(why),
    };
    let Some(engine) = app.document_state.board_engine(&board) else {
        return refused(format!("{} is not open; open it first", entry.name));
    };
    if engine.board().unknown.newer_minor_source.is_some() {
        return refused(format!(
            "{} was written by a newer Signex and is read-only here",
            entry.name
        ));
    }
    let plan = plan_reannotate(engine.board(), options);
    let revision = engine.revision();
    let name = board.display().to_string();
    let dry_run = matches!(args.operand("dry_run"), Some(OperandValue::Bool(true)));
    if dry_run || plan.renames.is_empty() {
        let answer = plan_answer(&name, &plan, false);
        return (CommandOutcome::Answered(answer), Task::none());
    }
    match app.run_reannotate_step(&board, &entry.name, &plan, revision) {
        Ok(()) => (
            CommandOutcome::Answered(plan_answer(&name, &plan, true)),
            Task::none(),
        ),
        Err(why) => refused(format!("`reannotate_board`: nothing was renamed: {why}")),
    }
}
