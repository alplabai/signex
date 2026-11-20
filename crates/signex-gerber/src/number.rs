//! Exact number conversion: the digits of a file to integer nm in `i128`,
//! never through a float, rounded half away from zero once. Macro
//! arithmetic is the one place a float is used (`mm_f64_to_nm`).

use pcb_geom::MAX_ABS_NM;

use crate::image::Units;

const NM_PER_MM: i128 = 1_000_000;
const NM_PER_INCH: i128 = 25_400_000;
/// The most digits a number may have, so every product fits an `i128`.
const MAX_DIGITS: usize = 30;

/// nm in one file unit.
pub(crate) fn nm_per_unit(units: Units) -> i128 {
    match units {
        Units::Millimetres => NM_PER_MM,
        Units::Inches => NM_PER_INCH,
    }
}

/// Which zeros a coordinate without a decimal point leaves out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Omitted {
    /// `X0012` with 2.4 digits is 0.0012: count decimals from the right.
    Leading,
    /// `X0012` with 2.4 digits is 00.1200: pad on the right.
    Trailing,
}

/// The digits of a coordinate written without a decimal point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CoordFormat {
    pub integer: u32,
    pub decimal: u32,
    pub omitted: Omitted,
}

/// A decimal number as its digits and the count of them after the point:
/// `-1.25` is `(-125, 2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fixed {
    pub value: i128,
    pub decimals: u32,
}

/// `num / den` rounded half away from zero; `den` is positive.
pub(crate) fn round_half_away(num: i128, den: i128) -> i128 {
    let quotient = num / den;
    let rest = num % den;
    if 2 * rest.abs() >= den {
        quotient + num.signum()
    } else {
        quotient
    }
}

fn split_sign(text: &str) -> (i128, &str) {
    match text.as_bytes().first() {
        Some(b'-') => (-1, &text[1..]),
        Some(b'+') => (1, &text[1..]),
        _ => (1, text),
    }
}

fn digits_value(digits: &str) -> Option<i128> {
    if digits.len() > MAX_DIGITS || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if digits.is_empty() {
        return Some(0);
    }
    digits.parse().ok()
}

/// `-1.25`, `+.5`, `3.`, `12`: a sign, digits and at most one point, with
/// at least one digit.
pub(crate) fn parse_decimal(text: &str) -> Option<Fixed> {
    let (sign, unsigned) = split_sign(text.trim());
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    let decimals = u32::try_from(fraction.len()).ok()?;
    let joined = format!("{whole}{fraction}");
    Some(Fixed {
        value: sign * digits_value(&joined)?,
        decimals,
    })
}

/// A fixed-point value in `units` as nm, within the geometry's reach.
pub(crate) fn fixed_to_nm(fixed: Fixed, units: Units) -> Option<i64> {
    let scale = 10i128.checked_pow(fixed.decimals)?;
    let nm = round_half_away(fixed.value.checked_mul(nm_per_unit(units))?, scale);
    i64::try_from(nm)
        .ok()
        .filter(|v| v.unsigned_abs() <= MAX_ABS_NM.unsigned_abs())
}

/// A decimal written in `units` (an aperture size, an Excellon coordinate
/// with a point) as nm.
pub(crate) fn decimal_to_nm(text: &str, units: Units) -> Option<i64> {
    fixed_to_nm(parse_decimal(text)?, units)
}

/// A coordinate in `format` as nm. One written with a decimal point is
/// read as a decimal whatever the format says.
pub(crate) fn coordinate_to_nm(text: &str, format: CoordFormat, units: Units) -> Option<i64> {
    if text.contains('.') {
        return decimal_to_nm(text, units);
    }
    let (sign, digits) = split_sign(text.trim());
    if digits.is_empty() {
        return None;
    }
    let value = digits_value(digits)?;
    let value = match format.omitted {
        Omitted::Leading => value,
        Omitted::Trailing => {
            let width = usize::try_from(format.integer + format.decimal).ok()?;
            let pad = u32::try_from(width.checked_sub(digits.len())?).ok()?;
            value.checked_mul(10i128.checked_pow(pad)?)?
        }
    };
    fixed_to_nm(
        Fixed {
            value: sign * value,
            decimals: format.decimal,
        },
        units,
    )
}

