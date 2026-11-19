//! The No-ERC directive (#1699): a point marker that silences ERC findings
//! located on it. It names the ERC rules it silences; an empty list silences
//! every rule, which is what a marker saved before the list existed loads as.
//! Design note: `docs/SCHEMATIC_DIRECTIVES.md`, "No-ERC rule list".

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ALL_ERC_RULES, Point};

/// A No-ERC directive placed on a sheet (`SchematicSheet::no_erc_directives`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoErcDirective {
    pub uuid: Uuid,
    pub position: Point,
    /// ERC rule ids (`builtin::unused_pin`) the directive silences; empty
    /// silences every rule. Omitted when empty, so a marker without a list
    /// saves exactly as it did before the list existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<String>,
}

impl NoErcDirective {
    /// A directive at `position` that silences every rule.
    pub fn new(uuid: Uuid, position: Point) -> Self {
        Self {
            uuid,
            position,
            rules: Vec::new(),
        }
    }

    /// Whether this directive silences `rule_id` at its point.
    pub fn suppresses(&self, rule_id: &str) -> bool {
        no_erc_rules_suppress(&self.rules, rule_id)
    }
}

/// The stored form of a rule list: sorted, without repeats, and empty when it
/// names [`ALL_ERC_RULES`], so a list saves the same whatever order it was
/// picked in. The engine stores every list in this form.
pub fn canonical_no_erc_rules(mut rules: Vec<String>) -> Vec<String> {
    if rules.iter().any(|r| r == ALL_ERC_RULES) {
        return Vec::new();
    }
    rules.sort_unstable();
    rules.dedup();
    rules
}

/// Whether a No-ERC rule list silences `rule_id`: an empty list, or one that
/// names the rule or [`ALL_ERC_RULES`]. The one test every suppression path
/// shares (the per-sheet run and the project-wide passes).
pub fn no_erc_rules_suppress(rules: &[String], rule_id: &str) -> bool {
    rules.is_empty() || rules.iter().any(|r| r == rule_id || r == ALL_ERC_RULES)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_origin(rules: &[&str]) -> NoErcDirective {
        NoErcDirective {
            uuid: Uuid::nil(),
            position: Point::new(0.0, 0.0),
            rules: rules.iter().map(|r| r.to_string()).collect(),
        }
    }

    #[test]
    fn an_empty_list_suppresses_every_rule() {
        assert!(at_origin(&[]).suppresses("builtin::unused_pin"));
        assert!(at_origin(&[]).suppresses("user::anything"));
    }

    #[test]
    fn a_list_suppresses_only_its_rules() {
        let d = at_origin(&["builtin::unused_pin"]);
        assert!(d.suppresses("builtin::unused_pin"));
        assert!(!d.suppresses("builtin::pin_type_conflict"));
        assert!(at_origin(&[ALL_ERC_RULES]).suppresses("builtin::pin_type_conflict"));
    }

    #[test]
    fn the_canonical_list_is_sorted_without_repeats_and_star_is_every_rule() {
        let ids = |list: &[&str]| list.iter().map(|r| r.to_string()).collect::<Vec<_>>();
        assert_eq!(
            canonical_no_erc_rules(ids(&["builtin::b", "builtin::a", "builtin::b"])),
            ids(&["builtin::a", "builtin::b"])
        );
        assert!(canonical_no_erc_rules(ids(&["builtin::a", ALL_ERC_RULES])).is_empty());
    }
}
