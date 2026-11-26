//! Expressions in part fields (#1986): a field whose text starts with `=`
//! is evaluated with `signex-expr`, through the one variable model of
//! [`crate::variables`]. The source text is what is saved; the evaluated
//! text, with its derived unit, is what is drawn and exported.
//!
//! A name in a field expression is looked up innermost first: the part's
//! own fields (`= Value * 2`), then the sheet's and the project's variables.
//! `R1.Value` is the field of the part designated `R1` on the same sheet; a
//! multi-unit part resolves to the part, whichever unit is placed first.
//! [`PartFields`] qualifies every own-field name of a part's expression with
//! the part's key ([`crate::part_keys`]), so one evaluation follows
//! references across parts and the evaluator catches a cycle between two
//! parts. See `docs/architecture/signex-expr.md`.

use std::borrow::Cow;

use signex_expr::{ExprNode, eval, parse};

use crate::part_keys::{part_key, part_named};
use crate::schematic::Symbol;
use crate::variables::{Binding, ExprError, FieldText, Layered, Resolver, classify};

pub use crate::part_keys::{follow_part_keys, rename_part_references, rename_part_references_in};

/// The text of `field` on `symbol`: `Value`, `Footprint`, `Datasheet`,
/// `Designator` (or `Reference`), else a field or a custom property, an
/// exact name first, then one in any ASCII case. Of fields whose names
/// differ only in case, the first by name (byte order) is read, so the
/// answer never depends on the map's order.
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
    let field_in_any_case = symbol
        .fields
        .iter()
        .filter(|(key, _)| any_case(key))
        .min_by(|(a, _), (b, _)| a.cmp(b));
    if let Some((_, value)) = field_in_any_case {
        return Some(value.as_str());
    }
    let property = properties.iter().find(|p| any_case(&p.key))?;
    Some(property.value.as_str())
}

/// The parts of one sheet as a [`Resolver`] of `<key>.<field>` names.
pub struct PartFields<'a> {
    symbols: &'a [Symbol],
    /// The designators are shown with the product-aspect sign (`-QAC1`,
    /// EN IEC 81346-2 full codes), which an expression must not type.
    signed: bool,
}

impl<'a> PartFields<'a> {
    pub fn new(symbols: &'a [Symbol], signed: bool) -> Self {
        Self { symbols, signed }
    }
}

