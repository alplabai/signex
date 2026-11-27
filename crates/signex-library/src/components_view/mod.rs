//! The Components panel's table, as pure functions over library rows
//! (mockup row 52). No UI: the panel
//! calls these and draws the result.
//!
//! - [`column_value`]: a row's value in a column ([`CellValue`]), from its
//!   fields, its parameters, a template min-to-max pair, or [`RowFacts`]
//!   the panel resolves (symbol and footprint names, stock, price).
//! - [`sorted_rows`] / [`arrange_rows`]: nested sort and nested grouping;
//!   [`visible_lines`] flattens the groups the panel has open.
//! - [`CellValue::range_verdict`]: a range filter over a cell, a min-to-max
//!   value judged on Covers, Min, Max or its whole range.
//! - [`ViewSetup`]: a user's saved columns, widths, sort, grouping and
//!   pins for one category, started from the template's
//!   [`CategoryView`](crate::templates::CategoryView).
//! - [`detail_parameters`]: the Details card's parameter list.

mod cell;
mod details;
mod group;
mod order;
mod setup;

#[cfg(test)]
mod cell_tests;
#[cfg(test)]
mod group_tests;
#[cfg(test)]
mod setup_tests;
#[cfg(test)]
mod test_rows;

pub use cell::{CellText, CellValue, GroupKey, RowFacts, column_value};
pub use details::{DetailEntry, detail_parameters};
pub use group::{
    GroupNode, GroupPath, ViewLine, ViewNode, arrange_rows, group_paths, visible_lines,
};
pub use order::{compare_cells, sorted_rows};
pub use setup::{ViewSetup, category_view, offered_columns};