/// A macro result in `units` as nm: the one float-to-nm step.
pub(crate) fn mm_f64_to_nm(value: f64, units: Units) -> Option<i64> {
    let nm = (value * nm_per_unit(units) as f64).round();
    (nm.is_finite() && nm.abs() <= MAX_ABS_NM as f64).then_some(nm as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MM: Units = Units::Millimetres;
    const INCH: Units = Units::Inches;

    fn format(integer: u32, decimal: u32, omitted: Omitted) -> CoordFormat {
        CoordFormat {
            integer,
            decimal,
            omitted,
        }
    }

    #[test]
    fn six_decimal_millimetres_are_the_nm_themselves() {
        let f = format(4, 6, Omitted::Leading);
        assert_eq!(coordinate_to_nm("4000000", f, MM), Some(4_000_000));
        assert_eq!(coordinate_to_nm("-5000000", f, MM), Some(-5_000_000));
        assert_eq!(coordinate_to_nm("0", f, MM), Some(0));
        assert_eq!(coordinate_to_nm("+12", f, MM), Some(12));
    }

    #[test]
    fn inches_convert_exactly() {
        let f = format(2, 4, Omitted::Leading);
        // 1.0000 inch.
        assert_eq!(coordinate_to_nm("10000", f, INCH), Some(25_400_000));
        // 0.0001 inch is 2540 nm.
        assert_eq!(coordinate_to_nm("1", f, INCH), Some(2_540));
        assert_eq!(coordinate_to_nm("-25", f, INCH), Some(-63_500));
    }

    #[test]
    fn trailing_omission_pads_on_the_right() {
        let f = format(2, 4, Omitted::Trailing);
        // "0012" is 00.12 inch.
        assert_eq!(coordinate_to_nm("0012", f, INCH), Some(3_048_000));
        // "-01" is -01.0000 inch.
        assert_eq!(coordinate_to_nm("-01", f, INCH), Some(-25_400_000));
        // More digits than the format holds.
        assert_eq!(coordinate_to_nm("1234567", f, INCH), None);
        let mm = format(3, 3, Omitted::Trailing);
        assert_eq!(coordinate_to_nm("015", mm, MM), Some(15_000_000));
    }

    #[test]
    fn a_decimal_point_wins_over_the_format() {
        let f = format(2, 4, Omitted::Leading);
        assert_eq!(coordinate_to_nm("1.5", f, MM), Some(1_500_000));
        assert_eq!(coordinate_to_nm("-.5", f, INCH), Some(-12_700_000));
    }

    #[test]
    fn decimals_round_half_away_from_zero() {
        // 0.0000005 mm is half a nm.
        assert_eq!(decimal_to_nm("0.0000005", MM), Some(1));
        assert_eq!(decimal_to_nm("-0.0000005", MM), Some(-1));
        assert_eq!(decimal_to_nm("0.0000004", MM), Some(0));
        assert_eq!(decimal_to_nm("1.2", MM), Some(1_200_000));
        assert_eq!(decimal_to_nm("3.", MM), Some(3_000_000));
        assert_eq!(round_half_away(-3, 2), -2);
        assert_eq!(round_half_away(5, 4), 1);
    }

    #[test]
    fn junk_and_out_of_reach_values_are_refused() {
        assert_eq!(parse_decimal(""), None);
        assert_eq!(parse_decimal("."), None);
        assert_eq!(parse_decimal("1.2.3"), None);
        assert_eq!(parse_decimal("1e3"), None);
        assert_eq!(decimal_to_nm("2000000", MM), None);
        assert_eq!(decimal_to_nm(&"9".repeat(40), MM), None);
        let f = format(4, 6, Omitted::Leading);
        assert_eq!(coordinate_to_nm("", f, MM), None);
        assert_eq!(coordinate_to_nm("-", f, MM), None);
        assert_eq!(coordinate_to_nm("12a", f, MM), None);
    }

    #[test]
    fn a_macro_float_rounds_once() {
        assert_eq!(mm_f64_to_nm(0.25, MM), Some(250_000));
        assert_eq!(mm_f64_to_nm(0.1, INCH), Some(2_540_000));
        assert_eq!(mm_f64_to_nm(f64::NAN, MM), None);
        assert_eq!(mm_f64_to_nm(1e9, MM), None);
    }
}
