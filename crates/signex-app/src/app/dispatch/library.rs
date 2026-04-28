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

use iced::Task;

use super::super::*;
use crate::library::commands;
use crate::library::messages::{
    BrowserEditMsg, CloseLibraryChoice, EditorMsg, GraphicHandleMsg, LibraryMessage, ParamKindMsg,
    PickerMsg, PrimitiveEditorMsg, PrimitivePickerMsg, SettingsMsg, SymbolSelectionMsg,
    SymbolToolMsg,
};
use crate::library::state::{
    CloseLibraryConfirmState, ComponentPreviewState, DeleteConfirmState, DocumentOptionsModalState,
    EditRowModalState, EditorAddress, LibraryCreateOptionsState, NewComponentState, PickerState,
    PreviewTab, PrimitivePickerState, PrimitivePickerTarget,
};
use signex_library::{PrimitiveKind, PrimitiveRef, RowId};

impl Signex {
    pub(crate) fn dispatch_library_message(&mut self, msg: LibraryMessage) -> Task<Message> {
        match msg {
            LibraryMessage::OpenLibraryDialog => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Open Library (*.snxlib/)")
                        .pick_folder()
                        .await
                        .map(|f| f.path().to_path_buf())
                },
                |path| Message::Library(LibraryMessage::OpenLibraryAt(path)),
            ),
            LibraryMessage::OpenLibraryAt(None) => Task::none(),
            LibraryMessage::OpenLibraryAt(Some(path)) => {
                if let Err(e) = commands::open_library(&mut self.library, path.clone()) {
                    tracing::warn!(target: "signex::library", error = %e, path = %path.display(), "open_library failed");
                    route_open_error(&mut self.library, &path, &e);
                }
                Task::none()
            }
            LibraryMessage::CloseLibrary(path) => {
                // If any Component Preview editors against this library
                // are dirty, divert to the confirm modal so the user
                // can Save All / Discard All / Cancel rather than
                // losing the edits silently. The modal handler
                // (`CloseLibraryConfirm`) finishes the close once the
                // user picks an option.
                let dirty = self.library.dirty_editors_for_library(&path);
                if dirty.is_empty() {
                    self.library.close_library(&path);
                } else {
                    let library_name = self
                        .library
                        .library_at(&path)
                        .map(|lib| lib.display_name.clone())
                        .unwrap_or_else(|| {
                            path.file_name()
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_else(|| path.display().to_string())
                        });
                    self.library.close_library_confirm = Some(CloseLibraryConfirmState {
                        library_path: path,
                        library_name,
                        dirty_editors: dirty,
                    });
                }
                Task::none()
            }
            LibraryMessage::OpenPicker => {
                self.library.picker = Some(PickerState::default());
                Task::none()
            }
            LibraryMessage::ClosePicker => {
                self.library.picker = None;
                Task::none()
            }

