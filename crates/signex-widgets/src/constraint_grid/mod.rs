//! Constraint grid: a spreadsheet of typed constraint columns over an
//! object hierarchy (All > classes > nets > pairs), plus the
//! class-to-class matrix variant.
//!
//! A cell inherits down the hierarchy: a value set on its own row is drawn
//! strong with a reset glyph, an inherited one muted, and the tooltip names
//! the source row. Column groups collapse; the find box lands on a column
//! by title and opens its group. State lives in [`ConstraintGrid`] /
//! [`ClassMatrix`] in the caller's model; the views are pure and emit
//! [`GridMessage`] / [`MatrixMessage`], and `update` returns the value
//! change for the caller to store in its rules.

mod cell;
mod matrix;
mod matrix_view;
mod model;
mod style;
mod update;
mod view;

pub use cell::EDITOR_ID;
pub use matrix::{
    ClassMatrix, ClassPair, MatrixChange, MatrixClass, MatrixEdit, MatrixMessage, PairClearance,
};
pub use matrix_view::{MATRIX_EDITOR_ID, MatrixText, matrix_view};
pub use model::{
    CellEdit, CellRef, CellValue, ColumnGroup, ColumnId, ColumnKind, ConstraintGrid, GridColumn,
    GridError, GridRow, GroupId, Resolved, RowId, RowKind, ValueSource, VisibleRow,
};
/// The views format lengths with the caller's locale; re-exported so a
/// caller without its own i18n dependency (the chrome catalog) can pass one.
pub use signex_i18n::I18n;
pub use style::{GridMetrics, GridStyle, GridText};
pub use update::{GridChange, GridMessage, format_mm, parse_mm};
pub use view::{find_box, grid_width, view};

#[cfg(test)]
mod matrix_tests;
#[cfg(test)]
mod tests;
