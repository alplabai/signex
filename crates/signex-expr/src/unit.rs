//! `Quantity` (a value with a unit) and the literal parser.
//!
//! Cleanroom implementation. Conversion factors are common physical
//! constants:
//! - 1 mil = 0.0254 mm  (= 1 thou; SI-defined inch / 1000)
//! - 1 in  = 25.4   mm  (international inch, ISO 31-1)
//! - 1 µm  = 0.001  mm
//! - 1 deg = π/180  rad
//!
//! Lengths, angles and plain numbers keep the unit they were written in
//! (`100mil` stays mils until an operator converts it). Every other unit
//! is [`Unit::Derived`]: a [`Dimension`] with the value in base units
//! (mm, rad, s, V, A).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::dimension::Dimension;
use crate::symbols::{
    LiteralContext, apply_suffix, is_unit_char, prefix_factor, refuses_prefix_only,
};

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
    /// Any other dimension (Ω, F, Hz, mm², V/s, ...). The value is in base
    /// units. Never a bare length, angle or plain number:
    /// [`Quantity::from_base`] maps those to `Mm`, `Rad` and
    /// `Dimensionless`.
    Derived(Dimension),
}

impl Unit {
    /// Family this unit belongs to. Conversions between different
    /// families return [`UnitError::WrongFamily`].
    pub fn family(self) -> UnitFamily {
        match self {
            Unit::Mm | Unit::Mil | Unit::In | Unit::Um => UnitFamily::Length,
            Unit::Deg | Unit::Rad => UnitFamily::Angle,
            Unit::Dimensionless => UnitFamily::Count,
            Unit::Derived(_) => UnitFamily::Derived,
        }
    }
}

/// Group of compatible units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnitFamily {
    Length,
    Angle,
    Count,
    /// Every other dimension. Two derived quantities are compatible only
    /// when their dimensions are equal.
    Derived,
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

    /// A quantity from a value in base units and its dimension. A bare
    /// length, angle or plain number gets its own unit (mm, rad, none).
    pub fn from_base(value: f64, dim: Dimension) -> Self {
        if dim == Dimension::NONE {
            Self::count(value)
        } else if dim == Dimension::LENGTH {
            Self::length(value)
        } else if dim == Dimension::ANGLE {
            Self::angle(value)
        } else {
            Self {
                value,
                unit: Unit::Derived(dim),
            }
        }
    }

    /// The value in base units (mm, rad, s, V, A) and its dimension.
    pub fn to_base(self) -> (f64, Dimension) {
        match self.unit {
            Unit::Mm => (self.value, Dimension::LENGTH),
            Unit::Mil => (self.value * 0.0254, Dimension::LENGTH),
            Unit::In => (self.value * 25.4, Dimension::LENGTH),
            Unit::Um => (self.value * 0.001, Dimension::LENGTH),
            Unit::Deg => (self.value * std::f64::consts::PI / 180.0, Dimension::ANGLE),
            Unit::Rad => (self.value, Dimension::ANGLE),
            Unit::Dimensionless => (self.value, Dimension::NONE),
            Unit::Derived(dim) => (self.value, dim),
        }
    }

    /// The dimension of this quantity.
    pub fn dimension(self) -> Dimension {
        self.to_base().1
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

/// Parse `num` as a finite `f64`. `f64::from_str` accepts `NaN` and `inf`
/// and overflows `1e999` to infinity; none of those is a usable value.
fn parse_finite(num: &str, whole: &str) -> Result<f64, UnitError> {
    num.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| UnitError::Parse(whole.to_string()))
}

/// Read RKM notation, where a prefix letter stands for the decimal point:
/// `4k7` is `4.7k`, `2n2` is `2.2n` (maintainer decision 2026-11-18). Only
/// the prefix letters of [`crate::symbols`] count; `R` is not read.
fn parse_rkm(num: &str) -> Option<f64> {
    let (at, marker) = num.char_indices().find(|(_, c)| !c.is_ascii_digit())?;
    let whole = &num[..at];
    let rest = &num[at + marker.len_utf8()..];
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(whole) || !digits(rest) {
        return None;
    }
    let factor = prefix_factor(&marker.to_string())?;
    let value: f64 = format!("{whole}.{rest}").parse().ok()?;
    Some(value * factor)
}

/// Split `s` into its number and its trailing unit suffix.
fn split_suffix(s: &str) -> (&str, &str) {
    let at = s
        .char_indices()
        .rev()
        .take_while(|&(_, c)| is_unit_char(c))
        .last()
        .map_or(s.len(), |(i, _)| i);
    s.split_at(at)
}

/// Parse a string like `"0.5mm"`, `"100 mil"`, `"90deg"`, `"16"`,
/// `"100nF"`, `"4.7k"` or `"4k7"` in the [`LiteralContext::General`]
/// context.
///
/// Accepts whitespace inside and around the input. A bare number with
/// no suffix is interpreted as [`Unit::Dimensionless`]. Unknown
/// suffixes return [`UnitError::Parse`].
pub fn parse_quantity(s: &str) -> Result<Quantity, UnitError> {
    parse_quantity_with(s, LiteralContext::General)
}

/// [`parse_quantity`] in an explicit context: a bare `m` reads differently
/// (see [`crate::symbols::bare_m_meaning`]), and
/// every context but [`LiteralContext::General`] refuses prefix-only and RKM
/// literals.
pub fn parse_quantity_with(s: &str, context: LiteralContext) -> Result<Quantity, UnitError> {
    let trimmed = s.trim();
    let (num_part, suffix) = split_suffix(trimmed);
    let num = num_part.trim();
    // The numeric part must be non-empty; otherwise something like "mm"
    // alone would parse as 0.0 mm.
    if num.is_empty() {
        return Err(UnitError::Parse(s.to_string()));
    }
    let value = match parse_rkm(num) {
        Some(_) if refuses_prefix_only(context) => return Err(UnitError::Parse(s.to_string())),
        Some(v) => v,
        None => parse_finite(num, s)?,
    };
    let q = if suffix.is_empty() {
        Quantity::count(value)
    } else {
        apply_suffix(value, suffix, context).ok_or_else(|| UnitError::Parse(s.to_string()))?
    };
    if q.value.is_finite() {
        Ok(q)
    } else {
        Err(UnitError::Parse(s.to_string()))
    }
}
