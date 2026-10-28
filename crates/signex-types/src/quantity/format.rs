//! Lossless value formatting: engineering (`4.7 kΩ`), RKM (`4k7`) and
//! ASCII (`4.7 kohm`).
//!
//! The digits come from the shortest round-trip form (`{:e}`), capped at
//! 15 significant digits, and the decimal point is shifted as a string,
//! so no float arithmetic (and no float noise) is involved.

use super::prefix::SiPrefix;
use super::unit::Unit;
use super::value::Quantity;

/// Significant digits kept at most.
const MAX_SIG_DIGITS: usize = 15;

/// Which output profile to render.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ValueStyle {
    /// ISO 80000: `4.7 kΩ`, `−200 ppm/°C`. The default.
    #[default]
    Engineering,
    /// IEC 60062 RKM code for Ω, F and H: `4k7`, `2R2`, `100n`. Other
    /// units render as Engineering (a percent as `±1%`).
    Rkm,
    /// Engineering with `µ`->`u`, `Ω`->`ohm`, `°` dropped, `±`->`+/-`.
    Ascii,
}

/// Where a value lands: sign, mantissa text, and chosen prefix.
pub(super) struct Layout {
    pub negative: bool,
    pub text: String,
    pub prefix: SiPrefix,
    /// `text` carries an `e<exp>` because no allowed prefix fits.
    pub scientific: bool,
}

/// Significant digits and decimal exponent of `abs` (> 0): `4700` gives
/// `("47", 3)`, meaning 4.7e3.
fn digits_and_exponent(abs: f64) -> (String, i32) {
    let mut s = format!("{abs:e}");
    if s.split('e')
        .next()
        .is_some_and(|m| m.len() > MAX_SIG_DIGITS + 1)
    {
        s = format!("{abs:.*e}", MAX_SIG_DIGITS - 1);
    }
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    (digits.to_string(), exp.parse().unwrap_or(0))
}

/// Move the decimal point of `d1.d2d3...` right by `shift` places.
fn place_point(digits: &str, shift: i32) -> String {
    let int_len = shift + 1;
    let n = digits.len() as i32;
    if int_len <= 0 {
        format!("0.{}{digits}", "0".repeat((-int_len) as usize))
    } else if int_len >= n {
        format!("{digits}{}", "0".repeat((int_len - n) as usize))
    } else {
        let (a, b) = digits.split_at(int_len as usize);
        format!("{a}.{b}")
    }
}

pub(super) fn layout(base: f64, unit: &Unit) -> Layout {
    if base == 0.0 {
        return Layout {
            negative: false,
            text: "0".into(),
            prefix: SiPrefix::One,
            scientific: false,
        };
    }
    let (digits, exp) = digits_and_exponent(base.abs());
    let allowed = unit.allowed_prefixes();
    let prefix = allowed
        .iter()
        .rev()
        .find(|p| p.exponent() <= exp)
        .or(allowed.first())
        .copied()
        .unwrap_or(SiPrefix::One);
    let shift = exp - prefix.exponent();
    let (text, prefix, scientific) = if (-3..6).contains(&shift) {
        (place_point(&digits, shift), prefix, false)
    } else {
        (
            format!("{}e{exp}", place_point(&digits, 0)),
            SiPrefix::One,
            true,
        )
    };
    Layout {
        negative: base < 0.0,
        text,
        prefix,
        scientific,
    }
}

/// `base` in engineering style (`"4.7 kΩ"`, `"0 Ω"`).
pub fn format_eng(base: f64, unit: &Unit) -> String {
    match Quantity::new(base, unit.clone()) {
        Some(q) => format_quantity(&q, ValueStyle::Engineering, false),
        None => format!("{base} {}", unit.glyph()),
    }
}

/// Render `q` in `style`. `plus_minus` prefixes `±` (`+/-` in ASCII) for
/// template slots with `display_plus_minus` (tolerances).
pub fn format_quantity(q: &Quantity, style: ValueStyle, plus_minus: bool) -> String {
    let l = layout(q.base(), q.unit());
    if style == ValueStyle::Rkm
        && let Some(code) = rkm(&l, q.unit())
    {
        return with_sign(plus_minus, style, code);
    }
    if style == ValueStyle::Rkm && *q.unit() == Unit::Percent && !l.negative {
        return with_sign(plus_minus, style, format!("{}%", l.text));
    }
    let ascii = style == ValueStyle::Ascii;
    let sign = match (l.negative, ascii) {
        (false, _) => "",
        (true, true) => "-",
        (true, false) => "\u{2212}",
    };
    let (prefix, unit) = if ascii {
        (l.prefix.ascii_symbol(), q.unit().ascii())
    } else {
        (l.prefix.symbol(), q.unit().glyph())
    };
    let suffix = format!("{prefix}{unit}");
    let sep = if suffix.is_empty() { "" } else { " " };
    with_sign(plus_minus, style, format!("{sign}{}{sep}{suffix}", l.text))
}

fn with_sign(plus_minus: bool, style: ValueStyle, body: String) -> String {
    match (plus_minus, style) {
        (false, _) => body,
        (true, ValueStyle::Ascii) => format!("+/-{body}"),
        (true, _) => format!("±{body}"),
    }
}

/// RKM code (`4k7`, `470R`, `100n`), or `None` when the value or unit has
/// no RKM form (negative, scientific, units other than Ω/F/H, or a bare
/// F or H with no prefix).
fn rkm(l: &Layout, unit: &Unit) -> Option<String> {
    if l.negative || l.scientific || !matches!(unit, Unit::Ohm | Unit::Farad | Unit::Henry) {
        return None;
    }
    let marker = match (l.prefix, unit) {
        (SiPrefix::One, Unit::Ohm) => "R",
        (SiPrefix::One, _) => return None,
        (SiPrefix::Micro, _) => "u",
        (p, _) => p.symbol(),
    };
    Some(if l.text.contains('.') {
        l.text.replacen('.', marker, 1)
    } else {
        format!("{}{marker}", l.text)
    })
}
