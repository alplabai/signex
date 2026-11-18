//! Flags on written rules (#2398): a rule that never governs anything on
//! the board (shadowed), two equally specific rules that ask different
//! values of the same nets (a conflict), a rule that lowers a net class's
//! clearance, and a scope that matches no net. Each is found by the same
//! resolution the check uses, over the board's nets, copper layers and
//! object pairs. See `docs/DRC_RULES.md`, "Rules manager".

use std::collections::BTreeSet;

use signex_types::net_class::NetClass;
use signex_types::pcb::PcbBoard;

use crate::DrcRules;
use crate::user_rules::{self, CopperObject, LimitKind, ScopedRule};

/// What is wrong with one written rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleFlag {
    /// It never sets a value on this board: `by` always wins where it
    /// applies.
    Shadowed { by: String },
    /// An equally specific rule asks a different value of some of the
    /// same nets; the stricter one wins.
    Conflict { with: String },
    /// It asks less than net class `class`'s nets would need without it.
    LowersClass {
        class: String,
        from_nm: i64,
        to_nm: i64,
    },
    /// Its net, class or pattern scope matches no net of this board.
    NoNets,
    /// It asks less than the fab floor rule `floor` (#2311) for `limit`:
    /// `from_nm` is the floor's value, `to_nm` this rule's.
    BelowFabFloor {
        floor: String,
        limit: LimitKind,
        from_nm: i64,
        to_nm: i64,
    },
}

impl RuleFlag {
    pub const fn as_str(&self) -> &'static str {
        match self {
            RuleFlag::Shadowed { .. } => "shadowed",
            RuleFlag::Conflict { .. } => "conflict",
            RuleFlag::LowersClass { .. } => "lowers_class",
            RuleFlag::NoNets => "no_nets",
            RuleFlag::BelowFabFloor { .. } => "below_fab_floor",
        }
    }
}

/// A flag and the rule it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlaggedRule {
    pub rule: String,
    pub flag: RuleFlag,
}

/// The object pairs a clearance can be between, plus "unnamed".
const PAIRS: [[Option<CopperObject>; 2]; 7] = {
    use CopperObject::{Pad, Track, Via};
    [
        [None, None],
        [Some(Track), Some(Track)],
        [Some(Track), Some(Pad)],
        [Some(Track), Some(Via)],
        [Some(Pad), Some(Pad)],
        [Some(Pad), Some(Via)],
        [Some(Via), Some(Via)],
    ]
};

impl DrcRules {
    /// Every flag on the written rules, rule by rule in written order.
    /// Resolve the rules against `board` first ([`Self::with_user_rules`]).
    pub fn rule_flags(&self, board: &PcbBoard, classes: &[NetClass]) -> Vec<FlaggedRule> {
        let mut out: Vec<FlaggedRule> = Vec::new();
        let mut add = |rule: &str, flag: RuleFlag| {
            let flagged = FlaggedRule {
                rule: rule.to_string(),
                flag,
            };
            if !out.contains(&flagged) {
                out.push(flagged);
            }
        };
        let nets = self.net_representatives(board);
        for scoped in &self.user {
            if scoped.covers_no_net() {
                add(&scoped.rule.name, RuleFlag::NoNets);
                continue;
            }
            for &kind in LimitKind::ALL {
                if scoped.rule.limit(kind).is_none() {
                    continue;
                }
                let found = self.contest(scoped, kind, &nets);
                if let (false, Some(by)) = (found.won, found.beaten_by) {
                    add(&scoped.rule.name, RuleFlag::Shadowed { by });
                }
                for other in found.conflicts {
                    add(
                        &scoped.rule.name,
                        RuleFlag::Conflict {
                            with: other.clone(),
                        },
                    );
                    add(
                        &other,
                        RuleFlag::Conflict {
                            with: scoped.rule.name.clone(),
                        },
                    );
                }
            }
        }
        for lowered in crate::rule_notes::lowered(self, board, classes) {
            add(
                &lowered.rule,
                RuleFlag::LowersClass {
                    class: lowered.class,
                    from_nm: lowered.from_nm,
                    to_nm: lowered.to_nm,
                },
            );
        }
        for below in crate::fab_floor::below_floor(&self.user) {
            add(
                &below.rule,
                RuleFlag::BelowFabFloor {
                    floor: below.floor,
                    limit: below.kind,
                    from_nm: below.floor_nm,
                    to_nm: below.value_nm,
                },
            );
        }
        out
    }

    /// One net for each distinct set of written rules that cover a net, the
    /// first of the board's nets with that set. Resolution reads a net only
    /// through which rules cover it, so that net answers for the others and
    /// the flags cost the number of sets, not the number of nets.
    pub(crate) fn net_representatives(&self, board: &PcbBoard) -> Vec<u32> {
        let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();
        board
            .nets
            .iter()
            .map(|n| n.number)
            .filter(|&net| seen.insert(self.user.iter().map(|s| s.side_of(net)).collect()))
            .collect()
    }

    /// The other sides a contest for `net` looks at: none, unless `scoped`
    /// is a class-pair rule, then each representative net it faces.
    fn partners(scoped: &ScopedRule, net: u32, nets: &[u32]) -> Vec<Option<u32>> {
        if !scoped.is_pair_scope() {
            return vec![None];
        }
        nets.iter()
            .copied()
            .filter(|&other| scoped.covers_partner(net, Some(other)))
            .map(Some)
            .collect()
    }

    /// Where `scoped` applies, whether it ever wins, the first rule that
    /// beat it, and the equally specific rules that beat it with another
    /// value.
    fn contest(&self, scoped: &ScopedRule, kind: LimitKind, nets: &[u32]) -> Contest {
        let mut found = Contest::default();
        let own = scoped.rule.limit(kind);
        let pairs: Vec<[Option<CopperObject>; 2]> = match (kind, scoped.rule.pair) {
            (_, Some([a, b])) => vec![[Some(a), Some(b)]],
            (LimitKind::MinClearance, None) => PAIRS.to_vec(),
            _ => vec![[None, None]],
        };
        let bits: Vec<u32> = (0..32)
            .filter(|bit| self.copper & (1 << bit) != 0)
            .collect();
        for &net in nets.iter().filter(|&&net| scoped.covers_net(net)) {
            let partners = Self::partners(scoped, net, nets);
            let tries: Vec<(u32, Option<u32>)> = bits
                .iter()
                .filter(|&&bit| scoped.covers(net, 1 << bit))
                .flat_map(|&bit| partners.iter().map(move |&other| (bit, other)))
                .collect();
            for (bit, other) in tries {
                for &objects in &pairs {
                    let picked =
                        user_rules::pick_between(&self.user, kind, net, other, 1 << bit, objects);
                    let Some((_, won)) = picked else {
                        continue;
                    };
                    if std::ptr::eq(won, scoped) {
                        found.won = true;
                        continue;
                    }
                    let name = &won.rule.name;
                    if found.beaten_by.is_none() {
                        found.beaten_by = Some(name.clone());
                    }
                    let equal = won.rule.rank() == scoped.rule.rank();
                    if equal && won.rule.limit(kind) != own && !found.conflicts.contains(name) {
                        found.conflicts.push(name.clone());
                    }
                }
            }
        }
        found
    }
}

#[derive(Debug, Default)]
struct Contest {
    won: bool,
    beaten_by: Option<String>,
    conflicts: Vec<String>,
}
