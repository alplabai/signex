//! The lines of the drift table (mockup row 135): instances of one part
//! that show the same line except the designator, and carry no conflict,
//! share one row listing every designator ("R10, R11").

use super::{BumpKind, LibraryUpdateEntry, LibraryUpdatesState};

/// One line of the drift table: the entries it stands for, in designator
/// order. Never empty.
#[derive(Debug, Clone)]
pub struct DriftRow<'a> {
    pub entries: Vec<&'a LibraryUpdateEntry>,
}

impl<'a> DriftRow<'a> {
    /// The first entry: every cell but the designators reads from it, and
    /// its checkbox stands for the row (the entries share a `lib_id`, so
    /// they share a checkbox).
    pub fn lead(&self) -> &'a LibraryUpdateEntry {
        self.entries[0]
    }

    /// The designators of the row, comma-separated.
    pub fn ref_des(&self) -> String {
        self.entries
            .iter()
            .map(|e| e.ref_des.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// `entry` would draw the same line as this row, but for its designator.
    fn takes(&self, entry: &LibraryUpdateEntry) -> bool {
        let lead = self.lead();
        lead.conflicts.is_empty()
            && entry.conflicts.is_empty()
            && lead.lib_id == entry.lib_id
            && lead.library_id == entry.library_id
            && lead.row_id == entry.row_id
            && lead.internal_pn == entry.internal_pn
            && lead.current_version == entry.current_version
            && lead.latest_version == entry.latest_version
    }
}

impl LibraryUpdatesState {
    /// The table rows, in the order of their first entry.
    pub fn rows(&self) -> Vec<DriftRow<'_>> {
        let mut rows: Vec<DriftRow<'_>> = Vec::new();
        for entry in &self.entries {
            match rows.iter_mut().find(|row| row.takes(entry)) {
                Some(row) => row.entries.push(entry),
                None => rows.push(DriftRow {
                    entries: vec![entry],
                }),
            }
        }
        rows
    }

    /// Number of table rows of bump `kind`: the count on its summary pill.
    pub fn count_of(&self, kind: BumpKind) -> usize {
        self.rows()
            .iter()
            .filter(|row| row.lead().bump_kind == kind)
            .count()
    }
}
