//! Editing the design checklist (#1291): start it from a template or from
//! another project's items, reset it from its template, add an item, set one
//! field of an item from typed text, move an item to Open, Done or Waived,
//! and remove one. Each change to an item is a line of its history. A
//! refused edit changes nothing and says why. An unreadable stored checklist
//! is never edited over.

use super::history::{EditStamp, HistoryEntry, HistoryEvent, record};
use super::link::ChecklistLink;
use super::{ChecklistItem, ChecklistStage, DesignChecklist, ItemState};
use crate::project::{ProjectData, UnknownJson};

/// A field an editor sets from text, as a command names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChecklistField {
    Title,
    Stage,
    Owner,
    Link,
    Notes,
}

impl ChecklistField {
    pub const ALL: [Self; 5] = [
        Self::Title,
        Self::Stage,
        Self::Owner,
        Self::Link,
        Self::Notes,
    ];

    /// The field's name in a command.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Stage => "stage",
            Self::Owner => "owner",
            Self::Link => "link",
            Self::Notes => "notes",
        }
    }

    /// The field named `key`.
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|field| field.key() == key)
    }

    /// The history event a change of this field writes.
    const fn event(self) -> HistoryEvent {
        match self {
            Self::Title => HistoryEvent::Title,
            Self::Stage => HistoryEvent::Stage,
            Self::Owner => HistoryEvent::Owner,
            Self::Link => HistoryEvent::Link,
            Self::Notes => HistoryEvent::Notes,
        }
    }
}

/// An item a start adds: its stage, its title in the person's words and its
/// link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewChecklistItem {
    pub stage: ChecklistStage,
    pub title: String,
    pub link: Option<ChecklistLink>,
}

/// Why a checklist edit was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChecklistProblem {
    /// The stored checklist cannot be read by this build: it is kept as
    /// written and nothing is edited over it.
    Unreadable,
    /// The project already has a checklist; a start would replace it.
    AlreadyStarted,
    /// The project has no checklist yet: start one first.
    NotStarted,
    /// The checklist was not started from a template, so there is none to
    /// reset from.
    NoTemplate,
    UnknownTemplate(String),
    /// None of the user's saved templates has this name.
    UnknownSavedTemplate(String),
    /// A saved template needs a name.
    EmptyName,
    /// No item has this id.
    UnknownItem(String),
    UnknownStage(String),
    UnknownState(String),
    UnknownField(String),
    UnknownLink(String),
    /// An item needs a title.
    EmptyTitle,
    /// Waived needs a reason.
    NeedsReason,
    /// The checklist could not be stored (it did not encode).
    NotStored(String),
}

impl std::fmt::Display for ChecklistProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable => f.write_str(
                "the project's design checklist cannot be read by this version; it is kept as written",
            ),
            Self::AlreadyStarted => f.write_str("the project already has a design checklist"),
            Self::NotStarted => f.write_str("the project has no design checklist yet"),
            Self::NoTemplate => {
                f.write_str("the design checklist was not started from a template")
            }
            Self::UnknownTemplate(text) => write!(
                f,
                "`{text}` is no checklist template; release_review, prototype_build, blank or user:<name>"
            ),
            Self::UnknownSavedTemplate(name) => {
                write!(f, "no saved checklist template is called `{name}`")
            }
            Self::EmptyName => f.write_str("a checklist template needs a name"),
            Self::UnknownItem(id) => write!(f, "the design checklist has no item `{id}`"),
            Self::UnknownStage(text) => {
                write!(f, "`{text}` is no stage; schematic, layout or fabrication")
            }
            Self::UnknownState(text) => write!(f, "`{text}` is no state; open, done or waived"),
            Self::UnknownField(text) => write!(
                f,
                "`{text}` is no checklist field; title, stage, owner, link or notes"
            ),
            Self::UnknownLink(text) => write!(
                f,
                "`{text}` is no link; erc, drc, title_block, requirement:<id> or object:<what>"
            ),
            Self::EmptyTitle => f.write_str("a checklist item needs a title"),
            Self::NeedsReason => f.write_str("a waived item needs a reason"),
            Self::NotStored(why) => write!(f, "the design checklist was not stored: {why}"),
        }
    }
}

