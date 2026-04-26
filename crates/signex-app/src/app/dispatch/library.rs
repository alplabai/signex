//! Library subsystem dispatcher. Routes
//! [`crate::library::LibraryMessage`] to the right side-effecting
//! handler.
//!
//! WS-E (refactor): the editor inline-edit handlers were trimmed back
//! to the bindings still present on the new `Revision` shape (Overview
//! and History only). Symbol, Footprint, 3D, Sim, and Pin Map dispatch
//! will return in WS-F and WS-G as the new editors land. The New
//! Component flow is end-to-end here for the first time.

use iced::Task;

use super::super::*;
use crate::library::commands;
use crate::library::messages::{EditorMsg, LibraryMessage, PickerMsg, SettingsMsg};
use crate::library::state::{ComponentEditorState, EditorTab, NewComponentState, PickerState};

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
            LibraryMessage::OpenEditor {
                library_path,
                component_id,
            } => self.handle_open_editor(library_path, component_id),
            LibraryMessage::EditorWindowOpened {
                library_path,
                component_id,
                window_id,
            } => {
                match commands::load_component_for_editor(
                    &mut self.library,
                    &library_path,
                    component_id,
                ) {
                    Ok(editor) => {
                        self.library.open_editors.insert(window_id, editor);
                        self.ui_state.windows.insert(
                            window_id,
                            super::super::state::WindowKind::ComponentEditor {
                                library_path,
                                component_id,
                            },
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            error = %e,
                            "failed to load component for editor; closing window"
                        );
                        return iced::window::close(window_id);
                    }
                }
                Task::none()
            }
            LibraryMessage::EditorEvent { window_id, msg } => {
                self.handle_editor_event(window_id, msg)
            }
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

    fn handle_open_editor(
        &mut self,
        library_path: std::path::PathBuf,
        component_id: signex_library::ComponentId,
    ) -> Task<Message> {
        if let Err(e) =
            commands::load_component_for_editor(&mut self.library, &library_path, component_id)
        {
            tracing::warn!(target: "signex::library", error = %e, "open editor pre-load failed");
            return Task::none();
        }
        if self
            .library
            .open_editors
            .values()
            .any(|st| st.library_root == library_path && st.component_id == component_id)
        {
            return Task::none();
        }

        let (_id, open_task) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(960.0, 720.0),
            decorations: false,
            resizable: true,
            ..Default::default()
        });
        let path_clone = library_path.clone();
        open_task.map(move |settled_id| {
            Message::Library(LibraryMessage::EditorWindowOpened {
                library_path: path_clone.clone(),
                component_id,
                window_id: settled_id,
            })
        })
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

    fn handle_editor_event(
        &mut self,
        window_id: iced::window::Id,
        msg: EditorMsg,
    ) -> Task<Message> {
        match msg {
            EditorMsg::CloseEditor => {
                self.library.open_editors.remove(&window_id);
                return iced::window::close(window_id);
            }
            EditorMsg::SaveDraft => {
                if let Err(e) = commands::save_draft(&mut self.library, window_id) {
                    tracing::warn!(target: "signex::library", error = %e, "save_draft failed");
                }
                return Task::none();
            }
            EditorMsg::Commit => {
                match commands::commit_revision(
                    &mut self.library,
                    window_id,
                    "commit (signex-app phase 1)",
                ) {
                    Ok(rev) => {
                        if let Some(editor) = self.library.open_editors.get_mut(&window_id) {
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
                if let Some(editor) = self.library.open_editors.get_mut(&window_id) {
                    editor.review_dialog_open = true;
                    editor.review_status = None;
                }
                return Task::none();
            }
            EditorMsg::SubmitForReviewNotesChanged(s) => {
                if let Some(editor) = self.library.open_editors.get_mut(&window_id) {
                    editor.review_notes_buf = s;
                }
                return Task::none();
            }
            EditorMsg::SubmitForReviewCancel => {
                if let Some(editor) = self.library.open_editors.get_mut(&window_id) {
                    editor.review_dialog_open = false;
                    editor.review_status = None;
                }
                return Task::none();
            }
            EditorMsg::SubmitForReviewConfirm => {
                let editor = match self.library.open_editors.get_mut(&window_id) {
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
                            window_id,
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
                            window_id,
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
                    window_id,
                    msg: EditorMsg::SubmitForReviewResult(result),
                }));
            }
            EditorMsg::SubmitForReviewResult(result) => {
                if let Some(editor) = self.library.open_editors.get_mut(&window_id) {
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
                if let Some(editor) = self.library.open_editors.get_mut(&window_id) {
                    editor.active_tab = EditorTab::WhereUsed;
                }
                return Task::none();
            }
            // Symbol-tab side effects — open the PDF picker, hand the
            // chosen path to a worker that runs the heuristic.
            EditorMsg::SymbolPickAiPdf => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Select datasheet PDF")
                            .add_filter("PDF", &["pdf"])
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    move |path| {
                        Message::Library(LibraryMessage::EditorEvent {
                            window_id,
                            msg: EditorMsg::SymbolPickedAiPdf(path),
                        })
                    },
                );
            }
            EditorMsg::SymbolPickedAiPdf(None) => return Task::none(),
            EditorMsg::SymbolPickedAiPdf(Some(path)) => {
                use crate::library::editor::symbol::ai_stub::AiPinoutPreview;
                let preview = match std::fs::read(&path) {
                    Ok(bytes) => AiPinoutPreview::from_pdf(&bytes),
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            error = %e,
                            path = %path.display(),
                            "failed to read datasheet PDF"
                        );
                        AiPinoutPreview::default()
                    }
                };
                if let Some(editor) = self.library.open_editors.get_mut(&window_id) {
                    editor.symbol_ai_preview = Some(preview);
                }
                return Task::none();
            }
            EditorMsg::SymbolApplyAiPreview => {
                let mut should_save = false;
                if let Some(editor) = self.library.open_editors.get_mut(&window_id)
                    && let Some(preview) = editor.symbol_ai_preview.take()
                {
                    let pins = preview.into_apply_list();
                    crate::library::editor::symbol::state::apply_ai_pinout(
                        &mut editor.symbol,
                        pins,
                    );
                    should_save = true;
                }
                if should_save {
                    save_symbol(&mut self.library, window_id);
                }
                return Task::none();
            }
            EditorMsg::SymbolDismissAiPreview => {
                if let Some(editor) = self.library.open_editors.get_mut(&window_id) {
                    editor.symbol_ai_preview = None;
                }
                return Task::none();
            }
            EditorMsg::DatasheetUploadDialog => {
                return Task::future(async move {
                    let picked = rfd::AsyncFileDialog::new()
                        .set_title("Pin Datasheet PDF")
                        .add_filter("PDF", &["pdf"])
                        .pick_file()
                        .await;
                    let resolved = match picked {
                        Some(f) => {
                            let bytes = f.read().await;
                            let filename = f
                                .file_name();
                            Some((bytes, filename))
                        }
                        None => None,
                    };
                    Message::Library(crate::library::LibraryMessage::EditorEvent {
                        window_id,
                        msg: EditorMsg::DatasheetUploadResult(resolved),
                    })
                });
            }
            EditorMsg::Model3dUploadDialog => {
                return Task::future(async move {
                    let picked = rfd::AsyncFileDialog::new()
                        .set_title("Upload 3D Model")
                        .add_filter("3D Models", &["step", "stp", "wrl", "glb", "gltf"])
                        .add_filter("STEP", &["step", "stp"])
                        .add_filter("VRML", &["wrl"])
                        .add_filter("glTF / GLB", &["glb", "gltf"])
                        .pick_file()
                        .await;
                    let resolved = match picked {
                        Some(f) => {
                            let bytes = f.read().await;
                            let filename = f.file_name();
                            Some((bytes, filename))
                        }
                        None => None,
                    };
                    Message::Library(crate::library::LibraryMessage::EditorEvent {
                        window_id,
                        msg: EditorMsg::Model3dUploadResult(resolved),
                    })
                });
            }
            // WS-F: STEP attach picker — runs `rfd`, hashes the file
            // bytes, and lands in `EditorMsg::StepAttachResult` where
            // the dispatcher writes the file under `<lib_root>/step/`.
            EditorMsg::StepAttachDialog => {
                return Task::future(async move {
                    let picked = rfd::AsyncFileDialog::new()
                        .set_title("Attach STEP file")
                        .add_filter("STEP", &["step", "stp"])
                        .pick_file()
                        .await;
                    let resolved = match picked {
                        Some(f) => {
                            let bytes = f.read().await;
                            let filename = f.file_name();
                            Some((bytes, filename))
                        }
                        None => None,
                    };
                    Message::Library(crate::library::LibraryMessage::EditorEvent {
                        window_id,
                        msg: EditorMsg::StepAttachResult(resolved),
                    })
                });
            }
            _ => {}
        }

        // All remaining variants mutate the editor in place.
        let Some(editor) = self.library.open_editors.get_mut(&window_id) else {
            return Task::none();
        };
        apply_inline_edit(editor, msg);
        Task::none()
    }
}

