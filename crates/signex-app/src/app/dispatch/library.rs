//! Library subsystem dispatcher. Routes
//! [`crate::library::LibraryMessage`] to the right side-effecting
//! handler.
//!
//! WS-E (refactor): the editor inline-edit handlers were trimmed back
//! to the bindings still present on the new `Revision` shape (Overview
//! and History only). Symbol, Footprint, 3D, Sim, and Pin Map dispatch
//! will return in WS-F and WS-G as the new editors land. The New
//! Component flow is end-to-end here for the first time.
//!
//! WS-I (refactor): the Component Editor opens as a tab in the main
//! window — the editor state lives in `LibraryState.editors` keyed by
//! `(library_path, component_id)`. Detach-to-window stays available
//! via the existing tab-undock flow; the daemon-window setup that
//! Wave 2 used (one editor per OS window via `iced::window::open`)
//! is gone.

use iced::Task;

use super::super::*;
use crate::library::commands;
use crate::library::messages::{
    EditorMsg, LibraryMessage, ParamKindMsg, PickerMsg, SettingsMsg, SymbolSelectionMsg,
    SymbolToolMsg,
};
use crate::library::state::{
    ComponentEditorState, EditorAddress, EditorTab, NewComponentState, PickerState,
};

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

            // ── WS-E: New Component flow ────────────────────────────
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
                match commands::create_component(
                    &mut self.library,
                    &nc.internal_pn,
                    library_idx,
                    nc.class.clone(),
                    &nc.category,
                ) {
                    Ok(created) => {
                        self.library.new_component = None;
                        return Task::done(Message::Library(LibraryMessage::OpenEditor {
                            library_path: created.library_root,
                            component_id: created.component_id,
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
            // WS-I: tab-not-window
            LibraryMessage::OpenEditor {
                library_path,
                component_id,
            } => self.handle_open_editor(library_path, component_id),
            LibraryMessage::EditorEvent {
                library_path,
                component_id,
                msg,
            } => self.handle_editor_event(EditorAddress::new(library_path, component_id), msg),
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
                    "close-library confirm modal — full UI ships Phase 3"
                );
                Task::none()
            }
            LibraryMessage::CloseLibraryConfirm(_) => Task::none(),
            LibraryMessage::PlaceLibraryComponent {
                library_path,
                component_id,
                version,
            } => {
                tracing::info!(
                    target: "signex::library",
                    library = %library_path.display(),
                    component = %component_id,
                    version = %version,
                    "place library component — engine wire-up ships Phase 3"
                );
                Task::none()
            }
            // WS-H: Project tree library wiring
            LibraryMessage::CreateLibraryAt(project_root) => {
                self.handle_create_library_for_project(project_root)
            }
            // ── WS-5 (DBLib): row-tier panel wiring ──────────────
            LibraryMessage::OpenComponentRow {
                library_path,
                table,
                row_id,
            } => {
                // WS-6 owns the actual Component Preview tab
                // construction. v0.9-refactor-2 plan §11 lays out the
                // 5-tab `ComponentPreviewState`; until that lands we
                // emit a tracing breadcrumb and re-fire
                // `ComponentPreviewOpened` so WS-6 can pick the
                // synthetic-tab-path off the same address shape used
                // here.
                let synthetic_path = library_path
                    .join("tables")
                    .join(format!("{table}.tsv#{row_id}"));
                tracing::info!(
                    target: "signex::library",
                    library = %library_path.display(),
                    table = %table,
                    row_id = %row_id,
                    "OpenComponentRow — Component Preview tab construction lands in WS-6"
                );
                Task::done(Message::Library(LibraryMessage::ComponentPreviewOpened {
                    path: synthetic_path,
                    table,
                    row_id,
                }))
            }
            LibraryMessage::OpenPrimitiveEditor { path } => {
                // WS-7 owns the standalone `.snxsym` / `.snxfpt`
                // document-tab handler. Until that lands, log the
                // request so the panel-side trace is visible end to
                // end.
                tracing::info!(
                    target: "signex::library",
                    primitive = %path.display(),
                    "OpenPrimitiveEditor — standalone primitive tab lands in WS-7"
                );
                Task::none()
            }
            LibraryMessage::ComponentPreviewOpened {
                path,
                table,
                row_id,
            } => {
                // Trace-only sink. WS-6 will replace the body with
                // real tab construction; the variant exists in the
                // Wave 2 contract so all four slices stitch together
                // at merge time without further enum churn.
                tracing::debug!(
                    target: "signex::library",
                    path = %path.display(),
                    table = %table,
                    row_id = %row_id,
                    "ComponentPreviewOpened — placeholder until WS-6"
                );
                Task::none()
            }
            LibraryMessage::NewComponentSetTable(table) => {
                // WS-8 owns the New Component modal's table picker;
                // the dispatcher shape is already wired so the four
                // Wave 2 slices stitch at merge. v0.9 keeps this as
                // a trace-only no-op until the modal grows the
                // Table pick_list.
                tracing::debug!(
                    target: "signex::library",
                    table = %table,
                    "NewComponentSetTable — modal pre-select lands in WS-8"
                );
                Task::none()
            }
        }
    }

    /// WS-H: create a fresh `<name>.snxlib/` under the project rooted at
    /// `project_root` (the `.snxprj` / `.standard_pro` path). The library
    /// name is auto-derived from the project stem so the right-click
    /// flow stays one click — name conflicts surface as a tracing
    /// warn and the user can re-run from the menu when needed; a
    /// proper rename modal lands with the v0.9.x project-tree polish.
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

        // Default name: `<project stem>-lib`. The plan calls for a
        // name dialog (Goal #2 / step H4); v0.9 WS-H ships the
        // round-trip first and a Phase 2 patch can drop a TextInput
        // modal in place of the auto-name.
        let stem = loaded
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("project");
        let mut name = format!("{stem}-lib");

        // Disambiguate against the project-dir contents so back-to-
        // back invocations don't collide with a `Conflict` error.
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

        // Refresh the panel so the new library appears under the
        // Libraries node immediately. The project tree is rebuilt
        // from `PanelContext::projects`, so the new entry shows up
        // on the next view tick.
        self.refresh_panel_ctx();
        Task::none()
    }

    // WS-I: tab-not-window
    /// Open the Component Editor for `(library_path, component_id)`
    /// as a tab in the main window's tab bar. If the same component
    /// is already open in another tab, just activate it. Detach to a
    /// separate window remains available via the existing tab-undock
    /// flow (the user right-clicks the tab → "Open In New Window").
    fn handle_open_editor(
        &mut self,
        library_path: std::path::PathBuf,
        component_id: uuid::Uuid,
    ) -> Task<Message> {
        let address = EditorAddress::new(library_path.clone(), component_id);
        let synthetic_path = address.synthetic_tab_path();

        // Already open? Just activate the existing tab.
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

        // Pre-load the editor state. If the load fails, surface the
        // error and bail without leaving an empty tab behind.
        let editor = match commands::load_component_for_editor(
            &mut self.library,
            &library_path,
            component_id,
        ) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(target: "signex::library", error = %e, "open editor pre-load failed");
                return Task::none();
            }
        };

        let title = editor.display_internal_pn.clone();
        let project_id = self
            .document_state
            .project_for_path(&synthetic_path)
            .map(|p| p.id);
        self.library.editors.insert(address.clone(), editor);
        self.park_active_schematic_session();
        self.document_state.tabs.push(crate::app::TabInfo {
            title,
            path: synthetic_path,
            cached_document: None,
            dirty: false,
            project_id,
            kind: crate::app::TabKind::ComponentEditor(crate::app::ComponentEditorTab {
                library_path: address.library_path.clone(),
                component_id: address.component_id,
            }),
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        // Component Editor tabs don't drive `active_path` /
        // schematic-engine activation; clear those so the canvas
        // doesn't render a stale schematic underneath the editor.
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

    // WS-I: tab-not-window
    fn handle_editor_event(&mut self, address: EditorAddress, msg: EditorMsg) -> Task<Message> {
        match msg {
            EditorMsg::CloseEditor => {
                // Close the editor tab carrying this address. The
                // editor state lives in `library.editors`; tab close
                // also takes care of any undocked window via
                // `close_tab_now`.
                let synthetic = address.synthetic_tab_path();
                if let Some(idx) = self
                    .document_state
                    .tabs
                    .iter()
                    .position(|t| t.path == synthetic)
                {
                    return self.close_tab_now(idx);
                }
                // No tab found — drop any orphan editor state and
                // continue gracefully.
                self.library.editors.remove(&address);
                return Task::none();
            }
            EditorMsg::SaveDraft => {
                if let Err(e) = commands::save_draft(&mut self.library, &address) {
                    tracing::warn!(target: "signex::library", error = %e, "save_draft failed");
                }
                return Task::none();
            }
            EditorMsg::Commit => {
                // WS-5 (DBLib): the v0.9-original `commit_revision`
                // path mutated `Component::revisions` + `head`; that
                // model is gone in the row tier. WS-6 owns the
                // replacement (a single `adapter.update_row` call
                // with a fresh content hash). Leaving the handler as
                // a trace-only stub until the row-shaped editor lands.
                tracing::warn!(
                    target: "signex::library",
                    address = ?address,
                    "EditorMsg::Commit — WS-6 wires the row-tier commit; stub no-op"
                );
                return Task::none();
            }
            EditorMsg::SubmitForReview => {
                if let Some(editor) = self.library.editors.get_mut(&address) {
                    editor.review_dialog_open = true;
                    editor.review_status = None;
                }
                return Task::none();
            }
            EditorMsg::SubmitForReviewNotesChanged(s) => {
                if let Some(editor) = self.library.editors.get_mut(&address) {
                    editor.review_notes_buf = s;
                }
                return Task::none();
            }
            EditorMsg::SubmitForReviewCancel => {
                if let Some(editor) = self.library.editors.get_mut(&address) {
                    editor.review_dialog_open = false;
                    editor.review_status = None;
                }
                return Task::none();
            }
            EditorMsg::SubmitForReviewConfirm => {
                // WS-5 (DBLib): the v0.9-original `save_revision` API
                // is gone — review submissions for a row tier ride
                // through `adapter.update_row(&table, row, msg)` with
                // the lifecycle bumped on the row payload. WS-6 wires
                // the replacement; until then the handler reports a
                // "not yet implemented" status so the UI doesn't
                // hang in the in-flight state.
                if let Some(editor) = self.library.editors.get_mut(&address) {
                    editor.review_in_flight = false;
                    editor.review_status = Some(
                        "Submit-for-Review wires through to the new update_row path in WS-6".into(),
                    );
                }
                return Task::done(Message::Library(LibraryMessage::EditorEvent {
                    library_path: address.library_path.clone(),
                    component_id: address.component_id,
                    msg: EditorMsg::SubmitForReviewResult(Err(
                        "submit-for-review path is WS-6 territory".into(),
                    )),
                }));
            }
            EditorMsg::SubmitForReviewResult(result) => {
                if let Some(editor) = self.library.editors.get_mut(&address) {
                    editor.review_in_flight = false;
                    match result {
                        Ok(()) => {
                            editor.review_dialog_open = false;
                            editor.review_status = None;
                            editor.review_notes_buf.clear();
                            editor.draft.state = signex_library::LifecycleState::InReview;
                        }
                        Err(reason) => {
                            editor.review_status = Some(format!("Failed: {reason}"));
                        }
                    }
                }
                return Task::none();
            }
            EditorMsg::OpenWhereUsedTab => {
                if let Some(editor) = self.library.editors.get_mut(&address) {
                    editor.active_tab = EditorTab::WhereUsed;
                }
                return Task::none();
            }
            // ── WS-F2: tab switch with lazy primitive resolve ────────
            EditorMsg::SelectTab(tab) => {
                self.handle_select_editor_tab(&address, tab);
                return Task::none();
            }
            // ── WS-F2: AI-stub PDF picker ────────────────────────────
            EditorMsg::SymbolPickAiPdf => {
                let library_path = address.library_path.clone();
                let component_id = address.component_id;
                return Task::perform(
                    async {
                        let picked = rfd::AsyncFileDialog::new()
                            .set_title("Pick datasheet PDF for AI pinout heuristic")
                            .add_filter("PDF", &["pdf"])
                            .add_filter("All files", &["*"])
                            .pick_file()
                            .await;
                        match picked {
                            Some(handle) => {
                                let bytes = handle.read().await;
                                Some(bytes)
                            }
                            None => None,
                        }
                    },
                    move |bytes| {
                        Message::Library(LibraryMessage::EditorEvent {
                            library_path: library_path.clone(),
                            component_id,
                            msg: EditorMsg::SymbolPickedAiPdf(bytes),
                        })
                    },
                );
            }
            // ── WS-F2: STEP attach picker ────────────────────────────
            EditorMsg::StepAttachDialog => {
                let library_path = address.library_path.clone();
                let component_id = address.component_id;
                return Task::perform(
                    async {
                        let picked = rfd::AsyncFileDialog::new()
                            .set_title("Attach STEP file")
                            .add_filter("STEP", &["step", "stp"])
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
                            component_id,
                            msg: EditorMsg::StepAttachResult(result),
                        })
                    },
                );
            }
            EditorMsg::StepAttachResult(payload) => {
                self.handle_step_attach_result(&address, payload);
                return Task::none();
            }
            EditorMsg::SymbolPickedAiPdf(payload) => {
                self.handle_symbol_picked_ai_pdf(&address, payload);
                return Task::none();
            }
            _ => {}
        }

        // All remaining variants mutate the editor in place.
        let Some(editor) = self.library.editors.get_mut(&address) else {
            return Task::none();
        };
        apply_inline_edit(editor, msg);
        Task::none()
    }

    /// WS-F2: lazy-load the bound primitive when the user enters a tab
    /// that needs it. Symbol / Footprint / PinMap all fall back to
    /// `LibrarySet::resolve_*` (and seed an empty primitive when the
    /// resolver returns nothing — the New-Component flow saves
    /// primitives via the no-op `LibrarySet::save_*` shim until WS-C
    /// wires real persistence).
    fn handle_select_editor_tab(&mut self, address: &EditorAddress, tab: EditorTab) {
        let Some(editor) = self.library.editors.get_mut(address) else {
            return;
        };
        editor.active_tab = tab;

        match tab {
            EditorTab::Symbol => {
                if editor.symbol.is_none() {
                    let resolved = self.library.set.resolve_symbol(&editor.draft.symbol_ref);
                    let editor = self
                        .library
                        .editors
                        .get_mut(address)
                        .expect("editor present");
                    editor.symbol = Some(resolved.unwrap_or_else(|| {
                        signex_library::Symbol::empty(editor.display_internal_pn.as_str())
                    }));
                }
            }
            EditorTab::Footprint => {
                if editor.footprint.is_none() {
                    let resolved = editor
                        .draft
                        .footprint_ref
                        .as_ref()
                        .and_then(|r| self.library.set.resolve_footprint(r));
                    let editor = self
                        .library
                        .editors
                        .get_mut(address)
                        .expect("editor present");
                    editor.footprint = Some(resolved.unwrap_or_else(|| {
                        signex_library::Footprint::empty(editor.display_internal_pn.as_str())
                    }));
                }
                // Build / refresh canvas-side mirror.
                if let Some(editor) = self.library.editors.get_mut(address)
                    && let Some(fp) = editor.footprint.as_ref()
                    && editor.footprint_state.is_none()
                {
                    editor.footprint_state = Some(
                        crate::library::editor::footprint::state::FootprintEditorState::from_footprint(fp),
                    );
                }
            }
            EditorTab::PinMap => {
                // Pin Map needs both primitives — resolve them eagerly.
                if let Some(editor) = self.library.editors.get_mut(address)
                    && editor.symbol.is_none()
                {
                    let resolved = self.library.set.resolve_symbol(&editor.draft.symbol_ref);
                    let editor = self
                        .library
                        .editors
                        .get_mut(address)
                        .expect("editor present");
                    editor.symbol = Some(resolved.unwrap_or_else(|| {
                        signex_library::Symbol::empty(editor.display_internal_pn.as_str())
                    }));
                }
                if let Some(editor) = self.library.editors.get_mut(address)
                    && editor.footprint.is_none()
                {
                    let resolved = editor
                        .draft
                        .footprint_ref
                        .as_ref()
                        .and_then(|r| self.library.set.resolve_footprint(r));
                    let editor = self
                        .library
                        .editors
                        .get_mut(address)
                        .expect("editor present");
                    editor.footprint = Some(resolved.unwrap_or_else(|| {
                        signex_library::Footprint::empty(editor.display_internal_pn.as_str())
                    }));
                }
            }
            // WS-L: Sim tab
            EditorTab::Sim => {
                // Symbol is needed for the pin/node table — resolve it
                // alongside the sim model so the table renders without
                // an additional tab switch.
                if let Some(editor) = self.library.editors.get_mut(address)
                    && editor.symbol.is_none()
                {
                    let resolved = self.library.set.resolve_symbol(&editor.draft.symbol_ref);
                    let editor = self
                        .library
                        .editors
                        .get_mut(address)
                        .expect("editor present");
                    editor.symbol = Some(resolved.unwrap_or_else(|| {
                        signex_library::Symbol::empty(editor.display_internal_pn.as_str())
                    }));
                }
                // Lazy-resolve the SimModel primitive when the binding
                // exists. Missing-binding stays as `None` so the view
                // can render the "no SPICE model bound" placeholder
                // and a "Has SPICE Model" toggle to opt in.
                if let Some(editor) = self.library.editors.get_mut(address)
                    && editor.sim.is_none()
                    && let Some(sim_ref) = editor.draft.sim_ref
                {
                    let resolved = self.library.set.resolve_sim(&sim_ref);
                    let editor = self
                        .library
                        .editors
                        .get_mut(address)
                        .expect("editor present");
                    editor.sim = resolved;
                }
                // Seed the live `text_editor::Content` from the sim's
                // body — mirrored back on every SimBodyAction. When no
                // sim model is bound the content stays `None`.
                if let Some(editor) = self.library.editors.get_mut(address)
                    && editor.sim_body.is_none()
                    && let Some(sim) = editor.sim.as_ref()
                {
                    let body = sim.body.clone();
                    let editor = self
                        .library
                        .editors
                        .get_mut(address)
                        .expect("editor present");
                    editor.sim_body = Some(iced::widget::text_editor::Content::with_text(&body));
                }
            }
            _ => {}
        }
    }

    /// WS-F2: package the picked PDF bytes into the AI-stub heuristic
    /// preview so the view can render the apply/cancel card.
    fn handle_symbol_picked_ai_pdf(&mut self, address: &EditorAddress, payload: Option<Vec<u8>>) {
        let Some(editor) = self.library.editors.get_mut(address) else {
            return;
        };
        match payload {
            Some(bytes) => {
                let preview =
                    crate::library::editor::symbol::ai_stub::AiPinoutPreview::from_pdf(&bytes);
                editor.symbol_ai_preview = Some(preview);
            }
            None => {
                editor.symbol_ai_preview = None;
            }
        }
    }

    /// WS-F2: stash the picked STEP file under
    /// `<lib_root>/step/<sha256>.step` and bind a `StepAttachment`
    /// onto the active footprint primitive.
    fn handle_step_attach_result(
        &mut self,
        address: &EditorAddress,
        payload: Option<(Vec<u8>, String)>,
    ) {
        let Some(editor) = self.library.editors.get_mut(address) else {
            return;
        };
        let Some((bytes, filename)) = payload else {
            return;
        };
        let lib_root = editor.library_root.clone();
        let attachment = crate::library::editor::footprint::step_attach::stash_step(
            &lib_root, &bytes, &filename,
        );
        if let Some(att) = attachment
            && let Some(fp) = editor.footprint.as_mut()
        {
            fp.step_attachment = Some(att);
            editor.dirty = true;
        }
    }
}

