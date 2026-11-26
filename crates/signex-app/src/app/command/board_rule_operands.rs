//! The board rule edits an agent or the CLI runs by naming the board:
//! `set_teardrop_rule` (the board's teardrop rule), `set_pad_teardrop` (a
//! pad's own teardrop setting) and `set_pour_properties` (a pour's fill
//! priority and island removal). Each acts on the board `document` names,
//! is one undo step and answers with what the board then holds. The pour
//! edit runs the zone edits the pour's Properties rows run.
//! Behaviour: `docs/PCB_TOOLS.md`, "Teardrop rule" and "Pour properties".

use serde_json::{Value, json};
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::primitive_edit::{ZoneEdit, zone_edits_command};
use signex_i18n::{MessageId, ids, t};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_options::BoardOptions;
use signex_types::pcb_teardrop_rule::TeardropRule;
use uuid::Uuid;

use super::board_edit_operands::{answered, board, named_board, refused, text, uuids};
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, iced::Task<Message>);

fn flag(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(on)) => Some(*on),
        _ => None,
    }
}

/// The plain number operand `name`, `Ok(None)` when not given.
fn number(args: &CommandArgs, name: &str) -> Result<Option<f64>, String> {
    let Some(raw) = text(args, name) else {
        return Ok(None);
    };
    raw.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(Some)
        .ok_or_else(|| format!("`{name}` must be a number, not `{raw}`"))
}

/// Replace the board's options with `options` as one undo step labelled
/// `what`; the same options are no step.
fn set_options(
    app: &mut Signex,
    path: &std::path::Path,
    options: BoardOptions,
    what: MessageId,
) -> Result<(), String> {
    if board(app, path).is_some_and(|b| b.options == options) {
        return Ok(());
    }
    let label = t!(app.ui_state.i18n, what).into_string();
    let command = BoardCommand::SetBoardOptions {
        options: Box::new(options),
    };
    app.run_board_step(path, vec![command], &label)
}

/// A pad's own setting as its operand word.
fn pad_word(setting: Option<bool>) -> &'static str {
    match setting {
        Some(true) => "on",
        Some(false) => "off",
        None => "rule",
    }
}

fn rule_json(rule: &TeardropRule) -> Value {
    let overrides: Vec<Value> = rule
        .pad_overrides
        .iter()
        .map(|(id, on)| json!({ "id": id, "teardrop": pad_word(Some(*on)) }))
        .collect();
    json!({
        "pads": rule.pads,
        "vias": rule.vias,
        "width_share": rule.width_share,
        "length_share": rule.length_share,
        "pad_overrides": overrides,
    })
}

/// `set_teardrop_rule document=.. [pads=..] [vias=..] [width_share=..]
/// [length_share=..]`: a field left out keeps its value.
pub(super) fn set_teardrop_rule(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match named_board(app, args, "`set_teardrop_rule`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let Some(held) = board(app, &path) else {
        return refused("the board is no longer open");
    };
    let mut options = held.options.clone();
    let rule = &mut options.teardrops;
    let (pads, vias) = (flag(args, "pads"), flag(args, "vias"));
    let (width, length) = match (number(args, "width_share"), number(args, "length_share")) {
        (Ok(width), Ok(length)) => (width, length),
        (Err(why), _) | (_, Err(why)) => return refused(why),
    };
    if pads.is_none() && vias.is_none() && width.is_none() && length.is_none() {
        return refused(
            "`set_teardrop_rule` needs at least one of `pads`, `vias`, `width_share` \
             and `length_share`",
        );
    }
    rule.pads = pads.unwrap_or(rule.pads);
    rule.vias = vias.unwrap_or(rule.vias);
    rule.width_share = width.unwrap_or(rule.width_share);
    rule.length_share = length.unwrap_or(rule.length_share);
    if let Err(why) = rule.check() {
        return refused(why.to_string());
    }
    if let Err(why) = set_options(app, &path, options, ids::PCB_BOARD_TEARDROP_RULE_UNDO) {
        return refused(why);
    }
    let Some(held) = board(app, &path) else {
        return refused("the board is no longer open");
    };
    answered(json!({ "document": path, "teardrop_rule": rule_json(&held.options.teardrops) }))
}

