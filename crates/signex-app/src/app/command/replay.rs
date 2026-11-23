//! The sanctioned entries that run a command's operands from outside this
//! module (#2731): Command Log Replay and the headless front end. Each states
//! its origin, and `scripts/check_command_entry.sh` keeps every other caller
//! out. Replay's rules: `docs/COMMAND_LOG.md`.

use iced::Task;
use serde_json::{Map, Value};

use super::{CommandArgs, CommandOutcome, OperandError};
use crate::app::agent::Origin;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

impl Signex {
    /// Replay a logged row's operands as the person's command, logged as a
    /// new row with `operands`. `Err` when the recorded texts no longer
    /// parse: nothing ran. The caller has already passed Replay's gate, which
    /// refuses a read-only query row (DECISIONS 2026-11-23).
    pub(crate) fn replay_operands(
        &mut self,
        command: &AppCommandId,
        operands: Map<String, Value>,
        texts: &[String],
    ) -> Result<Task<Message>, OperandError> {
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let args = CommandArgs::parse_operands(command, &refs)?;
        let (_, task) = self.logged_command(command, operands, None, |app| {
            app.with_origin(Origin::Person, |app| app.run_operands(command, &args))
        });
        Ok(task)
    }

    /// The headless front end's operand run, under the agent origin its
    /// caller set, journaled like any agent edit.
    pub(crate) fn run_headless_operands(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        self.journaled(command, args, |app| app.run_operands(command, args))
    }
}
