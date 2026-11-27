//! Workspace layouts (mockup row 166): apply, Save Layout and Manage Layouts. The
//! rules live in `crate::workspace_layouts`; this file turns them into Model
//! changes and `Task`s (the `prefs.json` write, the export and import files,
//! the toast timer). Design: docs/WORKSPACE_LAYOUTS.md.

use std::path::PathBuf;

use iced::Task;
use iced::widget::operation;
use signex_i18n::{ids, t};

use crate::app::handlers::dock::project_navigation::add_io::on_blocking_pool;
use crate::app::{Message, Signex, TileLayout, WindowMsg};
use crate::diagnostics::{log_error_line, log_info, log_warning};
use crate::dock::DockLayoutSnapshot;
use crate::workspace_layouts::{
    BuiltinLayout, Captured, DocumentSplit, ImportRefusal, LayoutId, LayoutsMenu,
    ManageLayoutsDialog, ManageLayoutsMsg, Naming, RowDrag, SaveLayoutDialog, SaveLayoutMsg,
    SaveOutcome, UndoToast, WorkspaceLayoutMsg, naming, problems, store,
};

/// Widget id of Save Layout's name field, focused when it opens.
pub(crate) static SAVE_NAME_INPUT_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("signex.workspace_layouts.save_name"));

/// Widget id of Manage Layouts' inline Rename field.
pub(crate) static RENAME_INPUT_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("signex.workspace_layouts.rename"));

/// The name an export file is offered under.
const EXPORT_FILE_NAME: &str = "workspace-layouts.json";

fn tile_layout(split: DocumentSplit) -> TileLayout {
    match split {
        DocumentSplit::SideBySide => TileLayout::SideBySide,
        DocumentSplit::Stacked => TileLayout::Stacked,
    }
}

/// Local time as a layout's "Saved" line shows it.
fn now_stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

impl Signex {
    pub(crate) fn dispatch_workspace_layout_message(
        &mut self,
        msg: WorkspaceLayoutMsg,
    ) -> Task<Message> {
        match msg {
            WorkspaceLayoutMsg::ApplyRequested(id) => self.apply_workspace_layout(&id),
            WorkspaceLayoutMsg::ResetActiveRequested => self.reset_active_workspace_layout(),
            WorkspaceLayoutMsg::Save(msg) => self.dispatch_save_layout(msg),
            WorkspaceLayoutMsg::Manage(msg) => self.dispatch_manage_layouts(msg),
            WorkspaceLayoutMsg::Saved(Ok(())) => Task::none(),
            WorkspaceLayoutMsg::Saved(Err(reason)) => {
                let line =
                    t!(self.ui_state.i18n, ids::WORKSPACE_LAYOUTS_NOT_SAVED, "why" => reason);
                log_error_line(line.into_string());
                self.sync_diagnostics_panel_ctx();
                Task::none()
            }
        }
    }

    /// The layout names in the app's language.
    pub(crate) fn layout_naming(&self) -> Naming {
        naming(&self.ui_state.i18n)
    }

    /// What View ▸ Window ▸ Layouts lists.
    pub(crate) fn layouts_menu(&self) -> LayoutsMenu {
        LayoutsMenu::of(
            &self.ui_state.workspace_layouts.layouts,
            &self.layout_naming(),
        )
    }

    /// The document area's split now, when the main window is tiled.
    pub(crate) fn current_document_split(&self) -> Option<DocumentSplit> {
        self.tiled_panes().map(|(layout, _)| match layout {
            TileLayout::SideBySide => DocumentSplit::SideBySide,
            TileLayout::Stacked => DocumentSplit::Stacked,
        })
    }

    /// How many windows the arrangement has: the main one and every window
    /// of its own (undocked documents, detached panels and modals).
    pub(crate) fn arrangement_window_count(&self) -> usize {
        1 + self.ui_state.windows.len()
    }

    fn capture_layout(&self, panels: bool, split: bool) -> Captured {
        Captured {
            dock: panels.then(|| DockLayoutSnapshot::capture(&self.document_view.dock)),
            split: split.then(|| self.current_document_split()).flatten(),
            saved_at: now_stamp(),
        }
    }

