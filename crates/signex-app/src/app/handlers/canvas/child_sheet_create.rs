//! Creating the child sheet a sheet symbol points at (#1275).
//!
//! The file is written beside the PARENT sheet, where descending resolves it
//! (`resolve_child_sheet_path`), with one hierarchical label per entry on the
//! symbol. The write runs off the update thread and never overwrites. It is
//! not an edit: like Save it is not undone, and the Messages panel records it.
//! Design: docs/HIERARCHY.md, "Creating the child sheet".

use iced::Task;
use uuid::Uuid;

use super::super::super::*;
use super::super::dock::project_navigation::add_io;
use crate::app::contracts::CreateChildSheetConfirmState;
use crate::app::state::ProjectId;
use add_io::ChildSheetWriteError;

/// What creating a child sheet writes, decided in `update`, written on a worker.
pub(crate) struct ChildSheetPlan {
    pub parent: std::path::PathBuf,
    pub path: std::path::PathBuf,
    pub document: signex_types::schematic::SchematicSheet,
    pub project_id: Option<ProjectId>,
}

impl Signex {
    /// A sheet symbol was descended into but its file is missing: say why on
    /// screen and in the Messages panel, and offer to create it.
    pub(super) fn offer_child_sheet_creation(&mut self, symbol: Uuid, path: std::path::PathBuf) {
        let Some(name) = self.sheet_symbol_name(symbol) else {
            return;
        };
        let Some(parent) = self.saved_parent_sheet() else {
            self.refuse_child_sheet_without_parent(&name);
            return;
        };
        crate::diagnostics::log_info(format!(
            "Sheet '{name}' has no file yet ({}); asking whether to create it",
            path.display()
        ));
        // The answer plans against THIS parent, not whichever tab is active
        // by then.
        self.ui_state.create_child_sheet_confirm = Some(CreateChildSheetConfirmState {
            symbol,
            name,
            path,
            parent,
        });
    }

    fn sheet_symbol_name(&self, symbol: Uuid) -> Option<String> {
        let engine = self.document_state.active_engine()?;
        let found = engine
            .document()
            .child_sheets
            .iter()
            .find(|c| c.uuid == symbol);
        found.map(|c| c.name.clone())
    }

    /// The focused sheet's path when it is a file on disk: the folder the
    /// child is created in.
    fn saved_parent_sheet(&self) -> Option<std::path::PathBuf> {
        self.active_tab_path().filter(|path| path.is_file())
    }

    /// The file goes beside the parent sheet, so an unsaved parent has no
    /// place to put it.
    fn refuse_child_sheet_without_parent(&mut self, name: &str) {
        let detail = format!(
            "Sheet '{name}' has no file yet, and its parent sheet is not saved, so there is no \
             folder to create it in. Save the parent sheet first."
        );
        self.refuse_child_sheet(detail);
    }

