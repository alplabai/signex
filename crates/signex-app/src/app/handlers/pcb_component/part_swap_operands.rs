//! `swap_pins` and `swap_gates` from their operands (#2410): an agent or the
//! CLI names the board (`document`), the component (`designator`) and the two
//! pins (`pins=a,b`, pad numbers) or gates (`units=a,b`), and gets what the
//! swap did as JSON. The same `Signex::swap_part` as the board menu.

use iced::Task;
use serde_json::json;
use signex_engine::pcb::placement;

use super::part_swap::SwapRequest;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    let reason = reason.into();
    (CommandOutcome::disabled(reason), Task::none())
}

/// The two members of operand `name`, written `a,b`.
fn pair(args: &CommandArgs, name: &str) -> Result<(String, String), String> {
    let text = match args.operand(name) {
        Some(OperandValue::Text(text)) => text.as_str(),
        _ => return Err(format!("needs the operand `{name}`, written `a,b`")),
    };
    let members: Vec<&str> = text
        .split(',')
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .collect();
    match members.as_slice() {
        [a, b] => Ok(((*a).to_string(), (*b).to_string())),
        _ => Err(format!(
            "`{name}` names two members, written `a,b`, not `{text}`"
        )),
    }
}

/// Run `request` built from the operands on the component they name.
fn swap(app: &mut Signex, args: &CommandArgs, request: Result<SwapRequest, String>) -> Ran {
    let board = match app.named_output_board(args) {
        Ok(board) => board,
        Err(reason) => return refused(reason),
    };
    let request = match request {
        Ok(request) => request,
        Err(reason) => return refused(reason),
    };
    let designator = match args.operand("designator") {
        Some(OperandValue::Text(text)) => text.clone(),
        _ => return refused("needs the operand `designator`"),
    };
    let footprint = match app
        .document_state
        .pcb_board(&board)
        .ok_or_else(|| "the board is not open".to_string())
        .and_then(|pcb| {
            let stored = app.document_state.stored_reference_for(&board, &designator);
            placement::by_designator(pcb, stored)
        }) {
        Ok(footprint) => footprint,
        Err(reason) => return refused(reason),
    };
    match app.swap_part(&board, footprint, &request) {
        Ok(report) => {
            let mut answer = serde_json::to_value(&report).unwrap_or_else(|_| json!({}));
            answer["document"] = json!(board.display().to_string());
            (CommandOutcome::Answered(answer), Task::none())
        }
        Err(reason) => refused(reason),
    }
}

/// `swap_pins document=<board> designator=<ref> pins=<a>,<b>`.
pub(crate) fn swap_pins(app: &mut Signex, args: &CommandArgs) -> Ran {
    let request = pair(args, "pins").map(|(a, b)| SwapRequest::Pins(a, b));
    swap(app, args, request)
}

/// `swap_gates document=<board> designator=<ref> units=<a>,<b>`.
pub(crate) fn swap_gates(app: &mut Signex, args: &CommandArgs) -> Ran {
    let request = pair(args, "units").and_then(|(a, b)| {
        let unit = |text: &str| {
            text.parse::<u32>()
                .ok()
                .filter(|unit| *unit > 0)
                .ok_or_else(|| format!("`units` are unit numbers from 1, not `{text}`"))
        };
        Ok(SwapRequest::Gates(unit(a.as_str())?, unit(b.as_str())?))
    });
    swap(app, args, request)
}
