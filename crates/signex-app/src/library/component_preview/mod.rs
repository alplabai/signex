//! The Component Preview surface: inline editing of a library component
//! row (datasheet, pin map, supply, parameters, simulation) without
//! opening the standalone primitive editors.
//!
//! The update logic that applies these inline edits lives in [`updates`],
//! split by field-group concern.

mod updates;

pub(crate) use updates::apply_inline_edit;
