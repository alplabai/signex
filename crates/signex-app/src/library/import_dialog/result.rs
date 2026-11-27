//! The end of step 3 (mockup row 211, "Done: result" and "Done: loss
//! report"): what a finished import did, and its loss report grouped by
//! severity and code. Also the same report opened later from the Library
//! Browser, read from the library's stored `import-report.json`. Pure.

use std::path::{Path, PathBuf};

use signex_i18n::Translatable;

use super::preview::CHIP_SEVERITIES;
use super::{ImportDialogState, Step};
use crate::library_import::events::Counts;
use crate::library_import::report::{ImportReport, REPORT_MD, ReportItem, Severity};
use crate::library_import::{ExitKind, ImportJobOutcome, RunMode};

/// The two views of a finished import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResultTab {
    /// The result card.
    #[default]
    Result,
    /// The loss report.
    Report,
}

/// What the last Save report… or Open report did.
#[derive(Debug, Clone, PartialEq)]
pub enum ReportNote {
    Saved(PathBuf),
    Failed(Translatable),
}

/// Where a finished import was mounted (#3123).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mounted {
    /// Recorded as a global library.
    AllProjects,
    /// Added to this project's libraries, by its name.
    Project(String),
    /// Mounted, but the project it was meant for closed before the end, or
    /// the library has no readable id; it is on no project's list.
    Unrecorded,
    /// Not mounted: the open library at this path has the same library id.
    Clash(PathBuf),
}

/// A library the imported one's manifest `requires`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredLibrary {
    pub id: uuid::Uuid,
    /// The open library that has the id, by its name; `None` when none is
    /// open.
    pub open_as: Option<String>,
}

/// What a finished run did, for the result card.
#[derive(Debug, Clone, PartialEq)]
pub struct RunFacts {
    pub mode: RunMode,
    pub exit: ExitKind,
    pub counts: Counts,
    /// The import commit; `None` when the update changed nothing.
    pub commit: Option<String>,
    /// Commits that are not imports, found before an update.
    pub local_commits: usize,
    /// Settled once the library is recorded, after the run's own end.
    pub mounted: Option<Mounted>,
    pub required: Vec<RequiredLibrary>,
}

/// A finished import, or a library's stored report.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportResult {
    /// The library folder, which holds the report files.
    pub library_dir: PathBuf,
    /// The `.snxlib`, when the run wrote one (or the stored report's).
    pub library_file: Option<PathBuf>,
    pub report: Option<ImportReport>,
    /// Why the report could not be read, when it could not.
    pub report_problem: Option<String>,
    /// `None` for a stored report opened from the Library Browser.
    pub run: Option<RunFacts>,
}

impl ImportResult {
    /// The result of an import or update that wrote `outcome` into
    /// `output`.
    pub fn of_run(mode: RunMode, outcome: &ImportJobOutcome, output: &Path) -> Self {
        Self {
            library_dir: outcome
                .run
                .library
                .clone()
                .unwrap_or_else(|| output.to_path_buf()),
            library_file: outcome.library_file.clone(),
            report: outcome.run.report.clone(),
            report_problem: outcome.run.report_problem.clone(),
            run: Some(RunFacts {
                mode,
                exit: outcome.run.exit,
                counts: outcome.run.summary.counts,
                commit: outcome.commit.clone(),
                local_commits: outcome.local_commits,
                mounted: None,
                required: Vec::new(),
            }),
        }
    }

    /// The stored report of the library `library_file`, as read.
    pub fn stored(library_file: PathBuf, read: Result<ImportReport, String>) -> Self {
        let library_dir = library_file
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf);
        let (report, report_problem) = match read {
            Ok(report) => (Some(report), None),
            Err(why) => (None, Some(why)),
        };
        Self {
            library_dir,
            library_file: Some(library_file),
            report,
            report_problem,
            run: None,
        }
    }

    /// The readable copy Save report… and Open report use.
    pub fn report_md(&self) -> PathBuf {
        self.library_dir.join(REPORT_MD)
    }

    /// The library's name: its file's stem, else its folder's name.
    pub fn library_name(&self) -> String {
        self.library_file
            .as_deref()
            .and_then(Path::file_stem)
            .or_else(|| self.library_dir.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Items at `severity` in the report.
    pub fn count(&self, severity: Severity) -> usize {
        self.report.as_ref().map_or(0, |r| r.count(severity))
    }
}

/// The items of one code under a severity, with their index in the report.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeGroup<'a> {
    pub code: &'a str,
    pub items: Vec<(usize, &'a ReportItem)>,
}

/// One severity of the loss report and its codes, most items first.
#[derive(Debug, Clone, PartialEq)]
pub struct SeverityGroup<'a> {
    pub severity: Severity,
    /// Items of this severity the filter shows.
    pub count: usize,
    pub codes: Vec<CodeGroup<'a>>,
}

impl ImportDialogState {
    /// The report the dialog shows: the finished run's (or the stored
    /// one) at the end of step 3, else the dry run's on step 2.
    pub fn shown_report(&self) -> Option<&ImportReport> {
        match &self.result {
            Some(result) => result.report.as_ref(),
            None => self.preview_outcome().and_then(|o| o.report.as_ref()),
        }
    }

    /// The finished import or the stored report, once step 3 has one.
    pub fn finished(&self) -> Option<&ImportResult> {
        self.result.as_deref()
    }

    /// A stored report opened from the Library Browser, not a run's end.
    pub fn is_stored_report(&self) -> bool {
        self.finished().is_some_and(|r| r.run.is_none())
    }

    /// Show `result` at the end of step 3, on its result card (a stored
    /// report on the report itself), with a fresh filter.
    pub fn show_result(&mut self, result: ImportResult) {
        self.result_tab = if result.run.is_some() {
            ResultTab::Result
        } else {
            ResultTab::Report
        };
        self.result = Some(Box::new(result));
        self.step = Step::Import;
        self.import_end = None;
        self.filter.clear();
        self.hidden.clear();
        self.selected_item = None;
        self.report_note = None;
    }

    /// The dialog opened on a library's stored report.
    pub fn for_stored_report(result: ImportResult) -> Self {
        let mut state = Self::new();
        state.show_result(result);
        state
    }

    /// The visible items grouped by severity (most severe first, then any
    /// severity a newer contract added) and, within one, by code (most
    /// items first, then by code).
    pub fn report_groups(&self) -> Vec<SeverityGroup<'_>> {
        let visible = self.visible_items();
        CHIP_SEVERITIES
            .iter()
            .copied()
            .chain(std::iter::once(Severity::Other))
            .filter_map(|severity| {
                let of_severity: Vec<(usize, &ReportItem)> = visible
                    .iter()
                    .copied()
                    .filter(|(_, item)| item.severity == severity)
                    .collect();
                (!of_severity.is_empty()).then(|| SeverityGroup {
                    severity,
                    count: of_severity.len(),
                    codes: by_code(of_severity),
                })
            })
            .collect()
    }
}

fn by_code<'a>(items: Vec<(usize, &'a ReportItem)>) -> Vec<CodeGroup<'a>> {
    let mut codes: Vec<CodeGroup<'a>> = Vec::new();
    for (index, item) in items {
        match codes.iter_mut().find(|g| g.code == item.code) {
            Some(group) => group.items.push((index, item)),
            None => codes.push(CodeGroup {
                code: item.code.as_str(),
                items: vec![(index, item)],
            }),
        }
    }
    codes.sort_by(|a, b| b.items.len().cmp(&a.items.len()).then(a.code.cmp(b.code)));
    codes
}
