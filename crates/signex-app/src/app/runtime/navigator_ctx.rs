//! Keeps the Navigator panel's sheet hierarchy current.
//!
//! `refresh_panel_ctx` runs after nearly every update, and assembling the
//! project's sheets reads the closed ones from disk. So the hierarchy is only
//! rebuilt when what it depends on — the project's pages and root, the open
//! tabs, and the child-sheet references of the open sheets — has changed.
//! Everything cheaper (active-row highlight, open markers) is re-derived each
//! time from the cached hierarchy.
//!
//! The rebuild is disk IO, so it never runs in `update` itself: a refresh only
//! records what it wants ([`NavigatorModel::wanted`]), `update` starts the
//! build as a Task, and the result lands as `ProjectMsg::NavigatorBuilt`. A
//! generation counter drops a result a newer request overtook.
//!
//! Known limit: a *closed* child sheet that changes on disk (renamed, deleted,
//! corrupted) is not noticed until a tab opens or closes, because stat-ing
//! every sheet on each refresh would cost more than the staleness does.

use std::path::PathBuf;

use iced::Task;

use super::super::*;
use crate::app::project_sheets::{
    OpenSheets, assemble_project_sheets_taking, open_sheets_snapshot, project_root_sheet_path,
};
use crate::app::sheet_hierarchy::build_sheet_hierarchy;
use crate::app::state::scope::CaseFold;
use crate::panels::{NavigatorBuildRequest, NavigatorBuilt, NavigatorModel};

/// How many times in a row a failing build is retried before the Navigator
/// stops asking (a refresh runs after every update, so an unbounded retry of a
/// build that always fails would spin).
const MAX_BUILD_ATTEMPTS: u8 = 3;

/// The disk-reading half of a Navigator rebuild, over a snapshot of the open
/// sheets so it can run on a blocking thread.
pub(crate) fn build_navigator_hierarchy(
    open: OpenSheets,
    request: &NavigatorBuildRequest,
) -> NavigatorBuilt {
    let set = assemble_project_sheets_taking(open, &request.pages, &request.root, request.fold);
    let hierarchy = build_sheet_hierarchy(
        &set.sheets,
        &set.unreadable,
        &request.root,
        &set.pages_outside_the_hierarchy,
        request.fold,
    );
    NavigatorBuilt {
        hierarchy,
        unreadable: set.unreadable,
    }
}

impl Signex {
    /// The Navigator model for the current state, reusing `previous`'s cached
    /// hierarchy when nothing it was built from has changed.
    pub(super) fn next_navigator_model(&self, mut previous: NavigatorModel) -> NavigatorModel {
        let Some((root, pages, fold)) = self.navigator_project_scope() else {
            // The counter survives so a build still running for the project
            // just left cannot match a later request.
            return NavigatorModel {
                generation: previous.generation + 1,
                ..NavigatorModel::default()
            };
        };

        let open: Vec<PathBuf> = self
            .document_state
            .tabs
            .iter()
            .map(|tab| tab.path.clone())
            .collect();
        let active = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.path.clone());

