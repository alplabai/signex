//! Where a reference fix and Open in notebook meet the notebook tab. A
//! notebook file is written only on Save, so a fix edits the text the
//! notebook tab holds, never the file on disk. The tab holds the notebook
//! file itself; a chapter file has no buffer, and while the tab holds no
//! text a fix says so.

use iced::Task;
use signex_i18n::{ids, t};
use signex_notebook::{
    DEFAULT_NOTEBOOK_FILE, Fix, Reference, ReferenceReport, apply_fix, apply_plan, plan_update_all,
};

use crate::app::state::ProjectId;
use crate::app::{Message, NotebookMsg, Signex};

impl Signex {
    /// The project whose notebook the references were read from.
    fn references_project(&self) -> Option<ProjectId> {
        self.ui_state
            .annotations
            .target
            .as_ref()
            .map(|(project, _)| *project)
    }

    /// The text of the notebook file `file` (inside `notebook/`) as the
    /// notebook tab holds it, unsaved edits included; `None` while no tab
    /// holds it.
    pub(crate) fn notebook_buffer(&self, file: &str) -> Option<String> {
        if file != DEFAULT_NOTEBOOK_FILE {
            return None;
        }
        self.notebook_tab_file_text(self.references_project()?)
    }

    /// Put `text` in the notebook tab's buffer of `file`, written on Save;
    /// the tab compiles again.
    pub(crate) fn replace_notebook_buffer(
        &mut self,
        file: &str,
        text: String,
    ) -> Result<Task<Message>, String> {
        let project = self
            .references_project()
            .filter(|project| {
                file == DEFAULT_NOTEBOOK_FILE && self.notebook_tab_file_text(*project).is_some()
            })
            .ok_or_else(|| {
                t!(self.ui_state.i18n, ids::NOTEBOOK_REF_OPEN_FIRST, "file" => file).into_string()
            })?;
        self.replace_notebook_tab_file_text(project, &text)
    }

    /// Open the notebook tab at the block of `file` starting on `line`: the
    /// tab opens (or comes forward) and, when it already holds the
    /// notebook file's text, its cursor moves to the line.
    pub(crate) fn open_notebook_at(&mut self, file: &str, line: u32) -> Task<Message> {
        let text = t!(
            self.ui_state.i18n, ids::NOTEBOOK_REF_OPEN_AT,
            "file" => file, "line" => line
        );
        crate::diagnostics::log_info(text.into_string());
        let Some(project) = self.references_project() else {
            return Task::none();
        };
        let open = self.dispatch_notebook_message(NotebookMsg::OpenRequested {
            project: Some(project),
        });
        if file == DEFAULT_NOTEBOOK_FILE {
            let moved = self.dispatch_notebook_message(NotebookMsg::LinePicked {
                project_id: project,
                line,
            });
            return Task::batch([open, moved]);
        }
        open
    }

    /// Update all: every renamed reference of `report` updated in the
    /// notebook tab's text of `file`; the others are left for a choice.
    pub(crate) fn apply_update_all(
        &mut self,
        file: &str,
        report: &ReferenceReport,
    ) -> Result<(usize, Task<Message>), String> {
        let Some(text) = self.notebook_buffer(file) else {
            let line = t!(self.ui_state.i18n, ids::NOTEBOOK_REF_OPEN_FIRST, "file" => file);
            return Err(line.into_string());
        };
        let fixed = apply_plan(&text, &plan_update_all(report)).map_err(|why| why.to_string())?;
        let task = self.replace_notebook_buffer(file, fixed.text)?;
        Ok((fixed.changed, task))
    }

    /// Apply `fix` to `reference` in the notebook tab's text of `file`;
    /// how many uses changed, or why nothing did.
    pub(crate) fn fix_reference_in_buffer(
        &mut self,
        file: &str,
        reference: &Reference,
        fix: &Fix,
    ) -> Result<(usize, Task<Message>), String> {
        let Some(text) = self.notebook_buffer(file) else {
            let line = t!(self.ui_state.i18n, ids::NOTEBOOK_REF_OPEN_FIRST, "file" => file);
            return Err(line.into_string());
        };
        let fixed = apply_fix(&text, reference, fix).map_err(|why| why.to_string())?;
        let task = self.replace_notebook_buffer(file, fixed.text)?;
        Ok((fixed.changed, task))
    }
}
