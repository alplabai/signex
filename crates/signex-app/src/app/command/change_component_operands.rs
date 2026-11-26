//! `change_component` from operands (#2954): the placed part `id` swapped for
//! a library row, as Change Component's Change applies it (one engine
//! command, one undo step), with the pins mapped by number and name or by
//! hand (`pins=1:2,3:-`) and the user parameters `drop` names cleared. The
//! answer is the pin mapping (old pin, new pin, net) and what the swap does
//! to the wires; `dry_run=true` answers it and changes nothing. The row is
//! read off the UI thread first (`edit_reads`, as for `place_component`).
//! Rules and refusals: `docs/CLI.md`, "Changing a part's component".

use iced::Task;
use serde_json::{Value, json};
use signex_engine::{
    Command, ComponentChangePreview, ComponentReplacement, change_component_preview, pin_choices,
    pin_table,
};
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use super::part_operands::library_row;
use super::placement_operands::{run_edit, text};
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::placed_part::replacement_of;
use crate::app::mutation_gateway::MutationOutcome as Applied;
use crate::app::{Message, Signex};
use crate::library::messages::PlacedPartTarget;

type Ran = (CommandOutcome, Task<Message>);

/// `change_component id=.. library=.. part=.. [pins=old:new,..]
/// [drop=Name,..] [dry_run=true] [document=..]`, on the named `document`,
/// else the active sheet.
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

/// The answer: the mapping and what the swap does to the wires.
fn answer(
    reference: &str,
    part: &str,
    doc: &SchematicSheet,
    id: Uuid,
    replacement: &ComponentReplacement,
    preview: &ComponentChangePreview,
    (kept, dropped): (&[String], &[String]),
) -> Value {
    let pins: Vec<Value> = pin_table(doc, id, replacement)
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
        .collect();
    json!({
        "designator": reference,
        "part": part,
        "mapped_by_hand": replacement.pin_map.is_some(),
        "pins": pins,
        "dangling": preview.unmatched_connected,
        "moved": preview.moved_connected,
        "carried": preview.carried,
        "gained": preview.gained,
        "kept": kept,
        "dropped": dropped,
    })
}

impl Signex {
    fn change_component_from_operands(
        &mut self,
        args: &CommandArgs,
    ) -> Result<CommandOutcome, String> {
        let given = text(args, "id")?;
        let id = Uuid::parse_str(given.trim())
            .map_err(|_| format!("`id` must be a part id (a uuid), got `{given}`"))?;
        let sheet = self
            .document_state
            .edit_path()
            .map(std::path::Path::to_path_buf)
            .ok_or("no schematic is open")?;
        let engine = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?;
        let doc = engine.document();
        let symbol = doc
            .symbols
            .iter()
            .find(|s| s.uuid == id && !s.is_power)
            .ok_or_else(|| format!("no placed part {id} on this sheet"))?;
        let target = PlacedPartTarget {
            sheet,
            engine_instance: engine.instance_id(),
            symbol_id: id,
        };
        if let Some(advice) = self.change_blocker(&target, symbol) {
            return Err(advice);
        }
        let reference = symbol.reference.clone();
        let library = self.open_library_named(&text(args, "library")?)?;
        let (table, row_id) = library_row(library, &text(args, "part")?)?;
        let resolved = self.prefetched_part(&(library.root.clone(), table, row_id))?;
        if resolved.is_power {
            return Err(format!(
                "{} is a power symbol; pick a component",
                resolved.internal_pn
            ));
        }
        let mut replacement = replacement_of(&resolved);
        if let Ok(pins) = text(args, "pins") {
            replacement = mapped_by_hand(doc, id, replacement, &pins)?;
        }
        let preview = change_component_preview(doc, id, &replacement);
        if let Some(unit) = preview.missing_unit {
            return Err(format!(
                "the new part has no drawing for unit {unit} of {reference}"
            ));
        }
        if let Some(why) = &preview.refused {
            return Err(why.clone());
        }
        let user = replacement.user_parameters(doc, id);
        let dropped = names(&text(args, "drop").unwrap_or_default());
        if let Some(name) = dropped.iter().find(|name| !user.contains(name)) {
            return Err(format!(
                "`{name}` is not a parameter of {reference} the new part leaves; user parameters: {}",
                user.join(", ")
            ));
        }
        let kept: Vec<String> = user.into_iter().filter(|p| !dropped.contains(p)).collect();
        let part = resolved.internal_pn.as_str();
        let mut answered = answer(
            &reference,
            part,
            doc,
            id,
            &replacement,
            &preview,
            (&kept, &dropped),
        );
        let dry_run = matches!(args.operand("dry_run"), Some(OperandValue::Bool(true)));
        answered["dry_run"] = json!(dry_run);
        if dry_run {
            return Ok(CommandOutcome::Answered(answered));
        }
        let command = Command::ChangeComponent {
            symbol_id: id,
            replacement: replacement.keeping_parameters(doc, id, &kept),
        };
        if let Some(reason) = engine.lock_refusal(&command) {
            return Err(reason);
        }
        match self.apply_engine_commands_reporting(vec![command], true, true) {
            (Applied::Changed | Applied::Unchanged, _) => Ok(CommandOutcome::Answered(answered)),
            (Applied::Failed, reason) => {
                Err(reason.unwrap_or_else(|| "the engine refused the change".to_string()))
            }
        }
    }
}
