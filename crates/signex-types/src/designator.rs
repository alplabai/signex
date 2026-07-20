//! Natural ordering for reference designators and pin numbers.
//!
//! Designators mix a letter prefix with a decimal index (`R1`, `R10`, `VR2`)
//! and may carry further sections (`U1_2`, `J3-10`). Ordering them with
//! [`str::cmp`] compares bytes, so `R10` lands between `R1` and `R2` and every
//! BOM, netlist and drift list reads wrong. Every such list routes through
//! [`compare_references`] instead.

use std::cmp::Ordering;

/// Split off the leading run of same-kind bytes (all digits, or all non-digits).
///
/// The input must be non-empty; callers check that before recursing.
fn take_run(bytes: &[u8]) -> (&[u8], &[u8]) {
    let digits = bytes[0].is_ascii_digit();
    let len = bytes
        .iter()
        .take_while(|b| b.is_ascii_digit() == digits)
        .count();
    bytes.split_at(len)
}

/// Compare two digit runs by numeric value.
///
/// Leading zeros are stripped and the significant lengths compared first, so
/// arbitrarily long numeric tails stay correctly ordered where `parse::<u64>()`
/// would overflow and mis-order.
fn compare_digit_runs(a: &[u8], b: &[u8]) -> Ordering {
    let a_significant = strip_leading_zeros(a);
    let b_significant = strip_leading_zeros(b);
    a_significant
        .len()
        .cmp(&b_significant.len())
        .then_with(|| a_significant.cmp(b_significant))
}

fn strip_leading_zeros(digits: &[u8]) -> &[u8] {
    let zeros = digits.iter().take_while(|b| **b == b'0').count();
    // An all-zero run is the value zero, not the empty string.
    &digits[zeros.min(digits.len().saturating_sub(1))..]
}

/// Compare two text runs case-insensitively, so `r1` sits with `R1` rather
/// than after every upper-case designator.
fn compare_text_runs(a: &[u8], b: &[u8]) -> Ordering {
    a.iter()
        .map(u8::to_ascii_lowercase)
        .cmp(b.iter().map(u8::to_ascii_lowercase))
}

/// Natural designator order: `R1 < R2 < R9 < R10`, not `str::cmp`'s
/// `R1 < R10 < R2`.
///
/// Both strings are walked as alternating digit / non-digit runs, so the rule
/// applies to every section of a multi-section designator (`U1_2 < U1_10`), not
/// just the first. Equal-under-the-rule inputs fall back to a byte compare so
/// the result is a total order (required by `sort_by`) and `R01` still differs
/// from `R1`.
pub fn compare_references(a: &str, b: &str) -> Ordering {
    let mut left = a.as_bytes();
    let mut right = b.as_bytes();

    loop {
        match (left.first(), right.first()) {
            (None, None) => break,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(l), Some(r)) => {
                let (left_run, left_rest) = take_run(left);
                let (right_run, right_rest) = take_run(right);
                let ordering = match (l.is_ascii_digit(), r.is_ascii_digit()) {
                    (true, true) => compare_digit_runs(left_run, right_run),
                    (false, false) => compare_text_runs(left_run, right_run),
                    // A number sorts ahead of text at the same position.
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                };
                if ordering != Ordering::Equal {
                    return ordering;
                }
                left = left_rest;
                right = right_rest;
            }
        }
    }

    a.cmp(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut items: Vec<&str>) -> Vec<&str> {
        items.sort_by(|a, b| compare_references(a, b));
        items
    }

    #[test]
    fn orders_index_numerically_not_lexicographically() {
        assert_eq!(
            sorted(vec!["R10", "R2", "R1", "R9"]),
            vec!["R1", "R2", "R9", "R10"]
        );
    }

    #[test]
    fn orders_every_section_of_a_multi_section_designator() {
        assert_eq!(sorted(vec!["U1_10", "U1_2"]), vec!["U1_2", "U1_10"]);
        assert_eq!(sorted(vec!["J3-10", "J3-2"]), vec!["J3-2", "J3-10"]);
    }

    #[test]
    fn orders_prefixes_case_insensitively() {
        assert_eq!(sorted(vec!["VR1", "r1"]), vec!["r1", "VR1"]);
        assert_eq!(sorted(vec!["R2", "r1"]), vec!["r1", "R2"]);
    }

    #[test]
    fn orders_numeric_tails_too_long_to_parse() {
        let huge = "R99999999999999999999999";
        assert_eq!(
            sorted(vec!["VR", huge, "R10", "R", "R2"]),
            vec!["R", "R2", "R10", huge, "VR"]
        );
    }

    #[test]
    fn treats_leading_zeros_as_padding_but_keeps_a_total_order() {
        assert_eq!(compare_digit_runs(b"007", b"7"), Ordering::Equal);
        assert_eq!(compare_digit_runs(b"0", b"00"), Ordering::Equal);
        assert_eq!(compare_references("R007", "R7"), Ordering::Less);
        assert_eq!(compare_references("R7", "R7"), Ordering::Equal);
    }

    #[test]
    fn is_a_strict_total_order_over_a_mixed_set() {
        let items = vec!["R1", "R01", "r1", "R10", "1R", "R", "", "R1A", "R1_2"];
        for a in &items {
            for b in &items {
                assert_eq!(
                    compare_references(a, b),
                    compare_references(b, a).reverse(),
                    "antisymmetry broken for {a:?} vs {b:?}"
                );
            }
        }
    }
}
