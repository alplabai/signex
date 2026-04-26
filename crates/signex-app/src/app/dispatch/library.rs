//! Library subsystem dispatcher. Routes
//! [`crate::library::LibraryMessage`] to the right side-effecting
//! handler. Phase 1 covers: open library, picker open/close/filter,
//! editor open/close, all the inline form fields on Overview /
//! Params / Supply, plus distributor settings.

use iced::Task;

use super::super::*;
use crate::library::commands;
use crate::library::messages::{EditorMsg, LibraryMessage, PickerMsg, SettingsMsg};
use crate::library::state::{ComponentEditorState, EditorTab, PickerState};

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
            LibraryMessage::NewComponent => {
                tracing::warn!(
                    target: "signex::library",
                    "new component flow shipped in Phase 2 — placeholder fires NoOp"
                );
                Task::none()
            }
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
                // Stash the editor state under the new window's id and
                // register the window with the multi-window dispatcher.
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
        }
    }

    fn handle_open_editor(
        &mut self,
        library_path: std::path::PathBuf,
        component_id: signex_library::ComponentId,
    ) -> Task<Message> {
        // Pre-load to fail fast if the component is missing — no point
        // opening an empty window. Drop the loaded value and let
        // `EditorWindowOpened` reload by id once the OS window is up.
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
            // Already open — Phase 2 focuses the existing window;
            // Phase 1 just bails so we don't spawn duplicates.
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
                // Resolve which library this summary came from so the
                // place flow can use it later. Linear scan — open
                // libraries are small.
                let path = self
                    .library
                    .open_libraries
                    .iter()
                    .find(|lib| lib.cached_components.iter().any(|c| c.uuid == summary.uuid))
                    .map(|lib| lib.root.clone());
                picker.selected = path.map(|p| (p, summary));
            }
            PickerMsg::PlaceSelected => {
                // Phase 1 stub — actual placement (engine integration)
                // ships in Phase 2. We log + close the modal.
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
            // ── DigiKey OAuth ────────────────────────────────────
            SettingsMsg::DigiKeyConnect => {
                if self.library.settings.digikey_in_flight {
                    return Task::none();
                }
                self.library.settings.digikey_in_flight = true;
                self.library.settings.digikey_status =
                    Some("Waiting for browser…".to_string());
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
                        self.library.settings.digikey_status =
                            Some(format!("Failed: {reason}"));
                    }
                    (None, None) => {
                        self.library.settings.digikey_status = Some("Cancelled".to_string());
                    }
                }
            }
            // ── Mouser ───────────────────────────────────────────
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
                            // Sentinel MPN per spec — known-good Yageo
                            // resistor ID that Mouser stocks.
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
                                // Persist the key on success — same
                                // path the keyring CLI uses.
                                let store =
                                    KeyringStore::for_provider("mouser", "default");
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
            // ── Preference order ────────────────────────────────
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
                        // Refresh the editor's component snapshot so
                        // History tab reflects the new revision.
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
                    // Keep `review_notes_buf` so the user doesn't
                    // lose their typing if they re-open the modal.
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

                // Snapshot everything we need before letting go of
                // the &mut borrow.
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

                // Run the save_revision call on a worker thread —
                // git2 / fs are blocking and we don't want to stall
                // the iced runtime even briefly.
                //
                // Note: the LocalGitAdapter does the `review/<uuid>`
                // branch routing automatically when
                // `manifest.workflow.review_required` is true (see
                // `signex-library/src/adapters/local_git.rs`).
                let library_root_clone = library_root.clone();
                let lib_now = self.library.library_at_mut(&library_root);
                let Some(lib) = lib_now else {
                    return Task::done(Message::Library(LibraryMessage::EditorEvent {
                        window_id,
                        msg: EditorMsg::SubmitForReviewResult(Err(
                            "library no longer open".into()
                        )),
                    }));
                };
                // Run inline — adapter is on the main state. Phase 2
                // can lift this to a worker thread once the adapter
                // implements Send + Sync wrappers properly.
                let _ = library_root_clone;
                let result = lib
                    .adapter
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
                            // Reflect the InReview state in the
                            // header bar so the user sees the
                            // transition immediately.
                            editor.draft.state =
                                signex_library::LifecycleState::InReview;
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