    /// The `Task` that writes the list to `prefs.json` off the UI thread. The
    /// writes run in the order they were made, so the newest list lands last;
    /// quit waits for them.
    fn persist_workspace_layouts(&self) -> Task<Message> {
        let state = &self.ui_state.workspace_layouts;
        // An unreadable stored list is never replaced (DECISIONS 2026-11-23).
        if state.kept.list_unreadable() {
            return Task::none();
        }
        let value = store::to_json_keeping(&state.layouts, &state.kept);
        let sink = std::sync::Arc::clone(&state.store);
        crate::fonts::background_pref_merge(
            store::PREFS_KEY,
            move || sink.write(&value),
            |result| WorkspaceLayoutMsg::Saved(result).into(),
        )
    }

    /// True when the stored list is unreadable, so this edit is refused;
    /// Messages says why, as the dialogs' banner does.
    fn refuse_layout_list_edit(&mut self) -> bool {
        if !self.ui_state.workspace_layouts.kept.list_unreadable() {
            return false;
        }
        let line = t!(self.ui_state.i18n, ids::WORKSPACE_LAYOUTS_LIST_LOCKED);
        log_warning(line.into_string());
        self.sync_diagnostics_panel_ctx();
        true
    }

    /// Arrange the panels of layout `id` and split the document area as it
    /// says. Open documents are untouched; the dock's own persistence saves
    /// the new dock.
    pub(crate) fn apply_workspace_layout(&mut self, id: &LayoutId) -> Task<Message> {
        let Some(layout) = self.ui_state.workspace_layouts.layouts.get(id) else {
            let line = t!(self.ui_state.i18n, ids::WORKSPACE_LAYOUTS_GONE);
            log_warning(line.into_string());
            return self.finish_update();
        };
        let mut tasks = Vec::new();
        if let Some(dock) = &layout.dock {
            self.arrange_dock(dock);
            tasks.push(self.persist_dock_layout_soon());
        }
        if let Some(split) = layout.split {
            tasks.push(Task::done(Message::Window(WindowMsg::TileRequested(
                tile_layout(split),
            ))));
        }
        let naming = self.layout_naming();
        let layouts = &mut self.ui_state.workspace_layouts.layouts;
        layouts.set_active(Some(id.clone()));
        let name = layouts.name_of(id, &naming).unwrap_or_default();
        let line = t!(self.ui_state.i18n, ids::WORKSPACE_LAYOUTS_APPLIED, "name" => name);
        log_info(line.into_string());
        tasks.push(self.persist_workspace_layouts());
        tasks.push(self.finish_update());
        Task::batch(tasks)
    }

    /// Replace the dock with `snapshot`'s panels. A panel in a window of its
    /// own stays there, and a floating panel the layout does not place keeps
    /// floating.
    fn arrange_dock(&mut self, snapshot: &DockLayoutSnapshot) {
        let detached: Vec<crate::panels::PanelKind> = crate::panels::ALL_PANELS
            .iter()
            .copied()
            .filter(|&kind| {
                self.is_panel_open(kind) && self.document_view.dock.locate(kind).is_none()
            })
            .collect();
        let mut wanted = snapshot.clone();
        for region in [&mut wanted.left, &mut wanted.right, &mut wanted.bottom] {
            region.panels.retain(|kind| !detached.contains(kind));
        }
        let mut dock = crate::fonts::dock_from_snapshot(&wanted);
        let extensions = &self.extensions;
        dock.adopt_extension_panels(|key| extensions.panel(key));
        for floating in std::mem::take(&mut self.document_view.dock.floating) {
            if dock.locate(floating.kind).is_none() {
                dock.floating.push(floating);
            }
        }
        self.document_view.dock = dock;
    }

    /// Reset "<name>": drop the active built-in's override and apply it as
    /// shipped.
    fn reset_active_workspace_layout(&mut self) -> Task<Message> {
        let layouts = &mut self.ui_state.workspace_layouts.layouts;
        let Some(builtin) = layouts.resettable() else {
            let line = t!(self.ui_state.i18n, ids::WORKSPACE_LAYOUTS_NOTHING_TO_RESET);
            log_info(line.into_string());
            return self.finish_update();
        };
        layouts.reset_builtin(builtin);
        self.apply_workspace_layout(&LayoutId::Builtin(builtin))
    }

