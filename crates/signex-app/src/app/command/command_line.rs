//! The operands of a command named on `signex`'s own command line
//! (`signex notebook generate`, #2945): parsed here, through the same parse
//! as every other caller, and then run through the headless entry.

use super::CommandArgs;
use crate::keymap::AppCommandId;

/// `operands` (`name`, `value`) as the arguments of `command`, or why they
/// do not parse.
pub fn command_line_args(
    command: &AppCommandId,
    operands: &[(String, String)],
) -> Result<CommandArgs, String> {
    let texts: Vec<String> = operands
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    CommandArgs::parse_operands(command, &refs).map_err(|error| error.to_string())
}
