//! The Gerber Viewer's read problems in the app-wide Messages panel (#2539,
//! #193e): one row per problem, with the file, the line, what was not read
//! and what is drawn instead, so nothing a reader passes over is silent.
//! The rows outlive the viewer tab. See `docs/GERBER_VIEWER.md`, "Read
//! problems in Messages".

use signex_gerber::{ProblemKind, ReadProblem, Severity};
use signex_i18n::{I18n, Localized, ids, t};

/// The log target of the rows: the Messages Source code reads
/// `SIGNEX-GERBER_VIEWER` (`diagnostics::diagnostic_code_from_target`).
pub(crate) const TARGET: &str = "signex::gerber_viewer";

/// Log one Messages row per problem, in the order given, at the level of
/// its severity.
pub fn report_read_problems(problems: &[&ReadProblem], i18n: &I18n) {
    for problem in problems {
        let level = level_of(problem.kind.severity());
        let text = problem_text(problem, i18n).into_string();
        log::log!(target: TARGET, level, "{text}");
    }
}

/// The log level of a severity: errors, warnings and information as they
/// are.
pub(crate) fn level_of(severity: Severity) -> log::Level {
    match severity {
        Severity::Error => log::Level::Error,
        Severity::Warning => log::Level::Warn,
        Severity::Info => log::Level::Info,
    }
}

/// The plain sentence of one problem, from the catalog.
pub(crate) fn problem_text(problem: &ReadProblem, i18n: &I18n) -> Localized {
    let file = problem.at.file.as_str();
    // Line numbers are text, so a long file reads "line 1200", not "1,200".
    let line = problem.at.line.to_string();
    let line = line.as_str();
    match &problem.kind {
        ProblemKind::UnknownApertureMacro { name, flashes } => t!(
            i18n,
            ids::GERBER_VIEWER_READ_UNKNOWN_APERTURE_MACRO,
            "file" => file,
            "line" => line,
            "name" => name.as_str(),
            "flashes" => *flashes
        ),
        ProblemKind::BadMacroParameters { name } => t!(
            i18n,
            ids::GERBER_VIEWER_READ_BAD_MACRO_PARAMETERS,
            "file" => file,
            "line" => line,
            "name" => name.as_str()
        ),
        ProblemKind::UndefinedAperture { d_code, uses } => t!(
            i18n,
            ids::GERBER_VIEWER_READ_UNDEFINED_APERTURE,
            "file" => file,
            "line" => line,
            "d_code" => d_code.to_string(),
            "uses" => *uses
        ),
        ProblemKind::BadApertureDefinition { d_code } => t!(
            i18n,
            ids::GERBER_VIEWER_READ_BAD_APERTURE_DEFINITION,
            "file" => file,
            "line" => line,
            "d_code" => d_code.to_string()
        ),
        ProblemKind::MissingFormat => {
            at_line(i18n, ids::GERBER_VIEWER_READ_MISSING_FORMAT, file, line)
        }
        ProblemKind::MissingUnits => {
            at_line(i18n, ids::GERBER_VIEWER_READ_MISSING_UNITS, file, line)
        }
        ProblemKind::BadCoordinate => {
            at_line(i18n, ids::GERBER_VIEWER_READ_BAD_COORDINATE, file, line)
        }
        ProblemKind::UnsupportedCommand { command } => t!(
            i18n,
            ids::GERBER_VIEWER_READ_UNSUPPORTED_COMMAND,
            "file" => file,
            "line" => line,
            "command" => command.as_str()
        ),
        ProblemKind::UnclosedRegion => {
            at_line(i18n, ids::GERBER_VIEWER_READ_UNCLOSED_REGION, file, line)
        }
        ProblemKind::MissingEndOfFile => at_line(
            i18n,
            ids::GERBER_VIEWER_READ_MISSING_END_OF_FILE,
            file,
            line,
        ),
        ProblemKind::ToolWithoutSize { tool, holes } => t!(
            i18n,
            ids::GERBER_VIEWER_READ_TOOL_WITHOUT_SIZE,
            "file" => file,
            "line" => line,
            "tool" => tool_number(*tool),
            "holes" => *holes
        ),
        ProblemKind::UndefinedTool { tool, holes } => t!(
            i18n,
            ids::GERBER_VIEWER_READ_UNDEFINED_TOOL,
            "file" => file,
            "line" => line,
            "tool" => tool_number(*tool),
            "holes" => *holes
        ),
        ProblemKind::BadExcellonHeader { text } => t!(
            i18n,
            ids::GERBER_VIEWER_READ_BAD_EXCELLON_HEADER,
            "file" => file,
            "line" => line,
            "text" => text.as_str()
        ),
        ProblemKind::NoFileFunction => {
            at_line(i18n, ids::GERBER_VIEWER_READ_NO_FILE_FUNCTION, file, line)
        }
        ProblemKind::NotText => at_line(i18n, ids::GERBER_VIEWER_READ_NOT_TEXT, file, line),
        ProblemKind::NotAFabricationFile => at_line(
            i18n,
            ids::GERBER_VIEWER_READ_NOT_A_FABRICATION_FILE,
            file,
            line,
        ),
        ProblemKind::BadJobFile { why } => t!(
            i18n,
            ids::GERBER_VIEWER_READ_BAD_JOB_FILE,
            "file" => file,
            "line" => line,
            "why" => why.as_str()
        ),
    }
}

/// A problem whose sentence needs only the file and the line.
fn at_line(i18n: &I18n, id: signex_i18n::MessageId, file: &str, line: &str) -> Localized {
    t!(i18n, id, "file" => file, "line" => line)
}

/// A drill tool number as drill files write it: `2` is `02`, as in `T02`.
fn tool_number(tool: u32) -> String {
    format!("{tool:02}")
}

#[cfg(test)]
#[path = "gerber_read_tests.rs"]
mod tests;
