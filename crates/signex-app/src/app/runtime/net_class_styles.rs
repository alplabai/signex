//! Keeps the canvas's resolved net-class styles current.
//!
//! The join needs the project's stitched netlist, which reads every sheet
//! that is not open in a tab from disk. That never happens inside `update`:
//! the refresh snapshots what it needs, resolves on a blocking thread and
//! applies the answer through `UiMsg::NetClassStylesResolved`. Meanwhile the
//! previously resolved styles stay drawn.
//!
//! Only an edit that can change connectivity (the engine's
//! `connectivity_revision`), a class-table edit, or a change of a project's
//! sheet list or root asks for new work; moving a text note does not. Each
//! project is planned, resolved and applied on its own key, so one project's
//! edits never discard another's answer, and the open sheets of only the
//! projects being resolved are cloned for the worker.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use crate::app::contracts::{Message, UiMsg};
use crate::app::project_sheets::{self, OpenSheets};
use crate::app::state::net_class_styles::{
    ProjectKey, ResolveFailed, ResolvedProject, ResolvedStyles,
};
use crate::app::state::project_case::fold_of;
use crate::app::state::scope;
use crate::app::state::{DocumentState, ProjectId};

use super::Signex;

/// A project with open sheets, and the key its resolution is cached under.
struct PlannedProject {
    project: ProjectId,
    key: ProjectKey,
    open_paths: Vec<PathBuf>,
    project_dir: PathBuf,
}

/// Everything a worker needs to stitch one project: owned, so it can leave
/// the UI thread.
struct ProjectJob {
    project: ProjectId,
    key: ProjectKey,
    project_dir: PathBuf,
}

/// A planned background resolution. [`ClassStyleJob::run`] does the disk
/// reads; nothing else in this module does.
pub(crate) struct ClassStyleJob {
    open: Arc<OpenSheets>,
    jobs: Vec<ProjectJob>,
}

impl ClassStyleJob {
    /// Stitch each project and join its class table. Blocking.
    pub(crate) fn run(self) -> ResolvedStyles {
        let open = self.open;
        let projects = self
            .jobs
            .into_iter()
            .map(|job| resolve_project(&open, job))
            .collect();
        ResolvedStyles { projects }
    }

    /// `true` when no project needs a netlist, so the answer needs no IO.
    fn needs_no_io(&self) -> bool {
        self.jobs.iter().all(|job| job.key.classes.is_empty())
    }
}

fn resolve_project(open: &OpenSheets, job: ProjectJob) -> ResolvedProject {
    let ProjectJob {
        project,
        key,
        project_dir,
    } = job;
    let mut resolved = ResolvedProject {
        project,
        key,
        styles: Default::default(),
        nets: Default::default(),
        warnings: Vec::new(),
    };
    if resolved.key.classes.is_empty() {
        return resolved;
    }
    let stitched = crate::app::stitched_netlist::stitched_project_netlist_from(
        open,
        &resolved.key.pages,
        &resolved.key.root,
        resolved.key.fold,
        Some(project_dir),
    );
    resolved.warnings = stitched.warnings;
    if let Some(netlist) = stitched.netlist {
        resolved.styles = signex_net::element_class_styles(&netlist, &resolved.key.classes);
        resolved.nets = signex_net::element_net_names(&netlist);
    }
    resolved
}

/// Stamp over every open sheet's identity and connectivity revision. Moves on
/// any edit that can change a net, and on a sheet opening, closing or being
/// replaced; not on an edit that leaves connectivity alone.
fn connectivity_stamp(document: &DocumentState) -> u64 {
    document.engines.iter().fold(0u64, |acc, (path, engine)| {
        let mut hasher = DefaultHasher::new();
        (path, engine.instance_id(), engine.connectivity_revision()).hash(&mut hasher);
        acc.wrapping_add(hasher.finish())
    })
}

/// Every project that owns an open sheet, with the key to resolve it under.
fn plan_projects(document: &DocumentState) -> Vec<PlannedProject> {
    struct Open {
        stamp: u64,
        paths: Vec<PathBuf>,
    }
    let refs = document.child_sheet_refs();
    let mut open: std::collections::HashMap<ProjectId, Open> = Default::default();
    for (path, engine) in &document.engines {
        let Some(project) = scope::project_owning_sheet(&document.projects, &refs, path) else {
            continue;
        };
        let mut hasher = DefaultHasher::new();
        (path, engine.instance_id(), engine.connectivity_revision()).hash(&mut hasher);
        let entry = open.entry(project.id).or_insert_with(|| Open {
            stamp: 0,
            paths: Vec::new(),
        });
        entry.stamp = entry.stamp.wrapping_add(hasher.finish());
        entry.paths.push(path.clone());
    }
    document
        .projects
        .iter()
        .filter_map(|project| {
            let mut entry = open.remove(&project.id)?;
            // The first open sheet by path names the project when it has no
            // declared root.
            entry.paths.sort();
            let root = project_sheets::resolve_netlist_root_for(document, entry.paths.first()?);
            Some(PlannedProject {
                project: project.id,
                key: ProjectKey {
                    connectivity: entry.stamp,
                    pages: project_sheets::project_pages(Some(project)),
                    root,
                    fold: fold_of(Some(project)),
                    classes: project.data.net_classes.clone(),
                },
                open_paths: entry.paths,
                project_dir: project.dir().to_path_buf(),
            })
        })
        .collect()
}

