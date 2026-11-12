//! The commands that run their operands (#1934, #2155): a command acts on the
//! operands a caller parsed ([`CommandArgs::parse_operands`]) only if it has
//! a handler here. One that declares operands and has none is refused
//! ([`Signex::run_operands`]), so no caller is ever told `Ran` for operands
//! nobody read. There is no flag to switch: a handler is code that reads them.
//!
//! A handler runs the same app commit path as the click or the button of the
//! same id, so one command id has one behaviour on every surface. A command
//! that names a document (`document_operands`) acts on that document alone and
//! is not gated by what the active tab can do.

use iced::Task;

use super::agent_edits as agent;
use super::document_operands as doc;
use super::file_operands as file;
use super::hierarchy_operands as hierarchy;
use super::part_operands as part;
use super::placement_operands as place;
use super::{CommandArgs, CommandOutcome, enablement};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// What running one command from its operands did, and any follow-up task.
type Ran = (CommandOutcome, Task<Message>);

/// Runs one command from its parsed operands.
pub(crate) type OperandHandler = fn(&mut Signex, &CommandArgs) -> Ran;

/// Every command that runs its operands, by id. Each needs a click-equivalence
/// test (`operand_handler_tests`, `placement_operand_tests`);
/// `every_operand_command_has_a_handler_or_is_refused`
/// keeps this table and the catalog in step.
const HANDLERS: &[(&str, OperandHandler)] = &[
    (
        "new_project_with_profile",
        crate::app::new_project::new_project_with_profile,
    ),
    ("color_net", doc::color_net),
    ("clear_all_net_colors", doc::clear_all_net_colors),
    ("undo", doc::undo),
    ("redo", doc::redo),
    ("save_document", doc::save),
    ("save_document_as", doc::save_as),
    ("close_active_document", doc::close),
    ("open_document", file::open),
    ("new_document", file::new_project),
    (
        "new_pcb_document",
        crate::app::document_setup::new_pcb_document,
    ),
    (
        "new_schematic_sheet",
        crate::app::document_setup::new_schematic_sheet,
    ),
    ("assign_net_class", doc::assign_net_class),
    (
        "update_pcb_from_schematic",
        crate::app::update_pcb::update_pcb_from_schematic,
    ),
    ("apply_edits", agent::apply_edits),
    ("revert", agent::revert),
    ("place_net_label", place::net_label),
    ("place_local_net_label", place::net_label),
    ("place_global_label", place::global_label),
    ("place_hierarchical_label", place::hierarchical_label),
    ("place_power_symbol", place::power),
    ("place_no_connect", place::no_connect),
    ("place_junction", place::junction),
    ("place_no_erc", place::no_erc),
    ("place_wire", place::wire),
    ("draw_wire", place::wire),
    ("place_bus", place::bus),
    ("draw_bus", place::bus),
    ("place_text", place::text_note),
    ("place_text_string", place::text_note),
    ("draw_line", place::line),
    ("draw_graphic_line", place::line),
    ("draw_rectangle", place::rectangle),
    ("place_component", part::place_component),
    ("place_sheet_symbol", hierarchy::place_sheet_symbol),
    ("place_sheet_entry", hierarchy::place_sheet_entry),
    ("sync_sheet_pins", hierarchy::sync_sheet_pins),
];

/// The handler of `command`, if it runs its operands.
pub(crate) fn operand_handler(command: &AppCommandId) -> Option<OperandHandler> {
    HANDLERS
        .iter()
        .find(|(id, _)| *id == command.as_str())
        .map(|(_, handler)| *handler)
}

/// Commands a headless run hands to their handler even with no operands:
/// the GUI form opens a setup dialog, the headless form makes the document
/// from the project's own setup (#2232), as it did before the dialog.
pub(crate) fn runs_headless_without_operands(command: &AppCommandId) -> bool {
    matches!(command.as_str(), "new_pcb_document" | "new_schematic_sheet")
}

/// Why `command` does nothing without operands on a surface that gives it
/// none (palette, key binding, menu): its GUI surface is another widget, and
/// a command never opens, closes or focuses one as a side effect.
pub(crate) fn needs_operands_reason(command: &AppCommandId) -> Option<&'static str> {
    match command.as_str() {
        "assign_net_class" => Some(
            "`assign_net_class` needs the operand `net_name` — \
             use the Net Class row in Properties",
        ),
        "apply_edits" => Some(
            "`apply_edits` needs the operands `document`, `expect_revision` and `edits` — \
             it has no surface but an agent's call",
        ),
        "revert" => Some(
            "`revert` needs the operands `document` and `group` — \
             Undo is the person's way back",
        ),
        _ => None,
    }
}

/// The ids that run their operands.
#[cfg(test)]
pub(super) fn handled_ids() -> impl Iterator<Item = &'static str> {
    HANDLERS.iter().map(|(id, _)| *id)
}

/// Does `command` always name its document (a required `document` operand)?
/// A placement takes an optional one and keeps the active tab's gate without it.
pub(crate) fn names_its_document(command: &AppCommandId) -> bool {
    crate::keymap::metadata_for(command).is_some_and(|meta| {
        meta.operands
            .iter()
            .any(|op| op.name == "document" && op.required)
    })
}

impl Signex {
    /// Run `command` from the operands in `args`: its handler. A command with
    /// no handler is refused. One that names its document (`document`) is
    /// run on that document, with ids made unique first and without the
    /// active tab's enablement; any other is behind the same enablement gate
    /// as a dispatch.
    pub(crate) fn run_operands(&mut self, command: &AppCommandId, args: &CommandArgs) -> Ran {
        let Some(handler) = operand_handler(command) else {
            return refused("operands are declared but not run yet (#1934)");
        };
        if names_its_document(command) || args.operand("document").is_some() {
            self.reconcile_document_ids();
        } else if let Some(reason) =
            enablement::disabled_reason(command, &self.command_context(None))
        {
            return refused(reason);
        }
        // What its checks read from disk is read off the UI thread first, and
        // the command runs when that lands (#2199).
        if let Some(deferred) = self.defer_for_reads(command, args) {
            return deferred;
        }
        self.document_state.agent_edits.project_wanted.set(false);
        // The socket only: a headless run has no person, and its own `undo`
        // refuses a grouped agent step (docs/AGENT_EDITS.md, "Origin").
        let ran = if self.agent.serving && agent::runs_as_one_step(command) {
            agent::run_as_one_step(self, command, args, handler)
        } else {
            handler(self, args)
        };
        // A check found that it needs the project's closed sheets: it refused
        // with nothing applied; read them and run the command again.
        if self.document_state.agent_edits.project_wanted.take()
            && matches!(ran.0, CommandOutcome::Disabled { .. })
            && let Some(deferred) = self.defer_for_project(command, args)
        {
            return deferred;
        }
        ran
    }
}

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}
