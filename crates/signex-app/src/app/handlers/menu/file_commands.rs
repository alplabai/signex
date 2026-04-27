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
                        .add_filter("Standard Schematic", &["standard_sch"])
                        .add_filter("Standard Project", &["standard_pro"])
                        .add_filter(
                            "All Supported",
                            &["snxprj", "snxsch", "standard_sch", "standard_pro"],
                        )
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
                        .add_filter("Standard Schematic", &["standard_sch"])
                        .save_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                },
                |path| path.map(Message::SaveFileAs).unwrap_or(Message::Noop),
            )),
            MenuMessage::NewProject => Some(Task::none()),
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
            _ => None,
        }
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

        let resolved_root = match self.document_state.projects.get(project_idx).and_then(
            |loaded| {
                loaded
                    .data
                    .libraries
                    .get(library_idx)
                    .map(|entry| loaded.data.resolve_library_path(entry))
            },
        ) {
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

        let library_id = match self.library.library_at(&resolved_root) {
            Some(lib) => lib.library_id,
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

        let (new_path, save_result) = match kind {
            PrimitiveKind::Symbol => {
                let sym = signex_library::Symbol::empty("NewSymbol");
                let uuid = sym.uuid;
                let path = resolved_root.join("symbols").join(format!("{uuid}.snxsym"));
                let result = adapter.save_symbol(sym, "add new symbol");
                (path, result)
            }
            PrimitiveKind::Footprint => {
                let fp = signex_library::Footprint::empty("NewFootprint");
                let uuid = fp.uuid;
                let path = resolved_root
                    .join("footprints")
                    .join(format!("{uuid}.snxfpt"));
                let result = adapter.save_footprint(fp, "add new footprint");
                (path, result)
            }
            PrimitiveKind::Sim => {
                tracing::warn!(
                    target: "signex::library",
                    "Add Library primitive: Sim creation not wired from this menu"
                );
                return Task::none();
            }
            _ => {
                tracing::warn!(
                    target: "signex::library",
                    ?kind,
                    "Add Library primitive: unsupported PrimitiveKind variant"
                );
                return Task::none();
            }
        };

        if let Err(e) = save_result {
            tracing::warn!(
                target: "signex::library",
                ?kind,
                root = %resolved_root.display(),
                error = %e,
                "Add Library primitive: adapter save failed"
            );
            return Task::none();
        }

        // Dismiss the context menu so the user gets visual feedback —
        // the menu wasn't auto-closing for the stubbed arms.
        self.interaction_state.project_tree_context_menu = None;
        self.interaction_state.context_submenu = None;

        // Rescan the library directory so the new file shows up under
        // Libraries ▸ <name> ▸ Symbols / Footprints.
        self.refresh_panel_ctx();

        // Open the new file as a standalone primitive-editor tab.
        self.handle_open_primitive(new_path)
    }
}
