//! The rule editor list filter: plain view state, UI-free (ADR-0006).

/// What the rule editor's list shows (#1289): every rule, or only the rules
/// that differ from the shipped profile, narrowed by a name filter. Each
/// caller of the editor holds its own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleListFilter {
    pub changed_only: bool,
    pub query: String,
}