    fn dispatch_save_layout(&mut self, msg: SaveLayoutMsg) -> Task<Message> {
        let naming = self.layout_naming();
        let state = &mut self.ui_state.workspace_layouts;
        match msg {
            SaveLayoutMsg::Requested => {
                state.save = Some(SaveLayoutDialog {
                    name: state.layouts.first_free_name(&naming),
                    store_panels: true,
                    store_split: true,
                });
                Task::batch([
                    operation::focus(SAVE_NAME_INPUT_ID.clone()),
                    operation::text_input::select_all(SAVE_NAME_INPUT_ID.clone()),
                ])
            }
            SaveLayoutMsg::NameEdited(name) => {
                if let Some(dialog) = state.save.as_mut() {
                    dialog.name = name;
                }
                Task::none()
            }
            SaveLayoutMsg::PanelsToggled => {
                if let Some(dialog) = state.save.as_mut() {
                    dialog.store_panels = !dialog.store_panels;
                }
                Task::none()
            }
            SaveLayoutMsg::SplitToggled => {
                if let Some(dialog) = state.save.as_mut() {
                    dialog.store_split = !dialog.store_split;
                }
                Task::none()
            }
            SaveLayoutMsg::Cancelled => {
                state.save = None;
                Task::none()
            }
            SaveLayoutMsg::Confirmed => self.confirm_save_layout(&naming),
        }
    }

    /// Whether Save Layout's Save would store something: panels, or a split
    /// that exists.
    pub(crate) fn save_layout_stores_something(&self, dialog: &SaveLayoutDialog) -> bool {
        dialog.store_panels || (dialog.store_split && self.current_document_split().is_some())
    }

    fn confirm_save_layout(&mut self, naming: &Naming) -> Task<Message> {
        let Some(dialog) = self.ui_state.workspace_layouts.save.clone() else {
            return Task::none();
        };
        if self.refuse_layout_list_edit() {
            return Task::none();
        }
        if !self.save_layout_stores_something(&dialog) {
            return Task::none();
        }
        let captured = self.capture_layout(dialog.store_panels, dialog.store_split);
        let layouts = &mut self.ui_state.workspace_layouts.layouts;
        // A refused name keeps the dialog open; it already says why.
        let Ok(outcome) = layouts.save(&dialog.name, captured, naming) else {
            return Task::none();
        };
        // One whole message per outcome, so a language words each its way.
        let (id, said) = match outcome {
            SaveOutcome::Added(id) => (id, ids::WORKSPACE_LAYOUTS_LAYOUT_SAVED),
            SaveOutcome::Replaced(id) => (id, ids::WORKSPACE_LAYOUTS_LAYOUT_REPLACED),
        };
        // The arrangement on screen is this layout now.
        layouts.set_active(Some(id.clone()));
        let name = layouts.name_of(&id, naming).unwrap_or_default();
        self.ui_state.workspace_layouts.save = None;
        log_info(t!(self.ui_state.i18n, said, "name" => name).into_string());
        Task::batch([self.persist_workspace_layouts(), self.finish_update()])
    }

