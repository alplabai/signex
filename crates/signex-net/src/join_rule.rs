//! The one name rule of the headless intent checks (#2199): whether an edit
//! that joins nets, or names one, needs `allow_merge`. Both the per-sheet
//! check and the project check read it. Rule and decisions: `docs/CLI.md`,
//! "Why `allow_merge`".

use std::collections::BTreeSet;

/// The names one net carries, split as the rule reads them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NetNames {
    /// Net labels, global labels and power ports.
    pub own: BTreeSet<String>,
    /// Names the net carries only through a hierarchical label.
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
/// whose own names are `after_own` needs `allow_merge`. True when
/// * two absorbed nets that both have own names have different own names;
/// * two absorbed nets with no own name have different aliases;
/// * an own-named net that carries aliases meets a net with no own name whose
///   aliases it does not all carry (`I2C1_SDA` + alias `SDA` with alias `SCL`);
/// * or the result has an own name that no absorbed net carried, own or
///   alias, while one of them had own names (a rename).
///
/// Everything else joins freely: an own-named net with no alias meeting an
/// alias-only one (the parent's `I2C1_SDA` wired to the entry `SDA`), a name
/// the net already carries, a gained alias (a hierarchical label is a port,
/// not a name: #2199 comment 6759998042), and nets with no own name gaining
/// one. Every clause is symmetric, so the verdict does not depend on the
/// nets' order.
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
        (true, true) => a.aliases != b.aliases,
        (false, true) => alias_short(a, b),
        (true, false) => alias_short(b, a),
    }
}

/// An own-named net meeting an alias-only one: a short only when the own-named
/// net already carries aliases and not every alias of the other is among its
/// names.
fn alias_short(owned: &NetNames, alias_only: &NetNames) -> bool {
    !owned.aliases.is_empty() && !alias_only.aliases.iter().all(|n| owned.carries(n))
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
    fn own_name_sets_that_overlap_but_differ_are_a_short_in_either_order() {
        assert!(join(&net(&["I2C_SDA", "SDA"], &[]), &net(&["SDA"], &[])));
    }

    #[test]
    fn an_own_named_net_with_an_alias_meeting_another_alias_is_a_short() {
        let owned = net(&["I2C1_SDA"], &["SDA"]);
        assert!(join(&owned, &net(&[], &["SCL"])));
        assert!(!join(&owned, &net(&[], &["SDA"])));
        // With no alias of its own the parent net takes the entry's name.
        assert!(!join(&net(&["I2C1_SDA"], &[]), &net(&[], &["SCL"])));
    }

    #[test]
    fn same_effective_names_join_and_different_ones_short() {
        assert!(!join(&net(&["VCC"], &[]), &net(&["VCC"], &["X"])));
        assert!(join(&net(&["VCC"], &[]), &net(&["GND"], &[])));
        assert!(!join(&net(&[], &["SDA"]), &net(&[], &["SDA"])));
        assert!(join(&net(&[], &["SDA"]), &net(&[], &["SCL"])));
        assert!(!join(&net(&[], &[]), &net(&["GND"], &["X"])));
    }

    #[test]
    fn a_name_the_net_carries_is_no_rename_and_a_new_one_is() {
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
