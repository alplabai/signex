//! Generate a design report from what the project knows (mockup row 201,
//! #2945): requirements and how each was met, the architecture, key
//! decisions, a BOM summary, the ERC, DRC and requirement results,
//! simulation results, outputs produced, open items and the change log.
//!
//! The report is its own notebook file, `notebook/generated/design-report.snxnbk`,
//! that Regenerate replaces as a whole; the hand-written notebook is never
//! changed. The caller gathers [`ProjectFacts`] (reading files and running
//! checks off the UI thread); [`generate`] turns them into the file text and
//! the [`GenerateSummary`] the command answers with. Same facts and options,
//! same file: nothing in it depends on the clock. See
//! `docs/DESIGN_NOTEBOOK_GENERATE.md`.

mod blocks;
mod facts;
mod render;
mod requirements;
mod sections;
mod summary;
mod tables;

pub use blocks::{BlockFields, block_fields, cites};
pub use facts::{
    BomCost, BomFacts, ChangeEntry, Fact, OutputFile, ProjectFacts, SheetFact, SimulationRun,
};
pub use requirements::{RequirementResult, RequirementRow, requirement_rows, statement};
pub use sections::{ChecksSource, GenerateOptions, Section, Template, parse_sections};
pub use summary::{
    CheckCounts, Checks, FailOn, GenerateStatus, GenerateSummary, NotRead, RequirementCounts,
    SourceNote, WriteDecision, parse_fail_on, write_decision,
};

use crate::compile::{CompileOptions, DiagnosticSeverity, compile};
use crate::file::NotebookFile;
use crate::sources::NotebookSources;

/// The folder inside `notebook/` that holds generated notebooks.
pub const GENERATED_DIR: &str = "generated";
/// The file name of the generated design report.
pub const GENERATED_FILE: &str = "design-report.snxnbk";
/// The report as a notebook name, the form `--notebook` takes
/// (`signex notebook export-pdf --notebook generated/design-report`).
pub const GENERATED_NOTEBOOK: &str = "generated/design-report";
/// The command name the summary carries.
pub const GENERATE_COMMAND: &str = "notebook.generate";

/// The report's path inside the project folder:
/// `notebook/generated/design-report.snxnbk`.
pub fn generated_path() -> String {
    format!("{}/{GENERATED_DIR}/{GENERATED_FILE}", crate::NOTEBOOK_DIR)
}

/// The report's path inside `notebook/`, the form a compile names it by.
fn generated_source_path() -> String {
    format!("{GENERATED_DIR}/{GENERATED_FILE}")
}

/// A generated report: the whole file text and what the command answers.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedNotebook {
    /// The path inside the project folder ([`generated_path`]).
    pub path: String,
    /// The whole `.snxnbk` text, header included.
    pub text: String,
    /// What was written and where it came from. `written` is false until the
    /// caller writes the file and says so.
    pub summary: GenerateSummary,
}

/// The report `options` asks for, from `facts`. Pure: the page count comes
/// from compiling the text with the bundled package and fonts only.
pub fn generate(facts: &ProjectFacts, options: &GenerateOptions) -> GeneratedNotebook {
    let rows = requirement_rows(
        &facts.requirements,
        facts.requirement_check.value(),
        facts.block_list(),
    );
    let body = render::report_body(facts, options, &rows);
    // A new file of the current format writes its token and nothing else.
    let text = NotebookFile::new(body)
        .write()
        .unwrap_or_else(|_| String::new());
    let compiled = compile(
        &NotebookSources::new(generated_source_path(), text.clone()),
        &CompileOptions::default(),
    );
    let problems: Vec<String> = compiled
        .diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .map(|d| format!("line {}: {}", d.line, d.message))
        .collect();
    let pages = compiled
        .notebook
        .as_ref()
        .map_or(0, |notebook| notebook.page_count());
    let summary = GenerateSummary::of(facts, options, &rows, pages, problems);
    GeneratedNotebook {
        path: generated_path(),
        text,
        summary,
    }
}