    fn dispatch_manage_layouts(&mut self, msg: ManageLayoutsMsg) -> Task<Message> {
        if let ManageLayoutsMsg::Requested = msg {
            let layouts = &self.ui_state.workspace_layouts.layouts;
            let selected = layouts
                .active()
                .cloned()
                .or_else(|| layouts.user().first().map(|l| l.id.clone()))
                .unwrap_or(LayoutId::Builtin(BuiltinLayout::Schematic));
            self.ui_state.workspace_layouts.manage = Some(ManageLayoutsDialog::new(selected));
            return Task::none();
        }
        if self.ui_state.workspace_layouts.manage.is_none() {
            return Task::none();
        }
        if msg.edits_the_list() && self.refuse_layout_list_edit() {
            self.with_manage(|dialog| dialog.rename = None);
            return Task::none();
        }
        match msg {
            ManageLayoutsMsg::Requested => Task::none(),
            ManageLayoutsMsg::Closed => {
                self.ui_state.workspace_layouts.manage = None;
                Task::none()
            }
            ManageLayoutsMsg::RowSelected(id) => {
                self.with_manage(|dialog| {
                    dialog.selected = id;
                    dialog.rename = None;
                });
                Task::none()
            }
            ManageLayoutsMsg::RenameStarted => self.start_layout_rename(),
            ManageLayoutsMsg::RenameEdited(text) => {
                self.with_manage(|dialog| dialog.rename = Some(text));
                Task::none()
            }
            ManageLayoutsMsg::RenameCommitted => self.commit_layout_rename(),
            ManageLayoutsMsg::RenameCancelled => {
                self.with_manage(|dialog| dialog.rename = None);
                Task::none()
            }
            ManageLayoutsMsg::DuplicateRequested => self.duplicate_selected_layout(),
            ManageLayoutsMsg::DeleteRequested => self.delete_selected_layout(),
            ManageLayoutsMsg::UndoDeleteRequested => self.undo_layout_delete(),
            ManageLayoutsMsg::ToastExpired(generation) => {
                self.with_manage(|dialog| {
                    if dialog.toast.as_ref().map(|t| t.generation) == Some(generation) {
                        dialog.toast = None;
                    }
                });
                Task::none()
            }
            ManageLayoutsMsg::UpdateFromCurrentRequested => self.update_selected_from_current(),
            ManageLayoutsMsg::ApplyRequested => {
                let selected = self.selected_layout();
                selected.map_or_else(Task::none, |id| self.apply_workspace_layout(&id))
            }
            ManageLayoutsMsg::GripPressed(index) => {
                self.with_manage(|dialog| {
                    dialog.drag = Some(RowDrag {
                        from: index,
                        over: index,
                    });
                });
                Task::none()
            }
            ManageLayoutsMsg::RowEntered(index) => {
                self.with_manage(|dialog| {
                    if let Some(drag) = dialog.drag.as_mut() {
                        drag.over = index;
                    }
                });
                Task::none()
            }
            ManageLayoutsMsg::GripReleased => self.drop_dragged_layout(),
            ManageLayoutsMsg::ImportRequested => self.pick_layouts_import(),
            ManageLayoutsMsg::ImportPicked(path) => path.map_or_else(Task::none, read_import),
            ManageLayoutsMsg::ImportRead { path, result } => self.layouts_imported(&path, result),
            ManageLayoutsMsg::ExportRequested => self.pick_layouts_export(),
            ManageLayoutsMsg::ExportPicked(path) => {
                path.map_or_else(Task::none, |path| self.write_layouts_export(path))
            }
            ManageLayoutsMsg::ExportWritten(Ok(path)) => {
                let line = t!(
                    self.ui_state.i18n,
                    ids::WORKSPACE_LAYOUTS_EXPORTED,
                    "path" => path.display().to_string(),
                );
                log_info(line.into_string());
                self.finish_update()
            }
            ManageLayoutsMsg::ExportWritten(Err(why)) => {
                let line =
                    t!(self.ui_state.i18n, ids::WORKSPACE_LAYOUTS_NOT_EXPORTED, "why" => why);
                log_error_line(line.into_string());
                self.finish_update()
            }
        }
    }

    fn with_manage(&mut self, change: impl FnOnce(&mut ManageLayoutsDialog)) {
        if let Some(dialog) = self.ui_state.workspace_layouts.manage.as_mut() {
            change(dialog);
        }
    }

    fn selected_layout(&self) -> Option<LayoutId> {
        self.ui_state
            .workspace_layouts
            .manage
            .as_ref()
            .map(|dialog| dialog.selected.clone())
    }

