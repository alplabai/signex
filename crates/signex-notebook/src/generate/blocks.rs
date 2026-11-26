//! What the report reads from a notebook block beyond its kind, status and
//! text: its typed id and a measurement's value, from the block's own
//! arguments, and whether the block cites a requirement.

use crate::annotation::Annotation;
use crate::id::BlockId;
use crate::reference::{Value, parse_args};

/// The arguments of a block the report prints.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockFields {
    /// The `id:` argument, when it is a typed id of the block's kind.
    pub id: Option<BlockId>,
    /// The `value:` argument, as written.
    pub value: Option<String>,
    /// The `unit:` argument, as written.
    pub unit: Option<String>,
}

impl BlockFields {
    /// The value with its unit (`3.310 V`), when the block records one.
    pub fn reading(&self) -> Option<String> {
        let value = self.value.as_deref()?;
        Some(match self.unit.as_deref() {
            Some(unit) if !unit.is_empty() => format!("{value} {unit}"),
            _ => value.to_string(),
        })
    }
}

/// The id, value and unit `block` is written with. An `id:` of another
/// kind's prefix, or one that is not a typed id, is left out.
pub fn block_fields(block: &Annotation) -> BlockFields {
    let Some(args) = call_arguments(&block.source) else {
        return BlockFields::default();
    };
    let mut fields = BlockFields::default();
    for arg in parse_args(args) {
        let (Some(name), Value::Text(text)) = (arg.name.as_deref(), arg.value.clone()) else {
            continue;
        };
        match name {
            "id" => fields.id = BlockId::parse_for(block.kind, &text),
            "value" => fields.value = Some(text),
            "unit" => fields.unit = Some(text),
            _ => {}
        }
    }
    fields
}

/// The name a list prints `block` by: its id, else its title.
pub(super) fn label(block: &Annotation) -> String {
    block_fields(block)
        .id
        .map_or_else(|| block.title.clone(), |id| id.to_string())
}

/// Does `block` name `id` (a requirement id, `REQ-001`) in its title or text,
/// as a whole word? `REQ-1` is not cited by `REQ-10`.
pub fn cites(block: &Annotation, id: &str) -> bool {
    let id = id.trim();
    !id.is_empty() && (has_word(&block.title, id) || has_word(&block.snippet, id))
}

fn has_word(text: &str, word: &str) -> bool {
    let is_word_char = |c: char| c.is_alphanumeric() || c == '-' || c == '_';
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
    })
}

/// The text between the parentheses of the call `source` starts with
/// (`#measurement(id: "MEAS-009", ...)[...]`); `None` when the call has none.
fn call_arguments(source: &str) -> Option<&str> {
    let name_end = source
        .char_indices()
        .skip(1)
        .find(|&(_, c)| !(c.is_alphanumeric() || c == '_' || c == '-'))
        .map(|(i, _)| i)?;
    if !source[name_end..].starts_with('(') {
        return None;
    }
    let open = name_end;
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for (i, c) in source[open..].char_indices() {
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
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(&source[open + 1..open + i]);
                }
            }
            _ => {}
        }
    }
    None
}