impl Signex {
    /// Plan the net-class style refresh after an update, or `None` when
    /// nothing needs resolving.
    ///
    /// A cheap stamp over the open sheets' connectivity revisions and the
    /// projects' class tables gates the work, so an update that edited neither
    /// (a pointer move, a text-note edit) does nothing. A session with no
    /// classes anywhere returns before gathering any input. Reads no file:
    /// the sheets of the projects to resolve are cloned, not loaded.
    pub(crate) fn plan_net_class_refresh(&mut self) -> Option<ClassStyleJob> {
        let document = &self.document_state;
        let any_classes = document
            .projects
            .iter()
            .any(|project| !project.data.net_classes.is_empty());
        if !any_classes && self.ui_state.net_class_styles.is_empty() {
            return None;
        }
        let stamp = connectivity_stamp(document);
        if !self
            .ui_state
            .net_class_styles
            .needs_refresh(stamp, &document.projects)
        {
            return None;
        }

        let planned = plan_projects(document);
        let keep: Vec<ProjectId> = planned.iter().map(|p| p.project).collect();
        let cache = &mut self.ui_state.net_class_styles;
        let pruned = cache.retain_projects(&keep);
        let mut snapshot_paths = Vec::new();
        let mut jobs = Vec::new();
        for project in planned {
            if !cache.want(project.project, &project.key) {
                continue;
            }
            if !project.key.classes.is_empty() {
                snapshot_paths.extend(project.open_paths);
            }
            jobs.push(ProjectJob {
                project: project.project,
                key: project.key,
                project_dir: project.project_dir,
            });
        }
        if pruned {
            self.clear_schematic_content_caches();
        }
        if jobs.is_empty() {
            return None;
        }
        let open = project_sheets::open_sheets_snapshot_of(&self.document_state, &snapshot_paths);
        Some(ClassStyleJob {
            open: Arc::new(open),
            jobs,
        })
    }

    /// Re-resolve the net-class style of every project whose inputs changed
    /// after an update. Returns the background task that does the disk reads;
    /// the styles already drawn stay until its answer is applied.
    pub(crate) fn refresh_net_class_styles(&mut self) -> Task<Message> {
        let Some(job) = self.plan_net_class_refresh() else {
            return Task::none();
        };
        if job.needs_no_io() {
            let resolved = job.run();
            self.apply_net_class_styles(&Ok(resolved));
            return Task::none();
        }
        let projects = job
            .jobs
            .iter()
            .map(|j| (j.project, j.key.clone()))
            .collect::<Vec<_>>();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || job.run())
                    .await
                    .map_err(|e| ResolveFailed {
                        projects,
                        reason: format!("the net-class style task failed: {e}"),
                    })
            },
            |result| Message::Ui(UiMsg::NetClassStylesResolved(Arc::new(result))),
        )
    }

    /// Apply a finished resolution to the projects it is still the answer for.
    /// When anything drawn changed, the schematic canvases drop their content
    /// caches so the next frame repaints with the new style, and when a net
    /// name changed the panel context is rebuilt for the label picker.
    pub(crate) fn apply_net_class_styles(
        &mut self,
        result: &Result<ResolvedStyles, ResolveFailed>,
    ) {
        let resolved = match result {
            Ok(resolved) => resolved,
            Err(failed) => {
                crate::diagnostics::log_warning(&failed.reason);
                self.ui_state.net_class_styles.give_up(&failed.projects);
                return;
            }
        };
        let applied = self.ui_state.net_class_styles.apply(resolved);
        for warning in &applied.warnings {
            crate::diagnostics::log_warning(warning);
        }
        if applied.styles_changed {
            self.clear_schematic_content_caches();
        }
        if applied.styles_changed || applied.nets_changed {
            self.refresh_panel_ctx();
        }
    }

    fn clear_schematic_content_caches(&mut self) {
        for canvas in self.interaction_state.all_schematic_canvases_mut() {
            canvas.clear_content_cache();
        }
    }

    /// The failure a dead worker would report for `project`'s current request,
    /// for a regression test that cannot name a [`ProjectKey`].
    #[doc(hidden)]
    #[must_use]
    pub fn net_class_failure_of_current_request(
        &self,
        project: ProjectId,
        reason: &str,
    ) -> Option<ResolveFailed> {
        let key = self.ui_state.net_class_styles.wanted_key(project)?.clone();
        Some(ResolveFailed {
            projects: vec![(project, key)],
            reason: reason.to_string(),
        })
    }

    /// Resolve and apply the net-class styles on this thread, for a regression
    /// test that has no async runtime to run the spawned task.
    #[doc(hidden)]
    pub fn settle_net_class_styles_blocking(&mut self) {
        // The update that came before planned a task this test never ran;
        // treat it as lost so the same inputs are planned again.
        self.ui_state.net_class_styles.give_up_pending();
        if let Some(job) = self.plan_net_class_refresh() {
            let resolved = job.run();
            self.apply_net_class_styles(&Ok(resolved));
        }
    }
}
