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
            LibraryMessage::Noop => Task::none(),
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
                if let Some(editor) = self.library.open_editors.get_mut(&window_id)
                    && let Some(preview) = editor.symbol_ai_preview.take()
                {
                    let pins = preview.into_apply_list();
                    editor.symbol_doc.apply_ai_pinout(pins);
                    sync_symbol_sexpr(editor);
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

/// Round-trip the editable [`SymbolDoc`] back into
/// `SchematicSide.symbol.sexpr`. Called after every symbol-edit so
/// `Save Draft` / `Commit` see the current canvas state without an
/// explicit "save symbol" affordance.
fn sync_symbol_sexpr(editor: &mut ComponentEditorState) {
    editor.draft.schematic.symbol.sexpr = editor.symbol_doc.to_sexpr();
}

/// Apply an inline form edit to the editor draft. Pulled out so the
/// dispatcher's match arm stays small and the borrow scope is
/// obvious.
fn apply_inline_edit(editor: &mut ComponentEditorState, msg: EditorMsg) {
    use crate::library::editor::sim;
    use crate::library::editor::three_d::{Model3dUploadInfo, hash_bytes_hex, is_supported_extension};
    use signex_library::{ModelRef, ParamValue, SpiceModel, SupplierLink};
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
        EditorMsg::DatasheetSetMode(mode) => editor.set_datasheet_mode(mode),
        EditorMsg::DatasheetSetUrl(s) => editor.set_datasheet_url(s),
        EditorMsg::DatasheetUploadResult(Some((bytes, filename))) => {
            let hash = hash_bytes_hex(&bytes);
            tracing::warn!(
                target: "signex::library",
                bytes = bytes.len(),
                filename = %filename,
                "datasheet PDF storage shipped in WS-A's local-git adapter — not wired to UI yet"
            );
            editor.set_datasheet_pinned(hash, filename);
        }
        EditorMsg::DatasheetUploadResult(None) => {
            // User cancelled — no state change.
        }
        EditorMsg::Model3dUploadResult(Some((bytes, filename))) => {
            let extension = filename
                .rsplit_once('.')
                .map(|(_, e)| e.to_ascii_lowercase())
                .unwrap_or_default();
            if !is_supported_extension(&extension) {
                tracing::warn!(
                    target: "signex::library",
                    filename = %filename,
                    extension = %extension,
                    "rejected 3D upload — unsupported extension"
                );
                return;
            }
            let hash = hash_bytes_hex(&bytes);
            let info = Model3dUploadInfo {
                filename: filename.clone(),
                hash_hex: hash.clone(),
                size_bytes: bytes.len() as u64,
                extension: extension.clone(),
            };
            let path = info.storage_path();
            tracing::warn!(
                target: "signex::library",
                bytes = bytes.len(),
                path = %path,
                "3D model storage shipped in WS-A's local-git adapter — not wired to UI yet"
            );
            // Preserve any offset/rotation the user pre-set on the
            // empty-path placeholder grid.
            let prev_offset = editor.draft.pcb.model_3d.as_ref().map(|m| m.offset);
            let prev_rotation = editor.draft.pcb.model_3d.as_ref().map(|m| m.rotation);
            let model = ModelRef {
                path,
                offset: prev_offset.unwrap_or([0.0; 3]),
                rotation: prev_rotation.unwrap_or([0.0; 3]),
            };
            editor.set_model_3d(Some((model, info)));
        }
        EditorMsg::Model3dUploadResult(None) => {
            // User cancelled — no state change.
        }
        EditorMsg::Model3dRemove => {
            editor.set_model_3d(None);
        }
        EditorMsg::Model3dSetOffset { axis, value } => {
            if axis < 3
                && let Some(m) = editor.draft.pcb.model_3d.as_mut()
            {
                m.offset[axis] = value;
            }
        }
        EditorMsg::Model3dSetRotation { axis, value } => {
            if axis < 3
                && let Some(m) = editor.draft.pcb.model_3d.as_mut()
            {
                m.rotation[axis] = value;
            }
        }
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
        // ── Symbol-tab inline edits ─────────────────────────────
        EditorMsg::SymbolSetTool(tool) => {
            use crate::library::editor::symbol::canvas::SymbolTool;
            use crate::library::messages::SymbolToolMsg;
            editor.symbol_tool = match tool {
                SymbolToolMsg::Select => SymbolTool::Select,
                SymbolToolMsg::AddPin => SymbolTool::AddPin,
            };
        }
        EditorMsg::SymbolAddPin { x, y } => {
            let idx = editor.symbol_doc.add_pin(x, y);
            editor.symbol_doc.selected = Some(
                crate::library::editor::symbol::state::SymbolSelection::Pin(idx),
            );
            sync_symbol_sexpr(editor);
        }
        EditorMsg::SymbolSelect(sel) => {
            use crate::library::editor::symbol::state::{FieldKey, SymbolSelection};
            use crate::library::messages::SymbolSelectionMsg;
            editor.symbol_doc.selected = Some(match sel {
                SymbolSelectionMsg::Pin(idx) => SymbolSelection::Pin(idx),
                SymbolSelectionMsg::FieldReference => SymbolSelection::Field(FieldKey::Reference),
                SymbolSelectionMsg::FieldValue => SymbolSelection::Field(FieldKey::Value),
            });
        }
        EditorMsg::SymbolDeselect => {
            editor.symbol_doc.selected = None;
        }
        EditorMsg::SymbolMoveSelected { x, y } => {
            editor.symbol_doc.move_selected(x, y);
            sync_symbol_sexpr(editor);
        }
        EditorMsg::SymbolDeleteSelected => {
            editor.symbol_doc.delete_selected();
            sync_symbol_sexpr(editor);
        }
        EditorMsg::SymbolSetField { key, value } => {
            use crate::library::editor::symbol::state::FieldKey;
            use crate::library::messages::FieldKeyMsg;
            let key = match key {
                FieldKeyMsg::Reference => FieldKey::Reference,
                FieldKeyMsg::Value => FieldKey::Value,
            };
            editor.symbol_doc.set_field_value(key, value);
            sync_symbol_sexpr(editor);
        }
        EditorMsg::SymbolSetPinNumber { idx, number } => {
            editor.symbol_doc.set_pin_number(idx, number);
            sync_symbol_sexpr(editor);
        }
        EditorMsg::SymbolSetPinName { idx, name } => {
            editor.symbol_doc.set_pin_name(idx, name);
            sync_symbol_sexpr(editor);
        }
        EditorMsg::SymbolEdited(sexpr) => {
            editor.draft.schematic.symbol.sexpr = sexpr;
        }
        // ── Sim-tab inline edits ────────────────────────────────
        EditorMsg::SimSetEnabled(on) => {
            if on {
                // Re-derive pin numbers from the current symbol body
                // so a freshly-edited symbol always seeds the right
                // skeleton when the toggle is flipped on.
                let pins = sim::extract_pin_numbers(&editor.draft.schematic.symbol.sexpr);
                editor.sim.pin_numbers = pins;
                let body = editor.sim.body_text();
                let pin_map = editor
                    .draft
                    .shared
                    .simulation
                    .as_ref()
                    .map(|m| m.pin_map.clone())
                    .unwrap_or_else(|| sim::seed_empty_pin_map(&editor.sim.pin_numbers));
                editor.draft.shared.simulation = Some(SpiceModel { body, pin_map });
            } else {
                editor.draft.shared.simulation = None;
            }
        }
        EditorMsg::SimBodyAction(action) => {
            editor.sim.body.perform(action);
            // Mirror the new text back into the canonical model so a
            // Save Draft / Commit captures the body verbatim.
            if let Some(model) = editor.draft.shared.simulation.as_mut() {
                model.body = editor.sim.body_text();
            }
        }
        EditorMsg::SimSetPinNode { pin_number, value } => {
            // Toggling the model on is implicit — typing into a row
            // means the user wants a model. Avoids surprise behaviour
            // when the user starts mapping pins before checking the box.
            if editor.draft.shared.simulation.is_none() {
                editor.draft.shared.simulation = Some(SpiceModel {
                    body: editor.sim.body_text(),
                    pin_map: Default::default(),
                });
                if editor.sim.pin_numbers.is_empty() {
                    editor.sim.pin_numbers =
                        sim::extract_pin_numbers(&editor.draft.schematic.symbol.sexpr);
                }
            }
            if let Some(model) = editor.draft.shared.simulation.as_mut() {
                model.pin_map = sim::apply_pin_node_edit(&model.pin_map, &pin_number, value);
            }
        }
        EditorMsg::SimChanged(model) => {
            // Keep the live editor's body widget aligned with the
            // canonical model — used by paste-from-template / reset.
            if editor.sim.body_text() != model.body {
                editor.sim.body = iced::widget::text_editor::Content::with_text(&model.body);
            }
            editor.draft.shared.simulation = Some(model);
        }
        // Already handled in the outer match.
        EditorMsg::CloseEditor
        | EditorMsg::SaveDraft
        | EditorMsg::Commit
        | EditorMsg::SubmitForReview
        | EditorMsg::OpenWhereUsedTab
        | EditorMsg::SymbolPickAiPdf
        | EditorMsg::SymbolPickedAiPdf(_)
        | EditorMsg::SymbolApplyAiPreview
        | EditorMsg::SymbolDismissAiPreview
        | EditorMsg::DatasheetUploadDialog
        | EditorMsg::Model3dUploadDialog => {}
    }
}
