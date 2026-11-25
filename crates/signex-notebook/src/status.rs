//! What the notebook tab's status bar, Problems strip and `notebook_compile`
//! answer say about one compile (mockup row 195): the pages, the problems
//! by file, line and column, the outline, the references and the block
//! counts. Built from the sources and the compile, so the tab and the CLI
//! report the same thing.

use crate::compile::{Compilation, Diagnostic, DiagnosticSeverity, PageInfo};
use crate::outline::{Outline, outline};
use crate::reference::Reference;
use crate::scan::{FrontMatter, front_matter_end};
use crate::sources::NotebookSources;
use crate::syntax::distinct_references;

/// How many blocks of each kind the notebook has.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct BlockCounts {
    pub notes: usize,
    pub calculations: usize,
    pub measurements: usize,
    pub issues: usize,
    pub decisions: usize,
}

/// One compile of a notebook, as the status bar and the CLI report it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct NotebookStatus {
    /// True when the notebook compiled.
    pub compiled: bool,
    pub pages: Vec<PageInfo>,
    /// Errors first, then warnings, each in file and line order.
    pub problems: Vec<Diagnostic>,
    pub errors: usize,
    pub warnings: usize,
    pub outline: Outline,
    /// The distinct parts, pins and nets the notebook names, first mention
    /// first, across its chapters.
    pub references: Vec<Reference>,
    pub blocks: BlockCounts,
}

impl NotebookStatus {
    /// The status of `compilation`, a compile of `sources`.
    pub fn of(sources: &NotebookSources, compilation: &Compilation) -> Self {
        let outline = outline(sources);
        let mut blocks = BlockCounts::default();
        for block in &outline.blocks {
            let count = match block.kind {
                "note" => &mut blocks.notes,
                "calculation" => &mut blocks.calculations,
                "measurement" => &mut blocks.measurements,
                "issue" => &mut blocks.issues,
                _ => &mut blocks.decisions,
            };
            *count += 1;
        }
        let mut references: Vec<Reference> = Vec::new();
        for path in sources.paths() {
            let text = String::from_utf8_lossy(sources.get(path).unwrap_or_default());
            for reference in distinct_references(&text) {
                if !references.contains(&reference) {
                    references.push(reference);
                }
            }
        }
        let count = |severity| {
            compilation
                .diagnostics
                .iter()
                .filter(|d| d.severity == severity)
                .count()
        };
        Self {
            compiled: compilation.notebook.is_some(),
            pages: compilation
                .notebook
                .as_ref()
                .map(|notebook| notebook.pages())
                .unwrap_or_default(),
            problems: compilation.diagnostics.clone(),
            errors: count(DiagnosticSeverity::Error),
            warnings: count(DiagnosticSeverity::Warning),
            outline,
            references,
            blocks,
        }
    }

    /// The first problem of `file` below line `line`, wrapping to its first
    /// problem; `None` when the file has none. F8 walks the Problems strip
    /// of the open file with it.
    pub fn next_problem(&self, file: &str, line: u32) -> Option<&Diagnostic> {
        let mut located: Vec<&Diagnostic> = self
            .problems
            .iter()
            .filter(|d| d.file == file && d.line > 0)
            .collect();
        located.sort_by_key(|d| (d.line, d.column));
        located
            .iter()
            .find(|d| d.line > line)
            .or_else(|| located.first())
            .copied()
    }
}

/// How many lines the `+++` header of the file text `text` takes, both
/// fences included; 0 when it has none. The editor shows the body and
/// numbers its lines from one past these.
pub fn header_line_count(text: &str) -> u32 {
    match front_matter_end(text) {
        FrontMatter::Closed { line, .. } => line.saturating_sub(1),
        FrontMatter::None | FrontMatter::Unclosed => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompileOptions, compile};

    const NOTEBOOK: &str = "+++\nformat = \"snxnbk/1\"\n+++\n\
        #import \"@signex/notebook:0.1.0\": *\n\
        = Power stage\n\
        The divider #component(\"R7\"), #component(\"R8\") into #pin(\"U3\", \"12\").\n\
        #calculation(id: \"CALC-1.1\", title: [Output voltage])[x]\n\
        #measurement(id: \"MEAS-001\", title: [Ripple])[x]\n\
        #issue(id: \"ISS-001\", title: [Droop])[x]\n\
        Again #component(\"R7\").\n";

    fn status(text: &str) -> NotebookStatus {
        let sources = NotebookSources::new("design.snxnbk", text);
        NotebookStatus::of(&sources, &compile(&sources, &CompileOptions::default()))
    }

    #[test]
    fn a_notebook_that_compiles_reports_its_pages_outline_references_and_blocks() {
        let status = status(NOTEBOOK);
        assert!(status.compiled, "{:?}", status.problems);
        assert_eq!((status.errors, status.warnings), (0, 0));
        assert!(!status.pages.is_empty());
        assert_eq!(status.outline.sections[0].title, "Power stage");
        let names: Vec<String> = status.references.iter().map(Reference::name).collect();
        assert_eq!(names, ["R7", "R8", "U3.12"]);
        assert_eq!(
            status.blocks,
            BlockCounts {
                notes: 0,
                calculations: 1,
                measurements: 1,
                issues: 1,
                decisions: 0,
            }
        );
    }

    #[test]
    fn a_compile_error_is_reported_where_it_is_and_no_pages() {
        let broken = NOTEBOOK.replace("[x]\n#measurement", "[#(1 + 2]\n#measurement");
        let status = status(&broken);
        assert!(!status.compiled);
        assert!(status.pages.is_empty());
        assert!(status.errors >= 1, "{:?}", status.problems);
        let first = &status.problems[0];
        assert_eq!(first.file, "design.snxnbk");
        assert_eq!(first.line, 7, "the line in the file, header counted");
    }

    #[test]
    fn the_next_problem_of_a_file_follows_the_line_and_wraps() {
        let problem = |file: &str, line| Diagnostic {
            severity: DiagnosticSeverity::Error,
            file: file.into(),
            line,
            column: 1,
            message: String::new(),
            hints: Vec::new(),
        };
        let status = NotebookStatus {
            problems: vec![
                problem("design.snxnbk", 40),
                problem("chapters/a.snxnbk", 3),
                problem("design.snxnbk", 12),
                problem("", 0),
            ],
            ..status("= A\n")
        };
        let next = |file, line| {
            status
                .next_problem(file, line)
                .map(|d| (d.file.clone(), d.line))
        };
        assert_eq!(next("design.snxnbk", 1), Some(("design.snxnbk".into(), 12)));
        assert_eq!(
            next("design.snxnbk", 12),
            Some(("design.snxnbk".into(), 40))
        );
        assert_eq!(
            next("design.snxnbk", 40),
            Some(("design.snxnbk".into(), 12))
        );
        assert_eq!(
            next("chapters/a.snxnbk", 9),
            Some(("chapters/a.snxnbk".into(), 3))
        );
        assert_eq!(next("other.snxnbk", 1), None);
        let none = NotebookStatus {
            problems: Vec::new(),
            ..status("= A\n")
        };
        assert_eq!(none.next_problem("design.snxnbk", 1), None);
    }

    #[test]
    fn the_header_takes_its_lines_with_both_fences() {
        assert_eq!(header_line_count(NOTEBOOK), 3);
        assert_eq!(header_line_count("= No header\n"), 0);
        assert_eq!(header_line_count("+++\nformat = \"snxnbk/1\"\n"), 0);
    }
}
