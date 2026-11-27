//! An item's history (mockup row 206, the Item pane): what was done to it,
//! by whom and on which day, oldest first. The edits in `edit` write it; an
//! event a newer build wrote is kept and shown as written.

use serde::{Deserialize, Serialize};

use crate::project::UnknownJson;

/// The most entries an item keeps; past it the oldest go.
pub const HISTORY_LIMIT: usize = 50;

/// Who makes an edit and on which day, as the editor gives them. Both are
/// optional: a run that knows neither still records what was done.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditStamp {
    /// The day, `YYYY-MM-DD`.
    pub day: Option<String>,
    /// Who edits: the git identity in the app.
    pub by: Option<String>,
}

impl EditStamp {
    /// A stamp of `day` by `by`; blank text counts as none.
    pub fn new(day: Option<&str>, by: Option<&str>) -> Self {
        let clean = |text: Option<&str>| {
            text.map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        };
        Self {
            day: clean(day),
            by: clean(by),
        }
    }

    /// A stamp of `day` with no editor named.
    pub fn on(day: &str) -> Self {
        Self::new(Some(day), None)
    }
}

/// What a history entry says was done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HistoryEvent {
    /// The item was made: from a template (the value names it) or added.
    Created,
    /// The item came from another project's checklist (the value names it).
    Imported,
    Title,
    Stage,
    Owner,
    Link,
    Notes,
    /// Moved back to Open.
    Opened,
    Done,
    /// Waived; the value is the reason.
    Waived,
}

impl HistoryEvent {
    pub const ALL: [Self; 10] = [
        Self::Created,
        Self::Imported,
        Self::Title,
        Self::Stage,
        Self::Owner,
        Self::Link,
        Self::Notes,
        Self::Opened,
        Self::Done,
        Self::Waived,
    ];

    /// The event's name in the file.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Imported => "imported",
            Self::Title => "title",
            Self::Stage => "stage",
            Self::Owner => "owner",
            Self::Link => "link",
            Self::Notes => "notes",
            Self::Opened => "open",
            Self::Done => "done",
            Self::Waived => "waived",
        }
    }

    /// The event named `key`.
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|event| event.key() == key)
    }
}

/// One line of an item's history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// What was done, a [`HistoryEvent`] key; kept as text so an event a
    /// newer build wrote still reads.
    pub event: String,
    /// What it was set to, when the event has a value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// The day, `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    /// Keys written by a newer build that this one does not know.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

impl HistoryEntry {
    /// `event` with `value`, stamped with `stamp`.
    pub fn new(event: HistoryEvent, value: Option<String>, stamp: &EditStamp) -> Self {
        Self {
            event: event.key().to_string(),
            value,
            by: stamp.by.clone(),
            on: stamp.day.clone(),
            unknown: UnknownJson::default(),
        }
    }

    /// The event, when this build knows it.
    pub fn event(&self) -> Option<HistoryEvent> {
        HistoryEvent::parse(&self.event)
    }
}

/// Add `entry` to `history`, dropping the oldest past [`HISTORY_LIMIT`].
pub(super) fn record(history: &mut Vec<HistoryEntry>, entry: HistoryEntry) {
    history.push(entry);
    let over = history.len().saturating_sub(HISTORY_LIMIT);
    history.drain(..over);
}
