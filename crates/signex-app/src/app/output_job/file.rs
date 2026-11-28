//! The Export Job file on disk (#1299): the host side of
//! `signex_types::output_job`, which takes and returns the file's text and
//! does no IO (#2431). Each call is blocking IO: the app runs it from a task.

use std::path::Path;

use signex_types::output_job::{
    OutputJob, OutputJobError, RunRecord, check_output_job_size, encode_output_job,
    parse_output_job, with_run_record,
};

/// The job file's text, refused when it is larger than an Export Job reads.
fn read_text(path: &Path) -> Result<String, OutputJobError> {
    let size = std::fs::metadata(path)
        .map_err(|e| OutputJobError::Io(format!("{}: {e}", path.display())))?
        .len();
    check_output_job_size(path, size)?;
    std::fs::read_to_string(path)
        .map_err(|e| OutputJobError::Io(format!("{}: {e}", path.display())))
}

/// Write `text` to `path` crash-safely (temp, fsync, rename).
fn write_text(path: &Path, text: &str) -> Result<(), OutputJobError> {
    signex_types::atomic_io::atomic_write(path, text.as_bytes())
        .map_err(|e| OutputJobError::Io(e.to_string()))
}

/// Read the job file at `path`.
pub(crate) fn read_output_job(path: &Path) -> Result<OutputJob, OutputJobError> {
    parse_output_job(&read_text(path)?)
}

/// Write the job to `path` atomically.
pub(crate) fn write_output_job(path: &Path, job: &OutputJob) -> Result<(), OutputJobError> {
    write_text(path, &encode_output_job(job)?)
}

/// Add `record` to the history of the job file at `path`, keeping the rest
/// of the file as it is on disk.
pub(crate) fn append_run_record(path: &Path, record: RunRecord) -> Result<(), OutputJobError> {
    let text = with_run_record(&read_text(path)?, record)?;
    write_text(path, &text)
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
