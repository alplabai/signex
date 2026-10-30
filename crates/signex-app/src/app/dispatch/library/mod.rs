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
use crate::library::editor::footprint::sketch_solve_task::dispatch_pending;
use crate::library::editor::footprint::updates::{
    apply_footprint_clipboard_op, apply_footprint_primitive_edit,
};
use crate::library::messages::{
    BrowserEditMsg, CloseLibraryChoice, EditorMsg, FootprintEditorMsg, LibraryMessage, PickerMsg,
    PrimitiveEdit, PrimitivePickerMsg, SettingsMsg, SymbolEditorMsg,
};
use crate::library::state::{
    CloseLibraryConfirmState, ComponentPreviewState, DeleteConfirmState, DocumentOptionsModalState,
    EditRowModalState, EditorAddress, LibraryCreateOptionsState, PickerState, PreviewTab,
    PrimitivePickerState, PrimitivePickerTarget,
};
use signex_library::{PrimitiveKind, PrimitiveRef, RowId};

mod alternate_symbols;
mod browser;
mod commit_pending;
mod component_preview;
mod components_panel;
mod datasheet_save;
mod document_options;
#[cfg(test)]
mod edit_modal_tests;
mod editor;
mod external_edits;
#[cfg(test)]
mod grid_window_tests;
mod lifecycle;
mod new_component;
mod new_row;
mod pricing;
#[cfg(test)]
mod pricing_tests;
mod primitive_picker;
mod read_only;
mod recovery;
#[cfg(test)]
mod recovery_read_only_tests;
mod refresh;
mod registration;
mod rename_guard;
pub(crate) use rename_guard::LIBRARY_BACKGROUND_WRITE_REFUSAL;
mod settings;
#[cfg(test)]
mod settings_tests;
#[cfg(test)]
mod stale_row_tests;
mod stock_copy;
mod updates;
mod use_site;
mod write_done;
#[cfg(test)]
mod write_off_thread_tests;

use recovery::{
    handle_recovery_broken_binding, handle_recovery_git_missing, handle_recovery_library_missing,
    handle_recovery_library_missing_locate_result, open_error_notice, route_open_error,
};

impl Signex {
    /// Library arms that can queue a git commit (a primitive Save) or log a
    /// diagnostic run `finish_update` so the queue drains and the Messages
    /// panel republishes in the same frame (#111). Per-event traffic
    /// (editor cursor / drag ticks, search keystrokes, non-Save primitive
    /// edits) cannot queue a commit, so it only does the cheap diagnostics
    /// republish instead of the full `finish_update`.
    pub(crate) fn dispatch_library_message(&mut self, msg: LibraryMessage) -> Task<Message> {
        if let Some(refusal) = self.library_write_refusal(&msg) {
            crate::diagnostics::log_warning(&refusal);
            self.document_state.error_notice =
                Some(crate::app::state::ErrorNotice::library_change(refusal));
            return self.finish_update();
        }
        let per_event = matches!(
            &msg,
            LibraryMessage::EditorEvent { .. }
                | LibraryMessage::BrowserSearchChanged { .. }
                | LibraryMessage::PrimitiveEditorEvent {
                    msg: PrimitiveEdit::Symbol(_) | PrimitiveEdit::Footprint(_),
                    ..
                }
        );
        let task = self.route_library_message(msg);
        if per_event {
            self.sync_diagnostics_panel_ctx();
            return task;
        }
        Task::batch([task, self.finish_update()])
    }

