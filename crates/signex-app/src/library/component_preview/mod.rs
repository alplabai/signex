//! The Component Preview surface: inline editing of a library component
//! row (datasheet, pin map, supply, parameters, simulation) without
//! opening the standalone primitive editors.
//!
//! The reducer that applies these inline edits lives in [`reducer`], split
//! by field-group concern.

mod reducer;

pub(crate) use reducer::apply_inline_edit;