        let fingerprint = self.navigator_fingerprint(&root, &pages, fold);
        if previous.fingerprint == fingerprint {
            // Back at what is on screen: a build still running for something
            // else is overtaken.
            if previous.requested != fingerprint {
                previous.requested = fingerprint;
                previous.generation += 1;
                previous.wanted = None;
            }
        } else if previous.requested != fingerprint {
            // The sheets are read from disk, so the build runs from `update`
            // as a Task; until it lands the previous hierarchy stays on show.
            if !previous.requested.is_empty() {
                // A different state, not the retry of a failed build.
                previous.failed_builds = 0;
            }
            previous.generation += 1;
            previous.requested = fingerprint.clone();
            previous.wanted = Some(NavigatorBuildRequest {
                generation: previous.generation,
                fingerprint,
                root,
                pages,
                fold,
            });
        }
        previous.rebuild_tree(active.as_deref(), &open);
        previous
    }

    /// Start the hierarchy build a refresh asked for, if any.
    pub(crate) fn start_navigator_build(&mut self) -> Option<Task<Message>> {
        let request = self.document_state.panel_ctx.navigator.wanted.take()?;
        let open = self.navigator_open_snapshot(&request);
        let generation = request.generation;
        Some(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || build_navigator_hierarchy(open, &request))
                    .await
                    .map_err(|error| error.to_string())
            },
            move |built| {
                Message::Project(ProjectMsg::NavigatorBuilt(generation, built.map(Box::new)))
            },
        ))
    }

    /// The open sheets the build for `request` reads. Every open engine, not
    /// only those the in-memory references reach: the walk also follows
    /// references through closed sheets read from disk, so an open sheet
    /// behind a closed one must still contribute its unsaved edits.
    pub(crate) fn navigator_open_snapshot(&self, _request: &NavigatorBuildRequest) -> OpenSheets {
        open_sheets_snapshot(&self.document_state)
    }

    /// A build landed: swap the hierarchy in unless a newer request overtook it.
    pub(crate) fn handle_navigator_built(
        &mut self,
        generation: u64,
        built: Result<Box<NavigatorBuilt>, String>,
    ) {
        if self.document_state.panel_ctx.navigator.generation != generation {
            return;
        }
        let built = match built {
            Ok(built) => *built,
            Err(reason) => {
                let navigator = &mut self.document_state.panel_ctx.navigator;
                navigator.failed_builds = navigator.failed_builds.saturating_add(1);
                let retrying = navigator.failed_builds < MAX_BUILD_ATTEMPTS;
                if retrying {
                    // Forget the request so the next refresh asks again,
                    // rather than leaving the panel on the old hierarchy.
                    navigator.requested.clear();
                }
                crate::diagnostics::log_warning(format!(
                    "Navigator: the sheet hierarchy could not be built ({reason}){}",
                    if retrying {
                        "; retrying"
                    } else {
                        "; giving up until the project changes"
                    }
                ));
                return;
            }
        };
        // Said once per rebuild, not per refresh: the reason a sheet that
        // exists could not be read would otherwise only be a bare marker.
        for (path, why) in &built.unreadable {
            crate::diagnostics::log_warning(format!(
                "Navigator: sheet {} is unreadable: {why}",
                path.display()
            ));
        }
        let open: Vec<PathBuf> = self
            .document_state
            .tabs
            .iter()
            .map(|tab| tab.path.clone())
            .collect();
        let active = self.active_tab_path();
        let navigator = &mut self.document_state.panel_ctx.navigator;
        navigator.replace_hierarchy(built.hierarchy, active.as_deref(), &open);
        navigator.fingerprint = navigator.requested.clone();
        navigator.failed_builds = 0;
        self.refresh_panel_ctx();
    }

    /// The project the Navigator describes, as `(root sheet, declared pages,
    /// the project's key rules)`.
    ///
    /// The project owning the focused document, else the workspace's active
    /// project. Ownership alone is not enough: it is discovered through the
    /// *open* sheets' child references, so a leaf opened while its ancestors
    /// are closed resolves to no project — and the Navigator would go empty
    /// exactly when the user has navigated deep.
    pub(crate) fn navigator_project_scope(&self) -> Option<(PathBuf, Vec<PathBuf>, CaseFold)> {
        let project = self
            .document_state
            .active_document_project()
            .or_else(|| self.document_state.active_loaded_project())?;
        let root = project_root_sheet_path(project)?;
        let dir = project.dir();
        let pages = project
            .data
            .sheets
            .iter()
            .map(|s| dir.join(&s.filename))
            .collect();
        Some((root, pages, project.case_fold))
    }

    fn navigator_fingerprint(
        &self,
        root: &std::path::Path,
        pages: &[PathBuf],
        fold: CaseFold,
    ) -> String {
        let mut lines = vec![format!("root {}", root.display()), format!("fold {fold:?}")];
        lines.extend(pages.iter().map(|p| format!("page {}", p.display())));
        lines.extend(
            self.document_state
                .tabs
                .iter()
                .map(|t| format!("tab {}", t.path.display())),
        );
        let mut engines: Vec<_> = self.document_state.engines.iter().collect();
        engines.sort_by(|a, b| a.0.cmp(b.0));
        for (path, engine) in engines {
            lines.push(format!("sheet {}", path.display()));
            lines.extend(
                engine
                    .document()
                    .child_sheets
                    .iter()
                    .map(|c| format!("  ref {}|{}|{}", c.name, c.filename, c.uuid)),
            );
        }
        lines.join("\n")
    }
}

#[cfg(test)]
impl Signex {
    /// Run the build a refresh asked for to completion, in place of the Task
    /// `update` would have spawned, and deliver its result.
    pub(crate) fn settle_navigator(&mut self) {
        while let Some(request) = self.document_state.panel_ctx.navigator.wanted.take() {
            let open = self.navigator_open_snapshot(&request);
            let built = build_navigator_hierarchy(open, &request);
            self.handle_navigator_built(request.generation, Ok(Box::new(built)));
        }
    }
}
