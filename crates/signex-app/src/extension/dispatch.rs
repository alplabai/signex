//! An extension's catalog command on the shared dispatch path: every surface
//! (keymap, palette, menu, a headless run) reaches it through
//! `Signex::dispatch_command_reporting`, which hands an `ext.` id here.

use iced::Task;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

impl Signex {
    /// Run the installed extension command `command`: its handler, then its
    /// owner's `update` with the message the handler made. `None` for an id
    /// no installed extension added, which then takes the path of any id the
    /// catalog does not have.
    pub(crate) fn dispatch_extension_command(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> Option<(CommandOutcome, Task<Message>)> {
        let handled = self.extensions.command(command.as_str())?.handle(args);
        Some(match handled {
            Ok(message) => (
                CommandOutcome::Ran,
                self.dispatch_update(Message::Extension(message)),
            ),
            Err(reason) => (CommandOutcome::Disabled { reason }, Task::none()),
        })
    }
}
