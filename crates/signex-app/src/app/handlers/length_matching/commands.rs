//! The Length Matching commands with operands (#2797): `length_matching`
//! answers the board's groups, `set_length_group` makes or changes one and
//! `remove_length_group` removes one, each edit one board undo step on the
//! same path the tab's fields take.

use std::path::Path;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::pcb::length_match::{self, GroupEdit, GroupReport};
use signex_types::pcb_length::LengthTopology;

use crate::app::command::operand_mm::{operand_mm, operand_mm_or_zero};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::pcb_tune::{outcome, text_operand};
use crate::app::{Message, Signex};

/// `length_matching document=<board>`: every group with its members'
/// routed lengths against the band.
pub(crate) fn length_matching(
    app: &mut Signex,
    args: &CommandArgs,
) -> (CommandOutcome, Task<Message>) {
    outcome(report_answer(app, args))
}

fn report_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let path = app.named_output_board(args)?;
    Ok(groups_answer(app, &path, None))
}

/// `set_length_group document=<board> name=<group> [rename=<name>]
/// [target=<mm>] [tolerance=<mm>] [topology=<word>] [members=<a,b,..>]`:
/// the group as it is now.
pub(crate) fn set_length_group(
    app: &mut Signex,
    args: &CommandArgs,
) -> (CommandOutcome, Task<Message>) {
    outcome(set_answer(app, args))
}

fn set_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let path = app.named_output_board(args)?;
    let name = text_operand(args, "name").ok_or("name the group with `name`")?;
    let topology = match text_operand(args, "topology") {
        Some(word) => Some(
            LengthTopology::from_token(word)
                .ok_or_else(|| format!("`topology` is star, chain, fly_by or t, not `{word}`"))?,
        ),
        None => None,
    };
    let edit = GroupEdit {
        name: name.to_string(),
        rename: text_operand(args, "rename").map(str::to_string),
        target_mm: mm_operand(args, "target", operand_mm, "above 0 mm")?,
        tolerance_mm: mm_operand(args, "tolerance", operand_mm_or_zero, "0 mm or more")?,
        topology,
        members: text_operand(args, "members")
            .map(|list| list.split(',').map(|m| m.trim().to_string()).collect()),
    };
    app.set_length_group_on(&path, &edit)?;
    let now = edit.rename.as_deref().unwrap_or(name).trim().to_string();
    Ok(groups_answer(app, &path, Some(&now)))
}

/// `remove_length_group document=<board> name=<group>`: the groups left.
pub(crate) fn remove_length_group(
    app: &mut Signex,
    args: &CommandArgs,
) -> (CommandOutcome, Task<Message>) {
    outcome(remove_answer(app, args))
}

fn remove_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let path = app.named_output_board(args)?;
    let name = text_operand(args, "name").ok_or("name the group with `name`")?;
    app.remove_length_group_on(&path, name)?;
    let mut answer = groups_answer(app, &path, None);
    answer["removed"] = json!(name);
    Ok(answer)
}

/// `name` as a length in mm, read by `read` (one of the `operand_mm`
/// readers; `band` says what it takes); `None` when not given.
fn mm_operand(
    args: &CommandArgs,
    name: &str,
    read: fn(&str) -> Option<f64>,
    band: &str,
) -> Result<Option<f64>, String> {
    let Some(text) = text_operand(args, name) else {
        return Ok(None);
    };
    read(text)
        .map(Some)
        .ok_or_else(|| format!("`{name}` is a length {band} the board can store, not `{text}`"))
}

/// The board's groups as JSON, or only the group called `only`.
fn groups_answer(app: &Signex, path: &Path, only: Option<&str>) -> Value {
    let groups: Vec<Value> = app
        .document_state
        .pcb_board(path)
        .map(length_match::report)
        .unwrap_or_default()
        .iter()
        .filter(|report| only.is_none_or(|name| report.group.name == name))
        .map(group_json)
        .collect();
    json!({
        "document": path.display().to_string(),
        "groups": groups,
    })
}

fn group_json(report: &GroupReport) -> Value {
    let group = &report.group;
    let members: Vec<Value> = report
        .members
        .iter()
        .map(|m| {
            json!({
                "net": m.net,
                "length_mm": m.length_mm,
                "delta_mm": m.delta_mm,
                "status": m.status.token(),
            })
        })
        .collect();
    json!({
        "name": group.name,
        "rule": group.rule_name(),
        "target_mm": group.target_mm,
        "tolerance_mm": group.tolerance_mm,
        "topology": group.topology.token(),
        "out_of_band": report.out_of_band(),
        "unrouted": report.unrouted(),
        "members": members,
    })
}
