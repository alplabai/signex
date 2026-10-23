//! Bookkeeping for `.snxsym` containers the adapter could not read (#690).
//!
//! A container that fails to parse — hand-edited badly, truncated by a bad
//! merge, or written by a newer Signex with a bumped format token — must
//! affect only itself. The scan skips it and records it here so it is
//! reported once (per distinct error) instead of failing every symbol
//! operation in the library or flooding the log on every scan.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Longest error text kept for a skipped container, in characters.
const MAX_ERROR_CHARS: usize = 400;

/// One `.snxsym` container the scan skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolFileProblem {
    /// Absolute path of the unreadable container. The file is never
    /// rewritten or deleted by the adapter.
    pub path: PathBuf,
    /// Parse error, collapsed to one line and bounded in length.
    pub error: String,
}

impl SymbolFileProblem {
    pub(super) fn new(path: PathBuf, error: &str) -> Self {
        let one_line = error.split_whitespace().collect::<Vec<_>>().join(" ");
        let error = if one_line.chars().count() > MAX_ERROR_CHARS {
            let cut: String = one_line.chars().take(MAX_ERROR_CHARS - 1).collect();
            format!("{cut}…")
        } else {
            one_line
        };
        Self { path, error }
    }
}

/// What the latest scan found, plus what has already been reported.
#[derive(Debug, Default)]
pub(super) struct SymbolProblemLedger {
    current: BTreeMap<PathBuf, String>,
    reported: BTreeMap<PathBuf, String>,
}

impl SymbolProblemLedger {
    /// Replace the current set with `found`; return the entries that were
    /// not yet reported with this exact error, and mark them reported.
    pub(super) fn observe(&mut self, found: &[SymbolFileProblem]) -> Vec<SymbolFileProblem> {
        self.current = found
            .iter()
            .map(|p| (p.path.clone(), p.error.clone()))
            .collect();
        // A file that is healthy again may fail the same way later; report
        // it afresh then.
        self.reported
            .retain(|path, _| self.current.contains_key(path));
        let mut fresh = Vec::new();
        for p in found {
            if self.reported.get(&p.path) != Some(&p.error) {
                self.reported.insert(p.path.clone(), p.error.clone());
                fresh.push(p.clone());
            }
        }
        fresh
    }

    pub(super) fn current(&self) -> Vec<SymbolFileProblem> {
        self.current
            .iter()
            .map(|(path, error)| SymbolFileProblem {
                path: path.clone(),
                error: error.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem(name: &str, error: &str) -> SymbolFileProblem {
        SymbolFileProblem::new(PathBuf::from(name), error)
    }

    #[test]
    fn same_problem_is_reported_once_across_scans() {
        let mut ledger = SymbolProblemLedger::default();
        let p = problem("a.snxsym", "bad");
        assert_eq!(ledger.observe(std::slice::from_ref(&p)).len(), 1);
        assert!(ledger.observe(std::slice::from_ref(&p)).is_empty());
        assert_eq!(ledger.current(), vec![p]);
    }

    #[test]
    fn changed_error_or_recovered_then_rebroken_file_is_reported_again() {
        let mut ledger = SymbolProblemLedger::default();
        assert_eq!(ledger.observe(&[problem("a.snxsym", "one")]).len(), 1);
        assert_eq!(ledger.observe(&[problem("a.snxsym", "two")]).len(), 1);
        assert!(ledger.observe(&[]).is_empty());
        assert_eq!(ledger.observe(&[problem("a.snxsym", "two")]).len(), 1);
    }

    #[test]
    fn error_text_is_one_line_and_bounded() {
        let long = format!("line one\n  line two\n{}", "x".repeat(900));
        let p = problem("a.snxsym", &long);
        assert!(!p.error.contains('\n'));
        assert!(p.error.chars().count() <= MAX_ERROR_CHARS);
    }
}
