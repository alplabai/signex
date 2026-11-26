//! `change_component` from operands (#1297): the placed part `id` on the
//! sheet `document` swapped for a library row (`library` and `part`, as
//! `place_component` names one), the place-or-swap form behind
//! `search_parts`. It is the dialog's Change with every Keep now box
//! ticked: one `ChangeComponent` engine command, one undo step, refused for
//! a locked part, a part whose units are on other sheets, or a row with no
//! drawing for the placed unit. The row is read off the UI thread first
//! (`edit_reads`), as for `place_component`. Shape: `docs/CLI.md`, "BOM and
//! part search".

use std::path::Path;

use iced::Task;
use serde_json::json;
use signex_engine::{Command, change_component_preview};
use uuid::Uuid;

use super::placement_operands::{run_edit, text};
use super::{CommandArgs, CommandOutcome, library_row};
use crate::app::handlers::placed_part::{part_name, replacement_of};
use crate::app::mutation_gateway::MutationOutcome as Applied;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// `change_component document=.. id=.. library=.. part=..`.
pub(super) fn change_component(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| app.change_component_from_operands(args))
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
        let doc = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?
            .document();
        let symbol = doc
            .symbols
            .iter()
            .find(|s| s.uuid == symbol_id && !s.is_power)
            .ok_or_else(|| format!("the sheet has no placed part with id {symbol_id}"))?;
        let name = part_name(symbol);
        if let Some(advice) = self.change_blocker_on(&sheet, doc, symbol_id, symbol) {
            return Err(advice);
        }
        let replacement = replacement_of(&resolved);
        let preview = change_component_preview(doc, symbol_id, &replacement);
        if let Some(unit) = preview.missing_unit {
            return Err(format!(
                "{} has no drawing for unit {unit}, so {name} was not changed",
                resolved.internal_pn
            ));
        }
        let from = json!({
            "lib_id": symbol.lib_id,
            "library": symbol.library_id,
            "part": symbol.row_id,
        });
        let kept = replacement.user_parameters(doc, symbol_id);
        let command = Command::ChangeComponent {
            symbol_id,
            replacement: replacement.keeping_parameters(doc, symbol_id, &kept),
        };
        let (outcome, reason) = self.apply_engine_commands_reporting(vec![command], true, true);
        if outcome == Applied::Failed {
            return Err(reason.unwrap_or_else(|| format!("{name} was not changed")));
        }
        Ok(CommandOutcome::Answered(json!({
            "designator": name,
            "id": symbol_id,
            "changed": outcome == Applied::Changed,
            "from": from,
            "to": {
                "lib_id": resolved.lib_id,
                "library": resolved.library_id,
                "part": resolved.row_id,
                "internal_pn": resolved.internal_pn,
            },
            "pins": {
                "dangling": preview.unmatched_connected,
                "moved": preview.moved_connected,
                "gained": preview.gained,
            },
        })))
    }
}
