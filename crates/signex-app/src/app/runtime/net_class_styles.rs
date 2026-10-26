//! Keeps the canvas's resolved net-class styles current.
//!
//! The join needs the project's stitched netlist, which reads every sheet
//! that is not open in a tab from disk. That never happens inside `update`:
//! the refresh snapshots what it needs, resolves on a blocking thread and
//! applies the answer through `UiMsg::NetClassStylesResolved`. Meanwhile the
//! previously resolved styles stay drawn.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_types::net_class::NetClass;

use crate::app::contracts::{Message, UiMsg};
use crate::app::project_sheets::{self, OpenSheets};
use crate::app::state::ProjectId;
use crate::app::state::net_class_styles::{ResolvedProject, ResolvedStyles};
use crate::app::state::project_case::fold_of;
use crate::app::state::scope::{self, CaseFold};

use super::Signex;

/// Everything a worker needs to stitch one project: owned, so it can leave
/// the UI thread.
struct ProjectJob {
    project: ProjectId,
    revision: u64,
    classes: Vec<NetClass>,
    pages: Vec<PathBuf>,
    root: PathBuf,
    fold: CaseFold,
    project_dir: Option<PathBuf>,
}

/// A planned background resolution. [`ClassStyleJob::run`] does the disk
/// reads; nothing else in this module does.
pub(crate) struct ClassStyleJob {
    generation: u64,
    keep: Vec<ProjectId>,
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
            .map(|job| {
                let styles = if job.classes.is_empty() {
                    Default::default()
                } else {
                    crate::app::stitched_netlist::stitched_project_netlist_from(
                        &open,
                        &job.pages,
                        &job.root,
                        job.fold,
                        job.project_dir,
                    )
                    .map(|netlist| signex_net::element_class_styles(&netlist, &job.classes))
                    .unwrap_or_default()
                };
                ResolvedProject {
                    project: job.project,
                    revision: job.revision,
                    classes: job.classes,
                    styles,
                }
            })
            .collect();
        ResolvedStyles {
            generation: self.generation,
            keep: self.keep,
            projects,
        }
    }

    /// `true` when no project needs a netlist, so the answer needs no IO.
    fn needs_no_io(&self) -> bool {
        self.jobs.iter().all(|job| job.classes.is_empty())
    }
}

impl Signex {
    /// Plan the net-class style refresh after an update, or `None` when
    /// nothing drawn can have changed.
    ///
    /// A cheap stamp over the open sheets' revisions and the projects' class
    /// tables gates the work, so an update that edited neither (a pointer
    /// move) does nothing. A session with no classes anywhere returns before
    /// gathering any input. Reads no file: sheets are cloned, not loaded.
    pub(crate) fn plan_net_class_refresh(&mut self) -> Option<ClassStyleJob> {
        let document = &self.document_state;
        let any_classes = document
            .projects
            .iter()
            .any(|project| !project.data.net_classes.is_empty());
        if !any_classes && self.ui_state.net_class_styles.is_empty() {
            return None;
        }
        let stamp = document.engines.iter().fold(0u64, |acc, (path, engine)| {
            let mut hasher = DefaultHasher::new();
            (path, engine.revision()).hash(&mut hasher);
            acc.wrapping_add(hasher.finish())
        });
        if !self
            .ui_state
            .net_class_styles
            .needs_refresh(stamp, &document.projects)
        {
            return None;
        }

        // Per project: the stamp over its open sheets, and the first open
        // sheet by path, which names the project when its netlist is built.
        let refs = document.child_sheet_refs();
        let mut open: std::collections::HashMap<ProjectId, (u64, PathBuf)> = Default::default();
        for (path, engine) in &document.engines {
            let Some(project) = scope::project_owning_sheet(&document.projects, &refs, path) else {
                continue;
            };
            let entry = open.entry(project.id).or_insert_with(|| (0, path.clone()));
            entry.0 = entry.0.wrapping_add(engine.revision());
            if *path < entry.1 {
                entry.1 = path.clone();
            }
        }
        let cache = &self.ui_state.net_class_styles;
        let mut keep = Vec::new();
        let mut jobs = Vec::new();
        for project in &document.projects {
            let Some((revision, source)) = open.get(&project.id) else {
                continue;
            };
            keep.push(project.id);
            let classes = &project.data.net_classes;
            if cache.is_current(project.id, *revision, classes) {
                continue;
            }
            jobs.push(ProjectJob {
                project: project.id,
                revision: *revision,
                classes: classes.clone(),
                pages: project_sheets::project_pages(Some(project)),
                root: project_sheets::resolve_netlist_root_for(document, source),
                fold: fold_of(Some(project)),
                project_dir: Some(project.dir().to_path_buf()),
            });
        }
        let snapshot = if jobs.iter().any(|job| !job.classes.is_empty()) {
            project_sheets::open_sheets_snapshot(document)
        } else {
            OpenSheets::default()
        };
        Some(ClassStyleJob {
            generation: cache.generation(),
            keep,
            open: Arc::new(snapshot),
            jobs,
        })
    }

    /// Re-resolve the net-class style of every project with open sheets
    /// after an update. Returns the background task that does the disk
    /// reads; the styles already drawn stay until its answer is applied.
    pub(crate) fn refresh_net_class_styles(&mut self) -> Task<Message> {
        let Some(job) = self.plan_net_class_refresh() else {
            return Task::none();
        };
        if job.needs_no_io() {
            let resolved = job.run();
            self.apply_net_class_styles(&Ok(resolved));
            return Task::none();
        }
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || job.run())
                    .await
                    .map_err(|e| format!("the net-class style task failed: {e}"))
            },
            |result| Message::Ui(UiMsg::NetClassStylesResolved(Arc::new(result))),
        )
    }

    /// Apply a finished resolution, unless a newer refresh has started since.
    /// When anything drawn changed, the schematic canvases drop their content
    /// caches so the next frame repaints with the new style.
    pub(crate) fn apply_net_class_styles(&mut self, result: &Result<ResolvedStyles, String>) {
        let resolved = match result {
            Ok(resolved) => resolved,
            Err(reason) => {
                crate::diagnostics::log_warning(reason.clone());
                return;
            }
        };
        if self.ui_state.net_class_styles.apply(resolved) {
            for canvas in self.interaction_state.all_schematic_canvases_mut() {
                canvas.clear_content_cache();
            }
        }
    }

    /// Resolve and apply the net-class styles on this thread, for a regression
    /// test that has no async runtime to run the spawned task.
    #[doc(hidden)]
    pub fn settle_net_class_styles_blocking(&mut self) {
        self.ui_state.net_class_styles.reopen_gate();
        if let Some(job) = self.plan_net_class_refresh() {
            let resolved = job.run();
            self.apply_net_class_styles(&Ok(resolved));
        }
    }
}
