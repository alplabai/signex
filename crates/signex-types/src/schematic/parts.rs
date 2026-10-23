//! Grouping placed symbol instances into physical parts (#691).
//!
//! A multi-unit part (a dual op-amp, a quad gate) is placed as several
//! [`Symbol`](super::Symbol) instances that share one designator and differ
//! in `unit`. Annotation, ERC and the BOM all have to agree on which
//! instances are one part, so the rule lives here and nowhere else.

use std::collections::HashMap;

use super::Symbol;

/// The identity fields of a placed symbol that decide part membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartKey<'a> {
    pub reference: &'a str,
    pub lib_id: &'a str,
    pub unit: u32,
}

impl Symbol {
    /// The fields [`group_into_parts`] groups by.
    pub fn part_key(&self) -> PartKey<'_> {
        PartKey {
            reference: self.reference.trim(),
            lib_id: &self.lib_id,
            unit: self.unit,
        }
    }
}

/// Group instances into physical parts. Returns one `Vec` of input positions
/// per part, parts ordered by their first member, members in input order.
///
/// Two instances are units of one part when they share a designator and a
/// library symbol and carry different unit numbers. The same unit placed
/// twice under one designator is two parts (a duplicate designator, not a
/// second unit), and so are instances of different library symbols. Callers
/// choose the input order; it decides which instances pair up when several
/// unannotated (`U?`) parts of one library symbol are mixed together.
pub fn group_into_parts<'a>(items: impl IntoIterator<Item = PartKey<'a>>) -> Vec<Vec<usize>> {
    let mut parts: Vec<Vec<usize>> = Vec::new();
    // Unit numbers each part already holds, parallel to `parts`.
    let mut taken_units: Vec<Vec<u32>> = Vec::new();
    // Parts per (designator, library symbol), in creation order.
    let mut open: HashMap<(&str, &str), Vec<usize>> = HashMap::new();

    for (position, key) in items.into_iter().enumerate() {
        let candidates = open.entry((key.reference, key.lib_id)).or_default();
        match candidates
            .iter()
            .copied()
            .find(|&part| !taken_units[part].contains(&key.unit))
        {
            Some(part) => {
                taken_units[part].push(key.unit);
                parts[part].push(position);
            }
            None => {
                candidates.push(parts.len());
                taken_units.push(vec![key.unit]);
                parts.push(vec![position]);
            }
        }
    }
    parts
}

/// Part index per symbol index for the symbols a designator applies to
/// (not power ports, not `#` markers), grouped in `order` by
/// [`group_into_parts`]. Annotation and its preview both call this, so what
/// the dialog promises is what the pass assigns.
pub fn designatable_parts(symbols: &[Symbol], order: &[usize]) -> HashMap<usize, usize> {
    let eligible: Vec<usize> = order
        .iter()
        .copied()
        .filter(|&idx| !symbols[idx].is_power && !symbols[idx].reference.starts_with('#'))
        .collect();
    let parts = group_into_parts(eligible.iter().map(|&idx| symbols[idx].part_key()));
    let mut part_of = HashMap::new();
    for (part, members) in parts.iter().enumerate() {
        for &member in members {
            part_of.insert(eligible[member], part);
        }
    }
    part_of
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key<'a>(reference: &'a str, lib_id: &'a str, unit: u32) -> PartKey<'a> {
        PartKey {
            reference,
            lib_id,
            unit,
        }
    }

    #[test]
    fn units_with_one_designator_form_one_part() {
        let parts = group_into_parts([key("U1", "A", 1), key("U1", "A", 2)]);
        assert_eq!(parts, vec![vec![0, 1]]);
    }

    #[test]
    fn same_unit_twice_is_two_parts() {
        let parts = group_into_parts([key("U1", "A", 1), key("U1", "A", 1)]);
        assert_eq!(parts, vec![vec![0], vec![1]]);
    }

    #[test]
    fn different_library_symbols_are_different_parts() {
        let parts = group_into_parts([key("U1", "A", 1), key("U1", "B", 2)]);
        assert_eq!(parts, vec![vec![0], vec![1]]);
    }

    #[test]
    fn unannotated_units_pair_in_input_order() {
        let parts = group_into_parts([
            key("U?", "A", 1),
            key("U?", "A", 2),
            key("U?", "A", 1),
            key("U?", "A", 2),
        ]);
        assert_eq!(parts, vec![vec![0, 1], vec![2, 3]]);
    }

    #[test]
    fn empty_input_has_no_parts() {
        assert!(group_into_parts(std::iter::empty()).is_empty());
    }
}
