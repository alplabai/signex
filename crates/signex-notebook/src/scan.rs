//! Finding the annotation blocks of a notebook and what is wrong with them.
//! A block is one of the per-kind functions (`#note`, `#calculation`,
//! `#measurement`, `#issue`, `#decision`; read in `crate::block`) or the
//! earlier `#annotation(kind: ...)`, read here.

use crate::annotation::{Annotation, AnnotationKind, Status};
use crate::date::IsoDate;
use crate::lexer::{self, Call, Walk};
use crate::record::Record;
use crate::reference::{Reference, Value, parse_args};

/// The function name of the earlier one-function annotation block.
const BLOCK: &str = "annotation";

/// Whether a call named `name` is an annotation block.
fn is_block_name(name: &str) -> bool {
    name == BLOCK || AnnotationKind::from_function(name).is_some()
}
/// The line that opens and closes the TOML front matter.
const FRONT_MATTER_FENCE: &str = "+++";
/// The longest title, in characters.
pub const TITLE_MAX_CHARS: usize = 60;
/// The longest snippet, in characters, before it is cut with an ellipsis.
pub const SNIPPET_MAX_CHARS: usize = 200;

/// Something in a notebook that kept a block, or part of one, from being
/// read. Nothing is passed over without one. In a JSON result it is
/// `{"line": 3, "text": ..., "problem": "bad_id", ...}`, the problem's own
/// fields beside its name.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ScanProblem {
    /// The 1-based line it is on.
    pub line: u32,
    /// That line as written, trimmed.
    pub text: String,
    #[serde(flatten)]
    pub kind: ProblemKind,
}

/// What went wrong, and what the scan did about it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "problem", rename_all = "snake_case")]
pub enum ProblemKind {
    /// A `+++` front matter with no closing `+++`: the whole file is read as
    /// Typst.
    UnclosedFrontMatter,
    /// A block with no `kind:`; it is skipped.
    MissingKind,
    /// A `kind:` that is not one of the five; the block is skipped.
    UnknownKind { kind: String },
    /// A `status:` the block's kind does not have; the block is kept with
    /// no status.
    UnknownStatus {
        kind: AnnotationKind,
        status: String,
    },
    /// A `date:` that is not a `YYYY-MM-DD` date; the block is kept undated.
    BadDate { date: String },
    /// An argument the block does not take; it is ignored.
    UnknownArgument { argument: String },
    /// An argument that is not a quoted string; it is ignored (a `kind:`
    /// that is not a string skips the block).
    NotText { argument: String },
    /// `#annotation(...)` with no `[ ]` body right after it; it is skipped.
    MissingBody,
    /// A block whose brackets, string, raw text, equation or comment is never
    /// closed; it is skipped, and so is the rest of the file.
    UnbalancedBlock,
    /// Outside any block: something opened with `opener` is never closed, so
    /// the rest of the file is not read.
    Unclosed { opener: char },
    /// A reference whose arguments are not the quoted names it takes; it is
    /// left out.
    BadReference { reference: String },
    /// An `id:` that is not a typed id of the block's kind (`DEC-002` on a
    /// `#decision`); the block is kept without an id.
    BadId { kind: AnnotationKind, id: String },
    /// An id a block before it in the file already has; the block is kept
    /// with it.
    DuplicateId { id: String },
    /// A record field whose value is not one it takes (a `severity:` that
    /// is not minor, major or critical, a `from:` that is not a block id);
    /// the field is ignored.
    BadField { argument: String, value: String },
}

