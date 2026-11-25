//! Stale marks (mockup row 199): an attachment, or a reading, stores the
//! values of the parts it names when it was taken (`taken-with:`); when
//! one of them changes it is stale, with a note naming the change. Mark
//! still valid stores the current values in its place.

use std::collections::BTreeMap;

use crate::literal::calls_of;
use crate::measurement::Reading;
use crate::reference::quote;
use crate::scan::{FrontMatter, front_matter_end};
use crate::syntax::typst_source;

use super::Attachment;

/// The value of every part of the design, by designator, as the
/// schematic holds it now. Empty while the design is not read: then
/// nothing is stale, since nothing is known to have changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesignValues {
    values: BTreeMap<String, String>,
}

impl DesignValues {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the value of `designator`.
    pub fn insert(&mut self, designator: impl Into<String>, value: impl Into<String>) {
        self.values.insert(designator.into(), value.into());
    }

    /// The value of `designator`, if the design has that part.
    pub fn get(&self, designator: &str) -> Option<&str> {
        self.values.get(designator).map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl<K: Into<String>, V: Into<String>> FromIterator<(K, V)> for DesignValues {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut values = Self::new();
        for (designator, value) in iter {
            values.insert(designator, value);
        }
        values
    }
}

/// One part whose value is not what it was.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ValueChange {
    pub part: String,
    /// The value stored when it was taken.
    pub was: String,
    /// The value now; `None` when the part is gone from the design.
    pub now: Option<String>,
}

/// The parts of `taken_with` whose value in `values` differs, in the order
/// stored. None while `values` is empty. `4.7 µH`, `4.7uH` and `4.7 μH` are
/// the same value.
pub fn changes(taken_with: &[(String, String)], values: &DesignValues) -> Vec<ValueChange> {
    if values.is_empty() {
        return Vec::new();
    }
    taken_with
        .iter()
        .filter_map(|(part, was)| {
            let now = values.get(part);
            let same = now.is_some_and(|now| same_value(now, was));
            (!same).then(|| ValueChange {
                part: part.clone(),
                was: was.clone(),
                now: now.map(str::to_string),
            })
        })
        .collect()
}

/// What changed for `reading`: its own stored values, and those of the
/// attachment it was taken with, each part once.
pub fn reading_changes(
    reading: &Reading,
    attachments: &[Attachment],
    values: &DesignValues,
) -> Vec<ValueChange> {
    let mut found = changes(&reading.taken_with, values);
    let attachment = attachments
        .iter()
        .find(|a| !reading.attach.is_empty() && a.file == reading.attach);
    for change in attachment
        .map(|a| changes(&a.taken_with, values))
        .unwrap_or_default()
    {
        if !found.iter().any(|f| f.part == change.part) {
            found.push(change);
        }
    }
    found
}

/// Two part values compared as written by people: spaces ignored, `u` and
/// the Greek mu after a digit read as the micro sign.
fn same_value(a: &str, b: &str) -> bool {
    normalized(a) == normalized(b)
}

fn normalized(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut after_digit = false;
    for c in value.chars().filter(|c| !c.is_whitespace()) {
        let c = match c {
            'u' | '\u{3bc}' if after_digit => 'µ',
            other => other,
        };
        after_digit = c.is_ascii_digit() || c == '.';
        out.push(c);
    }
    out
}

/// Mark still valid: the file text `source` with the `taken-with:` of the
/// attachment or reading that starts on `line` set to the current values
/// of its parts (a part gone from the design is dropped). `None` when no
/// such call is there, it stores no values, or the design is not read.
pub fn mark_still_valid(source: &str, line: u32, values: &DesignValues) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    let text = typst_source(source);
    let call = [super::FUNCTION, crate::measurement::read::FUNCTION]
        .into_iter()
        .flat_map(|function| calls_of(&text, function))
        .find(|call| call.line == line)?;
    let span = call.arg_span("taken-with")?;
    let stored = crate::measurement::read::pairs(call.arg("taken-with"));
    let current: Vec<String> = stored
        .iter()
        .filter_map(|(part, _)| Some(format!("{}: {}", quote(part), quote(values.get(part)?))))
        .collect();
    let written = match current.len() {
        0 => "(:)".to_string(),
        _ => format!("({})", current.join(", ")),
    };
    let shift = |offset: usize| to_file_offset(source, offset);
    let (start, end) = (shift(span.start), shift(span.end));
    let mut out = String::with_capacity(source.len() + written.len());
    out.push_str(source.get(..start)?);
    out.push_str(&written);
    out.push_str(source.get(end..)?);
    Some(out)
}

/// A byte offset of the compiled text (`typst_source`, header lines
/// blanked) as an offset of the file text.
fn to_file_offset(source: &str, offset: usize) -> usize {
    match front_matter_end(source) {
        FrontMatter::Closed { body, .. } => {
            let blank = source[..body].matches('\n').count();
            offset - blank + body
        }
        FrontMatter::None | FrontMatter::Unclosed => offset,
    }
}
