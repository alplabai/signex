//! `query_power_tree document=<project, or one of its documents>
//! [variant=<name>]` (#1313, mockup row 171): the project's power tree
//! generated from its schematic (sources, converters, rails and loads with
//! the current each draws) and the power budget per sheet, for the variant
//! named or else the project's active one. Run off the UI thread
//! (`app::power_tree`). See `docs/POWER_TREE.md`.

use iced::Task;

use super::{refused, text_operand};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

impl Signex {
    /// Start the run `args` asks for; refused at once when the operands
    /// name no open project or a variant the project does not have.
    pub(super) fn query_power_tree(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        let id = match self.rules_project_named(args) {
            Ok(id) => id,
            Err(reason) => return refused(format!("`query_power_tree` {reason}")),
        };
        let Some(project) = self.document_state.project_by_id(id) else {
            return refused("`query_power_tree`: the project is no longer open".into());
        };
        let named = text_operand(args, "variant");
        let variant = match crate::app::power_tree::power_tree_variant(project, named) {
            Ok(variant) => variant,
            Err(reason) => return refused(format!("`query_power_tree`: {reason}")),
        };
        let job = self.power_tree_job(project, variant);
        let target = project.path.clone();
        self.answer_off_thread(command, args, target, power_tree_unread, move || job.run())
    }
}

/// Why the run gave no power tree, as the tab words it.
fn power_tree_unread(reason: String) -> signex_i18n::Translatable {
    signex_i18n::Translatable::new(signex_i18n::ids::BLOCK_DIAGRAM_POWER_UNREAD)
        .arg("reason", reason)
}
