//! Open, focus and close a project's notebook tab. One tab per project; its
//! path is the notebook file, `<project>/notebook/design.snxnbk`. The file
//! and the chapters it includes are read off the UI thread; with no file
//! the tab shows the empty state, and nothing is written until Save.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_notebook::{
    DEFAULT_NOTEBOOK_FILE, LoadProblem, NotebookFile, NotebookSources, header_line_count,
    notebook_path,
};

use super::{Load, NotebookMsg, NotebookTab, NotebookText};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex, TabInfo, TabKind};

/// The notebook file's path inside the notebook folder.
pub(super) const MAIN_FILE: &str = DEFAULT_NOTEBOOK_FILE;

/// What the read of a notebook found.
#[derive(Debug)]
pub struct NotebookRead {
    /// The notebook file's text; `None` when there is no file yet.
    pub text: Option<String>,
    /// The notebook file and every chapter it includes.
    pub sources: NotebookSources,
    /// The chapters that could not be read.
    pub problems: Vec<LoadProblem>,
}

/// Blocking: read the notebook in `folder` (the project's `notebook/`) and
/// its chapters. Run from `spawn_blocking`.
pub(super) fn read_notebook(folder: &Path) -> Result<NotebookRead, String> {
    let main = folder.join(MAIN_FILE);
    match std::fs::read(&main) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(NotebookRead {
            text: None,
            sources: NotebookSources::new(MAIN_FILE, Vec::new()),
            problems: Vec::new(),
        }),
        Err(error) => Err(format!("{}: {error}", main.display())),
        Ok(_) => {
            let (sources, problems) =
                NotebookSources::load(MAIN_FILE, |path| std::fs::read(folder.join(path)));
            let bytes = sources.get(MAIN_FILE).unwrap_or_default().to_vec();
            // A file that is not UTF-8 is not edited: a save would change
            // bytes the person never touched.
            let text = String::from_utf8(bytes)
                .map_err(|_| format!("{} is not UTF-8 text", main.display()))?;
            Ok(NotebookRead {
                text: Some(text),
                sources,
                problems,
            })
        }
    }
}

impl Signex {
    /// The notebook file of project `id`.
    pub(crate) fn notebook_path_of(&self, id: ProjectId) -> Option<PathBuf> {
        let project = self.document_state.project_by_id(id)?;
        Some(notebook_path(project.dir()))
    }

    /// The index of `id`'s notebook tab, if it is open.
    pub(crate) fn notebook_tab_index(&self, id: ProjectId) -> Option<usize> {
        self.document_state
            .tabs
            .iter()
            .position(|tab| matches!(tab.kind, TabKind::Notebook(held) if held == id))
    }

