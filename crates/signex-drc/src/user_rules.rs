//! Rules written in the DRC rule language (#2307 step 2b), compiled by
//! `signex-drc-dsl` into [`UserRule`]s and resolved against one board here.
//! Precedence is specificity first, strictness only between equals; the
//! board setup and net-class values stay below every written rule. See
//! `docs/architecture/pcb-drc.md`, "Rule language".

use std::collections::BTreeSet;

use signex_types::net_class::{NetClass, first_matching_class, glob_matches};
use signex_types::pcb::PcbBoard;
use signex_types::violation::Severity;

use pcb_geom::ItemRef;

pub use crate::limit_kind::{Limit, LimitKind};

/// Which nets a rule is for, least specific first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetScope {
    /// Every net.
    Board,
    /// Nets whose name matches a glob (`*`, `?`), as net-class patterns do.
    Pattern(String),
    /// The nets of one net class (a net's first matching class).
    Class(String),
    /// One net, by name.
    Net(String),
    /// A clearance between a net of one class and a net of the other, in
    /// either order (#2398, a class-to-class matrix cell). Ranks as a class
    /// rule with one more narrowing condition.
    ClassPair(String, String),
}

impl NetScope {
    /// The classes a class or class-pair scope names.
    pub fn classes(&self) -> Vec<&str> {
        match self {
            NetScope::Class(class) => vec![class.as_str()],
            NetScope::ClassPair(a, b) => vec![a.as_str(), b.as_str()],
            _ => Vec::new(),
        }
    }
}

/// A copper object a clearance rule can name. Holes, the board edge, mask
/// and silkscreen are not measured yet; the language reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CopperObject {
    Track,
    Pad,
    Via,
}

impl CopperObject {
    /// The kind of `item`; `None` for a footprint, a drawn area, a string or
    /// a dimension, which the copper rules do not measure. A copper arc is
    /// measured as a track and a free pad as a pad (#2373).
    pub fn of(item: &ItemRef) -> Option<CopperObject> {
        match item {
            ItemRef::Track(_) | ItemRef::Arc(_) => Some(CopperObject::Track),
            ItemRef::Pad { .. } | ItemRef::FreePad(_) => Some(CopperObject::Pad),
            ItemRef::Via(_) => Some(CopperObject::Via),
            ItemRef::Footprint(_)
            | ItemRef::Zone { .. }
            | ItemRef::Text(_)
            | ItemRef::Dimension(_)
            | ItemRef::Graphic { .. } => None,
        }
    }
}

/// Prefix of a written rule's id (`user::<name>`).
pub(crate) const USER_RULE_ID_PREFIX: &str = "user::";

/// A compiled rule of the rule language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRule {
    /// The name after `rule`; the rule id is `user::<name>`.
    pub name: String,
    /// From the rule's `then`; a severity entry for its id overrides it.
    pub severity: Severity,
    pub message: Option<String>,
    pub nets: NetScope,
    /// Copper layer bit mask (see [`pcb_geom::layer_bit`]); `None` for
    /// every layer.
    pub layers: Option<u32>,
    /// For a clearance rule: the two objects it is between, either order.
    pub pair: Option<[CopperObject; 2]>,
    pub limits: Vec<Limit>,
}

impl UserRule {
    pub fn id(&self) -> String {
        format!("{USER_RULE_ID_PREFIX}{}", self.name)
    }

    /// The value it sets for `kind`, if it limits that.
    pub fn limit(&self, kind: LimitKind) -> Option<i64> {
        self.limits
            .iter()
            .map(|l| l.split())
            .find(|(k, _)| *k == kind)
            .map(|(_, nm)| nm)
    }

    /// Specificity: the net scope first (net, class, pattern, board-wide:
    /// 3 down to 0), then how many narrowing conditions it adds (a layer,
    /// an object pair). A layer-only rule is `(0, 1)`, above board-wide.
    pub fn rank(&self) -> (u8, u8) {
        let scope = match self.nets {
            NetScope::Net(_) => 3,
            NetScope::Class(_) | NetScope::ClassPair(..) => 2,
            NetScope::Pattern(_) => 1,
            NetScope::Board => 0,
        };
        let narrowing = u8::from(self.layers.is_some())
            + u8::from(self.pair.is_some())
            + u8::from(matches!(self.nets, NetScope::ClassPair(..)));
        (scope, narrowing)
    }
}

/// A rule with the board's nets it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScopedRule {
    pub(crate) rule: UserRule,
    /// `None` for every net; for a class pair, the nets of both classes.
    nets: Option<BTreeSet<u32>>,
    /// For a class pair, each class's nets: a side is covered only with a
    /// net of the other class across.
    sides: Option<[BTreeSet<u32>; 2]>,
}

