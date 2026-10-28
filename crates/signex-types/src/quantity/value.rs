//! The structured quantity an editor holds: number + prefix + unit.

use std::fmt;

use super::format::{format_eng, pick_prefix};
use super::unit::{SiPrefix, Unit};

/// `mantissa × prefix` of a `unit`, as shown in the number field and the
/// two pickers. Maps onto a stored `Measurement { value: mantissa, unit:
/// "<prefix><unit>" }`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity {
    pub mantissa: f64,
    pub prefix: SiPrefix,
    pub unit: Unit,
}

impl Quantity {
    /// The value in the unit without prefix (`4.7 kΩ` -> `4700.0`).
    pub fn to_base(self) -> f64 {
        self.prefix.scale(self.mantissa)
    }

    /// The quantity for `base`, with the prefix that keeps the mantissa in
    /// `[1, 1000)` (clamped at `p`..`G`). The mantissa is exact, not
    /// rounded; [`format_eng`] does the display rounding.
    pub fn from_base_eng(base: f64, unit: Unit) -> Quantity {
        let prefix = pick_prefix(base, unit);
        Quantity {
            mantissa: prefix.unscale(base),
            prefix,
            unit,
        }
    }

    /// The unit string as stored in `Measurement.unit` (`"kΩ"`).
    pub fn unit_string(self) -> String {
        format!("{}{}", self.prefix.symbol(), self.unit.symbol())
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&format_eng(self.to_base(), self.unit))
    }
}
