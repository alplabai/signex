//! Sheet and project variables (#1987): the one variable model.
//!
//! A variable is a name bound to a stored text. The project's table is the
//! `.snxprj` `parameters` map; a sheet's table is `SchematicSheet::variables`
//! (`[extras.sheet.variables]` in `.snxsch`), and a sheet variable shadows a
//! project variable of the same name. A value is a literal text, or an `=`
//! expression ([`signex_expr::classify`]) evaluated with `signex-expr`; the
//! source text is what is saved, the evaluated text is what is drawn.
//!
//! `${NAME}` special strings, the markup's `@{name}` and an expression's
//! references all read the same tables through a [`Resolver`]: a
//! [`TextTable`] per scope, stacked innermost first with [`Layered`] (part
//! fields, then sheet, then project). See `docs/architecture/signex-expr.md`.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::hash::BuildHasher;

pub use signex_expr::{Binding, ExprError, FieldText, Layered, Resolver, classify};
use signex_expr::{ExprNode, eval, parse};

/// A table of stored texts (variables, part fields, designators, pin nets)
/// read as a [`Resolver`]. A name matches exactly first, then ignoring ASCII
/// case, as the markup's lookups always did.
pub struct TextTable<'a, M>(pub &'a M);

/// The first entry of `entries` whose key matches `name` exactly, else the
/// first that matches ignoring ASCII case.
fn text_binding<'a>(
    exact: Option<&'a String>,
    mut entries: impl Iterator<Item = (&'a String, &'a String)>,
    name: &str,
) -> Option<Binding<'a>> {
    exact
        .or_else(|| {
            entries
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value)
        })
        .map(|value| Binding::Text(Cow::Borrowed(value.as_str())))
}

impl Resolver for TextTable<'_, BTreeMap<String, String>> {
    fn resolve(&self, name: &str) -> Result<Option<Binding<'_>>, ExprError> {
        Ok(text_binding(self.0.get(name), self.0.iter(), name))
    }
}

impl<S: BuildHasher> Resolver for TextTable<'_, HashMap<String, String, S>> {
    fn resolve(&self, name: &str) -> Result<Option<Binding<'_>>, ExprError> {
        Ok(text_binding(self.0.get(name), self.0.iter(), name))
    }
}

/// Whether `name` can be a variable: `[A-Za-z_][A-Za-z0-9_]*`, the shape a
/// `${NAME}` special string and an expression reference both accept.
pub fn is_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// What the stored `text` shows under the `=` rule: an expression is
/// evaluated through `resolver`, a literal is shown as written (an escaping
/// `==` becomes `=`).
pub fn display_text(text: &str, resolver: &dyn Resolver) -> Result<String, ExprError> {
    match classify(text) {
        FieldText::Expression(body) => Ok(eval(&parse(body)?, resolver)?.to_string()),
        FieldText::Literal(literal) => Ok(literal.to_string()),
    }
}

/// What the variable `name` shows through `resolver`; `None` when no layer
/// knows it. An expression is evaluated as a reference to `name`, so a
/// cycle through it is reported as [`ExprError::Cycle`].
pub fn variable_text(resolver: &dyn Resolver, name: &str) -> Option<Result<String, ExprError>> {
    let literal = match resolver.resolve(name) {
        Ok(None) => return None,
        Err(error) => return Some(Err(error)),
        Ok(Some(Binding::Value(value))) => Some(value.to_string()),
        Ok(Some(Binding::Text(text))) => match classify(&text) {
            FieldText::Literal(literal) => Some(literal.to_string()),
            FieldText::Expression(_) => None,
        },
        Ok(Some(Binding::Expr(_))) => None,
    };
    if let Some(text) = literal {
        return Some(Ok(text));
    }
    let reference = ExprNode::Ref(name.to_string());
    Some(eval(&reference, resolver).map(|value| value.to_string()))
}

/// What the variable `name` is drawn as: [`variable_text`], or its source
/// text when its expression does not evaluate (the error is reported in
/// Messages, not drawn). `None` when no layer knows it.
pub fn variable_display(resolver: &dyn Resolver, name: &str) -> Option<String> {
    match variable_text(resolver, name)? {
        Ok(text) => Some(text),
        Err(_) => Some(match resolver.resolve(name) {
            Ok(Some(Binding::Text(text))) => text.into_owned(),
            _ => String::new(),
        }),
    }
}

/// Which table a variable is defined in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableScope {
    Sheet,
    Project,
}

/// A variable whose `=` expression does not evaluate: a cycle, an unknown
/// name, a unit mismatch. Reported in the Messages panel.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableIssue {
    pub scope: VariableScope,
    pub name: String,
    pub error: ExprError,
}

impl std::fmt::Display for VariableIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let scope = match self.scope {
            VariableScope::Sheet => "Sheet",
            VariableScope::Project => "Project",
        };
        write!(f, "{scope} variable '{}': {}", self.name, self.error)
    }
}

/// The expression variables of `table` that do not evaluate through
/// `resolver`, in name order. Literal values are text and never fail.
fn issues_in(
    scope: VariableScope,
    table: &BTreeMap<String, String>,
    resolver: &dyn Resolver,
) -> Vec<VariableIssue> {
    table
        .iter()
        .filter(|(_, text)| matches!(classify(text), FieldText::Expression(_)))
        .filter_map(|(name, _)| {
            let error = eval(&ExprNode::Ref(name.clone()), resolver).err()?;
            Some(VariableIssue {
                scope,
                name: name.clone(),
                error,
            })
        })
        .collect()
}

/// The project variables that do not evaluate on their own.
pub fn project_variable_issues(project: &BTreeMap<String, String>) -> Vec<VariableIssue> {
    issues_in(VariableScope::Project, project, &TextTable(project))
}

/// The sheet variables that do not evaluate on their sheet, where they
/// shadow the project's of the same name.
pub fn sheet_variable_issues(
    sheet: &BTreeMap<String, String>,
    project: &BTreeMap<String, String>,
) -> Vec<VariableIssue> {
    let (sheet_table, project_table) = (TextTable(sheet), TextTable(project));
    let layered = Layered::new(vec![&sheet_table as &dyn Resolver, &project_table]);
    issues_in(VariableScope::Sheet, sheet, &layered)
}

#[cfg(test)]
#[path = "variables_tests.rs"]
mod tests;
