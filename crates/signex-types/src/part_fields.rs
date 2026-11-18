//! Expressions in part fields (#1986): a field whose text starts with `=`
//! is evaluated with `signex-expr`, through the one variable model of
//! [`crate::variables`]. The source text is what is saved; the evaluated
//! text, with its derived unit, is what is drawn.
//!
//! A name in a field expression is looked up innermost first: the part's
//! own fields (`= Value * 2`), then the sheet's and the project's variables.
//! `R1.Value` is the field of the part designated `R1` on the same sheet; a
//! multi-unit part resolves to the part, whichever unit is placed first.
//! [`PartFields`] qualifies every own-field name of a part's expression with
//! the part's key (`Value` in R2 becomes `R2.Value`), so one evaluation
//! follows references across parts and the evaluator catches a cycle
//! between two parts. See `docs/architecture/signex-expr.md`.

use std::borrow::Cow;
use std::collections::HashMap;

use signex_expr::{ExprNode, eval, parse};

use crate::schematic::Symbol;
use crate::variables::{Binding, ExprError, FieldText, Layered, Resolver, classify};

/// The text of `field` on `symbol`: `Value`, `Footprint`, `Datasheet`,
/// `Designator` (or `Reference`), else a field or a custom property, an
/// exact name first, then one in any ASCII case.
pub fn part_field_text<'a>(symbol: &'a Symbol, field: &str) -> Option<&'a str> {
    let builtin = match field.to_ascii_lowercase().as_str() {
        "value" => Some(&symbol.value),
        "footprint" => Some(&symbol.footprint),
        "datasheet" => Some(&symbol.datasheet),
        "designator" | "reference" => Some(&symbol.reference),
        _ => None,
    };
    if let Some(text) = builtin {
        return Some(text.as_str());
    }
    if let Some(value) = symbol.fields.get(field) {
        return Some(value.as_str());
    }
    let properties = &symbol.custom_properties;
    if let Some(property) = properties.iter().find(|p| p.key == field) {
        return Some(property.value.as_str());
    }
    let any_case = |key: &str| key.eq_ignore_ascii_case(field);
    if let Some((_, value)) = symbol.fields.iter().find(|(key, _)| any_case(key)) {
        return Some(value.as_str());
    }
    let property = properties.iter().find(|p| any_case(&p.key))?;
    Some(property.value.as_str())
}

/// The key a part's fields are named under: its designator once it has a
/// number, else its uuid (an unannotated `R?` names no single part).
pub fn part_key(symbol: &Symbol) -> String {
    if symbol.reference.is_empty() || symbol.reference.contains('?') {
        symbol.uuid.to_string()
    } else {
        symbol.reference.clone()
    }
}

/// The parts of one sheet as a [`Resolver`] of `<key>.<field>` names.
pub struct PartFields<'a> {
    symbols: &'a [Symbol],
}

impl<'a> PartFields<'a> {
    pub fn new(symbols: &'a [Symbol]) -> Self {
        Self { symbols }
    }

    /// The part `key` names: the first symbol with that designator (a
    /// multi-unit part's units share it), else the symbol with that uuid.
    fn part(&self, key: &str) -> Option<&'a Symbol> {
        let symbols = self.symbols;
        symbols
            .iter()
            .find(|s| s.reference == key && !key.contains('?'))
            .or_else(|| symbols.iter().find(|s| s.uuid.to_string() == key))
    }
}

impl Resolver for PartFields<'_> {
    fn resolve(&self, name: &str) -> Result<Option<Binding<'_>>, ExprError> {
        let Some((key, field)) = name.split_once('.') else {
            return Ok(None);
        };
        let Some(symbol) = self.part(key) else {
            return Ok(None);
        };
        let Some(text) = part_field_text(symbol, field) else {
            return Ok(None);
        };
        match classify(text) {
            FieldText::Literal(_) => Ok(Some(Binding::Text(Cow::Borrowed(text)))),
            FieldText::Expression(body) => {
                let node = qualify(parse(body)?, symbol, key);
                Ok(Some(Binding::Expr(Cow::Owned(node))))
            }
        }
    }
}

/// `node` with every bare name that is a field of `symbol` qualified as
/// `<key>.<name>`; other names (variables, other parts) are left as written.
fn qualify(node: ExprNode, symbol: &Symbol, key: &str) -> ExprNode {
    let q = |n: ExprNode| qualify(n, symbol, key);
    let boxed = |n: Box<ExprNode>| Box::new(qualify(*n, symbol, key));
    let own_field = |name: &str| !name.contains('.') && part_field_text(symbol, name).is_some();
    match node {
        ExprNode::Ref(name) if own_field(&name) => ExprNode::Ref(format!("{key}.{name}")),
        ExprNode::Literal(_) | ExprNode::Ref(_) => node,
        ExprNode::Binary(op, lhs, rhs) => ExprNode::Binary(op, boxed(lhs), boxed(rhs)),
        ExprNode::Unary(op, inner) => ExprNode::Unary(op, boxed(inner)),
        ExprNode::Ternary(cond, then, otherwise) => {
            ExprNode::Ternary(boxed(cond), boxed(then), boxed(otherwise))
        }
        ExprNode::Lookup {
            key: lookup,
            keys,
            values,
        } => ExprNode::Lookup {
            key: boxed(lookup),
            keys: keys.into_iter().map(q).collect(),
            values: values.into_iter().map(q).collect(),
        },
        ExprNode::Call(func, args) => ExprNode::Call(func, args.into_iter().map(q).collect()),
    }
}