/// WS-F: persist the in-memory editor.symbol back into LibrarySet so
/// the next `from_head` re-read sees the new pin layout. Replaces the
/// pre-refactor `sync_symbol_sexpr` round-trip — Symbol primitives are
/// now typed, no sexpr serialisation involved.
fn save_symbol(library: &mut crate::library::state::LibraryState, window_id: iced::window::Id) {
    if let Some(editor) = library.open_editors.get_mut(&window_id) {
        editor.symbol.updated = chrono::Utc::now();
        let sym = editor.symbol.clone();
        let uuid = sym.uuid;
        // Write the binding ref so the editor doesn't lose its
        // primitive on the next reload.
        editor.draft.symbol_ref = signex_library::PrimitiveRef::new(
            editor.draft.symbol_ref.library_id,
            uuid,
        );
        library.set.save_symbol(sym);
    }
}

/// WS-F: persist the in-memory editor.footprint back into LibrarySet.
fn save_footprint(library: &mut crate::library::state::LibraryState, window_id: iced::window::Id) {
    if let Some(editor) = library.open_editors.get_mut(&window_id)
        && let Some(fp) = editor.footprint.as_mut()
    {
        fp.updated = chrono::Utc::now();
        let new_fp = fp.clone();
        let uuid = new_fp.uuid;
        editor.draft.footprint_ref = Some(signex_library::PrimitiveRef::new(
            editor
                .draft
                .footprint_ref
                .as_ref()
                .map(|r| r.library_id)
                .unwrap_or_default(),
            uuid,
        ));
        library.set.save_footprint(new_fp);
    }
}

