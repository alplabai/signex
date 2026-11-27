//! How the dialog words a run that went wrong (#3124): a converter that
//! crashed or stopped without a result, one that exited 1 with its own
//! error, and the end of its error output shown under either. Pure.
//!
//! The engine's own wording (`ImportError::to_translatable`) stays for the
//! notices and the log; the dialog names the two converter failures in the
//! terms the user meets them in.

use signex_i18n::{Translatable, ids};

use super::result::ImportResult;
use crate::library_import::report::Severity;
use crate::library_import::{ImportError, RunFailure};

/// How many lines of the converter's error output the dialog shows.
pub const STDERR_LINES: usize = 12;

/// A failed run as the dialog shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct RunProblem {
    pub reason: Translatable,
    /// The last [`STDERR_LINES`] lines of the converter's error output,
    /// trimmed; empty when it wrote none.
    pub stderr_tail: String,
}

impl RunProblem {
    pub fn of(failure: &RunFailure) -> Self {
        Self {
            reason: failure_reason(&failure.error),
            stderr_tail: last_lines(&failure.stderr_tail, STDERR_LINES),
        }
    }
}

/// Why the run failed, in the dialog's words.
pub fn failure_reason(error: &ImportError) -> Translatable {
    match error {
        // No summary line: the converter crashed or was stopped from outside.
        ImportError::NoSummary { status, .. } => {
            Translatable::new(ids::LIBRARY_IMPORT_DIALOG_CRASHED).arg("status", status.as_str())
        }
        // Exit 1 with a summary (contract §3.3): nothing usable written.
        ImportError::ConverterFailed { message } => {
            Translatable::new(ids::LIBRARY_IMPORT_DIALOG_FATAL).arg("error", message.as_str())
        }
        other => other.to_translatable(),
    }
}

/// The last `count` non-blank lines of `text`, trimmed.
pub fn last_lines(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .collect();
    lines[lines.len().saturating_sub(count)..].join("\n")
}

impl ImportResult {
    /// Items the run lost (dropped or not imported) when it exited 2
    /// (contract §3.3): the report's count, else the summary's skipped.
    pub fn lost_items(&self) -> u64 {
        match (&self.report, &self.run) {
            (Some(_), _) => (self.count(Severity::Dropped) + self.count(Severity::Error)) as u64,
            (None, Some(facts)) => facts.counts.skipped,
            (None, None) => 0,
        }
    }
}
