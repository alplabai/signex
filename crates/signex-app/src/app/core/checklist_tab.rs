//! The Design Checklist tab's own state (#1291, mockup row 206): the picked
//! template, the filter, the selected item, what is typed and not committed,
//! the last refusal and the last run of the linked checks. Plain state,
//! UI-free (ADR-0006). Not project data: it is never saved.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use signex_types::check::CheckReport;
use signex_types::project::{ChecklistField, ChecklistProblem, ChecklistTemplate, ItemState};

/// One run of the linked checks: what it found and when it finished.
#[derive(Debug, Clone)]
pub struct ChecklistRun {
    pub report: Result<CheckReport, String>,
    pub finished_at: DateTime<Utc>,
}

/// The tab's state, one per project.
#[derive(Debug, Clone, Default)]
pub struct ChecklistTabState {
    /// The template the empty tab's Start uses; `None` is Release review.
    pub template: Option<ChecklistTemplate>,
    /// Only items in this state; `None` shows all.
    pub filter: Option<ItemState>,
    /// The filter text over titles, owners and links.
    pub search: String,
    /// The item the pane shows, by id.
    pub selected: Option<String>,
    /// Fields as typed, by item id and field, until Enter commits them.
    pub drafts: HashMap<(String, ChecklistField), String>,
    /// The reason typed after Waived…, with the item it waives.
    pub waiving: Option<(String, String)>,
    /// Why the last edit was refused; cleared by the next edit.
    pub problem: Option<ChecklistProblem>,
    pub running: bool,
    pub last_run: Option<Arc<ChecklistRun>>,
}

impl ChecklistTabState {
    /// Forget what was typed: the boxes show the stored values again.
    pub(crate) fn clear_drafts(&mut self) {
        self.drafts.clear();
        self.waiving = None;
        self.problem = None;
    }

    /// The last finished run's report, when it finished.
    pub(crate) fn report(&self) -> Option<&CheckReport> {
        self.last_run
            .as_deref()
            .and_then(|run| run.report.as_ref().ok())
    }
}
