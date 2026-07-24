use std::fmt;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Unit enum
// ---------------------------------------------------------------------------
//
// The live in-memory model stores schematic/PCB positions as `f64` mm
// (`schematic::Point`). Integer nanometres exist only at the on-disk wire
// format boundary (`format/units.rs`), not as a workspace-wide coordinate
// type — this module used to also carry an `i64`-nm `Coord`/`Vec2` pair for
// that purpose, but nothing outside this module ever consumed them; deleted
// (#394).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Mm,
    Mil,
    Inch,
    Micrometer,
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unit::Mm => write!(f, "mm"),
            Unit::Mil => write!(f, "mil"),
            Unit::Inch => write!(f, "in"),
            Unit::Micrometer => write!(f, "um"),
        }
    }
}
