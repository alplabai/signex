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
    /// The kind of `item`; `None` for a footprint, which carries no copper.
    pub fn of(item: &ItemRef) -> Option<CopperObject> {
        match item {
            ItemRef::Track(_) => Some(CopperObject::Track),
            ItemRef::Pad { .. } => Some(CopperObject::Pad),
            ItemRef::Via(_) => Some(CopperObject::Via),
            ItemRef::Footprint(_) => None,
        }
    }
}

/// One limit a rule sets, in nm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    MinClearance(i64),
    MinTrackWidth(i64),
    MaxTrackWidth(i64),
    MinViaDiameter(i64),
    MinViaDrill(i64),
}

/// Which limit, without its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LimitKind {
    MinClearance,
    MinTrackWidth,
    MaxTrackWidth,
    MinViaDiameter,
    MinViaDrill,
}

impl Limit {
    fn split(self) -> (LimitKind, i64) {
        match self {
            Limit::MinClearance(nm) => (LimitKind::MinClearance, nm),
            Limit::MinTrackWidth(nm) => (LimitKind::MinTrackWidth, nm),
            Limit::MaxTrackWidth(nm) => (LimitKind::MaxTrackWidth, nm),
            Limit::MinViaDiameter(nm) => (LimitKind::MinViaDiameter, nm),
            Limit::MinViaDrill(nm) => (LimitKind::MinViaDrill, nm),
        }
    }
}

impl LimitKind {
    /// For a maximum the smaller value is the stricter one.
    fn stricter(self, a: i64, b: i64) -> bool {
        match self {
            LimitKind::MaxTrackWidth => a < b,
            _ => a > b,
        }
    }
}

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
        format!("user::{}", self.name)
    }

    /// Specificity: the net scope first (net, class, pattern, board-wide:
    /// 3 down to 0), then how many narrowing conditions it adds (a layer,
    /// an object pair). A layer-only rule is `(0, 1)`, above board-wide.
    fn rank(&self) -> (u8, u8) {
        let scope = match self.nets {
            NetScope::Net(_) => 3,
            NetScope::Class(_) => 2,
            NetScope::Pattern(_) => 1,
            NetScope::Board => 0,
        };
        let narrowing = u8::from(self.layers.is_some()) + u8::from(self.pair.is_some());
        (scope, narrowing)
    }
}

/// A rule with the board's nets it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScopedRule {
    pub(crate) rule: UserRule,
    /// `None` for every net.
    nets: Option<BTreeSet<u32>>,
}

impl ScopedRule {
    pub(crate) fn resolve(rule: UserRule, board: &PcbBoard, classes: &[NetClass]) -> Self {
        let member = |name: &str| match &rule.nets {
            NetScope::Board => true,
            NetScope::Net(net) => net == name,
            NetScope::Pattern(glob) => glob_matches(glob, name),
            NetScope::Class(class) => {
                first_matching_class(classes, name).is_some_and(|c| &c.name == class)
            }
        };
        let nets = (rule.nets != NetScope::Board).then(|| {
            board
                .nets
                .iter()
                .filter(|n| member(&n.name))
                .map(|n| n.number)
                .collect()
        });
        ScopedRule { rule, nets }
    }

    /// A net, class or pattern scope that covers no net of the board.
    pub(crate) fn covers_no_net(&self) -> bool {
        self.nets.as_ref().is_some_and(BTreeSet::is_empty)
    }

    fn covers_net(&self, net: u32) -> bool {
        self.nets.as_ref().is_none_or(|set| set.contains(&net))
    }

    fn covers(&self, net: u32, layers: u32) -> bool {
        self.covers_net(net) && self.rule.layers.is_none_or(|mask| mask & layers != 0)
    }

    fn pairs_with(&self, objects: [Option<CopperObject>; 2]) -> bool {
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
pub(crate) fn pick(
    rules: &[ScopedRule],
    kind: LimitKind,
    net: u32,
    layers: u32,
    objects: [Option<CopperObject>; 2],
) -> Option<(i64, &ScopedRule)> {
    let mut best: Option<((u8, u8), i64, &ScopedRule)> = None;
    for scoped in rules {
        if !scoped.covers(net, layers) || !scoped.pairs_with(objects) {
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
    rules
        .iter()
        .filter(|s| s.covers_net(net))
        .flat_map(|s| s.rule.limits.iter())
        .filter_map(|l| match l {
            Limit::MinClearance(nm) => Some(*nm),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}
