//! `apply_edits` and `revert` (#2178): an agent edits the shared engine as one
//! labelled undo step per request, and takes back only its own step. The
//! design and its limits are in `docs/AGENT_EDITS.md`.

mod apply;
mod group_run;
mod revert;
mod single;
mod wire;

#[cfg(test)]
mod group_run_tests;

pub(super) use apply::{apply_edits, listed_edits};
pub(super) use revert::revert;
pub(super) use single::{run_as_one_step, runs_as_one_step};
pub use wire::{
    EditSpec, MAX_EDITS, edit_list_text, edit_refusal, operand_text, parse_documents,
    parse_edit_list, parse_expect_revisions,
};
