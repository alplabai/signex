//! The Python cells of a notebook file: `#python(label: "...")[```python
//! ... ```]` (mockup row 200), read with the same walk as the annotation
//! blocks.

use std::collections::BTreeSet;

use crate::lexer;
use crate::reference::{Value, parse_args};
use crate::scan::{FrontMatter, front_matter_end};

/// The Typst function of a Python cell.
pub const CELL_FUNCTION: &str = "python";
/// The language a cell's raw block names.
pub const CELL_LANGUAGE: &str = "python";

/// One `#python` cell.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PythonCell {
    /// The file it is in, as a path inside the notebook folder.
    pub file: String,
    /// Its `label:`, which names it to the cache, the page and the
    /// commands.
    pub label: String,
    /// The 1-based line the `#python` is on.
    pub line: u32,
    /// The 1-based file line of the first line of the Python source.
    pub source_line: u32,
    /// The Python source, as written between the fences.
    pub source: String,
}

impl PythonCell {
    /// The file line of line `cell_line` (1-based) of the cell's source.
    pub fn file_line(&self, cell_line: u32) -> u32 {
        self.source_line
            .saturating_add(cell_line.max(1))
            .saturating_sub(1)
    }
}

/// A `#python` call that is not a cell the runner can take, or an argument
/// of one that is ignored.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CellProblem {
    pub file: String,
    pub line: u32,
    pub kind: CellProblemKind,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CellProblemKind {
    /// No `label:`; the cell is not run and its page says so.
    NoLabel,
    /// A `label:` that is not a quoted string; the cell is not run.
    LabelNotText,
    /// A second cell with a label already used; only the first runs.
    DuplicateLabel { label: String },
    /// No ```` ```python ```` raw block in the body; the cell is not run.
    NoSource,
    /// The raw block names another language; the cell is not run.
    NotPython { language: String },
    /// An argument a cell does not take; it is ignored.
    UnknownArgument { argument: String },
}

/// The cells of the file text `source` at `file`, in source order, and
/// what kept a call from being a cell. Labels are unique within the list.
pub fn cells_in(file: &str, source: &str) -> (Vec<PythonCell>, Vec<CellProblem>) {
    let mut seen = BTreeSet::new();
    read_cells(file, source, &mut seen)
}

/// The cells of every notebook file of `sources`, by path, and their
/// problems. A label is unique across the files: the first, by path, runs.
pub fn notebook_cells(sources: &crate::NotebookSources) -> (Vec<PythonCell>, Vec<CellProblem>) {
    let mut seen = BTreeSet::new();
    let (mut cells, mut problems) = (Vec::new(), Vec::new());
    let is_notebook = |path: &&str| path.ends_with(&format!(".{}", crate::NOTEBOOK_EXTENSION));
    for path in sources.paths().filter(is_notebook) {
        let text = String::from_utf8_lossy(sources.get(path).unwrap_or_default());
        let (found, wrong) = read_cells(path, &text, &mut seen);
        cells.extend(found);
        problems.extend(wrong);
    }
    (cells, problems)
}

fn read_cells(
    file: &str,
    source: &str,
    seen: &mut BTreeSet<String>,
) -> (Vec<PythonCell>, Vec<CellProblem>) {
    let (start, first_line) = match front_matter_end(source) {
        FrontMatter::Closed { body, line, .. } => (body, line),
        FrontMatter::None | FrontMatter::Unclosed => (0, 1),
    };
    let walk = lexer::walk(source, start, first_line);
    let (mut cells, mut problems) = (Vec::new(), Vec::new());
    let mut problem = |line: u32, kind: CellProblemKind| {
        problems.push(CellProblem {
            file: file.to_string(),
            line,
            kind,
        });
    };
    for call in &walk.calls {
        if source[call.name.clone()] != *CELL_FUNCTION || !call.complete {
            continue;
        }
        let args = call
            .args
            .clone()
            .map(|range| parse_args(&source[range]))
            .unwrap_or_default();
        let mut label = None;
        let mut label_not_text = false;
        for arg in args {
            match (arg.name.as_deref(), arg.value) {
                (Some("label"), Value::Text(text)) => label = Some(text.trim().to_string()),
                (Some("label"), Value::Other(_)) => label_not_text = true,
                (Some(name), _) => problem(
                    call.line,
                    CellProblemKind::UnknownArgument {
                        argument: name.to_string(),
                    },
                ),
                (None, Value::Text(argument) | Value::Other(argument)) => {
                    problem(call.line, CellProblemKind::UnknownArgument { argument });
                }
            }
        }
        let code = match call.body.clone() {
            Some(body) => raw_python(body.start, &source[body]),
            None => Err(CellProblemKind::NoSource),
        };
        let (offset, code) = match code {
            Ok(found) => found,
            Err(kind) => {
                problem(call.line, kind);
                continue;
            }
        };
        let label = match label.filter(|label| !label.is_empty()) {
            Some(label) => label,
            None if label_not_text => {
                problem(call.line, CellProblemKind::LabelNotText);
                continue;
            }
            None => {
                problem(call.line, CellProblemKind::NoLabel);
                continue;
            }
        };
        if !seen.insert(label.clone()) {
            problem(call.line, CellProblemKind::DuplicateLabel { label });
            continue;
        }
        cells.push(PythonCell {
            file: file.to_string(),
            label,
            line: call.line,
            source_line: line_of(source, offset),
            source: code,
        });
    }
    (cells, problems)
}

/// The Python of a cell body `body`, which starts at byte `at` of the file:
/// the content of the raw block of three or more backticks that opens it,
/// whose language must be `python`, with the indent its lines share taken
/// off, and the byte that content starts at.
fn raw_python(at: usize, body: &str) -> Result<(usize, String), CellProblemKind> {
    let leading = body.len() - body.trim_start().len();
    let rest = &body[leading..];
    let fence = rest.bytes().take_while(|&b| b == b'`').count();
    if fence < 3 {
        return Err(CellProblemKind::NoSource);
    }
    let after_fence = &rest[fence..];
    let language_end = after_fence
        .find(|c: char| c.is_whitespace() || c == '`')
        .unwrap_or(after_fence.len());
    let language = &after_fence[..language_end];
    if language != CELL_LANGUAGE {
        return Err(CellProblemKind::NotPython {
            language: language.to_string(),
        });
    }
    let after_language = &after_fence[language_end..];
    let inline = after_language.trim_start_matches([' ', '\t']);
    let skipped = after_language.len() - inline.len();
    let newline = if inline.starts_with("\r\n") {
        2
    } else {
        usize::from(inline.starts_with('\n'))
    };
    let code_start = language_end + skipped + newline;
    let code = &after_fence[code_start..];
    let closing = "`".repeat(fence);
    let Some(end) = code.find(&closing) else {
        return Err(CellProblemKind::NoSource);
    };
    // The closing fence's own indent is not code.
    let code = code[..end].trim_end_matches([' ', '\t']);
    let code = code
        .strip_suffix('\n')
        .map_or(code, |c| c.strip_suffix('\r').unwrap_or(c));
    Ok((at + leading + fence + code_start, dedent(code)))
}

/// `code` with the leading whitespace all its non-blank lines share taken
/// off each line.
fn dedent(code: &str) -> String {
    let indent = code
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    code.lines()
        .map(|line| line.get(indent..).unwrap_or_else(|| line.trim_start()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The 1-based line byte `offset` of `text` is on.
fn line_of(text: &str, offset: usize) -> u32 {
    let before = text.get(..offset).unwrap_or(text);
    u32::try_from(before.matches('\n').count() + 1).unwrap_or(u32::MAX)
}
