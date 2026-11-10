//! The per-project settings of the agent journals (#2253): whether they are
//! kept in git, whether a save commit summarises agent edits, and how many
//! lines each journal keeps. Stored in the `.snxprj` as one optional
//! `agent_journal` object; the journals themselves are in `signex-app`.
//! See `docs/AGENT_JOURNALS.md`.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// How many lines each journal keeps unless the project says otherwise.
pub const DEFAULT_JOURNAL_MAX_LINES: u32 = 10_000;
/// The fewest lines a journal may be set to keep.
pub const MIN_JOURNAL_MAX_LINES: u32 = 100;
/// The most lines a journal may be set to keep.
pub const MAX_JOURNAL_MAX_LINES: u32 = 1_000_000;

/// The `.snxprj` key of [`AgentJournalSettings`].
pub const AGENT_JOURNAL_KEY: &str = "agent_journal";

/// What a project asks of its agent journals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentJournalSettings {
    /// Keep `.signex/agent-*.jsonl` in git. When off, the managed `.gitignore`
    /// block lists them. Turning it off never untracks a journal already
    /// committed.
    pub keep_in_git: bool,
    /// The save commit's body lists the agent edits since the last commit
    /// (stored here; the commit message uses it in a later change).
    pub summarise_in_commit: bool,
    /// The most lines each journal keeps; older lines are dropped.
    #[serde(deserialize_with = "clamped_max_lines")]
    pub max_lines: u32,
}

impl Default for AgentJournalSettings {
    fn default() -> Self {
        Self {
            keep_in_git: true,
            summarise_in_commit: false,
            max_lines: DEFAULT_JOURNAL_MAX_LINES,
        }
    }
}

impl AgentJournalSettings {
    /// Is this what an untouched project has? Such a project saves without
    /// the key, byte-identical to a file an older build wrote.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// These settings with `max_lines` brought into its allowed range.
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            max_lines: self
                .max_lines
                .clamp(MIN_JOURNAL_MAX_LINES, MAX_JOURNAL_MAX_LINES),
            ..self
        }
    }
}

fn clamped_max_lines<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(value
        .as_u64()
        .map_or(DEFAULT_JOURNAL_MAX_LINES, |lines| {
            u32::try_from(lines).unwrap_or(MAX_JOURNAL_MAX_LINES)
        })
        .clamp(MIN_JOURNAL_MAX_LINES, MAX_JOURNAL_MAX_LINES))
}

/// Reads a stored `agent_journal`; a value of another shape reads as the
/// defaults, so a hand-edited key never makes the project unopenable.
pub(crate) fn lenient_settings<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<AgentJournalSettings, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_git_on_summary_off_ten_thousand_lines() {
        let settings = AgentJournalSettings::default();
        assert!(settings.keep_in_git);
        assert!(!settings.summarise_in_commit);
        assert_eq!(settings.max_lines, 10_000);
        assert!(settings.is_default());
    }

    #[test]
    fn a_partial_object_fills_the_rest_with_defaults() {
        let read: AgentJournalSettings = serde_json::from_str(r#"{"keep_in_git":false}"#).unwrap();
        assert!(!read.keep_in_git);
        assert_eq!(read.max_lines, DEFAULT_JOURNAL_MAX_LINES);
    }

    #[test]
    fn the_line_cap_is_kept_inside_its_range_whatever_was_stored() {
        for (stored, want) in [
            ("5", MIN_JOURNAL_MAX_LINES),
            ("99999999999", MAX_JOURNAL_MAX_LINES),
            ("\"many\"", DEFAULT_JOURNAL_MAX_LINES),
            ("-3", DEFAULT_JOURNAL_MAX_LINES),
            ("250", 250),
        ] {
            let read: AgentJournalSettings =
                serde_json::from_str(&format!(r#"{{"max_lines":{stored}}}"#)).unwrap();
            assert_eq!(read.max_lines, want, "stored {stored}");
        }
    }
}
