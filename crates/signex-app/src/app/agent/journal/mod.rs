//! The agent journals (#2253): a persistent, append-safe record, per project
//! under `.signex/`, of what agents changed (`agent-edits.jsonl`) and who
//! connected with what rights (`agent-access.jsonl`). One JSON object per
//! line, every line `"v": 1`.
//!
//! - [`entry`]: the two line types, their size cap, the replay check.
//! - [`append`], [`rotate`], [`safe_fs`]: the safe write (lock, one `write`,
//!   no links followed, private files, rotation at the line cap).
//! - [`read`]: the reader the viewer and the tests use.
//! - [`queue`], [`record`]: the app side: an ordered queue drained off the
//!   update thread, and the lines built from what `update` knows.
//! - [`summary`]: the per-file edit list a save commit carries in its body.
//!
//! The rules, the formats and the failure handling are in
//! `docs/AGENT_JOURNALS.md`.

pub mod append;
pub mod entry;
pub(crate) mod queue;
pub mod read;
pub(crate) mod record;
mod rotate;
mod safe_fs;
pub(crate) mod summary;

pub use append::{Journal, LOCK_FILE, append_line};
pub use entry::{AccessEntry, EditEntry, JOURNAL_VERSION, MAX_ENTRY_BYTES, MIN_LINE_BYTES};
pub use read::{Report, read_journal};

#[cfg(test)]
mod tests;
