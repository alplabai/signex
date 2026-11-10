//! Appending one line to a journal (#2253), safely: under an exclusive
//! advisory lock on a sibling file, as one `write` of the whole line, and
//! rotated at the line cap while the lock is still held. Every file is opened
//! through the `.signex/` folder handle ([`SignexDir`]). See
//! `docs/AGENT_JOURNALS.md`, "Append".

use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::Path;

use super::rotate::rotate_if_over;
use super::safe_fs::SignexDir;

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
    let dir = SignexDir::create(project_dir)?;
    append_line_in(&dir, journal, line, max_lines)
}

/// [`append_line`] into an already opened `.signex/` folder: the files are
/// opened through its handle, so a folder swapped for a link after it was
/// opened changes nothing.
pub(crate) fn append_line_in(
    dir: &SignexDir,
    journal: Journal,
    line: &str,
    max_lines: u32,
) -> Result<(), String> {
    let lock = dir.lock_file(LOCK_FILE)?;
    lock.lock().map_err(|error| {
        format!(
            "cannot lock {}: {error}",
            dir.path().join(LOCK_FILE).display()
        )
    })?;
    let name = journal.file_name();
    let shown = dir.path().join(name);
    let mut file = dir.append_file(name)?;
    let mut bytes = Vec::with_capacity(line.len() + 2);
    if ends_mid_line(&mut file)
        .map_err(|error| format!("cannot read {}: {error}", shown.display()))?
    {
        bytes.push(b'\n');
    }
    bytes.extend_from_slice(line.as_bytes());
    bytes.push(b'\n');
    // One syscall for a plain file: the line lands whole or not at all.
    file.write_all(&bytes)
        .map_err(|error| format!("cannot write {}: {error}", shown.display()))?;
    drop(file);
    rotate_if_over(dir, name, max_lines)
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
