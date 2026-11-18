//! The fab floor (#2311): the rules a fab profile or a generic tier writes
//! are named `fab_*`, and together they are the minimums the board's maker
//! can build. Any other written rule may be stricter; one that asks less
//! than the floor for the same limit is applied, and said, never silently.
//! See `docs/DRC_RULES.md`, "Fab profiles".

use crate::user_rules::{LimitKind, ScopedRule};

/// Prefix of the rules a fab profile or tier writes.
pub const FAB_RULE_PREFIX: &str = "fab_";

/// A written rule asking less than the fab floor for one limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BelowFloor {
    pub(crate) rule: String,
    pub(crate) floor: String,
    pub(crate) kind: LimitKind,
    pub(crate) value_nm: i64,
    pub(crate) floor_nm: i64,
}

/// Whether `name` is a fab floor rule.
pub fn is_fab_rule(name: &str) -> bool {
    name.starts_with(FAB_RULE_PREFIX)
}

/// Each non-fab rule limit looser than the strictest fab rule of that
/// limit, in written order.
pub(crate) fn below_floor(rules: &[ScopedRule]) -> Vec<BelowFloor> {
    let mut out = Vec::new();
    for &kind in LimitKind::ALL {
        let floor = rules
            .iter()
            .filter(|s| is_fab_rule(&s.rule.name))
            .filter_map(|s| Some((s.rule.limit(kind)?, &s.rule.name)))
            .reduce(|held, next| {
                if kind.stricter(next.0, held.0) {
                    next
                } else {
                    held
                }
            });
        let Some((floor_nm, floor)) = floor else {
            continue;
        };
        for scoped in rules.iter().filter(|s| !is_fab_rule(&s.rule.name)) {
            let Some(value_nm) = scoped.rule.limit(kind) else {
                continue;
            };
            if kind.stricter(floor_nm, value_nm) {
                out.push(BelowFloor {
                    rule: scoped.rule.name.clone(),
                    floor: floor.clone(),
                    kind,
                    value_nm,
                    floor_nm,
                });
            }
        }
    }
    out
}

/// The Messages line for one rule below the floor.
pub(crate) fn note(below: &BelowFloor) -> String {
    format!(
        "DRC rule '{}' asks {} {} mm, past the fab floor '{}' of {} mm",
        below.rule,
        below.kind.as_str(),
        crate::findings::mm(below.value_nm),
        below.floor,
        crate::findings::mm(below.floor_nm)
    )
}
