//! "Did you mean": the catalog ids closest to an unknown one (ADR-0006
//! slice 7), for the agent reply and the keymap-profile warning.

use super::all_command_ids;

/// How many close ids an unknown id is answered with, at most.
const MAX_CLOSE_IDS: usize = 3;

/// The largest edit distance still offered: a typo or two, not another
/// command.
const MAX_DISTANCE: usize = 3;

/// Up to [`MAX_CLOSE_IDS`] catalog ids within [`MAX_DISTANCE`] edits of
/// `unknown`, nearest first, ties by id.
pub fn closest_command_ids(unknown: &str) -> Vec<&'static str> {
    let mut close: Vec<(usize, &'static str)> = all_command_ids()
        .map(|id| (edit_distance(unknown, id), id))
        .filter(|(distance, _)| *distance <= MAX_DISTANCE)
        .collect();
    close.sort_unstable();
    close
        .into_iter()
        .take(MAX_CLOSE_IDS)
        .map(|(_, id)| id)
        .collect()
}

/// The Levenshtein distance between `a` and `b`, by characters.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitute = diagonal + usize::from(ca != *cb);
            diagonal = row[j + 1];
            row[j + 1] = substitute.min(row[j] + 1).min(diagonal + 1);
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_distance_counts_inserts_deletes_and_substitutions() {
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("abc", ""), 3);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("undo", "undo"), 0);
        assert_eq!(edit_distance("udno", "undo"), 2);
    }

    #[test]
    fn a_typo_is_answered_with_the_nearest_ids_first() {
        let close = closest_command_ids("plce_wire");
        assert_eq!(close.first(), Some(&"place_wire"), "{close:?}");
        assert!(close.len() <= MAX_CLOSE_IDS);
        assert_eq!(closest_command_ids("redo"), ["redo", "undo"]);
    }

    #[test]
    fn an_id_far_from_every_command_gets_no_suggestion() {
        assert!(closest_command_ids("zzzzzzzzzzzzzzzz").is_empty());
    }
}
