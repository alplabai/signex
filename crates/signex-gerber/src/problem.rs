//! Read problems: what a reader could not read, with the file and line it
//! came from. A reader never skips input without one (docs/GERBER_VIEWER.md,
//! "Read problems").

use thiserror::Error;

use crate::source::SourceLine;

/// How bad a problem is. Ordered most severe first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// What went wrong. The `Display` text is the plain English form for logs
/// and the command line; the app words each kind through its catalog.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProblemKind {
    #[error("aperture macro {name} is not defined; {flashes} objects use it")]
    UnknownApertureMacro { name: String, flashes: usize },
    #[error("aperture macro {name} has parameters that build no shape")]
    BadMacroParameters { name: String },
    #[error("aperture D{d_code} is not defined; {uses} objects use it")]
    UndefinedAperture { d_code: u32, uses: usize },
    #[error("aperture D{d_code} has a definition that cannot be read")]
    BadApertureDefinition { d_code: u32 },
    #[error("the file gives no coordinate format")]
    MissingFormat,
    #[error("the file gives no units")]
    MissingUnits,
    #[error("a coordinate cannot be read")]
    BadCoordinate,
    #[error("{command} is not supported")]
    UnsupportedCommand { command: String },
    #[error("a region is not closed")]
    UnclosedRegion,
    #[error("the file has no end-of-file command")]
    MissingEndOfFile,
    #[error("tool T{tool} has no size; {holes} holes use it")]
    ToolWithoutSize { tool: u32, holes: usize },
    #[error("tool T{tool} is not defined; {holes} holes use it")]
    UndefinedTool { tool: u32, holes: usize },
    #[error("the drill header line {text} cannot be read")]
    BadExcellonHeader { text: String },
    #[error("the file has no X2 file function; its name is taken from the file name")]
    NoFileFunction,
    #[error("the file is not text")]
    NotText,
    #[error("the file is not a Gerber, drill or job file")]
    NotAFabricationFile,
    #[error("the job file cannot be read: {why}")]
    BadJobFile { why: String },
}

impl ProblemKind {
    /// The one severity rule: a lost X2 file function and a missing end
    /// command are warnings, a file that is not fabrication data is
    /// information, everything else is an error.
    pub fn severity(&self) -> Severity {
        match self {
            ProblemKind::NoFileFunction | ProblemKind::MissingEndOfFile => Severity::Warning,
            ProblemKind::NotAFabricationFile => Severity::Info,
            _ => Severity::Error,
        }
    }
}

/// One problem, where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadProblem {
    pub at: SourceLine,
    pub kind: ProblemKind,
    /// The offending source line, verbatim, trimmed.
    pub text: String,
}

impl ReadProblem {
    pub fn severity(&self) -> Severity {
        self.kind.severity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_named_kinds_are_not_errors() {
        assert_eq!(ProblemKind::NoFileFunction.severity(), Severity::Warning);
        assert_eq!(ProblemKind::MissingEndOfFile.severity(), Severity::Warning);
        assert_eq!(ProblemKind::NotAFabricationFile.severity(), Severity::Info);
        assert_eq!(ProblemKind::NotText.severity(), Severity::Error);
        assert_eq!(
            ProblemKind::ToolWithoutSize { tool: 2, holes: 2 }.severity(),
            Severity::Error
        );
        assert!(Severity::Error < Severity::Warning);
    }
}
