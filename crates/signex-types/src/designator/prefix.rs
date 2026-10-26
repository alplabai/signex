//! Reference-prefix parsing: the one home for "what letters head this
//! designator and what number follows".

/// The alphabetic head of a reference, case preserved: `"R"` for `"R12"` and
/// `"R?"`, `"BT"` for `"BT1"`. A leading `#` is not alphabetic, so `"#PWR?"`
/// yields `""` (annotation never touches those).
pub fn alphabetic_prefix(reference: &str) -> &str {
    let end = reference
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(reference.len());
    &reference[..end]
}

/// Split a reference into its alphabetic prefix and the number that follows
/// it. The number is `None` when the remainder is not a plain `u32`
/// (`"R?"`, `"R1A"`, empty remainder).
pub fn split_reference(reference: &str) -> (&str, Option<u32>) {
    let prefix = alphabetic_prefix(reference);
    (prefix, reference[prefix.len()..].parse::<u32>().ok())
}

/// Reference prefix a library designator places with: `"U?"` -> `"U"`,
/// empty -> `"U"`. A leading `#` is kept (`"#PWR?"` -> `"#PWR"`), the
/// convention for power and virtual parts. Letters are upper-cased.
pub fn placement_prefix(designator: &str) -> String {
    let trimmed = designator.trim();
    let hash = if trimmed.starts_with('#') { "#" } else { "" };
    let letters = alphabetic_prefix(trimmed.trim_start_matches('#'));
    if letters.is_empty() {
        "U".into()
    } else {
        format!("{hash}{letters}").to_ascii_uppercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_prefix_keeps_a_leading_hash_and_defaults_to_u() {
        assert_eq!(placement_prefix("U?"), "U");
        assert_eq!(placement_prefix("#PWR?"), "#PWR");
        assert_eq!(placement_prefix("r1"), "R");
        assert_eq!(placement_prefix("?"), "U");
        assert_eq!(placement_prefix("#?"), "U");
    }

    #[test]
    fn multi_letter_prefixes_split_whole() {
        assert_eq!(split_reference("BT1"), ("BT", Some(1)));
        assert_eq!(split_reference("DS2"), ("DS", Some(2)));
        assert_eq!(split_reference("R?"), ("R", None));
        assert_eq!(split_reference("r12"), ("r", Some(12)));
        assert_eq!(split_reference("#PWR1"), ("", None));
        assert_eq!(split_reference("R1A"), ("R", None));
    }
}
