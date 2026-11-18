//! The names and numbers the ODB++ text files are written with.

use crate::fab::frame::millionths;

/// The longest entity name (job, step, layer) the export writes.
const NAME_MAX: usize = 64;

/// An entity name: lowercase ASCII letters, digits and `_ + - .`, at most
/// 64 characters, never starting with `.`, `+` or `-`. Any other character
/// becomes `_`.
pub(crate) fn entity_name(value: &str) -> String {
    let mut name: String = value
        .trim()
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() || name.starts_with(['.', '+', '-']) {
        name.insert(0, '_');
    }
    name.truncate(NAME_MAX);
    name
}

/// One word of a record (a net, designator, part or pin name): printable
/// ASCII without spaces or `;`, which end a word or start the attributes.
/// Any other character becomes `_`; an empty word is `_`.
pub(crate) fn word(value: &str) -> String {
    let word: String = value
        .trim()
        .chars()
        .map(|c| if c.is_ascii_graphic() && c != ';' { c } else { '_' })
        .collect();
    if word.is_empty() { "_".to_string() } else { word }
}

/// A length in nm as mm.
pub(crate) fn mm(nm: i64) -> String {
    millionths(nm)
}

/// A length in nm as micrometres, the unit of symbol sizes and drill
/// tools in a millimetre job.
pub(crate) fn um(nm: i64) -> String {
    millionths(nm.saturating_mul(1_000))
}

/// A counter-clockwise turn in micro-degrees as the clockwise degrees
/// ODB++ rotations are given in, in `0..360`.
pub(crate) fn cw_degrees(udeg: i64) -> String {
    millionths((-udeg).rem_euclid(360_000_000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_lowercase_ascii_and_never_empty() {
        assert_eq!(entity_name("TopCopper"), "topcopper");
        assert_eq!(entity_name("Fold Lines/2"), "fold_lines_2");
        assert_eq!(entity_name(""), "_");
        assert_eq!(entity_name("-x"), "_-x");
        assert_eq!(entity_name(&"a".repeat(80)).len(), 64);
        assert_eq!(word(" R 1;x "), "R_1_x");
        assert_eq!(word("Ω"), "_");
        assert_eq!(word(""), "_");
    }

    #[test]
    fn numbers_are_in_mm_um_and_clockwise_degrees() {
        assert_eq!(mm(1_250_000), "1.25");
        assert_eq!(um(1_250_000), "1250");
        assert_eq!(um(127), "0.127");
        assert_eq!(cw_degrees(0), "0");
        assert_eq!(cw_degrees(90_000_000), "270");
        assert_eq!(cw_degrees(-30_000_000), "30");
    }
}
