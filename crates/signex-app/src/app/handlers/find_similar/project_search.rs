//! The whole-project search: every sheet of the project, open or not, read
//! and searched on a blocking thread so the UI never waits on the disk. The
//! job gets owned data and never logs; the completion handler reports.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_engine::{SimilarValue, describe, find_similar};
use signex_i18n::{ids, t};

use super::super::super::*;
use crate::app::project_sheets::{self, OpenSheets};
use crate::app::state::scope::CaseFold;
use crate::find_similar::{FindSimilarMsg, ProjectMatches, SimilarMatch, shown_seed};

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
        let items = find_similar(sheet, &inputs.criteria);
        if items.is_empty() {
            continue;
        }
        found.elsewhere.push((path.clone(), items.len()));
        found
            .listed
            .extend(items.into_iter().map(|item| SimilarMatch {
                sheet: path.clone(),
                described: describe(sheet, &item),
                item,
            }));
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
        self.ui_state.find_similar_generation += 1;
        let generation = self.ui_state.find_similar_generation;
        let state = self.ui_state.find_similar.as_mut()?;
        state.running = true;
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

    /// Whether the active document is in a project a search can reach.
    pub(super) fn in_a_project(&self) -> bool {
        let ds = &self.document_state;
        project_sheets::resolve_netlist_root(ds).is_some() && ds.active_document_project().is_some()
    }

    /// A finished count: shown in the dialog unless the dialog was closed,
    /// or its criteria changed, in the meantime. Select N reports it.
    pub(super) fn apply_project_search(
        &mut self,
        generation: u64,
        result: &Result<ProjectMatches, String>,
    ) {
        self.close_find_similar_if_sheet_changed();
        if generation != self.ui_state.find_similar_generation {
            return;
        }
        let Some(state) = self.ui_state.find_similar.as_mut() else {
            return;
        };
        state.running = false;
        match result {
            Ok(found) => state.other_sheets = Some(found.clone()),
            Err(why) => crate::diagnostics::log_warning(why),
        }
    }

    /// What a project search found elsewhere: the total, then the matches,
    /// each on its own Messages row, which names its sheet and object so the
    /// row's Show opens it (mockup row 68).
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
        self.list_other_sheet_matches(found, dir.as_deref());
        for (path, why) in &found.unreadable {
            crate::diagnostics::log_warning(format!(
                "Find Similar: not searched, {why}: {}",
                path.display()
            ));
        }
    }

    /// One Messages row per match, up to [`LISTED_MATCHES_CAP`], then one
    /// line with the count left out, so a large search cannot push the
    /// summary and older lines (ERC results) out of the ring.
    fn list_other_sheet_matches(&self, found: &ProjectMatches, dir: Option<&std::path::Path>) {
        let i18n = &self.ui_state.i18n;
        for listed in found.listed.iter().take(LISTED_MATCHES_CAP) {
            let shown = dir
                .and_then(|d| listed.sheet.strip_prefix(d).ok())
                .unwrap_or(&listed.sheet);
            let line = t!(
                i18n,
                ids::FIND_SIMILAR_LISTED,
                "what" => shown_seed(&listed.described, i18n),
                "sheet" => shown.display().to_string()
            );
            crate::diagnostics::log_info_at(
                module_path!(),
                line.into_string(),
                listed.sheet.clone(),
                Some(listed.item),
            );
        }
        let left_out = found.listed.len().saturating_sub(LISTED_MATCHES_CAP);
        if left_out > 0 {
            let line = t!(i18n, ids::FIND_SIMILAR_NOT_LISTED, "count" => left_out);
            crate::diagnostics::log_info(line.into_string());
        }
    }
}

/// The most matches on other sheets listed in Messages: a quarter of the
/// Messages ring, so a listing never evicts the rest of the panel.
pub(super) const LISTED_MATCHES_CAP: usize = crate::diagnostics::MAX_DIAGNOSTIC_ENTRIES / 4;
