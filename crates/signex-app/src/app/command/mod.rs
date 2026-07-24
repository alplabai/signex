//! Home of the Command Registry (#278). Slice 1 landed the
//! id→[`crate::app::Message`] bridge (`bridge.rs`). Slice 2 adds the
//! registry's dispatch entry point ([`Signex::dispatch_command`]) and
//! its argument type ([`CommandArgs`]); enablement gating (slice 3) and
//! rewiring menus/palette onto it (slice 4) are still ahead.

mod args;
pub(crate) mod bridge;

pub(crate) use args::CommandArgs;
use bridge::core_to_message;

use iced::Task;

use super::*;

impl Signex {
    /// The Command Registry's dispatch entry point — resolve a stable
    /// [`crate::keymap::AppCommandId`] and run it exactly as if the
    /// resolved [`Message`] had been sent directly.
    ///
    /// Deliberately takes an id + args, never a [`Message`] — the whole
    /// point of the registry is that a caller (keyboard today; menu,
    /// command palette, and a future CLI later) names *what* to run
    /// without knowing the app's internal message shape.
    ///
    /// `args` is accepted but unused: [`core_to_message`] ignores it,
    /// because every catalog command is nullary today. It is here only
    /// so this signature is stable ahead of a consumer that needs it —
    /// do not thread it through `core_to_message` speculatively.
    ///
    /// An id can fail to resolve two ways, and both are a silent no-op,
    /// never a panic: it isn't in the catalog at all, or it's a real
    /// catalog id with no live arm in `core_to_message` yet. Ids come
    /// from user-editable TOML keymap profiles today and a CLI later —
    /// both are untrusted input, not a programmer error.
    pub(crate) fn dispatch_command(
        &mut self,
        command: &crate::keymap::AppCommandId,
        _args: CommandArgs,
    ) -> Task<Message> {
        match core_to_message(command) {
            Some(message) => self.dispatch_update(message),
            None => Task::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::AppCommandId;
    use crate::schematic_runtime::hit_test::SelectionMode;

    /// A known id resolves through the bridge and reaches the right
    /// handler — observed here via its state mutation rather than by
    /// comparing `Message`/`Task` values.
    #[test]
    fn known_id_dispatches_to_the_right_message() {
        let (mut app, _task) = Signex::new();
        app.ui_state.selection_mode = SelectionMode::Touching;

        let command = AppCommandId::new("cycle_selection_mode").unwrap();
        let _ = app.dispatch_command(&command, CommandArgs::none());

        assert_eq!(app.ui_state.selection_mode, SelectionMode::Inside);
    }

    /// An id with no catalog entry at all is untrusted input, not a
    /// programmer error — it must resolve to a no-op, never panic.
    #[test]
    fn unknown_id_is_a_no_op_and_does_not_panic() {
        let (mut app, _task) = Signex::new();

        let command = AppCommandId::new("not_a_real_command").unwrap();
        let task = app.dispatch_command(&command, CommandArgs::none());

        assert_eq!(task.units(), 0);
    }

    /// `clear_net_highlighting` is a real catalog entry
    /// (`keymap/catalog/schematic.rs`) with no arm in `core_to_message`
    /// yet — it must resolve to a no-op too, not panic.
    #[test]
    fn catalog_id_with_no_bridge_arm_is_a_no_op_and_does_not_panic() {
        let (mut app, _task) = Signex::new();

        let command = AppCommandId::new("clear_net_highlighting").unwrap();
        assert!(
            crate::keymap::metadata_for(&command).is_some(),
            "test fixture drifted: clear_net_highlighting is no longer a catalog id"
        );

        let task = app.dispatch_command(&command, CommandArgs::none());

        assert_eq!(task.units(), 0);
    }
}