    fn start_layout_rename(&mut self) -> Task<Message> {
        let Some(id) = self.selected_layout().filter(|id| id.builtin().is_none()) else {
            return Task::none();
        };
        let naming = self.layout_naming();
        let name = self
            .ui_state
            .workspace_layouts
            .layouts
            .name_of(&id, &naming)
            .unwrap_or_default();
        self.with_manage(|dialog| dialog.rename = Some(name));
        Task::batch([
            operation::focus(RENAME_INPUT_ID.clone()),
            operation::text_input::select_all(RENAME_INPUT_ID.clone()),
        ])
    }

    fn commit_layout_rename(&mut self) -> Task<Message> {
        let (Some(id), Some(text)) = (
            self.selected_layout(),
            self.ui_state
                .workspace_layouts
                .manage
                .as_ref()
                .and_then(|dialog| dialog.rename.clone()),
        ) else {
            return Task::none();
        };
        let naming = self.layout_naming();
        // A refused name keeps the field open; the dialog says why.
        if self
            .ui_state
            .workspace_layouts
            .layouts
            .rename(&id, &text, &naming)
            .is_err()
        {
            return Task::none();
        }
        self.with_manage(|dialog| dialog.rename = None);
        self.persist_workspace_layouts()
    }

    fn duplicate_selected_layout(&mut self) -> Task<Message> {
        let Some(id) = self.selected_layout() else {
            return Task::none();
        };
        let naming = self.layout_naming();
        let Some(copy) = self
            .ui_state
            .workspace_layouts
            .layouts
            .duplicate(&id, &naming)
        else {
            return Task::none();
        };
        self.with_manage(|dialog| {
            dialog.selected = copy;
            dialog.rename = None;
        });
        self.persist_workspace_layouts()
    }

    /// Delete removes the layout at once and shows the toast with Undo: the
    /// layouts are preferences, so no confirmation is asked.
    fn delete_selected_layout(&mut self) -> Task<Message> {
        let Some(id) = self.selected_layout().filter(|id| id.builtin().is_none()) else {
            return Task::none();
        };
        let state = &mut self.ui_state.workspace_layouts;
        let Some(deleted) = state.layouts.delete(&id) else {
            return Task::none();
        };
        state.toast_generation += 1;
        let generation = state.toast_generation;
        let next = state
            .layouts
            .user()
            .first()
            .map_or(LayoutId::Builtin(BuiltinLayout::Schematic), |l| {
                l.id.clone()
            });
        if let Some(dialog) = state.manage.as_mut() {
            dialog.selected = next;
            dialog.rename = None;
            dialog.toast = Some(UndoToast {
                deleted,
                generation,
            });
        }
        let expire = Task::perform(
            async {
                tokio::time::sleep(crate::workspace_layouts::UNDO_TOAST_DURATION).await;
            },
            move |()| ManageLayoutsMsg::ToastExpired(generation).into(),
        );
        Task::batch([self.persist_workspace_layouts(), expire])
    }

    fn undo_layout_delete(&mut self) -> Task<Message> {
        let naming = self.layout_naming();
        let state = &mut self.ui_state.workspace_layouts;
        let Some(toast) = state.manage.as_mut().and_then(|dialog| dialog.toast.take()) else {
            return Task::none();
        };
        let id = state.layouts.undo_delete(toast.deleted, &naming);
        self.with_manage(|dialog| dialog.selected = id);
        self.persist_workspace_layouts()
    }

    fn update_selected_from_current(&mut self) -> Task<Message> {
        let Some(id) = self.selected_layout() else {
            return Task::none();
        };
        // A stored edit this build cannot read is never replaced.
        if let Some(builtin) = id.builtin()
            && self.ui_state.workspace_layouts.kept.holds_override(builtin)
        {
            let name = self.layout_naming().builtin(builtin).to_string();
            let line = t!(self.ui_state.i18n, ids::WORKSPACE_LAYOUTS_EDIT_KEPT, "name" => name);
            log_warning(line.into_string());
            self.sync_diagnostics_panel_ctx();
            return Task::none();
        }
        let captured = self.capture_layout(true, true);
        if !self
            .ui_state
            .workspace_layouts
            .layouts
            .update_from_current(&id, captured)
        {
            return Task::none();
        }
        self.persist_workspace_layouts()
    }

