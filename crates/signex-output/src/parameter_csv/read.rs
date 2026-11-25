//! Reading a parameter table file back: the records, then the columns.

use super::{
    FOOTPRINT_COLUMN, ParameterCsvError, REFERENCE_COLUMN, UNIQUE_ID_COLUMN, VALUE_COLUMN,
    unguarded,
};

/// A parameter table file as read.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParameterCsv {
    /// The parameter columns, in file order.
    pub keys: Vec<String>,
    /// One entry per line after the titles.
    pub rows: Vec<CsvPart>,
}

/// One line of the file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CsvPart {
    /// The Unique Id cell, trimmed; empty when the file has no such column.
    pub unique_id: String,
    /// The Reference cell, trimmed.
    pub reference: String,
    /// One cell per entry of [`ParameterCsv::keys`]; `None` when the line
    /// ends before that column.
    pub values: Vec<Option<String>>,
}

/// What a column of the file holds.
enum Column {
    UniqueId,
    Reference,
    /// Value or Footprint: read-only in the table, so never imported.
    Context,
    Parameter(usize),
    /// A column with a blank title.
    Ignored,
}

/// Read a parameter table file. The Unique Id, Reference, Value and
/// Footprint titles match in any case, with spaces around them; parameter
/// titles match as written. A file needs a Unique Id or a Reference column.
pub fn parse(bytes: &[u8]) -> Result<ParameterCsv, ParameterCsvError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ParameterCsvError::NotUtf8)?;
    let mut records = records(text).into_iter();
    let titles = records.next().ok_or(ParameterCsvError::NoHeader)?;
    let mut keys: Vec<String> = Vec::new();
    let mut has_unique_id = false;
    let mut has_reference = false;
    let mut columns = Vec::with_capacity(titles.len());
    for title in titles {
        let title = unguarded(title);
        let fixed = title.trim();
        let column = if fixed.eq_ignore_ascii_case(UNIQUE_ID_COLUMN) {
            if has_unique_id {
                return Err(ParameterCsvError::DuplicateColumn(title));
            }
            has_unique_id = true;
            Column::UniqueId
        } else if fixed.eq_ignore_ascii_case(REFERENCE_COLUMN) {
            if has_reference {
                return Err(ParameterCsvError::DuplicateColumn(title));
            }
            has_reference = true;
            Column::Reference
        } else if fixed.eq_ignore_ascii_case(VALUE_COLUMN)
            || fixed.eq_ignore_ascii_case(FOOTPRINT_COLUMN)
        {
            Column::Context
        } else if fixed.is_empty() {
            Column::Ignored
        } else if keys.contains(&title) {
            return Err(ParameterCsvError::DuplicateColumn(title));
        } else {
            keys.push(title);
            Column::Parameter(keys.len() - 1)
        };
        columns.push(column);
    }
    if !has_unique_id && !has_reference {
        return Err(ParameterCsvError::NoKeyColumn);
    }
    let rows = records
        .map(|record| {
            let mut part = CsvPart {
                unique_id: String::new(),
                reference: String::new(),
                values: vec![None; keys.len()],
            };
            for (column, cell) in columns.iter().zip(record) {
                let cell = unguarded(cell);
                match column {
                    Column::UniqueId => part.unique_id = cell.trim().to_string(),
                    Column::Reference => part.reference = cell.trim().to_string(),
                    Column::Parameter(i) => part.values[*i] = Some(cell),
                    Column::Context | Column::Ignored => {}
                }
            }
            part
        })
        .collect();
    Ok(ParameterCsv { keys, rows })
}

/// The RFC 4180 records of `text`: a quoted field keeps its commas, line
/// breaks and doubled quotes; an empty line is no record.
fn records(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut has_content = false;
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' => {
                quoted = true;
                has_content = true;
            }
            ',' => {
                row.push(std::mem::take(&mut field));
                has_content = true;
            }
            '\r' => {}
            '\n' => {
                if has_content {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                has_content = false;
            }
            _ => {
                field.push(c);
                has_content = true;
            }
        }
    }
    if has_content {
        row.push(field);
        rows.push(row);
    }
    rows
}