/// What `field` of `symbol` shows: `None` for a literal (drawn as typed),
/// else its evaluated value with its unit, or why it does not evaluate.
/// `symbols` are the parts of its sheet; `variables` the sheet's and the
/// project's variable tables, innermost first.
pub fn part_field_value(
    symbols: &[Symbol],
    symbol: &Symbol,
    field: &str,
    variables: &[&dyn Resolver],
) -> Option<Result<String, ExprError>> {
    let text = part_field_text(symbol, field)?;
    if let FieldText::Literal(_) = classify(text) {
        return None;
    }
    let parts = PartFields::new(symbols);
    let mut layers: Vec<&dyn Resolver> = vec![&parts];
    layers.extend_from_slice(variables);
    let model = Layered::new(layers);
    // Evaluated as a reference to the field itself, so a field that refers
    // back to itself, directly or through another part, is a cycle.
    let reference = ExprNode::Ref(format!("{}.{field}", part_key(symbol)));
    Some(eval(&reference, &model).map(|value| value.to_string()))
}

/// [`part_field_value`] as drawn: the value, or the source text when the
/// expression does not evaluate (the error goes to Messages).
pub fn part_field_display(
    symbols: &[Symbol],
    symbol: &Symbol,
    field: &str,
    variables: &[&dyn Resolver],
) -> Option<String> {
    match part_field_value(symbols, symbol, field, variables)? {
        Ok(value) => Some(value),
        Err(_) => part_field_text(symbol, field).map(str::to_string),
    }
}

/// A part field whose expression does not evaluate.
#[derive(Debug, Clone, PartialEq)]
pub struct PartFieldIssue {
    pub part: String,
    pub field: String,
    pub error: ExprError,
}

impl std::fmt::Display for PartFieldIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} field '{}': {}", self.part, self.field, self.error)
    }
}

/// The value field and the user fields of `symbol`, by the name an
/// expression uses for each.
fn expression_fields(symbol: &Symbol) -> Vec<String> {
    let mut names = vec!["Value".to_string()];
    names.extend(symbol.fields.keys().cloned());
    names.extend(symbol.custom_properties.iter().map(|p| p.key.clone()));
    names
}

/// Every expression field of the parts in `symbols` that does not evaluate,
/// one per part (the first unit of a multi-unit part).
pub fn part_field_issues(symbols: &[Symbol], variables: &[&dyn Resolver]) -> Vec<PartFieldIssue> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for symbol in symbols.iter().filter(|s| !s.is_power) {
        let key = part_key(symbol);
        if !seen.insert(key.clone()) {
            continue;
        }
        for field in expression_fields(symbol) {
            if let Some(Err(error)) = part_field_value(symbols, symbol, &field, variables) {
                out.push(PartFieldIssue {
                    part: key.clone(),
                    field,
                    error,
                });
            }
        }
    }
    out
}

/// `text` with the part references of `renames` (old designator to new)
/// renamed, for an `=` expression; `None` when nothing changes. `= R1.Value`
/// becomes `= R5.Value` when R1 is renumbered to R5. Only a name followed by
/// `.` is a part reference; a unit after a number (`10mA`) is never one.
pub fn rename_part_references(text: &str, renames: &HashMap<String, String>) -> Option<String> {
    if renames.is_empty() || !matches!(classify(text), FieldText::Expression(_)) {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    let mut rest = text;
    let mut previous: Option<char> = None;
    while let Some(c) = rest.chars().next() {
        let starts_name = (c.is_ascii_alphabetic() || c == '_')
            && !previous.is_some_and(|p| p.is_ascii_alphanumeric() || p == '_' || p == '.');
        if !starts_name {
            out.push(c);
            previous = Some(c);
            rest = &rest[c.len_utf8()..];
            continue;
        }
        let end = rest
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .unwrap_or(rest.len());
        let name = &rest[..end];
        match renames.get(name) {
            Some(new) if rest[end..].starts_with('.') => {
                out.push_str(new);
                changed = true;
            }
            _ => out.push_str(name),
        }
        previous = name.chars().last();
        rest = &rest[end..];
    }
    changed.then_some(out)
}

/// Rename the part references in every field expression of `symbol` (its
/// value, footprint and user fields); returns whether any changed.
pub fn rename_part_references_in(symbol: &mut Symbol, renames: &HashMap<String, String>) -> bool {
    let mut changed = false;
    let mut rename = |text: &mut String| {
        if let Some(renamed) = rename_part_references(text, renames) {
            *text = renamed;
            changed = true;
        }
    };
    rename(&mut symbol.value);
    rename(&mut symbol.footprint);
    symbol.fields.values_mut().for_each(&mut rename);
    for property in &mut symbol.custom_properties {
        rename(&mut property.value);
    }
    changed
}

#[cfg(test)]
#[path = "part_fields_tests.rs"]
mod tests;
