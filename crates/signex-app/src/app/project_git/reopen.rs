//! After a switch that changed a `.snxprj`: the project closes and opens
//! again from the new file, with the tabs it had. The Close Project scan
//! decides what is unsaved: a document another project also claims keeps its
//! tab and edits, and one only this project holds stops the reopen.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{RenderTranslatable, Translatable, ids};

use super::{ProjectGitMsg, SwitchDone};
use crate::app::ProjectSection;
use crate::app::handlers::dock::project_navigation::add_io::on_blocking_pool;
use crate::app::state::sheet_refs::SheetRefs;
use crate::app::state::volume_case::directory_ignores_case;
use crate::app::state::{ErrorNotice, ProjectId};
use crate::app::{Message, Signex, TabKind};

/// What a reopened project shows again.
struct ReopenedView {
    /// Sheets and boards to open, the active one last.
    documents: Vec<PathBuf>,
    /// The Project tab's section, when that tab was open.
    section: Option<ProjectSection>,
    /// The Rules tab, when open: whether it was the active tab.
    rules: Option<bool>,
    /// Boards with a Layer Stack tab to bring back, and whether it was active.
    layer_stacks: Vec<(PathBuf, bool)>,
    /// This project's tabs of kept documents, which stay open.
    kept: Vec<PathBuf>,
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

impl Signex {
    pub(super) fn reopen_refs_task(&self, done: Box<SwitchDone>) -> Task<Message> {
        let inputs = self.document_state.sheet_ref_inputs();
        Task::perform(
            on_blocking_pool(move || Ok(Box::new(inputs.resolve(directory_ignores_case)))),
            move |result| Message::from(ProjectGitMsg::ReopenRefsResolved { done, result }),
        )
    }

    /// One project at a time: closing one shifts the indices the references
    /// are keyed by, so the next is read again.
    pub(super) fn finish_project_reopen(
        &mut self,
        mut done: Box<SwitchDone>,
        result: Result<Box<SheetRefs>, String>,
    ) -> Task<Message> {
        let refs = match result {
            Ok(refs) => refs,
            Err(why) => {
                let file = done.reopen.first().map(|rel| file_name(rel));
                let detail = Translatable::new(ids::PROJECT_VCS_SWITCH_REOPEN_FAILED)
                    .arg("file", file.unwrap_or_default())
                    .arg("why", why);
                self.report_reopen_problem(detail);
                self.document_state.branch_switch = None;
                return self.refresh_after_switch(&done);
            }
        };
        let next = done
            .reopen
            .last()
            .and_then(|rel| self.project_id_at(&done.root.join(rel)));
        if let Some(id) = next
            && !self.settle_before_reopen(id)
        {
            // The project stays open on its old file; the bake's notice shows.
            let rel = done.reopen.pop().unwrap_or_default();
            let detail =
                Translatable::new(ids::PROJECT_VCS_SWITCH_REOPEN_HELD).arg("file", file_name(&rel));
            crate::diagnostics::log_warning(format!("[git] {}", detail.render_en_us()));
            return self.continue_reopen(done, Task::none());
        }
        if !self.document_state.refs_are_current(&refs) {
            return self.reopen_refs_task(done);
        }
        let Some(rel) = done.reopen.pop() else {
            self.document_state.branch_switch = None;
            return self.refresh_after_switch(&done);
        };
        let path = done.root.join(&rel);
        let project = self.project_id_at(&path);
        let reopened = project.map_or_else(Task::none, |id| self.reopen_project(id, &refs));
        self.continue_reopen(done, reopened)
    }

    fn project_id_at(&self, path: &Path) -> Option<ProjectId> {
        self.document_state
            .projects
            .iter()
            .find(|project| project.path == path)
            .map(|project| project.id)
    }

    /// On to the next project, or finish once none is left.
    fn continue_reopen(&mut self, done: Box<SwitchDone>, reopened: Task<Message>) -> Task<Message> {
        if done.reopen.is_empty() {
            self.document_state.branch_switch = None;
            return Task::batch([reopened, self.refresh_after_switch(&done)]);
        }
        Task::batch([reopened, self.reopen_refs_task(done)])
    }

    /// Turn the project's pending drafts and owed board sketch into edits, so
    /// the scan sees them. `false`: the bake refused and its notice says why.
    fn settle_before_reopen(&mut self, project_id: ProjectId) -> bool {
        self.commit_drafts_of_project(project_id);
        let sketched = self.board_sketch_owed().is_some_and(|board| {
            self.document_state
                .document_project(&board)
                .is_some_and(|project| project.id == project_id)
        });
        !sketched || self.settle_board_sketch_of(None)
    }

    fn report_reopen_problem(&mut self, detail: Translatable) {
        crate::diagnostics::log_warning(format!("[git] {}", detail.render_en_us()));
        self.document_state.error_notice = Some(ErrorNotice::new(
            ids::PROJECT_VCS_SWITCH_REOPEN_TITLE,
            detail,
        ));
    }