/// Apply an inline form edit to the editor draft. WS-E only handles
/// the Overview + History fields that survive the data-model refactor.
/// Symbol / Footprint / 3D / Sim / Pin Map dispatch returns when WS-F
/// + WS-G land.
///
/// Visibility is `pub(crate)` so unit tests in sibling modules can
/// drive the editor through the same code path the dispatcher uses.
pub(crate) fn apply_inline_edit(editor: &mut ComponentEditorState, msg: EditorMsg) {
    match msg {
        // SelectTab is handled before reaching here (lazy-load needs
        // `&mut self.library.set`); this branch is the catch-all for
        // editors where the tab was already loaded.
        EditorMsg::SelectTab(tab) => editor.active_tab = tab,
        EditorMsg::OverviewSetDisplayName(s) => editor.display_internal_pn = s,
        EditorMsg::OverviewSetInternalPn(s) => {
            editor.component.internal_pn = signex_library::InternalPn::new(s.clone());
            editor.display_internal_pn = s;
        }
        EditorMsg::OverviewSetMpn(s) => {
            editor.draft.primary_mpn.mpn = s;
        }
        EditorMsg::OverviewSetManufacturer(s) => {
            editor.draft.primary_mpn.manufacturer = s;
        }
        EditorMsg::OverviewSetDescription(s) => {
            // WS-E: description is a free-form note field; the binding
            // record carries it on the primary MPN's `notes` slot for
            // now. WS-F will move it to a first-class field if needed.
            editor.draft.primary_mpn.notes = if s.trim().is_empty() { None } else { Some(s) };
        }
        EditorMsg::OverviewSetDatasheet(s) => {
            let trimmed = s.trim();
            editor.draft.datasheet = if trimmed.is_empty() {
                signex_library::DatasheetRef::default()
            } else {
                signex_library::DatasheetRef::url(trimmed)
            };
        }
        EditorMsg::OverviewSetLifecycle(state) => editor.draft.state = state,
        EditorMsg::HistorySelectRevision(version) => {
            editor.history_selected = Some(version);
        }
        // ── WS-G: Pin Map ─────────────────────────────────────
        EditorMsg::PinMapAutoMatchByNumber | EditorMsg::PinMapClearOverrides => {
            editor.draft.pin_map_overrides.clear();
            editor.pin_map.expanded_row = None;
            editor.pin_map.override_buf.clear();
            editor.dirty = true;
        }
        EditorMsg::PinMapAutoMatchByName => {
            // Stub — the name-based heuristic ships in a follow-up.
            tracing::warn!(
                target: "signex::library",
                "Pin Map: Auto-Match by Name is stubbed; awaiting heuristic implementation"
            );
        }
        EditorMsg::PinMapOpenOverrideEdit(pin) => {
            let seed = editor
                .draft
                .pin_map_overrides
                .iter()
                .find(|o| o.symbol_pin_number == pin)
                .map(|o| o.footprint_pad_number.clone())
                .unwrap_or_default();
            editor.pin_map.expanded_row = Some(pin);
            editor.pin_map.override_buf = seed;
        }
        EditorMsg::PinMapOverrideBufChanged { pin, value } => {
            if editor.pin_map.expanded_row.as_deref() == Some(pin.as_str()) {
                editor.pin_map.override_buf = value;
            }
        }
        EditorMsg::PinMapAddOverride { pin, pad } => {
            let trimmed = pad.trim();
            if trimmed.is_empty() {
                editor
                    .draft
                    .pin_map_overrides
                    .retain(|o| o.symbol_pin_number != pin);
            } else {
                use signex_library::PinPadOverride;
                if let Some(existing) = editor
                    .draft
                    .pin_map_overrides
                    .iter_mut()
                    .find(|o| o.symbol_pin_number == pin)
                {
                    existing.footprint_pad_number = trimmed.to_string();
                } else {
                    editor
                        .draft
                        .pin_map_overrides
                        .push(PinPadOverride::new(pin, trimmed));
                }
            }
            editor.pin_map.expanded_row = None;
            editor.pin_map.override_buf.clear();
            editor.dirty = true;
        }
        EditorMsg::PinMapCancelOverrideEdit => {
            editor.pin_map.expanded_row = None;
            editor.pin_map.override_buf.clear();
        }
        EditorMsg::PinMapRemoveOverride { pin } => {
            editor
                .draft
                .pin_map_overrides
                .retain(|o| o.symbol_pin_number != pin);
            editor.pin_map.expanded_row = None;
            editor.pin_map.override_buf.clear();
            editor.dirty = true;
        }
        // ── /WS-G ─────────────────────────────────────────────
        // ── WS-F2: Symbol tab edits ───────────────────────────
        EditorMsg::SymbolSetTool(tool) => {
            editor.symbol_tool = match tool {
                SymbolToolMsg::Select => crate::library::editor::symbol::canvas::SymbolTool::Select,
                SymbolToolMsg::AddPin => crate::library::editor::symbol::canvas::SymbolTool::AddPin,
            };
        }
        EditorMsg::SymbolAddPin { x, y } => {
            if let Some(sym) = editor.symbol.as_mut() {
                let idx = crate::library::editor::symbol::state::add_pin(sym, x, y);
                editor.symbol_selected = Some(
                    crate::library::editor::symbol::state::SymbolSelection::Pin(idx),
                );
                editor.dirty = true;
            }
        }
        EditorMsg::SymbolSelect(sel) => {
            use crate::library::editor::symbol::state::{FieldKey, SymbolSelection};
            editor.symbol_selected = Some(match sel {
                SymbolSelectionMsg::Pin(idx) => SymbolSelection::Pin(idx),
                SymbolSelectionMsg::FieldReference => SymbolSelection::Field(FieldKey::Reference),
                SymbolSelectionMsg::FieldValue => SymbolSelection::Field(FieldKey::Value),
            });
        }
        EditorMsg::SymbolDeselect => {
            editor.symbol_selected = None;
        }
        EditorMsg::SymbolMoveSelected { x, y } => {
            if let Some(sym) = editor.symbol.as_mut() {
                crate::library::editor::symbol::state::move_selected(
                    sym,
                    editor.symbol_selected,
                    x,
                    y,
                );
                editor.dirty = true;
            }
        }
        EditorMsg::SymbolDeleteSelected => {
            if let Some(sym) = editor.symbol.as_mut()
                && let Some(new_sel) = crate::library::editor::symbol::state::delete_selected(
                    sym,
                    editor.symbol_selected,
                )
            {
                editor.symbol_selected = new_sel;
                editor.dirty = true;
            }
        }
        EditorMsg::SymbolSetField { key: _, value: _ } => {
            // WS-E rebinds Designator/Value drag against `Component`;
            // a no-op here so the message is benign until that wave
            // ships.
        }
        EditorMsg::SymbolSetPinNumber { idx, number } => {
            if let Some(sym) = editor.symbol.as_mut()
                && let Some(pin) = sym.pins.get_mut(idx)
            {
                pin.number = number;
                editor.dirty = true;
            }
        }
        EditorMsg::SymbolSetPinName { idx, name } => {
            if let Some(sym) = editor.symbol.as_mut()
                && let Some(pin) = sym.pins.get_mut(idx)
            {
                pin.name = name;
                editor.dirty = true;
            }
        }
        EditorMsg::SymbolApplyAiPreview => {
            if let Some(preview) = editor.symbol_ai_preview.take()
                && let Some(sym) = editor.symbol.as_mut()
            {
                crate::library::editor::symbol::state::apply_ai_pinout(
                    sym,
                    preview.into_apply_list(),
                );
                editor.dirty = true;
            }
        }
        EditorMsg::SymbolDismissAiPreview => {
            editor.symbol_ai_preview = None;
        }
        EditorMsg::SaveSymbol(_uuid, sym) => {
            // Snapshot save — the canonical path is SaveDraft (which
            // saves the Revision binding); this variant is reserved
            // for any future "save the primitive only" flow.
            if let Some(stored) = editor.symbol.as_mut() {
                *stored = *sym;
                editor.dirty = true;
            }
        }
        // ── WS-F2: Footprint tab edits ────────────────────────
        EditorMsg::FootprintAddPad { x_mm, y_mm } => {
            // Lazy-build canvas mirror if the tab was opened without
            // going through `SelectTab` (e.g. direct hit on the
            // canvas before the tab switch handler ran).
            if editor.footprint_state.is_none()
                && let Some(fp) = editor.footprint.as_ref()
            {
                editor.footprint_state = Some(
                    crate::library::editor::footprint::state::FootprintEditorState::from_footprint(
                        fp,
                    ),
                );
            }
            if let Some(state) = editor.footprint_state.as_mut() {
                let _idx = state.add_pad_at(x_mm, y_mm);
                if let Some(fp) = editor.footprint.as_mut() {
                    crate::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(state, fp);
                }
                if let Some(cache) = editor.footprint_canvas_cache.get() {
                    cache.clear();
                }
                editor.dirty = true;
            }
        }
        EditorMsg::FootprintMovePad { idx, x_mm, y_mm } => {
            if let Some(state) = editor.footprint_state.as_mut() {
                state.move_pad(idx, x_mm, y_mm);
                if let Some(fp) = editor.footprint.as_mut() {
                    crate::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(state, fp);
                }
                if let Some(cache) = editor.footprint_canvas_cache.get() {
                    cache.clear();
                }
                editor.dirty = true;
            }
        }
        EditorMsg::FootprintCursorAt { x_mm, y_mm } => {
            if let Some(state) = editor.footprint_state.as_mut() {
                state.cursor_mm = Some((x_mm, y_mm));
            }
        }
        EditorMsg::FootprintSelectPad(sel) => {
            if let Some(state) = editor.footprint_state.as_mut() {
                state.selected_pad = sel;
                if let Some(cache) = editor.footprint_canvas_cache.get() {
                    cache.clear();
                }
            }
        }
        EditorMsg::FootprintDeleteSelected => {
            if let Some(state) = editor.footprint_state.as_mut()
                && let Some(idx) = state.selected_pad
            {
                state.delete_pad(idx);
                if let Some(fp) = editor.footprint.as_mut() {
                    crate::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(state, fp);
                }
                if let Some(cache) = editor.footprint_canvas_cache.get() {
                    cache.clear();
                }
                editor.dirty = true;
            }
        }
        EditorMsg::FootprintToggleLayer(name) => {
            if let Some(state) = editor.footprint_state.as_mut()
                && let Some(layer) =
                    crate::library::editor::footprint::layers::FpLayer::from_standard_name(&name)
            {
                state.layer_visibility.toggle(layer);
                if let Some(cache) = editor.footprint_canvas_cache.get() {
                    cache.clear();
                }
            }
        }
        EditorMsg::FootprintToggleAutoFit => {
            if let Some(state) = editor.footprint_state.as_mut() {
                state.toggle_auto_fit();
                if let Some(fp) = editor.footprint.as_mut() {
                    crate::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(state, fp);
                }
                if let Some(cache) = editor.footprint_canvas_cache.get() {
                    cache.clear();
                }
            }
        }
        EditorMsg::SaveFootprint(_uuid, fp) => {
            if let Some(stored) = editor.footprint.as_mut() {
                editor.footprint_state = Some(
                    crate::library::editor::footprint::state::FootprintEditorState::from_footprint(
                        &fp,
                    ),
                );
                *stored = *fp;
                if let Some(cache) = editor.footprint_canvas_cache.get() {
                    cache.clear();
                }
                editor.dirty = true;
            }
        }
        EditorMsg::SetBodyHeight(h) => {
            if let Some(fp) = editor.footprint.as_mut() {
                fp.body_3d.height_mm = h;
                editor.dirty = true;
            }
        }
        EditorMsg::SetBodyOffsetZ(z) => {
            if let Some(fp) = editor.footprint.as_mut() {
                fp.body_3d.offset_z_mm = z;
                editor.dirty = true;
            }
        }
        EditorMsg::SetBodyTopColor(c) => {
            if let Some(fp) = editor.footprint.as_mut() {
                fp.body_3d.top_color = c;
                editor.dirty = true;
            }
        }
        EditorMsg::SetBodySideColor(c) => {
            if let Some(fp) = editor.footprint.as_mut() {
                fp.body_3d.side_color = c;
                editor.dirty = true;
            }
        }
        EditorMsg::SetBodyShape(s) => {
            if let Some(fp) = editor.footprint.as_mut() {
                fp.body_3d.shape = s;
                editor.dirty = true;
            }
        }
        EditorMsg::StepAttachRemove => {
            if let Some(fp) = editor.footprint.as_mut() {
                fp.step_attachment = None;
                editor.dirty = true;
            }
        }
        // ── WS-K: Supply tab ──────────────────────────────────
        EditorMsg::SupplyPrimarySetManufacturer(s) => {
            editor.draft.primary_mpn.manufacturer = s;
            editor.dirty = true;
        }
        EditorMsg::SupplyPrimarySetMpn(s) => {
            editor.draft.primary_mpn.mpn = s;
            editor.dirty = true;
        }
        EditorMsg::SupplyPrimarySetStatus(status) => {
            editor.draft.primary_mpn.status = status;
            editor.dirty = true;
        }
        EditorMsg::SupplyPrimarySetNotes(s) => {
            editor.draft.primary_mpn.notes = if s.is_empty() { None } else { Some(s) };
            editor.dirty = true;
        }
        EditorMsg::SupplyAlternateAdd => {
            use signex_library::{AlternateStatus, ManufacturerPart};
            let mut row = ManufacturerPart::draft("", "");
            // New alternates default to Approved — `Primary` is the
            // headline part's slot, not an alternate-row status.
            row.status = AlternateStatus::Approved;
            editor.draft.alternates.push(row);
            editor.dirty = true;
        }
        EditorMsg::SupplyAlternateSetManufacturer { idx, value } => {
            if let Some(alt) = editor.draft.alternates.get_mut(idx) {
                alt.manufacturer = value;
                editor.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateSetMpn { idx, value } => {
            if let Some(alt) = editor.draft.alternates.get_mut(idx) {
                alt.mpn = value;
                editor.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateSetStatus { idx, value } => {
            if let Some(alt) = editor.draft.alternates.get_mut(idx) {
                alt.status = value;
                editor.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateSetNotes { idx, value } => {
            if let Some(alt) = editor.draft.alternates.get_mut(idx) {
                alt.notes = if value.is_empty() { None } else { Some(value) };
                editor.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateRemove { idx } => {
            if idx < editor.draft.alternates.len() {
                editor.draft.alternates.remove(idx);
                editor.dirty = true;
            }
        }
        EditorMsg::SupplyListingAdd => {
            use signex_library::DistributorListing;
            // Default new listings to DigiKey — matches the picker's
            // first option so the row renders sensibly out of the gate.
            editor
                .draft
                .supply
                .push(DistributorListing::new("DigiKey", ""));
            editor.dirty = true;
        }
        EditorMsg::SupplyListingSetDistributor { idx, value } => {
            if let Some(listing) = editor.draft.supply.get_mut(idx) {
                listing.distributor =
                    crate::library::editor::supply::distributor_source_to_string(value);
                editor.dirty = true;
            }
        }
        EditorMsg::SupplyListingSetSku { idx, value } => {
            if let Some(listing) = editor.draft.supply.get_mut(idx) {
                listing.sku = value;
                editor.dirty = true;
            }
        }
        EditorMsg::SupplyListingSetUrl { idx, value } => {
            if let Some(listing) = editor.draft.supply.get_mut(idx) {
                listing.url = if value.is_empty() { None } else { Some(value) };
                editor.dirty = true;
            }
        }
        EditorMsg::SupplyListingRemove { idx } => {
            if idx < editor.draft.supply.len() {
                editor.draft.supply.remove(idx);
                editor.dirty = true;
            }
        }
        // ── /WS-K ─────────────────────────────────────────────
        // ── WS-J: Params tab ──────────────────────────────────
        EditorMsg::ParamSetText { name, value } => {
            use signex_library::ParamValue;
            editor
                .draft
                .parameters
                .insert(name, ParamValue::Text(value));
            editor.dirty = true;
        }
        EditorMsg::ParamSetNumberBuf { name, buf } => {
            editor.params_edit_buf.insert(name, buf);
        }
        EditorMsg::ParamCommitNumber { name } => {
            use signex_library::ParamValue;
            if let Some(buf) = editor.params_edit_buf.get(&name) {
                let trimmed = buf.trim();
                if trimmed.is_empty() {
                    editor.draft.parameters.remove(&name);
                    editor.params_edit_buf.remove(&name);
                    editor.dirty = true;
                } else if let Ok(n) = trimmed.parse::<f64>() {
                    editor
                        .draft
                        .parameters
                        .insert(name.clone(), ParamValue::Number(n));
                    editor.params_edit_buf.insert(name, n.to_string());
                    editor.dirty = true;
                }
            }
        }
        EditorMsg::ParamSetMeasurementBuf { name, buf } => {
            editor.params_edit_buf.insert(name, buf);
        }
        EditorMsg::ParamCommitMeasurement { name, unit } => {
            use signex_library::ParamValue;
            if let Some(buf) = editor.params_edit_buf.get(&name) {
                let trimmed = buf.trim();
                if trimmed.is_empty() {
                    editor.draft.parameters.remove(&name);
                    editor.params_edit_buf.remove(&name);
                    editor.dirty = true;
                } else if let Ok(value) = trimmed.parse::<f64>() {
                    editor.draft.parameters.insert(
                        name.clone(),
                        ParamValue::Measurement {
                            value,
                            unit: unit.clone(),
                        },
                    );
                    editor.params_edit_buf.insert(name, value.to_string());
                    editor.dirty = true;
                }
            }
        }
        EditorMsg::ParamSetBool { name, value } => {
            use signex_library::ParamValue;
            editor
                .draft
                .parameters
                .insert(name, ParamValue::Bool(value));
            editor.dirty = true;
        }
        EditorMsg::ParamRemove { name } => {
            editor.draft.parameters.remove(&name);
            editor.params_edit_buf.remove(&name);
            editor.dirty = true;
        }
        EditorMsg::ParamAddCustom { name, kind } => {
            use signex_library::ParamValue;
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return;
            }
            if editor.draft.parameters.contains_key(trimmed) {
                return;
            }
            let key = trimmed.to_string();
            let value = match kind {
                ParamKindMsg::Text => ParamValue::Text(String::new()),
                ParamKindMsg::Number => {
                    editor.params_edit_buf.insert(key.clone(), String::new());
                    ParamValue::Number(0.0)
                }
                ParamKindMsg::Bool => ParamValue::Bool(false),
                ParamKindMsg::Measurement(unit) => {
                    editor.params_edit_buf.insert(key.clone(), String::new());
                    ParamValue::Measurement { value: 0.0, unit }
                }
            };
            editor.draft.parameters.insert(key, value);
            editor.dirty = true;
        }
        // ── /WS-J ─────────────────────────────────────────────
        // ── WS-L: Sim tab ────────────────────────────────────
        EditorMsg::SimSetEnabled(true) => {
            if editor.sim.is_none() {
                let model = signex_library::SimModel::empty(
                    editor.display_internal_pn.as_str(),
                    signex_library::SimKind::Spice3,
                );
                // Bind the new primitive via PrimitiveRef. We reuse
                // the same `library_id` the symbol_ref already points
                // at — every component lives inside one library, so
                // its primitives share that library_id.
                editor.draft.sim_ref = Some(signex_library::PrimitiveRef::new(
                    editor.draft.symbol_ref.library_id,
                    model.uuid,
                ));
                editor.sim_body = Some(iced::widget::text_editor::Content::new());
                editor.sim = Some(model);
                editor.dirty = true;
            }
        }
        EditorMsg::SimSetEnabled(false) => {
            editor.sim = None;
            editor.sim_body = None;
            editor.draft.sim_ref = None;
            editor.dirty = true;
        }
        EditorMsg::SimSetKind(kind) => {
            if let Some(sim) = editor.sim.as_mut() {
                sim.kind = kind;
                sim.updated = chrono::Utc::now();
                editor.dirty = true;
            }
        }
        EditorMsg::SimSetName(name) => {
            if let Some(sim) = editor.sim.as_mut() {
                sim.name = name;
                sim.updated = chrono::Utc::now();
                editor.dirty = true;
            }
        }
        EditorMsg::SimBodyAction(action) => {
            if let Some(content) = editor.sim_body.as_mut() {
                content.perform(action);
                if let Some(sim) = editor.sim.as_mut() {
                    sim.body = content.text();
                    sim.updated = chrono::Utc::now();
                    editor.dirty = true;
                }
            }
        }
        EditorMsg::SimSetPinNode { pin_number, value } => {
            if let Some(sim) = editor.sim.as_mut() {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    sim.default_node_map.remove(&pin_number);
                } else {
                    sim.default_node_map.insert(pin_number, trimmed.to_string());
                }
                sim.updated = chrono::Utc::now();
                editor.dirty = true;
            }
        }
        EditorMsg::SaveSim(_uuid, sm) => {
            // Snapshot save — the canonical persistence path is
            // SaveDraft (which writes the Revision binding); this
            // variant is reserved for any future "save the primitive
            // only" flow, mirroring SaveSymbol / SaveFootprint.
            if let Some(stored) = editor.sim.as_mut() {
                *stored = *sm;
                editor.dirty = true;
            }
        }
        // ── /WS-L ────────────────────────────────────────────
        // Already handled in the outer match.
        EditorMsg::CloseEditor
        | EditorMsg::SaveDraft
        | EditorMsg::Commit
        | EditorMsg::SubmitForReview
        | EditorMsg::SubmitForReviewNotesChanged(_)
        | EditorMsg::SubmitForReviewCancel
        | EditorMsg::SubmitForReviewConfirm
        | EditorMsg::SubmitForReviewResult(_)
        | EditorMsg::OpenWhereUsedTab
        | EditorMsg::SymbolPickAiPdf
        | EditorMsg::SymbolPickedAiPdf(_)
        | EditorMsg::StepAttachDialog
        | EditorMsg::StepAttachResult(_) => {}
    }
}

// ── WS-K: Supply tab tests ────────────────────────────────────────────
//
// These exercise `apply_inline_edit` directly against the inline-edit
// arms added in WS-K. The dispatcher is otherwise driven through `Signex`
// (the iced application), so we hand-build a `ComponentEditorState` from
// a minimal `Component` and assert that the supply / alternates branches
// mutate `editor.draft.*` exactly as the view expects.
#[cfg(test)]
mod supply_tests {
    use super::*;
    use signex_library::{
        AlternateStatus, Component, ComponentClass, DatasheetRef, DistributorListing,
        DistributorSource, InternalPn, LifecycleState, ManufacturerPart, ParamMap, PlmReserved,
        PrimitiveRef, Revision, Version,
    };
    use std::path::PathBuf;
    use uuid::Uuid;

    fn fixture_revision() -> Revision {
        let lib = Uuid::new_v4();
        Revision {
            version: Version::new(0, 1),
            state: LifecycleState::Draft,
            created: chrono::Utc::now(),
            author: "test".into(),
            message: "seed".into(),
            symbol_ref: PrimitiveRef::new(lib, Uuid::new_v4()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
            alternates: Vec::new(),
            supply: Vec::new(),
            datasheet: DatasheetRef::default(),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            content_hash: [0u8; 32],
        }
    }

    fn fixture_editor() -> ComponentEditorState {
        let rev = fixture_revision();
        let component = Component {
            uuid: Uuid::new_v4(),
            internal_pn: InternalPn::new("R0805_10k"),
            class: ComponentClass::generic(),
            category: PathBuf::new(),
            family: None,
            head: rev.version,
            revisions: vec![rev],
        };
        ComponentEditorState::from_head(PathBuf::from("/tmp/lib"), component, false)
    }

    /// Add three alternates, then remove the middle one. The remaining
    /// two must keep their original relative order.
    #[test]
    fn supply_alternates_add_and_remove_preserve_order() {
        let mut editor = fixture_editor();

        // Add three rows.
        for _ in 0..3 {
            apply_inline_edit(&mut editor, EditorMsg::SupplyAlternateAdd);
        }
        assert_eq!(editor.draft.alternates.len(), 3);

        // Tag each row so we can verify ordering after the remove.
        apply_inline_edit(
            &mut editor,
            EditorMsg::SupplyAlternateSetMpn {
                idx: 0,
                value: "ALT-A".into(),
            },
        );
        apply_inline_edit(
            &mut editor,
            EditorMsg::SupplyAlternateSetMpn {
                idx: 1,
                value: "ALT-B".into(),
            },
        );
        apply_inline_edit(
            &mut editor,
            EditorMsg::SupplyAlternateSetMpn {
                idx: 2,
                value: "ALT-C".into(),
            },
        );

        // Remove the middle row.
        apply_inline_edit(&mut editor, EditorMsg::SupplyAlternateRemove { idx: 1 });

        assert_eq!(editor.draft.alternates.len(), 2);
        assert_eq!(editor.draft.alternates[0].mpn, "ALT-A");
        assert_eq!(editor.draft.alternates[1].mpn, "ALT-C");
        // New rows default to `Approved` (Primary is reserved for the
        // headline part), so the surviving rows should keep that.
        assert_eq!(editor.draft.alternates[0].status, AlternateStatus::Approved);
        assert!(editor.dirty);
    }

    /// Removing a distributor listing at an out-of-bounds index must be
    /// a silent no-op — guards against stale messages racing the view.
    #[test]
    fn supply_listing_remove_out_of_bounds_is_noop() {
        let mut editor = fixture_editor();

        // Seed two listings.
        apply_inline_edit(&mut editor, EditorMsg::SupplyListingAdd);
        apply_inline_edit(&mut editor, EditorMsg::SupplyListingAdd);
        apply_inline_edit(
            &mut editor,
            EditorMsg::SupplyListingSetSku {
                idx: 0,
                value: "SKU-0".into(),
            },
        );
        apply_inline_edit(
            &mut editor,
            EditorMsg::SupplyListingSetSku {
                idx: 1,
                value: "SKU-1".into(),
            },
        );

        // Snapshot the listings, clear the dirty flag, then issue an
        // out-of-bounds remove. The list and dirty flag must be unchanged.
        let snapshot: Vec<DistributorListing> = editor.draft.supply.clone();
        editor.dirty = false;
        apply_inline_edit(&mut editor, EditorMsg::SupplyListingRemove { idx: 5 });

        assert_eq!(
            editor.draft.supply, snapshot,
            "stale remove must not mutate"
        );
        assert!(!editor.dirty, "out-of-bounds remove must not flip dirty");
    }

    /// Distributor pick_list converts the `DistributorSource` enum to the
    /// canonical string stored on `DistributorListing.distributor`.
    #[test]
    fn supply_listing_set_distributor_writes_canonical_string() {
        let mut editor = fixture_editor();
        apply_inline_edit(&mut editor, EditorMsg::SupplyListingAdd);
        apply_inline_edit(
            &mut editor,
            EditorMsg::SupplyListingSetDistributor {
                idx: 0,
                value: DistributorSource::Mouser,
            },
        );
        assert_eq!(editor.draft.supply[0].distributor, "Mouser");
        assert!(editor.dirty);
    }

    /// Setting URL to an empty string clears the `Option<String>` back
    /// to `None` (matches the `notes` semantics on the primary MPN).
    #[test]
    fn supply_listing_empty_url_clears_to_none() {
        let mut editor = fixture_editor();
        apply_inline_edit(&mut editor, EditorMsg::SupplyListingAdd);
        apply_inline_edit(
            &mut editor,
            EditorMsg::SupplyListingSetUrl {
                idx: 0,
                value: "https://example.com/sku".into(),
            },
        );
        assert_eq!(
            editor.draft.supply[0].url.as_deref(),
            Some("https://example.com/sku")
        );

        apply_inline_edit(
            &mut editor,
            EditorMsg::SupplyListingSetUrl {
                idx: 0,
                value: String::new(),
            },
        );
        assert!(editor.draft.supply[0].url.is_none());
    }
}
