//! Library subsystem dispatcher. Routes
//! [`crate::library::LibraryMessage`] to the right side-effecting
//! handler.
//!
//! v0.9-refactor-2 (DBLib model): the Component Editor became a
//! preview surface. Symbol/Footprint/Sim are read-only here; their
//! standalone `.snxsym` / `.snxfpt` / `.snxsim` document tabs (WS-7)
//! own actual editing. The dispatcher's editor handlers are scoped
//! to the five Component Preview tabs (Preview / Parameters / Supply
//! / Datasheet / Simulation).
//!
//! New-Component / picker-place / library-create flows depend on
//! `commands::*` helpers that are still being retargeted by other
//! Wave-3 slices (WS-5/8); the variants stay defined but log + bail
//! when the underlying helper hasn't landed yet.

use iced::Task;

use super::super::*;
use crate::library::commands;
use crate::library::messages::{
    EditorMsg, LibraryMessage, ParamKindMsg, PickerMsg, PrimitiveEditorMsg, SettingsMsg,
    SymbolSelectionMsg, SymbolToolMsg,
};
use crate::library::state::{
    ComponentPreviewState, EditorAddress, NewComponentState, PickerState, PreviewTab,
};
use signex_library::RowId;

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
                }
                Task::none()
            }
            LibraryMessage::CloseLibrary(path) => {
                self.library.close_library(&path);
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

            // ── New Component flow (WS-8 retargets the helper) ──────
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
                if let Some(nc) = self.library.new_component.as_mut() {
                    nc.class = class;
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
                // WS-8 wires the row-based `create_component_row` flow.
                // Until that helper lands here, log the request and
                // bail.
                tracing::warn!(
                    target: "signex::library",
                    "NewComponentSubmit: row-based create flow ships in WS-8"
                );
                self.library.new_component = None;
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
                tracing::info!(
                    target: "signex::library",
                    library = %library_path.display(),
                    dirty = dirty_editors.len(),
                    "close-library confirm modal — full UI ships post-WS6"
                );
                Task::none()
            }
            LibraryMessage::CloseLibraryConfirm(_) => Task::none(),
            LibraryMessage::PlaceLibraryComponent {
                library_path,
                table,
                row_id,
            } => {
                tracing::info!(
                    target: "signex::library",
                    library = %library_path.display(),
                    table = %table,
                    row = %row_id,
                    "place library row — engine wire-up ships in a follow-up patch"
                );
                Task::none()
            }
            LibraryMessage::CreateLibraryAt(project_root) => {
                self.handle_create_library_for_project(project_root)
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
            LibraryMessage::NewComponentSetTable(table) => {
                // WS-8: pin the row's target table on the modal state.
                if let Some(nc) = self.library.new_component.as_mut() {
                    nc.table = Some(table);
                    nc.error = None;
                }
                Task::none()
            }
            // WS-7 (refactor-2): standalone primitive editor tabs
            LibraryMessage::PrimitiveEditorEvent { path, msg } => {
                self.handle_primitive_editor_event(path, msg)
            }
        }
    }

    /// Create a fresh `<name>.snxlib/` under the project rooted at
    /// `project_root`. Default name `<project>-lib`; conflicts are
    /// suffix-disambiguated. The library is registered on
    /// `Project::libraries` so the project tree picks it up on the
    /// next refresh.
    fn handle_create_library_for_project(
        &mut self,
        project_root: std::path::PathBuf,
    ) -> Task<Message> {
        // Locate the LoadedProject so we can mutate its `libraries`
        // list. We match on `project_root` against the project file
        // path and against its parent dir — callers in WS-H emit the
        // file path, but a future menu wired to a tree-row right-
        // click could reasonably emit the directory.
        let Some(loaded) =
            self.document_state.projects.iter_mut().find(|p| {
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
        if project_dir.join(format!("{name}.snxlib")).exists() {
            for n in 2..=99 {
                let candidate = format!("{stem}-lib-{n}");
                if !project_dir.join(format!("{candidate}.snxlib")).exists() {
                    name = candidate;
                    break;
                }
            }
        }

        match crate::library::commands::create_library(&mut self.library, &mut loaded.data, &name) {
            Ok(library_id) => {
                tracing::info!(
                    target: "signex::library",
                    project = %loaded.path.display(),
                    library_name = %name,
                    library_id = %library_id,
                    "created project-local library"
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    project = %loaded.path.display(),
                    library_name = %name,
                    error = %e,
                    "create_library failed"
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

        // Pre-load the row from the adapter. WS-2's `read_row` is the
        // canonical lookup; if it fails we surface and bail without
        // leaving an empty tab behind.
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
                // WS-5 (DBLib): `ComponentSummary` lost its `uuid`
                // alias when WS-1 renamed `ComponentId` → `RowId`.
                // Match against the row tier's `row_id` directly.
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

    // WS-7 (refactor-2): standalone primitive editor tabs
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
                let bytes = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
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
                let symbol: signex_library::Symbol = match serde_json::from_str(&bytes) {
                    Ok(s) => s,
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
                    .unwrap_or_else(|| symbol.name.clone());
                let project_id = self.document_state.project_for_path(&path).map(|p| p.id);

                let state = crate::app::SymbolEditorState::new(path.clone(), symbol);
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

    // WS-7 (refactor-2): standalone primitive editor tabs
    /// Apply a primitive-editor inner message to the matching tab's
    /// editor state. Path-keyed lookup distinguishes Symbol vs
    /// Footprint; the dispatcher routes to the existing canvas-state
    /// helpers so the behaviour matches the in-Component Editor
    /// experience verbatim.
    pub(crate) fn handle_primitive_editor_event(
        &mut self,
        path: std::path::PathBuf,
        msg: PrimitiveEditorMsg,
    ) -> Task<Message> {
        // Save is a sibling of the canvas-mutation messages — route
        // through the standalone save path which writes JSON back to
        // disk and (when applicable) reloads in the LibrarySet.
        if matches!(msg, PrimitiveEditorMsg::Save) {
            self.save_primitive_tab_at(&path);
            return Task::none();
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

    // WS-7 (refactor-2): standalone primitive editor tabs
    /// Write the primitive at `path` back to disk as JSON, mark the
    /// tab clean, and (when the file lives under a `.snxlib/`) ask
    /// the matching `LibrarySet` adapter to reload its cached copy
    /// so any open Component Preview tabs (WS-6) see the new bytes.
    pub(crate) fn save_primitive_tab_at(&mut self, path: &std::path::Path) {
        // Symbol path.
        if let Some(editor) = self.document_state.symbol_editors.get_mut(path) {
            // Refresh the `updated` timestamp before serialising so
            // downstream consumers can detect the rewrite.
            editor.primitive.updated = chrono::Utc::now();
            let json = match serde_json::to_string_pretty(&editor.primitive) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        path = %path.display(),
                        error = %e,
                        "save primitive: serialize symbol failed",
                    );
                    return;
                }
            };
            if let Err(e) = std::fs::write(path, json) {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "save primitive: write .snxsym failed",
                );
                return;
            }
            editor.dirty = false;
            // Clear the project-scoped dirty marker if any callers
            // had set it.
            self.document_state.dirty_paths.remove(path);
            // Clear the matching tab's dirty flag too.
            if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
                tab.dirty = false;
            }
            // Best-effort LibrarySet reload — only fires when the
            // primitive lives under a `.snxlib/` we already have
            // mounted. Failures are non-fatal.
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
            if let Err(e) = std::fs::write(path, json) {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "save primitive: write .snxfpt failed",
                );
                return;
            }
            editor.dirty = false;
            self.document_state.dirty_paths.remove(path);
            if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
                tab.dirty = false;
            }
            self.reload_primitive_in_library_set(path);
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
        // helper (called out in plan §12 Step 7.4 as a small change in
        // `signex-library`). The standalone editor tab already holds
        // the authoritative copy of the primitive in memory; on-disk
        // round-trips happen here, and Component Preview tabs (WS-6)
        // pull through `LibrarySet::resolve_*` on the next view, so
        // the only hole this leaves is a Preview tab that's already
        // resolved + cached its primitive in editor state. WS-6 owns
        // that cache; we mark this as "no-op for now" so the wave
        // doesn't add a dependency on a sibling slice.
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
            };
        }
        PrimitiveEditorMsg::SymbolAddPin { x, y } => {
            let idx = crate::library::editor::symbol::state::add_pin(&mut editor.primitive, x, y);
            editor.selected = Some(SymbolSelection::Pin(idx));
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolSelect(sel) => {
            editor.selected = Some(match sel {
                SymbolSelectionMsg::Pin(idx) => SymbolSelection::Pin(idx),
                SymbolSelectionMsg::FieldReference => SymbolSelection::Field(FieldKey::Reference),
                SymbolSelectionMsg::FieldValue => SymbolSelection::Field(FieldKey::Value),
            });
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolDeselect => {
            editor.selected = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolMoveSelected { x, y } => {
            crate::library::editor::symbol::state::move_selected(
                &mut editor.primitive,
                editor.selected,
                x,
                y,
            );
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolDeleteSelected => {
            if let Some(new_sel) = crate::library::editor::symbol::state::delete_selected(
                &mut editor.primitive,
                editor.selected,
            ) {
                editor.selected = new_sel;
                editor.dirty = true;
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::SymbolSetPinNumber { idx, number } => {
            if let Some(pin) = editor.primitive.pins.get_mut(idx) {
                pin.number = number;
                editor.dirty = true;
            }
        }
        PrimitiveEditorMsg::SymbolSetPinName { idx, name } => {
            if let Some(pin) = editor.primitive.pins.get_mut(idx) {
                pin.name = name;
                editor.dirty = true;
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
        | PrimitiveEditorMsg::SymbolSelect(_)
        | PrimitiveEditorMsg::SymbolDeselect
        | PrimitiveEditorMsg::SymbolMoveSelected { .. }
        | PrimitiveEditorMsg::SymbolDeleteSelected
        | PrimitiveEditorMsg::SymbolSetPinNumber { .. }
        | PrimitiveEditorMsg::SymbolSetPinName { .. }
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
        // editors (WS-7); they're never fired through the Component
        // Preview surface in v0.9-refactor-2 but stay defined to keep
        // the message tree backwards-compatible.
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
