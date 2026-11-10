//! `update_pcb_from_schematic apply=true`: the headless form. It reads the
//! design and executes the rows ticked by default, through the same path as
//! the dialog's Execute (one labelled undo step).

use iced::Task;

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (
        CommandOutcome::Disabled {
            reason: reason.into(),
        },
        Task::none(),
    )
}

pub(crate) fn update_pcb_from_schematic(app: &mut Signex, args: &CommandArgs) -> Ran {
    if !matches!(args.operand("apply"), Some(OperandValue::Bool(true))) {
        return refused(
            "`update_pcb_from_schematic` lists its changes in the Update PCB dialog; \
             run it with `apply=true` to execute the default ticks",
        );
    }
    match app.open_update_pcb_from_view(true) {
        Ok(task) => (CommandOutcome::Ran, task),
        Err(why) => refused(why),
    }
}
