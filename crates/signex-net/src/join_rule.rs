//! The one name rule of the headless intent checks (#2199): whether an edit
//! that joins nets (or bus runs), or names one, needs `allow_merge`. The
//! per-sheet checks, the bus checks and the project check all read it. The
//! rule is frozen by #2199 comment 6761836527; `docs/CLI.md`, "Why
//! `allow_merge`".

use std::collections::BTreeSet;

/// The names one net (or bus run) carries, split as the rule reads them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NetNames {
    /// Net labels, global labels/ports and power ports (not power flags).
    pub own: BTreeSet<String>,
    /// Hierarchical label names.
    pub aliases: BTreeSet<String>,
}

impl NetNames {
    pub fn is_named(&self) -> bool {
        !self.own.is_empty() || !self.aliases.is_empty()
    }

    /// Every name, own and alias, sorted.
    pub fn all(&self) -> BTreeSet<String> {
        self.own.union(&self.aliases).cloned().collect()
    }

    fn carries(&self, name: &String) -> bool {
        self.own.contains(name) || self.aliases.contains(name)
    }
}

/// Whether the edit that made `absorbed` (the named nets before it) one net
/// whose own names are `after_own` needs `allow_merge`: two own-named nets
/// with different own sets, two alias-only nets sharing no alias, or a new
/// own name on a net with own names (a rename). Symmetric, so the order of
/// the nets does not matter. Rule: `docs/CLI.md`, "Wire intent", points 1-7.
pub(crate) fn needs_allow_merge(absorbed: &[&NetNames], after_own: &BTreeSet<String>) -> bool {
    let named: Vec<&NetNames> = absorbed.iter().copied().filter(|n| n.is_named()).collect();
    let has_own = named.iter().any(|n| !n.own.is_empty());
    let renamed = has_own
        && after_own
            .iter()
            .any(|name| !named.iter().any(|n| n.carries(name)));
    renamed
        || named
            .iter()
            .enumerate()
            .any(|(i, a)| named[i + 1..].iter().any(|b| short(a, b)))
}

/// Whether two named nets joined are a short.
fn short(a: &NetNames, b: &NetNames) -> bool {
    match (a.own.is_empty(), b.own.is_empty()) {
        (false, false) => a.own != b.own,
        (true, true) => a.aliases.is_disjoint(&b.aliases),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(own: &[&str], aliases: &[&str]) -> NetNames {
        let set = |names: &[&str]| names.iter().map(|n| (*n).to_string()).collect();
        NetNames {
            own: set(own),
            aliases: set(aliases),
        }
    }

    /// The verdict of joining `a` and `b`, asserted equal in both orders.
    fn join(a: &NetNames, b: &NetNames) -> bool {
        let names = a.own.union(&b.own).cloned().collect();
        let forward = needs_allow_merge(&[a, b], &names);
        let backward = needs_allow_merge(&[b, a], &names);
        assert_eq!(forward, backward, "order changed the verdict: {a:?} {b:?}");
        forward
    }

    #[test]
    fn two_own_named_nets_with_different_own_sets_are_a_short() {
        assert!(join(&net(&["I2C_SDA", "SDA"], &[]), &net(&["SDA"], &[])));
        assert!(join(&net(&["VCC"], &[]), &net(&["GND"], &[])));
        assert!(!join(&net(&["VCC"], &[]), &net(&["VCC"], &["X"])));
    }

    #[test]
    fn an_own_named_net_joins_an_alias_only_one_freely() {
        // 6761836527 point 3: even when the own-named net carries other
        // aliases (I2C1_SDA + SDA with SCL), which an earlier round refused.
        assert!(!join(&net(&["I2C1_SDA"], &["SDA"]), &net(&[], &["SCL"])));
        assert!(!join(&net(&["+5V"], &[]), &net(&[], &["VIN"])));
    }

    #[test]
    fn two_alias_only_nets_short_only_when_they_share_no_name() {
        assert!(join(&net(&[], &["SDA"]), &net(&[], &["SCL"])));
        assert!(!join(&net(&[], &["SDA"]), &net(&[], &["SDA"])));
        assert!(!join(&net(&[], &["SDA", "I2C_DATA"]), &net(&[], &["SDA"])));
        assert!(!join(&net(&[], &[]), &net(&["GND"], &["X"])));
    }

    #[test]
    fn a_new_own_name_renames_and_an_alias_or_a_carried_name_does_not() {
        // The own names after a label `name` is placed on VCC + alias SIG.
        let one = net(&["VCC"], &["SIG"]);
        let with = |name: &str| BTreeSet::from(["VCC".into(), name.to_string()]);
        assert!(!needs_allow_merge(&[&one], &with("SIG")));
        assert!(needs_allow_merge(&[&one], &with("GND")));
        // A gained alias (a hierarchical label) leaves the own names alone.
        let vcc = BTreeSet::from(["VCC".to_string()]);
        assert!(!needs_allow_merge(&[&net(&["VCC"], &[])], &vcc));
        // A net with no own name may gain one.
        assert!(!needs_allow_merge(
            &[&net(&[], &["SDA"])],
            &BTreeSet::from(["I2C_SDA".to_string()])
        ));
    }
}
