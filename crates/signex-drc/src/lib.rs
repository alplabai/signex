//! Read-only design-rule check of a board (#2222): clearance, shorts,
//! unconnected nets and board-sketch keepouts (#2234), in integer nm, plus
//! the limits of written rules (#2307, #2435): track width, via size,
//! holes, board edge, via-in-pad, silkscreen, mask, paste and courtyards.
//! Pure: the board goes in, a report comes out, and nothing here edits it.
//! Rules and what is not checked yet: `docs/architecture/pcb-drc.md`.

mod check;
mod checklist;
mod courtyard;
mod edge;
mod explain;
mod fab_floor;
pub mod findings;
mod flags;
mod holes;
mod keepout;
pub mod layout_plan;
mod limit_kind;
mod limits;
mod mask;
mod near;
mod profile;
pub mod rooms;
mod rule_notes;
mod rules;
mod severity;
mod silk;
mod user_rules;
mod waiver;

use uuid::Uuid;

use pcb_geom::{GeomError, ItemRef, PointNm, Rejected};
use signex_types::violation::Severity;

pub use check::run;
pub use checklist::RuleResult;
pub use explain::{Candidate, Effective, Explanation, Source, Verdict};
pub use fab_floor::{FAB_RULE_PREFIX, is_fab_rule};
pub use flags::{FlaggedRule, RuleFlag};
pub use profile::{DrcRuleChange, DrcRuleSection, adopt_profile_drc, diff_drc_against_profile};
pub use rules::{DEFAULT_CLEARANCE_MM, DrcRules};
pub use severity::{
    DrcSeverities, WrittenRuleEdit, follow_written_rule_edit, resolve_severities,
    resolve_severities_with, written_rule_override,
};
use user_rules::USER_RULE_ID_PREFIX;
pub use user_rules::{CopperObject, Limit, LimitKind, NetScope, UserRule};
pub use waiver::{WaiverState, fingerprint, rule_id, waiver_json, waiver_key, waiver_state};

/// What a violation breaks. Declared most severe first: a report sorts by
/// kind, so a capped list of rows keeps the shorts. A new kind goes after
/// the existing ones, so their rows keep their order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ViolationKind {
    /// Copper of two nets that touches or overlaps.
    Short,
    /// Copper of two nets, on a shared layer, closer than the rule.
    Clearance,
    /// One missing link of a net the ratsnest still shows open.
    Unconnected,
    /// A track, via, pad or footprint inside a board-sketch keepout that
    /// forbids it, on a layer the keepout covers (#2234).
    Keepout,
    /// A track narrower than a rule's minimum or wider than its maximum
    /// (`required_nm` is the limit broken).
    TrackWidth,
    /// A via whose copper diameter is under a rule's minimum.
    ViaDiameter,
    /// A via whose drill is under a rule's minimum.
    ViaDrill,
    /// A pad or via hole smaller than a rule's minimum or larger than its
    /// maximum (#2435).
    HoleSize,
    /// The copper ring around a via or plated pad hole narrower than a
    /// rule's minimum.
    AnnularRing,
    /// Two holes closer, edge to edge, than a rule's minimum.
    HoleToHole,
    /// Copper closer to the board outline than a rule's minimum.
    EdgeClearance,
    /// A via in, or closer than a rule's minimum to, a surface-mount pad.
    ViaInPad,
    /// Silkscreen over, or closer than a rule's minimum to, an exposed pad.
    SilkToPad,
    /// Silkscreen of two parts (or a part and a board drawing) closer than
    /// a rule's minimum.
    SilkToSilk,
    /// A strip of solder mask between two openings narrower than a rule's
    /// minimum.
    MaskSliver,
    /// A manual solder mask expansion outside a rule's range.
    MaskExpansion,
    /// A manual paste expansion outside a rule's range.
    PasteExpansion,
    /// The courtyards of two parts on one side overlapping or closer than
    /// a rule's minimum.
    CourtyardOverlap,
    /// A member part of a room with Keep parts inside whose body is
    /// outside the room's outline shrunk by its margin (#2457):
    /// `measured_nm` is how far, `required_nm` the margin.
    PartOutsideRoom,
}

/// One finding, sorted by `(kind, items, ..)` in a report.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Violation {
    pub kind: ViolationKind,
    /// The items involved, sorted: two for a clearance or short; for an
    /// unconnected link the pads at its ends (fewer when an end holds none);
    /// the one item for a keepout.
    pub items: Vec<ItemRef>,
    /// The two nets; the same net twice for an unconnected link or a
    /// keepout (0 for a footprint).
    pub nets: (u32, u32),
    /// Copper layers both items reach, as a bit mask (see
    /// [`copper_layers`](pcb_geom::copper_layers)); 0 for an unconnected link.
    /// For a keepout, the layers the item and the keepout share.
    pub layers: u32,
    /// Where it is: the midpoint of the closest points, the centre of the
    /// overlap, or the middle of the missing link.
    pub at: PointNm,
    /// The gap found, in nm: 0 for a short, the length of the missing link
    /// for an unconnected one. Under `required_nm` for a clearance.
    pub measured_nm: i64,
    /// The clearance the rules ask for between these nets; 0 for an
    /// unconnected link or a keepout.
    pub required_nm: i64,
    /// The keepout broken, for a [`ViolationKind::Keepout`].
    pub keepout: Option<Uuid>,
    /// The written rule (`user::<name>`) that set `required_nm`; `None`
    /// when the board setup or a net class did.
    pub rule: Option<String>,
}

/// The result of one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrcReport {
    /// The rules the run used, so the report says what it checked against.
    pub rules: DrcRules,
    pub violations: Vec<Violation>,
    /// Items left unchecked because their geometry did not convert; a
    /// footprint body a `no_components` keepout needed is one (#2300).
    pub skipped: Vec<Rejected>,
    /// Keepouts left unchecked because their boundary did not convert.
    pub skipped_keepouts: Vec<SkippedKeepout>,
}

/// A keepout whose boundary did not convert, so nothing was checked
/// against it (#2300).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkippedKeepout {
    pub keepout: Uuid,
    pub error: GeomError,
}

impl DrcReport {
    pub fn count(&self, kind: ViolationKind) -> usize {
        self.violations.iter().filter(|v| v.kind == kind).count()
    }

    pub fn is_clean(&self) -> bool {
        self.violations.is_empty() && self.skipped.is_empty() && self.skipped_keepouts.is_empty()
    }

    /// The resolved severity of `violation`'s rule; `None` when switched off.
    /// A violation of a written rule takes that rule's severity.
    pub fn severity_of(&self, violation: &Violation) -> Option<Severity> {
        match &violation.rule {
            Some(id) => self.rules.severities.of_rule(id),
            None => self.rules.severities.of(violation.kind),
        }
    }
}

#[cfg(test)]
mod base_rules_tests;
#[cfg(test)]
mod class_pair_tests;
#[cfg(test)]
mod drawn_copper_tests;
#[cfg(test)]
mod explain_tests;
#[cfg(test)]
mod findings_tests;
#[cfg(test)]
mod keepout_tests;
#[cfg(test)]
mod no_net_copper_tests;
#[cfg(test)]
mod profile_tests;
#[cfg(test)]
mod severity_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod user_rules_tests;
