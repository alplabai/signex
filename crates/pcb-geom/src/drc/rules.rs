//! The rules a run checks against: one board-wide minimum clearance, which
//! a net class can raise for its nets.

use std::collections::BTreeMap;

use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::pcb::PcbBoard;

use crate::units::{GeomError, MAX_ABS_NM, mm_to_nm};

/// The clearance used when the board's setup holds none (0 is "unset").
pub const DEFAULT_CLEARANCE_MM: f64 = 0.2;

/// Minimum clearances in nm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrcRules {
    /// Between any two nets.
    pub clearance: i64,
    /// Per net number, from its net class. A pair of nets needs the largest
    /// of its two nets' values and [`Self::clearance`].
    pub net_clearance: BTreeMap<u32, i64>,
}

impl DrcRules {
    pub fn new(clearance: i64) -> Self {
        DrcRules {
            clearance,
            net_clearance: BTreeMap::new(),
        }
    }

    /// The board's rules: `setup.clearance` (or [`DEFAULT_CLEARANCE_MM`]
    /// when unset), and each net's class clearance where its class sets one.
    /// `classes` is the project's net-class table; the first class that
    /// contains a net wins, as everywhere else.
    pub fn for_board(board: &PcbBoard, classes: &[NetClass]) -> Result<Self, GeomError> {
        let set = board.setup.as_ref().map_or(0.0, |s| s.clearance);
        let clearance = positive_mm(set, "setup.clearance")?
            .map_or_else(|| mm_to_nm(DEFAULT_CLEARANCE_MM, "default clearance"), Ok)?;
        let mut rules = DrcRules::new(clearance);
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

    /// The largest clearance any pair can need.
    pub(super) fn largest(&self) -> i64 {
        let class = self.net_clearance.values().copied().max().unwrap_or(0);
        self.clearance.max(class).min(MAX_ABS_NM)
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
