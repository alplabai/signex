//! Per-net waivers (#1581): a designer-accepted net-level finding, keyed by
//! rule id and net *name*.
//!
//! A net-level finding is deliberately not hidden by a No-ERC marker on one of
//! its pins, so this is the only way to dismiss one. The key is the net name,
//! not a pin or a position, so moving pins keeps the waiver. Renaming the net
//! ends it: the finding is then about a name no waiver mentions and shows
//! again, while the old entry stays stored (and listed) until it is restored.
//! A finding on an unnamed net can never be waived.

use std::collections::{BTreeMap, BTreeSet};

use crate::Violation;

/// Rule id -> names of the nets waived for that rule. The shape the project
/// file stores (`ProjectRules::erc_net_waivers`).
pub type NetWaivers = BTreeMap<String, BTreeSet<String>>;

/// The (rule id, net name) a finding would be waived by, or `None` for a
/// finding that is not net-level or sits on an unnamed net.
pub fn waiver_key(v: &Violation) -> Option<(String, String)> {
    let net = v.net.as_ref().filter(|n| !n.name.is_empty())?;
    Some((v.rule_id.to_string(), net.name.clone()))
}

/// Add (`waived`) or remove one waiver. Returns whether `waivers` changed. A
/// rule left with no nets is dropped so the file carries no empty entries.
pub fn set_waived(waivers: &mut NetWaivers, rule_id: &str, net: &str, waived: bool) -> bool {
    if waived {
        return waivers
            .entry(rule_id.to_string())
            .or_default()
            .insert(net.to_string());
    }
    let Some(nets) = waivers.get_mut(rule_id) else {
        return false;
    };
    let removed = nets.remove(net);
    if nets.is_empty() {
        waivers.remove(rule_id);
    }
    removed
}

/// Split `violations` into the ones that stay and a count of the findings
/// each waived (rule id, net) pair hid, so the app can show them as waived
/// instead of dropping them silently.
pub fn split_waived(
    violations: Vec<Violation>,
    waivers: &NetWaivers,
) -> (Vec<Violation>, BTreeMap<(String, String), usize>) {
    let mut hidden: BTreeMap<(String, String), usize> = BTreeMap::new();
    if waivers.is_empty() {
        return (violations, hidden);
    }
    let kept = violations
        .into_iter()
        .filter(|v| {
            let Some((rule, net)) = waiver_key(v) else {
                return true;
            };
            if waivers.get(&rule).is_some_and(|nets| nets.contains(&net)) {
                *hidden.entry((rule, net)).or_default() += 1;
                return false;
            }
            true
        })
        .collect();
    (kept, hidden)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NetFinding, RuleId, RuleKind, Severity};
    use signex_types::schematic::Point;

    fn net_violation(rule: &str, net: &str) -> Violation {
        let mut v = Violation::new(
            RuleId::user(rule),
            RuleKind::UnusedPin,
            Severity::Error,
            "m",
            Point::new(0.0, 0.0),
        );
        v.net = Some(NetFinding {
            name: net.into(),
            pins: Vec::new(),
        });
        v
    }

    #[test]
    fn a_waiver_hides_only_its_rule_and_net() {
        let mut w = NetWaivers::new();
        assert!(set_waived(&mut w, "user::r", "A", true));
        let all = vec![
            net_violation("r", "A"),
            net_violation("r", "B"),
            net_violation("other", "A"),
        ];
        let (kept, hidden) = split_waived(all, &w);
        assert_eq!(kept.len(), 2);
        assert_eq!(hidden[&("user::r".into(), "A".into())], 1);
    }

    #[test]
    fn unwaiving_restores_and_drops_empty_rules() {
        let mut w = NetWaivers::new();
        set_waived(&mut w, "user::r", "A", true);
        assert!(!set_waived(&mut w, "user::r", "A", true), "idempotent");
        assert!(set_waived(&mut w, "user::r", "A", false));
        assert!(w.is_empty());
        let (kept, hidden) = split_waived(vec![net_violation("r", "A")], &w);
        assert_eq!((kept.len(), hidden.len()), (1, 0));
    }

    #[test]
    fn a_renamed_net_no_longer_matches() {
        let mut w = NetWaivers::new();
        set_waived(&mut w, "user::r", "OLD", true);
        let (kept, hidden) = split_waived(vec![net_violation("r", "NEW")], &w);
        assert_eq!(kept.len(), 1);
        assert!(hidden.is_empty());
    }

    #[test]
    fn unnamed_and_non_net_findings_are_never_waived() {
        let mut w = NetWaivers::new();
        set_waived(&mut w, "user::r", "", true);
        let mut plain = net_violation("r", "A");
        plain.net = None;
        let (kept, _) = split_waived(vec![net_violation("r", ""), plain], &w);
        assert_eq!(kept.len(), 2);
    }
}
