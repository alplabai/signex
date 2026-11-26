//! One step of a project's rule history: plain data, UI-free (ADR-0006).

use signex_types::project::ProjectRules;

/// What one rule-history step restores: the project's stored rule set and
/// its router tool diameter, which a fab tier or profile preset sets in
/// the same step as its rules.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleState {
    pub rules: Option<ProjectRules>,
    pub router_tool_diameter: Option<f64>,
}

/// Test-only: production records a full [`RuleState`]; a rule set alone
/// would make its undo clear the project's router tool.
#[cfg(test)]
impl From<Option<ProjectRules>> for RuleState {
    /// A rule set alone, with no router tool.
    fn from(rules: Option<ProjectRules>) -> Self {
        Self {
            rules,
            router_tool_diameter: None,
        }
    }
}
