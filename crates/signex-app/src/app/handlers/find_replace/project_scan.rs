//! The project-scope scan: every sheet of the project, open or not, searched
//! on a blocking thread so the UI never waits on the disk.
//!
//! The job gets plain owned data (a snapshot of the open sheets' documents,
//! the page list, a copy of the hierarchy) and never logs; the completion
//! handler replays what the job found to the Messages panel.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_engine::DesignatorScope;

use super::super::super::*;
use crate::app::project_sheets::{self, OpenSheets};
use crate::app::sheet_hierarchy::SheetHierarchy;
use crate::app::state::scope::CaseFold;
use crate::find_replace::{FindMatch, FindReplaceMsg, ProjectScan};

/// Everything the scan needs, captured from `update`.
pub(super) struct ScanInputs {
    pub(super) query: String,
    pub(super) open: OpenSheets,
    pub(super) pages: Vec<PathBuf>,
    pub(super) root: PathBuf,
    pub(super) active: Option<PathBuf>,
    pub(super) fold: CaseFold,
    pub(super) hierarchy: Option<SheetHierarchy>,
}

/// The designators a sheet's hits are read at: every placement the walk
/// reaches it through, and whether that list is certain to be complete.
pub(super) fn designator_scope_of(
    hierarchy: Option<&SheetHierarchy>,
    path: &std::path::Path,
) -> DesignatorScope {
    hierarchy.map_or_else(DesignatorScope::default, |h| h.designator_scope(path))
}

fn run_scan(inputs: ScanInputs) -> ProjectScan {
    let set = project_sheets::assemble_project_sheets_from(
        &inputs.open,
        &inputs.pages,
        &inputs.root,
        inputs.fold,
    );
    let mut scan = ProjectScan {
        unreadable: set.unreadable.clone(),
        ..ProjectScan::default()
    };
    for path in project_sheets::ordered_project_sheet_paths(&set, inputs.active.as_deref()) {
        let Some(sheet) = set.sheets.get(&path).or_else(|| inputs.open.get(&path)) else {
            continue;
        };
        if sheet.unknown.newer_minor_source.is_some() {
            scan.read_only.push(path);
            continue;
        }
        let scope = designator_scope_of(inputs.hierarchy.as_ref(), &path);
        for hit in signex_engine::scan_sheet(sheet, &inputs.query, &scope) {
            let placement_label = hit
                .placement
                .as_ref()
                .filter(|_| scope.placements.len() > 1)
                .and_then(|p| scope.placements.iter().position(|q| q == p))
                .map(|i| format!("placement {} of {}", i + 1, scope.placements.len()));
            scan.matches.push(FindMatch {
                sheet: Some(path.clone()),
                hit,
                placement_label,
            });
        }
    }
    scan
}

/// `path` as the user knows it: relative to the project folder when it is
/// inside it, else the full path.
pub(super) fn shown_path(project_dir: Option<&std::path::Path>, path: &std::path::Path) -> String {
    project_dir
        .and_then(|dir| path.strip_prefix(dir).ok())
        .unwrap_or(path)
        .display()
        .to_string()
}

impl Signex {
    /// Claim a new scan generation and return the off-thread scan for the
    /// current query. An empty query (or no document) clears the results.
    pub(super) fn start_project_scan(&mut self) -> Task<Message> {
        let Some((generation, inputs)) = self.begin_project_scan() else {
            return Task::none();
        };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || run_scan(inputs))
                    .await
                    .map_err(|e| format!("Find worker failed: {e}"))
            },
            move |result| {
                Message::FindReplaceMsg(FindReplaceMsg::ProjectScanFinished {
                    generation,
                    result: Arc::new(result),
                })
            },
        )
    }

    /// Run the scan to completion on the calling thread - tests only.
    #[cfg(test)]
    pub(crate) fn run_project_scan_blocking(&mut self) {
        if let Some((generation, inputs)) = self.begin_project_scan() {
            self.apply_project_scan(generation, &Ok(run_scan(inputs)));
        }
    }

    /// Capture the scan's inputs from live state (memory only, no IO) and
    /// claim a new generation. `None` when there is nothing to search.
    fn begin_project_scan(&mut self) -> Option<(u64, ScanInputs)> {
        let query = self.ui_state.find_replace.query.trim().to_string();
        let inputs = self.capture_scan_inputs(&query);
        let find = &mut self.ui_state.find_replace;
        find.scan_generation += 1;
        let generation = find.scan_generation;
        let Some(inputs) = inputs.filter(|_| !query.is_empty()) else {
            find.matches.clear();
            find.selected_index = None;
            find.scanning = false;
            return None;
        };
        find.scanning = true;
        Some((generation, inputs))
    }

    /// The project as the search sees it, captured from live state (memory
    /// only). `None` when there is no project root to search from.
    pub(super) fn capture_scan_inputs(&self, query: &str) -> Option<ScanInputs> {
        let ds = &self.document_state;
        Some(ScanInputs {
            query: query.to_string(),
            open: project_sheets::open_sheets_snapshot(ds),
            pages: project_sheets::declared_pages(ds),
            root: project_sheets::resolve_netlist_root(ds)?,
            active: ds.active_path.clone(),
            fold: crate::app::state::project_case::fold_of(ds.active_document_project()),
            hierarchy: ds.panel_ctx.navigator.hierarchy.clone(),
        })
    }

    /// A finished scan: applied unless a newer one (or a Close, a scope
    /// change) has superseded it.
    pub(super) fn apply_project_scan(
        &mut self,
        generation: u64,
        result: &Result<ProjectScan, String>,
    ) {
        if generation != self.ui_state.find_replace.scan_generation {
            return;
        }
        self.ui_state.find_replace.scanning = false;
        let scan = match result {
            Ok(scan) => scan,
            Err(why) => {
                crate::diagnostics::log_warning(why);
                return;
            }
        };
        let dir = self
            .document_state
            .active_document_project()
            .map(|p| p.dir().to_path_buf());
        let find = &mut self.ui_state.find_replace;
        let mut skipped = Vec::new();
        for (path, why) in &scan.unreadable {
            skipped.push(format!("{} ({why})", shown_path(dir.as_deref(), path)));
            if find.skips_reported.insert(path.clone()) {
                crate::diagnostics::log_warning(format!(
                    "Find: not searched, {why}: {}",
                    path.display()
                ));
            }
        }
        for path in &scan.read_only {
            skipped.push(format!("{} (read-only)", shown_path(dir.as_deref(), path)));
            if find.skips_reported.insert(path.clone()) {
                crate::diagnostics::log_warning(format!(
                    "Find: not searched, read-only (written by a newer Signex): {}",
                    path.display()
                ));
            }
        }
        find.skipped_note =
            (!skipped.is_empty()).then(|| format!("Not searched: {}", skipped.join(", ")));
        find.matches = scan.matches.clone();
        find.selected_index = (!find.matches.is_empty()).then_some(0);
    }
}
