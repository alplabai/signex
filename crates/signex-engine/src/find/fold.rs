//! The one case-folding rule Find and Replace share.
//!
//! Each char folds to its single-char lowercase (a char whose lowercase is
//! several chars keeps itself), so a folded text has exactly as many chars as
//! the original and a match found in the folded text sits at the same char
//! position in the original. A literal query without Match case folds both
//! sides here (`query.rs`), and scan and replace share that query, so a hit
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

/// Whether `needle` occurs in `hay` at `at` (both folded, or neither).
pub(super) fn matches_at(hay: &[char], at: usize, needle: &[char]) -> bool {
    at + needle.len() <= hay.len() && hay[at..at + needle.len()] == *needle
}
