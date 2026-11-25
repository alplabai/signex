//! The notebook tab's router: one arm per message, no wildcard, so a new
//! message is a compile error until it is routed. The work sits in the
//! files named after what it acts on (`open`, `edit`, `compile`, `save`).

use std::path::PathBuf;

use iced::Task;
use signex_i18n::ids;
use signex_notebook::{NotebookFile, Template};

use super::{Load, NotebookMsg, NotebookText, SidePanel};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

impl Signex {
    pub(crate) fn dispatch_notebook_message(&mut self, msg: NotebookMsg) -> Task<Message> {
        use NotebookMsg as M;
        // A message for a project that closed meanwhile does nothing; a
        // command's caller is still answered.
        if !matches!(msg, M::CommandCompiled { .. })
            && let Some(id) = msg.project_id()
            && self.document_state.project_by_id(id).is_none()
        {
            return Task::none();
        }
        match msg {
            M::OpenRequested { project } => match self.rules_project_for(project) {
                Some(id) => self.open_notebook_tab(id),
                None => Task::none(),
            },
            M::Read { project_id, result } => self.notebook_read(project_id, result),
            M::TextActed { project_id, action } => self.notebook_text_acted(project_id, action),
            M::ViewPicked { project_id, view } => {
                self.on_notebook(project_id, |tab| tab.view = view);
                Task::none()
            }
            M::ViewCycled { project_id } => {
                self.on_notebook(project_id, |tab| tab.view = tab.view.next());
                Task::none()
            }
            M::PanelPicked { project_id, panel } => self.notebook_panel_picked(project_id, panel),
            M::LinePicked { project_id, line } => {
                self.notebook_line_picked(project_id, line);
                Task::none()
            }
            M::InsertPressed {
                project_id,
                insertion,
            } => self.notebook_insert(project_id, &insertion),
            M::BlockMenuToggled { project_id } => {
                self.on_notebook(project_id, |tab| {
                    tab.block_menu = !tab.block_menu;
                    tab.picker = None;
                });
                Task::none()
            }
            M::PickerOpened { project_id } => self.open_notebook_picker(project_id),
            M::PickerQueryChanged { project_id, query } => {
                self.on_notebook(project_id, |tab| {
                    if let Some(picker) = &mut tab.picker {
                        picker.query = query;
                        picker.row = 0;
                    }
                });
                Task::none()
            }
            M::PickerMoved { project_id, down } => {
                self.on_notebook(project_id, |tab| {
                    if let Some(picker) = &mut tab.picker {
                        picker.step(down);
                    }
                });
                Task::none()
            }
            M::PickerChosen { project_id, row } => self.notebook_picker_chosen(project_id, row),
            M::PickerClosed { project_id } => {
                self.on_notebook(project_id, |tab| tab.picker = None);
                Task::none()
            }
            M::Compiled {
                project_id,
                outcome,
            } => self.notebook_compiled(project_id, outcome),
            M::PageStepped {
                project_id,
                forward,
            } => self.notebook_page_stepped(project_id, forward),
            M::PagePicked { project_id, page } => self.draw_notebook_page(project_id, page),
            M::PageDrawn {
                project_id,
                generation,
                page,
                image,
            } => {
                self.notebook_page_drawn(project_id, generation, page, &image);
                Task::none()
            }
            M::NextProblemRequested { project_id } => {
                let _moved = self.notebook_next_problem(project_id);
                Task::none()
            }
            M::SavePressed { project_id } => self.save_notebook(project_id),
            M::Saved {
                project_id,
                written,
                result,
            } => {
                self.notebook_saved(project_id, &written, result);
                Task::none()
            }
            M::TemplateSelected {
                project_id,
                template,
            } => {
                self.on_notebook(project_id, |tab| tab.template = template);
                Task::none()
            }
            M::TemplatePicked {
                project_id,
                template,
            } => self.notebook_template_picked(project_id, template),
            M::ImportPressed { project_id } => self.pick_notebook_import(project_id),
            M::ImportPicked { project_id, paths } => self.notebook_import_picked(project_id, paths),
            M::ImportRead { project_id, result } => self.notebook_import_read(project_id, result),
            M::CommandCompiled {
                project_id: _,
                waiting,
                path,
                answer,
            } => self.notebook_command_compiled(*waiting, &path, answer.as_ref()),
            M::EscapePressed { project_id } => {
                self.on_notebook(project_id, |tab| {
                    if tab.picker.is_some() {
                        tab.picker = None;
                    } else {
                        tab.block_menu = false;
                    }
                });
                Task::none()
            }
        }
    }

