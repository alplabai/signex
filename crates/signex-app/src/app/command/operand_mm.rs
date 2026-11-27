//! The one reading of a length operand typed as text, in mm: a number with
//! an optional `mm` suffix, that the board file can store. A value past the
//! file's range, or one under half a nm that would be stored as 0 nm, is
//! refused here, so no later save fails on it.

use signex_types::format::mm_as_stored_nm;

/// `text` as a length above zero: at least 1 nm once stored.
pub(crate) fn operand_mm(text: &str) -> Option<f64> {
    parse(text).filter(|&(_, nm)| nm > 0).map(|(mm, _)| mm)
}

/// `text` as a length of zero or more.
pub(crate) fn operand_mm_or_zero(text: &str) -> Option<f64> {
    parse(text)
        .filter(|&(mm, nm)| mm >= 0.0 && nm >= 0)
        .map(|(mm, _)| mm)
}

/// The number and its stored nm; `None` when it is not a finite number the
/// file can store.
fn parse(text: &str) -> Option<(f64, i64)> {
    let text = text.trim();
    let bare = text.strip_suffix("mm").unwrap_or(text).trim();
    let mm = bare.parse::<f64>().ok()?;
    Some((mm, mm_as_stored_nm(mm)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_with_or_without_one_mm_suffix_reads() {
        assert_eq!(operand_mm("0.25"), Some(0.25));
        assert_eq!(operand_mm(" 0.25mm "), Some(0.25));
        assert_eq!(operand_mm("0.25 mm"), Some(0.25));
        assert_eq!(operand_mm_or_zero("0"), Some(0.0));
    }

    #[test]
    fn a_doubled_suffix_another_unit_or_a_value_the_file_cannot_store_is_refused() {
        for text in [
            "1mmmm", "0.25°", "1e13", "4e-7", "0", "-1", "NaN", "inf", "", "mm",
        ] {
            assert_eq!(operand_mm(text), None, "{text}");
        }
        for text in ["1mmmm", "0.25°", "1e13", "-4e-7", "-1", "NaN"] {
            assert_eq!(operand_mm_or_zero(text), None, "{text}");
        }
    }
}
