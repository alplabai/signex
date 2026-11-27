//! `change_component` from operands (#1297, #2954): the placed part `id` on
//! the sheet `document` swapped for a library row (`library` and `part`, as
//! `place_component` names one), the place-or-swap form behind
//! `search_parts`. It is the dialog's Change: one `ChangeComponent` engine
//! command, one undo step, with the pins mapped by number and name or by
//! hand (`pins=1:2,3:-`) and every user parameter kept but those `drop`
//! names; refused for a locked part, a part whose units are on other sheets,
//! a row with no drawing for the placed unit, or a hand mapping whose
//! carried wires would join another net. `dry_run=true` answers and changes
//! nothing. The row is read off the UI thread first (`edit_reads`), as for
//! `place_component`. Shape: `docs/CLI.md`, "Changing a part's component".

use std::path::Path;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::{
    Command, ComponentReplacement, change_component_preview, pin_choices, pin_table,
};
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use super::placement_operands::{run_edit, text};
use super::{CommandArgs, CommandOutcome, OperandValue, library_row};
use crate::app::handlers::placed_part::{part_name, replacement_of};
use crate::app::mutation_gateway::MutationOutcome as Applied;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// `change_component document=.. id=.. library=.. part=.. [pins=old:new,..]
/// [drop=Name,..] [dry_run=true]`.
pub(super) fn change_component(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| app.change_component_from_operands(args))
}

/// `pins=1:2,3:-`: each old pin and the new pin it goes to, `-` for none.
pub(super) fn pin_pairs(given: &str) -> Result<Vec<(String, Option<String>)>, String> {
    given
        .split(',')
        .map(str::trim)
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (old, new) = pair
                .split_once(':')
                .map(|(old, new)| (old.trim(), new.trim()))
                .filter(|(old, new)| !old.is_empty() && !new.is_empty())
                .ok_or_else(|| {
                    format!("`pins` pairs are old:new (or old:- for no pin), not `{pair}`")
                })?;
            Ok((old.to_string(), (new != "-").then(|| new.to_string())))
        })
        .collect()
}

/// The names of a comma-separated list, trimmed, each once.
fn names(given: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in given.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        if !out.iter().any(|o| o == name) {
            out.push(name.to_string());
        }
    }
    out
}

/// `replacement` with the hand mapping `pins` gives, checked against the
/// part's pins and the new part's.
fn mapped_by_hand(
    doc: &SchematicSheet,
    id: Uuid,
    mut replacement: ComponentReplacement,
    pins: &str,
) -> Result<ComponentReplacement, String> {
    let rows = pin_table(doc, id, &replacement);
    let choices = pin_choices(doc, id, &replacement);
    let mut map = change_component_preview(doc, id, &replacement).mapping;
    for (old, new) in pin_pairs(pins)? {
        if !rows.iter().any(|r| r.old == old) {
            return Err(format!("the part has no pin {old}"));
        }
        if let Some(new) = &new
            && !choices.iter().any(|c| c.key == *new)
        {
            return Err(format!("the new part has no pin {new}"));
        }
        map = map.remapped(&old, new.as_deref());
    }
    replacement.pin_map = Some(map);
    Ok(replacement)
}

/// One row per old pin: the new pin it goes to and the net on it.
fn mapping(doc: &SchematicSheet, id: Uuid, replacement: &ComponentReplacement) -> Vec<Value> {
    pin_table(doc, id, replacement)
        .into_iter()
        .map(|row| {
            let (new, new_name) = row.new.unzip();
            json!({
                "old": row.old,
                "old_name": row.old_name,
                "new": new,
                "new_name": new_name,
                "net": row.net,
            })
        })
        .collect()
}

impl Signex {
    fn change_component_from_operands(
        &mut self,
        args: &CommandArgs,
    ) -> Result<CommandOutcome, String> {
        let given = text(args, "id")?;
        let symbol_id = Uuid::parse_str(given.trim())
            .map_err(|_| format!("`id` is the uuid of a placed part, not `{given}`"))?;
        let sheet = self
            .document_state
            .edit_path()
            .map(Path::to_path_buf)
            .ok_or("no schematic is open")?;
        let library = self.open_library_named(&text(args, "library")?)?;
        let (table, row_id) = library_row(library, &text(args, "part")?)?;
        let resolved = self.prefetched_part(&(library.root.clone(), table, row_id))?;
        if resolved.is_power {
            return Err(format!(
                "{} is a power symbol: pick a component",
                resolved.internal_pn
            ));
        }
        let engine = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?;
        let doc = engine.document();
        let symbol = doc
            .symbols
            .iter()
            .find(|s| s.uuid == symbol_id && !s.is_power)
            .ok_or_else(|| format!("the sheet has no placed part with id {symbol_id}"))?;
        let name = part_name(symbol);
        if let Some(advice) = self.change_blocker_on(&sheet, doc, symbol_id, symbol) {
            return Err(advice);
        }
        let mut replacement = replacement_of(&resolved);
        if let Ok(pins) = text(args, "pins") {
            replacement = mapped_by_hand(doc, symbol_id, replacement, &pins)?;
        }
        let preview = change_component_preview(doc, symbol_id, &replacement);
        if let Some(unit) = preview.missing_unit {
            return Err(format!(
                "{} has no drawing for unit {unit}, so {name} was not changed",
                resolved.internal_pn
            ));
        }
        if let Some(why) = &preview.refused {
            return Err(why.clone());
        }
        let user = replacement.user_parameters(doc, symbol_id);
        let dropped = names(&text(args, "drop").unwrap_or_default());
        if let Some(missing) = dropped.iter().find(|d| !user.contains(d)) {
            return Err(format!(
                "`{missing}` is not a parameter of {name} the new part leaves; user parameters: {}",
                user.join(", ")
            ));
        }
        let kept: Vec<String> = user.into_iter().filter(|p| !dropped.contains(p)).collect();
        let dry_run = matches!(args.operand("dry_run"), Some(OperandValue::Bool(true)));
        let mut answer = json!({
            "designator": name,
            "id": symbol_id,
            "changed": false,
            "from": {
                "lib_id": symbol.lib_id,
                "library": symbol.library_id,
                "part": symbol.row_id,
            },
            "to": {
                "lib_id": resolved.lib_id,
                "library": resolved.library_id,
                "part": resolved.row_id,
                "internal_pn": resolved.internal_pn,
            },
            "pins": {
                "dangling": preview.unmatched_connected,
                "moved": preview.moved_connected,
                "carried": preview.carried,
                "gained": preview.gained,
            },
            "mapped_by_hand": replacement.pin_map.is_some(),
            "mapping": mapping(doc, symbol_id, &replacement),
            "kept": kept,
            "dropped": dropped,
            "dry_run": dry_run,
        });
        if dry_run {
            return Ok(CommandOutcome::Answered(answer));
        }
        let command = Command::ChangeComponent {
            symbol_id,
            replacement: replacement.keeping_parameters(doc, symbol_id, &kept),
        };
        if let Some(reason) = engine.lock_refusal(&command) {
            return Err(reason);
        }
        let (outcome, reason) = self.apply_engine_commands_reporting(vec![command], true, true);
        if outcome == Applied::Failed {
            return Err(reason.unwrap_or_else(|| format!("{name} was not changed")));
        }
        answer["changed"] = json!(outcome == Applied::Changed);
        Ok(CommandOutcome::Answered(answer))
    }
}
