//! The value an editor holds: a base-unit number and a unit.

use std::fmt;
use std::hash::{Hash, Hasher};

use super::format::{ValueStyle, format_quantity, layout};
use super::prefix::SiPrefix;
use super::unit::Unit;

/// A finite base-unit value (`4700.0` for 4.7 kΩ) plus its [`Unit`].
/// `-0.0` is normalised to `+0.0` so equal quantities hash and group
/// alike.
#[derive(Clone, Debug)]
pub struct Quantity {
    base: f64,
    unit: Unit,
}

impl Quantity {
    /// `None` if `base` is NaN or infinite.
    pub fn new(base: f64, unit: Unit) -> Option<Quantity> {
        if !base.is_finite() {
            return None;
        }
        Some(Quantity {
            base: if base == 0.0 { 0.0 } else { base },
            unit,
        })
    }

    pub fn base(&self) -> f64 {
        self.base
    }

    pub fn unit(&self) -> &Unit {
        &self.unit
    }

    /// Bit pattern of the (normalised) base value: the key for grouping
    /// and hashing alongside the unit.
    pub fn to_bits(&self) -> u64 {
        self.base.to_bits()
    }

    /// Mantissa text and prefix the default (engineering) display uses,
    /// for editors that show them in a number field and a picker. The
    /// prefix is always one of the unit's allowed prefixes; outside their
    /// range the mantissa carries an exponent (`1.2e-16`) with
    /// [`SiPrefix::One`].
    pub fn eng_parts(&self) -> (String, SiPrefix) {
        let l = layout(self.base, &self.unit, 15);
        let sign = if l.negative { "-" } else { "" };
        (format!("{sign}{}", l.text), l.prefix)
    }

    /// The number the default display shows, without prefix and unit:
    /// `4.7` for 4.7 kΩ, `100` for 100 nF. "Any prefix" comparisons read
    /// this (mockup row 52).
    pub fn displayed_number(&self) -> f64 {
        // The mantissa text is a valid number by construction (see
        // `eng_parts`); the fallback is unreachable in practice.
        self.eng_parts().0.parse().unwrap_or(self.base)
    }

    /// The number this value shows in `prefix` (`0.1` for 100 nF in µ),
    /// rounded to 12 significant digits so float noise from the scaling
    /// never decides a comparison.
    pub fn number_in(&self, prefix: SiPrefix) -> f64 {
        let scaled = self.base / 10f64.powi(prefix.exponent());
        format!("{scaled:.11e}").parse().unwrap_or(scaled)
    }

    /// Render in `style`; `plus_minus` prefixes `±` (`+/-` in ASCII) for
    /// template slots that set `display_plus_minus`. Up to 15 significant
    /// digits; see [`format_quantity_sig`](super::format_quantity_sig) to
    /// cap them.
    pub fn format(&self, style: ValueStyle, plus_minus: bool) -> String {
        format_quantity(self, style, plus_minus)
    }
}

impl PartialEq for Quantity {
    fn eq(&self, other: &Self) -> bool {
        self.to_bits() == other.to_bits() && self.unit == other.unit
    }
}

impl Eq for Quantity {}

impl Hash for Quantity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_bits().hash(state);
        self.unit.hash(state);
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&format_quantity(self, ValueStyle::Engineering, false))
    }
}
