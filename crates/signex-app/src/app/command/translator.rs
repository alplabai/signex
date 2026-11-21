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

use super::args::Operands;
use super::{CommandArgs, CommandOutcome, OperandValue, operand_handlers};
use crate::app::document_ids::DocumentTarget;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// The ids whose bridge arm translates: the handler of the message it sends
/// reads the selection and calls [`run`]. The parity table names that row
/// `translator`.
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
/// `operands`, filled in by a GUI handler from what the person has in view.
/// An id without a handler runs nothing (`NoBridgeArm`);
/// `every_translated_id_has_an_operand_handler` keeps that from happening.
pub(crate) fn run(
    app: &mut Signex,
    id: &str,
    document: &Path,
    operands: Vec<(&'static str, OperandValue)>,
) -> Ran {
    let handler = AppCommandId::new(id)
        .ok()
        .and_then(|command| operand_handlers::operand_handler(&command));
    let Some(handler) = handler else {
        return (CommandOutcome::NoBridgeArm, Task::none());
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
    handler(app, &args)
}

#[cfg(test)]
mod tests {
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
