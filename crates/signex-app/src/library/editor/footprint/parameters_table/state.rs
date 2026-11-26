//! What the parameters table holds open, and the pure rules it shows: the
//! names offered while a name is typed, and the value and unit of a row.

use std::collections::BTreeMap;

use signex_sketch::parameter::ParameterTable;
use signex_sketch::parameter_refs::{ParameterNameError, RowValue, check_new_name};
use signex_sketch::unit::{Quantity, UnitFamily};

/// Most names the autocomplete offers at once.
pub const MAX_SUGGESTIONS: usize = 6;

/// The table, open. `None` on the editor state means it is closed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParametersTableState {
    /// The row last typed in, added or made: highlighted, and the one the
    /// autocomplete reads.
    pub focus: Option<String>,
    /// The row whose ⋯ menu is open.
    pub menu: Option<String>,
    /// A rename being typed.
    pub rename: Option<RenameDraft>,
    /// A delete waiting for its answer: the parameter is in use.
    pub delete_prompt: Option<String>,
    /// Each parameter's last value, so a row in error shows the value the
    /// drawing still uses.
    pub last_good: BTreeMap<String, Quantity>,
    /// Another footprint's parameters, offered to copy (mockup row 189).
    pub import: Option<ParametersImport>,
}

/// The parameters of a picked footprint, each ticked to copy or not.
#[derive(Debug, Clone, PartialEq)]
pub struct ParametersImport {
    /// The footprint they come from.
    pub source: String,
    pub rows: Vec<ImportRow>,
}

/// One parameter offered for import.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportRow {
    pub name: String,
    pub expr: String,
    /// The name is in use here or not valid: it cannot be imported.
    pub is_refused: bool,
    pub is_picked: bool,
}

impl ParametersImport {
    /// `parameters` offered against `own`: every free name starts ticked,
    /// and a name in use is never overwritten.
    pub fn of(source: String, parameters: Vec<(String, String)>, own: &ParameterTable) -> Self {
        let rows = parameters
            .into_iter()
            .map(|(name, expr)| {
                let is_refused = check_new_name(own, &name).is_err();
                ImportRow {
                    name,
                    expr,
                    is_refused,
                    is_picked: !is_refused,
                }
            })
            .collect();
        Self { source, rows }
    }

    /// Tick or untick `name`; a refused row stays unticked.
    pub fn toggle(&mut self, name: &str) {
        if let Some(row) = self
            .rows
            .iter_mut()
            .find(|row| row.name == name && !row.is_refused)
        {
            row.is_picked = !row.is_picked;
        }
    }

    pub fn picked(&self) -> impl Iterator<Item = &ImportRow> {
        self.rows.iter().filter(|row| row.is_picked)
    }
}

/// A rename in progress.
#[derive(Debug, Clone, PartialEq)]
pub struct RenameDraft {
    pub from: String,
    pub draft: String,
    /// Why the last submit was refused.
    pub error: Option<ParameterNameError>,
}

impl ParametersTableState {
    /// The table, open with `name` focused.
    pub fn focused(name: impl Into<String>) -> Self {
        Self {
            focus: Some(name.into()),
            ..Self::default()
        }
    }

    /// Close the deepest open thing inside the table: the import list, the
    /// delete prompt, a rename, then a row menu. False when none was open.
    pub fn close_innermost(&mut self) -> bool {
        self.import.take().is_some()
            || self.delete_prompt.take().is_some()
            || self.rename.take().is_some()
            || self.menu.take().is_some()
    }

    /// Keep the value of every row that has one.
    pub fn remember(&mut self, rows: &BTreeMap<String, RowValue>) {
        for (name, row) in rows {
            if let RowValue::Value(q) = row {
                self.last_good.insert(name.clone(), *q);
            }
        }
    }
}

/// The unit a row's value is shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayUnit {
    Mm,
    Deg,
    Count,
}

/// A canonical value as the table shows it: millimetres, degrees or a
/// count, to four decimals with `.` (an engineering value, never
/// locale-grouped).
pub fn display_value(q: Quantity) -> (String, DisplayUnit) {
    let (value, unit) = match q.unit.family() {
        UnitFamily::Length => (q.value, DisplayUnit::Mm),
        UnitFamily::Angle => (q.value.to_degrees(), DisplayUnit::Deg),
        UnitFamily::Count | UnitFamily::Derived => (q.value, DisplayUnit::Count),
    };
    let rounded = (value * 1e4).round() / 1e4;
    let rounded = if rounded == 0.0 { 0.0 } else { rounded };
    (format!("{rounded}"), unit)
}

/// One name the autocomplete offers, and the expression picking it gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub name: String,
    pub completed: String,
}

/// The names to offer while `expr` ends in a partly typed name: every name
/// of `names` that starts with it and is longer, other than `own` (the
/// row's own name, which would be a cycle). Nothing when `expr` ends in a
/// number, a unit suffix (`0.5mm`) or punctuation.
pub fn suggestions<'a>(
    expr: &str,
    own: &str,
    names: impl IntoIterator<Item = &'a str>,
) -> Vec<Suggestion> {
    let Some(start) = trailing_name_start(expr) else {
        return Vec::new();
    };
    let prefix = &expr[start..];
    names
        .into_iter()
        .filter(|name| *name != own && name.len() > prefix.len() && name.starts_with(prefix))
        .take(MAX_SUGGESTIONS)
        .map(|name| Suggestion {
            name: name.to_string(),
            completed: format!("{}{name}", &expr[..start]),
        })
        .collect()
}

/// Where the name `expr` ends in starts: a run of `[A-Za-z0-9_]` that
/// starts with a letter or `_` and does not follow a digit or a `.`.
fn trailing_name_start(expr: &str) -> Option<usize> {
    let is_name_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let start = expr
        .char_indices()
        .rev()
        .take_while(|&(_, c)| is_name_char(c))
        .last()
        .map(|(i, _)| i)?;
    let first = expr[start..].chars().next()?;
    let before = expr[..start].chars().next_back();
    let starts_name = first.is_ascii_alphabetic() || first == '_';
    let after_literal = before.is_some_and(|c| c.is_ascii_digit() || c == '.');
    (starts_name && !after_literal).then_some(start)
}
