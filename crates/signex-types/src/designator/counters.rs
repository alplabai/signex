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

/// The next unused `prefix` designator, claimed in `counters` so the next
/// call gives the one after it. `None` when the number would pass `u32::MAX`.
pub fn take_next(prefix: &str, counters: &mut HashMap<String, u32>) -> Option<String> {
    let highest = counters.entry(prefix.to_owned()).or_insert(0);
    *highest = highest.checked_add(1)?;
    Some(format!("{prefix}{highest}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_counter_starts_after_the_highest_number_in_use() {
        let mut counters = seed_counters(["R1", "R7", "C3", "R?", "#PWR4", "U1A"]);
        assert_eq!(take_next("R", &mut counters).as_deref(), Some("R8"));
        assert_eq!(take_next("R", &mut counters).as_deref(), Some("R9"));
        assert_eq!(take_next("C", &mut counters).as_deref(), Some("C4"));
        assert_eq!(take_next("Q", &mut counters).as_deref(), Some("Q1"));
    }

    #[test]
    fn a_counter_at_the_limit_gives_nothing() {
        let mut counters = seed_counters(["R4294967295"]);
        assert_eq!(take_next("R", &mut counters), None);
    }
}
