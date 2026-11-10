//! Reading a journal (#2253), as the viewer and the tests do: whole lines
//! only, a trailing partial line skipped, a line of a newer `v` skipped, and
//! both said. Nothing is rewritten.

use std::io::Read as _;
use std::path::Path;

use serde::de::DeserializeOwned;

use super::entry::JOURNAL_VERSION;
use super::safe_fs::open_read;

/// The most a journal is read into memory.
pub const MAX_READ_BYTES: u64 = 512 * 1024 * 1024;

/// What a journal held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report<T> {
    /// The lines this build understands, in file order.
    pub entries: Vec<T>,
    /// The file ends in a line a crash cut short; it was skipped.
    pub partial_tail: bool,
    /// Lines of a newer `v`, skipped and left as they are.
    pub newer: usize,
    /// Whole lines that are not an entry of this version, skipped.
    pub damaged: usize,
}

impl<T> Default for Report<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            partial_tail: false,
            newer: 0,
            damaged: 0,
        }
    }
}

impl<T> Report<T> {
    /// What was skipped, one sentence each, for the Messages panel or stderr.
    pub fn problems(&self, path: &Path) -> Vec<String> {
        let name = path.display();
        let mut out = Vec::new();
        if self.partial_tail {
            out.push(format!(
                "{name} ends in a partial line (a crash cut it short); it was skipped"
            ));
        }
        if self.newer > 0 {
            out.push(format!(
                "{name} has {} line(s) from a newer Signex; they were skipped and left as they are",
                self.newer
            ));
        }
        if self.damaged > 0 {
            out.push(format!(
                "{name} has {} line(s) that are not journal entries; they were skipped",
                self.damaged
            ));
        }
        out
    }
}

/// Read the journal at `path`; a missing file is an empty report.
pub fn read_journal<T: DeserializeOwned>(path: &Path) -> Result<Report<T>, String> {
    let Some(file) = open_read(path)? else {
        return Ok(Report::default());
    };
    let mut bytes = Vec::new();
    file.take(MAX_READ_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if bytes.len() as u64 > MAX_READ_BYTES {
        return Err(format!(
            "{} is larger than {MAX_READ_BYTES} bytes; it was not read",
            path.display()
        ));
    }
    Ok(parse_lines(&bytes))
}

fn parse_lines<T: DeserializeOwned>(bytes: &[u8]) -> Report<T> {
    let mut report = Report::default();
    let mut rest = bytes;
    while !rest.is_empty() {
        let Some(end) = rest.iter().position(|byte| *byte == b'\n') else {
            report.partial_tail = true;
            break;
        };
        let (line, after) = (&rest[..end], &rest[end + 1..]);
        rest = after;
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match classify::<T>(line) {
            Line::Entry(entry) => report.entries.push(entry),
            Line::Newer => report.newer += 1,
            Line::Damaged => report.damaged += 1,
        }
    }
    report
}

enum Line<T> {
    Entry(T),
    Newer,
    Damaged,
}

fn classify<T: DeserializeOwned>(line: &[u8]) -> Line<T> {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(line) else {
        return Line::Damaged;
    };
    match value.get("v").and_then(serde_json::Value::as_u64) {
        Some(v) if v > JOURNAL_VERSION => Line::Newer,
        Some(_) => serde_json::from_value(value).map_or(Line::Damaged, Line::Entry),
        None => Line::Damaged,
    }
}
