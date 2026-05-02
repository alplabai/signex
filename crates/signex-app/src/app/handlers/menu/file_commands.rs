use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_menu_file_command(&mut self, msg: &MenuMessage) -> Option<Task<Message>> {
        match msg {
            MenuMessage::OpenProject => Some(Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Open Project or Schematic")
                        .add_filter("Signex Project", &["snxprj"])
                        .add_filter("Signex Schematic", &["snxsch"])
                        .add_filter("All Supported", &["snxprj", "snxsch"])
                        .add_filter("All files", &["*"])
                        .pick_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                },
                Message::FileOpened,
            )),
            MenuMessage::Save => Some(self.update(Message::SaveFile)),
            MenuMessage::SaveAs => Some(Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Save Schematic As")
                        .add_filter("Signex Schematic", &["snxsch"])
                        .save_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                },
                |path| path.map(Message::SaveFileAs).unwrap_or(Message::Noop),
            )),
            MenuMessage::NewProject => Some(Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("New Signex Project")
                        .set_file_name("Untitled.snxprj")
                        .add_filter("Signex Project", &["snxprj"])
                        .save_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                },
                Message::NewProjectFile,
            )),
            MenuMessage::PrintPreview => Some(self.update(Message::PrintPreviewRequested)),
            MenuMessage::ExportPdf => Some(self.update(Message::ExportPdfOpenDialog)),
            MenuMessage::ExportNetlist => self.handle_export_netlist_requested(),
            MenuMessage::ExportBom => Some(self.handle_bom_preview_open()),
            MenuMessage::LibraryOpenLibrary => Some(self.update(Message::Library(
                crate::library::LibraryMessage::OpenLibraryDialog,
            ))),
            MenuMessage::LibraryPlaceComponent => {
                Some(self.update(Message::Library(crate::library::LibraryMessage::OpenPicker)))
            }
            MenuMessage::LibraryNewComponent => Some(self.update(Message::Library(
                crate::library::LibraryMessage::NewComponent,
            ))),
            MenuMessage::AddComponentLibrary => {
                let path = self.document_state.active_project.and_then(|id| {
                    self.document_state
                        .projects
                        .iter()
                        .find(|p| p.id == id)
                        .map(|p| p.path.clone())
                });
                match path {
                    Some(path) => Some(self.update(Message::Library(
                        crate::library::LibraryMessage::CreateLibraryAt(path),
                    ))),
                    None => {
                        tracing::warn!(
                            target: "signex::library",
                            "Add Component Library: no active project to attach to"
                        );
                        Some(iced::Task::none())
                    }
                }
            }
            // Library node → Add New ▸ Component fires through the
            // existing New Component modal flow. Symbol / Footprint
            // mint a fresh primitive directly through the mounted
            // adapter (no modal — the new file opens in its own
            // standalone editor tab).
            MenuMessage::AddLibraryComponent => Some(self.update(Message::Library(
                crate::library::LibraryMessage::NewComponent,
            ))),
            MenuMessage::AddLibrarySymbol => {
                Some(self.handle_add_library_primitive(signex_library::PrimitiveKind::Symbol))
            }
            MenuMessage::AddLibraryFootprint => {
                Some(self.handle_add_library_primitive(signex_library::PrimitiveKind::Footprint))
            }
            MenuMessage::ToolsNewPart => self.dispatch_active_symbol_primitive_event(
                crate::library::messages::PrimitiveEditorMsg::SymbolNewPart,
            ),
            MenuMessage::ToolsRemovePart => self.dispatch_active_symbol_primitive_event(
                crate::library::messages::PrimitiveEditorMsg::SymbolRemovePart,
            ),
            MenuMessage::ToolsDocumentOptions => {
                // Resolve the active tab's containing `.snxlib` and
                // open the modal against its library_path. No-op on
                // non-primitive tabs (Altium-style "menu greys out
                // when not applicable" — modeled here as silent
                // no-op since MenuContext doesn't carry a SchLib
                // flag yet).
                let path = self
                    .document_state
                    .tabs
                    .get(self.document_state.active_tab)
                    .and_then(|t| match &t.kind {
                        crate::app::TabKind::SymbolEditor(p)
                        | crate::app::TabKind::FootprintEditor(p) => Some(p.clone()),
                        _ => None,
                    });
                let library_path = path.and_then(|p| {
                    self.library
                        .containing_library(&p)
                        .map(|lib| lib.root.clone())
                });
                library_path.map(|library_path| {
                    self.update(Message::Library(
                        crate::library::LibraryMessage::OpenDocumentOptions { library_path },
                    ))
                })
            }
            _ => None,
        }
    }

    /// Resolve the active tab; if it's a `.snxsym` standalone editor
    /// fire `msg` against its `path`. Returns `None` when no Symbol
    /// editor is active so the menu item silently no-ops on other
    /// tab kinds (mirrors `MenuMessage::Save`-style guards).
    fn dispatch_active_symbol_primitive_event(
        &mut self,
        msg: crate::library::messages::PrimitiveEditorMsg,
    ) -> Option<Task<Message>> {
        let path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| match &t.kind {
                crate::app::TabKind::SymbolEditor(p) => Some(p.clone()),
                _ => None,
            })?;
        Some(self.update(Message::Library(
            crate::library::LibraryMessage::PrimitiveEditorEvent { path, msg },
        )))
    }

    /// Right-click → Add New ▸ Symbol / Footprint. Resolves the
    /// clicked library from `project_tree_context_menu`, mints an
    /// empty primitive via the adapter (which writes the JSON file
    /// under `<library>/symbols|footprints/<uuid>.snx{sym,fpt}` and
    /// commits), refreshes the project tree so the new file appears,
    /// and opens the file as a standalone primitive-editor tab.
    fn handle_add_library_primitive(
        &mut self,
        kind: signex_library::PrimitiveKind,
    ) -> Task<Message> {
        use signex_library::PrimitiveKind;

        // The right-clicked tree path lives on `project_tree_context_menu`.
        // Library leaves sit at depth 3 (`[project_idx, libraries_idx, library_idx]`)
        // — see `view_project_tree_context_menu` for the same gate.
        let tree_path = match self
            .interaction_state
            .project_tree_context_menu
            .as_ref()
            .and_then(|m| m.path.clone())
        {
            Some(p) if p.len() == 3 => p,
            _ => {
                tracing::warn!(
                    target: "signex::library",
                    ?kind,
                    "Add Library primitive: no library node in context menu state"
                );
                return Task::none();
            }
        };

        let project_idx = tree_path[0];
        let library_idx = tree_path[2];

        let resolved_root = match self
            .document_state
            .projects
            .get(project_idx)
            .and_then(|loaded| {
                loaded
                    .data
                    .libraries
                    .get(library_idx)
                    .map(|entry| loaded.data.resolve_library_path(entry))
            }) {
            Some(p) => p,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    project_idx,
                    library_idx,
                    "Add Library primitive: cannot resolve library entry"
                );
                return Task::none();
            }
        };

        // `resolved_root` is the `.snxlib` *file* path (it's the
        // library's identity). For the on-disk `symbols/` /
        // `footprints/` siblings we need the file's *parent* dir —
        // grab it via `OpenLibrary::root_dir()` so a future shift in
        // how `root` is laid out only has to touch that helper.
        let (library_id, disk_root) = match self.library.library_at(&resolved_root) {
            Some(lib) => match lib.root_dir() {
                Some(dir) => (lib.library_id, dir.to_path_buf()),
                None => {
                    tracing::warn!(
                        target: "signex::library",
                        root = %resolved_root.display(),
                        "Add Library primitive: library has no parent dir"
                    );
                    return Task::none();
                }
            },
            None => {
                tracing::warn!(
                    target: "signex::library",
                    root = %resolved_root.display(),
                    "Add Library primitive: library not mounted"
                );
                return Task::none();
            }
        };

        let adapter = match self.library.set.get(library_id) {
            Some(a) => a,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    %library_id,
                    "Add Library primitive: adapter not in LibrarySet"
                );
                return Task::none();
            }
        };

        // We deliberately do NOT call `adapter.save_symbol` /
        // `save_footprint` here — that's what was auto-persisting
        // every "Add New ▸ Symbol" click to disk before the user had
        // a chance to discard or rename the new primitive. Instead,
        // build the editor state in memory (dirty = true) and let the
        // user's explicit Save be the first disk write. The adapter is
        // dropped to release the borrow on `self.library` so the
        // `handle_open_*_in_memory` calls below can mutate
        // `self.document_state` freely.
        let _ = (adapter, library_id); // intentionally unused after this point
        match kind {
            PrimitiveKind::Symbol => {
                let target = self.unique_new_symbol_path(&disk_root, "NewSymbol");
                let sym = signex_library::Symbol::empty("NewSymbol");
                let file = signex_library::SymbolFile::from_symbol(sym);

                self.interaction_state.project_tree_context_menu = None;
                self.interaction_state.context_submenu = None;
                self.handle_open_new_symbol_in_memory(target, file);
                Task::none()
            }
            PrimitiveKind::Footprint => {
                let target = self.unique_new_footprint_path(&disk_root, "NewFootprint");
                let fp = signex_library::Footprint::empty("NewFootprint");

                self.interaction_state.project_tree_context_menu = None;
                self.interaction_state.context_submenu = None;
                self.handle_open_new_footprint_in_memory(target, fp);
                Task::none()
            }
            PrimitiveKind::Sim => {
                tracing::warn!(
                    target: "signex::library",
                    "Add Library primitive: Sim creation not wired from this menu"
                );
                Task::none()
            }
            _ => {
                tracing::warn!(
                    target: "signex::library",
                    ?kind,
                    "Add Library primitive: unsupported PrimitiveKind variant"
                );
                Task::none()
            }
        }
    }

    /// Pick a path under `<root>/symbols/` that doesn't collide with
    /// an existing file on disk OR an in-memory editor tab. Tries
    /// `<base>.snxsym` first, then `<base>-2.snxsym`, `<base>-3`, etc.
    fn unique_new_symbol_path(&self, root: &std::path::Path, base: &str) -> std::path::PathBuf {
        let dir = root.join("symbols");
        let mut name = format!("{base}.snxsym");
        let mut path = dir.join(&name);
        if !self.path_in_use(&path) {
            return path;
        }
        for n in 2..=999 {
            name = format!("{base}-{n}.snxsym");
            path = dir.join(&name);
            if !self.path_in_use(&path) {
                return path;
            }
        }
        // 999 collisions is silly — fall through with the last
        // candidate; the user will see it overwrite something obvious.
        path
    }

    /// Counterpart to `unique_new_symbol_path` for footprints.
    fn unique_new_footprint_path(&self, root: &std::path::Path, base: &str) -> std::path::PathBuf {
        let dir = root.join("footprints");
        let mut name = format!("{base}.snxfpt");
        let mut path = dir.join(&name);
        if !self.path_in_use(&path) {
            return path;
        }
        for n in 2..=999 {
            name = format!("{base}-{n}.snxfpt");
            path = dir.join(&name);
            if !self.path_in_use(&path) {
                return path;
            }
        }
        path
    }

    fn path_in_use(&self, path: &std::path::Path) -> bool {
        path.exists()
            || self.document_state.symbol_editors.contains_key(path)
            || self.document_state.footprint_editors.contains_key(path)
    }

    /// Open `file` as a new in-memory `.snxsym` editor tab at `path`.
    /// The file does NOT exist on disk yet — `dirty` is set so the
    /// next user-Save writes it via the standard `save_primitive_tab_at`
    /// path (which `atomic_write`s, creating the parent dirs). Mirrors
    /// the disk-loading branch of `handle_open_primitive` minus the
    /// `std::fs::read` step.
    fn handle_open_new_symbol_in_memory(
        &mut self,
        path: std::path::PathBuf,
        file: signex_library::SymbolFile,
    ) {
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

        let mut state = crate::app::SymbolEditorState::new(path.clone(), file);
        state.dirty = true;
        self.document_state
            .symbol_editors
            .insert(path.clone(), state);
        // Track the dirty path so the project-close prompt picks it up
        // alongside other unsaved tabs.
        self.document_state.dirty_paths.insert(path.clone());

        self.park_active_schematic_session();
        self.document_state.tabs.push(crate::app::TabInfo {
            title,
            path: path.clone(),
            cached_document: None,
            dirty: true,
            project_id,
            kind: crate::app::TabKind::SymbolEditor(path),
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        self.document_state.active_path = None;
        self.refresh_panel_ctx();
    }

    /// Footprint counterpart to `handle_open_new_symbol_in_memory`.
    fn handle_open_new_footprint_in_memory(
        &mut self,
        path: std::path::PathBuf,
        primitive: signex_library::Footprint,
    ) {
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| primitive.name.clone());
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);

        let mut state = crate::app::FootprintEditorState::new(path.clone(), primitive);
        state.dirty = true;
        self.document_state
            .footprint_editors
            .insert(path.clone(), state);
        self.document_state.dirty_paths.insert(path.clone());

        self.park_active_schematic_session();
        self.document_state.tabs.push(crate::app::TabInfo {
            title,
            path: path.clone(),
            cached_document: None,
            dirty: true,
            project_id,
            kind: crate::app::TabKind::FootprintEditor(path),
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        self.document_state.active_path = None;
        self.refresh_panel_ctx();
    }
}

/// Walk `<library_root>/symbols/*.snxsym` and return the path of the
/// first file whose `SymbolFile` container holds the given symbol uuid.
/// Used by `Add New ▸ Symbol` to discover where the adapter wrote the
/// fresh symbol — the adapter slugifies `Symbol::name` so we can't
/// predict the filename ahead of time. Returns `None` when no file
/// owns the uuid (caller falls back to a best-guess path for the
/// editor tab; the open-primitive flow will then error with a
/// `tracing::warn` instead of crashing).
fn find_symbol_file_for_uuid(
    library_root: &std::path::Path,
    uuid: uuid::Uuid,
) -> Option<std::path::PathBuf> {
    let dir = library_root.join("symbols");
    let entries = std::fs::read_dir(&dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("snxsym") {
            continue;
        }
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if let Ok(file) = signex_library::SymbolFile::from_json(&bytes)
            && file.symbols.iter().any(|s| s.uuid == uuid)
        {
            return Some(path);
        }
    }
    None
}