    /// Focus `id`'s notebook tab, or open it at the end of the strip; read
    /// the file when the tab holds no text yet.
    pub(crate) fn open_notebook_tab(&mut self, id: ProjectId) -> Task<Message> {
        let Some(path) = self.notebook_path_of(id) else {
            return Task::none();
        };
        let existing = self.notebook_tab_index(id);
        let idx = match existing {
            Some(idx) => idx,
            None => {
                let kind = TabKind::Notebook(id);
                // A page tab: "<project> — Design Notebook".
                let title = self.page_tab_title(&kind).unwrap_or_default();
                self.document_state.tabs.push(TabInfo {
                    title,
                    path: path.clone(),
                    cached_document: None,
                    project_id: Some(id),
                    kind,
                });
                self.document_state.tabs.len() - 1
            }
        };
        if existing.is_none() || idx != self.document_state.active_tab {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }
        let needs_read = self
            .document_state
            .notebooks
            .get(&id)
            .is_none_or(|tab| tab.text.is_none() && tab.load != Load::Reading);
        if !needs_read {
            return Task::none();
        }
        self.document_state
            .notebooks
            .insert(id, NotebookTab::reading(path.clone()));
        let folder = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_notebook(&folder))
                    .await
                    .map_err(|error| format!("the notebook read did not finish: {error}"))
                    .and_then(|read| read)
            },
            move |result| {
                Message::from(NotebookMsg::Read {
                    project_id: id,
                    result: result.map(Arc::new),
                })
            },
        )
    }

    /// The read of `id`'s notebook landed.
    pub(super) fn notebook_read(
        &mut self,
        id: ProjectId,
        result: Result<Arc<NotebookRead>, String>,
    ) -> Task<Message> {
        let Some(tab) = self.document_state.notebooks.get_mut(&id) else {
            return Task::none();
        };
        // An edit or a template since the read started wins over it.
        if tab.load != Load::Reading {
            return Task::none();
        }
        let read = match result {
            Ok(read) => read,
            Err(why) => {
                crate::diagnostics::log_warning(format!("[notebook] {why}"));
                tab.load = Load::Failed(why);
                return Task::none();
            }
        };
        for problem in &read.problems {
            crate::diagnostics::log_warning(format!(
                "[notebook] {} line {}: {} could not be read",
                problem.file, problem.line, problem.path
            ));
        }
        tab.sources = read.sources.clone();
        let Some(text) = read.text.as_deref() else {
            tab.load = Load::Missing;
            return Task::none();
        };
        match NotebookFile::parse(text) {
            Ok(file) => {
                tab.header_lines = header_line_count(text);
                tab.text = Some(NotebookText::new(&file.body));
                tab.saved_body = Some(file.body.clone());
                tab.file = Some(file);
                tab.load = Load::Open;
                tab.refresh_text();
                self.request_notebook_compile(id)
            }
            Err(error) => {
                let why = format!("{}: {error}", tab.path.display());
                crate::diagnostics::log_warning(format!("[notebook] {why}"));
                tab.load = Load::Failed(why);
                Task::none()
            }
        }
    }

    /// Close the notebook tab at `idx`. Its state stays with the project,
    /// so unsaved text is still there when the tab opens again, and still
    /// counts as unsaved for Save All and the close and quit prompts.
    pub(crate) fn close_notebook_tab(&mut self, idx: usize, keep_session: bool) -> Task<Message> {
        if idx >= self.document_state.tabs.len() {
            return Task::none();
        }
        self.document_state.tabs.remove(idx);
        let len = self.document_state.tabs.len();
        let active = self.document_state.active_tab;
        if active > 0 && (idx < active || active >= len) {
            self.document_state.active_tab = active - 1;
        }
        if keep_session {
            self.settle_document_tiling();
        } else {
            self.sync_active_tab();
        }
        self.refresh_panel_ctx();
        Task::none()
    }

    /// The project closed: its notebook goes with it.
    pub(crate) fn forget_notebook(&mut self, id: ProjectId) {
        self.document_state.notebooks.remove(&id);
    }

    /// The project whose notebook tab the main window shows, if it shows
    /// one.
    pub(crate) fn notebook_tab_in_window(&self, is_main: bool) -> Option<ProjectId> {
        if !is_main {
            return None;
        }
        let docs = &self.document_state;
        docs.tabs
            .get(docs.active_tab)
            .and_then(|tab| tab.kind.as_notebook())
    }

    /// The project whose notebook file is `path`, if its notebook is held:
    /// the path it was opened at, or where its project puts it now.
    pub(crate) fn notebook_at_path(&self, path: &Path) -> Option<ProjectId> {
        self.document_state
            .notebooks
            .iter()
            .find(|(id, tab)| {
                tab.path == path || self.notebook_path_of(**id).is_some_and(|now| now == path)
            })
            .map(|(id, _)| *id)
    }

    /// Follow a project folder that moved: the notebook's state, its dirty
    /// mark and its tab take the path the project puts it at now, so a save
    /// never writes into the old folder. Returns that path.
    pub(super) fn sync_notebook_path(&mut self, id: ProjectId) -> Option<PathBuf> {
        let now = self.notebook_path_of(id)?;
        let tab = self.document_state.notebooks.get_mut(&id)?;
        if tab.path != now {
            let old = std::mem::replace(&mut tab.path, now.clone());
            self.document_state.rekey_document_dirty(&old, &now);
            if let Some(idx) = self.notebook_tab_index(id) {
                self.document_state.tabs[idx].path = now.clone();
            }
        }
        Some(now)
    }
}
