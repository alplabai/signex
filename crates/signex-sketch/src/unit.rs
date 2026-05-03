//! Strict-unit parser and `Quantity` type for sketch expressions.
//!
//! Cleanroom implementation. No third-party constraint-solver source
//! consulted. Conversion factors are common physical constants:
//! - 1 mil = 0.0254 mm  (= 1 thou; SI-defined inch / 1000)
//! - 1 in  = 25.4   mm  (international inch, ISO 31-1)
//! - 1 µm  = 0.001  mm
//! - 1 deg = π/180  rad
//!
//! See `docs/internal/SKETCH_MODE_v0.13_PLAN.md` Task 4.1.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A unit attached to a [`Quantity`].
///
/// Units are partitioned into [`UnitFamily`]s; conversions are
/// only legal within a family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Unit {
    /// Millimetre (canonical length unit).
    Mm,
    /// Mil = 1/1000 inch = 0.0254 mm.
    Mil,
    /// International inch = 25.4 mm exactly.
    In,
    /// Micrometre = 0.001 mm.
    Um,
    /// Degree = π/180 rad.
    Deg,
    /// Radian (canonical angle unit).
    Rad,
    /// Pure number; carries no unit.
    Dimensionless,
}

impl Unit {
    /// Family this unit belongs to. Conversions between different
    /// families return [`UnitError::WrongFamily`].
    pub fn family(self) -> UnitFamily {
        match self {
            Unit::Mm | Unit::Mil | Unit::In | Unit::Um => UnitFamily::Length,
            Unit::Deg | Unit::Rad => UnitFamily::Angle,
            Unit::Dimensionless => UnitFamily::Count,
        }
    }
}

/// Group of compatible units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnitFamily {
    Length,
    Angle,
    Count,
}

/// A scalar value paired with a [`Unit`].
///
/// `Quantity` is `Copy + Serialize + Deserialize` so it can sit
/// inside an `ExprNode::Literal` without further indirection.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    pub value: f64,
    pub unit: Unit,
}

impl Quantity {
    /// Construct a length quantity in millimetres.
    pub fn length(mm: f64) -> Self {
        Self {
            value: mm,
            unit: Unit::Mm,
        }
    }

    /// Construct an angle quantity in radians.
    pub fn angle(rad: f64) -> Self {
        Self {
            value: rad,
            unit: Unit::Rad,
        }
    }

    /// Construct a dimensionless count.
    pub fn count(n: f64) -> Self {
        Self {
            value: n,
            unit: Unit::Dimensionless,
        }
    }

    /// Convert to millimetres. Errors with [`UnitError::WrongFamily`]
    /// for non-length units.
    pub fn as_mm(self) -> Result<f64, UnitError> {
        match self.unit {
            Unit::Mm => Ok(self.value),
            Unit::Mil => Ok(self.value * 0.0254),
            Unit::In => Ok(self.value * 25.4),
            Unit::Um => Ok(self.value * 0.001),
            other => Err(UnitError::WrongFamily {
                expected: UnitFamily::Length,
                got: other.family(),
            }),
        }
    }

    /// Convert to radians. Errors with [`UnitError::WrongFamily`]
    /// for non-angle units.
    pub fn as_rad(self) -> Result<f64, UnitError> {
        match self.unit {
            Unit::Rad => Ok(self.value),
            Unit::Deg => Ok(self.value * std::f64::consts::PI / 180.0),
            other => Err(UnitError::WrongFamily {
                expected: UnitFamily::Angle,
                got: other.family(),
            }),
        }
    }

    /// Return the raw scalar if this quantity is dimensionless.
    /// Errors with [`UnitError::WrongFamily`] otherwise.
    pub fn as_count(self) -> Result<f64, UnitError> {
        match self.unit {
            Unit::Dimensionless => Ok(self.value),
            other => Err(UnitError::WrongFamily {
                expected: UnitFamily::Count,
                got: other.family(),
            }),
        }
    }
}

/// Errors produced by the unit parser and [`Quantity`] conversions.
#[derive(Debug, Error)]
pub enum UnitError {
    #[error("could not parse quantity '{0}'")]
    Parse(String),
    #[error("unit family mismatch: expected {expected:?}, got {got:?}")]
    WrongFamily {
        expected: UnitFamily,
        got: UnitFamily,
    },
    #[error("incompatible units in expression: {0:?} and {1:?}")]
    Incompatible(Unit, Unit),
}

/// Recognised unit suffixes. Order matters: longer / more-specific
/// suffixes come first so that `"mil"` is not split into `"m"` + `"il"`
/// or `"mi"` + `"l"`. `"in"` is last because it is the shortest and
/// could be a sub-string of longer suffixes if any are added later.
const SUFFIXES: &[(&str, Unit)] = &[
    ("mil", Unit::Mil),
    ("mm", Unit::Mm),
    ("um", Unit::Um),
    ("deg", Unit::Deg),
    ("rad", Unit::Rad),
    ("in", Unit::In),
];

/// Parse a string like `"0.5mm"`, `"100 mil"`, `"90deg"`, or `"16"`.
///
/// Accepts whitespace inside and around the input. A bare number with
/// no suffix is interpreted as [`Unit::Dimensionless`]. Unknown
/// suffixes return [`UnitError::Parse`].
pub fn parse_quantity(s: &str) -> Result<Quantity, UnitError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(UnitError::Parse(s.to_string()));
    }

    // Try each known suffix, longest / most-specific first.
    for &(suf, unit) in SUFFIXES {
        if let Some(num_part) = trimmed.strip_suffix(suf) {
            let num = num_part.trim();
            // The numeric part must be non-empty; otherwise something
            // like "mm" alone would parse as 0.0 mm.
            if num.is_empty() {
                return Err(UnitError::Parse(s.to_string()));
            }
            let value: f64 = num
                .parse()
                .map_err(|_| UnitError::Parse(s.to_string()))?;
            return Ok(Quantity { value, unit });
        }
    }

    // No suffix matched -> dimensionless number.
    let value: f64 = trimmed
        .parse()
        .map_err(|_| UnitError::Parse(s.to_string()))?;
    Ok(Quantity {
        value,
        unit: Unit::Dimensionless,
    })
}
