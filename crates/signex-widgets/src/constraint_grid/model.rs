//! The grid's layout and view state: a row hierarchy, grouped typed
//! columns, what is collapsed, selected and being typed. The cell values
//! are the caller's: the grid reads them through [`CellValues`] and never
//! stores one. `update.rs` is the only place that changes the view state.

use std::collections::{BTreeSet, HashMap};

use crate::Label;

/// A row of the hierarchy (the All row, a class, a net, a pair).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RowId(pub u32);

/// A typed constraint column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ColumnId(pub u32);

/// A collapsible group of columns (one rule type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupId(pub u32);

/// One cell: a row and a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellRef {
    pub row: RowId,
    pub column: ColumnId,
}

impl CellRef {
    pub fn new(row: RowId, column: ColumnId) -> Self {
        Self { row, column }
    }
}

/// What a row stands for. The hierarchy is the caller's; the kind only
/// tells the view and the caller what the row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    All,
    Class,
    Net,
    Pair,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GridRow {
    pub id: RowId,
    pub parent: Option<RowId>,
    pub kind: RowKind,
    /// The object's name (a class, net or pair name): user content.
    pub name: String,
}

/// The type of a column's values.
#[derive(Debug, Clone)]
pub enum ColumnKind {
    /// A length in mm, never negative.
    Length,
    /// One of the listed options, by index.
    Choice(Vec<Label<'static>>),
    /// On or off.
    Flag,
}

/// A cell value; its variant matches its column's [`ColumnKind`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellValue {
    Length(f64),
    Choice(usize),
    Flag(bool),
}

#[derive(Debug, Clone)]
pub struct GridColumn {
    pub id: ColumnId,
    pub title: Label<'static>,
    pub kind: ColumnKind,
}

impl GridColumn {
    /// Whether `value` is a valid value of this column.
    pub fn accepts(&self, value: CellValue) -> bool {
        match (&self.kind, value) {
            (ColumnKind::Length, CellValue::Length(mm)) => mm.is_finite() && mm >= 0.0,
            (ColumnKind::Choice(options), CellValue::Choice(i)) => i < options.len(),
            (ColumnKind::Flag, CellValue::Flag(_)) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColumnGroup {
    pub id: GroupId,
    pub title: Label<'static>,
    pub columns: Vec<GridColumn>,
}

/// Where a cell's value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueSource {
    /// Set on this row (an override).
    Own,
    /// Set on this ancestor row.
    Inherited(RowId),
    /// No row up the chain sets it: the column default.
    Default,
}

/// A cell's value and its source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolved {
    pub value: CellValue,
    pub source: ValueSource,
}

impl Resolved {
    pub fn is_own(&self) -> bool {
        self.source == ValueSource::Own
    }
}

/// The caller's read-only view of the cell values: what each cell holds
/// and where that comes from. The grid reads it on every update and view
/// and reports edits as [`GridChange`](super::GridChange)s for the caller
/// to store, so the caller's rules stay the one source of truth.
pub trait CellValues {
    /// The cell's value and its source; `None` for a cell the caller does
    /// not know.
    fn resolve(&self, cell: CellRef) -> Option<Resolved>;

    /// What the cell would hold without a value of its own: the parent
    /// row's value, or the column default on a root row. The inline editor
    /// shows it as its placeholder.
    fn inherited(&self, cell: CellRef) -> Option<Resolved>;

    /// The value set on the row itself, if any.
    fn own_value(&self, cell: CellRef) -> Option<CellValue> {
        self.resolve(cell).filter(Resolved::is_own).map(|r| r.value)
    }
}

/// A row as the view lays it out: indented under its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleRow {
    pub id: RowId,
    pub depth: usize,
    pub has_children: bool,
    pub is_open: bool,
}

/// A row could not be added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridError {
    /// The id is already in the grid.
    DuplicateRow(RowId),
    /// The parent is not in the grid yet; add parents first.
    UnknownParent(RowId),
}

/// An inline edit of a cell; `text` is what the user typed.
#[derive(Debug, Clone, PartialEq)]
pub struct CellEdit {
    pub cell: CellRef,
    pub text: String,
    /// The text the editor opened with. It is rounded to the field's
    /// decimals, so a submit of this text unchanged writes nothing.
    pub opened: String,
    /// The last submit did not parse as a valid length.
    pub is_invalid: bool,
}

