//! The schematic values a cell reads: `component("L1").value` and
//! `component("C15").param("ESR")`, found in its source before it runs, and
//! the values they have in the design now.

use serde::{Deserialize, Serialize};

use super::quantity::{Quantity, Unit};

/// The bridge call a cell reads a part with.
const CALL: &str = "component(";

/// What of a part a cell reads.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "field", content = "name", rename_all = "snake_case")]
pub enum Field {
    /// Its value, `.value`.
    Value,
    /// A part parameter, `.param("ESR")`.
    Param(String),
}

/// One value a cell reads, where its source reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValueRead {
    /// The part's designator.
    pub designator: String,
    pub field: Field,
    /// The Python name it is assigned to (`L` in `L = component("L1").value`),
    /// when the line is such an assignment.
    pub symbol: Option<String>,
    /// The 1-based line of the cell source it is first read on.
    pub cell_line: u32,
}

/// The design values a cell can read. The app answers from the open
/// project, a test from a table.
pub trait DesignValues {
    /// The value of part `designator` as written, if it has one.
    fn value(&self, designator: &str) -> Option<String>;
    /// Parameter `name` of part `designator` as written, if it has it.
    fn param(&self, designator: &str, name: &str) -> Option<String>;
}

/// One value a cell read, as it was in the design: the cache holds these,
/// and a cell is out of date when one of them changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputValue {
    pub designator: String,
    pub field: Field,
    /// The Python name it is assigned to, for the page's list of values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// The text as written in the design; `None` when the part or the
    /// parameter is not there.
    pub written: Option<String>,
    /// The value with its unit, when the text reads as one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<Quantity>,
}

impl InputValue {
    /// Where the value comes from, as the page names it: `L1 value`,
    /// `C15 ESR`.
    pub fn source_name(&self) -> String {
        match &self.field {
            Field::Value => format!("{} value", self.designator),
            Field::Param(name) => format!("{} {name}", self.designator),
        }
    }

    /// The value as the page prints it: the quantity in engineering form,
    /// else the text as written, else `missing`.
    pub fn display(&self) -> String {
        match (&self.quantity, &self.written) {
            (Some(quantity), _) => quantity.display(),
            (None, Some(written)) => written.clone(),
            (None, None) => "missing".to_string(),
        }
    }

    /// Whether `other` names the same part field.
    pub fn same_field(&self, other: &InputValue) -> bool {
        self.designator == other.designator && self.field == other.field
    }
}

/// The values `source`, a cell's Python, reads, first read first, each once.
/// A call whose arguments are not one quoted string is not listed: the cell
/// still runs, and the runtime records what it read.
pub fn value_reads(source: &str) -> Vec<ValueRead> {
    let mut reads: Vec<ValueRead> = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let code = strip_comment(line);
        let symbol = assigned_name(code);
        for (at, opener) in code.match_indices(CALL) {
            // `my_component(` or `x.component(` is another function.
            if code[..at].chars().next_back().is_some_and(is_name_char) {
                continue;
            }
            let call = &code[at + opener.len()..];
            let Some((designator, after)) = quoted(call) else {
                continue;
            };
            let Some(after) = after.trim_start().strip_prefix(')') else {
                continue;
            };
            let field = if after.starts_with(".value") {
                Field::Value
            } else if let Some(param) = after.strip_prefix(".param(")
                && let Some((name, tail)) = quoted(param)
                && tail.trim_start().starts_with(')')
            {
                Field::Param(name)
            } else {
                continue;
            };
            let known = reads
                .iter()
                .any(|read| read.designator == designator && read.field == field);
            if !known {
                reads.push(ValueRead {
                    designator,
                    field,
                    symbol: symbol.clone(),
                    cell_line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                });
            }
        }
    }
    reads
}

/// The design's current values of `reads`, each with its unit when the
/// text reads as a quantity (a value with no unit written takes the unit
/// its part kind implies).
pub fn current_inputs(reads: &[ValueRead], design: &dyn DesignValues) -> Vec<InputValue> {
    reads
        .iter()
        .map(|read| {
            let written = match &read.field {
                Field::Value => design.value(&read.designator),
                Field::Param(name) => design.param(&read.designator, name),
            };
            let implied = match read.field {
                Field::Value => Unit::implied_by(&read.designator),
                Field::Param(_) => Unit::One,
            };
            let quantity = written
                .as_deref()
                .and_then(|text| Quantity::parse(text, implied));
            InputValue {
                designator: read.designator.clone(),
                field: read.field.clone(),
                symbol: read.symbol.clone(),
                written,
                quantity,
            }
        })
        .collect()
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.'
}

/// `line` up to a `#` that is not inside a string.
fn strip_comment(line: &str) -> &str {
    let mut quote = None;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (None, '#') => return &line[..i],
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), _) if c == open => quote = None,
            _ => {}
        }
    }
    line
}

/// The name a line assigns to, `L` in `L = ...`; `None` for any other line.
fn assigned_name(code: &str) -> Option<String> {
    let (target, value) = code.split_once('=')?;
    if value.starts_with('=') {
        return None;
    }
    let target = target.trim();
    let mut chars = target.chars();
    let first = chars.next()?;
    let is_name = (first.is_alphabetic() || first == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
        && !code.starts_with(char::is_whitespace);
    is_name.then(|| target.to_string())
}

/// A string literal in single or double quotes opening `text` (spaces
/// before it skipped), unescaped only of `\"` and `\'`, and what follows.
fn quoted(text: &str) -> Option<(String, &str)> {
    let text = text.trim_start();
    let quote = text.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let body = &text[1..];
    let mut value = String::new();
    let mut escaped = false;
    for (i, c) in body.char_indices() {
        match c {
            _ if escaped => {
                value.push(c);
                escaped = false;
            }
            '\\' => escaped = true,
            _ if c == quote => {
                let name = value.trim().to_string();
                return (!name.is_empty()).then_some((name, &body[i + 1..]));
            }
            _ => value.push(c),
        }
    }
    None
}
