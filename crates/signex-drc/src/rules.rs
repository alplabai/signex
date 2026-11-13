//! The rules a run checks against: one board-wide minimum clearance, which
//! a net class can raise for its nets (the implicit rules), and the rules
//! written in the rule language, which rank above them.

use std::collections::BTreeMap;

use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::pcb::PcbBoard;
use signex_types::violation::Severity;

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
    /// What the rule-language text said that this run does not apply (a
    /// rule that did not compile, names something not checked yet, or a
    /// net class that does not exist) and the net classes a written rule
    /// lowers. Written to Messages as warnings with the report.
    pub notes: Vec<String>,
    /// Written to Messages as info: a rule for a net or pattern no net of
    /// this board has (a shared profile may name nets a board lacks).
    pub remarks: Vec<String>,
    /// Written rules not applied (invalid, repeated or not checked yet), and
    /// rule texts with a syntax error (none of their rules applied): shown
    /// beside the results, so a typo never passes as a clean board.
    pub rules_not_applied: usize,
    pub texts_not_applied: usize,
    /// Rule-language rules, resolved against the board's nets.
    pub(crate) user: Vec<ScopedRule>,
    /// The board's copper layers, set with the written rules: a through
    /// item's mask spans every layer, and a written rule is resolved only
    /// on the layers the board has.
    pub(crate) copper: u32,
}

impl DrcRules {
    pub fn new(clearance: i64) -> Self {
        DrcRules {
            clearance,
            net_clearance: BTreeMap::new(),
            from_default: false,
            severities: DrcSeverities::builtin(),
            notes: Vec::new(),
            remarks: Vec::new(),
            rules_not_applied: 0,
            texts_not_applied: 0,
            user: Vec::new(),
            copper: u32::MAX,
        }
    }

    /// `layers` clipped to the board's copper; as given when they share
    /// none (an item on a layer the stackup does not list).
    pub(crate) fn on_board(&self, layers: u32) -> u32 {
        match layers & self.copper {
            0 => layers,
            clipped => clipped,
        }
    }

    /// Add rule-language rules, resolving their net scopes against
    /// `board`'s nets and `classes` (the first matching class, as for the
    /// implicit rules). A rule whose scope matches no net, and a written
    /// clearance that wins for a net below what the net would otherwise
    /// need, are said ([`Self::notes`], [`Self::remarks`]), never silent.
    pub fn with_user_rules(
        mut self,
        board: &PcbBoard,
        classes: &[NetClass],
        rules: impl IntoIterator<Item = UserRule>,
    ) -> Self {
        let first = self.user.len();
        self.user.extend(
            rules
                .into_iter()
                .map(|rule| ScopedRule::resolve(rule, board, classes)),
        );
        self.copper = crate::rule_notes::board_copper(board);
        // A caller that never resolves the layers still gets each rule's
        // own severity from the report.
        for scoped in &self.user[first..] {
            self.severities.seed_written(&scoped.rule);
        }
        let (warnings, infos) = crate::rule_notes::empty_scopes(&self.user[first..], classes);
        self.notes.extend(warnings);
        self.remarks.extend(infos);
        let lowered = crate::rule_notes::lowered_classes(&self, board, classes);
        self.notes.extend(lowered);
        self
    }

    /// Whether a written rule sets a clearance (the default-clearance note
    /// and the summary then say the board value holds only where none does).
    pub fn sets_written_clearance(&self) -> bool {
        self.sets(LimitKind::MinClearance)
    }

    /// The rule-language rules this run applies, in written order.
    pub fn user_rules(&self) -> impl Iterator<Item = &UserRule> {
        self.user.iter().map(|s| &s.rule)
    }

    /// The clearance two copper items of different nets need where they
    /// share `layers`, and the written rule that set it. Each side is
    /// resolved for its own net: per shared layer the most specific written
    /// rule covering that net applies, the strictest among equals, else the
    /// net's implicit value. The pair needs the stricter side and the most
    /// of its layers, as [`Self::required`] does without written rules.
    pub(crate) fn required_between(
        &self,
        a: &CopperItem,
        b: &CopperItem,
        layers: u32,
    ) -> (i64, Option<String>) {
        if !user_rules::sets(&self.user, LimitKind::MinClearance) {
            return (self.required(a.net, b.net), None);
        }
        // A fixed side order, so a tie names the same rule whichever item
        // found the pair.
        let (first, second) = if a.item <= b.item { (a, b) } else { (b, a) };
        let objects = [
            CopperObject::of(&first.item),
            CopperObject::of(&second.item),
        ];
        let mut most: (i64, Option<&ScopedRule>) = (i64::MIN, None);
        let layers = self.on_board(layers);
        for bit in (0..32).filter(|bit| layers & (1 << bit) != 0) {
            for net in [first.net, second.net] {
                let side =
                    user_rules::pick(&self.user, LimitKind::MinClearance, net, 1 << bit, objects)
                        .map_or((self.required(net, net), None), |(nm, rule)| {
                            (nm, Some(rule))
                        });
                if side.0 > most.0 || (side.0 == most.0 && self.wins_tie(side.1, most.1)) {
                    most = side;
                }
            }
        }
        (most.0, most.1.map(|s| s.rule.id()))
    }

    /// Between two sides asking the same clearance: the more severe one
    /// wins, so a milder rule never downgrades the other side's finding;
    /// at equal severity a written rule is named over the implicit value.
    fn wins_tie(&self, side: Option<&ScopedRule>, held: Option<&ScopedRule>) -> bool {
        let rank = |s: Option<&ScopedRule>| {
            let severity = match s {
                Some(scoped) => self.severities.of_written(&scoped.rule),
                None => self.severities.of(crate::ViolationKind::Clearance),
            };
            match severity {
                Some(Severity::Error) => 3,
                Some(Severity::Warning) => 2,
                Some(Severity::Info) => 1,
                None => 0,
            }
        };
        let (side_rank, held_rank) = (rank(side), rank(held));
        side_rank > held_rank || (side_rank == held_rank && held.is_none() && side.is_some())
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
    /// class asks for more are found from that other item, and so are pairs
    /// where a written rule covering the other net does; the widest written
    /// rule covering `net` counts here.
    pub(super) fn reach_of(&self, net: u32) -> i64 {
        let widest = user_rules::widest_clearance(&self.user, net);
        self.required(net, net).max(widest).min(MAX_ABS_NM)
    }

    /// The limit of `kind` the written rules set for one object on
    /// `layers`, and the rule's id. Resolved per copper layer of the board
    /// the object reaches; the strictest layer's limit holds, so a rule
    /// for some layers never lifts another rule off the rest.
    pub(crate) fn object_limit(
        &self,
        kind: LimitKind,
        net: u32,
        layers: u32,
    ) -> Option<(i64, String)> {
        let layers = self.on_board(layers);
        let mut held: Option<(i64, &ScopedRule)> = None;
        for bit in (0..32).filter(|bit| layers & (1 << bit) != 0) {
            let Some((nm, scoped)) =
                user_rules::pick(&self.user, kind, net, 1 << bit, [None, None])
            else {
                continue;
            };
            if held.is_none_or(|(h, _)| kind.stricter(nm, h)) {
                held = Some((nm, scoped));
            }
        }
        held.map(|(nm, scoped)| (nm, scoped.rule.id()))
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