/// Apply an inline form edit to the editor draft. Pulled out so the
/// dispatcher's match arm stays small and the borrow scope is
/// obvious.
fn apply_inline_edit(editor: &mut ComponentEditorState, msg: EditorMsg) {
    use signex_library::{ParamValue, SupplierLink};
    match msg {
        EditorMsg::SelectTab(tab) => editor.active_tab = tab,
        EditorMsg::OverviewSetDisplayName(s) => editor.display_internal_pn = s,
        EditorMsg::OverviewSetInternalPn(s) => {
            // Phase 1 mirrors the value into both the live header and
            // the canonical Component identity. Phase 2 introduces
            // the rename-as-revision flow.
            editor.component.internal_pn = signex_library::InternalPn::new(s.clone());
            editor.display_internal_pn = s;
        }
        EditorMsg::OverviewSetMpn(s) => editor.draft.shared.mpn = s,
        EditorMsg::OverviewSetManufacturer(s) => editor.draft.shared.manufacturer = s,
        EditorMsg::OverviewSetDescription(s) => editor.draft.shared.description = s,
        EditorMsg::OverviewSetDatasheet(s) => editor.set_datasheet_url(s),
        EditorMsg::OverviewSetLifecycle(state) => editor.draft.state = state,
        EditorMsg::ParamAddRow => {
            let key = format!("param_{}", editor.draft.shared.parameters.len() + 1);
            editor
                .parameters_mut()
                .insert(key, ParamValue::Text(String::new()));
        }
        EditorMsg::ParamRemoveRow(idx) => {
            let key_to_remove = editor.draft.shared.parameters.keys().nth(idx).cloned();
            if let Some(k) = key_to_remove {
                editor.parameters_mut().remove(&k);
            }
        }
        EditorMsg::ParamSetKey { idx, key } => {
            let entries: Vec<(String, ParamValue)> = editor
                .draft
                .shared
                .parameters
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let mut rebuilt = signex_library::ParamMap::new();
            for (i, (k, v)) in entries.into_iter().enumerate() {
                if i == idx {
                    rebuilt.insert(key.clone(), v);
                } else {
                    rebuilt.insert(k, v);
                }
            }
            *editor.parameters_mut() = rebuilt;
        }
        EditorMsg::ParamSetValueText { idx, value } => {
            let key_at_idx = editor.draft.shared.parameters.keys().nth(idx).cloned();
            if let Some(k) = key_at_idx {
                editor.parameters_mut().insert(k, ParamValue::Text(value));
            }
        }
        EditorMsg::SupplyAddRow => {
            editor.supplier_links_mut().push(SupplierLink {
                distributor: String::new(),
                sku: String::new(),
                url: None,
            });
        }
        EditorMsg::SupplyRemoveRow(idx) => {
            let links = editor.supplier_links_mut();
            if idx < links.len() {
                links.remove(idx);
            }
        }
        EditorMsg::SupplySetDistributor { idx, value } => {
            if let Some(link) = editor.supplier_links_mut().get_mut(idx) {
                link.distributor = value;
            }
        }
        EditorMsg::SupplySetSku { idx, value } => {
            if let Some(link) = editor.supplier_links_mut().get_mut(idx) {
                link.sku = value;
            }
        }
        EditorMsg::SupplySetUrl { idx, value } => {
            if let Some(link) = editor.supplier_links_mut().get_mut(idx) {
                link.url = if value.trim().is_empty() {
                    None
                } else {
                    Some(value)
                };
            }
        }
        EditorMsg::SupplyPasteUrlChanged(_) => {
            // Phase 1 doesn't store the paste buffer separately —
            // Phase 2 wires the API resolution flow. No-op so the
            // text input still echoes back the user's keystroke.
        }
        EditorMsg::SupplyRefreshFromApi => {
            tracing::info!(
                target: "signex::library",
                "Refresh-from-API stub — Phase 2 wires the distributor adapter chain"
            );
        }
        EditorMsg::HistorySelectRevision(version) => {
            editor.history_selected = Some(version);
        }
        // Already handled in the outer match.
        EditorMsg::CloseEditor
        | EditorMsg::SaveDraft
        | EditorMsg::Commit
        | EditorMsg::SubmitForReview
        | EditorMsg::SubmitForReviewNotesChanged(_)
        | EditorMsg::SubmitForReviewCancel
        | EditorMsg::SubmitForReviewConfirm
        | EditorMsg::SubmitForReviewResult(_)
        | EditorMsg::OpenWhereUsedTab => {}
    }
}
