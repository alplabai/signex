//! Saved readings are read-only (mockup row 199): a correction is a new
//! reading that supersedes the old one. The editor asks which saved
//! readings an edit would change before it saves.

use crate::id::BlockId;
use crate::measurement::Reading;

/// Which reading: its file, its block id and its row in the block.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReadingKey {
    pub file: String,
    pub id: BlockId,
    pub row: u32,
    /// Its line in the saved file.
    pub line: u32,
}

/// The saved readings that `edited` changes or removes, in saved order. A
/// reading is found again by file, id and row; moving it to another line
/// is no change. A reading with no id cannot be told apart from a new one
/// and is not held read-only.
pub fn changed_saved_readings(saved: &[Reading], edited: &[Reading]) -> Vec<ReadingKey> {
    saved
        .iter()
        .filter_map(|old| {
            let id = old.id.clone()?;
            let same = edited.iter().find(|new| {
                new.file == old.file && new.id.as_ref() == Some(&id) && new.row == old.row
            });
            let unchanged = same.is_some_and(|new| recorded(new) == recorded(old));
            (!unchanged).then(|| ReadingKey {
                file: old.file.clone(),
                id,
                row: old.row,
                line: old.line,
            })
        })
        .collect()
}

/// What a reading records, without where it is written.
fn recorded(reading: &Reading) -> Reading {
    Reading {
        line: 0,
        ..reading.clone()
    }
}
