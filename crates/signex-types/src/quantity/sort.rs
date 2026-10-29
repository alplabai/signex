//! Ordering of quantities for sorted lists, and the one place free text is
//! read as a value.
//!
//! A typed value sorts by `(unit family, base value)`: every ohm value
//! ahead of every farad value, 2 kΩ before 10 kΩ, 100 nF before 1 µF. The
//! family order is [`Unit::STANDARD`]; an unknown unit follows them, by
//! its token. Nothing here is stored or used to accept input.

use std::cmp::Ordering;

use super::parse::{quantity_from_stored, split_unit_string};
use super::prefix::SiPrefix;
use super::unit::Unit;
use super::value::Quantity;

/// Position of a unit's family in a sorted column. Known units keep the
/// [`Unit::STANDARD`] order; every [`Unit::Other`] shares the last rank.
fn family_rank(unit: &Unit) -> usize {
    Unit::STANDARD
        .iter()
        .position(|u| u == unit)
        .unwrap_or(Unit::STANDARD.len())
}

/// Total order over quantities: unit family, then unknown-unit token,
/// then base value. A legacy `{4.7, "kΩ"}` and a canonical
/// `{4700, "ohm"}` read as the same quantity and compare `Equal`.
pub fn compare_for_sort(a: &Quantity, b: &Quantity) -> Ordering {
    family_rank(a.unit())
        .cmp(&family_rank(b.unit()))
        .then_with(|| match (a.unit(), b.unit()) {
            (Unit::Other(x), Unit::Other(y)) => x.cmp(y),
            _ => Ordering::Equal,
        })
        // `Quantity` is always finite, so `partial_cmp` never fails.
        .then_with(|| a.base().partial_cmp(&b.base()).unwrap_or(Ordering::Equal))
}

/// Unit a bare shorthand such as `10k` or `100n` most likely means:
/// resistor prefixes (`k M G R`) read as ohm, capacitor prefixes
/// (`p n u µ`) as farad, anything else as an unknown unit.
fn guess_unit(prefix: SiPrefix) -> Unit {
    match prefix {
        SiPrefix::Kilo | SiPrefix::Mega | SiPrefix::Giga | SiPrefix::One => Unit::Ohm,
        SiPrefix::Pico | SiPrefix::Nano | SiPrefix::Micro => Unit::Farad,
        _ => Unit::Other(String::new()),
    }
}

/// Read a BOM cell's value text (`10k`, `4k7`, `100n`, `4.7 kΩ`, `10uF`,
/// `4R7`) as a quantity, for ordering only.
///
/// This is a guess: a bare `10k` is assumed to be ohm and a bare `100n`
/// farad, which a BOM that has only the string cannot know. The result
/// must never be stored, shown, grouped on, or used to accept user
/// input; editors take a number field plus a prefix and unit picker and
/// never go through here. `None` for text that is not a number with an
/// optional prefix and unit.
pub fn sort_key_from_value_text(text: &str) -> Option<Quantity> {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let digits_end = compact
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || (i == 0 && (c == '-' || c == '+'))))
        .map_or(compact.len(), |(i, _)| i);
    let (lead, rest) = compact.split_at(digits_end);
    if !lead.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut chars = rest.chars();
    // RKM: the letter sits where the decimal point would (`4k7`, `4R7`).
    if let (Some(letter), Some(next)) = (chars.next(), rest.chars().nth(1))
        && next.is_ascii_digit()
        && !lead.contains('.')
    {
        let after = &rest[letter.len_utf8()..];
        let frac_end = after
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after.len());
        let (frac, tail) = after.split_at(frac_end);
        let mantissa: f64 = format!("{lead}.{frac}").parse().ok()?;
        let (prefix, unit) = if letter == 'R' || letter == 'r' {
            (SiPrefix::One, Unit::Ohm)
        } else {
            let prefix = SiPrefix::from_char(letter)?;
            (prefix, guess_unit(prefix))
        };
        let unit = if tail.is_empty() {
            unit
        } else {
            split_unit_string(tail)
                .filter(|(p, _)| *p == SiPrefix::One)?
                .1
        };
        return Quantity::from_mantissa(mantissa, prefix, unit);
    }
    let mantissa: f64 = lead.parse().ok()?;
    if rest.is_empty() {
        return Quantity::new(mantissa, Unit::Other(String::new()));
    }
    if let Some((prefix, unit)) = split_unit_string(rest) {
        return Quantity::from_mantissa(mantissa, prefix, unit);
    }
    let mut only = rest.chars();
    match (only.next(), only.next()) {
        (Some('R' | 'r'), None) => Quantity::from_mantissa(mantissa, SiPrefix::One, Unit::Ohm),
        (Some(c), None) => {
            let prefix = SiPrefix::from_char(c)?;
            Quantity::from_mantissa(mantissa, prefix, guess_unit(prefix))
        }
        _ => quantity_from_stored(mantissa, rest).filter(|q| !matches!(q.unit(), Unit::Other(_))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> Quantity {
        sort_key_from_value_text(text).unwrap_or_else(|| panic!("{text:?} should parse"))
    }

    #[test]
    fn reads_engineering_and_rkm_forms() {
        let ohm = |b| Quantity::new(b, Unit::Ohm).unwrap();
        let farad = |b| Quantity::new(b, Unit::Farad).unwrap();
        assert_eq!(key("10k"), ohm(10_000.0));
        assert_eq!(key("4k7"), ohm(4_700.0));
        assert_eq!(key("4.7 kΩ"), ohm(4_700.0));
        assert_eq!(key("4R7"), ohm(4.7));
        assert_eq!(key("10R"), ohm(10.0));
        assert_eq!(key("10 ohm"), ohm(10.0));
        assert_eq!(key("1M"), ohm(1_000_000.0));
        assert_eq!(key("100n"), farad(100e-9));
        assert_eq!(key("10uF"), farad(10e-6));
        assert_eq!(key("2u2F"), farad(2.2e-6));
        assert_eq!(key("4.7µF"), farad(4.7e-6));
    }

    #[test]
    fn leaves_plain_text_unparsed() {
        for text in ["", " ", "abc", "R0603", "k7", "10 X", "NaN", "4.7.1k", "-"] {
            assert!(sort_key_from_value_text(text).is_none(), "{text:?}");
        }
    }

    #[test]
    fn families_cluster_and_values_order_within() {
        let mut keys = [key("1u"), key("2k"), key("100n"), key("10k"), key("4k7")];
        keys.sort_by(compare_for_sort);
        let bases: Vec<f64> = keys.iter().map(Quantity::base).collect();
        assert_eq!(bases, [2000.0, 4700.0, 10_000.0, 100e-9, 1e-6]);
    }
}