/// The constraint grid's layout and view state. Built by the caller,
/// changed only through
/// [`ConstraintGrid::update`](crate::constraint_grid::ConstraintGrid::update).
#[derive(Debug, Clone, Default)]
pub struct ConstraintGrid {
    pub(super) rows: Vec<GridRow>,
    pub(super) groups: Vec<ColumnGroup>,
    pub(super) collapsed_rows: BTreeSet<RowId>,
    pub(super) collapsed_groups: BTreeSet<GroupId>,
    pub(super) selected: Option<CellRef>,
    pub(super) edit: Option<CellEdit>,
    pub(super) column_query: String,
    pub(super) found: Option<ColumnId>,
}

impl ConstraintGrid {
    /// A grid over `groups` with no rows yet.
    pub fn new(groups: Vec<ColumnGroup>) -> Self {
        Self {
            groups,
            ..Self::default()
        }
    }

    /// Append a row. Its parent must already be in the grid, so the
    /// hierarchy can never hold a cycle.
    pub fn push_row(&mut self, row: GridRow) -> Result<(), GridError> {
        if self.row(row.id).is_some() {
            return Err(GridError::DuplicateRow(row.id));
        }
        if let Some(parent) = row.parent
            && self.row(parent).is_none()
        {
            return Err(GridError::UnknownParent(parent));
        }
        self.rows.push(row);
        Ok(())
    }

    /// Start a row collapsed (the hierarchy below it hidden).
    pub fn collapse_row(&mut self, row: RowId) {
        self.collapsed_rows.insert(row);
    }

    pub fn rows(&self) -> &[GridRow] {
        &self.rows
    }

    pub fn groups(&self) -> &[ColumnGroup] {
        &self.groups
    }

    pub fn row(&self, id: RowId) -> Option<&GridRow> {
        self.rows.iter().find(|r| r.id == id)
    }

    pub fn column(&self, id: ColumnId) -> Option<&GridColumn> {
        self.groups
            .iter()
            .flat_map(|g| g.columns.iter())
            .find(|c| c.id == id)
    }

    pub fn is_group_collapsed(&self, group: GroupId) -> bool {
        self.collapsed_groups.contains(&group)
    }

    pub fn selected(&self) -> Option<CellRef> {
        self.selected
    }

    pub fn edit(&self) -> Option<&CellEdit> {
        self.edit.as_ref()
    }

    pub fn column_query(&self) -> &str {
        &self.column_query
    }

    /// The column the find box last landed on.
    pub fn found_column(&self) -> Option<ColumnId> {
        self.found
    }

    /// Rows in display order: depth-first, children in insertion order,
    /// nothing below a collapsed row.
    pub fn visible_rows(&self) -> Vec<VisibleRow> {
        let mut children: HashMap<Option<RowId>, Vec<RowId>> = HashMap::new();
        for row in &self.rows {
            children.entry(row.parent).or_default().push(row.id);
        }
        let mut out = Vec::with_capacity(self.rows.len());
        let mut stack: Vec<(RowId, usize)> = children
            .get(&None)
            .map(|roots| roots.iter().rev().map(|id| (*id, 0)).collect())
            .unwrap_or_default();
        while let Some((id, depth)) = stack.pop() {
            let kids = children.get(&Some(id));
            let has_children = kids.is_some_and(|k| !k.is_empty());
            let is_open = !self.collapsed_rows.contains(&id);
            out.push(VisibleRow {
                id,
                depth,
                has_children,
                is_open,
            });
            if let (Some(kids), true) = (kids, is_open) {
                stack.extend(kids.iter().rev().map(|k| (*k, depth + 1)));
            }
        }
        out
    }

    /// Columns whose title contains `query`, in display order. Both sides
    /// go through `fold` (the locale's search fold).
    pub fn matching_columns(&self, query: &str, fold: impl Fn(&str) -> String) -> Vec<ColumnId> {
        let needle = fold(query.trim());
        if needle.is_empty() {
            return Vec::new();
        }
        self.groups
            .iter()
            .flat_map(|g| g.columns.iter())
            .filter(|c| fold(c.title.as_str()).contains(&needle))
            .map(|c| c.id)
            .collect()
    }

    /// The group that holds `column`.
    pub fn group_of(&self, column: ColumnId) -> Option<GroupId> {
        self.groups
            .iter()
            .find(|g| g.columns.iter().any(|c| c.id == column))
            .map(|g| g.id)
    }
}
