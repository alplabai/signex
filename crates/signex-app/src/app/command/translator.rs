//! The GUI translator (ADR-0006, slice 6): a person's command leaves its
//! document and targets implicit, the board in view and its selection. The
//! GUI handler reads them, and this fills them in as the `document` and
//! `ids` operands and runs the same operand handler an agent or the CLI
//! runs, so one command id has one behaviour on every surface.
//!
//! Operands stay private to the `command` module (`args.rs`): a GUI module
//! hands plain values to [`run`] and never builds a `CommandArgs` itself.

use std::collections::BTreeMap;
use std::path::Path;

use iced::Task;
use signex_i18n::{Translatable, ids};

use super::args::Operands;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentTarget;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// The ids whose bridge arm translates: the handler of the message it sends
/// reads the selection and calls [`run`]. The parity table names that row
/// `translator`. `arrange_in_box` translates too, but on its second board
/// click (`pcb_arrange_box_clicked`); its bridge arm only arms the box, so
/// that row stays `bridge`. `swap_pins`, `swap_gates` and
/// `place_board_reuse_block` do not translate yet: their handlers name the
/// part by designator (and a gate by unit), the GUI by uuid (and pin).
#[cfg(test)]
const TRANSLATED: &[&str] = &[
    "board_shape_from_selection",
    "gloss_selected_routes",
    "keepout_from_selection",
    "save_board_reuse_block",
];

/// Whether the bridge arm of the command `id` is a translator.
#[cfg(test)]
pub(crate) fn is_translated(id: &str) -> bool {
    TRANSLATED.contains(&id)
}

/// Run `id`'s operand handler on the document at `document` with
/// `operands`, filled in by a GUI handler from what the person has in view,
/// through `run_operands`, the entry an agent or the CLI takes. The outcome
/// is `Answered`, `Disabled` or `Deferred`; anything else is turned into a
/// refusal. `every_translated_id_has_an_operand_handler` keeps an id
/// without a handler out.
pub(crate) fn run(
    app: &mut Signex,
    id: &str,
    document: &Path,
    operands: Vec<(&'static str, OperandValue)>,
) -> Ran {
    let Ok(command) = AppCommandId::new(id) else {
        return (not_available(), Task::none());
    };
    // A path that is not Unicode resolves to no document, and the handler
    // refuses it as it refuses an agent naming it.
    let target = DocumentTarget::Path(document.to_string_lossy().into_owned());
    let mut values = BTreeMap::from([("document".to_string(), OperandValue::Document(target))]);
    values.extend(operands.into_iter().map(|(n, v)| (n.to_string(), v)));
    let args = CommandArgs {
        context: Default::default(),
        operands: Operands {
            values,
            texts: BTreeMap::new(),
        },
    };
    // The person's command, not an agent's: every open document is in
    // reach (`document_entries`) and no agent undo step wraps it.
    let serving = std::mem::replace(&mut app.agent.serving, false);
    let (outcome, task) = app.run_operands(&command, &args);
    app.agent.serving = serving;
    let outcome = match outcome {
        CommandOutcome::Answered(_)
        | CommandOutcome::Disabled { .. }
        | CommandOutcome::Deferred { .. } => outcome,
        _ => not_available(),
    };
    (outcome, task)
}

fn not_available() -> CommandOutcome {
    CommandOutcome::Disabled {
        reason: Translatable::new(ids::NOTICE_NOT_AVAILABLE),
    }
}

#[cfg(test)]
mod tests {
    use super::super::operand_handlers;
    use super::*;

    #[test]
    fn every_translated_id_has_an_operand_handler() {
        for id in TRANSLATED {
            let command = AppCommandId::new(*id).expect("catalog id");
            assert!(
                operand_handlers::operand_handler(&command).is_some(),
                "{id}"
            );
        }
    }
}
