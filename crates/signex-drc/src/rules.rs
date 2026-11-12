//! The rules a run checks against: one board-wide minimum clearance, which
//! a net class can raise for its nets (the implicit rules), and the rules
//! written in the rule language, which rank above them.

use std::collections::BTreeMap;

use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::pcb::PcbBoard;

use pcb_geom::{CopperItem, GeomError, MAX_ABS_NM, mm_to_nm};

use crate::DrcSeverities;
use crate::user_rules::{self, CopperObject, LimitKind, ScopedRule, UserRule};

/// The clearance used when the board's setup holds none (0 is "unset").
pub const DEFAULT_CLEARANCE_MM: f64 = 0.2;

/// Minimum clearances in nm, and the severity of each rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrcRules {
    /// Between any two nets.
    pub clearance: i64,
    /// Per net number, from its net class. A pair of nets needs the largest
    /// of its two nets' values and [`Self::clearance`].
    pub net_clearance: BTreeMap<u32, i64>,
    /// [`Self::clearance`] is [`DEFAULT_CLEARANCE_MM`] because the board set
    /// none; a report says so rather than apply it silently.
    pub from_default: bool,
    /// Severity per rule (#2307); the built-in defaults unless the caller
    /// resolved the profile and project layers. Never filters a violation.
    pub severities: DrcSeverities,
    /// What the rule-language text said that this run does not apply: a
    /// rule that did not compile, or one that names something not checked
    /// yet. Written to Messages with the report.
    pub notes: Vec<String>,
    /// Rule-language rules, resolved against the board's nets.
    pub(crate) user: Vec<ScopedRule>,
}

impl DrcRules {
    pub fn new(clearance: i64) -> Self {
        DrcRules {
            clearance,
            net_clearance: BTreeMap::new(),
            from_default: false,
            severities: DrcSeverities::builtin(),
            notes: Vec::new(),
            user: Vec::new(),
        }
    }

    /// Add rule-language rules, resolving their net scopes against
    /// `board`'s nets and `classes` (the first matching class, as for the
    /// implicit rules).
    pub fn with_user_rules(
        mut self,
        board: &PcbBoard,
        classes: &[NetClass],
        rules: impl IntoIterator<Item = UserRule>,
    ) -> Self {
        self.user.extend(
            rules
                .into_iter()
                .map(|rule| ScopedRule::resolve(rule, board, classes)),
        );
        self
    }

    /// The rule-language rules this run applies, in written order.
    pub fn user_rules(&self) -> impl Iterator<Item = &UserRule> {
        self.user.iter().map(|s| &s.rule)
    }

    /// The clearance two copper items of different nets need where they
    /// share `layers`, and the written rule that set it. Per shared layer
    /// the most specific written rule applies, the strictest among equals;
    /// with none, [`Self::required`]. The pair needs the most of its layers.
    pub(crate) fn required_between(
        &self,
        a: &CopperItem,
        b: &CopperItem,
        layers: u32,
    ) -> (i64, Option<String>) {
        let implicit = self.required(a.net, b.net);
        if !user_rules::sets(&self.user, LimitKind::MinClearance) {
            return (implicit, None);
        }
        let objects = [CopperObject::of(&a.item), CopperObject::of(&b.item)];
        let nets = [a.net, b.net];
        let mut most: (i64, Option<&ScopedRule>) = (i64::MIN, None);
        for bit in (0..32).filter(|bit| layers & (1 << bit) != 0) {
            let on_layer = user_rules::pick(
                &self.user,
                LimitKind::MinClearance,
                &nets,
                1 << bit,
                objects,
            )
            .map_or((implicit, None), |(nm, rule)| (nm, Some(rule)));
            if on_layer.0 > most.0 {
                most = on_layer;
            }
        }
        (most.0, most.1.map(|s| s.rule.id()))
    }

    /// The board's rules: `setup.clearance` (or [`DEFAULT_CLEARANCE_MM`]
    /// when unset), and each net's class clearance where its class sets one.
    /// `classes` is the project's net-class table; the first class that
    /// contains a net wins, as everywhere else.
    pub fn for_board(board: &PcbBoard, classes: &[NetClass]) -> Result<Self, GeomError> {
        let set = board.setup.as_ref().map_or(0.0, |s| s.clearance);
        let board_nm = positive_mm(set, "setup.clearance")?;
        let clearance =
            board_nm.map_or_else(|| mm_to_nm(DEFAULT_CLEARANCE_MM, "default clearance"), Ok)?;
        let mut rules = DrcRules::new(clearance);
        rules.from_default = board_nm.is_none();
        for net in &board.nets {
            let Some(class) = first_matching_class(classes, &net.name) else {
                continue;
            };
            if let Some(nm) = positive_mm(class.clearance, "net class clearance")? {
                rules.net_clearance.insert(net.number, nm);
            }
        }
        Ok(rules)
    }

    /// The clearance two different nets need between them.
    pub fn required(&self, a: u32, b: u32) -> i64 {
        let of = |net: u32| self.net_clearance.get(&net).copied().unwrap_or(0);
        self.clearance.max(of(a)).max(of(b))
    }

    /// The most any pair with `net` in it can need: how far from an item of
    /// `net` another net's copper can matter. Pairs where the other net's
    /// class asks for more are found from that other item; a written rule
    /// can ask for its value from either side, so its largest counts here.
    pub(super) fn reach_of(&self, net: u32) -> i64 {
        let widest = user_rules::widest_clearance(&self.user);
        self.required(net, net).max(widest).min(MAX_ABS_NM)
    }

    /// The limit of `kind` the written rules set for one object, and the
    /// rule's id.
    pub(crate) fn object_limit(
        &self,
        kind: LimitKind,
        net: u32,
        layers: u32,
    ) -> Option<(i64, String)> {
        user_rules::pick(&self.user, kind, &[net], layers, [None, None])
            .map(|(nm, scoped)| (nm, scoped.rule.id()))
    }

    pub(crate) fn sets(&self, kind: LimitKind) -> bool {
        user_rules::sets(&self.user, kind)
    }
}

/// `Some(nm)` for a set value, `None` for unset (0 or less), an error for
/// a value that is not a length.
fn positive_mm(mm: f64, field: &'static str) -> Result<Option<i64>, GeomError> {
    if mm <= 0.0 && mm.is_finite() {
        return Ok(None);
    }
    mm_to_nm(mm, field).map(Some)
}
