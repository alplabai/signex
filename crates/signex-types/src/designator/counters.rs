//! Per-prefix designator counters shared by Annotate and Paste.

use std::collections::HashMap;

use super::split_reference;

/// The highest number in use per alphabetic prefix. Unnumbered references
/// (`R?`) and ones with no letters (`#PWR1`) count for nothing.
pub fn seed_counters<'a>(references: impl IntoIterator<Item = &'a str>) -> HashMap<String, u32> {
    let mut counters: HashMap<String, u32> = HashMap::new();
    for reference in references {
        let (prefix, number) = split_reference(reference);
        if prefix.is_empty() {
            continue;
        }
        if let Some(n) = number {
            let highest = counters.entry(prefix.to_owned()).or_insert(0);
            *highest = (*highest).max(n);
        }
    }
    counters
}

/// Raise `into` to the higher of its own and `from`'s number for every prefix.
pub fn merge_counters(into: &mut HashMap<String, u32>, from: &HashMap<String, u32>) {
    for (prefix, number) in from {
        let highest = into.entry(prefix.clone()).or_insert(0);
        *highest = (*highest).max(*number);
    }
}

/// The next unused `prefix` designator, claimed in `counters` so the next
/// call gives the one after it. `None` when the number would pass `u32::MAX`.
pub fn take_next(prefix: &str, counters: &mut HashMap<String, u32>) -> Option<String> {
    let highest = counters.entry(prefix.to_owned()).or_insert(0);
    *highest = highest.checked_add(1)?;
    Some(format!("{prefix}{highest}"))
}

/// `prefix` followed by one more than the highest number any of `references`
/// carries right after that prefix: what a part placed on a sheet or a board
/// is numbered (#1381, #2364).
pub fn next_reference<'a>(prefix: &str, references: impl IntoIterator<Item = &'a str>) -> String {
    let next = references
        .into_iter()
        .filter_map(|reference| {
            reference
                .trim()
                .strip_prefix(prefix)
                .and_then(|n| n.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    format!("{prefix}{next}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_reference_follows_the_highest_number_of_its_prefix() {
        assert_eq!(next_reference("D", ["D1", " D7 ", "DS9", "R12"]), "D8");
        assert_eq!(next_reference("D", ["R1"]), "D1");
    }

    #[test]
    fn the_counter_starts_after_the_highest_number_in_use() {
        let mut counters = seed_counters(["R1", "R7", "C3", "R?", "#PWR4", "U1A"]);
        assert_eq!(take_next("R", &mut counters).as_deref(), Some("R8"));
        assert_eq!(take_next("R", &mut counters).as_deref(), Some("R9"));
        assert_eq!(take_next("C", &mut counters).as_deref(), Some("C4"));
        assert_eq!(take_next("Q", &mut counters).as_deref(), Some("Q1"));
    }

    #[test]
    fn the_highest_number_wins_whatever_the_order() {
        let mut counters = seed_counters(["R7", "R1"]);
        assert_eq!(take_next("R", &mut counters).as_deref(), Some("R8"));
        let mut counters = seed_counters(["R1", "R7"]);
        assert_eq!(take_next("R", &mut counters).as_deref(), Some("R8"));
    }

    /// Each EN IEC 81346-2 class code counts on its own (#1416): `QAC` does
    /// not share a counter with `Q`, nor `RAA` with `RAC`.
    #[test]
    fn each_class_code_has_its_own_counter() {
        let mut counters = seed_counters(["Q9", "QAC2", "RAC4", "RAA1"]);
        assert_eq!(take_next("QAC", &mut counters).as_deref(), Some("QAC3"));
        assert_eq!(take_next("Q", &mut counters).as_deref(), Some("Q10"));
        assert_eq!(take_next("RAA", &mut counters).as_deref(), Some("RAA2"));
        assert_eq!(take_next("RAC", &mut counters).as_deref(), Some("RAC5"));
        assert_eq!(next_reference("QAC", ["Q9", "QAC2"]), "QAC3");
        assert_eq!(next_reference("Q", ["Q9", "QAC12"]), "Q10");
    }

    #[test]
    fn merging_keeps_the_higher_number_per_prefix() {
        let mut a = seed_counters(["R3", "C9"]);
        merge_counters(&mut a, &seed_counters(["R5", "C2", "U1"]));
        assert_eq!(take_next("R", &mut a).as_deref(), Some("R6"));
        assert_eq!(take_next("C", &mut a).as_deref(), Some("C10"));
        assert_eq!(take_next("U", &mut a).as_deref(), Some("U2"));
    }

    #[test]
    fn a_counter_at_the_limit_gives_nothing() {
        let mut counters = seed_counters(["R4294967295"]);
        assert_eq!(take_next("R", &mut counters), None);
    }
}
