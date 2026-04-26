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
        match msg {
            SettingsMsg::DigiKeyConnect => {
                tracing::info!(
                    target: "signex::library",
                    "DigiKey OAuth flow stub — Phase 2 wires the real PKCE flow"
                );
            }
            SettingsMsg::MouserApiKeyChanged(s) => {
                self.library.settings.mouser_api_key_buf = s;
            }
            SettingsMsg::MouserTest => {
                let len = self.library.settings.mouser_api_key_buf.len();
                self.library.settings.mouser_status = Some(if len == 0 {
                    "Cannot test — paste an API key first.".to_string()
                } else {
                    format!("Phase 1 stub — would test key (len = {len}) against Mouser API.")
                });
            }
            SettingsMsg::PreferenceUp(src) => {
                let order = &mut self.library.settings.preferred_order;
                if let Some(i) = order.iter().position(|s| *s == src)
                    && i > 0
                {
                    order.swap(i, i - 1);
                }
            }
            SettingsMsg::PreferenceDown(src) => {
                let order = &mut self.library.settings.preferred_order;
                if let Some(i) = order.iter().position(|s| *s == src)
                    && i + 1 < order.len()
                {
                    order.swap(i, i + 1);
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
                tracing::info!(
                    target: "signex::library",
                    "submit-for-review flow stub — Phase 2 wires the review request UI"
                );
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
        // ── Footprint tab ─────────────────────────────────────
        EditorMsg::FootprintAddPad { x_mm, y_mm } => {
            editor.ensure_footprint_state();
            if let Some(fp) = editor.footprint_state.as_mut() {
                fp.add_pad_at(x_mm, y_mm);
            }
            editor.flush_footprint_to_draft();
        }
        EditorMsg::FootprintMovePad { idx, x_mm, y_mm } => {
            editor.ensure_footprint_state();
            if let Some(fp) = editor.footprint_state.as_mut() {
                fp.move_pad(idx, x_mm, y_mm);
            }
            editor.flush_footprint_to_draft();
        }
        EditorMsg::FootprintCursorAt { x_mm, y_mm } => {
            editor.ensure_footprint_state();
            if let Some(fp) = editor.footprint_state.as_mut() {
                fp.cursor_mm = Some((x_mm, y_mm));
            }
        }
        EditorMsg::FootprintSelectPad(idx) => {
            editor.ensure_footprint_state();
            if let Some(fp) = editor.footprint_state.as_mut() {
                fp.selected_pad = idx;
            }
        }
        EditorMsg::FootprintDeleteSelected => {
            editor.ensure_footprint_state();
            if let Some(fp) = editor.footprint_state.as_mut()
                && let Some(sel) = fp.selected_pad
            {
                fp.delete_pad(sel);
            }
            editor.flush_footprint_to_draft();
        }
        EditorMsg::FootprintToggleLayer(name) => {
            editor.ensure_footprint_state();
            if let Some(fp) = editor.footprint_state.as_mut()
                && let Some(layer) =
                    crate::library::editor::footprint::layers::FpLayer::from_standard_name(&name)
            {
                fp.layer_visibility.toggle(layer);
            }
        }
        EditorMsg::FootprintToggleAutoFit => {
            editor.ensure_footprint_state();
            if let Some(fp) = editor.footprint_state.as_mut() {
                fp.toggle_auto_fit();
            }
            editor.flush_footprint_to_draft();
        }
        EditorMsg::FootprintEdited(sexpr) => {
            editor.draft.pcb.footprint.sexpr = sexpr.clone();
            editor.footprint_state = Some(
                crate::library::editor::footprint::state::FootprintEditorState::from_sexpr(
                    &sexpr,
                ),
            );
            editor.footprint_canvas_cache = std::sync::OnceLock::new();
        }
        // Already handled in the outer match.
        EditorMsg::CloseEditor
        | EditorMsg::SaveDraft
        | EditorMsg::Commit
        | EditorMsg::SubmitForReview
        | EditorMsg::OpenWhereUsedTab => {}
    }
}
