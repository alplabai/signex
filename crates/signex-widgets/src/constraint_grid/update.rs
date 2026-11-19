//! The grid's messages and its one mutation site.

use super::model::{CellEdit, CellRef, CellValue, ColumnKind, ConstraintGrid, GroupId, RowId};

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

/// A value change the caller stores in its rules. The grid has already
/// applied it to itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridChange {
    ValueSet { cell: CellRef, value: CellValue },
    ValueCleared { cell: CellRef },
}

impl ConstraintGrid {
    /// Apply `message`. `fold` is the locale's search fold (pass
    /// `|s| i18n.fold_for_search(s)`). Returns the value change, if any.
    pub fn update(
        &mut self,
        message: GridMessage,
        fold: impl Fn(&str) -> String,
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
                self.press_cell(cell);
                None
            }
            GridMessage::EditTextChanged(text) => {
                if let Some(edit) = &mut self.edit {
                    edit.text = text;
                    edit.is_invalid = false;
                }
                None
            }
            GridMessage::EditSubmitted => self.submit_edit(),
            GridMessage::EditCancelled => {
                self.edit = None;
                None
            }
            GridMessage::ChoicePicked(cell, index) => {
                self.edit = None;
                self.write(cell, CellValue::Choice(index))
            }
            GridMessage::FlagToggled(cell, on) => {
                self.selected = Some(cell);
                self.write(cell, CellValue::Flag(on))
            }
            GridMessage::CellReset(cell) => {
                if self.edit.as_ref().is_some_and(|e| e.cell == cell) {
                    self.edit = None;
                }
                self.clear(cell)
            }
            GridMessage::ColumnQueryChanged(query) => {
                self.column_query = query;
                self.found = None;
                self.find_next(&fold);
                None
            }
            GridMessage::ColumnQuerySubmitted => {
                self.find_next(&fold);
                None
            }
        }
    }

    fn press_cell(&mut self, cell: CellRef) {
        self.selected = Some(cell);
        let Some(column) = self.column(cell.column) else {
            return;
        };
        self.edit = match column.kind {
            // A length cell opens a text editor holding its own value; an
            // empty editor shows the inherited value as its placeholder.
            ColumnKind::Length => Some(CellEdit {
                cell,
                text: match self.own_value(cell) {
                    Some(CellValue::Length(mm)) => format_mm(mm),
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

    fn submit_edit(&mut self) -> Option<GridChange> {
        let edit = self.edit.as_ref()?;
        let cell = edit.cell;
        let text = edit.text.trim();
        // An empty editor means "inherit": drop the row's own value.
        if text.is_empty() {
            self.edit = None;
            return self.clear(cell);
        }
        match parse_mm(text) {
            Some(mm) => {
                self.edit = None;
                self.write(cell, CellValue::Length(mm))
            }
            None => {
                if let Some(edit) = &mut self.edit {
                    edit.is_invalid = true;
                }
                None
            }
        }
    }

    fn write(&mut self, cell: CellRef, value: CellValue) -> Option<GridChange> {
        if self.row(cell.row).is_none() || !self.column(cell.column)?.accepts(value) {
            return None;
        }
        if self.values.get(&cell) == Some(&value) {
            return None;
        }
        self.values.insert(cell, value);
        Some(GridChange::ValueSet { cell, value })
    }

    fn clear(&mut self, cell: CellRef) -> Option<GridChange> {
        self.values
            .remove(&cell)
            .map(|_| GridChange::ValueCleared { cell })
    }

    /// Move `found` to the match after it (wrapping), opening its group.
    fn find_next(&mut self, fold: &impl Fn(&str) -> String) {
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

fn toggle<T: Ord>(set: &mut std::collections::BTreeSet<T>, item: T) {
    if !set.remove(&item) {
        set.insert(item);
    }
}

/// Parse a typed length in mm. A decimal comma is read as a point; a
/// negative, non-finite or non-numeric entry is refused.
pub fn parse_mm(text: &str) -> Option<f64> {
    let mm: f64 = text.trim().replace(',', ".").parse().ok()?;
    (mm.is_finite() && mm >= 0.0).then_some(mm)
}

/// A length for the inline editor: up to four decimals, no trailing zeros.
pub fn format_mm(mm: f64) -> String {
    let fixed = format!("{mm:.4}");
    let trimmed = fixed.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}
