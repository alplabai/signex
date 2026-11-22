//! After a switch that changed a `.snxprj`: the project closes and opens
//! again from the new file, with the tabs it had. Documents edited during
//! the switch keep their tabs and edits.

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
    /// Documents with edits, whose tabs stay open.
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
        if !self.document_state.refs_are_current(&refs) {
            return self.reopen_refs_task(done);
        }
        let Some(rel) = done.reopen.pop() else {
            self.document_state.branch_switch = None;
            return self.refresh_after_switch(&done);
        };
        let path = done.root.join(&rel);
        let project = self
            .document_state
            .projects
            .iter()
            .find(|project| project.path == path)
            .map(|project| project.id);
        let reopened = project.map_or_else(Task::none, |id| self.reopen_project(id, &refs));
        if done.reopen.is_empty() {
            self.document_state.branch_switch = None;
            return Task::batch([reopened, self.refresh_after_switch(&done)]);
        }
        Task::batch([reopened, self.reopen_refs_task(done)])
    }

    fn report_reopen_problem(&mut self, detail: Translatable) {
        crate::diagnostics::log_warning(format!("[git] {}", detail.render_en_us()));
        self.document_state.error_notice = Some(ErrorNotice::new(
            ids::PROJECT_VCS_SWITCH_REOPEN_TITLE,
            detail,
        ));
    }

    fn reopened_view(&self, project_id: ProjectId, dir: &Path) -> ReopenedView {
        let d = &self.document_state;
        let kept: Vec<PathBuf> = d
            .dirty_document_paths()
            .into_iter()
            .filter(|path| path.starts_with(dir))
            .collect();
        let active = d.tabs.get(d.active_tab).map(|tab| tab.path.clone());
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
            kept,
        }
    }

    /// Close the project and open it from the new file. A `.snxprj` edited
    /// during the switch stays open: closing would drop those edits.
    fn reopen_project(&mut self, project_id: ProjectId, refs: &SheetRefs) -> Task<Message> {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return Task::none();
        };
        let (path, dir) = (project.path.clone(), project.dir().to_path_buf());
        if self.document_state.is_document_dirty(&path) {
            self.report_reopen_problem(
                Translatable::new(ids::PROJECT_VCS_SWITCH_REOPEN_HELD)
                    .arg("file", file_name(&path)),
            );
            return Task::none();
        }
        let view = self.reopened_view(project_id, &dir);
        let closed = self.execute_close_project(project_id, refs, &view.kept);
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
        let mut tasks = Vec::new();
        for document in view.documents {
            match self.open_document_path(document) {
                Ok(task) => tasks.push(task),
                Err(error) => {
                    crate::diagnostics::log_error("Reopen after a branch switch", &error);
                }
            }
        }
        tasks
    }
}
