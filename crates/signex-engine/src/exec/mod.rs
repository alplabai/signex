//! `Engine::execute` command handlers, grouped by command family.
//!
//! The public `execute` in the crate root is a thin dispatcher: it clones
//! the pre-image once and routes each `Command` to one of these grouped
//! handlers. Splitting the giant match keeps every file under the
//! line-count cap while preserving the exact arm-to-body mapping.

mod edits;
mod place;
mod structure;
