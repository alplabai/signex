//! Command Log Replay (Caner, 2026-11-22). Every command records what it took
//! when it finished, and Replay runs that again on the same items, leaving
//! the person's selection and tool as they were. Rules: `docs/COMMAND_LOG.md`.
//!
//! - [`safe_list`]: the only commands Replay may run, with their proofs.
//! - [`gate`]: the check every Replay passes first.
//! - [`sheet_guard`]: the target check of Move, Delete and Duplicate.
//! - [`scope`]: how each catalog command replays.
//! - [`book`]: the recorded input of each row.
//! - [`capture`]: recording a finished edit.
//! - [`run`]: Replay on selection targets; [`edits`]: on recorded edits.
//! - [`rebase`]: a recorded whole object on the sheet as it is now.
//! - [`editors`]: the footprint and symbol editors.
//! - [`editor_panel`]: their Properties panel edits and pad drags.
//! - [`selection`]: holding and putting back the person's selection.

mod book;
mod capture;
mod editor_panel;
mod editors;
mod edits;
mod gate;
mod rebase;
mod run;
mod safe_list;
mod scope;
mod selection;
mod sheet_guard;

pub(crate) use book::{BoardDrop, ReplayBook};
#[cfg(test)]
pub(super) use safe_list::{SAFE_COMMANDS, replay_proof};
#[cfg(test)]
pub(super) use scope::{ContinuesAs, ReplayScope, replay_scope};