    /// Say why nothing was created, on screen and in the Messages panel.
    fn refuse_child_sheet(&mut self, detail: String) {
        crate::diagnostics::log_warning(&detail);
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::create(detail));
    }

    pub(crate) fn handle_create_child_sheet_answered(&mut self, create: bool) -> Task<Message> {
        let Some(confirm) = self.ui_state.create_child_sheet_confirm.take() else {
            return Task::none();
        };
        if !create {
            crate::diagnostics::log_info(format!(
                "Sheet '{}' was left without a file",
                confirm.name
            ));
            return Task::none();
        }
        self.create_child_sheet_in(confirm.parent, confirm.symbol, Some(confirm.path))
    }

    /// The context-menu route: write the child of `symbol_id` beside the
    /// focused sheet and open it. An existing file is never touched.
    pub(crate) fn create_child_sheet_from_symbol(&mut self, symbol_id: Uuid) -> Task<Message> {
        let Some(parent) = self.saved_parent_sheet() else {
            let name = self.sheet_symbol_name(symbol_id).unwrap_or_default();
            self.refuse_child_sheet_without_parent(&name);
            return Task::none();
        };
        self.create_child_sheet_in(parent, symbol_id, None)
    }

    /// Write the child of `symbol_id` of the sheet `parent`, off the update
    /// thread. `expected` is the path the user was shown; a different one
    /// is refused rather than written.
    fn create_child_sheet_in(
        &mut self,
        parent: std::path::PathBuf,
        symbol_id: Uuid,
        expected: Option<std::path::PathBuf>,
    ) -> Task<Message> {
        let Some(plan) = self.plan_child_sheet_creation(&parent, symbol_id, expected.as_deref())
        else {
            return Task::none();
        };
        self.begin_project_write(&plan.path);
        let ChildSheetPlan {
            parent,
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
                    symbol: symbol_id,
                    path: path.clone(),
                    result,
                })
            },
        )
    }

    /// Decide what creating the child of `symbol_id` of `parent` writes and
    /// where, reading the Model only. `None` (always with a log line) when
    /// there is nothing to write: the symbol is gone, the parent is unsaved,
    /// the name is unusable, the path is not the one shown, the project is
    /// being moved, or the file already exists (then it is just opened).
    pub(crate) fn plan_child_sheet_creation(
        &mut self,
        parent: &std::path::Path,
        symbol_id: Uuid,
        expected: Option<&std::path::Path>,
    ) -> Option<ChildSheetPlan> {
        let symbol = self
            .document_state
            .engines
            .get(parent)
            .and_then(|e| {
                e.document()
                    .child_sheets
                    .iter()
                    .find(|c| c.uuid == symbol_id)
            })
            .cloned();
        let Some(symbol) = symbol else {
            crate::diagnostics::log_warning(format!(
                "The sheet symbol is no longer in {}; no child sheet was created",
                parent.display()
            ));
            return None;
        };
        if !parent.is_file() {
            self.refuse_child_sheet_without_parent(&symbol.name);
            return None;
        }
        let Some(path) = crate::app::project_sheets::resolve_child_reference(
            parent.parent().unwrap_or_else(|| std::path::Path::new("")),
            &symbol.filename,
        ) else {
            crate::diagnostics::log_warning(format!(
                "Sheet '{}' has no usable file name ('{}'); nothing was created",
                symbol.name, symbol.filename
            ));
            return None;
        };
        if expected.is_some_and(|shown| shown != path) {
            self.refuse_child_sheet(format!(
                "Sheet '{}' now points at {}, not the file you were asked about; nothing was \
                 created",
                symbol.name,
                path.display()
            ));
            return None;
        }
        if path.exists() {
            self.open_existing_child_sheet(parent, symbol_id, &symbol.filename, path);
            return None;
        }
        if self.refuse_with_notice_while_project_moves(
            &path,
            "create",
            crate::app::state::ErrorNotice::create,
        ) {
            return None;
        }
        let project_id = self.document_state.document_project(parent).map(|p| p.id);
        let document = signex_engine::child_document_for_symbol(
            crate::app::handlers::document_files::blank_schematic_sheet_for_new_doc(),
            &symbol,
        );
        Some(ChildSheetPlan {
            parent: parent.to_path_buf(),
            path,
            document,
            project_id,
        })
    }

    /// Open `path`, the child of `symbol` of `parent`: through the symbol when
    /// `parent` is the focused sheet (recording the placement), else plainly.
    fn open_existing_child_sheet(
        &mut self,
        parent: &std::path::Path,
        symbol: Uuid,
        filename: &str,
        path: std::path::PathBuf,
    ) {
        if self.active_tab_path().as_deref() == Some(parent) {
            self.descend_through_sheet_symbol(symbol, filename);
        } else {
            self.open_sheet_with_placement(path, None);
        }
    }

    /// The write landed (or failed). Register the file with the project, then
    /// open it the way a descend does.
    pub(crate) fn handle_child_sheet_created(
        &mut self,
        project_id: Option<ProjectId>,
        parent: &std::path::Path,
        symbol: Uuid,
        path: std::path::PathBuf,
        result: Result<(), ChildSheetWriteError>,
    ) -> Task<Message> {
        self.end_project_write(&path);
        match result {
            Ok(()) => {
                crate::diagnostics::log_info(format!("Created child sheet {}", path.display()))
            }
            Err(ChildSheetWriteError::AlreadyExists) => crate::diagnostics::log_info(format!(
                "Child sheet {} already exists; opened it instead of overwriting",
                path.display()
            )),
            Err(ChildSheetWriteError::Failed(e)) => {
                crate::diagnostics::log_error("Create child sheet", &anyhow::anyhow!("{e}"));
                self.document_state.error_notice = Some(crate::app::state::ErrorNotice::create(e));
                return Task::none();
            }
        }
        if let Some(id) = project_id {
            self.register_child_sheet_in_project(id, &path);
        }
        let filename = self
            .document_state
            .engines
            .get(parent)
            .and_then(|e| e.document().child_sheets.iter().find(|c| c.uuid == symbol))
            .map(|c| c.filename.clone());
        match filename {
            Some(filename) => self.open_existing_child_sheet(parent, symbol, &filename, path),
            None => self.open_sheet_with_placement(path, None),
        }
        Task::none()
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
        let Some(relative) = project
            .path
            .parent()
            .and_then(|dir| path.strip_prefix(dir).ok())
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
        self.document_state.dirty_paths.insert(project_path);
        self.refresh_panel_ctx();
    }
}
