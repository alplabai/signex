//! Read-only design-rule check of a board (#2222): clearance, shorts,
//! unconnected nets and board-sketch keepouts (#2234), in integer nm. Pure:
//! the board goes in, a report comes out, and nothing here edits it. Rules
//! and what is not checked yet: `docs/architecture/pcb-drc.md`.

mod check;
mod keepout;
mod rules;
mod severity;

use uuid::Uuid;

use pcb_geom::{ItemRef, PointNm, Rejected};
use signex_types::violation::Severity;

pub use check::run;
pub use rules::{DEFAULT_CLEARANCE_MM, DrcRules};
pub use severity::{DrcSeverities, resolve_severities};

/// What a violation breaks. Declared most severe first: a report sorts by
/// kind, so a capped list of rows keeps the shorts.
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
}

/// The result of one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrcReport {
    /// The rules the run used, so the report says what it checked against.
    pub rules: DrcRules,
    pub violations: Vec<Violation>,
    /// Items left unchecked because their geometry did not convert.
    pub skipped: Vec<Rejected>,
}

impl DrcReport {
    pub fn count(&self, kind: ViolationKind) -> usize {
        self.violations.iter().filter(|v| v.kind == kind).count()
    }

    pub fn is_clean(&self) -> bool {
        self.violations.is_empty() && self.skipped.is_empty()
    }

    /// The resolved severity of `violation`'s rule; `None` when switched off.
    pub fn severity_of(&self, violation: &Violation) -> Option<Severity> {
        self.rules.severities.of(violation.kind)
    }
}

#[cfg(test)]
mod keepout_tests;
#[cfg(test)]
mod severity_tests;
#[cfg(test)]
mod tests;
