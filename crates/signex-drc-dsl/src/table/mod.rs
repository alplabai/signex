//! The rule text as a table (#2398): one row per written rule, with the
//! rule's span in the text. The table and the text are two views of one
//! rule list: editing a row rewrites only that rule's span, so every other
//! rule stays exactly as typed. A rule the table cannot show field by field
//! (a condition or limit it has no column for) is still a row, marked
//! `text_only`, and is edited in the text. See `docs/DRC_RULES.md`,
//! "Rules manager".

mod edit;
mod kinds;
pub mod matrix;
pub mod presets;
mod render;
pub mod sets;

pub use edit::{
    EditError, RuleField, add_rule, enable_rule, parse_bool, parse_scope, remove_rule,
    replace_rule, scope_text, set_field,
};
pub use kinds::{LimitField, RuleKind};
pub use render::{RuleDraft, render};

use signex_drc::{NetScope, UserRule};
use signex_types::violation::Severity;

use crate::ast::{CondOp, LimitAst, RuleAst, SeverityAst, Span};
use crate::{Diagnostic, Judged, Level};

/// A limit with its length as written (`0.2mm`, `8 mil`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowLimit {
    pub field: LimitField,
    pub value: String,
}

/// One written rule.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleRow {
    pub name: String,
    /// The rule in the text, from `disabled` or `rule` to its last word.
    pub span: Span,
    pub enabled: bool,
    /// `None` for a target this build does not know.
    pub kind: Option<RuleKind>,
    pub scope: NetScope,
    /// The layer condition as written (`outer`, `TopCopper`).
    pub layer: Option<String>,
    /// The object pair as written (`track`, `pad`).
    pub pair: Option<[String; 2]>,
    pub limits: Vec<RowLimit>,
    pub severity: Severity,
    pub message: Option<String>,
    /// The table has no column for part of it; edited in the text.
    pub text_only: bool,
    /// What the compiler said about this rule.
    pub problems: Vec<Diagnostic>,
    /// The compiled rule, also for a disabled one.
    pub compiled: Option<UserRule>,
    /// Run DRC applies it.
    pub applied: bool,
}

impl RuleRow {
    /// The rule id severities and violations name (`user::<name>`).
    pub fn id(&self) -> String {
        format!("{}{}", RULE_ID_PREFIX, self.name)
    }

    pub fn limit(&self, field: LimitField) -> Option<&RowLimit> {
        self.limits.iter().find(|l| l.field == field)
    }

    pub fn error_count(&self) -> usize {
        self.problems
            .iter()
            .filter(|d| d.level == Level::Error)
            .count()
    }

    /// The rule's text.
    pub fn text<'a>(&self, src: &'a str) -> &'a str {
        src.get(self.span.clone()).unwrap_or("")
    }

    /// The row as an editable draft; `None` for a text-only row.
    pub fn draft(&self) -> Option<RuleDraft> {
        if self.text_only {
            return None;
        }
        Some(RuleDraft {
            name: self.name.clone(),
            enabled: self.enabled,
            kind: self.kind?,
            scope: self.scope.clone(),
            layer: self.layer.clone(),
            pair: self.pair.clone(),
            limits: self.limits.clone(),
            severity: self.severity,
            message: self.message.clone(),
        })
    }
}

/// Prefix of a written rule's id, as `signex-drc` writes it.
pub const RULE_ID_PREFIX: &str = "user::";

/// The rows of one rule text.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuleTable {
    pub rows: Vec<RuleRow>,
    /// Syntax errors; with any, `rows` is empty and no rule applies.
    pub syntax: Vec<Diagnostic>,
}

impl RuleTable {
    /// The first rule called `name` (a later one of the same name is the
    /// repeated one); `user::<name>` is read as `name`.
    pub fn row(&self, name: &str) -> Option<&RuleRow> {
        let name = name.strip_prefix(RULE_ID_PREFIX).unwrap_or(name);
        self.rows.iter().find(|r| r.name == name)
    }

