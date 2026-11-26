//! The timeline of a net, part or pin (mockup row 199, margin lane): its
//! readings over time against the limit and the computed value, then the
//! list with revision, board and conditions. A failing reading is marked
//! fail, one taken with part values that have changed since is stale.

use std::cmp::Reverse;

use crate::attachment::{Attachment, DesignValues, ValueChange, reading_changes};
use crate::id::BlockId;
use crate::measurement::{Limit, Reading, Verdict};
use crate::reference::Reference;
use crate::timestamp::Timestamp;

/// One reading on the timeline.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TimelineEntry {
    pub id: Option<BlockId>,
    pub row: u32,
    pub file: String,
    pub line: u32,
    pub quantity: String,
    pub timestamp: Option<Timestamp>,
    pub value: Option<f64>,
    pub value_text: String,
    pub unit: String,
    pub verdict: Option<Verdict>,
    pub error: Option<f64>,
    pub rev: String,
    pub board: String,
    pub conditions: String,
    pub operator: String,
    /// The part values that changed since it was taken; empty when current.
    pub changes: Vec<ValueChange>,
    /// The reading that corrects it.
    pub superseded_by: Option<BlockId>,
}

impl TimelineEntry {
    pub fn is_stale(&self) -> bool {
        !self.changes.is_empty()
    }
}

/// The readings of one target, newest first.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Timeline {
    pub reference: Reference,
    /// The quantity shown; empty for every quantity of the target.
    pub quantity: String,
    /// The newest reading's unit and limit.
    pub unit: String,
    pub limit: Option<Limit>,
    /// The newest computed value given with a reading.
    pub computed: Option<f64>,
    pub entries: Vec<TimelineEntry>,
    pub readings: usize,
    pub fails: usize,
    pub stale: usize,
}

/// The quantities measured on `reference`, first written first.
pub fn quantities_of(readings: &[Reading], reference: &Reference) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for reading in readings.iter().filter(|r| r.is_of(reference)) {
        if !found.contains(&reading.quantity) {
            found.push(reading.quantity.clone());
        }
    }
    found
}

/// The timeline of `reference`: its readings of `quantity` (all of them
/// with `None`), newest first, undated last, each marked stale against
/// `values` through its own `taken-with:` or its attachment's.
pub fn timeline(
    readings: &[Reading],
    reference: &Reference,
    quantity: Option<&str>,
    attachments: &[Attachment],
    values: &DesignValues,
) -> Timeline {
    let mut chosen: Vec<&Reading> = readings
        .iter()
        .filter(|r| r.is_of(reference))
        .filter(|r| quantity.is_none_or(|q| r.quantity == q))
        .collect();
    chosen.sort_by_key(|r| (r.timestamp.is_none(), Reverse(r.timestamp)));
    let entries: Vec<TimelineEntry> = chosen
        .iter()
        .map(|reading| {
            let check = reading.check();
            let superseded_by = readings
                .iter()
                .find(|other| other.supersedes.is_some() && other.supersedes == reading.id)
                .and_then(|other| other.id.clone());
            TimelineEntry {
                id: reading.id.clone(),
                row: reading.row,
                file: reading.file.clone(),
                line: reading.line,
                quantity: reading.quantity.clone(),
                timestamp: reading.timestamp,
                value: reading.value,
                value_text: reading.value_text.clone(),
                unit: reading.unit.clone(),
                verdict: check.map(|c| c.verdict),
                error: check.and_then(|c| c.error),
                rev: reading.rev.clone(),
                board: reading.board.clone(),
                conditions: reading.conditions.clone(),
                operator: reading.operator.clone(),
                changes: reading_changes(reading, attachments, values),
                superseded_by,
            }
        })
        .collect();
    let newest = chosen.first();
    Timeline {
        reference: reference.clone(),
        quantity: quantity.unwrap_or_default().to_string(),
        unit: newest.map(|r| r.unit.clone()).unwrap_or_default(),
        limit: newest.and_then(|r| r.limit),
        computed: chosen.iter().find_map(|r| r.computed),
        readings: entries.len(),
        fails: entries
            .iter()
            .filter(|e| e.verdict == Some(Verdict::Fail))
            .count(),
        stale: entries.iter().filter(|e| e.is_stale()).count(),
        entries,
    }
}

/// The CSV columns of [`Timeline::to_csv`].
pub const CSV_HEADER: &str = "id,row,timestamp,quantity,value,unit,result,error_percent,rev,board,conditions,operator,stale,file,line";

impl Timeline {
    /// The list as CSV, newest first: one line per reading under
    /// [`CSV_HEADER`], the error in percent, `stale` true or false.
    pub fn to_csv(&self) -> String {
        let mut out = String::from(CSV_HEADER);
        out.push('\n');
        for entry in &self.entries {
            let cells = [
                entry
                    .id
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                entry.row.to_string(),
                entry.timestamp.map(|t| t.to_string()).unwrap_or_default(),
                entry.quantity.clone(),
                entry.value_text.clone(),
                entry.unit.clone(),
                entry
                    .verdict
                    .map(|v| v.label().to_lowercase())
                    .unwrap_or_default(),
                entry
                    .error
                    .map(|e| format!("{}", e * 100.0))
                    .unwrap_or_default(),
                entry.rev.clone(),
                entry.board.clone(),
                entry.conditions.clone(),
                entry.operator.clone(),
                entry.is_stale().to_string(),
                entry.file.clone(),
                entry.line.to_string(),
            ];
            let line: Vec<String> = cells.iter().map(|cell| csv_cell(cell)).collect();
            out.push_str(&line.join(","));
            out.push('\n');
        }
        out
    }
}

/// `cell` quoted when it holds a comma, a quote or a line break.
fn csv_cell(cell: &str) -> String {
    if cell.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}
