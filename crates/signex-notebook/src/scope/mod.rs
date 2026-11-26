//! Scope captures (mockup row 199): a scope's CSV export read as a time
//! column and one column per channel, then drawn as a vector plot with
//! 10 x 8 divisions, axis units, the channel legend (coupling, V/div,
//! bandwidth limit), the cursors with their readout and the trigger marker
//! ([`plot`]). A PNG screenshot is shown as is and never comes here.

mod plot;
mod spec;

pub use plot::{AxisLabel, DIVISIONS_X, DIVISIONS_Y, PlotChannel, PlotCursor, ScopePlot, plot};
pub use spec::{ChannelSpec, Coupling, Cursors, PlotSpec, Trigger};

/// The most samples a capture may hold, all channels counted; a larger
/// file is refused rather than read into memory.
pub const MAX_SAMPLES: usize = 4_000_000;

/// A capture: the time of each sample and each channel's samples.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeCapture {
    pub time_unit: String,
    pub times: Vec<f64>,
    pub channels: Vec<Trace>,
}

/// One channel's samples.
#[derive(Debug, Clone, PartialEq)]
pub struct Trace {
    pub name: String,
    pub unit: String,
    pub samples: Vec<f64>,
}

/// Why a CSV is not a capture. Lines are 1-based.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScopeError {
    #[error("the scope file holds no samples")]
    Empty,
    #[error("line {line} has no channel column after the time")]
    NoChannel { line: usize },
    #[error("line {line} has {found} columns, the header {expected}")]
    Ragged {
        line: usize,
        found: usize,
        expected: usize,
    },
    #[error("line {line}, column {column} is not a number")]
    NotANumber { line: usize, column: usize },
    #[error("the scope file holds more than {MAX_SAMPLES} samples")]
    TooLarge,
}

/// Read a scope CSV: optional lines of instrument metadata, a header line
/// of column names (`Time (s),CH1 (V)`), an optional line of units, then
/// one line per sample. The first column is the time. Commas, semicolons
/// or tabs separate the cells; with semicolons a decimal comma is read. A
/// name's unit is in parentheses or brackets; without one the time is in
/// seconds and a channel in volts. Blank lines and lines starting with `#`
/// are skipped.
pub fn parse_csv(text: &str) -> Result<ScopeCapture, ScopeError> {
    let lines: Vec<(usize, &str)> = text
        .lines()
        .enumerate()
        .map(|(i, line)| (i + 1, line.trim()))
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'))
        .collect();
    let first_data = lines
        .iter()
        .position(|(_, line)| numbers(line, separator(line)).is_some())
        .ok_or(ScopeError::Empty)?;
    let separator = separator(lines[first_data].1);
    let header: Vec<&str> = lines[..first_data]
        .iter()
        .rev()
        .take(2)
        .rev()
        .map(|(_, line)| *line)
        .collect();
    let (names, unit_cells) = match header.as_slice() {
        [names, units] => match units_row(units, separator) {
            Some(units) => (Some(*names), units),
            None => (Some(*units), Vec::new()),
        },
        [names] => (Some(*names), Vec::new()),
        _ => (None, Vec::new()),
    };
    let width = cells(lines[first_data].1, separator).len();
    if width < 2 {
        return Err(ScopeError::NoChannel {
            line: lines[first_data].0,
        });
    }
    let mut columns: Vec<Vec<f64>> = vec![Vec::new(); width];
    for (number, line) in &lines[first_data..] {
        let row = cells(line, separator);
        if row.len() != width {
            return Err(ScopeError::Ragged {
                line: *number,
                found: row.len(),
                expected: width,
            });
        }
        for (column, cell) in row.iter().enumerate() {
            let value = number_of(cell, separator).ok_or(ScopeError::NotANumber {
                line: *number,
                column: column + 1,
            })?;
            columns[column].push(value);
        }
        if columns[0].len() * width > MAX_SAMPLES {
            return Err(ScopeError::TooLarge);
        }
    }
    let name_cells: Vec<String> = names.map(|n| cells(n, separator)).unwrap_or_default();
    let column_label = |index: usize, default_name: String, default_unit: &str| {
        let (name, unit) = split_unit(name_cells.get(index).map_or("", String::as_str));
        let name = if name.is_empty() { default_name } else { name };
        let unit = unit_cells
            .get(index)
            .filter(|u| !u.is_empty())
            .cloned()
            .or(unit)
            .unwrap_or_else(|| default_unit.to_string());
        (name, unit)
    };
    let mut columns = columns.into_iter();
    let times = columns.next().unwrap_or_default();
    let (_, time_unit) = column_label(0, "Time".to_string(), "s");
    let channels = columns
        .enumerate()
        .map(|(i, samples)| {
            let (name, unit) = column_label(i + 1, format!("CH{}", i + 1), "V");
            Trace {
                name,
                unit,
                samples,
            }
        })
        .collect();
    Ok(ScopeCapture {
        time_unit,
        times,
        channels,
    })
}

/// The separator of `line`: a tab, a semicolon or a comma.
fn separator(line: &str) -> char {
    if line.contains('\t') {
        '\t'
    } else if line.contains(';') {
        ';'
    } else {
        ','
    }
}

fn cells(line: &str, separator: char) -> Vec<String> {
    line.split(separator)
        .map(|cell| cell.trim().trim_matches('"').trim().to_string())
        .collect()
}

/// Every cell of `line` as a number, when they all are and there are two
/// or more.
fn numbers(line: &str, separator: char) -> Option<Vec<f64>> {
    let row = cells(line, separator);
    if row.len() < 2 {
        return None;
    }
    row.iter().map(|cell| number_of(cell, separator)).collect()
}

fn number_of(cell: &str, separator: char) -> Option<f64> {
    let cell = if separator == ';' {
        cell.replace(',', ".")
    } else {
        cell.to_string()
    };
    cell.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// The units of a line whose every cell is a unit (`s,V`, `(s),(mV)`,
/// `Second,Volt`), spelled-out names written as symbols; `None` for a line
/// of names.
fn units_row(line: &str, separator: char) -> Option<Vec<String>> {
    cells(line, separator)
        .iter()
        .map(|cell| {
            let bare = cell.trim_matches(['(', ')', '[', ']']).trim();
            let spelled = match bare.to_ascii_lowercase().as_str() {
                "second" | "seconds" => Some("s"),
                "volt" | "volts" => Some("V"),
                "amp" | "amps" | "ampere" | "amperes" => Some("A"),
                "hertz" => Some("Hz"),
                "watt" | "watts" => Some("W"),
                _ => None,
            };
            match spelled {
                Some(symbol) => Some(symbol.to_string()),
                None if bare.chars().count() <= 3 && !bare.chars().any(|c| c.is_ascii_digit()) => {
                    Some(bare.to_string())
                }
                None => None,
            }
        })
        .collect()
}

/// `CH1 (V)` as `("CH1", Some("V"))`; `Time [s]` as `("Time", Some("s"))`.
fn split_unit(cell: &str) -> (String, Option<String>) {
    for (open, close) in [('(', ')'), ('[', ']')] {
        if let Some(start) = cell.rfind(open)
            && cell.trim_end().ends_with(close)
        {
            let unit = cell[start + 1..cell.trim_end().len() - 1]
                .trim()
                .to_string();
            return (
                cell[..start].trim().to_string(),
                Some(unit).filter(|u| !u.is_empty()),
            );
        }
    }
    (cell.trim().to_string(), None)
}