    fn drop_dragged_layout(&mut self) -> Task<Message> {
        let Some(drag) = self
            .ui_state
            .workspace_layouts
            .manage
            .as_mut()
            .and_then(|dialog| dialog.drag.take())
        else {
            return Task::none();
        };
        if self.refuse_layout_list_edit() {
            return Task::none();
        }
        if !self
            .ui_state
            .workspace_layouts
            .layouts
            .reorder(drag.from, drag.over)
        {
            return Task::none();
        }
        self.persist_workspace_layouts()
    }

    fn pick_layouts_import(&self) -> Task<Message> {
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::WORKSPACE_LAYOUTS_IMPORT_TITLE)
            .filter_in(i18n, ids::WORKSPACE_LAYOUTS_FILE_FILTER, &["json"]);
        Task::perform(dialog.pick_file(), |picked| {
            ManageLayoutsMsg::ImportPicked(picked).into()
        })
    }

    fn layouts_imported(
        &mut self,
        path: &std::path::Path,
        result: Result<store::ImportRead, ImportRefusal>,
    ) -> Task<Message> {
        let (incoming, skipped) = match result {
            Ok(read) => read,
            Err(why) => {
                let line = t!(
                    self.ui_state.i18n,
                    ids::WORKSPACE_LAYOUTS_NOT_IMPORTED,
                    "path" => path.display().to_string(),
                    "why" => problems::refusal(&why),
                );
                log_error_line(line.into_string());
                return self.finish_update();
            }
        };
        for problem in &skipped {
            log_warning(problems::problem_line(&self.ui_state.i18n, problem));
        }
        let naming = self.layout_naming();
        let added = self
            .ui_state
            .workspace_layouts
            .layouts
            .import(incoming, &naming);
        let line = t!(
            self.ui_state.i18n,
            ids::WORKSPACE_LAYOUTS_IMPORTED,
            "count" => added.len(),
            "path" => path.display().to_string(),
        );
        log_info(line.into_string());
        if let Some(first) = added.first().cloned() {
            self.with_manage(|dialog| dialog.selected = first);
        }
        Task::batch([self.persist_workspace_layouts(), self.finish_update()])
    }

    fn pick_layouts_export(&self) -> Task<Message> {
        if self.ui_state.workspace_layouts.layouts.user().is_empty() {
            return Task::none();
        }
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::WORKSPACE_LAYOUTS_EXPORT_TITLE)
            .filter_in(i18n, ids::WORKSPACE_LAYOUTS_FILE_FILTER, &["json"])
            .set_file_name(EXPORT_FILE_NAME);
        Task::perform(dialog.save_file(), |picked| {
            ManageLayoutsMsg::ExportPicked(picked).into()
        })
    }

    fn write_layouts_export(&self, path: PathBuf) -> Task<Message> {
        let value = store::export_json(self.ui_state.workspace_layouts.layouts.user());
        Task::perform(
            on_blocking_pool(move || {
                // The system's or serialiser's reason; the Messages line
                // around it comes from the catalog.
                let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
                signex_types::atomic_io::atomic_write(&path, text.as_bytes())
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                Ok(path)
            }),
            |result| ManageLayoutsMsg::ExportWritten(result).into(),
        )
    }
}

/// Read and parse an export file off the UI thread.
fn read_import(path: PathBuf) -> Task<Message> {
    let read_path = path.clone();
    Task::perform(
        on_blocking_pool(move || Ok(parse_import(&read_path))),
        move |read: Result<_, String>| {
            // A stopped worker reads as a failed read.
            let result = read.unwrap_or_else(|why| Err(ImportRefusal::ReadFailed(why)));
            ManageLayoutsMsg::ImportRead { path, result }.into()
        },
    )
}

/// Blocking: the layouts of the export file at `path`.
fn parse_import(path: &std::path::Path) -> Result<store::ImportRead, ImportRefusal> {
    let bytes = std::fs::read(path).map_err(|e| ImportRefusal::ReadFailed(e.to_string()))?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| ImportRefusal::NotJson(e.to_string()))?;
    store::import_json(&value)
}
