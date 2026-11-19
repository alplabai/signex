//! The grid's messages and its one mutation site. The mutation is view
//! state only; a value edit comes back as a [`GridChange`].

use super::model::{
    CellEdit, CellRef, CellValue, CellValues, ColumnKind, ConstraintGrid, GroupId, RowId,
};
use crate::LengthFormat;

/// What happened in the grid.
#[derive(Debug, Clone, PartialEq)]
pub enum GridMessage {
    /// A row's expand chevron was pressed.
    RowToggled(RowId),
    /// A column group's header was pressed.
    GroupToggled(GroupId),
    /// A length or choice cell was pressed: select it and edit it.
    CellPressed(CellRef),
    /// The inline length editor's text changed.
    EditTextChanged(String),
    /// Enter was pressed in the inline length editor.
    EditSubmitted,
    /// The inline edit was dismissed without a change.
    EditCancelled,
    /// An option of a choice cell was picked.
    ChoicePicked(CellRef, usize),
    /// A flag cell's checkbox was toggled.
    FlagToggled(CellRef, bool),
    /// A cell's reset was pressed: drop its own value.
    CellReset(CellRef),
    /// The find-a-column text changed.
    ColumnQueryChanged(String),
    /// Enter was pressed in the find box: go to the next match.
    ColumnQuerySubmitted,
}

/// A value change for the caller to store in its rules. The grid holds no
/// values: the next view reads the stored change back through
/// [`CellValues`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridChange {
    ValueSet { cell: CellRef, value: CellValue },
    ValueCleared { cell: CellRef },
}

impl ConstraintGrid {
    /// Apply `message` to the view state. `values` is the caller's view of
    /// the cells, `format` its locale and display unit (the find box folds
    /// with the locale's search fold). Returns the value change, if any.
    pub fn update(
        &mut self,
        message: GridMessage,
        values: &impl CellValues,
        format: LengthFormat<'_>,
    ) -> Option<GridChange> {
        match message {
            GridMessage::RowToggled(row) => {
                toggle(&mut self.collapsed_rows, row);
                None
            }
            GridMessage::GroupToggled(group) => {
                toggle(&mut self.collapsed_groups, group);
                None
            }
            GridMessage::CellPressed(cell) => {
                self.press_cell(cell, values, format);
                None
            }
            GridMessage::EditTextChanged(text) => {
                if let Some(edit) = &mut self.edit {
                    edit.text = text;
                    edit.is_invalid = false;
                }
                None
            }
            GridMessage::EditSubmitted => self.submit_edit(values, format),
            GridMessage::EditCancelled => {
                self.edit = None;
                None
            }
            GridMessage::ChoicePicked(cell, index) => {
                self.edit = None;
                self.write(cell, CellValue::Choice(index), values)
            }
            GridMessage::FlagToggled(cell, on) => {
                self.selected = Some(cell);
                self.write(cell, CellValue::Flag(on), values)
            }
            GridMessage::CellReset(cell) => {
                if self.edit.as_ref().is_some_and(|e| e.cell == cell) {
                    self.edit = None;
                }
                clear(cell, values)
            }
            GridMessage::ColumnQueryChanged(query) => {
                self.column_query = query;
                self.found = None;
                self.find_next(format);
                None
            }
            GridMessage::ColumnQuerySubmitted => {
                self.find_next(format);
                None
            }
        }
    }

    fn press_cell(&mut self, cell: CellRef, values: &impl CellValues, format: LengthFormat<'_>) {
        self.selected = Some(cell);
        let Some(column) = self.column(cell.column) else {
            return;
        };
        self.edit = match column.kind {
            // A length cell opens a text editor holding its own value; an
            // empty editor shows the inherited value as its placeholder.
            ColumnKind::Length => Some(CellEdit {
                cell,
                text: match values.own_value(cell) {
                    Some(CellValue::Length(mm)) => format.field_text(mm),
                    _ => String::new(),
                },
                is_invalid: false,
            }),
            // A choice cell opens its options; the view reads `selected`.
            ColumnKind::Choice(_) => Some(CellEdit {
                cell,
                text: String::new(),
                is_invalid: false,
            }),
            ColumnKind::Flag => None,
        };
    }

    fn submit_edit(
        &mut self,
        values: &impl CellValues,
        format: LengthFormat<'_>,
    ) -> Option<GridChange> {
        let edit = self.edit.as_ref()?;
        let cell = edit.cell;
        let text = edit.text.trim();
        // An empty editor means "inherit": drop the row's own value.
        if text.is_empty() {
            self.edit = None;
            return clear(cell, values);
        }
        // A length the column refuses (a negative one) is as invalid as
        // text that is not a number.
        let value = format
            .parse(text)
            .map(CellValue::Length)
            .filter(|v| self.column(cell.column).is_some_and(|c| c.accepts(*v)));
        match value {
            Some(value) => {
                self.edit = None;
                self.write(cell, value, values)
            }
            None => {
                if let Some(edit) = &mut self.edit {
                    edit.is_invalid = true;
                }
                None
            }
        }
    }

    /// The change that sets `value` on `cell`, unless the column refuses it
    /// or the row already holds it.
    fn write(
        &self,
        cell: CellRef,
        value: CellValue,
        values: &impl CellValues,
    ) -> Option<GridChange> {
        if self.row(cell.row).is_none() || !self.column(cell.column)?.accepts(value) {
            return None;
        }
        if values.own_value(cell) == Some(value) {
            return None;
        }
        Some(GridChange::ValueSet { cell, value })
    }

    /// Move `found` to the match after it (wrapping), opening its group.
    fn find_next(&mut self, format: LengthFormat<'_>) {
        let fold = |s: &str| format.i18n.fold_for_search(s);
        let matches = self.matching_columns(&self.column_query, fold);
        let next = match self
            .found
            .and_then(|f| matches.iter().position(|m| *m == f))
        {
            Some(i) => matches.get((i + 1) % matches.len()).copied(),
            None => matches.first().copied(),
        };
        self.found = next;
        if let Some(group) = next.and_then(|c| self.group_of(c)) {
            self.collapsed_groups.remove(&group);
        }
    }
}

/// The change that drops `cell`'s own value, if it has one.
fn clear(cell: CellRef, values: &impl CellValues) -> Option<GridChange> {
    values
        .own_value(cell)
        .map(|_| GridChange::ValueCleared { cell })
}

fn toggle<T: Ord>(set: &mut std::collections::BTreeSet<T>, item: T) {
    if !set.remove(&item) {
        set.insert(item);
    }
}
