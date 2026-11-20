//! Creating the child sheet a sheet symbol points at (#1275).
//!
//! The file is written beside the PARENT sheet, where descending resolves it
//! (`resolve_child_sheet_path`), with one hierarchical label per entry on the
//! symbol. The write runs off the update thread and never overwrites. It is
//! not an edit: like Save it is not undone, and the Messages panel records it.
//! Design: docs/HIERARCHY.md, "Creating the child sheet".

use iced::Task;
use signex_i18n::RenderTranslatable;
use uuid::Uuid;

use super::super::super::*;
use super::super::dock::project_navigation::add_io;
use super::child_sheet_path::{
    escapes_through_symlink, reference_from, relative_within, resolve_new_sheet_path,
};
use crate::app::contracts::CreateChildSheetConfirmState;
use crate::app::state::{ProjectId, untranslated};
use add_io::ChildSheetWriteError;
use signex_i18n::{Translatable, ids};

/// What creating a child sheet writes, decided in `update`, written on a worker.
pub(crate) struct ChildSheetPlan {
    pub parent: std::path::PathBuf,
    pub symbol: Uuid,
    pub path: std::path::PathBuf,
    pub document: signex_types::schematic::SchematicSheet,
    pub project_id: Option<ProjectId>,
}

impl Signex {
    /// A sheet symbol was descended into (double-click, Enter, "Open Child
    /// Sheet", the context menu) but its file is missing: open the Create
    /// Sheet dialog. `loud`: an unsaved parent gets a notice, not just a log.
    pub(crate) fn open_create_child_sheet_dialog(&mut self, symbol: Uuid, loud: bool) {
        let Some((name, filename)) = self.sheet_symbol_identity(symbol) else {
            return;
        };
        let Some(parent) = self.saved_parent_sheet() else {
            let detail = Translatable::new(ids::NOTICE_CHILD_SHEET_NO_FOLDER).arg("name", name);
            if loud {
                self.refuse_child_sheet(detail);
            } else {
                crate::diagnostics::log_warning(detail.render_en_us());
            }
            return;
        };
        let project_dir = self
            .document_state
            .document_project(&parent)
            .map(|p| p.dir().to_path_buf())
            .or_else(|| parent.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_default();
        crate::diagnostics::log_info(format!(
            "Sheet '{name}' has no file yet; asking where to create it"
        ));
        self.ui_state.create_child_sheet_confirm = Some(CreateChildSheetConfirmState {
            symbol,
            name,
            parent,
            project_dir,
            path_text: filename,
            error: None,
            exists: false,
            reuse_warning: None,
        });
    }

    /// The context-menu route: open the child when its file exists, else the
    /// Create Sheet dialog.
    pub(crate) fn create_child_sheet_from_symbol(&mut self, symbol_id: Uuid) -> Task<Message> {
        match self.sheet_symbol_identity(symbol_id) {
            Some((_, filename)) => self.descend_through_sheet_symbol(symbol_id, &filename),
            None => Task::none(),
        }
    }

    fn sheet_symbol_identity(&self, symbol: Uuid) -> Option<(String, String)> {
        let engine = self.document_state.active_engine()?;
        let found = engine
            .document()
            .child_sheets
            .iter()
            .find(|c| c.uuid == symbol);
        found.map(|c| (c.name.clone(), c.filename.clone()))
    }

    /// The focused sheet's path when it is a file on disk: the folder the
    /// child is created in.
    fn saved_parent_sheet(&self) -> Option<std::path::PathBuf> {
        self.active_tab_path().filter(|path| path.is_file())
    }

    /// Say why nothing was created, on screen and in the Messages panel.
    fn refuse_child_sheet(&mut self, detail: Translatable) {
        crate::diagnostics::log_warning(detail.render_en_us());
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::create(detail));
    }

    pub(crate) fn handle_create_child_sheet_path_changed(&mut self, text: String) -> Task<Message> {
        if let Some(confirm) = self.ui_state.create_child_sheet_confirm.as_mut() {
            confirm.path_text = text;
            confirm.error = None;
            confirm.exists = false;
            confirm.reuse_warning = None;
        }
        Task::none()
    }

