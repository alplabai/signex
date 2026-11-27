//! Reading one per-kind block, `#note(...)`, `#calculation(...)`,
//! `#measurement(...)`, `#issue(...)` or `#decision(...)` (mockup row 198):
//! its typed id, title, status and date, its record fields and its
//! references. The body is optional; a field the kind does not take, or a
//! value it cannot use, is reported and ignored, never a reason to skip the
//! block.

use crate::annotation::{Annotation, AnnotationKind, Status};
use crate::date::IsoDate;
use crate::id::BlockId;
use crate::lexer::Walk;
use crate::record::value::FieldValue;
use crate::record::{FieldProblem, Fields, Record, read_record};
use crate::reference::{Value, parse_args, quote};
use crate::scan::{
    ProblemKind, SNIPPET_MAX_CHARS, TITLE_MAX_CHARS, clip_chars, references_in, snippet_of,
    title_of,
};

/// The block `index` of `walk`, a call of `kind`'s function; its problems
/// go to `problems` as `(line, kind)`.
pub(crate) fn read_kind_block(
    source: &str,
    walk: &Walk,
    index: usize,
    kind: AnnotationKind,
    problems: &mut Vec<(u32, ProblemKind)>,
) -> Option<Annotation> {
    let call = &walk.calls[index];
    let line = call.line;
    let args = call
        .args
        .clone()
        .map(|range| parse_args(&source[range]))
        .unwrap_or_default();
    let mut head = Head::default();
    let mut fields = Fields::new();
    let mut positional_body = None;
    for arg in args {
        let raw = match &arg.value {
            Value::Text(text) => quote(text),
            Value::Other(raw) => raw.clone(),
        };
        let Some(name) = arg.name else {
            // `#note([text])`: the body passed in the parentheses.
            match FieldValue::read(&raw) {
                FieldValue::Content(text) if positional_body.is_none() => {
                    positional_body = Some(text);
                }
                _ => problems.push((line, ProblemKind::UnknownArgument { argument: raw })),
            }
            continue;
        };
        match head.slot(&name) {
            Some(slot) => match FieldValue::read(&raw) {
                FieldValue::None => {}
                value => match value.as_text() {
                    Some(text) => *slot = Some(text.trim().to_string()),
                    None => problems.push((line, ProblemKind::NotText { argument: name })),
                },
            },
            None => fields.push(name, &raw),
        }
    }
    let mut found = Vec::new();
    let record = read_record(kind, &fields, &mut found);
    problems.extend(
        found
            .into_iter()
            .map(|problem| (line, field_problem(problem))),
    );
    let body = match call.body_text.clone() {
        // The walk copies whole characters, so the range is on boundaries.
        Some(range) => walk.text.get(range).unwrap_or_default().to_string(),
        None => positional_body.unwrap_or_default(),
    };
    let mut distinct = Vec::new();
    let body_references = references_in(source, walk, index, problems);
    for reference in fields.references().into_iter().chain(body_references) {
        if !distinct.contains(&reference) {
            distinct.push(reference);
        }
    }
    let title = head
        .title
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| title_of(&body));
    let snippet = match snippet_of(&body) {
        text if text.is_empty() => fallback_snippet(&record),
        text => text,
    };
    Some(Annotation {
        kind,
        id: head.id.and_then(|id| {
            let parsed = BlockId::parse_for(kind, &id);
            if parsed.is_none() {
                problems.push((line, ProblemKind::BadId { kind, id }));
            }
            parsed
        }),
        title: clip_chars(&title, TITLE_MAX_CHARS, false),
        status: head.status.and_then(|status| {
            let parsed = Status::parse(kind, &status);
            if parsed.is_none() {
                problems.push((line, ProblemKind::UnknownStatus { kind, status }));
            }
            parsed
        }),
        date: head.date.and_then(|date| {
            let parsed = IsoDate::parse(&date);
            if parsed.is_none() {
                problems.push((line, ProblemKind::BadDate { date }));
            }
            parsed
        }),
        source: source[call.start..call.end].to_string(),
        snippet: clip_chars(&snippet, SNIPPET_MAX_CHARS, true),
        references: distinct,
        record,
        line,
        span: call.start..call.end,
    })
}

/// The arguments every kind shares.
#[derive(Default)]
struct Head {
    id: Option<String>,
    title: Option<String>,
    status: Option<String>,
    date: Option<String>,
}

impl Head {
    fn slot(&mut self, name: &str) -> Option<&mut Option<String>> {
        match name {
            "id" => Some(&mut self.id),
            "title" => Some(&mut self.title),
            "status" => Some(&mut self.status),
            "date" => Some(&mut self.date),
            _ => None,
        }
    }
}

fn field_problem(problem: FieldProblem) -> ProblemKind {
    match problem {
        FieldProblem::Unknown { argument } => ProblemKind::UnknownArgument { argument },
        FieldProblem::Bad { argument, value } => ProblemKind::BadField { argument, value },
    }
}

/// A block with no body reads as its record: a Decision as what it chose.
fn fallback_snippet(record: &Record) -> String {
    match record {
        Record::Decision(decision) => decision.chosen.clone().unwrap_or_default(),
        Record::Issue(issue) => issue.closes_when.clone().unwrap_or_default(),
        _ => String::new(),
    }
}
