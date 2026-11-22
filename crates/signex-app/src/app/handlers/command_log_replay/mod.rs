//! Command Log Replay (Caner, 2026-11-22). Every command records what it took
//! when it finished, and Replay runs that again on the same items, leaving
//! the person's selection and tool as they were. Rules: `docs/COMMAND_LOG.md`.
//!
//! - [`scope`]: how each catalog command replays.
//! - [`book`]: the recorded input of each row.
//! - [`capture`]: recording a finished edit.
//! - [`run`]: Replay on selection targets; [`edits`]: on recorded edits.
//! - [`operands`]: a row with operands that edited a board.
//! - [`rebase`]: a recorded whole object on the sheet as it is now.
//! - [`editors`]: the footprint and symbol editors.
//! - [`editor_panel`]: their Properties panel edits and pad drags.
//! - [`selection`]: holding and putting back the person's selection.

mod book;
mod capture;
mod editor_panel;
mod editors;
mod edits;
mod operands;
mod rebase;
mod run;
mod scope;
mod selection;

pub(crate) use book::ReplayBook;
#[cfg(test)]
pub(super) use scope::{ContinuesAs, ReplayScope, replay_scope};
