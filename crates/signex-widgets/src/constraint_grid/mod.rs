//! Constraint grid: a spreadsheet of typed constraint columns over an
//! object hierarchy (All > classes > nets > pairs), plus the
//! class-to-class matrix variant.
//!
//! A cell inherits down the hierarchy: a value set on its own row is drawn
//! strong with a reset glyph, an inherited one muted, and the tooltip names
//! the source row. Column groups collapse; the find box lands on a column
//! by title and opens its group. [`ConstraintGrid`] / [`ClassMatrix`] hold
//! only layout and view state in the caller's model; the values stay in the
//! caller's rules and reach the views through [`CellValues`] /
//! [`PairValues`]. The views are pure and emit [`GridMessage`] /
//! [`MatrixMessage`], and `update` returns the value change for the caller
//! to store.

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
    PairValues,
};
pub use matrix_view::{MATRIX_EDITOR_ID, MatrixText, cell_text, matrix_view, mirror_swatch};
pub use model::{
    CellEdit, CellRef, CellValue, CellValues, ColumnGroup, ColumnId, ColumnKind, ConstraintGrid,
    GridColumn, GridError, GridRow, GroupId, Resolved, RowId, RowKind, ValueSource, VisibleRow,
};
pub use style::{GridMetrics, GridStyle, GridText};
pub use update::{GridChange, GridMessage};
pub use view::{find_box, grid_width, view};

#[cfg(test)]
mod matrix_tests;
#[cfg(test)]
mod tests;
