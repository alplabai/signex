//! `swap_schematic_pins` and `swap_schematic_gates` from their operands
//! (#1261): an agent or the CLI names the sheet (`document`), the part (`ids`,
//! one placed unit's uuid, or `designator`) and the two pins (`pins=a,b`,
//! pin numbers) or gates (`units=a,b`), and gets what the swap did as JSON.
//! The Part Actions rows run the same handler through the translator, naming
//! the part by uuid. Only the sheet changes; Update PCB carries the swap to
//! the board.

use std::path::Path;

use iced::Task;
use signex_engine::part_swap::SheetSwapRequest;
use uuid::Uuid;

use super::document_operands::named_sheet;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::pcb_component::part_swap_operands::{pair, unit_pair};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// `swap_schematic_pins document=<sheet> (ids=<unit uuid> | designator=<ref>)
/// pins=<a>,<b>`.
pub(super) fn swap_schematic_pins(app: &mut Signex, args: &CommandArgs) -> Ran {
    let request = pair(args, "pins", false).map(|(a, b)| SheetSwapRequest::Pins(a, b));
    swap(app, args, "swap_schematic_pins", request)
}

/// `swap_schematic_gates document=<sheet> (ids=<unit uuid> |
/// designator=<ref>) units=<a>,<b>`.
pub(super) fn swap_schematic_gates(app: &mut Signex, args: &CommandArgs) -> Ran {
    let request = unit_pair(args).map(|(a, b)| SheetSwapRequest::Gates(a, b));
    swap(app, args, "swap_schematic_gates", request)
}

fn swap(
    app: &mut Signex,
    args: &CommandArgs,
    command: &str,
    request: Result<SheetSwapRequest, String>,
) -> Ran {
    let ran = named_sheet(app, args, "a schematic swap").and_then(|path| {
        let request = request?;
        let symbol = part_on(app, &path, args)?;
        app.swap_on_sheet_at(&path, symbol, &request)
    });
    match ran {
        Ok(report) => {
            let answer = serde_json::to_value(&report).unwrap_or_default();
            (CommandOutcome::Answered(answer), Task::none())
        }
        Err(reason) => {
            // The Part Actions rows say it themselves (`handle_sheet_swap`).
            if !app.agent.from_view {
                app.refuse_sheet_swap(command, &reason);
            }
            let reason = crate::app::sheet_swap::sheet_swap_refusal(command, reason);
            (CommandOutcome::Disabled { reason }, Task::none())
        }
    }
}

/// The placed unit `args` names on the sheet at `path`: `ids` (one uuid) or
/// `designator`, exactly one of the two. A blank `designator` is not given;
/// a designator carried by two parts of the sheet is refused.
fn part_on(app: &Signex, path: &Path, args: &CommandArgs) -> Result<Uuid, String> {
    let doc = app
        .document_state
        .engines
        .get(path)
        .map(|engine| engine.document())
        .ok_or_else(|| format!("{} is not loaded", path.display()))?;
    let designator = match args.operand("designator") {
        Some(OperandValue::Text(text)) if !text.trim().is_empty() => Some(text.trim()),
        _ => None,
    };
    let is_part = |id: &Uuid| doc.symbols.iter().any(|s| s.uuid == *id && !s.is_power);
    match (args.targets("ids"), designator) {
        (Some(_), Some(_)) => Err("name the part by `ids` or by `designator`, not both".into()),
        (None, None) => {
            Err("name the part by `ids` (a placed unit's uuid) or by `designator`".into())
        }
        (Some([id]), None) if is_part(id) => Ok(*id),
        (Some([id]), None) => Err(format!("no part {id} on this sheet")),
        (Some(many), None) => Err(format!(
            "`ids` names one placed unit of the part, not {}",
            many.len()
        )),
        (None, Some(designator)) => {
            let named: Vec<Uuid> = doc
                .symbols
                .iter()
                .filter(|s| !s.is_power && s.reference.trim() == designator)
                .map(|s| s.uuid)
                .collect();
            let first = *named
                .first()
                .ok_or_else(|| format!("no part {designator} on this sheet"))?;
            let part = signex_engine::placed_unit_ids(doc, first);
            if named.iter().all(|id| part.contains(id)) {
                Ok(first)
            } else {
                Err(format!(
                    "{designator} names more than one part on this sheet; name a unit by `ids`"
                ))
            }
        }
    }
}
