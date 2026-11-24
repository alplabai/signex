//! The rules behind the Component Classes table, free of any widget: which
//! keys are wrong, which rows a filter keeps, and where a row's index goes
//! when another row moves or is removed.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use crate::fonts::ComponentClassEntry;

/// What is wrong with a row's key. Keys are compared as Save stores them
/// ([`saved_row`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClassKeyIssue {
    /// The row has a label but no key: Save leaves such a row out.
    Missing,
    /// The key is not lower snake case (`a-z`, `0-9`, `_`), the shape of
    /// the seed keys. Save still stores it as typed.
    Malformed,
    /// An earlier row Save keeps has the same key; `first` is that row's
    /// index. Save keeps one class per key, so the two rows would become one.
    Duplicate { first: usize },
}

/// A row as Save stores it: key and label trimmed; `None` when either is
/// empty, as Save leaves such a row out. The one rule for both the table's
/// errors and Save.
fn saved_row(entry: &ComponentClassEntry) -> Option<(&str, &str)> {
    let (key, label) = (entry.key.trim(), entry.label.trim());
    (!key.is_empty() && !label.is_empty()).then_some((key, label))
}

/// Whether a trimmed, non-empty `key` is lower snake case.
fn is_well_formed_key(key: &str) -> bool {
    key.bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// The classes Save stores from the table's rows, in row order: each
/// [`saved_row`], one class per key, a repeated key taking the later label.
pub(crate) fn saved_classes(rows: &[ComponentClassEntry]) -> Vec<ComponentClassEntry> {
    let mut saved: Vec<ComponentClassEntry> = Vec::new();
    for (key, label) in rows.iter().filter_map(saved_row) {
        match saved.iter_mut().find(|e| e.key == key) {
            Some(existing) => existing.label = label.to_string(),
            None => saved.push(ComponentClassEntry {
                key: key.to_string(),
                label: label.to_string(),
            }),
        }
    }
    saved
}

/// The issue of each row's key, in row order. A row with neither key nor
/// label (a fresh Add Class row) has none.
pub(crate) fn class_key_issues(classes: &[ComponentClassEntry]) -> Vec<Option<ClassKeyIssue>> {
    let mut first_of: HashMap<&str, usize> = HashMap::new();
    classes
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let key = entry.key.trim();
            if key.is_empty() {
                return (!entry.label.trim().is_empty()).then_some(ClassKeyIssue::Missing);
            }
            let issue = match first_of.entry(key) {
                Entry::Occupied(first) => Some(ClassKeyIssue::Duplicate {
                    first: *first.get(),
                }),
                Entry::Vacant(slot) => {
                    // Only a row Save keeps can be the one a later row repeats.
                    if saved_row(entry).is_some() {
                        slot.insert(index);
                    }
                    None
                }
            };
            issue.or_else(|| (!is_well_formed_key(key)).then_some(ClassKeyIssue::Malformed))
        })
        .collect()
}

/// Whether the filter keeps `entry`: its key or label contains `folded`, a
/// query already folded with [`signex_i18n::I18n::fold_for_search`]. An
/// empty query keeps every row.
pub(crate) fn matches_filter(
    i18n: &signex_i18n::I18n,
    entry: &ComponentClassEntry,
    folded: &str,
) -> bool {
    folded.is_empty()
        || i18n.fold_for_search(&entry.key).contains(folded)
        || i18n.fold_for_search(&entry.label).contains(folded)
}

/// The key Add Class generates from a label: lower snake case, every run
/// of other characters one `_`, none at either end ("Transistor — BJT" is
/// `transistor_bjt`). Empty for a label without an ASCII letter or digit.
pub(crate) fn key_from_label(label: &str) -> String {
    let mut key = String::with_capacity(label.len());
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            // A class key the user never reads, not display text.
            // ast-grep-ignore: no-case-in-view
            key.push(c.to_ascii_lowercase());
        } else if !key.is_empty() && !key.ends_with('_') {
            key.push('_');
        }
    }
    let kept = key.trim_end_matches('_').len();
    key.truncate(kept);
    key
}

/// Whether a row's key is still the one generated from its label, so it
/// follows the label; a key typed by hand stops following.
pub(crate) fn key_follows_label(entry: &ComponentClassEntry) -> bool {
    entry.key == key_from_label(&entry.label)
}

/// Where the row at `index` is after the row at `from` moved to `to`.
pub(crate) fn follow_move(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        to
    } else if from < to && index > from && index <= to {
        index - 1
    } else if to < from && index >= to && index < from {
        index + 1
    } else {
        index
    }
}

