//! The per-project settings of the agent journals (#2253): whether they are
//! kept in git, whether a save commit summarises agent edits, and how many
//! lines each journal keeps. Stored in the `.snxprj` as one optional
//! `agent_journal` object; the journals themselves are in `signex-app`.
//! See `docs/AGENT_JOURNALS.md`.

use serde::{Deserialize, Serialize};
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

/// The keys this build knows inside `agent_journal`.
const KNOWN_KEYS: [&str; 3] = ["keep_in_git", "summarise_in_commit", "max_lines"];

/// Read a stored `agent_journal`. The keys this build knows are read strictly
/// (a wrong type, such as `"max_lines": "5000"`, is a refusal); keys it does not
/// know are kept in `unknown` and written back with the settings. A value it
/// cannot read never makes the project unopenable: the defaults apply, a
/// warning says so (Messages), and the raw value is kept in `unknown` so a save
/// writes it back untouched until the settings are edited.
pub(crate) fn take_agent_journal(data: &mut crate::project::ProjectData, raw: Value) {
    let unreadable = |data: &mut crate::project::ProjectData, why: String, raw: Value| {
        tracing::warn!(
            target: "signex::project",
            "The project's agent journal settings are not readable ({why}); the defaults \
             apply and the stored value is kept unchanged until you edit them."
        );
        data.unknown.insert(AGENT_JOURNAL_KEY.to_string(), raw);
    };
    let Value::Object(map) = &raw else {
        unreadable(data, "not an object".into(), raw);
        return;
    };
    let (known, extras): (
        serde_json::Map<String, Value>,
        serde_json::Map<String, Value>,
    ) = map
        .clone()
        .into_iter()
        .partition(|(key, _)| KNOWN_KEYS.contains(&key.as_str()));
    match serde_json::from_value::<AgentJournalSettings>(Value::Object(known)) {
        Ok(settings) => {
            data.agent_journal = settings.clamped();
            if !extras.is_empty() {
                data.unknown
                    .insert(AGENT_JOURNAL_KEY.to_string(), Value::Object(extras));
            }
        }
        Err(why) => unreadable(data, why.to_string(), raw),
    }
}

/// The value written for a project whose settings are not the defaults: the
/// settings, with the keys a newer build stored beside them.
pub(crate) fn merged_value(data: &crate::project::ProjectData) -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(data.agent_journal)?;
    if let (Value::Object(map), Some(Value::Object(extras))) =
        (&mut value, data.unknown.get(AGENT_JOURNAL_KEY))
    {
        for (key, extra) in extras {
            if !KNOWN_KEYS.contains(&key.as_str()) {
                map.entry(key.clone()).or_insert_with(|| extra.clone());
            }
        }
    }
    Ok(value)
}

impl crate::project::ProjectData {
    /// The file has an `agent_journal` value this build could not read: kept
    /// verbatim for re-save, the defaults apply. (Keys only a newer build
    /// knows, beside readable settings, are not that.)
    pub fn has_unreadable_agent_journal(&self) -> bool {
        match self.unknown.get(AGENT_JOURNAL_KEY) {
            Some(Value::Object(map)) => KNOWN_KEYS.iter().any(|key| map.contains_key(*key)),
            Some(_) => true,
            None => false,
        }
    }

    /// Store `settings` (kept in range). An unreadable stored value is
    /// replaced by them; keys a newer build stored beside them stay.
    pub fn set_agent_journal(&mut self, settings: AgentJournalSettings) {
        self.agent_journal = settings.clamped();
        if self.has_unreadable_agent_journal() {
            self.unknown.remove(AGENT_JOURNAL_KEY);
        }
    }
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
    fn the_line_cap_is_clamped_into_its_range() {
        for (lines, want) in [
            (5, MIN_JOURNAL_MAX_LINES),
            (4_000_000_000, MAX_JOURNAL_MAX_LINES),
            (250, 250),
        ] {
            let settings = AgentJournalSettings {
                max_lines: lines,
                ..AgentJournalSettings::default()
            };
            assert_eq!(settings.clamped().max_lines, want, "{lines}");
        }
    }
}
