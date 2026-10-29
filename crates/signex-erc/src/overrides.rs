//! Per-rule severity overrides. Applied after a run so the engine stays
//! ignorant of user preferences; the app hands in its override map and shows
//! whatever comes back.

use std::collections::HashMap;

use crate::{RuleKind, Severity, Violation};

/// Apply the user's per-rule severity overrides to `violations`, drop the
/// ones overridden to [`Severity::Off`], and order the rest Error → Warning →
/// Info (ties broken by rule name for a stable report).
///
/// An override is the user's verbatim choice: it replaces the severity of
/// every violation of that built-in rule, including rules that grade
/// individual findings (`BadHierSheetPin`). The exception is an
/// [`RuleKind::is_on_off_only`] rule (`PinTypeConflict`): its per-cell
/// severities come from the pin matrix, so only `Off` applies and any other
/// row value leaves each finding at its cell's severity. Overrides are resolved through
/// [`Violation::rule_id`], so they only ever touch built-ins: a DSL (`user::`)
/// violation matches nothing here and keeps the severity its rule declared.
pub fn apply_overrides(
    mut violations: Vec<Violation>,
    overrides: &HashMap<RuleKind, Severity>,
) -> Vec<Violation> {
    for v in &mut violations {
        let Some(kind) = v.builtin_kind() else {
            continue;
        };
        if let Some(&sev) = overrides.get(&kind)
            && (!kind.is_on_off_only() || sev == Severity::Off)
        {
            v.severity = sev;
        }
    }
    violations.retain(|v| v.severity != Severity::Off);
    violations.sort_by_cached_key(|v| {
        let bucket = match v.severity {
            Severity::Error => 0,
            Severity::Warning => 1,
            Severity::Info => 2,
            Severity::Off => 3,
        };
        // Built-ins keep their historical tiebreak (the kind's variant name)
        // so existing reports order identically; DSL rules sort by id.
        let name = match v.builtin_kind() {
            Some(kind) => format!("{kind:?}"),
            None => v.rule_id.to_string(),
        };
        // Within a rule, the order a reader expects, not emission order
        // (rules pick their reported unit or pin by UUID): the message in
        // natural order, so designators read C3, R1, R2, R12, then location.
        (
            bucket,
            name,
            natural_key(&v.message),
            signex_net::pt_key(&v.location),
        )
    });
    violations
}

/// One run of a [`natural_key`]: text, or a number compared by value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Chunk {
    /// Digits without leading zeros, compared by length then text, so any
    /// length of digits orders by value without overflow.
    Number(usize, String),
    Text(String),
}

