//! Building and caching the project net index off the UI thread.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use crate::app::project_sheets::{self, OpenSheets};
use crate::app::runtime::net_class_styles::plan_projects;
use crate::app::state::ProjectId;
use crate::app::state::net_class_styles::ProjectKey;
use crate::app::state::net_nav::{IndexOutcome, PendingNetAction, ProjectNetData, Stored};

use super::super::super::*;

/// What a caller gets when it needs a project's index.
pub(super) enum IndexLookup {
    Ready(Arc<ProjectNetData>),
    /// The build is under way (or just started): return the task and park the
    /// request as pending.
    Building(Task<Message>),
    /// No index can come: the project is gone or its last build failed.
    Unavailable,
}

/// A planned background build. [`NetIndexJob::run`] does the disk reads.
pub(crate) struct NetIndexJob {
    open: Arc<OpenSheets>,
    project: ProjectId,
    key: ProjectKey,
    project_dir: PathBuf,
}

impl NetIndexJob {
    /// Stitch the project and index its nets. Blocking.
    pub(crate) fn run(self) -> IndexOutcome {
        let Self {
            open,
            project,
            key,
            project_dir,
        } = self;
        let stitched = crate::app::stitched_netlist::stitched_index_from(
            &open,
            &key.pages,
            &key.root,
            key.fold,
            Some(project_dir.clone()),
        );
        let result = match stitched.index {
            Some(index) => Ok(Arc::new(ProjectNetData {
                index,
                paths: stitched.sheet_paths,
                base_dir: Some(project_dir),
                fold: key.fold,
                classes: key.classes.clone(),
            })),
            None => Err(format!(
                "the project's root sheet '{}' did not assemble",
                key.root.display()
            )),
        };
        IndexOutcome {
            project,
            key,
            result,
            warnings: stitched.warnings,
        }
    }
}

impl Signex {
    /// Plan a build of `project`'s index when the cache holds none for its
    /// current key. `None` when it is fresh, already building, or failed.
    fn plan_net_index_job(&mut self, project: ProjectId) -> Result<NetIndexJob, IndexLookup> {
        let planned = plan_projects(&self.document_state)
            .into_iter()
            .find(|p| p.project == project)
            .ok_or(IndexLookup::Unavailable)?;
        let cache = &mut self.document_state.net_nav.index;
        if let Some(data) = cache.fresh(project, &planned.key) {
            return Err(IndexLookup::Ready(data));
        }
        if !cache.want(project, &planned.key) {
            return Err(if cache.is_building(project, &planned.key) {
                IndexLookup::Building(Task::none())
            } else {
                IndexLookup::Unavailable
            });
        }
        let open =
            project_sheets::open_sheets_snapshot_of(&self.document_state, &planned.open_paths);
        Ok(NetIndexJob {
            open: Arc::new(open),
            project,
            key: planned.key,
            project_dir: planned.project_dir,
        })
    }

    /// The index of `project` for its current content, starting a build when
    /// there is none.
    pub(super) fn net_index_lookup(&mut self, project: ProjectId) -> IndexLookup {
        let job = match self.plan_net_index_job(project) {
            Ok(job) => job,
            Err(lookup) => return lookup,
        };
        let (project, key) = (job.project, job.key.clone());
        IndexLookup::Building(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || job.run())
                    .await
                    .unwrap_or_else(|e| IndexOutcome {
                        project,
                        key,
                        result: Err(format!("the net index task failed: {e}")),
                        warnings: Vec::new(),
                    })
            },
            |outcome| Message::Ui(UiMsg::NetNav(NetNavMsg::IndexResolved(Arc::new(outcome)))),
        ))
    }

    /// Park `action` and hand back the build's task.
    pub(super) fn wait_for_net_index(
        &mut self,
        action: PendingNetAction,
        build: Task<Message>,
    ) -> Task<Message> {
        self.document_state.net_nav.pending = Some(action);
        build
    }

    /// Store a finished build and re-run what waited for it.
    pub(super) fn apply_net_index(&mut self, outcome: &IndexOutcome) -> Task<Message> {
        match self.document_state.net_nav.index.apply(outcome) {
            Stored::Stale => Task::none(),
            Stored::Failed(reason) => {
                self.document_state.net_nav.pending = None;
                crate::diagnostics::log_warning(format!(
                    "Net navigation unavailable: {reason}. Check the project's root sheet."
                ));
                Task::none()
            }
            Stored::Fresh(data) => {
                for warning in &outcome.warnings {
                    crate::diagnostics::log_warning(warning);
                }
                self.refresh_highlight_from(outcome.project, &data);
                match self.document_state.net_nav.pending.take() {
                    Some(action) => self.run_pending_net_action(action),
                    None => Task::none(),
                }
            }
        }
    }

    /// Keep a highlighted net's index fresh through edits, so what stays
    /// bright follows the document.
    pub(crate) fn refresh_net_nav(&mut self) -> Task<Message> {
        let build = match self
            .document_state
            .net_nav
            .highlight
            .as_ref()
            .map(|h| h.project)
        {
            Some(project) => match self.net_index_lookup(project) {
                IndexLookup::Building(task) => task,
                _ => Task::none(),
            },
            None => Task::none(),
        };
        self.sync_net_highlight_to_canvases();
        build
    }

    /// Build and apply the active project's index on this thread, for a
    /// regression test that has no async runtime to run the spawned task.
    #[doc(hidden)]
    pub fn settle_net_index_blocking(&mut self) {
        let Some(project) = self
            .document_state
            .active_path
            .as_deref()
            .and_then(|p| self.project_of_sheet(p))
        else {
            return;
        };
        // The update that came before planned a task this test never ran.
        self.document_state.net_nav.index.forget_pending();
        if let Ok(job) = self.plan_net_index_job(project) {
            let outcome = job.run();
            let _follow_up = self.apply_net_index(&outcome);
        }
    }
}