impl Resolver for PartFields<'_> {
    fn resolve(&self, name: &str) -> Result<Option<Binding<'_>>, ExprError> {
        let Some((key, field)) = name.split_once('.') else {
            return Ok(None);
        };
        let Some(symbol) = part_named(self.symbols, key) else {
            return Ok(None);
        };
        let Some(text) = part_field_text(symbol, field) else {
            return Ok(None);
        };
        match classify(text) {
            FieldText::Literal(_) => Ok(Some(Binding::Text(Cow::Borrowed(text)))),
            FieldText::Expression(body) => {
                refuse_signed_reference(body, self.symbols, self.signed)?;
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

/// The identifier at the start of `text` (`[A-Za-z0-9_]*`).
fn leading_name(text: &str) -> &str {
    let end = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(text.len());
    &text[..end]
}

/// Refuse `-QAC1.Value` in an expression: under the full-codes standard the
/// leading `-` of a designator is display and export only (#1416), so a
/// reference written with it would silently negate the value. A `-` is
/// refused when it is unary (only an operator, `(` or nothing before it), a
/// part reference follows it directly, and that part's designator is shown
/// signed (`signed`, or an export copy that stores `-QAC1`).
fn refuse_signed_reference(body: &str, symbols: &[Symbol], signed: bool) -> Result<(), ExprError> {
    let operators = "+-*/%^(,<>=!&|?:[";
    let mut previous: Option<char> = None;
    for (at, c) in body.char_indices() {
        let unary = previous.is_none_or(|p| operators.contains(p));
        if c == '-' && unary {
            let rest = &body[at + 1..];
            let name = leading_name(rest);
            let field = rest[name.len()..].strip_prefix('.').map(leading_name);
            let shown_signed = |part: &Symbol| signed || part.reference.starts_with('-');
            if let Some(field) = field
                && part_named(symbols, name).is_some_and(shown_signed)
            {
                return Err(ExprError::Domain(format!(
                    "'-{name}': the leading '-' of a designator is display-only; \
                     write {name}.{field}"
                )));
            }
        }
        if !c.is_whitespace() {
            previous = Some(c);
        }
    }
    Ok(())
}

/// What `field` of `symbol` shows: `None` for a literal (drawn as typed),
/// else its evaluated value with its unit, or why it does not evaluate.
/// `symbols` are the parts of its sheet; `variables` the sheet's and the
/// project's variable tables, innermost first; `signed` whether the
/// designators are shown with the product-aspect sign.
pub fn part_field_value(
    symbols: &[Symbol],
    symbol: &Symbol,
    field: &str,
    variables: &[&dyn Resolver],
    signed: bool,
) -> Option<Result<String, ExprError>> {
    let text = part_field_text(symbol, field)?;
    let FieldText::Expression(body) = classify(text) else {
        return None;
    };
    if let Err(error) = refuse_signed_reference(body, symbols, signed) {
        return Some(Err(error));
    }
    let key = part_key(symbol, symbols);
    let symbols = with_own_text(symbols, symbol, &key, field, text);
    let parts = PartFields::new(&symbols, signed);
    let mut layers: Vec<&dyn Resolver> = vec![&parts];
    layers.extend_from_slice(variables);
    let model = Layered::new(layers);
    // Evaluated as a reference to the field itself, so a field that refers
    // back to itself, directly or through another part, is a cycle.
    let reference = ExprNode::Ref(format!("{key}.{field}"));
    Some(eval(&reference, &model).map(|value| value.to_string()))
}

/// `symbols` with the part `key` names read as `symbol`, when `symbol`'s
/// `field` says something else than the stored part's: `symbol` is then a
/// drawn copy (a variant's own value, #2948), and its expression must be
/// the one evaluated, not the sheet's text.
fn with_own_text<'a>(
    symbols: &'a [Symbol],
    symbol: &Symbol,
    key: &str,
    field: &str,
    text: &str,
) -> Cow<'a, [Symbol]> {
    let Some(at) = part_named(symbols, key)
        .filter(|stored| part_field_text(stored, field) != Some(text))
        .and_then(|stored| symbols.iter().position(|s| s.uuid == stored.uuid))
    else {
        return Cow::Borrowed(symbols);
    };
    let mut owned = symbols.to_vec();
    owned[at] = symbol.clone();
    Cow::Owned(owned)
}

/// [`part_field_value`] as drawn: the value, or the source text when the
/// expression does not evaluate (the error goes to Messages).
pub fn part_field_display(
    symbols: &[Symbol],
    symbol: &Symbol,
    field: &str,
    variables: &[&dyn Resolver],
    signed: bool,
) -> Option<String> {
    match part_field_value(symbols, symbol, field, variables, signed)? {
        Ok(value) => Some(value),
        Err(_) => part_field_text(symbol, field).map(str::to_string),
    }
}

/// A part's value as the BOM, the netlist, ERC and the Net Inspector read
/// it (#1986): an `=` expression evaluated, a literal as stored. A power
/// port's value is a net name and is never evaluated; a value that does not
/// evaluate keeps its source text.
pub fn evaluated_value(
    symbols: &[Symbol],
    symbol: &Symbol,
    variables: &[&dyn Resolver],
    signed: bool,
) -> String {
    if symbol.is_power {
        return symbol.value.clone();
    }
    part_field_display(symbols, symbol, "Value", variables, signed)
        .unwrap_or_else(|| symbol.value.clone())
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
pub fn part_field_issues(
    symbols: &[Symbol],
    variables: &[&dyn Resolver],
    signed: bool,
) -> Vec<PartFieldIssue> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for symbol in symbols.iter().filter(|s| !s.is_power) {
        let key = part_key(symbol, symbols);
        if !seen.insert(key.clone()) {
            continue;
        }
        // Messages names a part by its designator; the uuid key only when
        // it has none.
        let part = if symbol.reference.is_empty() {
            key
        } else {
            symbol.reference.clone()
        };
        for field in expression_fields(symbol) {
            let value = part_field_value(symbols, symbol, &field, variables, signed);
            if let Some(Err(error)) = value {
                out.push(PartFieldIssue {
                    part: part.clone(),
                    field,
                    error,
                });
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "part_fields_tests.rs"]
mod tests;
