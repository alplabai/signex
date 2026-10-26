//! `Engine::execute` command handlers, grouped by command family.
//!
//! The public `execute` in the crate root is a thin dispatcher: it clones
//! the pre-image once and routes each `Command` to one of these grouped
//! handlers. Splitting the giant match keeps every file under the
//! line-count cap while preserving the exact arm-to-body mapping.

mod batch;
mod edits;
mod font_family;
mod place;
mod replace_lib_symbol;
mod structure;

#[cfg(test)]
mod font_family_tests;
#[cfg(test)]
mod patch_exactness_tests;
#[cfg(test)]
mod register_lib_symbol_tests;
#[cfg(test)]
mod replace_lib_symbol_tests;