            // ── New Component flow ───────────────────────────────────
            LibraryMessage::NewComponent => {
                self.library.new_component = Some(NewComponentState {
                    library_idx: (!self.library.open_libraries.is_empty()).then_some(0),
                    ..NewComponentState::default()
                });
                Task::none()
            }
            LibraryMessage::CloseNewComponent => {
                self.library.new_component = None;
                Task::none()
            }
            LibraryMessage::NewComponentSetInternalPn(s) => {
                if let Some(nc) = self.library.new_component.as_mut() {
                    nc.internal_pn = s;
                    nc.error = None;
                }
                Task::none()
            }
            LibraryMessage::NewComponentSetLibrary(idx) => {
                if let Some(nc) = self.library.new_component.as_mut() {
                    nc.library_idx = Some(idx);
                    nc.error = None;
                }
                Task::none()
            }
            LibraryMessage::NewComponentSetClass(class) => {
                // Changing class does NOT overwrite the table pick —
                // that's the user's explicit choice. Class only
                // affects the parameter template.
                if let Some(nc) = self.library.new_component.as_mut() {
                    nc.class = class;
                    nc.error = None;
                }
                Task::none()
            }
            LibraryMessage::NewComponentSetTable(name) => {
                // User picked a target table. If exactly one class is
                // associated with this table in the manifest, surface
                // that as the auto-class so the form fills out
                // sensibly. Otherwise the user keeps editing the class
                // independently.
                if let Some(nc) = self.library.new_component.as_mut() {
                    if !name.is_empty() {
                        nc.table = Some(name.clone());
                        // Try to autoselect the matching class from the
                        // manifest (`[[tables]]` override). Only triggers
                        // when the user picked a manifest-declared table.
                        if let Some(library_idx) = nc.library_idx
                            && let Some(lib) = self.library.open_libraries.get(library_idx)
                            && let Some(adapter) = self.library.set.get(lib.library_id)
                            && let Some(cfg) =
                                adapter.manifest().tables().iter().find(|c| c.name == name)
                            && let Some(first) = cfg.classes.first()
                        {
                            nc.class = signex_library::ComponentClass::new(first);
                        }
                    } else {
                        nc.table = None;
                    }
                    nc.error = None;
                }
                Task::none()
            }
            LibraryMessage::NewComponentSetCategory(s) => {
                if let Some(nc) = self.library.new_component.as_mut() {
                    nc.category = s;
                    nc.error = None;
                }
                Task::none()
            }
            LibraryMessage::NewComponentSubmit => {
                let Some(nc) = self.library.new_component.as_ref().cloned() else {
                    return Task::none();
                };
                let library_idx = match nc.library_idx {
                    Some(i) => i,
                    None => {
                        if let Some(slot) = self.library.new_component.as_mut() {
                            slot.error = Some("Pick a target library before submitting.".into());
                        }
                        return Task::none();
                    }
                };
                // Target table — modal pick takes precedence. When
                // the manifest declared no `[[tables]]` overrides the
                // modal still surfaces a default-pluralised slot;
                // fall back to `Manifest::table_for_class` if the
                // user submitted with an unset pick (ghost case when
                // the modal opens with neither a pre-pick nor a
                // user-selected table).
                let library_path = match self.library.open_libraries.get(library_idx) {
                    Some(lib) => lib.root.clone(),
                    None => {
                        if let Some(slot) = self.library.new_component.as_mut() {
                            slot.error = Some("Selected library is no longer open.".into());
                        }
                        return Task::none();
                    }
                };
                let table = match nc.table.clone() {
                    Some(t) => t,
                    None => {
                        let resolved = self
                            .library
                            .open_libraries
                            .get(library_idx)
                            .and_then(|lib| self.library.set.get(lib.library_id))
                            .map(|adapter| adapter.manifest().table_for_class(nc.class.as_str()));
                        match resolved {
                            Some(t) => t,
                            None => {
                                if let Some(slot) = self.library.new_component.as_mut() {
                                    slot.error =
                                        Some("Pick a target table before submitting.".into());
                                }
                                return Task::none();
                            }
                        }
                    }
                };
                match commands::create_component_row(
                    &mut self.library,
                    library_idx,
                    &table,
                    &nc.internal_pn,
                    nc.class.clone(),
                    nc.symbol_ref,
                    nc.footprint_ref,
                ) {
                    Ok(row_id) => {
                        self.library.new_component = None;
                        return Task::done(Message::Library(LibraryMessage::OpenComponentRow {
                            library_path,
                            table,
                            row_id,
                        }));
                    }
                    Err(e) => {
                        if let Some(slot) = self.library.new_component.as_mut() {
                            slot.error = Some(e.to_string());
                        }
                    }
                }
                Task::none()
            }
            // ────────────────────────────────────────────────────────
            LibraryMessage::ToggleLibraryTreeNode(idx) => {
                if let Some(slot) = self.library.expanded.get_mut(idx) {
                    *slot = !*slot;
                }
                Task::none()
            }
            LibraryMessage::OpenComponentRow {
                library_path,
                table,
                row_id,
            } => self.handle_open_component_row(library_path, table, row_id),
            LibraryMessage::OpenPrimitiveEditor { path } => self.handle_open_primitive(path),
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
            } => {
                // Direct opener for the modal — used when callers
                // already know the dirty list (e.g. a future
                // workspace-close batch op). For the user-driven
                // close path, `CloseLibrary` is the entry point and
                // it diverts here automatically.
                let library_name = self
                    .library
                    .library_at(&library_path)
                    .map(|lib| lib.display_name.clone())
                    .unwrap_or_else(|| {
                        library_path
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| library_path.display().to_string())
                    });
                self.library.close_library_confirm = Some(CloseLibraryConfirmState {
                    library_path,
                    library_name,
                    dirty_editors,
                });
                Task::none()
            }
            LibraryMessage::CloseLibraryConfirm(choice) => {
                let Some(confirm) = self.library.close_library_confirm.take() else {
                    return Task::none();
                };
                match choice {
                    CloseLibraryChoice::Cancel => {
                        // No state change — user kept the library open.
                    }
                    CloseLibraryChoice::DiscardAll => {
                        // Drop every dirty editor and proceed with the close.
                        // `close_library` retains-not by `library_path`, so
                        // this happens automatically as part of the close.
                        self.library.close_library(&confirm.library_path);
                    }
                    CloseLibraryChoice::SaveAll => {
                        // Persist every dirty editor's row through the
                        // adapter (`handle_save_row` already runs the
                        // hash + commit cycle), then close the
                        // library. Failures are logged; we still
                        // proceed with the close so the user isn't
                        // trapped (the rows stay on disk in their
                        // last good state).
                        for address in &confirm.dirty_editors {
                            self.handle_save_row(address);
                        }
                        self.library.close_library(&confirm.library_path);
                    }
                }
                Task::none()
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
            } => {
                // Stage 11 of `v0.9-snxlib-as-file-plan.md`: pop the
                // "Library Options" modal here instead of creating
                // immediately. The modal lets the user opt into Git
                // LFS for binary 3D models before any disk +
                // `git init` runs. Confirming dispatches
                // `LibraryCreateOptionsConfirm` which calls into
                // `handle_create_library_at_path`.
                self.library.create_options = Some(LibraryCreateOptionsState {
                    project_path,
                    lib_path,
                    use_lfs: false,
                });
                Task::none()
            }
            LibraryMessage::LibraryCreateOptionsToggleLfs => {
                if let Some(state) = self.library.create_options.as_mut() {
                    state.use_lfs = !state.use_lfs;
                }
                Task::none()
            }
            LibraryMessage::LibraryCreateOptionsCancel => {
                self.library.create_options = None;
                Task::none()
            }
            LibraryMessage::LibraryCreateOptionsConfirm => {
                if let Some(state) = self.library.create_options.take() {
                    self.handle_create_library_at_path(
                        state.project_path,
                        state.lib_path,
                        state.use_lfs,
                    )
                } else {
                    Task::none()
                }
            }
            LibraryMessage::ComponentPreviewOpened {
                path,
                table,
                row_id,
            } => {
                tracing::debug!(
                    target: "signex::library",
                    path = %path.display(),
                    table = %table,
                    row_id = %row_id,
                    "ComponentPreviewOpened — Component Preview tab opened"
                );
                Task::none()
            }
            LibraryMessage::PrimitiveEditorEvent { path, msg } => {
                self.handle_primitive_editor_event(path, msg)
            }
            // ── Library Browser tab ──────────────────────────────────
            LibraryMessage::OpenLibraryBrowser(path) => self.handle_open_library_browser(path),
            LibraryMessage::BrowserSelectTable {
                library_path,
                table,
            } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.active_table = Some(table);
                    state.selected_row = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserSearchChanged {
                library_path,
                value,
            } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.search = value;
                }
                Task::none()
            }
            LibraryMessage::BrowserSelectRow {
                library_path,
                table,
                row_id,
            } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    // Switch active table when the click lands on a row
                    // in a different table — keeps the preview pane and
                    // selection coherent.
                    state.active_table = Some(table);
                    state.selected_row = Some(row_id);
                }
                Task::none()
            }
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
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.delete_confirm = None;
                }
                Task::none()
            }
            LibraryMessage::OpenPrimitivePicker { kind, target } => {
                self.library.primitive_picker = Some(PrimitivePickerState {
                    kind,
                    target,
                    filter: String::new(),
                    error: None,
                });
                Task::none()
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
            } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.cell_edit.insert((row_id, column), value);
                }
                Task::none()
            }
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
            } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.cell_edit.remove(&(row_id, column));
                }
                Task::none()
            }
            // ── Document Options modal (Tools ▸ Document Options) ──
            LibraryMessage::OpenDocumentOptions { library_path } => {
                if let Some(lib) = self.library.library_at(&library_path) {
                    self.library.document_options = Some(DocumentOptionsModalState {
                        library_path: lib.root.clone(),
                        library_name: lib.display_name.clone(),
                        draft: lib.display,
                    });
                }
                Task::none()
            }
            LibraryMessage::DocumentOptionsSetSheetColor(c) => {
                if let Some(s) = self.library.document_options.as_mut() {
                    s.draft.sheet_color = c;
                }
                Task::none()
            }
            LibraryMessage::DocumentOptionsToggleGrid => {
                if let Some(s) = self.library.document_options.as_mut() {
                    s.draft.grid_visible = !s.draft.grid_visible;
                }
                Task::none()
            }
            LibraryMessage::DocumentOptionsCycleGridSize => {
                if let Some(s) = self.library.document_options.as_mut() {
                    let sizes = crate::canvas::grid::GRID_SIZES_MM;
                    let i = sizes
                        .iter()
                        .position(|sz| (sz - s.draft.grid_size_mm).abs() < f32::EPSILON)
                        .unwrap_or(2);
                    s.draft.grid_size_mm = sizes[(i + 1) % sizes.len()];
                }
                Task::none()
            }
            LibraryMessage::DocumentOptionsCycleUnit => {
                use signex_types::coord::Unit;
                if let Some(s) = self.library.document_options.as_mut() {
                    s.draft.unit = match s.draft.unit {
                        Unit::Mm => Unit::Mil,
                        Unit::Mil => Unit::Inch,
                        Unit::Inch => Unit::Micrometer,
                        Unit::Micrometer => Unit::Mm,
                    };
                }
                Task::none()
            }
            LibraryMessage::DocumentOptionsApply => {
                if let Some(s) = self.library.document_options.take()
                    && let Some(lib) = self.library.containing_library_mut(&s.library_path)
                {
                    lib.display = s.draft;
                }
                // Clear every primitive editor's canvas cache so the
                // new sheet color / grid paints immediately. Cheap.
                for editor in self.document_state.symbol_editors.values_mut() {
                    editor.canvas_cache.clear();
                }
                for editor in self.document_state.footprint_editors.values_mut() {
                    editor.canvas_cache.clear();
                }
                Task::none()
            }
            LibraryMessage::DocumentOptionsCancel => {
                self.library.document_options = None;
                Task::none()
            }

            // Recovery dialogs (Stage 10).
            LibraryMessage::RecoveryLibraryMissing(choice) => {
                handle_recovery_library_missing(self, choice)
            }
            LibraryMessage::RecoveryLibraryMissingLocateResult(picked) => {
                self.library.recovery = None;
                if let Some(new_path) = picked {
                    return Task::done(Message::Library(LibraryMessage::OpenLibraryAt(Some(
                        new_path,
                    ))));
                }
                Task::none()
            }
            LibraryMessage::RecoveryGitMissing(choice) => {
                handle_recovery_git_missing(self, choice)
            }
            LibraryMessage::RecoveryBrokenBinding(choice) => {
                handle_recovery_broken_binding(self, choice)
            }
        }
    }

    /// Open `.snxlib` at `path` as a Library Browser tab. Mounts the
    /// library if not already mounted, seeds the browser state, and
    /// pushes (or activates) a `TabKind::LibraryBrowser` tab. Phase 1.
    pub(crate) fn handle_open_library_browser(
        &mut self,
        path: std::path::PathBuf,
    ) -> Task<Message> {
        // 1. Mount the library if it isn't already. `open_library` is
        //    idempotent — re-mounting an already-open library is a
        //    no-op.
        if let Err(e) = commands::open_library(&mut self.library, path.clone()) {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %e,
                "open_library_browser: open_library failed"
            );
        }

        // 2. Seed per-browser state if the path isn't already there.
        // 2b. Default `active_table` to the first table the library
        //     exposes, if any. Compute it through an immutable borrow
        //     before we take the mutable browser-entry.
        let default_table: Option<String> = self.library.library_at(&path).and_then(|lib| {
            let mut names: Vec<&String> = lib.tables.keys().collect();
            names.sort();
            names.first().map(|s| (*s).clone())
        });

        let entry = self
            .library
            .library_browsers
            .entry(path.clone())
            .or_insert_with(|| crate::library::state::LibraryBrowserState::new(path.clone()));

        if entry.active_table.is_none() {
            entry.active_table = default_table;
        }

        // 3. Activate an existing tab if one is already open for this
        //    path; otherwise push a fresh tab.
        if let Some(idx) = self.document_state.tabs.iter().position(|t| t.path == path) {
            if idx != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = idx;
                self.sync_active_tab();
            }
            return Task::none();
        }

        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| path.display().to_string());
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);

        self.park_active_schematic_session();
        self.document_state.tabs.push(crate::app::TabInfo {
            title,
            path: path.clone(),
            cached_document: None,
            dirty: false,
            project_id,
            kind: crate::app::TabKind::LibraryBrowser(path),
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        // Library Browser tabs don't drive `active_path` — clear so the
        // canvas pane doesn't render a stale schematic.
        self.document_state.active_path = None;
        self.refresh_panel_ctx();
        Task::none()
    }

    /// Open the New Component modal pre-set to the browser's library
    /// + active table. Wraps the existing `NewComponent` flow.
    fn handle_browser_add_component(
        &mut self,
        library_path: std::path::PathBuf,
        table: Option<String>,
    ) -> Task<Message> {
        // Find the library index inside `open_libraries`. The modal's
        // pick_list is index-based, not path-based, so we need the
        // position lookup.
        let library_idx = self
            .library
            .open_libraries
            .iter()
            .position(|lib| lib.root == library_path);
        tracing::warn!(
            target: "signex::library",
            library = %library_path.display(),
            ?table,
            ?library_idx,
            "browser: Add Component clicked — opening New Component modal"
        );
        self.library.new_component = Some(NewComponentState {
            internal_pn: String::new(),
            library_idx,
            table,
            class: signex_library::ComponentClass::generic(),
            category: String::new(),
            symbol_ref: None,
            footprint_ref: None,
            error: None,
        });
        Task::none()
    }

    /// Phase 2 — open the delete-row confirm modal. Records
    /// `(table, row_id, internal_pn)` on the browser state so the
    /// modal can render a confident message.
    fn handle_browser_delete_row_request(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        let internal_pn = self
            .library
            .library_at(&library_path)
            .and_then(|lib| lib.tables.get(&table))
            .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
            .map(|r| r.internal_pn.as_str().to_string())
            .unwrap_or_else(|| format!("row {row_id}"));
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.delete_confirm = Some(DeleteConfirmState {
                table,
                row_id,
                internal_pn,
            });
        }
        Task::none()
    }

    /// Confirm step — actually delete the row through
    /// `adapter.delete_row` and refresh the cache.
    fn handle_browser_delete_row_confirm(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    "browser delete: library not mounted"
                );
                return Task::none();
            }
        };
        let adapter = match self.library.set.get(library_id) {
            Some(a) => a,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    "browser delete: adapter not present in set"
                );
                return Task::none();
            }
        };
        match adapter.delete_row(&table, row_id, "delete row") {
            Ok(_) => {
                tracing::info!(
                    target: "signex::library",
                    path = %library_path.display(),
                    table = %table,
                    row = %row_id,
                    "browser delete: row removed"
                );
                if let Err(e) = self.library.refresh_components(&library_path) {
                    tracing::warn!(
                        target: "signex::library",
                        path = %library_path.display(),
                        error = %e,
                        "browser delete: refresh_components failed"
                    );
                }
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    if state.selected_row == Some(row_id) {
                        state.selected_row = None;
                    }
                    state.delete_confirm = None;
                    // Drop any cached cell-edit buffers for the gone row.
                    state.cell_edit.retain(|(rid, _), _| *rid != row_id);
                }
            }
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    table = %table,
                    error = %e,
                    "browser delete: delete_row failed"
                );
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.delete_confirm = None;
                }
            }
        }
        Task::none()
    }

    /// Open the Edit Component Details modal for a row. Loads the row
    /// from the library cache and seeds the modal with a working copy.
    fn handle_browser_open_edit_modal(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        let row = self
            .library
            .library_at(&library_path)
            .and_then(|lib| lib.tables.get(&table))
            .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
            .cloned();
        let Some(row) = row else {
            tracing::warn!(
                target: "signex::library",
                path = %library_path.display(),
                table = %table,
                row = %row_id,
                "browser open edit modal: row not found in cache"
            );
            return Task::none();
        };
        let address = EditorAddress::new(library_path.clone(), table, row_id);
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.edit_modal = Some(EditRowModalState::new(address, row));
        }
        Task::none()
    }

    /// Apply a `BrowserEditMsg` to the active edit modal for `library_path`.
    fn handle_browser_edit_msg(
        &mut self,
        library_path: std::path::PathBuf,
        msg: BrowserEditMsg,
    ) -> Task<Message> {
        // Some variants need to fire follow-up tasks (open picker,
        // close modal). We collect those into `next` and return them
        // after releasing the borrow.
        let mut next: Option<Task<Message>> = None;
        // Save needs a separate path — we read the draft, drop the
        // borrow, run the adapter call, then resume.
        let mut save_request: Option<(EditorAddress, signex_library::ComponentRow)> = None;
        let mut close_modal = false;
        if let Some(state) = self.library.library_browsers.get_mut(&library_path)
            && let Some(modal) = state.edit_modal.as_mut()
        {
            match msg {
                BrowserEditMsg::SetInternalPn(s) => {
                    modal.draft.internal_pn = signex_library::InternalPn::new(s);
                    modal.error = None;
                }
                BrowserEditMsg::SetClass(class) => {
                    modal.draft.class = class;
                    modal.error = None;
                }
                BrowserEditMsg::SetState(state_v) => {
                    modal.draft.state = state_v;
                    modal.error = None;
                }
                BrowserEditMsg::SetDatasheetUrl(s) => {
                    modal.draft.datasheet = signex_library::DatasheetRef::url(s);
                    modal.error = None;
                }
                BrowserEditMsg::SetManufacturer(s) => {
                    modal.draft.primary_mpn.manufacturer = s;
                    modal.error = None;
                }
                BrowserEditMsg::SetMpn(s) => {
                    modal.draft.primary_mpn.mpn = s;
                    modal.error = None;
                }
                BrowserEditMsg::SetParamValue { key, value } => {
                    let entry = modal
                        .param_buf
                        .entry(key)
                        .or_insert_with(|| (String::new(), String::new()));
                    entry.0 = value;
                }
                BrowserEditMsg::SetParamUnit { key, unit } => {
                    let entry = modal
                        .param_buf
                        .entry(key)
                        .or_insert_with(|| (String::new(), String::new()));
                    entry.1 = unit;
                }
                BrowserEditMsg::CommitParam { key } => {
                    if let Some((value, unit)) = modal.param_buf.get(&key).cloned() {
                        let pv = if !unit.trim().is_empty() {
                            // Try parse as f64 first, otherwise store as text.
                            value
                                .parse::<f64>()
                                .ok()
                                .map(|n| signex_library::ParamValue::Measurement {
                                    value: n,
                                    unit: unit.clone(),
                                })
                                .unwrap_or_else(|| {
                                    signex_library::ParamValue::Text(format!("{value} {unit}"))
                                })
                        } else if let Ok(n) = value.parse::<f64>() {
                            signex_library::ParamValue::Number(n)
                        } else if value.eq_ignore_ascii_case("true") {
                            signex_library::ParamValue::Bool(true)
                        } else if value.eq_ignore_ascii_case("false") {
                            signex_library::ParamValue::Bool(false)
                        } else {
                            signex_library::ParamValue::Text(value)
                        };
                        modal.draft.parameters.insert(key, pv);
                    }
                }
                BrowserEditMsg::AddParam => {
                    // Find a unique key like "param_N".
                    let mut idx = modal.draft.parameters.len() + 1;
                    let key = loop {
                        let candidate = format!("param_{idx}");
                        if !modal.draft.parameters.contains_key(&candidate) {
                            break candidate;
                        }
                        idx += 1;
                    };
                    modal
                        .draft
                        .parameters
                        .insert(key.clone(), signex_library::ParamValue::Text(String::new()));
                    modal.param_buf.insert(key, (String::new(), String::new()));
                }
                BrowserEditMsg::DeleteParam { key } => {
                    modal.draft.parameters.remove(&key);
                    modal.param_buf.remove(&key);
                }
                BrowserEditMsg::OpenSymbolPicker => {
                    next = Some(Task::done(Message::Library(
                        LibraryMessage::OpenPrimitivePicker {
                            kind: PrimitiveKind::Symbol,
                            target: PrimitivePickerTarget::EditRowModal(modal.address.clone()),
                        },
                    )));
                }
                BrowserEditMsg::OpenFootprintPicker => {
                    next = Some(Task::done(Message::Library(
                        LibraryMessage::OpenPrimitivePicker {
                            kind: PrimitiveKind::Footprint,
                            target: PrimitivePickerTarget::EditRowModal(modal.address.clone()),
                        },
                    )));
                }
                BrowserEditMsg::Save => {
                    save_request = Some((modal.address.clone(), modal.draft.clone()));
                }
                BrowserEditMsg::Cancel => {
                    close_modal = true;
                }
            }
        }
        if close_modal && let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.edit_modal = None;
        }
        if let Some((address, mut draft)) = save_request {
            // Refresh content_hash before saving.
            match signex_library::hash_row_content(&draft) {
                Ok(h) => {
                    draft.content_hash = h;
                }
                Err(e) => {
                    if let Some(state) = self.library.library_browsers.get_mut(&library_path)
                        && let Some(modal) = state.edit_modal.as_mut()
                    {
                        modal.error = Some(format!("hash failed: {e}"));
                    }
                    return next.unwrap_or_else(Task::none);
                }
            }
            let library_id = self
                .library
                .library_at(&address.library_path)
                .map(|lib| lib.library_id);
            let result = match library_id.and_then(|id| self.library.set.get(id)) {
                Some(adapter) => adapter.update_row(&address.table, draft, "edit row"),
                None => Err(signex_library::LibraryError::NotFound(
                    address.library_path.display().to_string(),
                )),
            };
            match result {
                Ok(_) => {
                    if let Err(e) = self.library.refresh_components(&address.library_path) {
                        tracing::warn!(
                            target: "signex::library",
                            path = %address.library_path.display(),
                            error = %e,
                            "browser edit: refresh_components failed"
                        );
                    }
                    if let Some(state) =
                        self.library.library_browsers.get_mut(&address.library_path)
                    {
                        state.edit_modal = None;
                    }
                }
                Err(e) => {
                    if let Some(state) = self.library.library_browsers.get_mut(&library_path)
                        && let Some(modal) = state.edit_modal.as_mut()
                    {
                        modal.error = Some(e.to_string());
                    }
                }
            }
        }
        next.unwrap_or_else(Task::none)
    }

    /// Commit a per-cell inline edit to the row. Re-hashes + persists.
    fn handle_browser_cell_commit(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
        column: String,
    ) -> Task<Message> {
        // Drop the buffer eagerly — if the save fails we re-insert below.
        let buf = match self
            .library
            .library_browsers
            .get_mut(&library_path)
            .and_then(|s| s.cell_edit.remove(&(row_id, column.clone())))
        {
            Some(v) => v,
            None => return Task::none(),
        };
        // Read the current row from the cache, mutate, re-hash, save.
        let mut row = match self
            .library
            .library_at(&library_path)
            .and_then(|lib| lib.tables.get(&table))
            .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
            .cloned()
        {
            Some(r) => r,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    table = %table,
                    row = %row_id,
                    "browser cell commit: row not found in cache"
                );
                return Task::none();
            }
        };
        match column.as_str() {
            "internal_pn" => {
                row.internal_pn = signex_library::InternalPn::new(buf.clone());
            }
            "manufacturer" => {
                row.primary_mpn.manufacturer = buf.clone();
            }
            "mpn" => {
                row.primary_mpn.mpn = buf.clone();
            }
            other if other.starts_with("parameters.") => {
                let key = &other["parameters.".len()..];
                // Preserve unit on commit by reading the existing value.
                let new_value = match row.parameters.get(key) {
                    Some(signex_library::ParamValue::Measurement { unit, .. }) => {
                        match buf.parse::<f64>() {
                            Ok(n) => signex_library::ParamValue::Measurement {
                                value: n,
                                unit: unit.clone(),
                            },
                            Err(_) => signex_library::ParamValue::Text(buf.clone()),
                        }
                    }
                    Some(signex_library::ParamValue::Number(_)) => match buf.parse::<f64>() {
                        Ok(n) => signex_library::ParamValue::Number(n),
                        Err(_) => signex_library::ParamValue::Text(buf.clone()),
                    },
                    Some(signex_library::ParamValue::Bool(_)) => {
                        if buf.eq_ignore_ascii_case("true") {
                            signex_library::ParamValue::Bool(true)
                        } else if buf.eq_ignore_ascii_case("false") {
                            signex_library::ParamValue::Bool(false)
                        } else {
                            signex_library::ParamValue::Text(buf.clone())
                        }
                    }
                    _ => signex_library::ParamValue::Text(buf.clone()),
                };
                row.parameters.insert(key.to_string(), new_value);
            }
            _ => {
                tracing::warn!(
                    target: "signex::library",
                    column = %column,
                    "browser cell commit: unknown column"
                );
                return Task::none();
            }
        }
        match signex_library::hash_row_content(&row) {
            Ok(h) => row.content_hash = h,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    error = %e,
                    "browser cell commit: hash failed; reverting buffer"
                );
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.cell_edit.insert((row_id, column), buf);
                }
                return Task::none();
            }
        }
        let library_id = self
            .library
            .library_at(&library_path)
            .map(|lib| lib.library_id);
        let result = match library_id.and_then(|id| self.library.set.get(id)) {
            Some(adapter) => adapter.update_row(&table, row, "edit cell"),
            None => Err(signex_library::LibraryError::NotFound(
                library_path.display().to_string(),
            )),
        };
        if let Err(e) = result {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "browser cell commit: update_row failed"
            );
            if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                state.cell_edit.insert((row_id, column), buf);
            }
            return Task::none();
        }
        if let Err(e) = self.library.refresh_components(&library_path) {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "browser cell commit: refresh_components failed"
            );
        }
        Task::none()
    }

    /// Apply a primitive picker sub-message. Most variants close the
    /// modal once the pick lands.
    fn handle_primitive_picker_msg(&mut self, msg: PrimitivePickerMsg) -> Task<Message> {
        match msg {
            PrimitivePickerMsg::SetFilter(s) => {
                if let Some(picker) = self.library.primitive_picker.as_mut() {
                    picker.filter = s;
                    picker.error = None;
                }
                Task::none()
            }
            PrimitivePickerMsg::Cancel => {
                self.library.primitive_picker = None;
                Task::none()
            }
            PrimitivePickerMsg::Pick(primitive_ref) => self.apply_primitive_pick(primitive_ref),
            PrimitivePickerMsg::Browse => {
                let kind = self
                    .library
                    .primitive_picker
                    .as_ref()
                    .map(|p| p.kind)
                    .unwrap_or(PrimitiveKind::Symbol);
                let (label, ext) = match kind {
                    PrimitiveKind::Symbol => ("Pick Symbol (*.snxsym)", "snxsym"),
                    PrimitiveKind::Footprint => ("Pick Footprint (*.snxfpt)", "snxfpt"),
                    PrimitiveKind::Sim => ("Pick Sim Model (*.snxsim)", "snxsim"),
                    _ => ("Pick Primitive", ""),
                };
                Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .set_title(label)
                            .add_filter(ext, &[ext])
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    |path| {
                        Message::Library(LibraryMessage::PrimitivePicker(
                            PrimitivePickerMsg::BrowseResult(path),
                        ))
                    },
                )
            }
            PrimitivePickerMsg::BrowseResult(None) => Task::none(),
            PrimitivePickerMsg::BrowseResult(Some(path)) => {
                self.handle_primitive_picker_browse_result(path)
            }
        }
    }

    /// A primitive ref has been picked — apply it to the picker's
    /// configured target and close the modal.
    fn apply_primitive_pick(&mut self, primitive_ref: PrimitiveRef) -> Task<Message> {
        let Some(picker) = self.library.primitive_picker.take() else {
            return Task::none();
        };
        match picker.target {
            PrimitivePickerTarget::PreviewRow(address) => {
                self.apply_primitive_pick_to_preview(address, picker.kind, primitive_ref);
            }
            PrimitivePickerTarget::EditRowModal(address) => {
                if let Some(state) = self.library.library_browsers.get_mut(&address.library_path)
                    && let Some(modal) = state.edit_modal.as_mut()
                    && modal.address == address
                {
                    match picker.kind {
                        PrimitiveKind::Symbol => {
                            modal.draft.symbol_ref = primitive_ref;
                        }
                        PrimitiveKind::Footprint => {
                            modal.draft.footprint_ref = Some(primitive_ref);
                        }
                        PrimitiveKind::Sim => {
                            modal.draft.sim_ref = Some(primitive_ref);
                        }
                        _ => {}
                    }
                    modal.error = None;
                }
            }
            PrimitivePickerTarget::NewComponentForm => {
                if let Some(nc) = self.library.new_component.as_mut() {
                    match picker.kind {
                        PrimitiveKind::Symbol => {
                            nc.symbol_ref = Some(primitive_ref);
                        }
                        PrimitiveKind::Footprint => {
                            nc.footprint_ref = Some(primitive_ref);
                        }
                        PrimitiveKind::Sim => { /* nothing today */ }
                        _ => {}
                    }
                    nc.error = None;
                }
            }
        }
        Task::none()
    }

    /// Component Preview tab — apply a freshly-picked primitive ref to
    /// the row, resolve through the LibrarySet, save via update_row.
    fn apply_primitive_pick_to_preview(
        &mut self,
        address: EditorAddress,
        kind: PrimitiveKind,
        primitive_ref: PrimitiveRef,
    ) {
        let Some(state) = self.library.editors.get_mut(&address) else {
            return;
        };
        match kind {
            PrimitiveKind::Symbol => {
                state.row.symbol_ref = primitive_ref;
                state.symbol = self.library.set.resolve_symbol(&primitive_ref);
            }
            PrimitiveKind::Footprint => {
                state.row.footprint_ref = Some(primitive_ref);
                state.footprint = self.library.set.resolve_footprint(&primitive_ref);
            }
            PrimitiveKind::Sim => {
                state.row.sim_ref = Some(primitive_ref);
                state.sim = self.library.set.resolve_sim(&primitive_ref);
            }
            _ => return,
        }
        // Refresh content_hash + save.
        let mut row = state.row.clone();
        match signex_library::hash_row_content(&row) {
            Ok(h) => {
                row.content_hash = h;
                state.row.content_hash = h;
            }
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    error = %e,
                    "primitive pick: hash failed"
                );
                return;
            }
        }
        let library_id = self
            .library
            .library_at(&address.library_path)
            .map(|lib| lib.library_id);
        let msg = match kind {
            PrimitiveKind::Symbol => "bind symbol",
            PrimitiveKind::Footprint => "bind footprint",
            PrimitiveKind::Sim => "bind sim",
            _ => "bind primitive",
        };
        let result = match library_id.and_then(|id| self.library.set.get(id)) {
            Some(adapter) => adapter.update_row(&address.table, row, msg),
            None => Err(signex_library::LibraryError::NotFound(
                address.library_path.display().to_string(),
            )),
        };
        if let Err(e) = result {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "primitive pick: update_row failed"
            );
            return;
        }
        if let Err(e) = self.library.refresh_components(&address.library_path) {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "primitive pick: refresh_components failed"
            );
        }
    }

    /// Filesystem-picked primitive — auto-mount the containing
    /// `.snxlib`, then synthesize a Pick.
    fn handle_primitive_picker_browse_result(&mut self, file: std::path::PathBuf) -> Task<Message> {
        // Locate the containing `.snxlib`. Path layout is
        // `<some>/<lib>.snxlib/<symbols|footprints|sims>/<uuid>.<ext>`.
        let snxlib_dir = file
            .ancestors()
            .find(|p| {
                p.extension()
                    .and_then(|s| s.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("snxlib"))
                    .unwrap_or(false)
            })
            .map(|p| p.to_path_buf());
        let Some(snxlib_dir) = snxlib_dir else {
            if let Some(picker) = self.library.primitive_picker.as_mut() {
                picker.error = Some(
                    "Picked file is not inside a `.snxlib` library. v0.9 only supports primitives bound through libraries."
                        .into(),
                );
            }
            return Task::none();
        };
        // Mount the library if not already.
        if let Err(e) = commands::open_library(&mut self.library, snxlib_dir.clone()) {
            tracing::warn!(
                target: "signex::library",
                path = %snxlib_dir.display(),
                error = %e,
                "browse-pick: open_library failed"
            );
            if let Some(picker) = self.library.primitive_picker.as_mut() {
                picker.error = Some(format!("open library failed: {e}"));
            }
            return Task::none();
        }
        // Resolve library_id + parse uuid from filename.
        let library_id = match self.library.library_at(&snxlib_dir) {
            Some(lib) => lib.library_id,
            None => {
                if let Some(picker) = self.library.primitive_picker.as_mut() {
                    picker.error = Some("Library failed to mount.".into());
                }
                return Task::none();
            }
        };
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let uuid = match uuid::Uuid::parse_str(stem) {
            Ok(u) => u,
            Err(_) => {
                if let Some(picker) = self.library.primitive_picker.as_mut() {
                    picker.error = Some(format!(
                        "Filename `{stem}` is not a UUID — pick a primitive file in `<lib>.snxlib/symbols/`."
                    ));
                }
                return Task::none();
            }
        };
        let primitive_ref = PrimitiveRef::new(library_id, uuid);
        Task::done(Message::Library(LibraryMessage::PrimitivePicker(
            PrimitivePickerMsg::Pick(primitive_ref),
        )))
    }

    /// Spawn the "New Component Library" Save-As dialog for the
    /// project rooted at `project_root`. The dialog defaults to
    /// `<project_dir>/<project>-lib.snxlib` so the common
    /// project-local case is one Enter key, but the user can navigate
    /// to a global directory to create a shared library. On confirm,
    /// the dialog dispatches `CreateLibraryAtPath` which calls
    /// `commands::create_library_at` to do the actual disk + manifest
    /// + git init.
    ///
    /// We deliberately do NOT touch disk here — the previous "instant
    /// create on click" behaviour was confusing because users
    /// couldn't see where it was going to land or override the
    /// default name.
    fn handle_create_library_for_project(
        &mut self,
        project_root: std::path::PathBuf,
    ) -> Task<Message> {
        // Locate the LoadedProject so we can derive the suggested
        // path. The dispatch handler that consumes the dialog result
        // re-resolves the project at apply time so a project unload
        // between dialog spawn + confirm is recoverable.
        let Some(loaded) =
            self.document_state.projects.iter().find(|p| {
                p.path == project_root || p.path.parent() == Some(project_root.as_path())
            })
        else {
            tracing::warn!(
                target: "signex::library",
                path = %project_root.display(),
                "create library: no loaded project matches root"
            );
            return Task::none();
        };

        let stem = loaded
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("project");
        let mut name = format!("{stem}-lib");

        let project_dir = std::path::PathBuf::from(&loaded.data.dir);
        // Pre-disambiguate the default name so the user doesn't get a
        // misleading "<project>-lib" suggestion when that path already
        // exists. Conflicts get a `-2`, `-3`, … suffix matching the
        // pattern `commands::create_library` previously used.
        if project_dir.join(format!("{name}.snxlib")).exists() {
            for n in 2..=99 {
                let candidate = format!("{stem}-lib-{n}");
                if !project_dir.join(format!("{candidate}.snxlib")).exists() {
                    name = candidate;
                    break;
                }
            }
        }
        let suggested_filename = format!("{name}.snxlib");
        let project_path = loaded.path.clone();

        Task::perform(
            async move {
                rfd::AsyncFileDialog::new()
                    .set_title("New Component Library")
                    .add_filter("Signex Component Library", &["snxlib"])
                    .set_directory(&project_dir)
                    .set_file_name(&suggested_filename)
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            move |picked| match picked {
                Some(lib_path) => Message::Library(LibraryMessage::CreateLibraryAtPath {
                    project_path: project_path.clone(),
                    lib_path,
                }),
                None => Message::Noop,
            },
        )
    }

    /// Resolution of the "Library Options" modal (Stage 11 of
    /// `v0.9-snxlib-as-file-plan.md`). Re-resolves the project (in
    /// case it was unloaded between modal spawn + confirm), then runs
    /// `commands::create_library_at` to do the disk init + manifest +
    /// git scaffolding + project registration. `use_lfs` carries the
    /// modal's checkbox state — when on, the adapter writes
    /// `.gitattributes` for `*.step` / `*.stp` / `*.wrl` / `*.iges`
    /// and stages it into the initial commit.
    fn handle_create_library_at_path(
        &mut self,
        project_path: std::path::PathBuf,
        lib_path: std::path::PathBuf,
        use_lfs: bool,
    ) -> Task<Message> {
        let Some(loaded) = self
            .document_state
            .projects
            .iter_mut()
            .find(|p| p.path == project_path)
        else {
            tracing::warn!(
                target: "signex::library",
                path = %project_path.display(),
                "create library: project unloaded between dialog spawn and confirm"
            );
            return Task::none();
        };

        match crate::library::commands::create_library_at(
            &mut self.library,
            &mut loaded.data,
            lib_path.clone(),
            use_lfs,
        ) {
            Ok(library_id) => {
                tracing::info!(
                    target: "signex::library",
                    project = %loaded.path.display(),
                    library = %lib_path.display(),
                    library_id = %library_id,
                    use_lfs,
                    "created library via save-as dialog"
                );
                // Mark the project file dirty so the user is prompted
                // to persist the new library entry in the `.snxprj`.
                let project_path = loaded.path.clone();
                self.document_state.dirty_paths.insert(project_path);
            }
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    project = %loaded.path.display(),
                    library = %lib_path.display(),
                    use_lfs,
                    error = %e,
                    "create_library_at failed"
                );
            }
        }

        self.refresh_panel_ctx();
        Task::none()
    }

    /// Open the Component Preview tab for `(library_path, table, row_id)`.
    /// Re-uses the existing tab if one is already open.
    fn handle_open_component_row(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        let address = EditorAddress::new(library_path.clone(), table.clone(), row_id);
        let synthetic_path = address.synthetic_tab_path();

        if let Some(idx) = self
            .document_state
            .tabs
            .iter()
            .position(|t| t.path == synthetic_path)
        {
            if idx != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = idx;
                self.sync_active_tab();
            }
            return Task::none();
        }

        // Pre-load the row from the adapter via `read_row`; if it
        // fails we surface and bail without leaving an empty tab
        // behind.
        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    "open component row: library not open"
                );
                return Task::none();
            }
        };
        let row_result = self
            .library
            .set
            .get(library_id)
            .ok_or_else(|| {
                signex_library::LibraryError::NotFound(library_path.display().to_string())
            })
            .and_then(|adapter| adapter.read_row(&table, row_id));
        let row = match row_result {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(target: "signex::library", error = %e, "open component row: read_row failed");
                return Task::none();
            }
        };

        let title = row.internal_pn.as_str().to_string();
        let project_id = self
            .document_state
            .project_for_path(&synthetic_path)
            .map(|p| p.id);
        let preview = ComponentPreviewState::from_row(library_path.clone(), table.clone(), row);
        self.library.editors.insert(address.clone(), preview);
        self.park_active_schematic_session();
        self.document_state.tabs.push(crate::app::TabInfo {
            title,
            path: synthetic_path,
            cached_document: None,
            dirty: false,
            project_id,
            kind: crate::app::TabKind::ComponentEditor(crate::app::ComponentEditorTab {
                library_path: address.library_path.clone(),
                table: address.table.clone(),
                row_id: address.row_id,
            }),
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        self.document_state.active_path = None;
        self.refresh_panel_ctx();
        Task::none()
    }

    fn handle_picker_message(&mut self, msg: PickerMsg) -> Task<Message> {
        let Some(picker) = self.library.picker.as_mut() else {
            return Task::none();
        };
        match msg {
            PickerMsg::FilterChanged(s) => {
                picker.filter = s;
            }
            PickerMsg::SelectComponent(summary) => {
                // `ComponentSummary` carries `row_id` directly in the
                // DBLib model; match against that.
                let path = self
                    .library
                    .open_libraries
                    .iter()
                    .find(|lib| {
                        lib.cached_components
                            .iter()
                            .any(|c| c.row_id == summary.row_id)
                    })
                    .map(|lib| lib.root.clone());
                picker.selected = path.map(|p| (p, summary));
            }
            PickerMsg::PlaceSelected => {
                if let Some((path, summary)) = picker.selected.clone() {
                    tracing::warn!(
                        target: "signex::library",
                        library = %path.display(),
                        internal_pn = %summary.internal_pn.as_str(),
                        "place flow shipped in Phase 2 — picker dismissed"
                    );
                }
                self.library.picker = None;
            }
        }
        Task::none()
    }

    fn handle_library_settings_message(&mut self, msg: SettingsMsg) -> Task<Message> {
        use crate::library::settings::{digikey_oauth, persistence};
        use signex_library::distributor::DistributorAdapter;
        use signex_library::distributors::digikey::{DIGIKEY_AUTH_URL, DIGIKEY_TOKEN_URL};
        use signex_library::distributors::keyring::KeyringStore;
        use signex_library::distributors::mouser::MouserAdapter;

        match msg {
            SettingsMsg::DigiKeyConnect => {
                if self.library.settings.digikey_in_flight {
                    return Task::none();
                }
                self.library.settings.digikey_in_flight = true;
                self.library.settings.digikey_status = Some("Waiting for browser…".to_string());
                let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                self.library.settings.digikey_cancel = Some(cancel_flag.clone());
                let (client_id, client_secret) = digikey_oauth::read_env_credentials();
                let auth_url = DIGIKEY_AUTH_URL.to_string();
                let token_url = DIGIKEY_TOKEN_URL.to_string();
                return Task::perform(
                    async move {
                        let cancel = digikey_oauth::CancelHandle::from_flag(cancel_flag);
                        tokio::task::spawn_blocking(move || {
                            digikey_oauth::run_blocking(
                                client_id,
                                client_secret,
                                auth_url,
                                token_url,
                                cancel,
                                true,
                            )
                        })
                        .await
                        .unwrap_or(digikey_oauth::Outcome::Failed {
                            reason: "worker thread panicked".into(),
                        })
                    },
                    |outcome| {
                        let (label, err) = match outcome {
                            digikey_oauth::Outcome::Connected { account_label } => {
                                (Some(account_label), None)
                            }
                            digikey_oauth::Outcome::Failed { reason } => (None, Some(reason)),
                            digikey_oauth::Outcome::Cancelled => (None, None),
                        };
                        Message::Library(LibraryMessage::Settings(
                            SettingsMsg::DigiKeyOAuthResult {
                                connected_label: label,
                                error: err,
                            },
                        ))
                    },
                );
            }
            SettingsMsg::DigiKeyCancel => {
                if let Some(flag) = self.library.settings.digikey_cancel.as_ref() {
                    flag.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                self.library.settings.digikey_cancel = None;
                self.library.settings.digikey_in_flight = false;
                self.library.settings.digikey_status = Some("Cancelled".to_string());
            }
            SettingsMsg::DigiKeyOAuthResult {
                connected_label,
                error,
            } => {
                self.library.settings.digikey_in_flight = false;
                self.library.settings.digikey_cancel = None;
                match (connected_label, error) {
                    (Some(label), _) => {
                        self.library.settings.digikey_account_email = Some(label.clone());
                        self.library.settings.digikey_status =
                            Some(format!("Connected as {label}"));
                    }
                    (_, Some(reason)) => {
                        self.library.settings.digikey_status = Some(format!("Failed: {reason}"));
                    }
                    (None, None) => {
                        self.library.settings.digikey_status = Some("Cancelled".to_string());
                    }
                }
            }
            SettingsMsg::MouserApiKeyChanged(s) => {
                self.library.settings.mouser_api_key_buf = s;
            }
            SettingsMsg::MouserTest => {
                if self.library.settings.mouser_in_flight {
                    return Task::none();
                }
                let key = self.library.settings.mouser_api_key_buf.clone();
                if key.is_empty() {
                    self.library.settings.mouser_status =
                        Some("Cannot test — paste an API key first.".to_string());
                    return Task::none();
                }
                self.library.settings.mouser_in_flight = true;
                self.library.settings.mouser_status = Some("Testing…".to_string());
                let key_for_writeback = key.clone();
                return Task::perform(
                    async move {
                        let key_for_test = key.clone();
                        tokio::task::spawn_blocking(move || {
                            const SENTINEL_MPN: &str = "RC0805FR-0710KL";
                            let adapter = MouserAdapter::with_api_key(
                                "https://api.mouser.com/api/v1/search/keyword",
                                key_for_test,
                                None,
                            );
                            adapter
                                .lookup_by_mpn(SENTINEL_MPN)
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("worker thread panicked: {e}")))
                    },
                    move |result| {
                        let result = match result {
                            Ok(()) => {
                                let store = KeyringStore::for_provider("mouser", "default");
                                if let Err(e) = store.set_secret(&key_for_writeback) {
                                    Err(format!("API key valid, but keyring write failed: {e}"))
                                } else {
                                    Ok(())
                                }
                            }
                            Err(e) => Err(e),
                        };
                        Message::Library(LibraryMessage::Settings(SettingsMsg::MouserTestResult(
                            result,
                        )))
                    },
                );
            }
            SettingsMsg::MouserTestResult(result) => {
                self.library.settings.mouser_in_flight = false;
                self.library.settings.mouser_status = Some(match result {
                    Ok(()) => "\u{2713} Connected & key saved to keyring.".to_string(),
                    Err(e) => format!("Failed: {e}"),
                });
            }
            SettingsMsg::PreferenceUp(src) => {
                let order = &mut self.library.settings.preferred_order;
                if let Some(i) = order.iter().position(|s| *s == src)
                    && i > 0
                {
                    order.swap(i, i - 1);
                    persistence::save_preferred_order(order);
                }
            }
            SettingsMsg::PreferenceDown(src) => {
                let order = &mut self.library.settings.preferred_order;
                if let Some(i) = order.iter().position(|s| *s == src)
                    && i + 1 < order.len()
                {
                    order.swap(i, i + 1);
                    persistence::save_preferred_order(order);
                }
            }
        }
        Task::none()
    }

    /// Component Preview event handler.
    fn handle_editor_event(&mut self, address: EditorAddress, msg: EditorMsg) -> Task<Message> {
        match msg {
            EditorMsg::CloseEditor => {
                let synthetic = address.synthetic_tab_path();
                if let Some(idx) = self
                    .document_state
                    .tabs
                    .iter()
                    .position(|t| t.path == synthetic)
                {
                    return self.close_tab_now(idx);
                }
                self.library.editors.remove(&address);
                return Task::none();
            }
            EditorMsg::SaveDraft | EditorMsg::Commit => {
                self.handle_save_row(&address);
                return Task::none();
            }
            EditorMsg::SelectTab(tab) => {
                self.handle_select_preview_tab(&address, tab);
                return Task::none();
            }
            EditorMsg::OpenWhereUsedTab => {
                if let Some(state) = self.library.editors.get_mut(&address) {
                    state.active_tab = PreviewTab::Preview;
                }
                return Task::none();
            }
            // Submit-for-review is dropped from the Component Preview
            // surface in v0.9-refactor-2 — review workflows happen
            // outside the row context. The variants stay in the message
            // tree for potential future revival.
            EditorMsg::SubmitForReview
            | EditorMsg::SubmitForReviewNotesChanged(_)
            | EditorMsg::SubmitForReviewCancel
            | EditorMsg::SubmitForReviewConfirm
            | EditorMsg::SubmitForReviewResult(_) => {
                tracing::debug!(
                    target: "signex::library",
                    "submit-for-review is not wired in the Component Preview surface"
                );
                return Task::none();
            }
            EditorMsg::DatasheetUploadDialog => {
                let library_path = address.library_path.clone();
                let table = address.table.clone();
                let row_id = address.row_id;
                return Task::perform(
                    async {
                        let picked = rfd::AsyncFileDialog::new()
                            .set_title("Pin datasheet PDF")
                            .add_filter("PDF", &["pdf"])
                            .add_filter("All files", &["*"])
                            .pick_file()
                            .await;
                        match picked {
                            Some(handle) => {
                                let filename = handle.file_name();
                                let bytes = handle.read().await;
                                Some((bytes, filename))
                            }
                            None => None,
                        }
                    },
                    move |result| {
                        Message::Library(LibraryMessage::EditorEvent {
                            library_path: library_path.clone(),
                            table: table.clone(),
                            row_id,
                            msg: EditorMsg::DatasheetUploadResult(result),
                        })
                    },
                );
            }
            _ => {}
        }

        let Some(state) = self.library.editors.get_mut(&address) else {
            return Task::none();
        };
        apply_inline_edit(state, msg);
        Task::none()
    }

    fn handle_select_preview_tab(&mut self, address: &EditorAddress, tab: PreviewTab) {
        let Some(state) = self.library.editors.get_mut(address) else {
            return;
        };
        state.active_tab = tab;

        match tab {
            PreviewTab::Preview => {
                if state.symbol.is_none() {
                    let symbol_ref = state.row.symbol_ref;
                    let resolved = self.library.set.resolve_symbol(&symbol_ref);
                    if let Some(state) = self.library.editors.get_mut(address) {
                        state.symbol = resolved;
                    }
                }
                if let Some(state) = self.library.editors.get_mut(address)
                    && state.footprint.is_none()
                {
                    let resolved = state
                        .row
                        .footprint_ref
                        .as_ref()
                        .and_then(|r| self.library.set.resolve_footprint(r));
                    if let Some(state) = self.library.editors.get_mut(address) {
                        state.footprint = resolved;
                    }
                }
            }
            PreviewTab::Simulation => {
                if state.sim.is_none()
                    && let Some(sim_ref) = state.row.sim_ref.as_ref()
                {
                    let sim_ref = *sim_ref;
                    let resolved = self.library.set.resolve_sim(&sim_ref);
                    if let Some(state) = self.library.editors.get_mut(address) {
                        if let Some(sim) = resolved.as_ref() {
                            state.sim_body = Some(iced::widget::text_editor::Content::with_text(
                                sim.body.as_str(),
                            ));
                        }
                        state.sim = resolved;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_save_row(&mut self, address: &EditorAddress) {
        let Some(state) = self.library.editors.get(address) else {
            return;
        };
        let library_path = state.library_path.clone();
        let table = state.table.clone();
        let mut row = state.row.clone();
        row.updated = chrono::Utc::now();
        if let Err(e) = row.refresh_content_hash() {
            tracing::warn!(target: "signex::library", error = %e, "save row: refresh_content_hash failed");
        }

        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => {
                tracing::warn!(target: "signex::library", "save row: library not open");
                return;
            }
        };
        let adapter = match self.library.set.get(library_id) {
            Some(a) => a,
            None => {
                tracing::warn!(target: "signex::library", "save row: library not mounted");
                return;
            }
        };
        match adapter.update_row(&table, row.clone(), "edit row (signex-app)") {
            Ok(()) => {
                if let Some(state) = self.library.editors.get_mut(address) {
                    state.row = row;
                    state.dirty = false;
                }
                if let Err(e) = self.library.refresh_components(&library_path) {
                    tracing::warn!(target: "signex::library", error = %e, "post-save refresh failed");
                }
            }
            Err(e) => {
                tracing::warn!(target: "signex::library", error = %e, "update_row failed");
            }
        }
    }

    /// Open a `.snxsym` or `.snxfpt` as a main-window document tab.
    /// Reads the file from disk, builds the matching editor state,
    /// and pushes a `TabKind::SymbolEditor(path)` /
    /// `FootprintEditor(path)` tab into `DocumentState.tabs`.
    ///
    /// Activates an existing tab when the same path is already open
    /// instead of duplicating; surfaces parse / IO failures via
    /// `tracing::warn` (and silently bails — leaving the tab bar
    /// untouched).
    pub(crate) fn handle_open_primitive(&mut self, path: std::path::PathBuf) -> Task<Message> {
        // Already open? Just activate the existing tab.
        if let Some(idx) = self.document_state.tabs.iter().position(|t| t.path == path) {
            if idx != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = idx;
                self.sync_active_tab();
            }
            return Task::none();
        }

        // Dispatch on extension. `.snxsym` → Symbol; `.snxfpt` →
        // Footprint. Anything else is rejected with a tracing warn so
        // a stray dispatch from the project tree doesn't push a
        // bogus tab.
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match ext.as_str() {
            "snxsym" => {
                let bytes = match std::fs::read(&path) {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            error = %e,
                            "open primitive: read .snxsym failed",
                        );
                        return Task::none();
                    }
                };
                let file = match signex_library::SymbolFile::from_json(&bytes) {
                    Ok(f) if !f.symbols.is_empty() => f,
                    Ok(_) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            "open primitive: .snxsym contains zero symbols",
                        );
                        return Task::none();
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            error = %e,
                            "open primitive: parse .snxsym failed",
                        );
                        return Task::none();
                    }
                };

                let title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| {
                        if !file.display_name.is_empty() {
                            file.display_name.clone()
                        } else {
                            file.symbols[0].name.clone()
                        }
                    });
                let project_id = self.document_state.project_for_path(&path).map(|p| p.id);

                let state = crate::app::SymbolEditorState::new(path.clone(), file);
                self.document_state
                    .symbol_editors
                    .insert(path.clone(), state);

                self.park_active_schematic_session();
                self.document_state.tabs.push(crate::app::TabInfo {
                    title,
                    path: path.clone(),
                    cached_document: None,
                    dirty: false,
                    project_id,
                    kind: crate::app::TabKind::SymbolEditor(path),
                });
                self.document_state.active_tab = self.document_state.tabs.len() - 1;
                // Standalone primitive tabs don't drive `active_path`
                // — clear so the canvas doesn't render a stale schematic.
                self.document_state.active_path = None;
                self.refresh_panel_ctx();
                Task::none()
            }
            "snxfpt" => {
                let bytes = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            error = %e,
                            "open primitive: read .snxfpt failed",
                        );
                        return Task::none();
                    }
                };
                let footprint: signex_library::Footprint = match serde_json::from_str(&bytes) {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            error = %e,
                            "open primitive: parse .snxfpt failed",
                        );
                        return Task::none();
                    }
                };

                let title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| footprint.name.clone());
                let project_id = self.document_state.project_for_path(&path).map(|p| p.id);

                let state = crate::app::FootprintEditorState::new(path.clone(), footprint);
                self.document_state
                    .footprint_editors
                    .insert(path.clone(), state);

                self.park_active_schematic_session();
                self.document_state.tabs.push(crate::app::TabInfo {
                    title,
                    path: path.clone(),
                    cached_document: None,
                    dirty: false,
                    project_id,
                    kind: crate::app::TabKind::FootprintEditor(path),
                });
                self.document_state.active_tab = self.document_state.tabs.len() - 1;
                self.document_state.active_path = None;
                self.refresh_panel_ctx();
                Task::none()
            }
            other => {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    ext = %other,
                    "open primitive: unsupported extension",
                );
                Task::none()
            }
        }
    }

    /// Apply a primitive-editor inner message to the matching tab's
    /// editor state. Path-keyed lookup distinguishes Symbol vs
    /// Footprint; the dispatcher routes to the existing canvas-state
    /// helpers so the standalone tab behaviour matches the in-Component
    /// Editor experience verbatim.
    pub(crate) fn handle_primitive_editor_event(
        &mut self,
        path: std::path::PathBuf,
        msg: PrimitiveEditorMsg,
    ) -> Task<Message> {
        // Save is a sibling of the canvas-mutation messages — route
        // through the standalone save path which writes JSON back to
        // disk and (when applicable) reloads in the LibrarySet. When
        // the file doesn't exist on disk yet (newly-minted in-memory
        // tab from `Add New ▸ Symbol` / `Add New ▸ Footprint`), spawn
        // the Save-As dialog instead so the user picks where it lands
        // — same gate as the top-level `Message::SaveFile` path uses.
        if matches!(msg, PrimitiveEditorMsg::Save) {
            if !path.exists() {
                return crate::app::handlers::document_files::spawn_save_as_for_new_primitive(
                    path,
                );
            }
            self.save_primitive_tab_at(&path);
            return Task::none();
        }

        // Per-library display settings (sheet color, grid, unit)
        // mutate `OpenLibrary.display` rather than the per-tab editor
        // state — every primitive editor opened from the same
        // `.snxlib` shares the same view settings (Altium "Document
        // Options" parity). Run these before the editor-level
        // dispatch so the editor closure doesn't see them.
        match &msg {
            PrimitiveEditorMsg::SymbolSetSheetColor(color) => {
                let color = *color;
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    lib.display.sheet_color = color;
                }
                self.invalidate_primitive_canvas_cache(&path);
                return Task::none();
            }
            PrimitiveEditorMsg::SymbolToggleGrid => {
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    lib.display.grid_visible = !lib.display.grid_visible;
                }
                self.invalidate_primitive_canvas_cache(&path);
                return Task::none();
            }
            PrimitiveEditorMsg::SymbolCycleGridSize => {
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    let sizes = crate::canvas::grid::GRID_SIZES_MM;
                    let current_idx = sizes
                        .iter()
                        .position(|s| (s - lib.display.grid_size_mm).abs() < f32::EPSILON)
                        .unwrap_or(2);
                    let next_idx = (current_idx + 1) % sizes.len();
                    lib.display.grid_size_mm = sizes[next_idx];
                }
                self.invalidate_primitive_canvas_cache(&path);
                return Task::none();
            }
            PrimitiveEditorMsg::SymbolCycleUnit => {
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    use signex_types::coord::Unit;
                    lib.display.unit = match lib.display.unit {
                        Unit::Mm => Unit::Mil,
                        Unit::Mil => Unit::Inch,
                        Unit::Inch => Unit::Micrometer,
                        Unit::Micrometer => Unit::Mm,
                    };
                }
                // Unit only affects the status footer text — no
                // canvas redraw needed, but cache clear is harmless
                // and keeps the message handling shape consistent.
                return Task::none();
            }
            _ => {}
        }

        // Symbol-only mutations.
        if let Some(editor) = self.document_state.symbol_editors.get_mut(&path) {
            apply_symbol_primitive_edit(editor, msg);
            return Task::none();
        }

        // Footprint-only mutations.
        if let Some(editor) = self.document_state.footprint_editors.get_mut(&path) {
            apply_footprint_primitive_edit(editor, msg);
            return Task::none();
        }

        tracing::warn!(
            target: "signex::library",
            path = %path.display(),
            "primitive editor event: no matching tab state",
        );
        Task::none()
    }

    /// Clear the canvas cache for the primitive editor tab keyed by
    /// `path`. Used by the per-library display-settings handlers so
    /// the visible canvas redraws as soon as the user flips bg /
    /// grid / etc.
    fn invalidate_primitive_canvas_cache(&mut self, path: &std::path::Path) {
        if let Some(editor) = self.document_state.symbol_editors.get_mut(path) {
            editor.canvas_cache.clear();
        }
        if let Some(editor) = self.document_state.footprint_editors.get_mut(path) {
            editor.canvas_cache.clear();
        }
    }

    /// Write the primitive at `path` back to disk as JSON, commit
    /// through the matching adapter (when the file lives under a
    /// mounted `.snxlib/`), mark the tab clean, and ask the
    /// `LibrarySet` to reload its cached copy so any open Component
    /// Preview tabs see the new bytes.
    pub(crate) fn save_primitive_tab_at(&mut self, path: &std::path::Path) {
        // Symbol path — write the full multi-symbol container back to
        // disk so other symbols in the same file are preserved.
        if let Some(editor) = self.document_state.symbol_editors.get_mut(path) {
            // Refresh the active symbol's + the file's updated
            // timestamps so downstream consumers can detect the rewrite.
            let now = chrono::Utc::now();
            editor.primitive_mut().updated = now;
            editor.file.updated = now;
            let json = match serde_json::to_string_pretty(&editor.file) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        path = %path.display(),
                        error = %e,
                        "save primitive: serialize symbol file failed",
                    );
                    return;
                }
            };
            if let Err(e) = atomic_write(path, json.as_bytes()) {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "save primitive: write .snxsym failed",
                );
                return;
            }
            // Capture the symbol name for the commit message before
            // dropping the editor borrow.
            let sym_name = editor.primitive().name.clone();
            editor.dirty = false;
            // Clear the project-scoped dirty marker if any callers
            // had set it.
            self.document_state.dirty_paths.remove(path);
            // Clear the matching tab's dirty flag too.
            if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
                tab.dirty = false;
            }
            // Commit through the matching adapter so the edit lands
            // in git history. No-op when the file lives outside any
            // mounted library (lone-file edit) or when the adapter
            // has no version control (database backend).
            self.commit_external_change_for(path, &format!("save symbol {sym_name}"));
            // Refresh the matching library's primitive cache so the
            // picker modal picks up the new symbol immediately.
            self.refresh_primitive_cache_for(path);
            // Best-effort LibrarySet reload so Component Preview
            // tabs that already cached the primitive see the new bytes.
            self.reload_primitive_in_library_set(path);
            return;
        }

        // Footprint path.
        if let Some(editor) = self.document_state.footprint_editors.get_mut(path) {
            // Sync the canvas-mirrored pad list back into the
            // primitive before serialising — `state.pads` is
            // authoritative on the editor side; without this, in-
            // editor pad edits wouldn't persist.
            crate::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(
                &editor.state,
                &mut editor.primitive,
            );
            editor.primitive.updated = chrono::Utc::now();
            let json = match serde_json::to_string_pretty(&editor.primitive) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        path = %path.display(),
                        error = %e,
                        "save primitive: serialize footprint failed",
                    );
                    return;
                }
            };
            if let Err(e) = atomic_write(path, json.as_bytes()) {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "save primitive: write .snxfpt failed",
                );
                return;
            }
            let fp_name = editor.primitive.name.clone();
            editor.dirty = false;
            self.document_state.dirty_paths.remove(path);
            if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
                tab.dirty = false;
            }
            self.commit_external_change_for(path, &format!("save footprint {fp_name}"));
            self.refresh_primitive_cache_for(path);
            self.reload_primitive_in_library_set(path);
        }
    }

    /// Find the open library whose root contains `path`, then ask its
    /// adapter to stage + commit. Best-effort: silently returns when
    /// no mounted library covers `path` (lone-file edit) or when the
    /// commit itself fails (warning is emitted via tracing). Never
    /// blocks the user — the file write already succeeded.
    fn commit_external_change_for(&self, path: &std::path::Path, message: &str) {
        // Find the open library whose working dir is an ancestor of
        // `path`. `lib.root` is the `.snxlib` *file* path now, so we
        // walk against its parent directory (where `symbols/` and
        // `footprints/` actually live).
        let lib = self
            .library
            .open_libraries
            .iter()
            .find(|lib| {
                lib.root_dir()
                    .map(|d| path.starts_with(d))
                    .unwrap_or(false)
            });
        let Some(lib) = lib else {
            return;
        };
        let Some(adapter) = self.library.set.get(lib.library_id) else {
            return;
        };
        if let Err(e) = adapter.commit_external_change(path, message) {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %e,
                "save primitive: commit_external_change failed (file written; commit deferred)",
            );
        }
    }

    /// Refresh the matching library's per-kind primitive cache so the
    /// picker modal sees the just-saved primitive without waiting
    /// for the next full `refresh_components` round-trip. No-op when
    /// `path` lives outside any mounted library.
    fn refresh_primitive_cache_for(&mut self, path: &std::path::Path) {
        // Same `root_dir()` ancestor walk as
        // `commit_external_change_for` — `lib.root` is the `.snxlib`
        // file, the on-disk children sit under its parent dir.
        let library_id = match self
            .library
            .open_libraries
            .iter()
            .find(|lib| {
                lib.root_dir()
                    .map(|d| path.starts_with(d))
                    .unwrap_or(false)
            }) {
            Some(lib) => lib.library_id,
            None => return,
        };
        // Two-step borrow dance: snapshot the listings through the
        // mounted adapter, then move them onto the OpenLibrary entry.
        let (symbols, footprints, sims) = match self.library.set.get(library_id) {
            Some(adapter) => (
                adapter.list_symbols().unwrap_or_default(),
                adapter.list_footprints().unwrap_or_default(),
                adapter.list_sims().unwrap_or_default(),
            ),
            None => return,
        };
        if let Some(lib) = self
            .library
            .open_libraries
            .iter_mut()
            .find(|lib| lib.library_id == library_id)
        {
            lib.cached_symbols = symbols;
            lib.cached_footprints = footprints;
            lib.cached_sims = sims;
        }
    }

    /// Walk the open libraries to find one whose root contains
    /// `path` (e.g. `…/mylib.snxlib/symbols/foo.snxsym` lives under
    /// `…/mylib.snxlib/`), and ask the matching adapter to reload
    /// the primitive UUID encoded in the file. The adapter's
    /// `reload_primitive` (where supported) repopulates its in-memory
    /// cache so any Component Preview tabs that resolve through
    /// `LibrarySet` see the new bytes on the next render.
    ///
    /// Best-effort — returns silently when the path isn't under a
    /// mounted library or when the adapter has no reload hook.
    fn reload_primitive_in_library_set(&mut self, _path: &std::path::Path) {
        // Stubbed pending the corresponding `LibrarySet::reload_primitive`
        // helper. The standalone editor tab already holds the
        // authoritative copy of the primitive in memory and on-disk
        // round-trips happen here; Component Preview tabs pull
        // through `LibrarySet::resolve_*` on the next view, so the
        // only hole this leaves is a Preview tab that has already
        // resolved + cached its primitive in editor state.
    }
}

