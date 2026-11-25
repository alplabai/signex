//! What a run of Generate answers (mockup row 201, the command line): the
//! output path, pages, sections, requirement and check counts, open items,
//! stale references, whether the file was written, where each section's
//! data came from and what could not be read. Also the two rules of the
//! command line: a requirement failure exits 0 unless `--fail-on` names it,
//! and an existing report is replaced only when asked.

use serde::Serialize;
use signex_types::check::{CheckReport, Severity};

use super::facts::{Fact, ProjectFacts};
use super::render::{open_issue_labels, severity_count};
use super::requirements::{RequirementResult, RequirementRow};
use super::sections::{GenerateOptions, Section};
use super::{GENERATE_COMMAND, generated_path};

/// The answer of one run. Field names are the command line's, as drawn.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GenerateSummary {
    /// `notebook.generate`.
    pub command: &'static str,
    /// The project file's name.
    pub project: String,
    pub revision: Option<String>,
    pub variant: String,
    /// The report's path inside the project folder.
    pub output: String,
    pub pages: usize,
    /// The sections written, by token, in report order.
    pub sections: Vec<&'static str>,
    pub requirements: RequirementCounts,
    pub checks: Checks,
    /// The open Issue blocks, by id (or title when a block has none).
    pub open_items: Vec<String>,
    pub stale_references: usize,
    /// Whether the file was written; set by the caller that writes it.
    pub written: bool,
    pub status: GenerateStatus,
    /// Where each written section's data came from.
    pub sources: Vec<SourceNote>,
    /// What could not be read, by section (no simulation run yet, ...).
    pub not_read: Vec<NotRead>,
    /// Why the report did not compile, when it did not; left out when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
    /// The `--fail-on` conditions that held; left out when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failed_on: Vec<&'static str>,
}