/// The annotation blocks of `source`, a `.snxnbk` notebook (or plain Typst),
/// in source order, and every problem found, by line.
pub fn scan(source: &str) -> (Vec<Annotation>, Vec<ScanProblem>) {
    let mut problems = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    let problem = |line: u32, kind: ProblemKind| ScanProblem {
        line,
        text: lines
            .get(line.saturating_sub(1) as usize)
            .map_or(String::new(), |l| l.trim().to_string()),
        kind,
    };
    let (start, first_line) = match front_matter_end(source) {
        FrontMatter::None => (0, 1),
        FrontMatter::Closed { body, line, .. } => (body, line),
        FrontMatter::Unclosed => {
            problems.push(problem(1, ProblemKind::UnclosedFrontMatter));
            (0, 1)
        }
    };
    let walk = lexer::walk(source, start, first_line);
    let skipped = unbalanced_block(&walk, source);
    let mut annotations = Vec::new();
    for (index, call) in walk.calls.iter().enumerate() {
        let name = &source[call.name.clone()];
        if !is_block_name(name) || !call.complete || inside_block(&walk, source, call) {
            continue;
        }
        let mut found = Vec::new();
        let read = match AnnotationKind::from_function(name) {
            Some(kind) => crate::block::read_kind_block(source, &walk, index, kind, &mut found),
            None => read_block(source, &walk, index, &mut found),
        };
        if let Some(id) = read.as_ref().and_then(|a| a.id.as_ref())
            && annotations
                .iter()
                .any(|a: &Annotation| a.id.as_ref() == Some(id))
        {
            let id = id.to_string();
            found.push((call.line, ProblemKind::DuplicateId { id }));
        }
        problems.extend(found.into_iter().map(|(line, kind)| problem(line, kind)));
        annotations.extend(read);
    }
    match (skipped, walk.unclosed) {
        (Some(index), _) => problems.push(problem(
            walk.calls[index].line,
            ProblemKind::UnbalancedBlock,
        )),
        (None, Some(unclosed)) => problems.push(problem(
            unclosed.line,
            ProblemKind::Unclosed {
                opener: unclosed.opener,
            },
        )),
        (None, None) => {}
    }
    problems.sort_by_key(|p| p.line);
    (annotations, problems)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FrontMatter {
    None,
    /// The TOML is the bytes `toml`, between the fence lines; the Typst
    /// starts at byte `body`, on line `line`.
    Closed {
        toml: std::ops::Range<usize>,
        body: usize,
        line: u32,
    },
    Unclosed,
}

/// Where the TOML front matter between two leading `+++` lines ends. Its
/// keys hold the reference ids and belong to the notebook tab; the file
/// model (`crate::file`) reads them through this same split.
pub(crate) fn front_matter_end(source: &str) -> FrontMatter {
    let text = source.strip_prefix('\u{feff}').unwrap_or(source);
    let offset = source.len() - text.len();
    let mut lines = text.split_inclusive('\n');
    if lines.next().map(str::trim) != Some(FRONT_MATTER_FENCE) {
        return FrontMatter::None;
    }
    let mut at = offset + text.split_inclusive('\n').next().map_or(0, str::len);
    let toml_start = at;
    for (number, line) in lines.enumerate() {
        let fence_start = at;
        at += line.len();
        if line.trim() == FRONT_MATTER_FENCE {
            // This fence is line `number + 2`; the Typst starts on the next.
            let line_number = u32::try_from(number + 3).unwrap_or(u32::MAX);
            return FrontMatter::Closed {
                toml: toml_start..fence_start,
                body: at,
                line: line_number,
            };
        }
    }
    FrontMatter::Unclosed
}

/// The outermost annotation block still open when the source ended, if one
/// was.
fn unbalanced_block(walk: &Walk, source: &str) -> Option<usize> {
    walk.unclosed?;
    walk.open_calls
        .iter()
        .copied()
        .find(|&index| is_block_name(&source[walk.calls[index].name.clone()]))
}

/// Whether `call` sits inside another annotation block (a block in a block
/// belongs to the outer one's text).
fn inside_block(walk: &Walk, source: &str, call: &Call) -> bool {
    let mut parent = call.parent;
    while let Some(index) = parent {
        let outer = &walk.calls[index];
        if is_block_name(&source[outer.name.clone()]) {
            return true;
        }
        parent = outer.parent;
    }
    false
}

/// The annotation of the block `index`, or `None` when it is skipped; its
/// problems go to `problems` as `(line, kind)`.
fn read_block(
    source: &str,
    walk: &Walk,
    index: usize,
    problems: &mut Vec<(u32, ProblemKind)>,
) -> Option<Annotation> {
    let call = &walk.calls[index];
    let line = call.line;
    let args = call
        .args
        .clone()
        .map(|range| parse_args(&source[range]))
        .unwrap_or_default();
    let mut kind_text = None;
    let (mut status_text, mut title, mut date_text) = (None, None, None);
    let mut kind_not_text = false;
    for arg in args {
        let name = arg.name.unwrap_or_default();
        let slot = match name.as_str() {
            "kind" => &mut kind_text,
            "status" => &mut status_text,
            "title" => &mut title,
            "date" => &mut date_text,
            _ => {
                let argument = match arg.value {
                    _ if !name.is_empty() => name.clone(),
                    Value::Text(text) | Value::Other(text) => text,
                };
                problems.push((line, ProblemKind::UnknownArgument { argument }));
                continue;
            }
        };
        match arg.value {
            Value::Text(text) => *slot = Some(text),
            Value::Other(_) => {
                kind_not_text |= name == "kind";
                problems.push((line, ProblemKind::NotText { argument: name }));
            }
        }
    }
    let Some(kind_text) = kind_text else {
        // A `kind:` that is not a string was reported as such already.
        if !kind_not_text {
            problems.push((line, ProblemKind::MissingKind));
        }
        return None;
    };
    let Some(kind) = AnnotationKind::parse(kind_text.trim()) else {
        let kind = kind_text;
        problems.push((line, ProblemKind::UnknownKind { kind }));
        return None;
    };
    let (Some(_), Some(text_range)) = (call.body.clone(), call.body_text.clone()) else {
        problems.push((line, ProblemKind::MissingBody));
        return None;
    };
    let status = status_text.and_then(|status| {
        let parsed = Status::parse(kind, status.trim());
        if parsed.is_none() {
            problems.push((line, ProblemKind::UnknownStatus { kind, status }));
        }
        parsed
    });
    let date = date_text.and_then(|date| {
        let parsed = IsoDate::parse(date.trim());
        if parsed.is_none() {
            problems.push((line, ProblemKind::BadDate { date }));
        }
        parsed
    });
    // The walk copies whole characters, so the range is on boundaries.
    let body_text = walk.text.get(text_range).unwrap_or_default();
    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| title_of(body_text));
    Some(Annotation {
        kind,
        id: None,
        title: clip_chars(&title, TITLE_MAX_CHARS, false),
        status,
        date,
        source: source[call.start..call.end].to_string(),
        snippet: clip_chars(&snippet_of(body_text), SNIPPET_MAX_CHARS, true),
        references: references_in(source, walk, index, problems),
        record: Record::empty(kind),
        line,
        span: call.start..call.end,
    })
}

