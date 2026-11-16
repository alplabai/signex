//! A rule's fields written back as one line of rule text.

use signex_drc::NetScope;
use signex_types::violation::Severity;

use super::{RowLimit, RuleKind};
use crate::ast::Bound;

/// The editable fields of one rule: what a table row shows and what
/// `rules_add` takes.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleDraft {
    pub name: String,
    pub enabled: bool,
    pub kind: RuleKind,
    pub scope: NetScope,
    pub layer: Option<String>,
    pub pair: Option<[String; 2]>,
    pub limits: Vec<RowLimit>,
    pub severity: Severity,
    pub message: Option<String>,
}

impl RuleDraft {
    /// A board-wide error rule of `kind` with no limit yet.
    pub fn new(kind: RuleKind, name: impl Into<String>) -> Self {
        RuleDraft {
            name: name.into(),
            enabled: true,
            kind,
            scope: NetScope::Board,
            layer: None,
            pair: None,
            limits: Vec::new(),
            severity: Severity::Error,
            message: None,
        }
    }
}

/// `draft` as rule text: `rule <name> on <target> [between ..] [when ..]
/// require .. then <severity> ["message"]`, on one line.
pub fn render(draft: &RuleDraft) -> String {
    let mut out = String::new();
    if !draft.enabled {
        out.push_str("disabled ");
    }
    out.push_str(&format!("rule {} on {}", draft.name, draft.kind.keyword()));
    if let Some([a, b]) = &draft.pair {
        out.push_str(&format!(" between {a} and {b}"));
    }
    let mut conds: Vec<String> = Vec::new();
    match &draft.scope {
        NetScope::Board => {}
        NetScope::Net(net) => conds.push(format!("net == \"{net}\"")),
        NetScope::Class(class) => conds.push(format!("class == \"{class}\"")),
        NetScope::Pattern(glob) => conds.push(format!("net matches \"{glob}\"")),
    }
    if let Some(layer) = &draft.layer {
        if super::edit::is_name(layer) {
            conds.push(format!("layer == {layer}"));
        } else {
            conds.push(format!("layer == \"{layer}\""));
        }
    }
    if !conds.is_empty() {
        out.push_str(" when ");
        out.push_str(&conds.join(" and "));
    }
    let limits: Vec<String> = draft
        .limits
        .iter()
        .map(|l| {
            let op = match l.field.bound() {
                Bound::AtLeast => ">=",
                Bound::AtMost => "<=",
            };
            format!("{} {op} {}", l.field.word(), l.value)
        })
        .collect();
    out.push_str(" require ");
    out.push_str(&limits.join(" and "));
    out.push_str(" then ");
    out.push_str(severity_word(draft.severity));
    if let Some(message) = &draft.message {
        out.push_str(&format!(" \"{message}\""));
    }
    out
}

/// The severity as the rule text writes it.
pub(crate) const fn severity_word(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}
