//! The one case-folding rule Find and Replace share.
//!
//! Each char folds to its single-char lowercase (a char whose lowercase is
//! several chars keeps itself), so a folded text has exactly as many chars as
//! the original and a match found in the folded text sits at the same char
//! position in the original. Scan and replace both go through here, so a hit
//! the scan reports is always one the replace rewrites.

pub(super) fn fold(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(single), None) => single,
        _ => c,
    }
}

pub(super) fn folded(text: &str) -> Vec<char> {
    text.chars().map(fold).collect()
}

/// Whether `needle` (already folded) occurs in `hay` (already folded) at `at`.
pub(super) fn matches_at(hay: &[char], at: usize, needle: &[char]) -> bool {
    at + needle.len() <= hay.len() && hay[at..at + needle.len()] == *needle
}

/// Whether `text` contains `needle` under the shared folding rule.
pub(super) fn contains_folded(text: &str, needle: &[char]) -> bool {
    let hay = folded(text);
    (0..hay.len()).any(|at| matches_at(&hay, at, needle))
}