    pub fn is_unparsed(&self) -> bool {
        !self.syntax.is_empty()
    }
}

/// The table of `src`.
pub fn table(src: &str) -> RuleTable {
    let asts = match crate::parser::parse(src) {
        Ok(asts) => asts,
        Err(errors) => {
            let syntax = errors
                .into_iter()
                .map(|(span, message)| Diagnostic {
                    level: Level::Error,
                    rule: None,
                    span,
                    message,
                })
                .collect();
            return RuleTable {
                rows: Vec::new(),
                syntax,
            };
        }
    };
    let judged = crate::judge(&asts);
    let rows = asts
        .iter()
        .zip(judged)
        .map(|(ast, judged)| row_of(src, ast, judged))
        .collect();
    RuleTable {
        rows,
        syntax: Vec::new(),
    }
}

fn row_of(src: &str, ast: &RuleAst, judged: Judged) -> RuleRow {
    let kind = RuleKind::from_keyword(&ast.target.text);
    let mut text_only = kind.is_none();
    let (mut scope, mut layer) = (NetScope::Board, None);
    let (mut scopes, mut layers) = (0, 0);
    let mut others: Vec<String> = Vec::new();
    for cond in &ast.when {
        let value = cond.value.text.clone();
        match (cond.subject.text.as_str(), cond.op) {
            ("net", CondOp::Is) => scope = NetScope::Net(value),
            ("net", CondOp::Matches) => scope = NetScope::Pattern(value),
            ("class", CondOp::Is) => scope = NetScope::Class(value),
            ("other_class", CondOp::Is) => {
                others.push(value);
                continue;
            }
            ("layer", CondOp::Is) => {
                layer = Some(value);
                layers += 1;
                continue;
            }
            _ => {
                text_only = true;
                continue;
            }
        }
        scopes += 1;
    }
    // `class == "A" and other_class == "B"` is one scope cell (#2398).
    let paired = match (others.as_slice(), &scope) {
        ([], _) => None,
        ([other], NetScope::Class(class)) => {
            Some(Some(NetScope::ClassPair(class.clone(), other.clone())))
        }
        _ => Some(None),
    };
    match paired {
        None => {}
        Some(Some(pair)) => scope = pair,
        Some(None) => text_only = true,
    }
    text_only |= scopes > 1 || layers > 1;
    let mut limits: Vec<RowLimit> = Vec::new();
    for limit in &ast.require {
        let field = kind.and_then(|k| LimitField::of(k, &limit.field.text, limit.bound));
        match field {
            Some(field) if limits.iter().all(|l| l.field != field) => limits.push(RowLimit {
                field,
                value: written_length(src, limit),
            }),
            _ => text_only = true,
        }
    }
    RuleRow {
        name: ast.name.text.clone(),
        span: tight(src, &ast.span),
        enabled: !ast.disabled,
        kind,
        scope,
        layer,
        pair: ast
            .pair
            .as_ref()
            .map(|[a, b]| [a.text.clone(), b.text.clone()]),
        limits,
        severity: match ast.severity {
            SeverityAst::Error => Severity::Error,
            SeverityAst::Warning => Severity::Warning,
            SeverityAst::Info => Severity::Info,
        },
        message: ast.message.clone(),
        text_only,
        problems: judged.said,
        compiled: judged.rule,
        applied: judged.applied,
    }
}

/// The length as written: the number through its unit.
fn written_length(src: &str, limit: &LimitAst) -> String {
    let end = limit
        .unit
        .as_ref()
        .map_or(limit.number.span.end, |u| u.span.end);
    src.get(limit.number.span.start..end)
        .unwrap_or(&limit.number.text)
        .to_string()
}

/// `span` without the whitespace the parser took in around it.
pub(crate) fn tight(src: &str, span: &Span) -> Span {
    let Some(text) = src.get(span.clone()) else {
        return span.clone();
    };
    let start = span.start + (text.len() - text.trim_start().len());
    let end = span.start + text.trim_end().len();
    start..end.max(start)
}

#[cfg(test)]
mod matrix_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tier_tests;
