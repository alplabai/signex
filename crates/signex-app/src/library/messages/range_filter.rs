//! Messages of the range filter on a measurement column (#1572, phase 4).
//! One sub-enum shared by the library browser and the Place Component
//! picker; each embeds it under its own variant.

use signex_library::RangeOp;
use signex_types::quantity::{SiPrefix, Unit};

/// Which end of the range a number field or prefix picker belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeBound {
    Low,
    High,
}

/// Something the user did to a range filter. `key` is the parameter
/// column the filter is on.
#[derive(Debug, Clone, PartialEq)]
pub enum RangeFilterMsg {
    /// A column was picked to filter. `unit` is the column's unit, fixed
    /// for the life of the filter.
    ColumnPicked {
        key: String,
        unit: Unit,
    },
    OperatorPicked {
        key: String,
        op: RangeOp,
    },
    /// The number field of one bound was edited. Only digits, a sign and a
    /// decimal point are accepted; anything else is refused with a reason.
    NumberEdited {
        key: String,
        bound: RangeBound,
        text: String,
    },
    PrefixPicked {
        key: String,
        bound: RangeBound,
        prefix: SiPrefix,
    },
    /// One column's filter was removed.
    Cleared {
        key: String,
    },
    /// The chip of an applied filter was pressed: its editor opens.
    ChipPressed {
        key: String,
    },
    /// The editor of a filter was finished (Done or Enter): a complete
    /// filter folds back into its chip.
    EditingFinished {
        key: String,
    },
}
