//! The value of one block argument as the record readers see it: a string,
//! content, a number with its unit, a reference, an array or a dictionary.
//! Read from the argument text the scan cut out, with the same lexer.

use crate::lexer;
use crate::reference::{Reference, parse_args, unquote};

/// One argument value of an annotation block.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FieldValue {
    /// A string literal, unescaped: `"j.berg"`.
    Text(String),
    /// A content block as plain text, references as their names:
    /// `[Candidate A, 4.7 µH]`.
    Content(String),
    /// A number and the unit written right after it: `310`, `5%`,
    /// `2.5mm`.
    Number { value: f64, unit: Option<String> },
    /// A `component`, `net` or `pin` call.
    Reference(Reference),
    /// `(a, b, ...)`.
    Array(Vec<FieldValue>),
    /// `(max: 250)`.
    Dict(Vec<(String, FieldValue)>),
    /// `none`.
    None,
    /// Anything else: an expression, a variable, a reference whose
    /// arguments are not the quoted names it takes.
    Other,
}

impl FieldValue {
    /// The value read from its argument text.
    pub(crate) fn read(text: &str) -> Self {
        let text = text.trim();
        if let Some(string) = unquote(text) {
            return FieldValue::Text(string);
        }
        if text == "none" {
            return FieldValue::None;
        }
        if let Some(inner) = enclosed(text, '[', ']') {
            return FieldValue::Content(plain_text(inner));
        }
        if let Some(inner) = enclosed(text, '(', ')') {
            return read_group(inner);
        }
        if let Some(reference) = read_reference(text) {
            return reference;
        }
        match read_number(text) {
            Some((value, unit)) => FieldValue::Number { value, unit },
            None => FieldValue::Other,
        }
    }

    /// The text of a string or of content.
    pub(crate) fn as_text(&self) -> Option<&str> {
        match self {
            FieldValue::Text(text) | FieldValue::Content(text) => Some(text),
            _ => None,
        }
    }

    /// The number, of any unit.
    pub(crate) fn as_number(&self) -> Option<(f64, Option<&str>)> {
        match self {
            FieldValue::Number { value, unit } => Some((*value, unit.as_deref())),
            _ => None,
        }
    }

    /// The items of an array, or the value itself as the one item.
    pub(crate) fn items(&self) -> Vec<&FieldValue> {
        match self {
            FieldValue::Array(items) => items.iter().collect(),
            FieldValue::None => Vec::new(),
            other => vec![other],
        }
    }

    /// Every reference in the value, arrays and dictionaries included.
    pub(crate) fn references(&self) -> Vec<Reference> {
        match self {
            FieldValue::Reference(reference) => vec![reference.clone()],
            FieldValue::Array(items) => items.iter().flat_map(FieldValue::references).collect(),
            FieldValue::Dict(entries) => entries.iter().flat_map(|(_, v)| v.references()).collect(),
            _ => Vec::new(),
        }
    }
}

/// The inside of `text` when it opens with `open` and its matching closer
/// is its last character.
fn enclosed(text: &str, open: char, close: char) -> Option<&str> {
    let inner = text.strip_prefix(open)?.strip_suffix(close)?;
    // `(a) + (b)` opens and closes with parentheses but is not one group.
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for c in inner.chars() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
            }
            _ => {}
        }
    }
    (depth == 0).then_some(inner)
}

/// `(...)`: a dictionary when every item is named, else an array.
fn read_group(inner: &str) -> FieldValue {
    let args = parse_args(inner);
    if inner.trim() == ":" {
        return FieldValue::Dict(Vec::new());
    }
    if !args.is_empty() && args.iter().all(|arg| arg.name.is_some()) {
        let entries = split_named(inner);
        return FieldValue::Dict(entries);
    }
    FieldValue::Array(
        split_items(inner)
            .iter()
            .map(|t| FieldValue::read(t))
            .collect(),
    )
}

/// The raw texts of a group's items.
fn split_items(inner: &str) -> Vec<String> {
    crate::reference::split_top_level(inner)
        .into_iter()
        .map(str::trim)
        .filter(|piece| !piece.is_empty())
        .map(str::to_string)
        .collect()
}

/// `key: value` items, each value read.
fn split_named(inner: &str) -> Vec<(String, FieldValue)> {
    split_items(inner)
        .into_iter()
        .filter_map(|piece| {
            let (key, value) = piece.split_once(':')?;
            let key = unquote(key.trim()).unwrap_or_else(|| key.trim().to_string());
            Some((key, FieldValue::read(value)))
        })
        .collect()
}

/// `component("R7")`, `net("SW")`, `pin("U3", "3")`: the whole text is one
/// reference call.
fn read_reference(text: &str) -> Option<FieldValue> {
    let open = text.find('(')?;
    let name = &text[..open];
    let args = enclosed(&text[open..], '(', ')')?;
    match Reference::from_call(name, args)? {
        Ok(reference) => Some(FieldValue::Reference(reference)),
        Err(()) => Some(FieldValue::Other),
    }
}

/// A number with an optional sign and the unit letters (or `%`) after it.
fn read_number(text: &str) -> Option<(f64, Option<String>)> {
    let end = text
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || (i == 0 && (c == '-' || c == '+'))))
        .map_or(text.len(), |(i, _)| i);
    let value: f64 = text[..end].parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    let unit = text[end..].trim();
    if unit.is_empty() {
        return Some((value, None));
    }
    let is_unit = unit.chars().all(|c| c.is_alphabetic() || c == '%');
    is_unit.then(|| (value, Some(unit.to_string())))
}

/// Markup as a reader sees it: one line, references as their names.
pub(crate) fn plain_text(markup: &str) -> String {
    let walk = lexer::walk(markup, 0, 1);
    walk.text.split_whitespace().collect::<Vec<_>>().join(" ")
}
