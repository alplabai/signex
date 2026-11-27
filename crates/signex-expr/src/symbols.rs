//! The unit table: SI prefixes, named units and the reading of a literal's
//! suffix (`0.5mm`, `100nF`, `4.7k`, `4k7`).
//!
//! Lengths and angles keep their own [`Unit`] variants so a sketch value
//! keeps the unit it was written in. Every other unit is a symbol for a
//! [`Dimension`] with an optional SI prefix; its value is stored in base
//! units (s, V, A).

use crate::dimension::Dimension;
use crate::unit::{Quantity, Unit};

/// Where a literal is read. The meaning of a bare `m` depends on it (see
/// [`bare_m_meaning`]), and every context but [`LiteralContext::General`]
/// refuses a literal whose suffix is only an SI prefix (a bare `m` aside).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LiteralContext {
    /// Any value: a field, a variable, a BOM column.
    #[default]
    General,
    /// A value that must be a length (a pad width, a clearance). A bare
    /// `m` is metres; any other prefix-only or RKM literal (`4k7`, `1k`,
    /// `1u`) is refused (Caner 2026-11-27).
    Length,
    /// A sketch parameter: a length, an angle or a count that drives
    /// geometry. A prefix-only literal (`10m`, `1u`, `4k7`, `2n`) has no
    /// single reading there, so it is refused rather than baked.
    Geometry,
}

/// What a bare `m` after a number means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BareM {
    /// Metre: `10m` is 10 000 mm.
    Metre,
    /// The milli prefix on a plain number: `10m` is 0.01.
    Milli,
    /// No meaning: the literal is refused.
    Refused,
}

/// The one rule for a bare `m` after a number (maintainer decision
/// 2026-11-18, #1984, `docs/DECISIONS.md`): metre where a length is
/// expected, milli where any value is, refused in a sketch parameter, an
/// angle or a count. An
/// `m` followed by another unit symbol (`10mV`, `4.7mF`) is always milli
/// and never reaches this function.
pub fn bare_m_meaning(context: LiteralContext) -> BareM {
    match context {
        LiteralContext::Length => BareM::Metre,
        LiteralContext::General => BareM::Milli,
        LiteralContext::Geometry | LiteralContext::Angle | LiteralContext::Count => BareM::Refused,
    }
}

/// True when `context` refuses a literal whose suffix is only an SI prefix
/// (`1u`, `2n`, `1k`) or that is written in RKM notation (`4k7`). A bare
/// `m` is decided by [`bare_m_meaning`] first.
pub(crate) fn refuses_prefix_only(context: LiteralContext) -> bool {
    context != LiteralContext::General
}

/// Millimetres in one metre.
const MM_PER_M: f64 = 1000.0;

/// SI prefixes a literal may carry, with their factor. `u`, the micro sign
/// (U+00B5) and Greek mu (U+03BC) are all micro.
const PREFIXES: &[(&str, f64)] = &[
    ("p", 1e-12),
    ("n", 1e-9),
    ("u", 1e-6),
    ("\u{00B5}", 1e-6),
    ("\u{03BC}", 1e-6),
    ("m", 1e-3),
    ("k", 1e3),
    ("M", 1e6),
    ("G", 1e9),
];

/// Length and angle suffixes, matched exactly and never prefixed (`mil`
/// is not milli-`il`, `mm` is the millimetre).
const SKETCH_UNITS: &[(&str, Unit)] = &[
    ("mil", Unit::Mil),
    ("mm", Unit::Mm),
    ("um", Unit::Um),
    ("\u{00B5}m", Unit::Um),
    ("\u{03BC}m", Unit::Um),
    ("deg", Unit::Deg),
    ("rad", Unit::Rad),
    ("in", Unit::In),
];

/// Named units that take an SI prefix, each a symbol for a dimension.
/// Greek capital omega (U+03A9), the ohm sign (U+2126) and `ohm` are all
/// the ohm.
const NAMED_UNITS: &[(&str, Dimension)] = &[
    ("V", Dimension::VOLTAGE),
    ("A", Dimension::CURRENT),
    ("s", Dimension::TIME),
    ("Hz", Dimension::FREQUENCY),
    ("\u{03A9}", Dimension::RESISTANCE),
    ("\u{2126}", Dimension::RESISTANCE),
    ("ohm", Dimension::RESISTANCE),
    ("F", Dimension::CAPACITANCE),
    ("H", Dimension::INDUCTANCE),
    ("W", Dimension::POWER),
    ("C", Dimension::CHARGE),
];

/// Symbol a result of each named dimension is shown in, in lookup order.
const DISPLAY_UNITS: &[(Dimension, &str)] = &[
    (Dimension::RESISTANCE, "Ω"),
    (Dimension::POWER, "W"),
    (Dimension::CAPACITANCE, "F"),
    (Dimension::INDUCTANCE, "H"),
    (Dimension::CHARGE, "C"),
    (Dimension::FREQUENCY, "Hz"),
    (Dimension::VOLTAGE, "V"),
    (Dimension::CURRENT, "A"),
    (Dimension::TIME, "s"),
];

/// Display prefixes by power of ten, for [`crate::format`].
pub(crate) const DISPLAY_PREFIXES: &[(i32, &str)] = &[
    (-12, "p"),
    (-9, "n"),
    (-6, "µ"),
    (-3, "m"),
    (0, ""),
    (3, "k"),
    (6, "M"),
    (9, "G"),
];

/// The factor of an SI prefix symbol.
pub(crate) fn prefix_factor(symbol: &str) -> Option<f64> {
    PREFIXES.iter().find(|(s, _)| *s == symbol).map(|&(_, f)| f)
}

/// True for a character that can be part of a unit suffix.
pub(crate) fn is_unit_char(c: char) -> bool {
    c.is_ascii_alphabetic() || matches!(c, '\u{00B5}' | '\u{03BC}' | '\u{03A9}' | '\u{2126}')
}

fn named_unit(symbol: &str) -> Option<Dimension> {
    NAMED_UNITS
        .iter()
        .find(|(s, _)| *s == symbol)
        .map(|&(_, d)| d)
}

/// The symbol a value of dimension `dim` is shown in, if it has a name.
pub(crate) fn display_symbol(dim: Dimension) -> Option<&'static str> {
    DISPLAY_UNITS
        .iter()
        .find(|(d, _)| *d == dim)
        .map(|&(_, s)| s)
}

/// `value` with the unit `suffix` attached, or `None` for an unknown
/// suffix. `suffix` is non-empty.
pub(crate) fn apply_suffix(value: f64, suffix: &str, context: LiteralContext) -> Option<Quantity> {
    if let Some(&(_, unit)) = SKETCH_UNITS.iter().find(|(s, _)| *s == suffix) {
        return Some(Quantity { value, unit });
    }
    if suffix == "m" {
        return match bare_m_meaning(context) {
            BareM::Metre => Some(Quantity::length(value * MM_PER_M)),
            BareM::Milli => Some(Quantity::count(value * 1e-3)),
            BareM::Refused => None,
        };
    }
    if let Some(factor) = prefix_factor(suffix) {
        if refuses_prefix_only(context) {
            return None;
        }
        return Some(Quantity::count(value * factor));
    }
    if let Some(dim) = named_unit(suffix) {
        return Some(Quantity::from_base(value, dim));
    }
    PREFIXES.iter().find_map(|&(prefix, factor)| {
        let dim = named_unit(suffix.strip_prefix(prefix)?)?;
        Some(Quantity::from_base(value * factor, dim))
    })
}