impl ScopedRule {
    pub(crate) fn resolve(rule: UserRule, board: &PcbBoard, classes: &[NetClass]) -> Self {
        let in_class = |class: &str, name: &str| {
            first_matching_class(classes, name).is_some_and(|c| c.name == class)
        };
        let collect = |member: &dyn Fn(&str) -> bool| -> BTreeSet<u32> {
            board
                .nets
                .iter()
                .filter(|n| member(&n.name))
                .map(|n| n.number)
                .collect()
        };
        let (nets, sides) = match &rule.nets {
            NetScope::Board => (None, None),
            NetScope::Net(net) => (Some(collect(&|name: &str| net == name)), None),
            NetScope::Pattern(glob) => {
                (Some(collect(&|name: &str| glob_matches(glob, name))), None)
            }
            NetScope::Class(class) => (
                Some(collect(&|name: &str| in_class(class.as_str(), name))),
                None,
            ),
            NetScope::ClassPair(a, b) => {
                let side_a = collect(&|name: &str| in_class(a.as_str(), name));
                let side_b = collect(&|name: &str| in_class(b.as_str(), name));
                let both = side_a.union(&side_b).copied().collect();
                (Some(both), Some([side_a, side_b]))
            }
        };
        ScopedRule { rule, nets, sides }
    }

    /// A net, class or pattern scope that covers no net of the board; a
    /// class pair one of whose classes has none.
    pub(crate) fn covers_no_net(&self) -> bool {
        if let Some([a, b]) = &self.sides {
            return a.is_empty() || b.is_empty();
        }
        self.nets.as_ref().is_some_and(BTreeSet::is_empty)
    }

    /// Whether it is a class-pair rule.
    pub(crate) fn is_pair_scope(&self) -> bool {
        self.sides.is_some()
    }

    /// For a class pair: whether `net` faces `other` across the pair, in
    /// either order. Any other rule takes every partner.
    pub(crate) fn covers_partner(&self, net: u32, other: Option<u32>) -> bool {
        let Some([a, b]) = &self.sides else {
            return true;
        };
        let Some(other) = other else {
            return false;
        };
        (a.contains(&net) && b.contains(&other)) || (b.contains(&net) && a.contains(&other))
    }

    /// How it sees `net`: 0 not covered, 1 covered, and for a class pair 1
    /// or 2 by side (3 on both). Nets alike in this resolve alike.
    pub(crate) fn side_of(&self, net: u32) -> u8 {
        match &self.sides {
            Some([a, b]) => u8::from(a.contains(&net)) + 2 * u8::from(b.contains(&net)),
            None => u8::from(self.covers_net(net)),
        }
    }

    pub(crate) fn covers_net(&self, net: u32) -> bool {
        self.nets.as_ref().is_none_or(|set| set.contains(&net))
    }

    pub(crate) fn covers(&self, net: u32, layers: u32) -> bool {
        self.covers_net(net) && self.rule.layers.is_none_or(|mask| mask & layers != 0)
    }

    pub(crate) fn pairs_with(&self, objects: [Option<CopperObject>; 2]) -> bool {
        let Some([x, y]) = self.rule.pair else {
            return true;
        };
        let [Some(a), Some(b)] = objects else {
            return false;
        };
        (a, b) == (x, y) || (a, b) == (y, x)
    }
}

/// The limit of `kind` the written rules set for an object of `net` on
/// `layers` (for a clearance, one side of the pair, `objects` naming both
/// sides' kinds): the most specific rule's, the strictest among equals,
/// the first written among identical ones. `None` when no rule sets one.
/// A class-pair rule takes no part: see [`pick_between`].
pub(crate) fn pick(
    rules: &[ScopedRule],
    kind: LimitKind,
    net: u32,
    layers: u32,
    objects: [Option<CopperObject>; 2],
) -> Option<(i64, &ScopedRule)> {
    pick_between(rules, kind, net, None, layers, objects)
}

/// [`pick`] for one side of a clearance whose other side is `other`: a
/// class-pair rule takes part when `net` and `other` sit in its two
/// classes.
pub(crate) fn pick_between(
    rules: &[ScopedRule],
    kind: LimitKind,
    net: u32,
    other: Option<u32>,
    layers: u32,
    objects: [Option<CopperObject>; 2],
) -> Option<(i64, &ScopedRule)> {
    let mut best: Option<((u8, u8), i64, &ScopedRule)> = None;
    for scoped in rules {
        if !scoped.covers(net, layers)
            || !scoped.pairs_with(objects)
            || !scoped.covers_partner(net, other)
        {
            continue;
        }
        let rank = scoped.rule.rank();
        for (k, value) in scoped.rule.limits.iter().map(|l| l.split()) {
            if k != kind {
                continue;
            }
            let better =
                best.is_none_or(|(r, v, _)| rank > r || (rank == r && kind.stricter(value, v)));
            if better {
                best = Some((rank, value, scoped));
            }
        }
    }
    best.map(|(_, value, scoped)| (value, scoped))
}

/// Whether any rule sets a limit of `kind`.
pub(crate) fn sets(rules: &[ScopedRule], kind: LimitKind) -> bool {
    rules
        .iter()
        .flat_map(|s| s.rule.limits.iter())
        .any(|l| l.split().0 == kind)
}

/// The largest clearance a rule covering `net` sets; 0 with none.
pub(crate) fn widest_clearance(rules: &[ScopedRule], net: u32) -> i64 {
    widest_of(rules.iter().filter(|s| s.covers_net(net)))
}

/// The largest clearance any rule sets, whatever it covers; 0 with none.
pub(crate) fn widest_written_clearance(rules: &[ScopedRule]) -> i64 {
    widest_of(rules.iter())
}

fn widest_of<'a>(rules: impl Iterator<Item = &'a ScopedRule>) -> i64 {
    rules
        .flat_map(|s| s.rule.limits.iter())
        .filter_map(|l| match l {
            Limit::MinClearance(nm) => Some(*nm),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}