/// WS-F-only inline-edit dispatcher.
///
/// Pre-refactor `apply_inline_edit` covered every tab. WS-E owns the
/// rebuild for Overview / Params / Supply / Sim / 3D / History; until
/// it merges, this stub handles only Tab selection plus the WS-F
/// surfaces (Symbol primitive edits, Footprint primitive edits, Body3D
/// fields). Every other variant is a no-op.
///
/// TODO(merge-with-WS-E): restore the per-tab dispatch arms against
/// the new Revision binding fields.
fn apply_inline_edit(editor: &mut ComponentEditorState, msg: EditorMsg) {
    use crate::library::editor::footprint::state::FootprintEditorState;
    match msg {
        EditorMsg::SelectTab(tab) => editor.active_tab = tab,
        EditorMsg::HistorySelectRevision(version) => {
            editor.history_selected = Some(version);
        }
        // ── Symbol-tab inline edits (WS-F) ──────────────────────
        EditorMsg::SymbolSetTool(tool) => {
            use crate::library::editor::symbol::canvas::SymbolTool;
            use crate::library::messages::SymbolToolMsg;
            editor.symbol_tool = match tool {
                SymbolToolMsg::Select => SymbolTool::Select,
                SymbolToolMsg::AddPin => SymbolTool::AddPin,
            };
        }
        EditorMsg::SymbolAddPin { x, y } => {
            let idx = crate::library::editor::symbol::state::add_pin(&mut editor.symbol, x, y);
            editor.symbol_selected = Some(
                crate::library::editor::symbol::state::SymbolSelection::Pin(idx),
            );
            editor.dirty = true;
        }
        EditorMsg::SymbolSelect(sel) => {
            use crate::library::editor::symbol::state::{FieldKey, SymbolSelection};
            use crate::library::messages::SymbolSelectionMsg;
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
            crate::library::editor::symbol::state::move_selected(
                &mut editor.symbol,
                editor.symbol_selected,
                x,
                y,
            );
            editor.dirty = true;
        }
        EditorMsg::SymbolDeleteSelected => {
            if let Some(new_sel) = crate::library::editor::symbol::state::delete_selected(
                &mut editor.symbol,
                editor.symbol_selected,
            ) {
                editor.symbol_selected = new_sel;
            }
            editor.dirty = true;
        }
        EditorMsg::SymbolSetField { .. } => {
            // WS-F: symbol fields (Designator/Value) live on the
            // *Component* binding now, not the Symbol primitive.
            // WS-E owns rebuilding the Designator + Value editor on
            // top of `Component.internal_pn` / `primary_mpn`.
            editor.dirty = true;
        }
        EditorMsg::SymbolSetPinNumber { idx, number } => {
            if let Some(p) = editor.symbol.pins.get_mut(idx) {
                p.number = number;
                editor.dirty = true;
            }
        }
        EditorMsg::SymbolSetPinName { idx, name } => {
            if let Some(p) = editor.symbol.pins.get_mut(idx) {
                p.name = name;
                editor.dirty = true;
            }
        }
        EditorMsg::SaveSymbol(uuid, sym) => {
            // WS-F: in-place primitive replacement. The dispatcher's
            // outer `save_symbol` helper writes through to LibrarySet
            // *and* updates symbol_ref.uuid; this arm just lets a
            // caller stash the new primitive for the next save.
            editor.symbol = sym;
            editor.draft.symbol_ref =
                signex_library::PrimitiveRef::new(editor.draft.symbol_ref.library_id, uuid);
            editor.dirty = true;
        }
        // ── Footprint tab ────────────────────────────────────────
        EditorMsg::FootprintAddPad { x_mm, y_mm } => {
            editor.ensure_footprint_state();
            if let (Some(canvas_state), Some(fp)) =
                (editor.footprint_state.as_mut(), editor.footprint.as_mut())
            {
                let idx = canvas_state.add_pad_at(x_mm, y_mm);
                FootprintEditorState::sync_pads_to_primitive(canvas_state, fp);
                canvas_state.selected_pad = Some(idx);
            }
            editor.invalidate_footprint_cache();
            editor.dirty = true;
        }
        EditorMsg::FootprintMovePad { idx, x_mm, y_mm } => {
            editor.ensure_footprint_state();
            if let (Some(canvas_state), Some(fp)) =
                (editor.footprint_state.as_mut(), editor.footprint.as_mut())
            {
                canvas_state.move_pad(idx, x_mm, y_mm);
                FootprintEditorState::sync_pads_to_primitive(canvas_state, fp);
            }
            editor.invalidate_footprint_cache();
            editor.dirty = true;
        }
        EditorMsg::FootprintCursorAt { x_mm, y_mm } => {
            editor.ensure_footprint_state();
            if let Some(canvas_state) = editor.footprint_state.as_mut() {
                canvas_state.cursor_mm = Some((x_mm, y_mm));
            }
        }
        EditorMsg::FootprintSelectPad(idx) => {
            editor.ensure_footprint_state();
            if let Some(canvas_state) = editor.footprint_state.as_mut() {
                canvas_state.selected_pad = idx;
            }
        }
        EditorMsg::FootprintDeleteSelected => {
            editor.ensure_footprint_state();
            if let (Some(canvas_state), Some(fp)) =
                (editor.footprint_state.as_mut(), editor.footprint.as_mut())
                && let Some(sel) = canvas_state.selected_pad
            {
                canvas_state.delete_pad(sel);
                FootprintEditorState::sync_pads_to_primitive(canvas_state, fp);
            }
            editor.invalidate_footprint_cache();
            editor.dirty = true;
        }
        EditorMsg::FootprintToggleLayer(name) => {
            editor.ensure_footprint_state();
            if let Some(canvas_state) = editor.footprint_state.as_mut()
                && let Some(layer) =
                    crate::library::editor::footprint::layers::FpLayer::from_standard_name(&name)
            {
                canvas_state.layer_visibility.toggle(layer);
            }
            editor.invalidate_footprint_cache();
        }
        EditorMsg::FootprintToggleAutoFit => {
            editor.ensure_footprint_state();
            if let Some(canvas_state) = editor.footprint_state.as_mut() {
                canvas_state.toggle_auto_fit();
            }
            editor.invalidate_footprint_cache();
            editor.dirty = true;
        }
        EditorMsg::SaveFootprint(uuid, fp) => {
            editor.footprint = Some(fp);
            editor.draft.footprint_ref = Some(signex_library::PrimitiveRef::new(
                editor
                    .draft
                    .footprint_ref
                    .as_ref()
                    .map(|r| r.library_id)
                    .unwrap_or_default(),
                uuid,
            ));
            editor.invalidate_footprint_cache();
            editor.dirty = true;
        }
        // ── Body 3D editor pane (WS-F) ──────────────────────────
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
        EditorMsg::SetBodyShape(shape) => {
            if let Some(fp) = editor.footprint.as_mut() {
                fp.body_3d.shape = shape;
                editor.dirty = true;
            }
        }
        // ── STEP attachment (WS-F) ──────────────────────────────
        EditorMsg::StepAttachResult(Some((bytes, filename))) => {
            if let Some(fp) = editor.footprint.as_mut()
                && let Some(att) = crate::library::editor::footprint::step_attach::stash_step(
                    &editor.library_root,
                    &bytes,
                    &filename,
                )
            {
                fp.step_attachment = Some(att);
                editor.dirty = true;
            }
        }
        EditorMsg::StepAttachResult(None) => {
            // User cancelled — no state change.
        }
        EditorMsg::StepAttachRemove => {
            if let Some(fp) = editor.footprint.as_mut() {
                fp.step_attachment = None;
                editor.dirty = true;
            }
        }
        // ── Stubs for messages WS-E owns ─────────────────────────
        // Variants below all hit pre-refactor SchematicSide / PcbSide
        // / SharedSide / SpiceModel surfaces. WS-E rebuilds them on top
        // of the new Revision binding fields. Until then they're
        // accepted but no-op so the message tree shape stays stable
        // and the dispatcher compiles cleanly.
        // TODO(merge-with-WS-E): restore each arm's behaviour.
        EditorMsg::OverviewSetDisplayName(s) => editor.display_internal_pn = s,
        EditorMsg::OverviewSetInternalPn(s) => {
            editor.component.internal_pn = signex_library::InternalPn::new(s.clone());
            editor.display_internal_pn = s;
        }
        EditorMsg::OverviewSetMpn(_)
        | EditorMsg::OverviewSetManufacturer(_)
        | EditorMsg::OverviewSetDescription(_)
        | EditorMsg::OverviewSetDatasheet(_)
        | EditorMsg::OverviewSetLifecycle(_)
        | EditorMsg::DatasheetSetMode(_)
        | EditorMsg::DatasheetSetUrl(_)
        | EditorMsg::DatasheetUploadResult(_)
        | EditorMsg::Model3dUploadResult(_)
        | EditorMsg::Model3dRemove
        | EditorMsg::Model3dSetOffset { .. }
        | EditorMsg::Model3dSetRotation { .. }
        | EditorMsg::ParamAddRow
        | EditorMsg::ParamRemoveRow(_)
        | EditorMsg::ParamSetKey { .. }
        | EditorMsg::ParamSetValueText { .. }
        | EditorMsg::SupplyAddRow
        | EditorMsg::SupplyRemoveRow(_)
        | EditorMsg::SupplySetDistributor { .. }
        | EditorMsg::SupplySetSku { .. }
        | EditorMsg::SupplySetUrl { .. }
        | EditorMsg::SupplyPasteUrlChanged(_)
        | EditorMsg::SupplyRefreshFromApi
        | EditorMsg::SimSetEnabled(_)
        | EditorMsg::SimBodyAction(_)
        | EditorMsg::SimSetPinNode { .. }
        | EditorMsg::SimChanged => {}
        // ── Already handled in the outer match (Task-returning
        // arms or modal flows). ─────────────────────────────────
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
        | EditorMsg::SymbolApplyAiPreview
        | EditorMsg::SymbolDismissAiPreview
        | EditorMsg::DatasheetUploadDialog
        | EditorMsg::Model3dUploadDialog
        | EditorMsg::StepAttachDialog => {}
    }
}