/// `s` split into text and digit runs, so "R12" sorts after "R2".
fn natural_key(s: &str) -> Vec<Chunk> {
    let mut out = Vec::new();
    let mut run = String::new();
    let mut digits = false;
    let flush = |run: &mut String, digits: bool, out: &mut Vec<Chunk>| {
        if run.is_empty() {
            return;
        }
        let taken = std::mem::take(run);
        out.push(if digits {
            let value = taken.trim_start_matches('0').to_string();
            Chunk::Number(value.len(), value)
        } else {
            Chunk::Text(taken)
        });
    };
    for c in s.chars() {
        let is_digit = c.is_ascii_digit();
        if is_digit != digits {
            flush(&mut run, digits, &mut out);
            digits = is_digit;
        }
        run.push(c);
    }
    flush(&mut run, digits, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::violations_for;

    const LEVELS: [Severity; 4] = [
        Severity::Error,
        Severity::Warning,
        Severity::Info,
        Severity::Off,
    ];

    #[test]
    fn overriding_each_rule_changes_every_one_of_its_violations() {
        for kind in RuleKind::ALL {
            let base = violations_for(kind);
            assert!(!base.is_empty(), "fixture for {kind:?} produced nothing");
            for level in LEVELS {
                let map = HashMap::from([(kind, level)]);
                let out = apply_overrides(base.clone(), &map);
                if level == Severity::Off {
                    assert!(out.is_empty(), "{kind:?} Off must drop every finding");
                } else if kind.is_on_off_only() {
                    assert_eq!(out.len(), base.len(), "{kind:?} -> {level:?}");
                    assert!(
                        out.iter().zip(&base).all(|(o, b)| o.severity == b.severity),
                        "{kind:?} row severity must not flatten its findings",
                    );
                } else {
                    assert_eq!(out.len(), base.len(), "{kind:?} -> {level:?}");
                    assert!(
                        out.iter().all(|v| v.severity == level),
                        "{kind:?} -> {level:?} left a finding at its old severity",
                    );
                }
            }
        }
    }

    #[test]
    fn an_override_only_touches_its_own_rule() {
        let mut all = violations_for(RuleKind::UnusedPin);
        all.extend(violations_for(RuleKind::MissingPowerFlag));
        let map = HashMap::from([(RuleKind::MissingPowerFlag, Severity::Error)]);
        let out = apply_overrides(all, &map);
        for v in out {
            match v.rule {
                RuleKind::MissingPowerFlag => assert_eq!(v.severity, Severity::Error),
                other => assert_eq!(v.severity, other.default_severity()),
            }
        }
    }

    fn dsl_violation(placeholder: RuleKind, severity: Severity) -> Violation {
        Violation::new(
            crate::RuleId::user("my_rule"),
            placeholder,
            severity,
            "user rule",
            signex_types::schematic::Point::new(0.0, 0.0),
        )
    }

    #[test]
    fn builtin_overrides_never_touch_a_dsl_violation() {
        for kind in RuleKind::ALL {
            for level in LEVELS {
                let v = dsl_violation(kind, Severity::Warning);
                let out = apply_overrides(vec![v], &HashMap::from([(kind, level)]));
                assert_eq!(out.len(), 1, "{kind:?} -> {level:?} dropped a DSL rule");
                assert_eq!(out[0].severity, Severity::Warning);
            }
        }
    }

    #[test]
    fn every_builtin_violation_carries_its_own_rule_id() {
        for kind in RuleKind::ALL {
            for v in violations_for(kind) {
                assert_eq!(v.rule_id, crate::RuleId::builtin(kind));
                assert_eq!(v.builtin_kind(), Some(kind));
                assert_eq!(v.display_name(), kind.label());
            }
        }
    }

    #[test]
    fn converting_a_diagnostic_keeps_its_rule_id_not_the_placeholder_kind() {
        // The compiler files a DSL rule under a placeholder built-in kind;
        // the conversion must carry the diagnostic's own id, not rebuild one
        // from that kind.
        let mut d = crate::Diagnostic::new(
            RuleKind::NetLabelConflict,
            "user rule",
            signex_types::schematic::Point::new(0.0, 0.0),
        );
        d.rule_id = crate::RuleId::user("my_rule");
        let v = Violation::from(d);
        assert_eq!(v.rule_id, crate::RuleId::user("my_rule"));
        assert_eq!(v.builtin_kind(), None);
    }

    #[test]
    fn a_dsl_violation_is_named_by_its_own_id() {
        let v = dsl_violation(RuleKind::NetLabelConflict, Severity::Info);
        assert_eq!(v.builtin_kind(), None);
        assert_eq!(v.display_name(), "user::my_rule");
    }

    #[test]
    fn findings_of_one_rule_read_in_natural_designator_order() {
        let finding = |reference: &str| {
            Violation::new(
                crate::RuleId::builtin(RuleKind::PartWithoutFootprint),
                RuleKind::PartWithoutFootprint,
                Severity::Info,
                format!("Part {reference} has no footprint"),
                signex_types::schematic::Point::new(0.0, 0.0),
            )
        };
        for order in [["R12", "C3", "R1", "R2"], ["R1", "R2", "R12", "C3"]] {
            let out = apply_overrides(order.iter().map(|r| finding(r)).collect(), &HashMap::new());
            let shown: Vec<&str> = out.iter().map(|v| v.message.as_str()).collect();
            assert_eq!(
                shown,
                [
                    "Part C3 has no footprint",
                    "Part R1 has no footprint",
                    "Part R2 has no footprint",
                    "Part R12 has no footprint",
                ]
            );
        }
    }

    #[test]
    fn output_is_sorted_error_first() {
        let mut all = violations_for(RuleKind::MissingPowerFlag);
        all.extend(violations_for(RuleKind::DuplicateRefDesignator));
        let out = apply_overrides(all, &HashMap::new());
        let ranks: Vec<_> = out.iter().map(|v| v.severity).collect();
        assert_eq!(ranks.first(), Some(&Severity::Error));
        assert_eq!(ranks.last(), Some(&Severity::Info));
    }
}
