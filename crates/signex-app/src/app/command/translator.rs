//! The GUI translator (ADR-0006, slice 6): a person's command leaves its
//! document and targets implicit, the board in view and its selection. The
//! GUI handler reads them, and this fills them in as the `document` and
//! `ids` operands (uuids, never a designator, which a board may share) and
//! runs the same operand handler an agent or the CLI runs, so one command id
//! has one behaviour on every surface.
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

/// The ids whose GUI route translates: the handler of the message it sends
/// reads the selection and calls [`run`]. The parity table names a bridge
/// row of one `translator`. `arrange_in_box` translates too, but on its
/// second board click (`pcb_arrange_box_clicked`); its bridge arm only arms
/// the box, so that row stays `bridge`. `place_board_reuse_block` has no
/// bridge row: a context-menu row carries the block name. Each id's GUI
/// route test checks [`ran`], so a route that calls the engine itself
/// fails it. `delete_selection` on a footprint editor tab is the one GUI
/// half that does not translate: the editor deletes on its own until slice
/// S-L (a declared exception, `parity/owners.rs`). `change_footprint`
/// translates the footprint picker's Swap: the selection as `ids`, the
/// picked footprint as `footprint`, `unmatched=keep` (#2937).
#[cfg(test)]
pub(crate) const TRANSLATED: &[&str] = &[
    "arrange_in_room",
    "auto_place",
    "board_shape_from_selection",
    "bring_drawings_forward",
    "bring_drawings_to_front",
    "change_footprint",
    "delete_selection",
    "gloss_selected_routes",
    "keepout_from_selection",
    "place_board_reuse_block",
    "save_board_reuse_block",
    "send_drawings_backward",
    "send_drawings_to_back",
    "swap_gates",
    "swap_pins",
];

/// The translated ids a headless run without operands still runs on the
/// session's own selection, as before they took operands (#2326 decision 2:
/// the CLI keeps its own selection): their bridge arm translates it.
const ON_THE_SESSION_SELECTION: &[&str] = &[
    "bring_drawings_forward",
    "bring_drawings_to_front",
    "delete_selection",
    "send_drawings_backward",
    "send_drawings_to_back",
];

/// Whether a headless run of `id` without operands acts on the session's
/// selection rather than asking for them.
pub(crate) fn runs_on_the_session_selection(id: &str) -> bool {
    ON_THE_SESSION_SELECTION.contains(&id)
}

/// Whether the bridge arm of the command `id` is a translator.
#[cfg(test)]
pub(crate) fn is_translated(id: &str) -> bool {
    TRANSLATED.contains(&id)
}

#[cfg(test)]
thread_local! {
    /// The ids [`run`] ran on this thread, so a test can tell a GUI route
    /// that translates from one that calls the engine itself.
    static RAN: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Whether [`run`] ran `id` on this thread.
#[cfg(test)]
pub(crate) fn ran(id: &str) -> bool {
    RAN.with(|ran| ran.borrow().iter().any(|r| r == id))
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
    #[cfg(test)]
    RAN.with(|ran| ran.borrow_mut().push(id.to_string()));
    let Ok(command) = AppCommandId::new(id) else {
        return (not_available(), Task::none());
    };
    // The person's command, not an agent's: every open document is in
    // reach (`document_entries`) and no agent undo step wraps it.
    let serving = std::mem::replace(&mut app.agent.serving, false);
    let from_view = std::mem::replace(&mut app.agent.from_view, true);
    let ran = match document_target(app, document) {
        Some(target) => {
            let mut values =
                BTreeMap::from([("document".to_string(), OperandValue::Document(target))]);
            values.extend(operands.into_iter().map(|(n, v)| (n.to_string(), v)));
            let args = CommandArgs {
                context: Default::default(),
                operands: Operands {
                    values,
                    texts: BTreeMap::new(),
                },
            };
            app.run_operands(&command, &args)
        }
        None => (not_available(), Task::none()),
    };
    app.agent.serving = serving;
    app.agent.from_view = from_view;
    let (outcome, task) = ran;
    let outcome = match outcome {
        CommandOutcome::Answered(_)
        | CommandOutcome::Disabled { .. }
        | CommandOutcome::Deferred { .. } => outcome,
        _ => not_available(),
    };
    (outcome, task)
}

/// `document` as a target: its path, or its document id when the path is
/// not Unicode, which no text path can name. `None` for a non-Unicode path
/// of a document without an id.
fn document_target(app: &Signex, document: &Path) -> Option<DocumentTarget> {
    if let Some(text) = document.to_str() {
        return Some(DocumentTarget::Path(text.to_string()));
    }
    app.document_entries()
        .into_iter()
        .find(|entry| entry.path.as_deref() == Some(document))
        .and_then(|entry| entry.id)
        .map(DocumentTarget::Id)
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
