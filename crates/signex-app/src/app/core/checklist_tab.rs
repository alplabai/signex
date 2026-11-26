//! The Design Checklist tab's own state (#1291, mockup row 206): the picked
//! template, the filter, the selected item, what is typed and not committed,
//! the open menus, the user's saved templates as last read, the last refusal
//! or notice and the last run of the linked checks. Plain state, UI-free
//! (ADR-0006). Not project data: it is never saved.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use signex_types::check::CheckReport;
use signex_types::project::{
    ChecklistField, ChecklistItem, ChecklistProblem, ItemState, SavedTemplate, TemplateRef,
};

/// One run of the linked checks: what it found and when it finished.
#[derive(Debug, Clone)]
pub struct ChecklistRun {
    pub report: Result<CheckReport, String>,
    pub finished_at: DateTime<Utc>,
}

/// Which items the list shows (the filter chips).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChecklistFilter {
    #[default]
    All,
    /// Only items in this state.
    State(ItemState),
    /// Only the items the user owns.
    Mine,
}

impl ChecklistFilter {
    /// The filter's name in a command.
    pub const fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::State(state) => state.key(),
            Self::Mine => "mine",
        }
    }

    /// The filter named `key`.
    pub fn parse(key: &str) -> Option<Self> {
        match key.trim() {
            "all" => Some(Self::All),
            "mine" => Some(Self::Mine),
            other => ItemState::parse(other).map(Self::State),
        }
    }

    /// Does `item` pass, `me` being the user's name?
    pub fn shows(self, item: &ChecklistItem, me: &str) -> bool {
        match self {
            Self::All => true,
            Self::State(state) => item.state == state,
            Self::Mine => item.is_owned_by(me),
        }
    }
}

/// The user's saved templates as the tab last read them.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum SavedTemplates {
    #[default]
    NotRead,
    Read(Vec<SavedTemplate>),
    /// The library file could not be read; why.
    Unreadable(String),
}

impl SavedTemplates {
    /// The templates read, none before a read or after a failed one.
    pub fn list(&self) -> &[SavedTemplate] {
        match self {
            Self::Read(list) => list,
            Self::NotRead | Self::Unreadable(_) => &[],
        }
    }
}

/// What the tab says after an action that is not an edit of the checklist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChecklistNotice {
    /// Save as template wrote the template of this name.
    TemplateSaved(String),
    /// Save as template failed; why.
    TemplateNotSaved(String),
    /// The library file could not be read; why.
    LibraryUnreadable(String),
    /// Import from another project could not read that project; why.
    ImportFailed(String),
    /// The project picked to import from has no design checklist.
    NothingToImport(String),
    /// A design-object link names nothing in the project's open documents.
    LinkNotFound(String),
}

/// The tab's state, one per project.
#[derive(Debug, Clone, Default)]
pub struct ChecklistTabState {
    /// The template the empty tab's Start uses; `None` is Release review.
    pub template: Option<TemplateRef>,
    pub filter: ChecklistFilter,
    /// The filter text over titles, owners and links.
    pub search: String,
    /// The item the pane shows, by id.
    pub selected: Option<String>,
    /// Fields as typed, by item id and field, until Enter commits them.
    pub drafts: HashMap<(String, ChecklistField), String>,
    /// The reason typed after Waived…, with the item it waives.
    pub waiving: Option<(String, String)>,
    /// The bar's More menu is open.
    pub more_open: bool,
    /// The name typed for Save as template, while its box is open.
    pub template_name: Option<String>,
    /// The user's saved templates.
    pub saved: SavedTemplates,
    /// The item whose link picker is open, by id.
    pub link_picker: Option<String>,
    /// The link picker's filter text.
    pub link_query: String,
    /// Why the last edit was refused; cleared by the next edit.
    pub problem: Option<ChecklistProblem>,
    /// What the last action that was not an edit came to.
    pub notice: Option<ChecklistNotice>,
    pub running: bool,
    pub last_run: Option<Arc<ChecklistRun>>,
}

impl ChecklistTabState {
    /// Forget what was typed: the boxes show the stored values again.
    pub(crate) fn clear_drafts(&mut self) {
        self.drafts.clear();
        self.waiving = None;
        self.template_name = None;
        self.problem = None;
    }

    /// The last finished run's report, when it finished.
    pub(crate) fn report(&self) -> Option<&CheckReport> {
        self.last_run
            .as_deref()
            .and_then(|run| run.report.as_ref().ok())
    }
}