    /// What to show again. `scan_kept` are the dirty documents the close
    /// leaves open.
    fn reopened_view(&self, project_id: ProjectId, scan_kept: &[PathBuf]) -> ReopenedView {
        let d = &self.document_state;
        let kept: Vec<PathBuf> = scan_kept
            .iter()
            .filter(|path| {
                d.tabs
                    .iter()
                    .any(|tab| tab.path == **path && tab.project_id == Some(project_id))
            })
            .cloned()
            .collect();
        let active = d.tabs.get(d.active_tab).map(|tab| tab.path.clone());
        let (rules, layer_stacks) = self.reopened_pages(project_id, &kept);
        let mut documents: Vec<PathBuf> = d
            .tabs
            .iter()
            .filter(|tab| {
                tab.project_id == Some(project_id)
                    && matches!(tab.kind, TabKind::Schematic | TabKind::Pcb)
                    && !kept.contains(&tab.path)
            })
            .map(|tab| tab.path.clone())
            .collect();
        documents.sort_by_key(|path| Some(path) == active.as_ref());
        let has_tab = d
            .tabs
            .iter()
            .any(|tab| tab.kind.as_project() == Some(project_id));
        let section = has_tab.then(|| {
            d.project_tabs
                .get(&project_id)
                .map(|tab| tab.section)
                .unwrap_or_default()
        });
        ReopenedView {
            documents,
            section,
            rules,
            layer_stacks,
            kept,
        }
    }

    /// The project's Rules tab and its boards' Layer Stack tabs, each with
    /// whether it is the active tab. A kept board keeps its own page tab.
    fn reopened_pages(
        &self,
        project_id: ProjectId,
        kept: &[PathBuf],
    ) -> (Option<bool>, Vec<(PathBuf, bool)>) {
        let d = &self.document_state;
        let active = d.tabs.get(d.active_tab);
        let rules = d
            .tabs
            .iter()
            .any(|tab| tab.kind.as_rules() == Some(project_id))
            .then(|| active.and_then(|tab| tab.kind.as_rules()) == Some(project_id));
        let layer_stacks = d
            .tabs
            .iter()
            .filter(|tab| {
                tab.project_id == Some(project_id)
                    && tab.kind.is_layer_stack()
                    && !kept.contains(&tab.path)
            })
            .map(|tab| {
                let was_active =
                    active.is_some_and(|a| a.kind.is_layer_stack() && a.path == tab.path);
                (tab.path.clone(), was_active)
            })
            .collect();
        (rules, layer_stacks)
    }

    /// Close the project and open it from the new file. A document only this
    /// project holds unsaved stops it, as the Close Project prompt would:
    /// closing without asking would drop those edits.
    fn reopen_project(&mut self, project_id: ProjectId, refs: &SheetRefs) -> Task<Message> {
        let d = &self.document_state;
        let Some(index) = d.projects.iter().position(|p| p.id == project_id) else {
            return Task::none();
        };
        let path = d.projects[index].path.clone();
        let scan = self.current_close_scan(index, refs);
        let held = d
            .is_document_dirty(&path)
            .then(|| path.clone())
            .or_else(|| scan.mine.first().cloned())
            .or_else(|| scan.mine_rows.first().map(|row| row.library_path.clone()));
        if let Some(file) = held {
            self.report_reopen_problem(
                Translatable::new(ids::PROJECT_VCS_SWITCH_REOPEN_HELD)
                    .arg("file", file_name(&file)),
            );
            return Task::none();
        }
        let view = self.reopened_view(project_id, &scan.kept);
        // Every kept path stays out of the close's sweep, a tabless one too.
        let closed = self.execute_close_project(project_id, refs, &scan.kept);
        if self.document_state.project_by_id(project_id).is_some() {
            let detail = Translatable::new(ids::PROJECT_VCS_SWITCH_REOPEN_HELD)
                .arg("file", file_name(&path));
            self.report_reopen_problem(detail);
            return closed;
        }
        self.ui_state.project_git.repos.remove(&project_id);
        let Ok(opened) = self.open_path_reported(path.clone()) else {
            return closed;
        };
        let mut tasks = vec![closed, opened];
        tasks.extend(self.restore_reopened_view(&path, view));
        Task::batch(tasks)
    }

    /// Give the new project its kept tabs, its Project tab and its documents.
    fn restore_reopened_view(&mut self, path: &Path, view: ReopenedView) -> Vec<Task<Message>> {
        let index = self
            .document_state
            .projects
            .iter()
            .position(|project| project.path == path);
        let Some(index) = index else {
            return Vec::new();
        };
        let new_id = self.document_state.projects[index].id;
        for tab in &mut self.document_state.tabs {
            if tab.project_id.is_none() && view.kept.contains(&tab.path) {
                tab.project_id = Some(new_id);
            }
        }
        if let Some(section) = view.section {
            self.open_project_tab(index);
            if let Some(tab) = self.document_state.project_tabs.get_mut(&new_id) {
                tab.section = section;
            }
        }
        if view.rules == Some(false) {
            self.open_rules_tab(new_id);
        }
        // A board opens off the update thread: its Layer Stack tab follows.
        self.ui_state
            .project_git
            .reopened_stacks
            .extend(view.layer_stacks);
        let mut tasks = Vec::new();
        for document in view.documents {
            match self.open_document_path(document) {
                Ok(task) => tasks.push(task),
                Err(error) => {
                    crate::diagnostics::log_error("Reopen after a branch switch", &error);
                }
            }
        }
        if view.rules == Some(true) {
            self.open_rules_tab(new_id);
        }
        tasks
    }
}
