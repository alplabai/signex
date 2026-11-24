//! "Did you mean": the catalog ids closest to an unknown one (ADR-0006
//! slice 7), for the agent reply and the keymap-profile warning.

use super::all_command_ids;

/// How many close ids an unknown id is answered with, at most.
const MAX_CLOSE_IDS: usize = 3;

/// The largest edit distance still offered: a typo or two, not another
/// command.
const MAX_DISTANCE: usize = 3;

/// An id longer than this is answered with nothing, before any distance is
/// computed: no catalog id comes near, and an agent may send up to 1 MiB.
const MAX_UNKNOWN_CHARS: usize = 64;

/// Up to [`MAX_CLOSE_IDS`] catalog ids within [`max_distance`] edits of
/// `unknown`, nearest first, ties by id.
pub fn closest_command_ids(unknown: &str) -> Vec<&'static str> {
    let len = unknown.chars().take(MAX_UNKNOWN_CHARS + 1).count();
    if len > MAX_UNKNOWN_CHARS {
        return Vec::new();
    }
    let limit = max_distance(len);
    let mut close: Vec<(usize, &'static str)> = all_command_ids()
        .filter(|id| id.chars().count().abs_diff(len) <= limit)
        .map(|id| (edit_distance(unknown, id), id))
        .filter(|(distance, _)| *distance <= limit)
        .collect();
    close.sort_unstable();
    close
        .into_iter()
        .take(MAX_CLOSE_IDS)
        .map(|(_, id)| id)
        .collect()
}

/// The edits allowed for an id of `len` characters: half of it, at most
/// [`MAX_DISTANCE`], so `x` or an empty id is not "close" to `cut`.
fn max_distance(len: usize) -> usize {
    (len / 2).min(MAX_DISTANCE)
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

    /// `x` is three edits from `cut`; a short id may only miss by half of
    /// itself.
    #[test]
    fn a_short_id_gets_no_unrelated_suggestion() {
        for short in ["", "x", "zz", "abc"] {
            assert!(closest_command_ids(short).is_empty(), "{short:?}");
        }
        assert_eq!(closest_command_ids("redo_"), ["redo"]);
    }

    /// An agent may send an id of up to 1 MiB: it is answered at once, with
    /// nothing, instead of a distance to every catalog id.
    #[test]
    fn a_huge_id_is_answered_at_once_with_nothing() {
        let huge = "a".repeat(1 << 20);
        let started = std::time::Instant::now();
        assert!(closest_command_ids(&huge).is_empty());
        assert!(
            started.elapsed() < std::time::Duration::from_millis(100),
            "{:?}",
            started.elapsed()
        );
    }
}
