//! Keeping a journal to its line cap (#2253): when it holds more lines than
//! the cap, the newest ones are copied into a temporary file that is renamed
//! over it, so a reader sees the old file or the new one, never half. It is a
//! two-pass stream (count the lines, then copy the tail), so memory stays
//! constant however large the journal is. It rotates down to 90 % of the cap, so
//! a journal that has just been rotated is not rewritten by the very next
//! append. Called with the project's journal lock held, through the folder
//! handle, so the same link and link-count checks apply. Lines the reader does
//! not know (a newer `v`) are kept as the bytes they are.

use std::io::{Read as _, Seek as _, SeekFrom, Write as _};

use super::entry::MIN_LINE_BYTES;
use super::safe_fs::SignexDir;

/// How much is read or written at a time.
const CHUNK: usize = 64 * 1024;

/// The number of lines a rotation leaves: 90 % of `max_lines`, at least one.
pub const fn lines_kept_after_rotation(max_lines: u64) -> u64 {
    let kept = max_lines / 10 * 9;
    if kept == 0 { 1 } else { kept }
}

/// Rotate the journal `name` if it holds more than `max_lines` lines.
pub(super) fn rotate_if_over(dir: &SignexDir, name: &str, max_lines: u32) -> Result<(), String> {
    let cap = u64::from(max_lines);
    let Some(mut file) = dir.read_file(name)? else {
        return Ok(());
    };
    let shown = dir.path().join(name);
    let io = |error: std::io::Error| format!("cannot rotate {}: {error}", shown.display());
    let size = file.metadata().map_err(io)?.len();
    // Every line is at least MIN_LINE_BYTES long: a file this short cannot
    // hold more lines than the cap, so it is not read.
    if size / (MIN_LINE_BYTES as u64) <= cap {
        return Ok(());
    }
    let total = count_lines(&mut file).map_err(io)?;
    if total <= cap {
        return Ok(());
    }
    let skip = total - lines_kept_after_rotation(cap);
    file.seek(SeekFrom::Start(0)).map_err(io)?;
    let temp = format!("{name}.{}-rot.tmp", std::process::id());
    // A temporary left by a crashed rotation of a process with this id.
    dir.remove(&temp);
    let mut out = dir.create_temp(&temp)?;
    let copied = copy_after_lines(&mut file, &mut out, skip).and_then(|()| out.sync_all());
    drop(out);
    if let Err(error) = copied {
        dir.remove(&temp);
        return Err(io(error));
    }
    dir.rename(&temp, name).inspect_err(|_| dir.remove(&temp))
}

fn count_lines(file: &mut std::fs::File) -> std::io::Result<u64> {
    let mut buffer = vec![0_u8; CHUNK];
    let mut lines = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(lines);
        }
        lines += buffer[..read].iter().filter(|byte| **byte == b'\n').count() as u64;
    }
}

/// Copy everything after the first `skip` lines of `from` to `to`.
fn copy_after_lines(
    from: &mut std::fs::File,
    to: &mut std::fs::File,
    mut skip: u64,
) -> std::io::Result<()> {
    let mut buffer = vec![0_u8; CHUNK];
    loop {
        let read = from.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        let mut chunk = &buffer[..read];
        while skip > 0 && !chunk.is_empty() {
            match chunk.iter().position(|byte| *byte == b'\n') {
                Some(end) => {
                    chunk = &chunk[end + 1..];
                    skip -= 1;
                }
                None => chunk = &[],
            }
        }
        to.write_all(chunk)?;
    }
}
