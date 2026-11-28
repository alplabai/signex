//! Opening a `.snxsym` whose pin table holds a control character (#3129).
//!
//! Builds before #3129 wrote such a character into the `pins_tsv` literal
//! string, which TOML forbids, so the file saved and then never opened. The
//! repairing read removes those characters from the pin tables and reports
//! each one; it never writes. The Symbol Editor opens the result read-only
//! until the user saves it.

use std::fmt;

use uuid::Uuid;

use super::{PIN_TSV_BODY_COLUMN, PIN_TSV_COLUMNS, SymbolFile, SymbolFileError};

/// One character the repairing read removed from a pin table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolLoadRepair {
    /// The symbol's name; empty when the table could not be matched to one.
    pub symbol: String,
    pub symbol_uuid: Option<Uuid>,
    /// The pin's number; empty for the header row.
    pub pin: String,
    /// The pin table column, as the file names it (`name`, `description`…).
    pub column: &'static str,
    pub character: char,
}

impl fmt::Display for SymbolLoadRepair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "symbol {:?}, pin {:?}: the {} holds the control character {}",
            self.symbol,
            self.pin,
            self.column,
            crate::toml_literal::code_point(self.character)
        )
    }
}

/// Where a removed character sat: the pin table (in file order), the line
/// of its body and the cell of that line.
struct Removed {
    table: usize,
    line: usize,
    cell: usize,
    character: char,
}

/// How the writer opens a pin table: at the start of a line.
const OPENER: &str = "\npins_tsv = '''";
const CLOSER: &str = "'''";

/// The body's first line is the rest of the opener line, then the header.
const FIRST_PIN_LINE: usize = 2;

impl SymbolFile {
    /// [`Self::from_bytes`] that also opens a file whose pin tables hold a
    /// control character TOML forbids there: those characters are removed
    /// and returned, one [`SymbolLoadRepair`] each. A file without any is
    /// read exactly as [`Self::from_bytes`] reads it, with no repairs.
    pub fn from_bytes_repairing(
        bytes: &[u8],
    ) -> Result<(Self, Vec<SymbolLoadRepair>), SymbolFileError> {
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Err(SymbolFileError::Empty);
        }
        let text = std::str::from_utf8(bytes)?;
        let Some((repaired, removed)) = strip_pin_tables(text) else {
            return Ok((Self::parse_toml(text)?, Vec::new()));
        };
        let file = Self::parse_toml(&repaired)?;
        let repairs = describe(&file, removed);
        Ok((file, repairs))
    }
}

/// The error a strict read gives instead of the TOML one when the file
/// opens once repaired, so the message says where the character is.
pub(super) fn explain_failure(text: &str) -> Option<SymbolFileError> {
    let (repaired, removed) = strip_pin_tables(text)?;
    let file = SymbolFile::parse_toml(&repaired).ok()?;
    let mut repairs = describe(&file, removed);
    let count = repairs.len();
    Some(SymbolFileError::RepairableControlChars {
        first: repairs.swap_remove(0),
        count,
    })
}

/// `text` without the control characters of its pin tables, and where they
/// were; `None` when there are none. Tab, LF and a CR of a CRLF are kept.
fn strip_pin_tables(text: &str) -> Option<(String, Vec<Removed>)> {
    let mut out = String::with_capacity(text.len());
    let mut removed = Vec::new();
    let mut rest = text;
    let mut table = 0;
    while let Some(at) = rest.find(OPENER) {
        let body_start = at + OPENER.len();
        out.push_str(&rest[..body_start]);
        let tail = &rest[body_start..];
        let end = tail.find(CLOSER).unwrap_or(tail.len());
        strip_table(&tail[..end], table, &mut out, &mut removed);
        rest = &tail[end..];
        table += 1;
    }
    if removed.is_empty() {
        return None;
    }
    out.push_str(rest);
    Some((out, removed))
}

fn strip_table(body: &str, table: usize, out: &mut String, removed: &mut Vec<Removed>) {
    let (mut line, mut cell) = (0, 0);
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => {
                line += 1;
                cell = 0;
                out.push(c);
            }
            '\t' => {
                cell += 1;
                out.push(c);
            }
            '\r' if chars.peek() == Some(&'\n') => out.push(c),
            c if c.is_ascii_control() => removed.push(Removed {
                table,
                line,
                cell,
                character: c,
            }),
            c => out.push(c),
        }
    }
}

/// Name the symbol, pin and column of each removed character. Pin tables
/// are in symbol order, one per symbol.
fn describe(file: &SymbolFile, removed: Vec<Removed>) -> Vec<SymbolLoadRepair> {
    removed
        .into_iter()
        .map(|r| {
            let symbol = file.symbols.get(r.table);
            let pin = r
                .line
                .checked_sub(FIRST_PIN_LINE)
                .and_then(|i| symbol?.pins.get(i));
            SymbolLoadRepair {
                symbol: symbol.map(|s| s.name.clone()).unwrap_or_default(),
                symbol_uuid: symbol.map(|s| s.uuid),
                pin: pin.map(|p| p.number.clone()).unwrap_or_default(),
                column: PIN_TSV_COLUMNS
                    .get(r.cell)
                    .copied()
                    .unwrap_or(PIN_TSV_BODY_COLUMN),
                character: r.character,
            }
        })
        .collect()
}
