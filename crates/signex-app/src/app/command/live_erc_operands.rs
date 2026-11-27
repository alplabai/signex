//! `toggle_live_erc [enabled=..]` (#1288): turn live ERC on or off for a CLI
//! run or an agent, as the key or palette row does. Without `enabled` it
//! flips the preference. Answers with the preference as it is after.

use iced::Task;
use serde_json::json;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// `toggle_live_erc [enabled=true|false]`: the `live_erc` preference, saved.
/// Answers `{"enabled": <after>, "changed": <whether it moved>}`.
pub(super) fn toggle_live_erc(app: &mut Signex, args: &CommandArgs) -> Ran {
    let before = app.ui_state.live_erc.enabled;
    let after = match args.operand("enabled") {
        Some(OperandValue::Bool(enabled)) => *enabled,
        _ => !before,
    };
    let write = app.set_live_erc(after);
    let answer = json!({ "enabled": after, "changed": after != before });
    (CommandOutcome::Answered(answer), write)
}