impl std::error::Error for ChecklistProblem {}

/// `text` trimmed; `None` when that leaves nothing.
pub(super) fn optional_text(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Add `items` to `checklist` as new open items, numbered after the ones it
/// has, each with a first history line `made`.
fn fill(
    checklist: &mut DesignChecklist,
    items: Vec<NewChecklistItem>,
    made: &HistoryEntry,
    stamp: &EditStamp,
) -> Result<(), ChecklistProblem> {
    for item in items {
        let title = optional_text(&item.title).ok_or(ChecklistProblem::EmptyTitle)?;
        let id = checklist.next_id();
        checklist.items.push(ChecklistItem {
            id,
            stage: item.stage,
            title,
            state: ItemState::Open,
            reason: None,
            owner: None,
            link: item.link,
            notes: None,
            updated: stamp.day.clone(),
            history: vec![made.clone()],
            unknown: UnknownJson::default(),
        });
    }
    Ok(())
}

impl ProjectData {
    /// The checklist to edit: the stored one, or a new empty one.
    fn checklist_to_edit(&self) -> Result<DesignChecklist, ChecklistProblem> {
        Ok(self.design_checklist()?.unwrap_or_default())
    }

    /// Start the checklist from `template` (its key; `None` for a list of
    /// one's own) with `items`. Refused when the project has one.
    pub fn start_design_checklist(
        &mut self,
        template: Option<&str>,
        items: Vec<NewChecklistItem>,
        stamp: &EditStamp,
    ) -> Result<(), ChecklistProblem> {
        if self.design_checklist()?.is_some() {
            return Err(ChecklistProblem::AlreadyStarted);
        }
        let mut checklist = DesignChecklist {
            template: template.map(str::to_string),
            ..DesignChecklist::default()
        };
        let made = HistoryEntry::new(HistoryEvent::Created, checklist.template.clone(), stamp);
        fill(&mut checklist, items, &made, stamp)?;
        self.store_design_checklist(&checklist)
    }

    /// Start the checklist from the items of another project's checklist,
    /// `from` naming that project; it keeps that checklist's `template`.
    /// Each item comes in open, with its stage, title and link only.
    pub fn import_design_checklist(
        &mut self,
        from: &str,
        template: Option<&str>,
        items: Vec<NewChecklistItem>,
        stamp: &EditStamp,
    ) -> Result<(), ChecklistProblem> {
        if self.design_checklist()?.is_some() {
            return Err(ChecklistProblem::AlreadyStarted);
        }
        let mut checklist = DesignChecklist {
            template: template.map(str::to_string),
            ..DesignChecklist::default()
        };
        let made = HistoryEntry::new(HistoryEvent::Imported, Some(from.to_string()), stamp);
        fill(&mut checklist, items, &made, stamp)?;
        self.store_design_checklist(&checklist)
    }

    /// Put the items of `template` back in place of every item, numbered
    /// from 1 again; keys a newer build wrote on the checklist are kept.
    /// Refused when the project has no checklist.
    pub fn reset_design_checklist(
        &mut self,
        template: &str,
        items: Vec<NewChecklistItem>,
        stamp: &EditStamp,
    ) -> Result<(), ChecklistProblem> {
        let mut checklist = self
            .design_checklist()?
            .ok_or(ChecklistProblem::NotStarted)?;
        checklist.template = Some(template.to_string());
        checklist.items.clear();
        let made = HistoryEntry::new(HistoryEvent::Created, Some(template.to_string()), stamp);
        fill(&mut checklist, items, &made, stamp)?;
        self.store_design_checklist(&checklist)
    }

    /// Add an open item of `stage` titled `title` after the others, and
    /// return its id. A project without a checklist gets one.
    pub fn add_checklist_item(
        &mut self,
        stage: &str,
        title: &str,
        stamp: &EditStamp,
    ) -> Result<String, ChecklistProblem> {
        let mut checklist = self.checklist_to_edit()?;
        let stage = ChecklistStage::parse(stage.trim())
            .ok_or_else(|| ChecklistProblem::UnknownStage(stage.to_string()))?;
        let title = optional_text(title).ok_or(ChecklistProblem::EmptyTitle)?;
        let item = NewChecklistItem {
            stage,
            title,
            link: None,
        };
        let made = HistoryEntry::new(HistoryEvent::Created, None, stamp);
        fill(&mut checklist, vec![item], &made, stamp)?;
        let id = checklist
            .items
            .last()
            .map(|item| item.id.clone())
            .unwrap_or_default();
        self.store_design_checklist(&checklist)?;
        Ok(id)
    }

    /// Set `field` of item `id` from `text`; an empty text clears the owner,
    /// the link and the notes.
    pub fn set_checklist_field(
        &mut self,
        id: &str,
        field: ChecklistField,
        text: &str,
        stamp: &EditStamp,
    ) -> Result<(), ChecklistProblem> {
        let mut checklist = self.checklist_to_edit()?;
        let item = item_mut(&mut checklist, id)?;
        let before = item.clone();
        let value = match field {
            ChecklistField::Title => {
                item.title = optional_text(text).ok_or(ChecklistProblem::EmptyTitle)?;
                Some(item.title.clone())
            }
            ChecklistField::Stage => {
                item.stage = ChecklistStage::parse(text.trim())
                    .ok_or_else(|| ChecklistProblem::UnknownStage(text.to_string()))?;
                Some(item.stage.key().to_string())
            }
            ChecklistField::Owner => {
                item.owner = optional_text(text);
                item.owner.clone()
            }
            ChecklistField::Link => {
                item.link = ChecklistLink::parse(text).map_err(ChecklistProblem::UnknownLink)?;
                item.link.as_ref().map(ChecklistLink::to_text)
            }
            ChecklistField::Notes => {
                item.notes = optional_text(text);
                None
            }
        };
        let line = HistoryEntry::new(field.event(), value, stamp);
        touch(item, &before, stamp, line);
        self.store_design_checklist(&checklist)
    }

    /// Move item `id` to `state`. Waived needs a non-blank `reason`; Open and
    /// Done drop the reason.
    pub fn set_checklist_state(
        &mut self,
        id: &str,
        state: ItemState,
        reason: Option<&str>,
        stamp: &EditStamp,
    ) -> Result<(), ChecklistProblem> {
        let mut checklist = self.checklist_to_edit()?;
        let item = item_mut(&mut checklist, id)?;
        let before = item.clone();
        item.reason = match state {
            ItemState::Waived => Some(
                reason
                    .and_then(optional_text)
                    .ok_or(ChecklistProblem::NeedsReason)?,
            ),
            ItemState::Open | ItemState::Done => None,
        };
        item.state = state;
        let event = match state {
            ItemState::Open => HistoryEvent::Opened,
            ItemState::Done => HistoryEvent::Done,
            ItemState::Waived => HistoryEvent::Waived,
        };
        let line = HistoryEntry::new(event, item.reason.clone(), stamp);
        touch(item, &before, stamp, line);
        self.store_design_checklist(&checklist)
    }

    /// Remove item `id`.
    pub fn remove_checklist_item(&mut self, id: &str) -> Result<(), ChecklistProblem> {
        let mut checklist = self.checklist_to_edit()?;
        let before = checklist.items.len();
        checklist.items.retain(|item| item.id != id);
        if checklist.items.len() == before {
            return Err(ChecklistProblem::UnknownItem(id.to_string()));
        }
        self.store_design_checklist(&checklist)
    }
}

/// The item with id `id`, to edit.
fn item_mut<'c>(
    checklist: &'c mut DesignChecklist,
    id: &str,
) -> Result<&'c mut ChecklistItem, ChecklistProblem> {
    checklist
        .items
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or_else(|| ChecklistProblem::UnknownItem(id.to_string()))
}

/// When the edit changed the item (it was `before`): stamp the day, when
/// the editor gave one, and write `line` to its history. An edit that
/// changed nothing leaves the item as it was.
fn touch(item: &mut ChecklistItem, before: &ChecklistItem, stamp: &EditStamp, line: HistoryEntry) {
    if item == before {
        return;
    }
    if let Some(day) = &stamp.day {
        item.updated = Some(day.clone());
    }
    record(&mut item.history, line);
}
