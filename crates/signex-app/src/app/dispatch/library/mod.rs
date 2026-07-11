//! Library subsystem dispatcher. Routes
//! [`crate::library::LibraryMessage`] to the right side-effecting
//! handler.
//!
//! In the DBLib model the Component view is preview-only.
//! Symbol/Footprint/Sim render read-only here; the standalone
//! `.snxsym` / `.snxfpt` / `.snxsim` document tabs own actual
//! editing. The dispatcher's editor handlers are scoped to the five
//! Component Preview tabs (Preview / Parameters / Supply / Datasheet
//! / Simulation).
//!
//! This module is a thin router: every arm with real logic delegates
//! to a `handle_*` method (or free function) living in the matching
//! concern module below. Only the truly trivial arms (`Task::none()`,
//! a single state assignment) stay inline.

use iced::Task;

use super::super::*;
use crate::library::commands;
use crate::library::component_preview::apply_inline_edit;
use crate::library::editor::footprint::updates::{
    apply_footprint_clipboard_op, apply_footprint_primitive_edit,
};
use crate::library::messages::{
    BrowserEditMsg, CloseLibraryChoice, EditorMsg, FootprintEditorMsg, LibraryMessage, PickerMsg,
    PrimitiveEdit, PrimitivePickerMsg, SettingsMsg, SymbolEditorMsg,
};
use crate::library::state::{
    CloseLibraryConfirmState, ComponentPreviewState, DeleteConfirmState, DocumentOptionsModalState,
    EditRowModalState, EditorAddress, LibraryCreateOptionsState, NewComponentState, PickerState,
    PreviewTab, PrimitivePickerState, PrimitivePickerTarget,
};
use signex_library::{PrimitiveKind, PrimitiveRef, RowId};

mod browser;
mod browser_classes;
mod browser_grid;
mod browser_tables;
mod component_preview;
mod components_panel;
mod document_options;
mod editor;
mod lifecycle;
mod new_component;
mod primitive_picker;
mod recovery;
mod registration;
mod settings;
mod updates;

use recovery::{
    atomic_write, handle_recovery_broken_binding, handle_recovery_git_missing,
    handle_recovery_library_missing, handle_recovery_library_missing_locate_result,
    route_open_error,
};

