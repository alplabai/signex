//! The whole-project search: every sheet of the project, open or not, read
//! and searched on a blocking thread so the UI never waits on the disk. The
//! job gets owned data and never logs; the completion handler reports.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_engine::{SimilarValue, find_similar};

use super::super::super::*;
use crate::app::project_sheets::{self, OpenSheets};
use crate::app::state::scope::CaseFold;
use crate::find_similar::{FindSimilarMsg, ProjectMatches};

pub(super) struct SearchInputs {
    pub(super) criteria: Vec<SimilarValue>,
    pub(super) open: OpenSheets,
    pub(super) pages: Vec<PathBuf>,
    pub(super) root: PathBuf,
    pub(super) active: Option<PathBuf>,
    pub(super) fold: CaseFold,
}

pub(super) fn run_search(inputs: SearchInputs) -> ProjectMatches {
    let set = project_sheets::assemble_project_sheets_from(
        &inputs.open,
        &inputs.pages,
        &inputs.root,
        inputs.fold,
    );
    let mut found = ProjectMatches {
        unreadable: set.unreadable.clone(),
        ..ProjectMatches::default()
    };
    for path in project_sheets::ordered_project_sheet_paths(&set, inputs.active.as_deref()) {
        let Some(sheet) = set.sheets.get(&path).or_else(|| inputs.open.get(&path)) else {
            continue;
        };
        found.sheets_searched += 1;
        if inputs.active.as_deref() == Some(path.as_path()) {
            continue;
        }
        let count = find_similar(sheet, &inputs.criteria).len();
        if count > 0 {
            found.elsewhere.push((path, count));
        }
    }
    found
}

impl Signex {
    /// Start the search; `None` when the document is in no project, which is
    /// the cue to search the sheet alone.
    pub(super) fn start_project_search(&mut self) -> Option<Task<Message>> {
        let ds = &self.document_state;
        let root = project_sheets::resolve_netlist_root(ds)?;
        ds.active_document_project()?;
        let state = self.ui_state.find_similar.as_mut()?;
        state.generation += 1;
        state.running = true;
        let generation = state.generation;
        let inputs = SearchInputs {
            criteria: state.criteria(),
            open: project_sheets::open_sheets_snapshot(&self.document_state),
            pages: project_sheets::declared_pages(&self.document_state),
            root,
            active: self.document_state.active_path.clone(),
            fold: crate::app::state::project_case::fold_of(
                self.document_state.active_document_project(),
            ),
        };
        Some(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || run_search(inputs))
                    .await
                    .map_err(|e| format!("Find Similar worker failed: {e}"))
            },
            move |result| {
                Message::Selection(selection_request::SelectionRequest::FindSimilar(
                    FindSimilarMsg::ProjectSearched {
                        generation,
                        result: Arc::new(result),
                    },
                ))
            },
        ))
    }

    /// A finished search: applied unless the dialog was closed or searched
    /// again in the meantime.
    pub(super) fn apply_project_search(
        &mut self,
        generation: u64,
        result: &Result<ProjectMatches, String>,
    ) {
        let current = self
            .ui_state
            .find_similar
            .as_ref()
            .is_some_and(|s| s.generation == generation);
        if !current {
            return;
        }
        match result {
            Ok(found) => self.select_similar_on_this_sheet(Some(found)),
            Err(why) => {
                crate::diagnostics::log_warning(why);
                if let Some(state) = self.ui_state.find_similar.as_mut() {
                    state.running = false;
                }
            }
        }
    }

    /// The per-sheet counts of a project search, one line each in the log.
    pub(super) fn report_other_sheets(&self, found: &ProjectMatches) {
        let dir = self
            .document_state
            .active_document_project()
            .map(|p| p.dir().to_path_buf());
        let total: usize = found.elsewhere.iter().map(|(_, n)| n).sum();
        if total == 0 {
            crate::diagnostics::log_info(format!(
                "Find Similar: no matches on the other {} project sheet(s)",
                found.sheets_searched.saturating_sub(1)
            ));
        } else {
            crate::diagnostics::log_info(format!(
                "Find Similar: {total} more on {} other sheet(s), not selected (selection is per sheet)",
                found.elsewhere.len()
            ));
        }
        for (path, count) in &found.elsewhere {
            let shown = dir
                .as_deref()
                .and_then(|d| path.strip_prefix(d).ok())
                .unwrap_or(path);
            crate::diagnostics::log_info(format!("Find Similar: {count} on {}", shown.display()));
        }
        for (path, why) in &found.unreadable {
            crate::diagnostics::log_warning(format!(
                "Find Similar: not searched, {why}: {}",
                path.display()
            ));
        }
    }
}
