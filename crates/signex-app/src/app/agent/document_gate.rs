//! The agent channel's document gate (#1924): a command that is not a query
//! names an open project's document, never the person's active one, and a
//! close never moves the person's tab. A writer's `path` is checked here by
//! its text (`write_target`, #2470). See `docs/AGENT_SOCKET.md`.

use super::allow;
use crate::app::command::queries::is_query;
use crate::app::command::{CommandArgs, OperandValue};
use crate::app::{DocumentKey, Signex};
use crate::keymap::AppCommandId;

impl Signex {
    /// A command that is not a query acts on the document it names, one of an
    /// open project's, and never on the person's active one. Closing it must
    /// not move the person's tab either.
    pub(super) fn check_agent_document(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> Result<(), String> {
        let id = command.as_str();
        self.reconcile_document_ids();
        let target = match args.operand("document") {
            Some(OperandValue::Document(target)) => target,
            None if is_query(command) => return Ok(()),
            _ => {
                return Err(format!(
                    "`{id}` needs an explicit `document`: an agent never acts on the person's active one"
                ));
            }
        };
        let entry = self.resolve_document_target(target)?;
        if entry.project.is_none() {
            return Err(format!(
                "{} is not part of a project open in this app",
                entry.name
            ));
        }
        if allow::writes_in_project(id)
            && let Some(why) = super::write_target::path_refusal(args)
        {
            return Err(why);
        }
        if id == "close_active_document" {
            let active = self
                .document_state
                .tabs
                .get(self.document_state.active_tab)
                .and_then(DocumentKey::of);
            if entry.key.is_some() && entry.key == active {
                return Err(format!(
                    "{} is the person's active document; closing it would move their tab",
                    entry.name
                ));
            }
            if entry.open_in_window {
                return Err(format!(
                    "{} is open in a window of its own; closing it would close that window",
                    entry.name
                ));
            }
            // Closing drops a sheet's drafts of every kind (title block,
            // paper edge): the person's unconfirmed typing is not the agent's
            // to finish or lose.
            if entry
                .path
                .as_ref()
                .is_some_and(|path| self.document_view.drafts.has_sheet(path))
            {
                return Err(
                    "the person is editing this sheet's title block or page settings".to_string(),
                );
            }
        }
        Ok(())
    }
}