impl Signex {
    pub(crate) fn dispatch_library_message(&mut self, msg: LibraryMessage) -> Task<Message> {
        match msg {
            LibraryMessage::OpenLibraryDialog => self.handle_open_library_dialog(),
            LibraryMessage::OpenLibraryAt(None) => Task::none(),
            LibraryMessage::OpenLibraryAt(Some(path)) => self.handle_open_library_at(path),
            LibraryMessage::CloseLibrary(path) => self.handle_close_library(path),
            LibraryMessage::OpenPicker => {
                self.library.picker = Some(PickerState::default());
                Task::none()
            }
            LibraryMessage::ClosePicker => {
                self.library.picker = None;
                Task::none()
            }

            // ── New Component flow ───────────────────────────────────
            // v0.13 — No modal. Append a draft row directly to the
            // active library's first table, then OpenComponentRow so
            // the Library Browser tab focuses the new row. The user
            // fills in the PN inline in the table and picks the
            // symbol / footprint via the Properties panel.
            //
            // When no library is open OR no table exists, fall back
            // to the legacy modal path so the user gets a clear
            // picker UI (the modal handles "no library" + table
            // selection edge cases).
            LibraryMessage::NewComponent => self.handle_new_component(),
            LibraryMessage::CloseNewComponent => {
                self.library.new_component = None;
                Task::none()
            }
            LibraryMessage::NewComponentSetInternalPn(s) => {
                self.handle_new_component_set_internal_pn(s)
            }
            LibraryMessage::NewComponentSetLibrary(idx) => {
                self.handle_new_component_set_library(idx)
            }
            LibraryMessage::NewComponentSetClass(class) => {
                self.handle_new_component_set_class(class)
            }
            LibraryMessage::NewComponentSetTable(name) => self.handle_new_component_set_table(name),
            LibraryMessage::NewComponentSetCategory(s) => self.handle_new_component_set_category(s),
            LibraryMessage::BrowserBeginAddTable { library_path } => {
                self.handle_browser_begin_add_table(library_path)
            }
            LibraryMessage::BrowserSetNewTableName {
                library_path,
                value,
            } => self.handle_browser_set_new_table_name(library_path, value),
            LibraryMessage::BrowserCancelAddTable { library_path } => {
                self.handle_browser_cancel_add_table(library_path)
            }
            LibraryMessage::BrowserDeleteTable {
                library_path,
                table,
            } => self.handle_browser_delete_table(library_path, table),
            LibraryMessage::BrowserDismissDeleteError { library_path } => {
                self.handle_browser_dismiss_delete_error(library_path)
            }
            LibraryMessage::BrowserBeginRenameTable {
                library_path,
                table,
            } => self.handle_browser_begin_rename_table(library_path, table),
            LibraryMessage::BrowserSetRenameName {
                library_path,
                value,
            } => self.handle_browser_set_rename_name(library_path, value),
            LibraryMessage::BrowserCancelRenameTable { library_path } => {
                self.handle_browser_cancel_rename_table(library_path)
            }
            LibraryMessage::BrowserBeginAddClass { library_path } => {
                self.handle_browser_begin_add_class(library_path)
            }
            LibraryMessage::BrowserSetNewClassKey {
                library_path,
                value,
            } => self.handle_browser_set_new_class_key(library_path, value),
            LibraryMessage::BrowserSetNewClassLabel {
                library_path,
                value,
            } => self.handle_browser_set_new_class_label(library_path, value),
            LibraryMessage::BrowserCancelAddClass { library_path } => {
                self.handle_browser_cancel_add_class(library_path)
            }
            LibraryMessage::BrowserConfirmAddClass { library_path } => {
                self.handle_browser_confirm_add_class(library_path)
            }
            LibraryMessage::BrowserDeleteClass { library_path, key } => {
                self.handle_browser_delete_class(library_path, key)
            }
            LibraryMessage::BrowserBeginRenameClass { library_path, key } => {
                self.handle_browser_begin_rename_class(library_path, key)
            }
            LibraryMessage::BrowserSetRenameClassKey {
                library_path,
                value,
            } => self.handle_browser_set_rename_class_key(library_path, value),
            LibraryMessage::BrowserSetRenameClassLabel {
                library_path,
                value,
            } => self.handle_browser_set_rename_class_label(library_path, value),
            LibraryMessage::BrowserCancelRenameClass { library_path } => {
                self.handle_browser_cancel_rename_class(library_path)
            }
            LibraryMessage::BrowserConfirmRenameClass { library_path } => {
                self.handle_browser_confirm_rename_class(library_path)
            }
            LibraryMessage::BrowserConfirmRenameTable { library_path } => {
                self.handle_browser_confirm_rename_table(library_path)
            }
            LibraryMessage::BrowserConfirmAddTable { library_path } => {
                self.handle_browser_confirm_add_table(library_path)
            }
            LibraryMessage::NewComponentToggleAdvanced => {
                self.handle_new_component_toggle_advanced()
            }
            LibraryMessage::NewComponentBeginCreateTable => {
                self.handle_new_component_begin_create_table()
            }
            LibraryMessage::NewComponentSetNewTableName(name) => {
                self.handle_new_component_set_new_table_name(name)
            }
            LibraryMessage::NewComponentCancelCreateTable => {
                self.handle_new_component_cancel_create_table()
            }
            LibraryMessage::NewComponentConfirmCreateTable => {
                self.handle_new_component_confirm_create_table()
            }
            LibraryMessage::NewComponentSubmit => self.handle_new_component_submit(),
            LibraryMessage::NewComponentSubmit => {
                // WS-8 will replace `commands::create_component` with the
                // row-based `create_component_row`. Until that lands, the
                // dispatcher logs the request and bails.
                tracing::warn!(
                    target: "signex::library",
                    "NewComponentSubmit: row-based create flow ships in WS-8"
                );
                self.library.new_component = None;
                Task::none()
            }

            LibraryMessage::ToggleLibraryTreeNode(idx) => self.handle_toggle_library_tree_node(idx),
            LibraryMessage::OpenComponentRow {
                library_path,
                table,
                row_id,
            } => self.handle_open_component_row(library_path, table, row_id),
            LibraryMessage::OpenPrimitiveEditor { path } => self.handle_open_primitive_editor(path),
            LibraryMessage::EditorEvent {
                library_path,
                table,
                row_id,
                msg,
            } => self.handle_editor_event(EditorAddress::new(library_path, table, row_id), msg),
            LibraryMessage::Picker(msg) => self.handle_picker_message(msg),
            LibraryMessage::Settings(msg) => self.handle_library_settings_message(msg),
            LibraryMessage::JumpToUseSite(site) => {
                commands::jump_to_use_site(&site);
                Task::none()
            }
            LibraryMessage::Noop => Task::none(),

            LibraryMessage::ConfirmCloseLibrary {
                library_path,
                dirty_editors,
            } => self.handle_confirm_close_library(library_path, dirty_editors),
            LibraryMessage::CloseLibraryConfirm(choice) => {
                self.handle_close_library_confirm(choice)
            }
            LibraryMessage::PlaceLibraryComponent {
                library_path,
                table,
                row_id,
            } => self.handle_place_library_component(library_path, table, row_id),
            LibraryMessage::CreateLibraryAt(project_root) => {
                self.handle_create_library_for_project(project_root)
            }
            LibraryMessage::CreateLibraryAtPath {
                project_path,
                lib_path,
            } => self.handle_prompt_library_create_options(project_path, lib_path),
            LibraryMessage::LibraryCreateOptionsToggleLfs => {
                self.handle_library_create_options_toggle_lfs()
            }
            LibraryMessage::LibraryCreateOptionsToggleGit => {
                self.handle_library_create_options_toggle_git()
            }
            LibraryMessage::LibraryCreateOptionsCancel => {
                self.library.create_options = None;
                Task::none()
            }
            LibraryMessage::LibraryCreateOptionsConfirm => {
                self.handle_library_create_options_confirm()
            }
            LibraryMessage::AddLibrarySymbolFilePicked(path) => {
                self.handle_add_library_symbol_file_picked(path)
            }
            LibraryMessage::AddLibraryFootprintFilePicked(path) => {
                self.handle_add_library_footprint_file_picked(path)
            }
            LibraryMessage::ComponentPreviewOpened {
                path,
                table,
                row_id,
            } => self.handle_component_preview_opened(path, table, row_id),
            LibraryMessage::PrimitiveEditorEvent { path, msg } => {
                self.handle_primitive_editor_event(path, msg)
            }
            // ── Library Browser tab ──────────────────────────────────
            LibraryMessage::OpenLibraryBrowser(path) => self.handle_open_library_browser(path),
            LibraryMessage::BrowserSelectTable {
                library_path,
                table,
            } => self.handle_browser_select_table(library_path, table),
            LibraryMessage::BrowserSearchChanged {
                library_path,
                value,
            } => self.handle_browser_search_changed(library_path, value),
            LibraryMessage::BrowserSortColumn {
                library_path,
                column_key,
            } => self.handle_browser_sort_column(library_path, column_key),
            LibraryMessage::BrowserSelectRow {
                library_path,
                table,
                row_id,
            } => self.handle_browser_select_row(library_path, table, row_id),
            LibraryMessage::BrowserAddComponent {
                library_path,
                table,
            } => self.handle_browser_add_component(library_path, table),
            LibraryMessage::BrowserDeleteRowRequest {
                library_path,
                table,
                row_id,
            } => self.handle_browser_delete_row_request(library_path, table, row_id),
            LibraryMessage::BrowserDeleteRowConfirm {
                library_path,
                table,
                row_id,
            } => self.handle_browser_delete_row_confirm(library_path, table, row_id),
            LibraryMessage::BrowserDeleteRowCancel { library_path } => {
                self.handle_browser_delete_row_cancel(library_path)
            }
            LibraryMessage::OpenPrimitivePicker { kind, target } => {
                self.handle_open_primitive_picker(kind, target)
            }
            LibraryMessage::PrimitivePicker(msg) => self.handle_primitive_picker_msg(msg),
            LibraryMessage::BrowserOpenEditModal {
                library_path,
                table,
                row_id,
            } => self.handle_browser_open_edit_modal(library_path, table, row_id),
            LibraryMessage::BrowserEdit { library_path, msg } => {
                self.handle_browser_edit_msg(library_path, msg)
            }
            LibraryMessage::BrowserCellEdit {
                library_path,
                row_id,
                column,
                value,
            } => self.handle_browser_cell_edit(library_path, row_id, column, value),
            LibraryMessage::BrowserCellCommit {
                library_path,
                table,
                row_id,
                column,
            } => self.handle_browser_cell_commit(library_path, table, row_id, column),
            LibraryMessage::BrowserCellCancel {
                library_path,
                row_id,
                column,
            } => self.handle_browser_cell_cancel(library_path, row_id, column),
            LibraryMessage::BrowserSetLifecycleFilter {
                library_path,
                filter,
            } => self.handle_browser_set_lifecycle_filter(library_path, filter),
            LibraryMessage::BrowserClassFilterClicked { library_path, key } => {
                self.handle_browser_class_filter_clicked(library_path, key)
            }
            LibraryMessage::BrowserRefreshPricing {
                library_path,
                table,
                row_id,
            } => self.handle_browser_refresh_pricing(library_path, table, row_id),
            LibraryMessage::LibraryRefreshAllPricing(library_path) => {
                self.handle_library_refresh_all_pricing(library_path)
            }
            // ── Document Options modal (Tools ▸ Document Options) ──
            LibraryMessage::OpenDocumentOptions { library_path } => {
                self.handle_open_document_options(library_path)
            }
            LibraryMessage::DocumentOptionsSetSheetColor(c) => {
                self.handle_document_options_set_sheet_color(c)
            }
            LibraryMessage::DocumentOptionsToggleGrid => self.handle_document_options_toggle_grid(),
            LibraryMessage::DocumentOptionsCycleGridSize => {
                self.handle_document_options_cycle_grid_size()
            }
            LibraryMessage::DocumentOptionsCycleUnit => self.handle_document_options_cycle_unit(),
            LibraryMessage::DocumentOptionsApply => self.handle_document_options_apply(),
            LibraryMessage::DocumentOptionsCancel => {
                self.library.document_options = None;
                Task::none()
            }

            // Recovery dialogs (Stage 10).
            LibraryMessage::RecoveryLibraryMissing(choice) => {
                handle_recovery_library_missing(self, choice)
            }
            LibraryMessage::RecoveryLibraryMissingLocateResult(picked) => {
                handle_recovery_library_missing_locate_result(self, picked)
            }
            LibraryMessage::RecoveryGitMissing(choice) => handle_recovery_git_missing(self, choice),
            LibraryMessage::RecoveryBrokenBinding(choice) => {
                handle_recovery_broken_binding(self, choice)
            }

            // ── Library Updates Available modal (Stage 16) ─────────
            LibraryMessage::LibraryUpdatesToggleSelection(symbol_uuid) => {
                self.handle_library_updates_toggle_selection(symbol_uuid)
            }
            LibraryMessage::LibraryUpdatesApply => {
                self.handle_library_updates_apply();
                Task::none()
            }
            LibraryMessage::LibraryUpdatesSkipAll => self.handle_library_updates_skip_all(),
            LibraryMessage::LibraryUpdatesCancel => {
                self.library.library_updates = None;
                Task::none()
            }

            // ── Components Panel (Stage 9) ────────────────────────────
            LibraryMessage::ComponentsPanelToggleSection(src) => {
                self.handle_components_panel_toggle_section(src)
            }
            LibraryMessage::ComponentsPanelSetFilter(value) => {
                self.library.components_panel.filter = value;
                Task::none()
            }
            LibraryMessage::ComponentsPanelAddLibrary(source) => {
                self.handle_components_panel_add_library(source)
            }
            LibraryMessage::ComponentsPanelAddLibraryAt { source, path } => {
                self.handle_components_panel_add_library_at(source, path)
            }
            LibraryMessage::ComponentsPanelPromoteToGlobal(path) => {
                self.handle_components_panel_promote_to_global(path)
            }
            LibraryMessage::ComponentsPanelManageGlobal => {
                self.handle_components_panel_manage_global()
            }
            LibraryMessage::ComponentsPanelAddToProject { library_path } => {
                self.handle_components_panel_add_to_project(library_path)
            }
            LibraryMessage::ComponentsPanelPlace {
                library_path,
                table,
                row_id,
            } => self.handle_components_panel_place(library_path, table, row_id),
        }
    }
}