    /// Run `change` on `id`'s notebook tab state, if it is held.
    fn on_notebook(&mut self, id: ProjectId, change: impl FnOnce(&mut super::NotebookTab)) {
        if let Some(tab) = self.document_state.notebooks.get_mut(&id) {
            change(tab);
        }
    }

    /// A side panel was picked. Pages needs thumbnails, which a compile
    /// draws: compile once when they are missing.
    pub(super) fn notebook_panel_picked(
        &mut self,
        id: ProjectId,
        panel: SidePanel,
    ) -> Task<Message> {
        let Some(tab) = self.document_state.notebooks.get_mut(&id) else {
            return Task::none();
        };
        tab.panel = panel;
        if panel == SidePanel::Pages && tab.compile.thumbnails.is_empty() {
            return self.request_notebook_compile(id);
        }
        Task::none()
    }

    /// The page navigator's previous or next page.
    fn notebook_page_stepped(&mut self, id: ProjectId, forward: bool) -> Task<Message> {
        let Some(tab) = self.document_state.notebooks.get(&id) else {
            return Task::none();
        };
        let count = tab
            .compile
            .last_good
            .as_ref()
            .map_or(0, |good| good.status.pages.len());
        let page = if forward {
            (tab.page + 1).min(count.saturating_sub(1))
        } else {
            tab.page.saturating_sub(1)
        };
        if page == tab.page {
            return Task::none();
        }
        self.draw_notebook_page(id, page)
    }

    /// The empty state's template: a new notebook in the editor, unsaved;
    /// the file is written on Save.
    fn notebook_template_picked(&mut self, id: ProjectId, template: Template) -> Task<Message> {
        let name = self
            .document_state
            .project_by_id(id)
            .map(crate::app::page_tab::project_display_name)
            .unwrap_or_default();
        let Some(today) = super::compile::today() else {
            crate::diagnostics::log_warning("[notebook] the clock gives no date");
            return Task::none();
        };
        let Some(tab) = self.document_state.notebooks.get_mut(&id) else {
            return Task::none();
        };
        if tab.load != Load::Missing {
            return Task::none();
        }
        let file = template.file(&name, today);
        start_new(tab, file);
        self.notebook_edited(id)
    }

    /// The empty state's Import .typ files…: pick one or more `.typ` files.
    fn pick_notebook_import(&mut self, id: ProjectId) -> Task<Message> {
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::NOTEBOOK_TAB_IMPORT_TITLE)
            .filter_in(i18n, ids::NOTEBOOK_TAB_IMPORT_FILTER, &["typ"]);
        Task::perform(dialog.pick_files(), move |paths| {
            Message::from(NotebookMsg::ImportPicked {
                project_id: id,
                paths,
            })
        })
    }

    /// The import picker closed: read the files, in the order picked.
    fn notebook_import_picked(
        &mut self,
        id: ProjectId,
        paths: Option<Vec<PathBuf>>,
    ) -> Task<Message> {
        let Some(paths) = paths.filter(|paths| !paths.is_empty()) else {
            return Task::none();
        };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_typ_files(&paths))
                    .await
                    .map_err(|error| format!("the import did not finish: {error}"))
                    .and_then(|read| read)
            },
            move |result| {
                Message::from(NotebookMsg::ImportRead {
                    project_id: id,
                    result,
                })
            },
        )
    }

    /// The imported Typst becomes a new notebook's body, unsaved.
    fn notebook_import_read(
        &mut self,
        id: ProjectId,
        result: Result<String, String>,
    ) -> Task<Message> {
        let body = match result {
            Ok(body) => body,
            Err(why) => {
                crate::diagnostics::log_warning(format!("[notebook] Import failed: {why}"));
                return Task::none();
            }
        };
        let Some(tab) = self.document_state.notebooks.get_mut(&id) else {
            return Task::none();
        };
        if tab.load != Load::Missing {
            return Task::none();
        }
        start_new(tab, NotebookFile::new(body));
        self.notebook_edited(id)
    }
}

/// `tab`, empty until now, holds the new notebook `file`, not saved.
pub(super) fn start_new(tab: &mut super::NotebookTab, file: NotebookFile) {
    tab.text = Some(NotebookText::new(&file.body));
    tab.header_lines = 0;
    tab.saved_body = None;
    tab.file = Some(file);
    tab.load = Load::Open;
}

/// Blocking: the text of the `.typ` files at `paths`, one after the other.
fn read_typ_files(paths: &[PathBuf]) -> Result<String, String> {
    let mut body = String::new();
    for path in paths {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(&text);
    }
    Ok(body)
}
