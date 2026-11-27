//! Run checks (mockup row 206): the checks the items link to, run through
//! the requirement checks off the UI thread over the project as it is in
//! memory. The link card's Run check runs one item's check and keeps what
//! the last run said of the others. Nothing is written and nothing reaches
//! Messages.

use std::sync::Arc;

use chrono::Utc;
use iced::Task;
use serde_json::json;
use signex_types::project::{ChecklistLink, ProjectData, RequirementEntry, merge_link_run};

use super::{ChecklistMsg, ChecklistRun, message};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

/// The links of `data`'s checklist a run checks: every item's, or only item
/// `only`'s.
fn run_links(data: &ProjectData, only: Option<&str>) -> Vec<ChecklistLink> {
    let Ok(Some(checklist)) = data.design_checklist() else {
        return Vec::new();
    };
    checklist
        .items
        .into_iter()
        .filter(|item| only.is_none_or(|id| item.id == id))
        .filter_map(|item| item.link)
        .collect()
}

/// The requirement entries a run checks for `data`'s checklist (or item
/// `only` of it): the project's requirements an item links to, and the
/// checklist's own ERC and DRC checks when an item links to them (errors
/// fail them, warnings do not).
pub(crate) fn run_entries(data: &ProjectData, only: Option<&str>) -> Vec<RequirementEntry> {
    let links = run_links(data, only);
    let mut entries: Vec<RequirementEntry> = data
        .requirements
        .iter()
        .filter(|entry| {
            links.iter().any(|link| {
                matches!(link, ChecklistLink::Requirement { id } if entry.id() == Some(id.as_str()))
            })
        })
        .cloned()
        .collect();
    for (link, kind) in [
        (ChecklistLink::Erc, "erc_clean"),
        (ChecklistLink::Drc, "drc_clean"),
    ] {
        if links.contains(&link)
            && let Some(id) = link.run_id()
        {
            entries.push(RequirementEntry::read(
                json!({"id": id, "kind": kind, "allow_warnings": true}),
            ));
        }
    }
    entries
}

/// The run ids item `item`'s link is checked under.
fn item_run_ids(data: &ProjectData, item: &str) -> Vec<String> {
    run_links(data, Some(item))
        .iter()
        .filter_map(|link| link.run_id().map(str::to_string))
        .collect()
}

impl Signex {
    /// Run checks (`only`: the link card's Run check of that item): the
    /// checks the items link to, over the project as it is in memory, off
    /// the UI thread; the tab shows the result when it lands. A run already
    /// going keeps going and this one is not started.
    pub(crate) fn run_checklist_in_tab(
        &mut self,
        project_id: ProjectId,
        only: Option<String>,
    ) -> Task<Message> {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return Task::none();
        };
        if self
            .checklist_tab_ref(project_id)
            .is_some_and(|tab| tab.running)
        {
            return Task::none();
        }
        let entries = run_entries(&project.data, only.as_deref());
        let run_ids = only
            .as_deref()
            .map(|item| item_run_ids(&project.data, item));
        let job = self.requirements_job_over(project, entries, false);
        self.checklist_tab(project_id).running = true;
        Task::perform(
            async move {
                let ran = tokio::task::spawn_blocking(job)
                    .await
                    .map_err(|error| format!("the check did not finish: {error}"));
                ChecklistRun {
                    report: ran.map(|(report, _)| report),
                    finished_at: Utc::now(),
                }
            },
            move |run| {
                let msg = ChecklistMsg::RunFinished {
                    run: Arc::new(run),
                    only: run_ids,
                };
                message(project_id, msg)
            },
        )
    }

    /// A run landed: it is the tab's last run; a one-item run keeps what the
    /// last run said of the other links.
    pub(super) fn checklist_run_finished(
        &mut self,
        project_id: ProjectId,
        run: Arc<ChecklistRun>,
        only: Option<Vec<String>>,
    ) {
        let tab = self.checklist_tab(project_id);
        tab.running = false;
        let merged = match (&only, run.report.as_ref()) {
            (Some(ids), Ok(report)) => Some(ChecklistRun {
                report: Ok(merge_link_run(tab.report(), report.clone(), ids)),
                finished_at: run.finished_at,
            }),
            _ => None,
        };
        tab.last_run = Some(merged.map(Arc::new).unwrap_or(run));
    }
}
