//! Validate, the first step of a profile's run (mockup row 114): ERC over
//! the job's project, DRC over its board when the run makes a board output,
//! and the BOM checks of a BOM output. The checks are the ones `check_erc`
//! and `check_drc` run, captured in `update` and run off the UI thread.

use std::path::Path;

use signex_types::check::{CheckReport, Severity};

use crate::app::Signex;

/// The most finding lines the Validate node's log keeps.
const MAX_LINES: usize = 50;

type ErcJob = Box<dyn FnOnce() -> CheckReport + Send>;
type DrcJob = Box<dyn FnOnce() -> Result<CheckReport, String> + Send>;

/// The checks of one Validate, captured from memory.
pub(crate) struct ChecksJob {
    erc: Option<ErcJob>,
    drc: Option<DrcJob>,
    bom_errors: usize,
    bom_warnings: usize,
}

/// What Validate found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidateOutcome {
    pub errors: usize,
    pub warnings: usize,
    /// One line a finding, `ERC: …`, `DRC: …`, at most [`MAX_LINES`].
    pub lines: Vec<String>,
    /// Which checks found an error: `erc`, `drc`, `bom`.
    pub failing: Vec<&'static str>,
}

impl ChecksJob {
    /// Run the checks. Blocking: run it off the UI thread.
    pub(crate) fn run(self) -> ValidateOutcome {
        let mut outcome = ValidateOutcome::default();
        if let Some(erc) = self.erc {
            outcome.add("erc", "ERC", erc());
        }
        if let Some(drc) = self.drc {
            match drc() {
                Ok(report) => outcome.add("drc", "DRC", report),
                Err(why) => {
                    outcome.errors += 1;
                    outcome.failing.push("drc");
                    outcome
                        .lines
                        .push(format!("DRC: the check could not run: {why}"));
                }
            }
        }
        if self.bom_errors > 0 || self.bom_warnings > 0 {
            outcome.errors += self.bom_errors;
            outcome.warnings += self.bom_warnings;
            if self.bom_errors > 0 {
                outcome.failing.push("bom");
            }
            outcome.lines.push(format!(
                "BOM checks: {} error(s), {} warning(s)",
                self.bom_errors, self.bom_warnings
            ));
        }
        outcome.lines.truncate(MAX_LINES);
        outcome
    }
}

impl ValidateOutcome {
    fn add(&mut self, id: &'static str, check: &str, report: CheckReport) {
        let mut errors = 0;
        for finding in &report.findings {
            match finding.severity {
                Severity::Error => errors += 1,
                Severity::Warning => self.warnings += 1,
                Severity::Info => continue,
            }
            self.lines.push(format!("{check}: {}", finding.message));
        }
        if errors > 0 {
            self.failing.push(id);
        }
        self.errors += errors;
    }

    /// The sentence a stopped run gives: which check found how many errors.
    pub fn summary(&self) -> String {
        let checks: Vec<String> = self
            .failing
            .iter()
            .map(|check| check.to_ascii_uppercase())
            .collect();
        format!(
            "{} found {} error(s) and {} warning(s)",
            if checks.is_empty() {
                "Validate".to_string()
            } else {
                checks.join(", ")
            },
            self.errors,
            self.warnings
        )
    }
}

impl Signex {
    /// The checks of a run whose outputs come from the open sheet `source`:
    /// ERC always, DRC when `board` (the project's board) is given, and the
    /// BOM checks' counts.
    pub(crate) fn validate_job(
        &self,
        source: &Path,
        board: Option<&Path>,
        bom: (usize, usize),
    ) -> ChecksJob {
        let project = self.document_state.document_project(source);
        let erc: ErcJob = Box::new(self.erc_check_job(source, project));
        let drc = board.map(|board| {
            let project = self.document_state.board_project(board, None);
            let job: DrcJob = Box::new(self.drc_check_job(board, project));
            job
        });
        ChecksJob {
            erc: Some(erc),
            drc,
            bom_errors: bom.0,
            bom_warnings: bom.1,
        }
    }
}
