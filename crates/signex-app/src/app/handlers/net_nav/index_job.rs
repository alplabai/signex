//! Building and caching the project net index off the UI thread.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use crate::app::project_sheets::{self, OpenSheets};
use crate::app::runtime::net_class_styles::{PlannedProject, plan_projects};
use crate::app::state::ProjectId;
use crate::app::state::net_class_styles::ProjectKey;
use crate::app::state::net_nav::{IndexOutcome, Pending, PendingNetAction, ProjectNetData, Stored};

use super::super::super::*;
use super::messages;

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
        Ok(self.job_for(planned))
    }

    /// The build for a planned project: a snapshot of its open sheets.
    fn job_for(&self, planned: PlannedProject) -> NetIndexJob {
        let open =
            project_sheets::open_sheets_snapshot_of(&self.document_state, &planned.open_paths);
        NetIndexJob {
            open: Arc::new(open),
            project: planned.project,
            key: planned.key,
            project_dir: planned.project_dir,
        }
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

    /// Park `action` and hand back the build's task. A command the user gave
    /// is never displaced by the hover's own timer.
    pub(super) fn wait_for_net_index(
        &mut self,
        project: ProjectId,
        action: PendingNetAction,
        build: Task<Message>,
    ) -> Task<Message> {
        let pending = Pending {
            project,
            action,
            sheet: self.document_state.active_path.clone(),
            selected: self.interaction_state.active_canvas().selected.clone(),
            cursor: self.cursor_world_now(),
        };
        let nav = &mut self.document_state.net_nav;
        if action != PendingNetAction::HoverDwell || nav.pending.is_none() {
            nav.pending = Some(pending);
        }
        build
    }

    /// Store a finished build and re-run what waited for it.
    pub(super) fn apply_net_index(&mut self, outcome: &IndexOutcome) -> Task<Message> {
        match self.document_state.net_nav.index.apply(outcome) {
            // A newer key arrived while this one ran: build it now, once.
            Stored::Stale => match self.net_index_lookup(outcome.project) {
                IndexLookup::Building(build) => build,
                IndexLookup::Ready(_) => self.run_pending_for(outcome.project),
                IndexLookup::Unavailable => {
                    self.drop_pending_of(outcome.project, messages::command_dropped_left());
                    Task::none()
                }
            },
            Stored::Failed(reason) => {
                let nav = &mut self.document_state.net_nav;
                if nav
                    .pending
                    .as_ref()
                    .is_some_and(|p| p.project == outcome.project)
                {
                    nav.pending = None;
                }
                crate::diagnostics::log_warning(messages::index_failed(&reason));
                Task::none()
            }
            Stored::Fresh(data) => {
                self.report_index_warnings(outcome);
                self.refresh_highlight_from(outcome.project, &data);
                self.run_pending_for(outcome.project)
            }
        }
    }

    /// Run the parked command when its own project's index lands, and only
    /// then, while that project is still open and in view. A command parked
    /// for another project waits for that one's build, or is dropped when it
    /// is gone; it never runs against a different project.
    fn run_pending_for(&mut self, landed: ProjectId) -> Task<Message> {
        let Some(pending) = self.document_state.net_nav.pending.take() else {
            return Task::none();
        };
        let Pending {
            project,
            action,
            sheet,
            selected,
            cursor,
        } = pending.clone();
        let user_command = action != PendingNetAction::HoverDwell;
        if project != landed {
            if self.project_is_planned(project) {
                self.document_state.net_nav.pending = Some(pending);
            } else if user_command {
                crate::diagnostics::log_info(messages::command_dropped_left());
            }
            return Task::none();
        }
        let in_view = self
            .document_state
            .active_path
            .as_deref()
            .and_then(|p| self.project_of_sheet(p))
            == Some(project);
        if !in_view {
            if user_command {
                crate::diagnostics::log_info(messages::command_dropped_left());
            }
            return Task::none();
        }
        let same_context = sheet == self.document_state.active_path
            && selected == self.interaction_state.active_canvas().selected;
        if user_command && !same_context {
            crate::diagnostics::log_info(messages::command_dropped_moved());
            return Task::none();
        }
        // A command aimed at what is under the pointer is about that spot.
        if action == PendingNetAction::HighlightUnderCursor
            && pointer_moved(cursor, self.cursor_world_now())
        {
            crate::diagnostics::log_info(messages::command_dropped_pointer());
            return Task::none();
        }
        self.run_pending_net_action(action)
    }

    /// Whether `project` still has open sheets, so an index can still come.
    fn project_is_planned(&self, project: ProjectId) -> bool {
        plan_projects(&self.document_state)
            .iter()
            .any(|p| p.project == project)
    }

    /// Drop the parked command of `project` and say so, unless it was only
    /// the hover's.
    fn drop_pending_of(&mut self, project: ProjectId, line: String) {
        let nav = &mut self.document_state.net_nav;
        if nav.pending.as_ref().is_some_and(|p| p.project == project) {
            let dropped = nav.pending.take();
            if dropped.is_some_and(|p| p.action != PendingNetAction::HoverDwell) {
                crate::diagnostics::log_info(line);
            }
        }
    }

    /// Say which sheets would not read and what the stitch found. Each is said
    /// once until it clears, whichever of this and the net-class style refresh
    /// meets it first.
    fn report_index_warnings(&mut self, outcome: &IndexOutcome) {
        let reported = &mut self.document_state.net_nav.reported;
        for warning in &outcome.warnings {
            if reported.claim(outcome.project, warning) {
                crate::diagnostics::log_warning(messages::index_warning(warning));
            }
        }
        reported.settle(outcome.project, &outcome.warnings);
    }

    /// Keep what the index holds in step with the projects, and a highlighted
    /// net's index fresh through edits so what stays bright follows the
    /// document. Does nothing on an update that changed no open sheet, project
    /// list, root or sheet list.
    pub(crate) fn refresh_net_nav(&mut self) -> Task<Message> {
        let build = self.refresh_net_index();
        self.sync_net_highlight_to_canvases();
        build
    }

    fn refresh_net_index(&mut self) -> Task<Message> {
        let stamp = self.net_nav_stamp();
        if self.document_state.net_nav.refresh_stamp == Some(stamp) {
            return Task::none();
        }
        self.document_state.net_nav.refresh_stamp = Some(stamp);
        // A project that closed releases its index and its warning claims,
        // highlight or not.
        let open: Vec<ProjectId> = plan_projects(&self.document_state)
            .iter()
            .map(|p| p.project)
            .collect();
        self.document_state.net_nav.index.retain_projects(&open);
        self.document_state.net_nav.reported.retain_projects(&open);
        let Some(project) = self
            .document_state
            .net_nav
            .highlight
            .as_ref()
            .map(|h| h.project)
        else {
            return Task::none();
        };
        if self.document_state.project_by_id(project).is_none() {
            self.document_state.net_nav.highlight = None;
            crate::diagnostics::log_info(messages::project_closed());
            return Task::none();
        }
        match self.net_index_lookup(project) {
            IndexLookup::Building(task) => task,
            IndexLookup::Ready(_) | IndexLookup::Unavailable => Task::none(),
        }
    }

    /// A stamp over what the net index depends on: the open sheets'
    /// connectivity, and each project's root and sheet list.
    fn net_nav_stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let document = &self.document_state;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        crate::app::runtime::net_class_styles::connectivity_stamp(document).hash(&mut hasher);
        for project in &document.projects {
            project.id.hash(&mut hasher);
            project.data.schematic_root.hash(&mut hasher);
            for sheet in &project.data.sheets {
                sheet.filename.hash(&mut hasher);
            }
        }
        hasher.finish()
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
        // The update that came before planned a task this test never ran:
        // build it here, whether or not the cache thinks one is in flight.
        let Some(planned) = plan_projects(&self.document_state)
            .into_iter()
            .find(|p| p.project == project)
        else {
            return;
        };
        let cache = &mut self.document_state.net_nav.index;
        if cache.fresh(project, &planned.key).is_some() {
            return;
        }
        cache.want(project, &planned.key);
        let job = self.job_for(planned);
        let outcome = job.run();
        let _follow_up = self.apply_net_index(&outcome);
    }
}

/// How far the pointer may drift before it is aiming at something else, in mm.
const POINTER_DRIFT_MM: f64 = 0.5;

/// Whether the pointer left the spot a command was given at.
fn pointer_moved(then: Option<(f64, f64)>, now: Option<(f64, f64)>) -> bool {
    match (then, now) {
        (Some(a), Some(b)) => (a.0 - b.0).hypot(a.1 - b.1) > POINTER_DRIFT_MM,
        (None, None) => false,
        _ => true,
    }
}