    fn route_library_message(&mut self, msg: LibraryMessage) -> Task<Message> {
        match msg {
            LibraryMessage::OpenLibraryDialog => self.handle_open_library_dialog(),
            LibraryMessage::OpenLibraryAt(None) => Task::none(),
            LibraryMessage::OpenLibraryAt(Some(path)) => self.handle_open_library_at(path),
            LibraryMessage::CloseLibrary(path) => self.handle_close_library(path),
            LibraryMessage::OpenPicker => {
                self.library.picker = Some(PickerState::default());
                self.library.sync_view_caches();
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
            // When no library is open OR no table exists, nothing can
            // be appended; the handler tells the Messages panel why.
            LibraryMessage::NewComponent => self.handle_new_component(),
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
            LibraryMessage::BrowserCommitPending { library_path } => {
                self.handle_browser_commit_pending(library_path)
            }
            LibraryMessage::BrowserCommitPendingDone {
                library_path,
                result,
            } => self.handle_browser_commit_pending_done(&library_path, result),
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
            LibraryMessage::BrowserConfirmRenameTable { library_path } => {
                self.handle_browser_confirm_rename_table(library_path)
            }
            LibraryMessage::BrowserConfirmAddTable { library_path } => {
                self.handle_browser_confirm_add_table(library_path)
            }

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
            LibraryMessage::JumpToUseSite(site) => self.handle_jump_to_use_site(site),
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
            LibraryMessage::DriftChecked(report) => self.handle_library_drift_checked(*report),
            LibraryMessage::LibraryUpdateSymbolsRead(read) => {
                self.handle_library_update_symbols_read(*read)
            }
            LibraryMessage::LibraryComponentResolved {
                target_sheet,
                arm_epoch,
                library_path,
                result,
            } => self.handle_library_component_resolved(
                target_sheet,
                arm_epoch,
                library_path,
                result,
            ),
            LibraryMessage::DesignatorsScanned { generation, result } => {
                self.apply_designator_scan(generation, &result);
                Task::none()
            }
            LibraryMessage::PlacedPart(msg) => self.dispatch_placed_part_message(msg),
            LibraryMessage::SymbolChoiceResolved {
                target_sheet,
                engine_instance,
                symbol_id,
                request,
                label,
                result,
            } => self.handle_symbol_choice_resolved(
                crate::app::handlers::library_symbol_choice::SwitchContext {
                    target_sheet,
                    engine_instance,
                    symbol_id,
                    request,
                    label,
                },
                result,
            ),
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
            LibraryMessage::AddLibrarySymbolFilePicked { project_id, path } => {
                self.handle_add_library_symbol_file_picked(project_id, path)
            }
            LibraryMessage::AddLibraryFootprintFilePicked { project_id, path } => {
                self.handle_add_library_footprint_file_picked(project_id, path)
            }
            LibraryMessage::AddLibraryFileWritten {
                project_id,
                path,
                result,
            } => self.handle_add_library_file_written(project_id, path, result),
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
            LibraryMessage::MountFinished { path, prepared } => {
                self.handle_mount_finished(path, prepared)
            }
            LibraryMessage::RefreshFinished {
                path,
                generation,
                snapshot,
            } => {
                let task = self.handle_refresh_finished(path, generation, snapshot);
                self.close_libraries_whose_writes_are_done();
                task
            }
            LibraryMessage::PreviewsReread { readout } => {
                self.handle_previews_reread(readout);
                Task::none()
            }
            LibraryMessage::WriteFinished(done) => self.handle_write_finished(*done),
            LibraryMessage::BrowserSelectTable {
                library_path,
                table,
            } => self.handle_browser_select_table(library_path, table),
            LibraryMessage::BrowserSearchChanged {
                library_path,
                value,
            } => self.handle_browser_search_changed(library_path, value),
            LibraryMessage::BrowserRangeFilter { library_path, msg } => {
                self.handle_browser_range_filter(library_path, msg)
            }
            LibraryMessage::BrowserSortColumn {
                library_path,
                column_key,
            } => self.handle_browser_sort_column(library_path, column_key),
            LibraryMessage::BrowserSelectRow {
                library_path,
                table,
                row_id,
            } => self.handle_browser_select_row(library_path, table, row_id),
            LibraryMessage::BrowserGridScrolled {
                library_path,
                offset_y,
            } => self.handle_browser_grid_scrolled(library_path, offset_y),
            LibraryMessage::BrowserGridResized {
                library_path,
                viewport_h,
            } => self.handle_browser_grid_resized(library_path, viewport_h),
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
            LibraryMessage::GitReinitFinished { path, result } => {
                recovery::handle_git_reinit_finished(self, path, result)
            }
            LibraryMessage::RecoveryBrokenBinding(choice) => {
                handle_recovery_broken_binding(self, choice)
            }

            // ── Library Updates Available modal (Stage 16) ─────────
            LibraryMessage::LibraryUpdatesToggleSelection(symbol_uuid) => {
                self.handle_library_updates_toggle_selection(symbol_uuid)
            }
            LibraryMessage::LibraryUpdatesApply => self.handle_library_updates_apply(),
            LibraryMessage::LibraryUpdatesSkipAll => self.handle_library_updates_skip_all(),
            LibraryMessage::LibraryUpdatesCancel => {
                self.library.library_updates = None;
                self.show_next_queued_updates();
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
            LibraryMessage::ComponentsPanelPlace {
                library_path,
                table,
                row_id,
            } => self.handle_components_panel_place(library_path, table, row_id),
            LibraryMessage::ExternalEditsPollTicked => self.handle_external_edits_poll_ticked(),
            LibraryMessage::ExternalEditsPolled(changed) => {
                self.handle_external_edits_polled(changed)
            }
            LibraryMessage::PricingRefreshFinished {
                library_path,
                outcome,
            } => self.handle_pricing_refresh_finished(library_path, outcome),
            LibraryMessage::StockCopy(msg) => self.handle_stock_copy(msg),
        }
    }
}
