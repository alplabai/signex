//! `Engine::execute` command handlers, grouped by command family.
//!
//! The public `execute` in the crate root is a thin dispatcher: it clones
//! the pre-image once and routes each `Command` to one of these grouped
//! handlers. Splitting the giant match keeps every file under the
//! line-count cap while preserving the exact arm-to-body mapping.

mod batch;
pub(crate) mod change_component;
mod designator_in;
mod drawings;
mod edits;
mod font_family;
pub(crate) mod pin_changes;
mod place;
mod replace_lib_symbol;
mod structure;
mod symbol_body;

#[cfg(test)]
mod change_component_tests;
#[cfg(test)]
mod designator_in_tests;
#[cfg(test)]
mod drawings_tests;
#[cfg(test)]
mod font_family_tests;
#[cfg(test)]
mod patch_exactness_tests;
#[cfg(test)]
mod register_lib_symbol_tests;
#[cfg(test)]
mod replace_lib_symbol_tests;
#[cfg(test)]
mod symbol_body_tests;
