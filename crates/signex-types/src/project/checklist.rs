//! The project's design checklist (#1291, mockup row 206): the review items
//! to close before a release, grouped by stage, each Open, Done or Waived
//! with a reason, with an owner, notes and an optional link to a check or a
//! design object. Stored under the `design_checklist` key of the `.snxprj`
//! and read from the project's key bag on demand, so an untouched section is
//! written back exactly as it was read; keys this build does not know (on
//! the checklist or an item) are kept on every edit. No format token
//! changes: a build that predates the checklist keeps the key as it is.
//! Rules: `docs/DESIGN_CHECKLIST.md`.

mod edit;
mod link;
mod template;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use edit::{ChecklistField, ChecklistProblem, NewChecklistItem};
pub use link::{ChecklistLink, LinkResult, TITLE_BLOCK_REQUIRED, title_block_missing};
pub use template::{CHECKLIST_TEMPLATES, ChecklistTemplate, TemplateItem, TemplateLink};

use super::{ProjectData, UnknownJson};

/// The `.snxprj` key of the checklist.
pub const CHECKLIST_KEY: &str = "design_checklist";

/// The stage an item belongs to, in the order the tab shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChecklistStage {
    Schematic,
    Layout,
    Fabrication,
}

impl ChecklistStage {
    pub const ALL: [Self; 3] = [Self::Schematic, Self::Layout, Self::Fabrication];

    /// The stage's name in the file and in a command.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Schematic => "schematic",
            Self::Layout => "layout",
            Self::Fabrication => "fabrication",
        }
    }

    /// The stage named `key`.
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|stage| stage.key() == key)
    }
}

/// Where an item stands. A waived item counts as closed and keeps its
/// reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    #[default]
    Open,
    Done,
    Waived,
}

impl ItemState {
    pub const ALL: [Self; 3] = [Self::Open, Self::Done, Self::Waived];

    /// The state's name in the file and in a command.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Done => "done",
            Self::Waived => "waived",
        }
    }

    /// The state named `key`.
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.key() == key)
    }

    /// Done or waived: no longer to do.
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Done | Self::Waived)
    }
}

/// One review item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChecklistItem {
    /// Unique within the checklist; a command names the item by it.
    pub id: String,
    pub stage: ChecklistStage,
    pub title: String,
    #[serde(default)]
    pub state: ItemState,
    /// Why the item was waived; only a waived item has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<ChecklistLink>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// The day of the last edit, `YYYY-MM-DD`, as the editor gave it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    /// Keys written by a newer build that this one does not know.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

/// The checklist: the template it started from and its items in list order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DesignChecklist {
    /// The key of the template it started from (`release_review`); `None`
    /// for one started from an added item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(default)]
    pub items: Vec<ChecklistItem>,
    /// Keys written by a newer build that this one does not know.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

/// How many items of a set are in each state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StateCounts {
    pub open: usize,
    pub done: usize,
    pub waived: usize,
}

impl StateCounts {
    /// Every item counted.
    pub const fn total(self) -> usize {
        self.open + self.done + self.waived
    }

    /// Done or waived.
    pub const fn closed(self) -> usize {
        self.done + self.waived
    }
}

impl DesignChecklist {
    /// The item with id `id`.
    pub fn item(&self, id: &str) -> Option<&ChecklistItem> {
        self.items.iter().find(|item| item.id == id)
    }

    /// The counts of the items of `stage`, or of every item when `None`.
    pub fn counts(&self, stage: Option<ChecklistStage>) -> StateCounts {
        let mut counts = StateCounts::default();
        let items = self
            .items
            .iter()
            .filter(|item| stage.is_none_or(|stage| item.stage == stage));
        for item in items {
            match item.state {
                ItemState::Open => counts.open += 1,
                ItemState::Done => counts.done += 1,
                ItemState::Waived => counts.waived += 1,
            }
        }
        counts
    }

    /// The id a new item gets: one above the highest number in use.
    pub(crate) fn next_id(&self) -> String {
        let highest = self
            .items
            .iter()
            .filter_map(|item| item.id.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        (highest + 1).to_string()
    }
}

impl ProjectData {
    /// The project's design checklist: `Ok(None)` when it has none, refused
    /// when the stored value cannot be read by this build. Such a value is
    /// kept as written and never replaced.
    pub fn design_checklist(&self) -> Result<Option<DesignChecklist>, ChecklistProblem> {
        match self.unknown.get(CHECKLIST_KEY) {
            None => Ok(None),
            Some(raw) => serde_json::from_value::<DesignChecklist>(raw.clone())
                .map(Some)
                .map_err(|_| ChecklistProblem::Unreadable),
        }
    }

    /// The stored checklist value cannot be read by this build.
    pub fn has_unreadable_checklist(&self) -> bool {
        self.design_checklist().is_err()
    }

    /// The checklist's stored value, for an undo snapshot.
    pub fn design_checklist_value(&self) -> Option<&Value> {
        self.unknown.get(CHECKLIST_KEY)
    }

    /// Put a value taken by [`Self::design_checklist_value`] back.
    pub fn restore_design_checklist_value(&mut self, value: Option<Value>) {
        match value {
            Some(value) => self.unknown.insert(CHECKLIST_KEY.into(), value),
            None => self.unknown.remove(CHECKLIST_KEY),
        }
    }

    /// Store `checklist` in place of the one held.
    pub(crate) fn store_design_checklist(
        &mut self,
        checklist: &DesignChecklist,
    ) -> Result<(), ChecklistProblem> {
        let value = serde_json::to_value(checklist)
            .map_err(|error| ChecklistProblem::NotStored(error.to_string()))?;
        self.unknown.insert(CHECKLIST_KEY.into(), value);
        Ok(())
    }
}
