//! Reading a sheet's child references from a file nobody vouches for.
//!
//! The ownership walk follows `child_sheets` references written inside sheet
//! files, so every reference and every file it reaches is untrusted input.
//! Following a reference must never reach a remote host, a device or an
//! unbounded stream, and one walk must not run forever. Anything refused here
//! is reported as an unreadable sheet, which under the strict ownership rule
//! (#756) switches Discard All off: the safe side.
//!
//! Local `../` and absolute references are NOT refused: ownership needs them
//! (#741). Which network names are refused is [`super::reference_policy`].

use std::io::Read;
use std::path::Path;

/// Largest sheet file the walk reads. A real sheet is far smaller; this only
/// bounds a hostile or corrupt file.
pub(crate) const MAX_SHEET_BYTES: u64 = 64 * 1024 * 1024;

/// Most sheets one walk visits before it gives up.
pub(crate) const MAX_SHEETS_VISITED: usize = 10_000;

/// Bounds for one ownership walk. A struct so a test can use tiny ones.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WalkLimits {
    pub(crate) max_sheet_bytes: u64,
    pub(crate) max_sheets: usize,
}

impl Default for WalkLimits {
    fn default() -> Self {
        Self {
            max_sheet_bytes: MAX_SHEET_BYTES,
            max_sheets: MAX_SHEETS_VISITED,
        }
    }
}

/// Open `path` for reading; on Unix with `O_NONBLOCK`, so a FIFO or device
/// swapped in after the metadata check cannot hang the open.
fn open_nonblocking(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    options.open(path)
}

/// `child_sheets` filenames of the sheet at `path`. A missing file has none;
/// a file that is not a regular file, is larger than `max_bytes`, or cannot be
/// read or parsed returns why.
pub(crate) fn read_child_refs(path: &Path, max_bytes: u64) -> Result<Vec<String>, String> {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return Err("is not a regular file".to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("could not be read: {e}")),
    }
    // The path may have been swapped for a FIFO since the check above, and
    // opening a FIFO that has no writer blocks forever: open without
    // blocking, then decide on the handle that was actually opened.
    let file = open_nonblocking(path).map_err(|e| format!("could not be read: {e}"))?;
    match file.metadata() {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return Err("is not a regular file".to_string()),
        Err(e) => return Err(format!("could not be read: {e}")),
    }
    // One byte past the limit tells an oversized file from one that fits
    // exactly; the metadata length alone could be stale or a lie.
    let mut text = String::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_string(&mut text)
        .map_err(|e| format!("could not be read: {e}"))?;
    if text.len() as u64 > max_bytes {
        return Err(format!("is larger than {max_bytes} bytes"));
    }
    signex_types::format::SnxSchematic::parse(&text)
        .map(|parsed| {
            parsed
                .sheet
                .child_sheets
                .into_iter()
                .map(|c| c.filename)
                .collect()
        })
        .map_err(|e| format!("could not be parsed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_oversized_sheet_is_unreadable_and_one_at_the_limit_is_read() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("big.snxsch");
        std::fs::write(&path, vec![b'x'; 200]).expect("write");

        let over = read_child_refs(&path, 199).expect_err("over the limit");
        // Exactly at the limit is not oversized; it then fails to PARSE.
        let at = read_child_refs(&path, 200).expect_err("not a sheet");

        assert!(over.contains("larger than 199"), "{over}");
        assert!(at.contains("could not be parsed"), "{at}");
    }

    #[test]
    fn a_directory_is_not_a_regular_file() {
        let tmp = tempfile::tempdir().expect("tempdir");

        let err = read_child_refs(tmp.path(), MAX_SHEET_BYTES).expect_err("directory");

        assert!(err.contains("not a regular file"), "{err}");
    }

    #[test]
    fn a_missing_file_has_no_references() {
        let tmp = tempfile::tempdir().expect("tempdir");

        let refs = read_child_refs(&tmp.path().join("no.snxsch"), MAX_SHEET_BYTES);

        assert_eq!(refs, Ok(Vec::new()));
    }

    #[cfg(unix)]
    #[test]
    fn a_device_is_never_read() {
        let err = read_child_refs(Path::new("/dev/zero"), MAX_SHEET_BYTES).expect_err("device");

        assert!(err.contains("not a regular file"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn opening_a_fifo_with_no_writer_does_not_block_and_is_not_a_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let fifo = tmp.path().join("pipe.snxsch");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo runs");
        assert!(made.success());

        // A plain `File::open` here blocks until a writer appears.
        let file = open_nonblocking(&fifo).expect("opens without a writer");

        assert!(!file.metadata().expect("handle metadata").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn a_fifo_is_never_opened() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let fifo = tmp.path().join("pipe.snxsch");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo runs");
        assert!(made.success());

        // Opening a FIFO with no writer would block forever; the check must
        // come first.
        let err = read_child_refs(&fifo, MAX_SHEET_BYTES).expect_err("fifo");

        assert!(err.contains("not a regular file"), "{err}");
    }
}