/// How the run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerateStatus {
    Ok,
    /// A `--fail-on` condition held.
    Fail,
    /// The report did not compile (`problems` says why).
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RequirementCounts {
    pub total: usize,
    pub pass: usize,
    pub fail: usize,
    /// The failing requirement ids.
    pub failed: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Checks {
    /// `None` (null) when ERC was not run or read.
    pub erc: Option<CheckCounts>,
    pub drc: Option<CheckCounts>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CheckCounts {
    pub errors: usize,
    pub warnings: usize,
    pub waived: usize,
}

impl CheckCounts {
    fn of(report: &CheckReport) -> Self {
        Self {
            errors: severity_count(report, Severity::Error),
            warnings: severity_count(report, Severity::Warning),
            waived: report.waived.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceNote {
    pub section: &'static str,
    pub from: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotRead {
    pub section: &'static str,
    pub why: String,
}

impl GenerateSummary {
    /// The answer for a report of `pages` pages built from `facts`; not yet
    /// written.
    pub(super) fn of(
        facts: &ProjectFacts,
        options: &GenerateOptions,
        rows: &[RequirementRow],
        pages: usize,
        problems: Vec<String>,
    ) -> Self {
        let failed: Vec<String> = rows
            .iter()
            .filter(|r| r.result == RequirementResult::Fail)
            .map(|r| r.id.clone())
            .collect();
        let requirements = RequirementCounts {
            total: rows.len(),
            pass: rows
                .iter()
                .filter(|r| r.result == RequirementResult::Pass)
                .count(),
            fail: failed.len(),
            failed,
        };
        let (sources, not_read) = provenance(facts, &options.sections);
        let status = if problems.is_empty() {
            GenerateStatus::Ok
        } else {
            GenerateStatus::Error
        };
        Self {
            command: GENERATE_COMMAND,
            project: facts.project_file.clone(),
            revision: facts.revision.clone(),
            variant: facts.variant.clone(),
            output: generated_path(),
            pages,
            sections: options.sections.iter().map(|s| s.token()).collect(),
            requirements,
            checks: Checks {
                erc: facts.erc.value().map(CheckCounts::of),
                drc: facts.drc.value().map(CheckCounts::of),
            },
            open_items: open_issue_labels(facts),
            stale_references: facts.stale_references,
            written: false,
            status,
            sources,
            not_read,
            problems,
            failed_on: Vec::new(),
        }
    }

    /// Apply `--fail-on`: the conditions that hold are named and the status
    /// becomes `fail` (an `error` stays). True when any held.
    pub fn apply_fail_on(&mut self, conditions: &[FailOn]) -> bool {
        self.failed_on = conditions
            .iter()
            .filter(|c| c.holds(self))
            .map(|c| c.token())
            .collect();
        let failed = !self.failed_on.is_empty();
        if failed && self.status == GenerateStatus::Ok {
            self.status = GenerateStatus::Fail;
        }
        failed
    }

    /// The command line's exit code: 0 when the status is ok, 1 when a
    /// `--fail-on` condition held or the report did not compile.
    pub fn exit_code(&self) -> u8 {
        match self.status {
            GenerateStatus::Ok => 0,
            GenerateStatus::Fail | GenerateStatus::Error => 1,
        }
    }
}

/// Where each section of `sections` came from, and what was not read.
fn provenance(facts: &ProjectFacts, sections: &[Section]) -> (Vec<SourceNote>, Vec<NotRead>) {
    let mut sources = Vec::new();
    let mut not_read = Vec::new();
    let mut note = |section: Section, from: Option<&str>, why: Option<&str>| {
        let section = section.token();
        if let Some(from) = from {
            sources.push(SourceNote {
                section,
                from: from.to_string(),
            });
        }
        if let Some(why) = why {
            not_read.push(NotRead {
                section,
                why: why.to_string(),
            });
        }
    };
    fn parts<T>(fact: &Fact<T>) -> (Option<&str>, Option<&str>) {
        (fact.from(), fact.why_not())
    }
    let project = facts.project_file.as_str();
    for section in sections.iter().copied() {
        let facts_of: Vec<(Option<&str>, Option<&str>)> = match section {
            Section::Summary | Section::Architecture => vec![(Some(project), None)],
            Section::Requirements => vec![(Some(project), None), parts(&facts.requirement_check)],
            Section::Decisions => vec![parts(&facts.blocks)],
            Section::Bom => vec![parts(&facts.bom)],
            Section::Checks => vec![
                parts(&facts.erc),
                parts(&facts.drc),
                parts(&facts.requirement_check),
            ],
            Section::Simulation => vec![parts(&facts.simulation)],
            Section::Outputs => vec![parts(&facts.outputs)],
            Section::OpenItems => vec![parts(&facts.blocks)],
            Section::Changelog => vec![parts(&facts.changes)],
        };
        for (from, why) in facts_of {
            note(section, from, why);
        }
    }
    (sources, not_read)
}

/// A condition that makes the command line exit non-zero (`--fail-on`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailOn {
    /// A requirement fails.
    Requirements,
    /// ERC or DRC has an error.
    Checks,
}

impl FailOn {
    pub const ALL: [FailOn; 2] = [FailOn::Requirements, FailOn::Checks];

    pub fn token(self) -> &'static str {
        match self {
            FailOn::Requirements => "requirements",
            FailOn::Checks => "checks",
        }
    }

    fn holds(self, summary: &GenerateSummary) -> bool {
        let errors = [&summary.checks.erc, &summary.checks.drc]
            .into_iter()
            .flatten()
            .map(|counts| counts.errors)
            .sum::<usize>();
        self.holds_on(summary.requirements.fail as u64, errors as u64)
    }

    /// Whether the condition holds for `failed` failing requirements and
    /// `errors` ERC and DRC errors, as an answer read back from JSON says.
    pub fn holds_on(self, failed: u64, errors: u64) -> bool {
        match self {
            FailOn::Requirements => failed > 0,
            FailOn::Checks => errors > 0,
        }
    }
}

/// The conditions a comma-separated `--fail-on` list names
/// (`requirements,checks`); refused when it names another.
pub fn parse_fail_on(text: &str) -> Result<Vec<FailOn>, String> {
    let mut picked = Vec::new();
    for name in text.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        let Some(condition) = FailOn::ALL.into_iter().find(|c| c.token() == name) else {
            return Err(format!(
                "`--fail-on {name}`: the conditions are requirements and checks"
            ));
        };
        if !picked.contains(&condition) {
            picked.push(condition);
        }
    }
    if picked.is_empty() {
        return Err("`--fail-on` needs requirements, checks or both".to_string());
    }
    Ok(picked)
}

/// What writing the report does to the file already there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteDecision {
    /// No report yet: write it.
    Create,
    /// A report is there and replacing it was asked for (`--force`, or
    /// Regenerate): write it over the old one, as a whole.
    Replace,
    /// A report is there and replacing it was not asked for: leave it.
    Refuse,
}

/// Whether to write the report, given whether one `exists` and whether
/// replacing it was asked for. The command line never overwrites without
/// `--force`.
pub fn write_decision(exists: bool, replace: bool) -> WriteDecision {
    match (exists, replace) {
        (false, _) => WriteDecision::Create,
        (true, true) => WriteDecision::Replace,
        (true, false) => WriteDecision::Refuse,
    }
}