    pub(crate) fn handle_create_child_sheet_browse(&mut self) -> Task<Message> {
        let Some(confirm) = self.ui_state.create_child_sheet_confirm.as_ref() else {
            return Task::none();
        };
        let dir = confirm.parent.parent().map(std::path::Path::to_path_buf);
        let name = std::path::Path::new(&confirm.path_text)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let picker = self.ui_state.io.picker.clone();
        let i18n = self.ui_state.i18n.clone();
        Task::perform(
            async move {
                let mut dialog = picker
                    .dialog()
                    .title_in(&i18n, ids::FILE_DIALOG_CREATE_SHEET)
                    .filter_in(&i18n, ids::FILE_FILTER_SCHEMATIC, &["snxsch"])
                    .set_file_name(&name);
                if let Some(dir) = dir {
                    dialog = dialog.set_directory(dir);
                }
                dialog.save_file().await
            },
            |picked| Message::Project(ProjectMsg::CreateChildSheetBrowsed(picked)),
        )
    }

    pub(crate) fn handle_create_child_sheet_browsed(
        &mut self,
        picked: Option<std::path::PathBuf>,
    ) -> Task<Message> {
        let fold = self
            .ui_state
            .create_child_sheet_confirm
            .as_ref()
            .map_or_else(crate::app::state::CaseFold::default, |c| {
                crate::app::state::project_case::fold_of(
                    self.document_state.document_project(&c.parent),
                )
            });
        let (Some(confirm), Some(picked)) =
            (self.ui_state.create_child_sheet_confirm.as_mut(), picked)
        else {
            return Task::none();
        };
        let dir = confirm
            .parent
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""));
        confirm.path_text = match relative_within(dir, &picked, fold) {
            Some(relative) => relative.to_string_lossy().replace('\\', "/"),
            None => picked.to_string_lossy().into_owned(),
        };
        confirm.error = None;
        confirm.exists = false;
        confirm.reuse_warning = None;
        Task::none()
    }

    pub(crate) fn handle_create_child_sheet_answered(&mut self, create: bool) -> Task<Message> {
        if !create {
            if let Some(confirm) = self.ui_state.create_child_sheet_confirm.take() {
                crate::diagnostics::log_info(format!(
                    "Sheet '{}' was left without a file",
                    confirm.name
                ));
            }
            return Task::none();
        }
        let Some(plan) = self.plan_child_sheet_from_dialog() else {
            return Task::none();
        };
        self.ui_state.create_child_sheet_confirm = None;
        self.begin_project_write(&plan.path);
        let ChildSheetPlan {
            parent,
            symbol,
            path,
            document,
            project_id,
        } = plan;
        Task::perform(
            add_io::write_new_schematic_off_thread(path.clone(), document),
            move |result| {
                Message::Project(ProjectMsg::ChildSheetCreated {
                    project_id,
                    parent: parent.clone(),
                    symbol,
                    path: path.clone(),
                    result,
                })
            },
        )
    }

    /// "Use existing file": the typed path already exists; link the symbol to
    /// it and open it, writing nothing. If the symbol cannot be linked (another
    /// symbol already uses that file) the dialog stays open with the reason.
    pub(crate) fn handle_create_child_sheet_open_existing(&mut self) -> Task<Message> {
        let Some(confirm) = self.ui_state.create_child_sheet_confirm.clone() else {
            return Task::none();
        };
        let Ok(path) = self.resolve_dialog_path(&confirm) else {
            return Task::none();
        };
        let project_id = self
            .document_state
            .document_project(&confirm.parent)
            .map(|p| p.id);
        match self.link_and_open_child_sheet(project_id, &confirm.parent, confirm.symbol, path) {
            Ok(task) => {
                self.ui_state.create_child_sheet_confirm = None;
                task
            }
            Err(why) => {
                self.set_dialog_error(why, true);
                Task::none()
            }
        }
    }

    fn resolve_dialog_path(
        &self,
        confirm: &CreateChildSheetConfirmState,
    ) -> Result<std::path::PathBuf, Translatable> {
        let parent_dir = confirm
            .parent
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""));
        let fold = crate::app::state::project_case::fold_of(
            self.document_state.document_project(&confirm.parent),
        );
        // The path rules also answer the command line, in English (#2061).
        let path =
            resolve_new_sheet_path(parent_dir, &confirm.project_dir, &confirm.path_text, fold)
                .map_err(untranslated)?;
        if escapes_through_symlink(&confirm.project_dir, &path) {
            return Err(ids::DIALOG_CREATE_CHILD_SHEET_LINKED_FOLDER.into());
        }
        Ok(path)
    }

    /// Create: validate what was typed and decide what to write. `None` keeps
    /// the dialog open, with the reason in `error` (a bad path, a file that
    /// is already there) or in the log (the symbol is gone).
    pub(crate) fn plan_child_sheet_from_dialog(&mut self) -> Option<ChildSheetPlan> {
        let confirm = self.ui_state.create_child_sheet_confirm.clone()?;
        let path = match self.resolve_dialog_path(&confirm) {
            Ok(path) => path,
            Err(why) => {
                self.set_dialog_error(why, false);
                return None;
            }
        };
        if path.exists() {
            let (why, offer_open) = self.existing_file_reply(&confirm, &path);
            let warning = offer_open
                .then(|| self.reuse_warning(&confirm, &path))
                .flatten();
            self.set_dialog_error(why, offer_open);
            if let Some(confirm) = self.ui_state.create_child_sheet_confirm.as_mut() {
                confirm.reuse_warning = warning;
            }
            return None;
        }
        if let Err(why) = self.refuse_while_project_moves(&path, "create") {
            self.set_dialog_error(untranslated(why.to_string()), false);
            return None;
        }
        let symbol = self
            .document_state
            .engines
            .get(&confirm.parent)
            .and_then(|e| {
                e.document()
                    .child_sheets
                    .iter()
                    .find(|c| c.uuid == confirm.symbol)
            })
            .cloned();
        let Some(symbol) = symbol else {
            crate::diagnostics::log_warning(format!(
                "The sheet symbol is no longer in {}; no child sheet was created",
                confirm.parent.display()
            ));
            self.ui_state.create_child_sheet_confirm = None;
            return None;
        };
        let project_id = self
            .document_state
            .document_project(&confirm.parent)
            .map(|p| p.id);
        // A new sheet starts from its project's sheet defaults (#2232).
        let mut blank = crate::app::handlers::document_files::blank_schematic_sheet_for_new_doc();
        if let Some(project) = project_id.and_then(|id| self.document_state.project_by_id(id)) {
            project.data.apply_sheet_defaults(&mut blank);
        }
        let document = signex_engine::child_document_for_symbol(blank, &symbol);
        Some(ChildSheetPlan {
            parent: confirm.parent,
            symbol: confirm.symbol,
            path,
            document,
            project_id,
        })
    }

    /// What the dialog says about a typed path that already holds a file, and
    /// whether "Use existing file" is offered. It is not when another sheet symbol of
    /// the parent already points at the file: one file is placed once, and
    /// linking this symbol too would be refused (#1835).
    fn existing_file_reply(
        &self,
        confirm: &CreateChildSheetConfirmState,
        path: &std::path::Path,
    ) -> (Translatable, bool) {
        let fold = crate::app::state::project_case::fold_of(
            self.document_state.document_project(&confirm.parent),
        );
        let taken_by = confirm
            .parent
            .parent()
            .map(|dir| reference_from(dir, path, fold))
            .and_then(|reference| {
                let engine = self.document_state.engines.get(&confirm.parent)?;
                signex_engine::other_sheet_symbol_using_file(
                    &engine.document().child_sheets,
                    confirm.symbol,
                    &reference,
                )
                .map(|other| other.name.clone())
            });
        match taken_by {
            Some(other) => (
                Translatable::new(ids::DIALOG_CREATE_CHILD_SHEET_FILE_PLACED).arg("symbol", other),
                false,
            ),
            None => (ids::DIALOG_CREATE_CHILD_SHEET_FILE_EXISTS.into(), true),
        }
    }

    /// What using the existing file at `path` would make of the symbol (mockup row 27):
    /// a copy of a sheet the project already has. Read from the Model only:
    /// the project's sheet list (`page` is the file's position in it, from 1)
    /// and the sheet symbols of the other open sheets. `None` when neither
    /// shows the file.
    fn reuse_warning(
        &self,
        confirm: &CreateChildSheetConfirmState,
        path: &std::path::Path,
    ) -> Option<Translatable> {
        use crate::app::state::scope::path_key_with;
        let project = self.document_state.document_project(&confirm.parent);
        let fold = crate::app::state::project_case::fold_of(project);
        let key = path_key_with(path, fold);
        let file = path.file_name()?.to_string_lossy().into_owned();
        let page = project.and_then(|p| {
            p.data
                .sheets
                .iter()
                .position(|entry| path_key_with(&p.dir().join(&entry.filename), fold) == key)
        });
        let placed_elsewhere = || {
            self.document_state.engines.iter().any(|(sheet, engine)| {
                let Some(dir) = sheet.parent().filter(|_| *sheet != confirm.parent) else {
                    return false;
                };
                engine.document().child_sheets.iter().any(|symbol| {
                    crate::app::project_sheets::resolve_child_reference_quiet(dir, &symbol.filename)
                        .is_ok_and(|child| path_key_with(&child, fold) == key)
                })
            })
        };
        let warning = match page {
            Some(index) => {
                Translatable::new(ids::DIALOG_CREATE_CHILD_SHEET_REUSE_PAGE).arg("page", index + 1)
            }
            None if placed_elsewhere() => Translatable::new(ids::DIALOG_CREATE_CHILD_SHEET_REUSE),
            None => return None,
        };
        Some(warning.arg("file", file).arg("name", confirm.name.clone()))
    }

    fn set_dialog_error(&mut self, why: Translatable, exists: bool) {
        if let Some(confirm) = self.ui_state.create_child_sheet_confirm.as_mut() {
            confirm.error = Some(why);
            confirm.exists = exists;
            if !exists {
                confirm.reuse_warning = None;
            }
        }
    }

    /// The write landed (or failed).
    pub(crate) fn handle_child_sheet_created(
        &mut self,
        project_id: Option<ProjectId>,
        parent: &std::path::Path,
        symbol: Uuid,
        path: std::path::PathBuf,
        result: Result<(), ChildSheetWriteError>,
    ) -> Task<Message> {
        self.end_project_write(&path);
        self.finish_child_sheet(project_id, parent, symbol, path, result)
    }

    /// Log the outcome, then link the symbol, register the file and open it.
    fn finish_child_sheet(
        &mut self,
        project_id: Option<ProjectId>,
        parent: &std::path::Path,
        symbol: Uuid,
        path: std::path::PathBuf,
        result: Result<(), ChildSheetWriteError>,
    ) -> Task<Message> {
        match result {
            Ok(()) => {
                crate::diagnostics::log_info(format!("Created child sheet {}", path.display()))
            }
            Err(ChildSheetWriteError::AlreadyExists) => crate::diagnostics::log_info(format!(
                "Child sheet {} already exists; used it instead of overwriting",
                path.display()
            )),
            Err(ChildSheetWriteError::Failed(e)) => {
                crate::diagnostics::log_error("Create child sheet", &anyhow::anyhow!("{e}"));
                self.document_state.error_notice =
                    Some(crate::app::state::ErrorNotice::create(untranslated(e)));
                return Task::none();
            }
        }
        match self.link_and_open_child_sheet(project_id, parent, symbol, path) {
            Ok(task) => task,
            Err(why) => {
                self.refuse_child_sheet(why);
                Task::none()
            }
        }
    }

    /// Point the symbol at `path` FIRST (the engine may refuse), then list the
    /// file in the project and open it. On refusal nothing is registered or
    /// opened and the engine's reason is returned.
    fn link_and_open_child_sheet(
        &mut self,
        project_id: Option<ProjectId>,
        parent: &std::path::Path,
        symbol: Uuid,
        path: std::path::PathBuf,
    ) -> Result<Task<Message>, Translatable> {
        self.point_symbol_at_file(parent, symbol, &path)?;
        if let Some(id) = project_id {
            self.register_child_sheet_in_project(id, &path);
        }
        let filename = self
            .document_state
            .engines
            .get(parent)
            .and_then(|e| e.document().child_sheets.iter().find(|c| c.uuid == symbol))
            .map(|c| c.filename.clone());
        Ok(match filename {
            Some(filename) => self.open_existing_child_sheet(parent, symbol, &filename, path),
            None => self.open_sheet_with_placement(path, None),
        })
    }

    /// Set the symbol's File field to `path`, relative to the parent sheet:
    /// one undoable step on the parent's document (focused first, as edits go
    /// through the active tab). `Err` is the engine's refusal, or why the
    /// parent could not be edited.
    fn point_symbol_at_file(
        &mut self,
        parent: &std::path::Path,
        symbol: Uuid,
        path: &std::path::Path,
    ) -> Result<(), Translatable> {
        let Some(dir) = parent.parent() else {
            return Ok(());
        };
        let fold =
            crate::app::state::project_case::fold_of(self.document_state.document_project(parent));
        let reference = reference_from(dir, path, fold);
        let current = self
            .document_state
            .engines
            .get(parent)
            .and_then(|e| e.document().child_sheets.iter().find(|c| c.uuid == symbol))
            .map(|c| c.filename.clone());
        let Some(current) = current else {
            return Err(ids::DIALOG_CREATE_CHILD_SHEET_SYMBOL_GONE.into());
        };
        if current == reference {
            return Ok(());
        }
        if self.active_tab_path().as_deref() != Some(parent) && !self.focus_open_sheet(parent) {
            return Err(
                Translatable::new(ids::DIALOG_CREATE_CHILD_SHEET_NOT_FOCUSED)
                    .arg("path", parent.display().to_string()),
            );
        }
        let (_, refused) = self.apply_engine_command_reporting(
            signex_engine::Command::ModifyChildSheet {
                sheet_id: symbol,
                name: None,
                filename: Some(reference),
            },
            false,
            true,
        );
        // The engine words its own refusals (#2060).
        refused.map_or(Ok(()), |why| Err(untranslated(why)))
    }

    /// Open `path`, the child of `symbol` of `parent`: through the symbol when
    /// `parent` is the focused sheet (recording the placement), else plainly.
    fn open_existing_child_sheet(
        &mut self,
        parent: &std::path::Path,
        symbol: Uuid,
        filename: &str,
        path: std::path::PathBuf,
    ) -> Task<Message> {
        if self.active_tab_path().as_deref() == Some(parent) {
            return self.descend_through_sheet_symbol(symbol, filename);
        }
        self.open_sheet_with_placement(path, None)
    }

    /// List `path` among the project's sheets (relative to the project folder)
    /// and mark the `.snxprj` dirty; it persists on Save, like Add New. A file
    /// outside the project folder, or a project closed during the write, is
    /// left unlisted and the Messages panel says so.
    fn register_child_sheet_in_project(&mut self, project_id: ProjectId, path: &std::path::Path) {
        let Some(project) = self.document_state.project_by_id_mut(project_id) else {
            crate::diagnostics::log_warning(format!(
                "Child sheet {} was created, but its project was closed first, so it is not \
                 listed in any project",
                path.display()
            ));
            return;
        };
        let fold = crate::app::state::project_case::fold_of(Some(&*project));
        let Some(relative) = project
            .path
            .parent()
            .and_then(|dir| relative_within(dir, path, fold))
        else {
            crate::diagnostics::log_warning(format!(
                "Child sheet {} is outside the project folder, so it is not listed in the project",
                path.display()
            ));
            return;
        };
        let filename = relative.to_string_lossy().replace('\\', "/");
        if !super::super::dock::project_navigation::register_sheet_entry(project, &filename) {
            return;
        }
        let project_path = project.path.clone();
        self.document_state.mark_document_dirty(&project_path);
        self.refresh_panel_ctx();
    }
}
