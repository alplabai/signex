//! Appending one line to a journal (#2253), safely: under an exclusive
//! advisory lock on a sibling file, as one `write` of the whole line, and
//! rotated at the line cap while the lock is still held. See
//! `docs/AGENT_JOURNALS.md`, "Append".

use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::Path;

use super::rotate::rotate_if_over;
use super::safe_fs::{ensure_signex_dir, open_append, open_lock};

/// The lock both journals of a project share. It is a sibling, never the
/// journal itself: rotation renames the journal, which would orphan a lock on
/// the old file. The managed `.gitignore` block always lists it.
pub const LOCK_FILE: &str = "agent-journal.lock";

/// One of a project's two journals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Journal {
    /// `agent-edits.jsonl`: the document-changing agent commands.
    Edits,
    /// `agent-access.jsonl`: connections and their capabilities.
    Access,
}

impl Journal {
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Edits => "agent-edits.jsonl",
            Self::Access => "agent-access.jsonl",
        }
    }
}

/// Append `line` (no newline in it) to `journal` of the project in
/// `project_dir`, keeping at most `max_lines` lines.
///
/// A crash can leave one partial last line; the next append starts on a new
/// line, so it is never fused with it.
pub fn append_line(
    project_dir: &Path,
    journal: Journal,
    line: &str,
    max_lines: u32,
) -> Result<(), String> {
    let dir = ensure_signex_dir(project_dir)?;
    let lock_path = dir.join(LOCK_FILE);
    let lock = open_lock(&lock_path)?;
    lock.lock()
        .map_err(|error| format!("cannot lock {}: {error}", lock_path.display()))?;
    let path = dir.join(journal.file_name());
    let mut file = open_append(&path)?;
    let mut bytes = Vec::with_capacity(line.len() + 2);
    if ends_mid_line(&mut file)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?
    {
        bytes.push(b'\n');
    }
    bytes.extend_from_slice(line.as_bytes());
    bytes.push(b'\n');
    // One syscall for a plain file: the line lands whole or not at all.
    file.write_all(&bytes)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    drop(file);
    rotate_if_over(&path, max_lines)
    // `lock` is released when it drops, after the rotation.
}

/// Does the file end without a newline (a crash cut its last line)?
fn ends_mid_line(file: &mut std::fs::File) -> std::io::Result<bool> {
    if file.metadata()?.len() == 0 {
        return Ok(false);
    }
    file.seek(SeekFrom::End(-1))?;
    let mut last = [0_u8; 1];
    file.read_exact(&mut last)?;
    Ok(last[0] != b'\n')
}
