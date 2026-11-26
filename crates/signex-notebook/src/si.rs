//! Numbers with SI prefixes, as a bench note writes them: `5 mV`,
//! `20 MHz`, `4.7 µH`, `1 µs`. Read for scope scales and cursors, written
//! for the plot's labels and readouts and the measurement table.

/// The prefixes read and written, largest first. `u` and the Greek mu are
/// read as micro; micro is written `µ` (U+00B5).
const PREFIXES: [(char, i32); 8] = [
    ('G', 9),
    ('M', 6),
    ('k', 3),
    ('m', -3),
    ('µ', -6),
    ('n', -9),
    ('p', -12),
    ('f', -15),
];

/// The thin space written between a number and its unit (U+2009).
pub const THIN_SPACE: char = '\u{2009}';
/// The minus sign written before a negative number (U+2212).
pub const MINUS: char = '\u{2212}';

/// `text` as a number in its base unit and the unit without its prefix:
/// `"5 mV"` is `(0.005, "V")`, `"20MHz"` is `(20e6, "Hz")`, `"-9 mV"` is
/// `(-0.009, "V")`, `"3.3"` is `(3.3, "")`. A lone prefix letter with no
/// unit after it is read as the unit (`"5 m"` is 5 of unit `m`). `None`
/// when the text does not start with a finite number.
pub fn parse(text: &str) -> Option<(f64, String)> {
    let text = text.trim().replace(MINUS, "-");
    let split = text
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || ((c == '-' || c == '+') && i == 0)))
        .map_or(text.len(), |(i, _)| i);
    let (number, rest) = text.split_at(split);
    // An exponent (`1e-6`) belongs to the number.
    let (number, rest) = match rest.strip_prefix(['e', 'E']) {
        Some(after) if after.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+') => {
            let end = after
                .char_indices()
                .find(|&(i, c)| !(c.is_ascii_digit() || ((c == '-' || c == '+') && i == 0)))
                .map_or(after.len(), |(i, _)| i);
            let exponent = &after[..end];
            (format!("{number}e{exponent}"), &after[end..])
        }
        _ => (number.to_string(), rest),
    };
    let value: f64 = number.parse().ok().filter(|v: &f64| v.is_finite())?;
    let rest = rest.trim();
    let mut chars = rest.chars();
    let first = chars.next();
    let unit_after = chars.as_str();
    let prefix = first.and_then(|c| {
        let c = if c == 'u' || c == '\u{3bc}' { 'µ' } else { c };
        PREFIXES.iter().find(|(p, _)| *p == c).map(|(_, e)| *e)
    });
    match prefix {
        Some(exponent) if !unit_after.is_empty() => {
            Some((value * 10f64.powi(exponent), unit_after.to_string()))
        }
        _ => Some((value, rest.to_string())),
    }
}

/// `value` of `unit` with the prefix that keeps 1 to 999 before the point,
/// to `significant` digits, trailing zeros dropped: `format(0.005, "V", 3)`
/// is `5 mV`, `format(5e5, "Hz", 3)` is `500 kHz`. Zero is `0 V`. A
/// negative value starts with the minus sign (U+2212); the unit follows a
/// thin space.
pub fn format(value: f64, unit: &str, significant: u32) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let (factor, prefix) = prefix_for(value);
    let number = number(value / factor, significant);
    let unit = format!("{prefix}{unit}");
    if unit.is_empty() {
        number
    } else {
        format!("{number}{THIN_SPACE}{unit}")
    }
}

/// `value` to `significant` digits with no prefix, trailing zeros dropped,
/// a minus sign (U+2212) before a negative one: `number(-15.0, 3)` is
/// `−15`, `number(0.5, 3)` is `0.5`.
pub fn number(value: f64, significant: u32) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let digits = trim_zeros(&fixed(value.abs(), decimals_for(value.abs(), significant)));
    if value < 0.0 && digits != "0" {
        format!("{MINUS}{digits}")
    } else {
        digits
    }
}

/// The power of a thousand that leaves 1 to 999 of `value` before the
/// point, and its prefix: `(1e-3, "m")` for 0.015, `(1.0, "")` for 1 to
/// 999 and for zero.
pub fn prefix_for(value: f64) -> (f64, &'static str) {
    let magnitude = value.abs();
    if magnitude == 0.0 || !magnitude.is_finite() {
        return (1.0, "");
    }
    let exponent = ((magnitude.log10() / 3.0).floor() as i32 * 3).clamp(-15, 9);
    let prefix = match exponent {
        9 => "G",
        6 => "M",
        3 => "k",
        -3 => "m",
        -6 => "µ",
        -9 => "n",
        -12 => "p",
        -15 => "f",
        _ => return (1.0, ""),
    };
    (10f64.powi(exponent), prefix)
}

/// The decimals that give `significant` digits for a positive `magnitude`.
pub(crate) fn decimals_for(magnitude: f64, significant: u32) -> usize {
    if magnitude == 0.0 || !magnitude.is_finite() {
        return 0;
    }
    let leading = magnitude.log10().floor() as i32 + 1;
    usize::try_from(i32::try_from(significant).unwrap_or(3) - leading).unwrap_or(0)
}

/// `value` with `decimals` digits after the point, rounded half away from
/// zero (as Typst's `calc.round`), so the table and the CLI agree.
pub(crate) fn fixed(value: f64, decimals: usize) -> String {
    let factor = 10f64.powi(i32::try_from(decimals).unwrap_or(0));
    let rounded = (value * factor).round() / factor;
    format!("{rounded:.decimals$}")
}

/// `text` without the zeros after its point, and without the point when
/// nothing is left after it.
fn trim_zeros(text: &str) -> String {
    if !text.contains('.') {
        return text.to_string();
    }
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The smallest of 1, 2 and 5 times a power of ten that is at least
/// `raw`: the scope's steps per division. `raw` at or below zero, or not
/// finite, is 1.
pub fn step_125(raw: f64) -> f64 {
    if !(raw.is_finite() && raw > 0.0) {
        return 1.0;
    }
    let decade = 10f64.powf(raw.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * decade)
        // A hair of slack, so 0.2 stays 0.2 and does not round up to 0.5.
        .find(|step| *step >= raw * (1.0 - 1e-9))
        .unwrap_or(10.0 * decade)
}
