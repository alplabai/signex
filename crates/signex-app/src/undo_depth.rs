//! The `undo_depth` setting: how many undo steps every history keeps.
//!
//! One number for the schematic engines, the net-class and General project
//! histories, the symbol, footprint and Component row editors and the rule
//! set. It is read from `prefs.json` at start (there is no Preferences row)
//! and from the headless boot option. A value outside
//! `MIN_UNDO_DEPTH..=MAX_UNDO_DEPTH` is never applied as written: it is
//! clamped or replaced by the default, and the change is reported.

use std::path::Path;

use signex_history::{DEFAULT_UNDO_DEPTH, MAX_UNDO_DEPTH, MIN_UNDO_DEPTH, undo_depth_in_range};

/// The key of `prefs.json`.
pub const UNDO_DEPTH_KEY: &str = "undo_depth";

/// How a stored `undo_depth` value was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoDepthRead {
    /// The depth to use: always within `MIN_UNDO_DEPTH..=MAX_UNDO_DEPTH`.
    pub depth: usize,
    /// Why the value was not used as written; `None` when it was. Meant for
    /// the Messages panel and the headless error output.
    pub problem: Option<String>,
}

/// Read the `undo_depth` value of `prefs.json`. A missing key is the default
/// and not a problem. A whole number above [`MAX_UNDO_DEPTH`] is clamped to
/// it; zero and anything that is not a whole number (text, null, negative,
/// fractional) fall back to the default. Both are reported.
pub fn parse_undo_depth(stored: Option<&serde_json::Value>) -> UndoDepthRead {
    match stored {
        None => UndoDepthRead {
            depth: DEFAULT_UNDO_DEPTH,
            problem: None,
        },
        Some(value) => resolve(
            value.as_u64(),
            &value.to_string(),
            "undo_depth in prefs.json",
        ),
    }
}

/// [`parse_undo_depth`] for a command-line or headless option given as text.
pub fn parse_undo_depth_text(text: &str) -> UndoDepthRead {
    resolve(
        text.trim().parse::<u64>().ok(),
        text,
        "the undo depth option",
    )
}

fn resolve(number: Option<u64>, shown: &str, origin: &str) -> UndoDepthRead {
    let bounds = format!("{MIN_UNDO_DEPTH} to {MAX_UNDO_DEPTH}");
    match number.map(|v| usize::try_from(v).unwrap_or(usize::MAX)) {
        Some(depth) if undo_depth_in_range(depth) => UndoDepthRead {
            depth,
            problem: None,
        },
        Some(depth) if depth > MAX_UNDO_DEPTH => UndoDepthRead {
            depth: MAX_UNDO_DEPTH,
            problem: Some(format!(
                "{origin} is {shown}, above {MAX_UNDO_DEPTH}; using {MAX_UNDO_DEPTH}"
            )),
        },
        _ => UndoDepthRead {
            depth: DEFAULT_UNDO_DEPTH,
            problem: Some(format!(
                "{origin} is {shown}, not a whole number from {bounds}; using {DEFAULT_UNDO_DEPTH}"
            )),
        },
    }
}

/// [`parse_undo_depth`] over an already-parsed `prefs.json`; a problem goes
/// to the Messages panel, naming `path`.
pub fn read_undo_depth_pref_from(json: Option<&serde_json::Value>, path: &Path) -> usize {
    let read = parse_undo_depth(json.and_then(|j| j.get(UNDO_DEPTH_KEY)));
    if let Some(problem) = &read.problem {
        crate::diagnostics::log_warning(format!("{}: {problem}", path.display()));
    }
    read.depth
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn depth(stored: serde_json::Value) -> UndoDepthRead {
        parse_undo_depth(Some(&stored))
    }

    #[test]
    fn a_missing_key_is_the_default_and_no_problem() {
        assert_eq!(
            parse_undo_depth(None),
            UndoDepthRead {
                depth: DEFAULT_UNDO_DEPTH,
                problem: None
            }
        );
    }

    #[test]
    fn a_whole_number_in_range_is_used_as_written() {
        for stored in [1, 100, 1000] {
            let read = depth(json!(stored));
            assert_eq!((read.depth, read.problem), (stored, None));
        }
    }

    #[test]
    fn zero_and_non_numbers_fall_back_to_the_default_with_a_problem() {
        for stored in [json!(0), json!(-3), json!(2.5), json!("100"), json!(null)] {
            let read = depth(stored.clone());
            assert_eq!(read.depth, DEFAULT_UNDO_DEPTH, "{stored}");
            assert!(read.problem.is_some(), "{stored}");
        }
    }

    #[test]
    fn an_absurd_depth_is_clamped_with_a_problem() {
        let read = depth(json!(u64::MAX));
        assert_eq!(read.depth, MAX_UNDO_DEPTH);
        assert!(read.problem.is_some());
        let read = depth(json!(1001));
        assert_eq!(read.depth, MAX_UNDO_DEPTH, "1001 is clamped to 1000");
        let problem = read.problem.expect("reported");
        assert!(
            problem.contains("1001") && problem.contains("using 1000"),
            "{problem}"
        );
    }

    #[test]
    fn the_headless_option_follows_the_same_rules() {
        for (text, want, warned) in [
            ("1000", 1000, false),
            ("1", 1, false),
            (" 250 ", 250, false),
            ("1001", 1000, true),
            ("0", DEFAULT_UNDO_DEPTH, true),
            ("-5", DEFAULT_UNDO_DEPTH, true),
            ("many", DEFAULT_UNDO_DEPTH, true),
            ("", DEFAULT_UNDO_DEPTH, true),
        ] {
            let read = parse_undo_depth_text(text);
            assert_eq!(read.depth, want, "{text:?}");
            assert_eq!(read.problem.is_some(), warned, "{text:?}");
        }
    }

    #[test]
    fn a_bad_stored_value_reaches_the_messages_panel() {
        crate::diagnostics::init_logging().ok();
        let before = crate::diagnostics::newest_entry_id();
        let prefs = json!({ "undo_depth": 0 });
        let path = std::path::Path::new("zq-undo-depth-prefs.json");
        assert_eq!(
            read_undo_depth_pref_from(Some(&prefs), path),
            DEFAULT_UNDO_DEPTH
        );
        let delta = crate::diagnostics::entries_since(before);
        assert!(
            delta
                .entries
                .iter()
                .any(|e| e.message.contains("zq-undo-depth-prefs.json")
                    && e.message.contains("undo_depth")),
            "the refusal is logged"
        );
    }
}
