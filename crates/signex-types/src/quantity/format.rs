//! Engineering-notation output: `4.7 kΩ`, `100 nF`, `1 µF`.

use super::unit::{SiPrefix, Unit};

/// Significant digits kept by the formatter.
const SIG_DIGITS: i32 = 4;

/// Mantissa text and prefix for `base` in `unit`, for editors that show
/// them in separate widgets. The mantissa is in `[1, 1000)` when a prefix
/// reaches it (clamped at `p`..`G`), has at most 4 significant digits and
/// no trailing zeros. Zero is `("0", One)`. Units without prefixes
/// (`%`, `ppm`, `°C`) always use [`SiPrefix::One`]. A non-finite `base`
/// comes back as its `f64` text.
pub fn format_parts(base: f64, unit: Unit) -> (String, SiPrefix) {
    if !base.is_finite() {
        return (base.to_string(), SiPrefix::One);
    }
    if base == 0.0 {
        return ("0".to_string(), SiPrefix::One);
    }
    let prefix = pick_prefix(base, unit);
    (fixed(prefix.unscale(base)), prefix)
}

/// The prefix whose mantissa for `base` falls in `[1, 1000)` after
/// display rounding; [`SiPrefix::One`] for zero, non-finite values and
/// units without prefixes.
pub(super) fn pick_prefix(base: f64, unit: Unit) -> SiPrefix {
    if !base.is_finite() || base == 0.0 || !unit.takes_prefix() {
        return SiPrefix::One;
    }
    let all = SiPrefix::ALL;
    let last = all.len() - 1;
    let guess = ((base.abs().log10() / 3.0).floor() as i32 * 3).clamp(-12, 9);
    let mut idx = all.iter().position(|p| p.exponent() == guess).unwrap_or(4);
    while idx > 0 && all[idx].unscale(base).abs() < 1.0 {
        idx -= 1;
    }
    while idx < last && all[idx].unscale(base).abs() >= 1000.0 {
        idx += 1;
    }
    // Rounding can carry 999.96 up to 1000: move to the next prefix.
    let rounded: f64 = fixed(all[idx].unscale(base)).parse().unwrap_or(0.0);
    if idx < last && rounded.abs() >= 1000.0 {
        idx += 1;
    }
    all[idx]
}

/// `base` as `"<mantissa> <prefix><unit>"`: `"4.7 kΩ"`, `"0 Ω"`, `"5 %"`.
pub fn format_eng(base: f64, unit: Unit) -> String {
    let (mantissa, prefix) = format_parts(base, unit);
    format!("{mantissa} {}{}", prefix.symbol(), unit.symbol())
}

/// Round to [`SIG_DIGITS`] significant digits, never dropping integer
/// digits, then trim trailing zeros.
fn fixed(m: f64) -> String {
    let magnitude = m.abs().log10().floor() as i32;
    let decimals = (SIG_DIGITS - 1 - magnitude).clamp(0, 30) as usize;
    let mut text = format!("{m:.decimals$}");
    if text.contains('.') {
        text.truncate(text.trim_end_matches('0').trim_end_matches('.').len());
    }
    if text == "-0" { "0".to_string() } else { text }
}
