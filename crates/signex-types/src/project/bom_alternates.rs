//! Alternate parts per BOM line (#2938): for a line of the project's BOM,
//! the library rows that may be bought in its place, in order of
//! preference. A line is named by its key (`signex_bom::line_key`): the
//! library row its parts were placed from, `<library id>/<row id>`, or
//! `mpn:<MPN>` for parts placed without one. An alternate is a library row,
//! `<library id>/<row id>`. Stored in the `.snxprj` under `bom_alternates`,
//! skipped when empty; no format token changes. See `docs/CLI.md`, "BOM".

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use super::ProjectData;

/// The longest line key or row key kept, in characters.
pub const MAX_BOM_KEY_CHARS: usize = 256;

/// The prefix of a line key that names its parts by MPN.
pub const MPN_LINE_PREFIX: &str = "mpn:";

/// One BOM line's alternates, most preferred first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BomLineAlternates {
    pub line: String,
    #[serde(default)]
    pub rows: Vec<String>,
}

/// Why an alternate edit was refused. A refused edit changes nothing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BomAlternateProblem {
    #[error(
        "`{0}` is no BOM line key: give the `key` `query_bom` answers for the line (`<library id>/<row id>` or `mpn:<MPN>`)"
    )]
    BadLine(String),
    #[error("`{0}` is no library row: give it as `<library id>/<row id>`")]
    BadRow(String),
    #[error("{row} is the line's own part, not an alternate")]
    OwnPart { row: String },
    #[error("{row} is already an alternate of line {line}")]
    AlreadyListed { line: String, row: String },
    #[error("{row} is no alternate of line {line}")]
    NotListed { line: String, row: String },
    #[error("line {line} has {count} alternate(s): `position` is 1 to {count}, not {position}")]
    NoPosition {
        line: String,
        position: usize,
        count: usize,
    },
}

/// `<library id>/<row id>`, both hyphenated uuids, as written; `None` for
/// anything else.
fn row_key(text: &str) -> Option<String> {
    let (library, row) = text.trim().split_once('/')?;
    let library = Uuid::parse_str(library.trim()).ok()?;
    let row = Uuid::parse_str(row.trim()).ok()?;
    Some(format!("{library}/{row}"))
}

/// A library row key in its stored form, or the reason it is refused.
pub fn bom_row_key(text: &str) -> Result<String, BomAlternateProblem> {
    row_key(text).ok_or_else(|| BomAlternateProblem::BadRow(text.trim().to_string()))
}

/// A line key in its stored form: a row key, or `mpn:` and the MPN
/// trimmed and upper-cased (as the BOM matches MPNs). Refused when it is
/// neither, too long, or holds a control character.
pub fn bom_line_key(text: &str) -> Result<String, BomAlternateProblem> {
    let text = text.trim();
    let bad = || BomAlternateProblem::BadLine(text.to_string());
    if text.chars().count() > MAX_BOM_KEY_CHARS || text.chars().any(char::is_control) {
        return Err(bad());
    }
    if let Some(key) = row_key(text) {
        return Ok(key);
    }
    match text.strip_prefix(MPN_LINE_PREFIX).map(str::trim) {
        Some(mpn) if !mpn.is_empty() => Ok(format!("{MPN_LINE_PREFIX}{}", mpn.to_uppercase())),
        _ => Err(bad()),
    }
}

impl ProjectData {
    /// The alternates of the line `line` (a stored key), most preferred
    /// first; empty when it has none.
    pub fn bom_alternates_of(&self, line: &str) -> &[String] {
        self.bom_alternates
            .iter()
            .find(|entry| entry.line == line)
            .map_or(&[], |entry| entry.rows.as_slice())
    }

    /// Add `row` after the line's other alternates.
    pub fn add_bom_alternate(&mut self, line: &str, row: &str) -> Result<(), BomAlternateProblem> {
        let (line, row) = (bom_line_key(line)?, bom_row_key(row)?);
        if line == row {
            return Err(BomAlternateProblem::OwnPart { row });
        }
        if self.bom_alternates_of(&line).contains(&row) {
            return Err(BomAlternateProblem::AlreadyListed { line, row });
        }
        match self.bom_alternates.iter_mut().find(|e| e.line == line) {
            Some(entry) => entry.rows.push(row),
            None => self.bom_alternates.push(BomLineAlternates {
                line,
                rows: vec![row],
            }),
        }
        Ok(())
    }

    /// Take `row` off the line's alternates; a line left with none is
    /// dropped, so the key is not saved empty.
    pub fn remove_bom_alternate(
        &mut self,
        line: &str,
        row: &str,
    ) -> Result<(), BomAlternateProblem> {
        let (line, row) = (bom_line_key(line)?, bom_row_key(row)?);
        let place = self.alternate_place(&line, &row)?;
        let entry = &mut self.bom_alternates[place.0];
        entry.rows.remove(place.1);
        if entry.rows.is_empty() {
            self.bom_alternates.remove(place.0);
        }
        Ok(())
    }

    /// Move `row` to `position` (1 is the most preferred) among the line's
    /// alternates.
    pub fn move_bom_alternate(
        &mut self,
        line: &str,
        row: &str,
        position: usize,
    ) -> Result<(), BomAlternateProblem> {
        let (line, row) = (bom_line_key(line)?, bom_row_key(row)?);
        let (entry, at) = self.alternate_place(&line, &row)?;
        let count = self.bom_alternates[entry].rows.len();
        if position == 0 || position > count {
            return Err(BomAlternateProblem::NoPosition {
                line,
                position,
                count,
            });
        }
        let rows = &mut self.bom_alternates[entry].rows;
        let moved = rows.remove(at);
        rows.insert(position - 1, moved);
        Ok(())
    }

    /// The entry of `line` and the place of `row` in it.
    fn alternate_place(
        &self,
        line: &str,
        row: &str,
    ) -> Result<(usize, usize), BomAlternateProblem> {
        let not_listed = || BomAlternateProblem::NotListed {
            line: line.to_string(),
            row: row.to_string(),
        };
        let entry = self
            .bom_alternates
            .iter()
            .position(|e| e.line == line)
            .ok_or_else(not_listed)?;
        let at = self.bom_alternates[entry]
            .rows
            .iter()
            .position(|r| r == row)
            .ok_or_else(not_listed)?;
        Ok((entry, at))
    }
}
