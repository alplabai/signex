//! `query_power_tree document=<project, or one of its documents>
//! [variant=<name>]` (#1313, mockup row 171): the project's power tree
//! generated from its schematic (sources, converters, rails and loads with
//! the current each draws) and the power budget per sheet, for the variant
//! named or else the project's active one. Run off the UI thread
//! (`app::power_tree`). See `docs/POWER_TREE.md`.

use iced::Task;
use signex_types::project::is_base_variant;

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
        let variant = match text_operand(args, "variant") {
            Some(name) => name.to_string(),
            None => project.data.active_variant.clone().unwrap_or_default(),
        };
        let variant = (!variant.trim().is_empty() && !is_base_variant(&variant)).then_some(variant);
        if let Some(name) = &variant
            && !project.data.variant_definitions.iter().any(|v| v == name)
        {
            return refused(format!(
                "`query_power_tree`: the project has no variant `{name}`"
            ));
        }
        let job = self.power_tree_job(project, variant);
        let target = project.path.clone();
        self.answer_off_thread(command, args, target, move || job.run())
    }
}