/// The references made inside block `index`, first mention first, each
/// once.
pub(crate) fn references_in(
    source: &str,
    walk: &Walk,
    index: usize,
    problems: &mut Vec<(u32, ProblemKind)>,
) -> Vec<Reference> {
    let mut found: Vec<Reference> = Vec::new();
    for call in walk.calls.iter().skip(index + 1) {
        if !descends_from(walk, call, index) {
            continue;
        }
        let name = &source[call.name.clone()];
        let args = call.args.clone().map_or("", |range| &source[range]);
        match Reference::from_call(name, args) {
            Some(Ok(reference)) if !found.contains(&reference) => found.push(reference),
            Some(Err(())) => problems.push((
                call.line,
                ProblemKind::BadReference {
                    reference: source[call.start..call.end].to_string(),
                },
            )),
            _ => {}
        }
    }
    found
}

fn descends_from(walk: &Walk, call: &Call, ancestor: usize) -> bool {
    let mut parent = call.parent;
    while let Some(index) = parent {
        if index == ancestor {
            return true;
        }
        parent = walk.calls[index].parent;
    }
    false
}

/// A body's lines as a reader sees them: trimmed, heading and list markers
/// dropped, blank lines gone.
fn text_lines(body: &str) -> impl Iterator<Item = String> + '_ {
    body.lines()
        .map(|line| {
            let line = line.trim();
            let line = line.trim_start_matches('=').trim_start();
            let line = ["- ", "+ "]
                .iter()
                .find_map(|marker| line.strip_prefix(marker))
                .unwrap_or(line);
            line.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .filter(|line| !line.is_empty())
}

/// The title a block without one gets: its first text line, cut at the
/// first `:`.
pub(crate) fn title_of(body: &str) -> String {
    let first = text_lines(body).next().unwrap_or_default();
    first
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub(crate) fn snippet_of(body: &str) -> String {
    text_lines(body).collect::<Vec<_>>().join(" ")
}

/// `text` cut to `max` characters, with an ellipsis when `ellipsis`.
pub(crate) fn clip_chars(text: &str, max: usize, ellipsis: bool) -> String {
    match text.char_indices().nth(max) {
        Some((end, _)) if ellipsis => format!("{}…", text[..end].trim_end()),
        Some((end, _)) => text[..end].trim_end().to_string(),
        None => text.to_string(),
    }
}
