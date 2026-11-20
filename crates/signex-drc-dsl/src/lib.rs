//! The DRC rule language (#2307 step 2b): rule text from the rule layers
//! (a profile's `drc.dsl`, the project's `rules.drc.dsl`) parsed,
//! validated and compiled into `signex_drc::UserRule`s. The same family as
//! `signex-erc-dsl`. Reference: `docs/DRC_RULES.md`; design and
//! precedence: `docs/architecture/pcb-drc.md`, "Rule language".
//! `schematic_rules` builds the rules a schematic's Net Parameters
//! directives ask for (#1281) and merges them into rule text.

pub mod ast;
mod compile;
#[cfg(any(test, feature = "test-support"))]
pub mod fixture;
pub mod parser;
pub mod schematic_rules;
pub mod table;

use std::collections::BTreeSet;

use signex_drc::UserRule;
use signex_types::project::DrcRuleSet;

pub use ast::Span;
pub use compile::length_nm;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// The rule (or, for a syntax error, the whole text) is not applied.
    Error,
    /// The rule is valid but not applied, or applied with a caveat.
    Warning,
}

/// One finding about the rule text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: Level,
    /// The rule's name; `None` for a syntax error.
    pub rule: Option<String>,
    /// Byte span in the text.
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    /// 1-based line and column (in characters) of the span's start.
    pub fn line_col(&self, src: &str) -> (usize, usize) {
        let before = &src[..self.span.start.min(src.len())];
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        (line, col)
    }

    /// `line 2, column 5: rule 'hv': ...`.
    pub fn render(&self, src: &str) -> String {
        let (line, col) = self.line_col(src);
        match &self.rule {
            Some(rule) => format!("line {line}, column {col}: rule '{rule}': {}", self.message),
            None => format!("line {line}, column {col}: {}", self.message),
        }
    }
}

/// The rules of one text and what was said about it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Compiled {
    /// The rules applied, in written order.
    pub rules: Vec<UserRule>,
    pub diagnostics: Vec<Diagnostic>,
    /// Rules written but not applied: invalid, repeated, or naming
    /// something not measured yet.
    pub not_applied: usize,
    /// The text has a syntax error, so none of it is applied.
    pub unparsed: bool,
    /// Valid rules written `disabled` (#2398): kept, not applied, not
    /// counted in `not_applied`.
    pub disabled: usize,
}

/// Parse, validate and compile `src`. A syntax error applies no rule of
/// the text; an invalid rule (or a second valid rule of the same name) is
/// dropped alone; a rule naming something not measured yet is not applied.
pub fn compile(src: &str) -> Compiled {
    let asts = match parser::parse(src) {
        Ok(asts) => asts,
        Err(errors) => {
            let diagnostics = errors
                .into_iter()
                .map(|(span, message)| Diagnostic {
                    level: Level::Error,
                    rule: None,
                    span,
                    message,
                })
                .collect();
            return Compiled {
                diagnostics,
                unparsed: true,
                ..Compiled::default()
            };
        }
    };
    let mut out = Compiled::default();
    for judged in judge(&asts) {
        out.not_applied += usize::from(judged.not_applied);
        out.disabled += usize::from(judged.disabled);
        out.diagnostics.extend(judged.said);
        if judged.applied
            && let Some(rule) = judged.rule
        {
            out.rules.push(rule);
        }
    }
    out
}

/// What one parsed rule came to, in written order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Judged {
    /// The compiled rule, also for a disabled one; `None` when invalid,
    /// repeated or not checked yet.
    pub(crate) rule: Option<UserRule>,
    /// Run DRC applies it.
    pub(crate) applied: bool,
    /// Written, meant to apply, and not applied.
    pub(crate) not_applied: bool,
    /// Written `disabled` and valid.
    pub(crate) disabled: bool,
    pub(crate) said: Vec<Diagnostic>,
}

/// Judge each rule: only a valid rule takes its name, so an invalid one
/// leaves it free; a second rule of a taken name is an error. A disabled
/// rule takes its name and is checked like any other, but never applied.
pub(crate) fn judge(asts: &[ast::RuleAst]) -> Vec<Judged> {
    let mut names = BTreeSet::new();
    let mut out = Vec::with_capacity(asts.len());
    for ast in asts {
        let name = ast.name.text.as_str();
        if names.contains(name) {
            out.push(Judged {
                rule: None,
                applied: false,
                not_applied: true,
                disabled: false,
                said: vec![Diagnostic {
                    level: Level::Error,
                    rule: Some(name.to_string()),
                    span: ast.name.span.clone(),
                    message: "a rule of this name is already defined".to_string(),
                }],
            });
            continue;
        }
        let judged = match compile::compile_rule(ast) {
            compile::Outcome::Rule(rule, said) => {
                names.insert(name);
                Judged {
                    rule: Some(rule),
                    applied: !ast.disabled,
                    not_applied: false,
                    disabled: ast.disabled,
                    said,
                }
            }
            compile::Outcome::NotChecked(said) => {
                names.insert(name);
                Judged {
                    rule: None,
                    applied: false,
                    not_applied: !ast.disabled,
                    disabled: ast.disabled,
                    said,
                }
            }
            compile::Outcome::Dropped(said) => Judged {
                rule: None,
                applied: false,
                not_applied: true,
                disabled: false,
                said,
            },
        };
        out.push(judged);
    }
    out
}

/// The rules of the rule layers and the Messages lines about them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayeredRules {
    pub rules: Vec<UserRule>,
    pub messages: Vec<String>,
    /// Rules not applied, over every layer.
    pub not_applied: usize,
    /// Layers whose text has a syntax error, so none of it is applied.
    pub unparsed: usize,
}

/// Compile the `dsl` text of `profile` then `project`. A project rule
/// replaces a profile rule of the same name (a later layer wins). Each
/// message names its layer and says what was not applied.
pub fn compile_layers(profile: Option<&DrcRuleSet>, project: Option<&DrcRuleSet>) -> LayeredRules {
    let mut out = LayeredRules::default();
    let layers = [("profile", profile), ("project", project)];
    for (layer, set) in layers {
        let Some(src) = set.and_then(|s| s.dsl.as_deref()) else {
            continue;
        };
        let compiled = compile(src);
        out.not_applied += compiled.not_applied;
        out.unparsed += usize::from(compiled.unparsed);
        for d in &compiled.diagnostics {
            let what = match (d.level, &d.rule) {
                (Level::Error, None) => "; no rule of this text is applied",
                (Level::Error, Some(_)) => "; the rule is skipped",
                (Level::Warning, _) => "",
            };
            out.messages
                .push(format!("DRC DSL ({layer}): {}{what}", d.render(src)));
        }
        for rule in compiled.rules {
            out.rules.retain(|r| r.name != rule.name);
            out.rules.push(rule);
        }
    }
    out
}

#[cfg(test)]
mod base_rules_tests;
#[cfg(test)]
mod tests;