/// Apply a primitive-editor event to a standalone Symbol editor
/// state. Mirrors the symbol-tab arms of `apply_inline_edit` but
/// against the path-keyed standalone state. Visibility is
/// `pub(crate)` so unit tests in sibling modules can drive the editor
/// through the same code path the dispatcher uses.
pub(crate) fn apply_symbol_primitive_edit(
    editor: &mut crate::app::SymbolEditorState,
    msg: PrimitiveEditorMsg,
) {
    use crate::library::editor::symbol::canvas::SymbolTool;
    use crate::library::editor::symbol::state::{FieldKey, SymbolSelection};

    match msg {
        PrimitiveEditorMsg::SymbolSetTool(tool) => {
            editor.tool = match tool {
                SymbolToolMsg::Select => SymbolTool::Select,
                SymbolToolMsg::AddPin => SymbolTool::AddPin,
                SymbolToolMsg::PlaceRectangle => SymbolTool::PlaceRectangle,
                SymbolToolMsg::PlaceLine => SymbolTool::PlaceLine,
                SymbolToolMsg::PlaceCircle => SymbolTool::PlaceCircle,
                SymbolToolMsg::PlaceArc => SymbolTool::PlaceArc,
                SymbolToolMsg::PlaceText => SymbolTool::PlaceText,
            };
        }
        PrimitiveEditorMsg::SymbolAddPin { x, y } => {
            let active_part = editor.active_part;
            let idx = crate::library::editor::symbol::state::add_pin(
                editor.primitive_mut(),
                x,
                y,
                active_part,
            );
            editor.selected = Some(SymbolSelection::Pin(idx));
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolAddRectangle { x, y } => {
            // Default 10×5 mm rectangle centred on the click. User
            // edits the corners later via Properties (graphics-properties
            // surface lands in a follow-up; for now they can move/delete
            // through the Select tool).
            const W: f64 = 5.08; // half-width 5.08 mm → 10.16 mm overall
            const H: f64 = 2.54; // half-height
            editor
                .primitive_mut()
                .graphics
                .push(signex_library::SymbolGraphic {
                    kind: signex_library::SymbolGraphicKind::Rectangle {
                        from: [x - W, y - H],
                        to: [x + W, y + H],
                    },
                    stroke_width: 0.15,
                });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolAddLine { x, y } => {
            // 5 mm horizontal line going right.
            const L: f64 = 5.08;
            editor
                .primitive_mut()
                .graphics
                .push(signex_library::SymbolGraphic {
                    kind: signex_library::SymbolGraphicKind::Line {
                        from: [x, y],
                        to: [x + L, y],
                    },
                    stroke_width: 0.15,
                });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolAddArc { x, y } => {
            // Default 2 mm-radius arc, 0°→90° quadrant centred on
            // the click. User edits start/end angle via Properties
            // (or drag-to-resize the start/end handles).
            editor
                .primitive_mut()
                .graphics
                .push(signex_library::SymbolGraphic {
                    kind: signex_library::SymbolGraphicKind::Arc {
                        center: [x, y],
                        radius: 2.0,
                        start_deg: 0.0,
                        end_deg: 90.0,
                    },
                    stroke_width: 0.15,
                });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolAddText { x, y } => {
            // Default "Text" label at the click position. User edits
            // the content + size via Properties.
            editor
                .primitive_mut()
                .graphics
                .push(signex_library::SymbolGraphic {
                    kind: signex_library::SymbolGraphicKind::Text {
                        position: [x, y],
                        content: "Text".to_string(),
                        size: 1.27,
                    },
                    stroke_width: 0.0,
                });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolAddCircle { x, y } => {
            // 2 mm-radius circle centred on the click.
            editor
                .primitive_mut()
                .graphics
                .push(signex_library::SymbolGraphic {
                    kind: signex_library::SymbolGraphicKind::Circle {
                        center: [x, y],
                        radius: 2.0,
                    },
                    stroke_width: 0.15,
                });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolSelect(sel) => {
            editor.selected = Some(match sel {
                SymbolSelectionMsg::Pin(idx) => SymbolSelection::Pin(idx),
                SymbolSelectionMsg::FieldReference => SymbolSelection::Field(FieldKey::Reference),
                SymbolSelectionMsg::FieldValue => SymbolSelection::Field(FieldKey::Value),
                SymbolSelectionMsg::Graphic(idx) => SymbolSelection::Graphic(idx),
            });
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolDeselect => {
            editor.selected = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolMoveSelected { x, y } => {
            let selected = editor.selected;
            crate::library::editor::symbol::state::move_selected(
                editor.primitive_mut(),
                selected,
                x,
                y,
            );
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolMoveGraphicHandle { idx, handle, x, y } => {
            let h = graphic_handle_msg_to_state(handle);
            crate::library::editor::symbol::state::move_graphic_handle(
                editor.primitive_mut(),
                idx,
                h,
                x,
                y,
            );
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolDeleteSelected => {
            let selected = editor.selected;
            if let Some(new_sel) = crate::library::editor::symbol::state::delete_selected(
                editor.primitive_mut(),
                selected,
            ) {
                editor.selected = new_sel;
                editor.dirty = true;
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::SymbolSetPinNumber { idx, number } => {
            if let Some(pin) = editor.primitive_mut().pins.get_mut(idx) {
                pin.number = number;
                editor.dirty = true;
            }
        }
        PrimitiveEditorMsg::SymbolSetPinName { idx, name } => {
            if let Some(pin) = editor.primitive_mut().pins.get_mut(idx) {
                pin.name = name;
                editor.dirty = true;
            }
        }
        // ── View / camera ────────────────────────────────────────
        PrimitiveEditorMsg::SymbolPan { dx, dy } => {
            editor.camera.pan(dx, dy);
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolZoom { sx, sy, delta } => {
            // Wheel events feed `delta`; the camera applies its own
            // ZOOM_FACTOR + clamp inside `zoom_at`.
            let viewport = iced::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            };
            if editor
                .camera
                .zoom_at(iced::Point::new(sx, sy), delta, viewport)
            {
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::SymbolFit => {
            // Compute the symbol bbox using the canvas helper, then
            // ask the camera to fit it. We don't have the actual
            // viewport size here — use a sensible default so the
            // first Fit recovers from any pan/zoom; the user can
            // press Home again after resizing the window for a
            // tighter fit.
            let (min_x, min_y, max_x, max_y) = symbol_bbox(editor.primitive());
            let world_rect = iced::Rectangle {
                x: min_x as f32,
                y: -(max_y as f32), // Standard y-up → screen y-down
                width: (max_x - min_x).max(1.0) as f32,
                height: (max_y - min_y).max(1.0) as f32,
            };
            let viewport = iced::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 500.0,
            };
            editor.camera.fit_rect(world_rect, viewport);
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolCursorAt { x_mm, y_mm } => {
            editor.cursor_mm = match (x_mm, y_mm) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            };
        }
        // SymbolSetSheetColor / SymbolToggleGrid / SymbolCycleGridSize
        // / SymbolCycleUnit are intercepted in handle_primitive_editor_event
        // before this match runs — they mutate `OpenLibrary.display`,
        // not the per-tab editor state. List them here so the
        // dispatcher stays exhaustive across the enum (and matches
        // the footprint catch-all).
        PrimitiveEditorMsg::SymbolSetSheetColor(_)
        | PrimitiveEditorMsg::SymbolToggleGrid
        | PrimitiveEditorMsg::SymbolCycleGridSize
        | PrimitiveEditorMsg::SymbolCycleUnit => {}
        // ── Multi-part component ────────────────────────────────
        PrimitiveEditorMsg::SymbolPrevPart => {
            if editor.active_part > 1 {
                editor.active_part -= 1;
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::SymbolNextPart => {
            let max = crate::library::editor::symbol::state::max_part_number(editor.primitive());
            if editor.active_part < max {
                editor.active_part += 1;
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::SymbolNewPart => {
            // Bump the symbol's max declared part by one and switch
            // to it. The new part starts pinless; the user adds pins
            // in Add Pin mode with the new active_part selected.
            let new_part =
                crate::library::editor::symbol::state::max_part_number(editor.primitive()) + 1;
            editor.active_part = new_part;
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolRemovePart => {
            // Refuse to remove if this is the only part — a single-
            // part symbol must always have part 1 active.
            let max = crate::library::editor::symbol::state::max_part_number(editor.primitive());
            if max <= 1 || editor.active_part <= 1 {
                // No-op; surface a tracing line so a user-visible
                // toast can land later if needed.
                tracing::debug!(
                    target: "signex::library",
                    active = editor.active_part,
                    max,
                    "SymbolRemovePart: refusing to remove the only part"
                );
            } else {
                let to_remove = editor.active_part;
                crate::library::editor::symbol::state::demote_part_pins_to_part_one(
                    editor.primitive_mut(),
                    to_remove,
                );
                editor.active_part = 1;
                editor.dirty = true;
                editor.canvas_cache.clear();
            }
        }
        // Footprint variants are no-ops on a Symbol editor — the
        // dispatcher uses path-keyed lookup so a misrouted event
        // can't actually reach this match arm in practice.
        PrimitiveEditorMsg::FootprintAddPad { .. }
        | PrimitiveEditorMsg::FootprintMovePad { .. }
        | PrimitiveEditorMsg::FootprintCursorAt { .. }
        | PrimitiveEditorMsg::FootprintSelectPad(_)
        | PrimitiveEditorMsg::FootprintDeleteSelected
        | PrimitiveEditorMsg::FootprintToggleLayer(_)
        | PrimitiveEditorMsg::FootprintToggleAutoFit
        | PrimitiveEditorMsg::Save => {}
    }
}

/// Atomic write — write `bytes` to `<path>.tmp` then `rename` over
/// `path`. A crash mid-write leaves either the original file intact
/// or `<path>.tmp` orphaned; the destination file is never half-
/// written. Used by the standalone primitive save path so the
/// `.snxsym` / `.snxfpt` container can't be corrupted by an
/// untimely crash.
fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.to_path_buf();
    let tmp_name = match tmp.file_name() {
        Some(name) => {
            let mut s = name.to_os_string();
            s.push(".tmp");
            s
        }
        None => return Err(std::io::Error::other("destination path has no file name")),
    };
    tmp.set_file_name(tmp_name);
    std::fs::write(&tmp, bytes)?;
    // Windows rename fails if the destination exists; remove it first.
    // (POSIX rename is atomic-replace by spec; no remove needed there
    // but the no-op when missing is harmless.)
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// World-space bbox covering the symbol's body + every pin + every
/// graphic. Used by `SymbolFit` so the dispatcher can compute a
/// `Camera::fit_rect` against the active symbol without reaching
/// into the canvas program. Matches the `SymbolCanvas::bbox` shape
/// (pad 5.08 mm around the body, 1.27 mm around every pin) so
/// click-Fit and Home key produce the same viewport.
fn symbol_bbox(sym: &signex_library::Symbol) -> (f64, f64, f64, f64) {
    use signex_library::SymbolGraphicKind;
    let mut min_x: f64 = -10.16;
    let mut min_y: f64 = -7.62;
    let mut max_x: f64 = 10.16;
    let mut max_y: f64 = 7.62;
    for g in &sym.graphics {
        if let SymbolGraphicKind::Rectangle { from, to } = &g.kind {
            min_x = min_x.min(from[0]).min(to[0]) - 5.08;
            min_y = min_y.min(from[1]).min(to[1]) - 5.08;
            max_x = max_x.max(from[0]).max(to[0]) + 5.08;
            max_y = max_y.max(from[1]).max(to[1]) + 5.08;
            break;
        }
    }
    for pin in &sym.pins {
        min_x = min_x.min(pin.position[0] - 1.27);
        min_y = min_y.min(pin.position[1] - 1.27);
        max_x = max_x.max(pin.position[0] + pin.length + 1.27);
        max_y = max_y.max(pin.position[1] + 1.27);
    }
    for g in &sym.graphics {
        match &g.kind {
            SymbolGraphicKind::Rectangle { from, to } | SymbolGraphicKind::Line { from, to } => {
                min_x = min_x.min(from[0]).min(to[0]);
                min_y = min_y.min(from[1]).min(to[1]);
                max_x = max_x.max(from[0]).max(to[0]);
                max_y = max_y.max(from[1]).max(to[1]);
            }
            SymbolGraphicKind::Circle { center, radius }
            | SymbolGraphicKind::Arc { center, radius, .. } => {
                min_x = min_x.min(center[0] - radius);
                min_y = min_y.min(center[1] - radius);
                max_x = max_x.max(center[0] + radius);
                max_y = max_y.max(center[1] + radius);
            }
            SymbolGraphicKind::Text { position, size, .. } => {
                min_x = min_x.min(position[0] - size);
                min_y = min_y.min(position[1] - size);
                max_x = max_x.max(position[0] + size);
                max_y = max_y.max(position[1] + size);
            }
        }
    }
    (min_x, min_y, max_x, max_y)
}

/// Translate the pure-data [`GraphicHandleMsg`] back into the
/// canvas-side [`crate::library::editor::symbol::state::GraphicHandle`].
fn graphic_handle_msg_to_state(
    msg: GraphicHandleMsg,
) -> crate::library::editor::symbol::state::GraphicHandle {
    use crate::library::editor::symbol::state::GraphicHandle;
    match msg {
        GraphicHandleMsg::RectCorner(c) => GraphicHandle::RectCorner(c),
        GraphicHandleMsg::LineEndpoint(e) => GraphicHandle::LineEndpoint(e),
        GraphicHandleMsg::CircleRadius => GraphicHandle::CircleRadius,
        GraphicHandleMsg::ArcStart => GraphicHandle::ArcStart,
        GraphicHandleMsg::ArcEnd => GraphicHandle::ArcEnd,
        GraphicHandleMsg::TextAnchor => GraphicHandle::TextAnchor,
    }
}

/// Apply a primitive-editor event to a standalone Footprint editor
/// state. Mirrors the footprint-tab arms of `apply_inline_edit` but
/// against the path-keyed standalone state.
pub(crate) fn apply_footprint_primitive_edit(
    editor: &mut crate::app::FootprintEditorState,
    msg: PrimitiveEditorMsg,
) {
    use crate::library::editor::footprint::layers::FpLayer;
    use crate::library::editor::footprint::state::FootprintEditorState as CanvasState;

    match msg {
        PrimitiveEditorMsg::FootprintAddPad { x_mm, y_mm } => {
            let _idx = editor.state.add_pad_at(x_mm, y_mm);
            CanvasState::sync_pads_to_primitive(&editor.state, &mut editor.primitive);
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintMovePad { idx, x_mm, y_mm } => {
            editor.state.move_pad(idx, x_mm, y_mm);
            CanvasState::sync_pads_to_primitive(&editor.state, &mut editor.primitive);
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintCursorAt { x_mm, y_mm } => {
            editor.state.cursor_mm = Some((x_mm, y_mm));
        }
        PrimitiveEditorMsg::FootprintSelectPad(sel) => {
            editor.state.selected_pad = sel;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintDeleteSelected => {
            if let Some(idx) = editor.state.selected_pad {
                editor.state.delete_pad(idx);
                CanvasState::sync_pads_to_primitive(&editor.state, &mut editor.primitive);
                editor.canvas_cache.clear();
                editor.dirty = true;
            }
        }
        PrimitiveEditorMsg::FootprintToggleLayer(name) => {
            if let Some(layer) = FpLayer::from_standard_name(&name) {
                editor.state.layer_visibility.toggle(layer);
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintToggleAutoFit => {
            editor.state.toggle_auto_fit();
            CanvasState::sync_pads_to_primitive(&editor.state, &mut editor.primitive);
            editor.canvas_cache.clear();
        }
        // Symbol variants are no-ops on a Footprint editor.
        PrimitiveEditorMsg::SymbolSetTool(_)
        | PrimitiveEditorMsg::SymbolAddPin { .. }
        | PrimitiveEditorMsg::SymbolAddRectangle { .. }
        | PrimitiveEditorMsg::SymbolAddLine { .. }
        | PrimitiveEditorMsg::SymbolAddCircle { .. }
        | PrimitiveEditorMsg::SymbolAddArc { .. }
        | PrimitiveEditorMsg::SymbolAddText { .. }
        | PrimitiveEditorMsg::SymbolSelect(_)
        | PrimitiveEditorMsg::SymbolDeselect
        | PrimitiveEditorMsg::SymbolMoveSelected { .. }
        | PrimitiveEditorMsg::SymbolMoveGraphicHandle { .. }
        | PrimitiveEditorMsg::SymbolDeleteSelected
        | PrimitiveEditorMsg::SymbolSetPinNumber { .. }
        | PrimitiveEditorMsg::SymbolSetPinName { .. }
        | PrimitiveEditorMsg::SymbolPrevPart
        | PrimitiveEditorMsg::SymbolNextPart
        | PrimitiveEditorMsg::SymbolNewPart
        | PrimitiveEditorMsg::SymbolRemovePart
        | PrimitiveEditorMsg::SymbolPan { .. }
        | PrimitiveEditorMsg::SymbolZoom { .. }
        | PrimitiveEditorMsg::SymbolFit
        | PrimitiveEditorMsg::SymbolCursorAt { .. }
        | PrimitiveEditorMsg::SymbolSetSheetColor(_)
        | PrimitiveEditorMsg::SymbolToggleGrid
        | PrimitiveEditorMsg::SymbolCycleGridSize
        | PrimitiveEditorMsg::SymbolCycleUnit
        | PrimitiveEditorMsg::Save => {}
    }
}

/// Apply inline-edit messages directly to a Component Preview state.
/// Tab switching, save, and async-bounce variants are handled before
/// reaching here — this is the catch-all for in-place row mutations
/// (parameters / supply / datasheet / pin-map / simulation).
pub(crate) fn apply_inline_edit(state: &mut ComponentPreviewState, msg: EditorMsg) {
    match msg {
        EditorMsg::SelectTab(tab) => state.active_tab = tab,
        // Component-level setters
        EditorMsg::SetLifecycle(s) => {
            state.row.state = s;
            state.dirty = true;
        }
        // Datasheet
        EditorMsg::DatasheetSetMode(mode) => {
            use crate::library::editor::datasheet_picker::DatasheetMode;
            match mode {
                DatasheetMode::Url => match &state.row.datasheet {
                    signex_library::DatasheetRef::Url { .. } => {}
                    _ => {
                        state.row.datasheet = signex_library::DatasheetRef::default();
                        state.dirty = true;
                    }
                },
                DatasheetMode::PinnedPdf => match &state.row.datasheet {
                    signex_library::DatasheetRef::HashPinned { .. } => {}
                    _ => {
                        state.row.datasheet = signex_library::DatasheetRef::HashPinned {
                            hash: String::new(),
                            filename: String::new(),
                        };
                        state.dirty = true;
                    }
                },
            }
        }
        EditorMsg::DatasheetSetUrl(s) => {
            let trimmed = s.trim();
            state.row.datasheet = if trimmed.is_empty() {
                signex_library::DatasheetRef::default()
            } else {
                signex_library::DatasheetRef::url(trimmed)
            };
            state.dirty = true;
        }
        EditorMsg::DatasheetUploadResult(payload) => {
            if let Some((bytes, filename)) = payload {
                use sha2::Digest;
                let mut hasher = sha2::Sha256::new();
                hasher.update(&bytes);
                let hash = format!("{:x}", hasher.finalize());
                state.row.datasheet = signex_library::DatasheetRef::hash_pinned(hash, filename);
                state.dirty = true;
            }
        }
        // Pin Map
        EditorMsg::PinMapAutoMatchByNumber | EditorMsg::PinMapClearOverrides => {
            state.row.pin_map_overrides.clear();
            state.pin_map_state.expanded_row = None;
            state.pin_map_state.override_buf.clear();
            state.dirty = true;
        }
        EditorMsg::PinMapAutoMatchByName => {
            tracing::warn!(
                target: "signex::library",
                "Pin Map: Auto-Match by Name is stubbed; awaiting heuristic implementation"
            );
        }
        EditorMsg::PinMapOpenOverrideEdit(pin) => {
            let seed = state
                .row
                .pin_map_overrides
                .iter()
                .find(|o| o.symbol_pin_number == pin)
                .map(|o| o.footprint_pad_number.clone())
                .unwrap_or_default();
            state.pin_map_state.expanded_row = Some(pin);
            state.pin_map_state.override_buf = seed;
        }
        EditorMsg::PinMapOverrideBufChanged { pin, value } => {
            if state.pin_map_state.expanded_row.as_deref() == Some(pin.as_str()) {
                state.pin_map_state.override_buf = value;
            }
        }
        EditorMsg::PinMapAddOverride { pin, pad } => {
            let trimmed = pad.trim();
            if trimmed.is_empty() {
                state
                    .row
                    .pin_map_overrides
                    .retain(|o| o.symbol_pin_number != pin);
            } else if let Some(existing) = state
                .row
                .pin_map_overrides
                .iter_mut()
                .find(|o| o.symbol_pin_number == pin)
            {
                existing.footprint_pad_number = trimmed.to_string();
            } else {
                state
                    .row
                    .pin_map_overrides
                    .push(signex_library::PinPadOverride::new(pin, trimmed));
            }
            state.pin_map_state.expanded_row = None;
            state.pin_map_state.override_buf.clear();
            state.dirty = true;
        }
        EditorMsg::PinMapCancelOverrideEdit => {
            state.pin_map_state.expanded_row = None;
            state.pin_map_state.override_buf.clear();
        }
        EditorMsg::PinMapRemoveOverride { pin } => {
            state
                .row
                .pin_map_overrides
                .retain(|o| o.symbol_pin_number != pin);
            state.pin_map_state.expanded_row = None;
            state.pin_map_state.override_buf.clear();
            state.dirty = true;
        }
        // Supply — primary
        EditorMsg::SupplyPrimarySetManufacturer(s) => {
            state.row.primary_mpn.manufacturer = s;
            state.dirty = true;
        }
        EditorMsg::SupplyPrimarySetMpn(s) => {
            state.row.primary_mpn.mpn = s;
            state.dirty = true;
        }
        EditorMsg::SupplyPrimarySetStatus(s) => {
            state.row.primary_mpn.status = s;
            state.dirty = true;
        }
        EditorMsg::SupplyPrimarySetNotes(s) => {
            state.row.primary_mpn.notes = if s.trim().is_empty() { None } else { Some(s) };
            state.dirty = true;
        }
        // Supply — alternates
        EditorMsg::SupplyAlternateAdd => {
            let mut alt = signex_library::ManufacturerPart::draft("", "");
            alt.status = signex_library::AlternateStatus::Approved;
            state.row.alternates.push(alt);
            state.dirty = true;
        }
        EditorMsg::SupplyAlternateSetManufacturer { idx, value } => {
            if let Some(alt) = state.row.alternates.get_mut(idx) {
                alt.manufacturer = value;
                state.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateSetMpn { idx, value } => {
            if let Some(alt) = state.row.alternates.get_mut(idx) {
                alt.mpn = value;
                state.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateSetStatus { idx, value } => {
            if let Some(alt) = state.row.alternates.get_mut(idx) {
                alt.status = value;
                state.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateSetNotes { idx, value } => {
            if let Some(alt) = state.row.alternates.get_mut(idx) {
                alt.notes = if value.trim().is_empty() {
                    None
                } else {
                    Some(value)
                };
                state.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateRemove { idx } => {
            if idx < state.row.alternates.len() {
                state.row.alternates.remove(idx);
                state.dirty = true;
            }
        }
        // Supply — listings
        EditorMsg::SupplyListingAdd => {
            state.row.supply.push(signex_library::DistributorListing {
                distributor: String::new(),
                sku: String::new(),
                url: None,
                moq: None,
            });
            state.dirty = true;
        }
        EditorMsg::SupplyListingSetDistributor { idx, value } => {
            if let Some(listing) = state.row.supply.get_mut(idx) {
                listing.distributor =
                    crate::library::editor::supply::distributor_source_to_string(value);
                state.dirty = true;
            }
        }
        EditorMsg::SupplyListingSetSku { idx, value } => {
            if let Some(listing) = state.row.supply.get_mut(idx) {
                listing.sku = value;
                state.dirty = true;
            }
        }
        EditorMsg::SupplyListingSetUrl { idx, value } => {
            if let Some(listing) = state.row.supply.get_mut(idx) {
                listing.url = if value.trim().is_empty() {
                    None
                } else {
                    Some(value)
                };
                state.dirty = true;
            }
        }
        EditorMsg::SupplyListingRemove { idx } => {
            if idx < state.row.supply.len() {
                state.row.supply.remove(idx);
                state.dirty = true;
            }
        }
        // Parameters
        EditorMsg::ParamSetText { name, value } => {
            if !name.is_empty() {
                state
                    .row
                    .parameters
                    .insert(name.clone(), signex_library::ParamValue::Text(value));
                state.dirty = true;
            }
        }
        EditorMsg::ParamSetNumberBuf { name, buf } => {
            state.params_edit_buf.insert(name, buf);
        }
        EditorMsg::ParamCommitNumber { name } => {
            if let Some(buf) = state.params_edit_buf.get(&name).cloned() {
                let trimmed = buf.trim();
                if let Ok(v) = trimmed.parse::<f64>() {
                    state
                        .row
                        .parameters
                        .insert(name, signex_library::ParamValue::Number(v));
                    state.dirty = true;
                }
            }
        }
        EditorMsg::ParamSetMeasurementBuf { name, buf } => {
            state.params_edit_buf.insert(name, buf);
        }
        EditorMsg::ParamCommitMeasurement { name, unit } => {
            if let Some(buf) = state.params_edit_buf.get(&name).cloned() {
                let trimmed = buf.trim();
                if let Ok(v) = trimmed.parse::<f64>() {
                    state.row.parameters.insert(
                        name,
                        signex_library::ParamValue::Measurement { value: v, unit },
                    );
                    state.dirty = true;
                }
            }
        }
        EditorMsg::ParamSetBool { name, value } => {
            state
                .row
                .parameters
                .insert(name, signex_library::ParamValue::Bool(value));
            state.dirty = true;
        }
        EditorMsg::ParamRemove { name } => {
            state.row.parameters.remove(&name);
            state.dirty = true;
        }
        EditorMsg::ParamAddCustom { name, kind } => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return;
            }
            let value = match kind {
                ParamKindMsg::Text => signex_library::ParamValue::Text(String::new()),
                ParamKindMsg::Number => signex_library::ParamValue::Number(0.0),
                ParamKindMsg::Bool => signex_library::ParamValue::Bool(false),
                ParamKindMsg::Measurement(unit) => {
                    signex_library::ParamValue::Measurement { value: 0.0, unit }
                }
            };
            state.row.parameters.insert(trimmed.to_string(), value);
            state.dirty = true;
        }
        // Sim
        EditorMsg::SimSetEnabled(enabled) => {
            if enabled {
                if state.row.sim_ref.is_none() {
                    let sim = signex_library::SimModel {
                        uuid: uuid::Uuid::now_v7(),
                        name: state.row.internal_pn.as_str().to_string(),
                        kind: signex_library::SimKind::Spice3,
                        body: String::new(),
                        default_node_map: std::collections::BTreeMap::new(),
                        created: chrono::Utc::now(),
                        updated: chrono::Utc::now(),
                    };
                    state.row.sim_ref = Some(signex_library::PrimitiveRef::new(
                        state.row.symbol_ref.library_id,
                        sim.uuid,
                    ));
                    state.sim_body = Some(iced::widget::text_editor::Content::new());
                    state.sim = Some(sim);
                }
            } else {
                state.row.sim_ref = None;
                state.sim = None;
                state.sim_body = None;
            }
            state.dirty = true;
        }
        EditorMsg::SimSetKind(kind) => {
            if let Some(sim) = state.sim.as_mut() {
                sim.kind = kind;
                sim.updated = chrono::Utc::now();
                state.dirty = true;
            }
        }
        EditorMsg::SimSetName(name) => {
            if let Some(sim) = state.sim.as_mut() {
                sim.name = name;
                sim.updated = chrono::Utc::now();
                state.dirty = true;
            }
        }
        EditorMsg::SimBodyAction(action) => {
            if let Some(content) = state.sim_body.as_mut() {
                content.perform(action);
                if let Some(sim) = state.sim.as_mut() {
                    sim.body = content.text();
                    sim.updated = chrono::Utc::now();
                }
                state.dirty = true;
            }
        }
        EditorMsg::SimSetPinNode { pin_number, value } => {
            if let Some(sim) = state.sim.as_mut() {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    sim.default_node_map.remove(&pin_number);
                } else {
                    sim.default_node_map.insert(pin_number, trimmed.to_string());
                }
                sim.updated = chrono::Utc::now();
                state.dirty = true;
            }
        }
        // Variants below are kept around for the standalone primitive
        // editors (`.snxsym` / `.snxfpt` document tabs); they're
        // never fired through the Component Preview surface but stay
        // defined to keep the message tree backwards-compatible.
        EditorMsg::CloseEditor
        | EditorMsg::SaveDraft
        | EditorMsg::Commit
        | EditorMsg::SubmitForReview
        | EditorMsg::SubmitForReviewNotesChanged(_)
        | EditorMsg::SubmitForReviewCancel
        | EditorMsg::SubmitForReviewConfirm
        | EditorMsg::SubmitForReviewResult(_)
        | EditorMsg::OpenWhereUsedTab
        | EditorMsg::DatasheetUploadDialog
        | EditorMsg::SymbolPickAiPdf
        | EditorMsg::SymbolPickedAiPdf(_)
        | EditorMsg::SymbolSetTool(_)
        | EditorMsg::SymbolAddPin { .. }
        | EditorMsg::SymbolSelect(_)
        | EditorMsg::SymbolDeselect
        | EditorMsg::SymbolMoveSelected { .. }
        | EditorMsg::SymbolDeleteSelected
        | EditorMsg::SymbolSetField { .. }
        | EditorMsg::SymbolSetPinNumber { .. }
        | EditorMsg::SymbolSetPinName { .. }
        | EditorMsg::SymbolApplyAiPreview
        | EditorMsg::SymbolDismissAiPreview
        | EditorMsg::SaveSymbol(_, _)
        | EditorMsg::FootprintAddPad { .. }
        | EditorMsg::FootprintMovePad { .. }
        | EditorMsg::FootprintCursorAt { .. }
        | EditorMsg::FootprintSelectPad(_)
        | EditorMsg::FootprintDeleteSelected
        | EditorMsg::FootprintToggleLayer(_)
        | EditorMsg::FootprintToggleAutoFit
        | EditorMsg::SaveFootprint(_, _)
        | EditorMsg::SetBodyHeight(_)
        | EditorMsg::SetBodyOffsetZ(_)
        | EditorMsg::SetBodyTopColor(_)
        | EditorMsg::SetBodySideColor(_)
        | EditorMsg::SetBodyShape(_)
        | EditorMsg::StepAttachDialog
        | EditorMsg::StepAttachResult(_)
        | EditorMsg::StepAttachRemove
        | EditorMsg::SaveSim(_, _) => {}
    }
}

// ─────────────────────────────────────────────────────────────────────
// Stage 10 — recovery dialog plumbing
// ─────────────────────────────────────────────────────────────────────
//
// `LocalGitAdapter::open` returns a few recoverable error shapes that
// shouldn't drop on the floor as a bare `tracing::warn!`. The user
// either wants to point Signex at a moved file, accept that history
// is gone, or remove the library from the project entirely. The
// recovery module owns the modal layer; this section owns the
// classification + per-choice action.

use crate::library::recovery::{
    BrokenBindingChoice, GitMissingChoice, LibraryMissingChoice, RecoveryDialog,
};
use crate::library::state::LibraryState;
use signex_library::{LibraryError, LocalGitAdapter};

/// Classify a `LocalGitAdapter::open` error and, if recoverable,
/// stash the matching `RecoveryDialog` on `LibraryState::recovery`.
/// Unrecoverable errors are left alone — the caller's `tracing::warn!`
/// is the only surface.
///
/// String-matches the error message produced by `LocalGitAdapter::open`
/// because the underlying `LibraryError` enum doesn't carry structured
/// "missing-snxlib" / "missing-git" variants in v0.9. This is the
/// lower-effort path called out in `v0.9-snxlib-as-file-plan.md` §2
/// Stage H — adding `LibraryError::MissingGitRepo` /
/// `LibraryError::MissingSnxlibFile` variants is a clean follow-up
/// once the rest of v0.9 settles.
pub(crate) fn route_open_error(
    state: &mut LibraryState,
    path: &std::path::Path,
    err: &LibraryError,
) {
    // Don't clobber an already-open recovery dialog; the user resolves
    // them sequentially.
    if state.recovery.is_some() {
        return;
    }
    let dialog = match err {
        LibraryError::NotFound(msg) if msg.contains("no .snxlib") => {
            Some(RecoveryDialog::LibraryMissing {
                path: path.to_path_buf(),
            })
        }
        LibraryError::Backend(msg) if msg.starts_with("git open") => {
            // v0.9: no remote field on the manifest yet. Stage 13+ will
            // populate this from `[users.<remote>]` so the
            // "Restore from remote" button activates.
            Some(RecoveryDialog::GitMissing {
                path: path.to_path_buf(),
                remote: None,
            })
        }
        _ => None,
    };
    if let Some(d) = dialog {
        state.recovery = Some(d);
    }
}

/// Handle the user's choice from the *Library missing* recovery dialog.
fn handle_recovery_library_missing(app: &mut Signex, choice: LibraryMissingChoice) -> Task<Message> {
    match choice {
        LibraryMissingChoice::Cancel => {
            app.library.recovery = None;
            Task::none()
        }
        LibraryMissingChoice::Locate => Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Locate Library (*.snxlib)")
                    .add_filter("Signex Library", &["snxlib"])
                    .pick_file()
                    .await
                    .map(|f| f.path().to_path_buf())
            },
            |path| Message::Library(LibraryMessage::RecoveryLibraryMissingLocateResult(path)),
        ),
        LibraryMissingChoice::RemoveFromProject => {
            let missing = match app.library.recovery.as_ref() {
                Some(RecoveryDialog::LibraryMissing { path }) => path.clone(),
                _ => {
                    app.library.recovery = None;
                    return Task::none();
                }
            };
            for project in app.document_state.projects.iter_mut() {
                // Compute resolved paths up-front so the closure can
                // borrow only the indices vector, not project.data
                // (which retain's closure also tries to read).
                let resolved: Vec<std::path::PathBuf> = project
                    .data
                    .libraries
                    .iter()
                    .map(|e| project.data.resolve_library_path(e))
                    .collect();
                let mut idx = 0usize;
                project.data.libraries.retain(|_| {
                    let keep = resolved[idx] != missing;
                    idx += 1;
                    keep
                });
            }
            app.library.recovery = None;
            Task::none()
        }
    }
}

/// Handle the user's choice from the *Git missing* recovery dialog.
fn handle_recovery_git_missing(app: &mut Signex, choice: GitMissingChoice) -> Task<Message> {
    match choice {
        GitMissingChoice::Cancel | GitMissingChoice::Skip => {
            app.library.recovery = None;
            Task::none()
        }
        GitMissingChoice::ReInit => {
            let path = match app.library.recovery.as_ref() {
                Some(RecoveryDialog::GitMissing { path, .. }) => path.clone(),
                _ => {
                    app.library.recovery = None;
                    return Task::none();
                }
            };
            app.library.recovery = None;
            match LocalGitAdapter::recover_init(&path) {
                Ok(_) => Task::done(Message::Library(LibraryMessage::OpenLibraryAt(Some(path)))),
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        path = %path.display(),
                        error = %e,
                        "git recover-init failed"
                    );
                    Task::none()
                }
            }
        }
        GitMissingChoice::RestoreFromRemote => {
            // v0.9 leaves this disabled — the manifest doesn't carry a
            // remote yet. Treat as Cancel.
            app.library.recovery = None;
            Task::none()
        }
    }
}

/// Handle the user's choice from the *Broken primitive binding* dialog.
///
/// v0.9 stub: the dispatch path that detects broken bindings hasn't
/// landed yet (Stage 12+ wires the row-load checks). The handler
/// therefore only knows how to close the dialog; the actual rebind /
/// remove-row flows queue behind the detection plumbing. The dialog
/// surface itself ships now so the overlay layer is in place.
fn handle_recovery_broken_binding(
    app: &mut Signex,
    _choice: BrokenBindingChoice,
) -> Task<Message> {
    app.library.recovery = None;
    Task::none()
}
