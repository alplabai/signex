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
                            slot.error =
                                Some("Pick a target library before submitting.".into());
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
        let Some(loaded) = self
            .document_state
            .projects
            .iter_mut()
            .find(|p| p.path == project_root || p.path.parent() == Some(project_root.as_path()))
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
        component_id: signex_library::ComponentId,
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
        let editor =
            match commands::load_component_for_editor(&mut self.library, &library_path, component_id) {
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
                let path = self
                    .library
                    .open_libraries
                    .iter()
                    .find(|lib| lib.cached_components.iter().any(|c| c.uuid == summary.uuid))
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
                        Message::Library(LibraryMessage::Settings(
                            SettingsMsg::MouserTestResult(result),
                        ))
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
    fn handle_editor_event(
        &mut self,
        address: EditorAddress,
        msg: EditorMsg,
    ) -> Task<Message> {
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
                match commands::commit_revision(
                    &mut self.library,
                    &address,
                    "commit (signex-app phase 1)",
                ) {
                    Ok(rev) => {
                        if let Some(editor) = self.library.editors.get_mut(&address) {
                            editor.component.revisions.push(rev.clone());
                            editor.component.revisions.sort_by_key(|r| r.version);
                            editor.component.head = rev.version;
                            editor.displayed_version = rev.version;
                            editor.history_selected = Some(rev.version);
                        }
                    }
                    Err(e) => {
                        tracing::warn!(target: "signex::library", error = %e, "commit failed");
                    }
                }
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
                let editor = match self.library.editors.get_mut(&address) {
                    Some(e) => e,
                    None => return Task::none(),
                };
                if editor.review_in_flight {
                    return Task::none();
                }
                editor.review_in_flight = true;
                editor.review_status = Some("Submitting…".to_string());

                let library_root = editor.library_root.clone();
                let component_id = editor.component_id;
                let mut revision = editor.draft.clone();
                revision.state = signex_library::LifecycleState::InReview;
                revision.refresh_content_hash();
                let message = if editor.review_notes_buf.trim().is_empty() {
                    format!("submit for review: {}", editor.display_internal_pn)
                } else {
                    format!(
                        "submit for review: {}\n\n{}",
                        editor.display_internal_pn,
                        editor.review_notes_buf.trim()
                    )
                };

                let library_id = match self.library.library_at(&library_root) {
                    Some(lib) => lib.library_id,
                    None => {
                        return Task::done(Message::Library(LibraryMessage::EditorEvent {
                            library_path: address.library_path.clone(),
                            component_id: address.component_id,
                            msg: EditorMsg::SubmitForReviewResult(Err(
                                "library no longer open".into(),
                            )),
                        }));
                    }
                };
                let adapter = match self.library.set.adapter(library_id) {
                    Some(a) => a,
                    None => {
                        return Task::done(Message::Library(LibraryMessage::EditorEvent {
                            library_path: address.library_path.clone(),
                            component_id: address.component_id,
                            msg: EditorMsg::SubmitForReviewResult(Err(
                                "library not mounted".into()
                            )),
                        }));
                    }
                };
                let result = adapter
                    .save_revision(component_id, revision, &message)
                    .map_err(|e| e.to_string());
                return Task::done(Message::Library(LibraryMessage::EditorEvent {
                    library_path: address.library_path,
                    component_id: address.component_id,
                    msg: EditorMsg::SubmitForReviewResult(result),
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
                                let filename = handle
                                    .file_name();
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
                    let resolved =
                        self.library.set.resolve_symbol(&editor.draft.symbol_ref);
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
            _ => {}
        }
    }

    /// WS-F2: package the picked PDF bytes into the AI-stub heuristic
    /// preview so the view can render the apply/cancel card.
    fn handle_symbol_picked_ai_pdf(
        &mut self,
        address: &EditorAddress,
        payload: Option<Vec<u8>>,
    ) {
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
fn apply_inline_edit(editor: &mut ComponentEditorState, msg: EditorMsg) {
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
            editor.draft.primary_mpn.notes =
                if s.trim().is_empty() { None } else { Some(s) };
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
                && let Some(new_sel) =
                    crate::library::editor::symbol::state::delete_selected(
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
                    crate::library::editor::footprint::state::FootprintEditorState::from_footprint(fp),
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
                && let Some(layer) = crate::library::editor::footprint::layers::FpLayer::from_standard_name(&name)
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
                    crate::library::editor::footprint::state::FootprintEditorState::from_footprint(&fp),
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
        // ── WS-J: Params tab ──────────────────────────────────────
        EditorMsg::ParamSetText { name, value } => {
            use signex_library::ParamValue;
            editor
                .draft
                .parameters
                .insert(name, ParamValue::Text(value));
            editor.dirty = true;
        }
        EditorMsg::ParamSetNumberBuf { name, buf } => {
            // Numeric edits stage in the live buffer; commit happens on
            // blur / Enter via `ParamCommitNumber`.
            editor.params_edit_buf.insert(name, buf);
        }
        EditorMsg::ParamCommitNumber { name } => {
            use signex_library::ParamValue;
            if let Some(buf) = editor.params_edit_buf.get(&name) {
                let trimmed = buf.trim();
                if trimmed.is_empty() {
                    // Empty buffer = remove the parameter entirely.
                    editor.draft.parameters.remove(&name);
                    editor.params_edit_buf.remove(&name);
                    editor.dirty = true;
                } else if let Ok(n) = trimmed.parse::<f64>() {
                    editor
                        .draft
                        .parameters
                        .insert(name.clone(), ParamValue::Number(n));
                    // Keep the buffer in sync so the display matches.
                    editor.params_edit_buf.insert(name, n.to_string());
                    editor.dirty = true;
                }
                // Bad parse: leave the buffer dirty so the user sees
                // their text and can fix the typo without losing it.
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
                // Already a row — don't clobber an existing value.
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
        // ── /WS-J ──────────────────────────────────────────────────
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
