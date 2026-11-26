//! The Parameter Manager's CSV file (#1295): the parameters of the listed
//! sheets' parts written out, a file read back, and what an import of it
//! changes.
//!
//! RFC 4180, UTF-8 with a byte-order mark and CRLF line ends, written by the
//! BOM's row writer. The first line holds the column titles: Unique Id (the
//! key an import matches by), Reference, Sheet (only when the table lists
//! all sheets, #1784), Value, Footprint, then one column per parameter in
//! use, sorted. Then one line per listed part (power ports left out), sheet
//! by sheet in sheet order. The format and the import rules:
//! docs/PARAMETER_CSV.md.

mod plan;
mod read;

#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::collections::BTreeSet;

use signex_types::schematic::{SchematicSheet, Symbol};

use crate::bom::csv::{is_formula_lead, write_csv_row};

pub use plan::{ImportPlan, ParameterChange, plan_import};
pub use read::{CsvPart, ParameterCsv, parse};

/// Title of the column that keys each line to its part: the part's uuid.
pub const UNIQUE_ID_COLUMN: &str = "Unique Id";
/// Title of the column that names each part.
pub const REFERENCE_COLUMN: &str = "Reference";
/// Title of the column that names each part's sheet (All sheets only).
pub const SHEET_COLUMN: &str = "Sheet";
/// Title of the value column: written for context, never imported.
pub const VALUE_COLUMN: &str = "Value";
/// Title of the footprint column: written for context, never imported.
pub const FOOTPRINT_COLUMN: &str = "Footprint";

/// Why a file is not a parameter table.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParameterCsvError {
    #[error("the file is not UTF-8 text")]
    NotUtf8,
    #[error("the file holds no column titles")]
    NoHeader,
    #[error("the file has neither a Unique Id nor a Reference column")]
    NoKeyColumn,
    #[error("two columns are titled {0}")]
    DuplicateColumn(String),
    #[error("{0}")]
    Write(String),
}

/// One part a parameter table lists.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedPart<'a> {
    pub symbol: &'a Symbol,
    /// The designator the table names it by: the one its canvas draws
    /// (#1151), which may differ from `symbol.reference`.
    pub reference: Cow<'a, str>,
}

impl<'a> ListedPart<'a> {
    /// `symbol`, named by its own `reference`.
    pub fn stored(symbol: &'a Symbol) -> Self {
        Self {
            symbol,
            reference: Cow::Borrowed(symbol.reference.as_str()),
        }
    }
}

/// One sheet a parameter table lists, and its listed parts in sheet order.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedSheet<'a> {
    /// What the Sheet column names it; unique among the listed sheets.
    pub name: String,
    pub parts: Vec<ListedPart<'a>>,
}

/// The parts a parameter table lists: every placed symbol except power
/// ports, in sheet order.
pub fn listed_parts(sheet: &SchematicSheet) -> Vec<&Symbol> {
    sheet.symbols.iter().filter(|s| !s.is_power).collect()
}

/// Every parameter one of `parts` carries, sorted.
pub fn parameter_keys<'s>(parts: impl IntoIterator<Item = &'s Symbol>) -> Vec<String> {
    parts
        .into_iter()
        .flat_map(|s| s.fields.keys().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// `sheets` as a parameter table file, with a Sheet column when
/// `with_sheet`. A part without a parameter gets an empty cell.
pub fn export(sheets: &[ListedSheet<'_>], with_sheet: bool) -> Result<Vec<u8>, ParameterCsvError> {
    let keys = parameter_keys(sheets.iter().flat_map(|s| s.parts.iter().map(|p| p.symbol)));
    let mut out = b"\xef\xbb\xbf".to_vec();
    let mut titles = vec![UNIQUE_ID_COLUMN.to_string(), REFERENCE_COLUMN.to_string()];
    if with_sheet {
        titles.push(SHEET_COLUMN.to_string());
    }
    titles.extend([VALUE_COLUMN.to_string(), FOOTPRINT_COLUMN.to_string()]);
    titles.extend(keys.iter().cloned());
    write_line(&mut out, &titles)?;
    for sheet in sheets {
        for part in &sheet.parts {
            let symbol = part.symbol;
            let mut cells = vec![symbol.uuid.to_string(), part.reference.to_string()];
            if with_sheet {
                cells.push(sheet.name.clone());
            }
            cells.extend([symbol.value.clone(), symbol.footprint.clone()]);
            cells.extend(
                keys.iter()
                    .map(|key| symbol.fields.get(key).cloned().unwrap_or_default()),
            );
            write_line(&mut out, &cells)?;
        }
    }
    Ok(out)
}

fn write_line(out: &mut Vec<u8>, cells: &[String]) -> Result<(), ParameterCsvError> {
    let cells: Vec<Cow<'_, str>> = cells.iter().map(|cell| guarded(cell)).collect();
    write_csv_row(out, &cells).map_err(|e| ParameterCsvError::Write(e.to_string()))?;
    out.extend_from_slice(b"\r\n");
    Ok(())
}

/// Whether a spreadsheet would run `text` as a formula, also behind the
/// quotes a guard put in front of it.
fn needs_guard(text: &str) -> bool {
    is_formula_lead(text.trim_start_matches('\''))
}

/// `text` with one more leading `'` when it [`needs_guard`], so the file
/// cannot run a formula and [`unguarded`] gives `text` back exactly.
fn guarded(text: &str) -> Cow<'_, str> {
    if needs_guard(text) {
        Cow::Owned(format!("'{text}"))
    } else {
        Cow::Borrowed(text)
    }
}

/// A cell as written before [`guarded`].
fn unguarded(cell: String) -> String {
    match cell.strip_prefix('\'') {
        Some(rest) if needs_guard(rest) => rest.to_string(),
        _ => cell,
    }
}
