//! Keeping a journal to its line cap (#2253): when it holds more lines than
//! the cap, the newest `max_lines` are written to a temporary file that is
//! renamed over it (`atomic_write`), so a reader sees the old file or the new
//! one, never half. Called with the project's journal lock held. Lines the
//! reader does not know (a newer `v`) are kept as the bytes they are.

use std::io::Read as _;
use std::path::Path;

use signex_types::atomic_io::atomic_write;

use super::entry::MIN_LINE_BYTES;
use super::safe_fs::open_read;

/// The most a journal is read into memory to be rotated.
pub const MAX_ROTATE_BYTES: u64 = 512 * 1024 * 1024;

/// Rotate `path` if it holds more than `max_lines` lines.
pub(super) fn rotate_if_over(path: &Path, max_lines: u32) -> Result<(), String> {
    let cap = u64::from(max_lines);
    let Some(mut file) = open_read(path)? else {
        return Ok(());
    };
    let size = file
        .metadata()
        .map_err(|error| format!("cannot look at {}: {error}", path.display()))?
        .len();
    // Every line is at least MIN_LINE_BYTES long: a file this short cannot
    // hold more lines than the cap, so it is not read.
    if size / (MIN_LINE_BYTES as u64) <= cap {
        return Ok(());
    }
    if size > MAX_ROTATE_BYTES {
        return Err(format!(
            "{} is larger than {MAX_ROTATE_BYTES} bytes; it was not rotated",
            path.display()
        ));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    drop(file);
    let Some(kept) = newest_lines(&bytes, cap) else {
        return Ok(());
    };
    atomic_write(path, kept).map_err(|error| format!("cannot rotate {}: {error}", path.display()))
}

/// The last `cap` whole lines of `bytes` (a partial last line is dropped),
/// or `None` when there are not more than `cap`.
fn newest_lines(bytes: &[u8], cap: u64) -> Option<&[u8]> {
    let ends: Vec<usize> = bytes
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte == b'\n')
        .map(|(index, _)| index)
        .collect();
    let total = ends.len() as u64;
    if total <= cap {
        return None;
    }
    let first_dropped = usize::try_from(total - cap - 1).ok()?;
    let last = *ends.last()?;
    Some(&bytes[ends[first_dropped] + 1..=last])
}