/// Where the row at `index` is after the row at `removed` was removed;
/// `None` when it was that row.
pub(crate) fn follow_remove(index: usize, removed: usize) -> Option<usize> {
    match index.cmp(&removed) {
        std::cmp::Ordering::Less => Some(index),
        std::cmp::Ordering::Equal => None,
        std::cmp::Ordering::Greater => Some(index - 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(key: &str, label: &str) -> ComponentClassEntry {
        ComponentClassEntry {
            key: key.into(),
            label: label.into(),
        }
    }

    #[test]
    fn a_repeated_key_names_the_first_row_that_has_it() {
        let classes = [
            class("sensor", "Sensor"),
            class("resistor", "Resistor"),
            class(" sensor ", "Hall sensor"),
        ];
        assert_eq!(
            class_key_issues(&classes),
            vec![None, None, Some(ClassKeyIssue::Duplicate { first: 0 })],
            "keys compare trimmed, as Save stores them"
        );
    }

    #[test]
    fn a_label_without_a_key_is_flagged_and_a_blank_new_row_is_not() {
        let classes = [class("", "Ferrite bead"), class("  ", ""), class("", "")];
        assert_eq!(
            class_key_issues(&classes),
            vec![Some(ClassKeyIssue::Missing), None, None]
        );
    }

    #[test]
    fn a_key_that_is_not_lower_snake_case_is_malformed() {
        let classes = [
            class("transistor_bjt2", "BJT"),
            class("Hall Sensor", "Hall sensor"),
            class("ferrite-bead", "Ferrite bead"),
        ];
        assert_eq!(
            class_key_issues(&classes),
            vec![
                None,
                Some(ClassKeyIssue::Malformed),
                Some(ClassKeyIssue::Malformed)
            ]
        );
        assert!(is_well_formed_key("led"));
        assert!(!is_well_formed_key("LED"));
    }

    /// Save and the table read rows the same way: a row Save leaves out
    /// (no label yet) is not the "first" a later row repeats.
    #[test]
    fn save_and_the_table_agree_on_which_rows_count() {
        let classes = [
            class("sensor", ""),
            class(" sensor ", "Hall sensor"),
            class("diode", "Diode"),
            class("diode", "Zener"),
            class("", "No key"),
        ];
        assert_eq!(
            class_key_issues(&classes),
            vec![
                None,
                None,
                None,
                Some(ClassKeyIssue::Duplicate { first: 2 }),
                Some(ClassKeyIssue::Missing)
            ]
        );
        assert_eq!(
            saved_classes(&classes),
            vec![class("sensor", "Hall sensor"), class("diode", "Zener")],
            "trimmed, rows without key or label left out, the later label wins"
        );
    }

    #[test]
    fn the_filter_matches_key_or_label_through_the_casemap_fold() {
        let en = signex_i18n::I18n::en_us();
        let fold = |q: &str| en.fold_for_search(q);
        let bjt = class("transistor_bjt", "Transistor \u{2014} BJT");
        assert!(matches_filter(&en, &bjt, &fold("BJT")));
        assert!(matches_filter(&en, &bjt, &fold("TRANSISTOR_")));
        assert!(!matches_filter(&en, &bjt, &fold("mosfet")));
        assert!(matches_filter(&en, &bjt, ""), "an empty filter keeps all");
    }

    /// Mockup row 150: Add Class generates the key from the label in lower
    /// snake case, the shape of the seed keys.
    #[test]
    fn the_generated_key_is_the_label_in_lower_snake_case() {
        assert_eq!(key_from_label("Ferrite bead"), "ferrite_bead");
        assert_eq!(key_from_label("Transistor \u{2014} BJT"), "transistor_bjt");
        assert_eq!(key_from_label("  Op-Amp (dual) "), "op_amp_dual");
        assert_eq!(key_from_label("2N2222 NPN"), "2n2222_npn");
        assert_eq!(key_from_label("LED"), "led");
        assert_eq!(key_from_label(""), "");
        assert_eq!(key_from_label(" \u{2014} "), "");
        for seed in crate::fonts::default_component_classes() {
            assert!(
                is_well_formed_key(&key_from_label(&seed.label)),
                "{}",
                seed.label
            );
        }
    }

    #[test]
    fn a_key_typed_by_hand_stops_following_the_label() {
        assert!(key_follows_label(&class("", "")));
        assert!(key_follows_label(&class("ferrite_bead", "Ferrite bead")));
        assert!(!key_follows_label(&class("fb", "Ferrite bead")));
    }

    #[test]
    fn a_tracked_row_follows_a_move_and_a_remove() {
        // [a b c d], b (1) moved to 3: [a c d b].
        assert_eq!(follow_move(1, 1, 3), 3);
        assert_eq!(follow_move(2, 1, 3), 1);
        assert_eq!(follow_move(3, 1, 3), 2);
        assert_eq!(follow_move(0, 1, 3), 0);
        // [a b c d], d (3) moved to 0: [d a b c].
        assert_eq!(follow_move(0, 3, 0), 1);
        assert_eq!(follow_move(2, 3, 0), 3);
        assert_eq!(follow_move(3, 3, 0), 0);

        assert_eq!(follow_remove(4, 2), Some(3));
        assert_eq!(follow_remove(2, 2), None);
        assert_eq!(follow_remove(1, 2), Some(1));
    }
}
