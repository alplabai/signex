//! The edits the measurement commands make to a notebook file: which call
//! Mark still valid rewrites, and where a new attachment is written.

use crate::id::BlockId;
use crate::record::NotebookRecord;

/// What Mark still valid acts on: an attachment, by its file inside
/// `notebook/attachments/`, or a reading, by its block id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaleTarget {
    Attachment(String),
    Reading(BlockId),
}

/// The notebook file and the line of the call `target` names, the first
/// one that stores part values; `None` when no such call is there.
pub fn locate(found: &NotebookRecord, target: &StaleTarget) -> Option<(String, u32)> {
    match target {
        StaleTarget::Attachment(file) => found
            .attachments
            .iter()
            .find(|a| &a.file == file && !a.taken_with.is_empty())
            .map(|a| (a.source_file.clone(), a.line)),
        StaleTarget::Reading(id) => found
            .readings
            .iter()
            .find(|r| r.id.as_ref() == Some(id) && !r.taken_with.is_empty())
            .map(|r| (r.file.clone(), r.line)),
    }
}

/// `text` with `call` written as a paragraph of its own at the end.
pub fn append_call(text: &str, call: &str) -> String {
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push_str(call);
    out.push('\n');
    out
}
