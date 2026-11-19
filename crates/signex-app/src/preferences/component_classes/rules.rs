//! The rules behind the Component Classes table, free of any widget: which
//! keys are wrong, which rows a filter keeps, and where a row's index goes
//! when another row moves or is removed.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use crate::fonts::ComponentClassEntry;

/// What is wrong with a row's key. Keys are compared as Save stores them,
/// trimmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClassKeyIssue {
    /// The row has a label but no key: Save leaves such a row out.
    Missing,
    /// An earlier row has the same key; `first` is that row's index. Save
    /// keeps one class per key, so the two rows would become one.
    Duplicate { first: usize },
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
            match first_of.entry(key) {
                Entry::Occupied(first) => Some(ClassKeyIssue::Duplicate {
                    first: *first.get(),
                }),
                Entry::Vacant(slot) => {
                    slot.insert(index);
                    None
                }
            }
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
    fn the_filter_matches_key_or_label_through_the_casemap_fold() {
        let en = signex_i18n::I18n::en_us();
        let fold = |q: &str| en.fold_for_search(q);
        let bjt = class("transistor_bjt", "Transistor \u{2014} BJT");
        assert!(matches_filter(&en, &bjt, &fold("BJT")));
        assert!(matches_filter(&en, &bjt, &fold("TRANSISTOR_")));
        assert!(!matches_filter(&en, &bjt, &fold("mosfet")));
        assert!(matches_filter(&en, &bjt, ""), "an empty filter keeps all");
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