/// Whether `id` is a pad of one of the board's footprints.
fn is_pad(board: &PcbBoard, id: Uuid) -> bool {
    board
        .footprints
        .iter()
        .any(|fp| fp.pads.iter().any(|p| p.uuid == id))
}

/// `set_pad_teardrop document=.. ids=.. teardrop=on|off|rule`.
pub(super) fn set_pad_teardrop(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match named_board(app, args, "`set_pad_teardrop`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let targets = match uuids(args) {
        Ok(targets) => targets,
        Err(reason) => return refused(reason),
    };
    let setting = match text(args, "teardrop").map(str::trim) {
        Some("on") => Some(true),
        Some("off") => Some(false),
        Some("rule") => None,
        Some(other) => {
            return refused(format!(
                "`{other}` is not a pad teardrop setting (on, off, rule)"
            ));
        }
        None => return refused("`set_pad_teardrop` needs the operand `teardrop`"),
    };
    let Some(held) = board(app, &path) else {
        return refused("the board is no longer open");
    };
    if let Some(id) = targets.iter().find(|id| !is_pad(held, **id)) {
        return refused(format!("{id} is not a pad of the board's footprints"));
    }
    let mut options = held.options.clone();
    for id in &targets {
        options.teardrops.set_pad_override(*id, setting);
    }
    if let Err(why) = set_options(app, &path, options, ids::PCB_BOARD_PAD_TEARDROP_UNDO) {
        return refused(why);
    }
    let Some(held) = board(app, &path) else {
        return refused("the board is no longer open");
    };
    let rule = &held.options.teardrops;
    let pads: Vec<Value> = targets
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "teardrop": pad_word(rule.pad_override(*id)),
                "takes_teardrop": rule.pad_wants(*id),
            })
        })
        .collect();
    answered(json!({ "document": path, "pads": pads }))
}

/// The pour edits the operands ask for, in Properties' row order.
fn pour_edits(args: &CommandArgs) -> Result<Vec<ZoneEdit>, String> {
    let mut edits = Vec::new();
    if let Some(priority) = number(args, "priority")? {
        edits.push(ZoneEdit::Priority(priority));
    }
    if let Some(area) = number(args, "islands_below")? {
        edits.push(ZoneEdit::MinIslandArea(area));
    }
    if let Some(on) = flag(args, "dead_copper") {
        edits.push(ZoneEdit::RemoveDeadIslands(on));
    }
    if edits.is_empty() {
        return Err("`set_pour_properties` needs at least one of `priority`, \
                    `islands_below` and `dead_copper`"
            .to_string());
    }
    Ok(edits)
}

/// `set_pour_properties document=.. ids=.. [priority=..] [islands_below=..]
/// [dead_copper=..]`: polygon pours only, as their Properties rows.
pub(super) fn set_pour_properties(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match named_board(app, args, "`set_pour_properties`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let targets = match uuids(args) {
        Ok(targets) => targets,
        Err(reason) => return refused(reason),
    };
    let edits = match pour_edits(args) {
        Ok(edits) => edits,
        Err(reason) => return refused(reason),
    };
    let Some(held) = board(app, &path) else {
        return refused("the board is no longer open");
    };
    let is_pour = |id: &Uuid| held.zones.iter().any(|z| z.uuid == *id && z.is_pour());
    if let Some(id) = targets.iter().find(|id| !is_pour(id)) {
        return refused(format!("{id} is not a polygon pour of the board"));
    }
    let command = match zone_edits_command(held, &targets, &edits) {
        Ok(command) => command,
        Err(why) => return refused(why.to_string()),
    };
    if let Some(command) = command {
        let label = t!(app.ui_state.i18n, ids::PCB_PRIM_POUR_TITLE).into_string();
        if let Err(why) = app.run_board_step(&path, vec![command], &label) {
            return refused(why);
        }
    }
    let pours: Vec<Value> = board(app, &path)
        .map(|b| {
            b.zones
                .iter()
                .filter(|z| targets.contains(&z.uuid))
                .map(|z| {
                    json!({
                        "id": z.uuid,
                        "priority": z.priority,
                        "islands_below_mm2": z.min_island_area,
                        "dead_copper": z.remove_dead_islands,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    answered(json!({ "document": path, "pours": pours }))
}

#[cfg(test)]
#[path = "board_rule_operand_tests.rs"]
mod tests;
