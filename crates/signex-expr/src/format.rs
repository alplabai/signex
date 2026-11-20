//! Showing a [`Quantity`]: `330Ω`, `33mW`, `2.54mm`, `4mm²`.
//!
//! A named dimension is shown with an engineering prefix; a length, an
//! angle or a plain number in the unit it carries; any other dimension as
//! a compound of base units. Values are rounded to six significant digits
//! so `3.3V / 10mA` reads `330Ω`, not `329.99999999999994Ω`.

use std::fmt;

use crate::symbols::{DISPLAY_PREFIXES, display_symbol};
use crate::unit::{Quantity, Unit};

/// Significant digits a shown value keeps.
const SIGNIFICANT_DIGITS: i32 = 6;

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suffix = match self.unit {
            Unit::Mm => "mm",
            Unit::Mil => "mil",
            Unit::In => "in",
            Unit::Um => "um",
            Unit::Deg => "deg",
            Unit::Rad => "rad",
            Unit::Dimensionless => "",
            Unit::Derived(dim) => {
                return match display_symbol(dim) {
                    Some(symbol) => f.write_str(&with_prefix(self.value, symbol)),
                    None => write!(f, "{}{}", compact(self.value), dim.compound_symbol()),
                };
            }
        };
        write!(f, "{}{suffix}", compact(self.value))
    }
}

/// `value` rounded to [`SIGNIFICANT_DIGITS`], without trailing zeros.
fn compact(value: f64) -> String {
    if value == 0.0 || !value.is_finite() {
        return format!("{value}");
    }
    let magnitude = value.abs().log10().floor() as i32;
    let decimals = (SIGNIFICANT_DIGITS - 1 - magnitude).max(0) as usize;
    let text = format!("{value:.decimals$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    }
}

/// `value` (in base units) with the engineering prefix that puts its
/// mantissa in `1..1000`, followed by `symbol`.
fn with_prefix(value: f64, symbol: &str) -> String {
    if value == 0.0 || !value.is_finite() {
        return format!("{}{symbol}", compact(value));
    }
    let (lowest, highest) = (
        DISPLAY_PREFIXES[0].0,
        DISPLAY_PREFIXES[DISPLAY_PREFIXES.len() - 1].0,
    );
    let exponent = |v: f64| ((v.abs().log10() / 3.0).floor() as i32 * 3).clamp(lowest, highest);
    let mut power = exponent(value);
    let mut mantissa = compact(value / 10f64.powi(power));
    // Rounding can carry a mantissa up to 1000 (999.9999999 -> "1000").
    let carried = mantissa
        .trim_start_matches('-')
        .parse::<f64>()
        .is_ok_and(|m| m >= 1000.0);
    if carried && power < highest {
        power += 3;
        mantissa = compact(value / 10f64.powi(power));
    }
    let prefix = DISPLAY_PREFIXES
        .iter()
        .find(|(p, _)| *p == power)
        .map_or("", |&(_, s)| s);
    format!("{mantissa}{prefix}{symbol}")
}
