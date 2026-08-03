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
                |p| Message::File(FileMsg::Opened(p)),
            )),
            MenuMessage::Save => Some(self.update(Message::File(FileMsg::Save))),
            MenuMessage::SaveAs => Some(Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Save Schematic As")
                        .add_filter("Signex Schematic", &["snxsch"])
                        .save_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                },
                |path| {
                    path.map(|p| Message::File(FileMsg::SaveAs(p)))
                        .unwrap_or(Message::Noop)
                },
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
                |p| Message::File(FileMsg::NewProject(p)),
            )),
            MenuMessage::PrintPreview => {
                Some(self.update(Message::PrintPreview(PrintPreviewMsg::Requested)))
            }
            MenuMessage::ExportPdf => Some(self.update(Message::Export(ExportMsg::PdfOpenDialog))),
            MenuMessage::ExportNetlist => self.handle_export_netlist_requested(),
            MenuMessage::ExportBom => Some(self.handle_bom_preview_open()),
            MenuMessage::Exit => Some(self.update(Message::Window(WindowMsg::CloseMainWindow))),
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
                crate::library::messages::SymbolEditorMsg::NewPart,
            ),
            MenuMessage::ToolsRemovePart => self.dispatch_active_symbol_primitive_event(
                crate::library::messages::SymbolEditorMsg::RemovePart,
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
        msg: crate::library::messages::SymbolEditorMsg,
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
            crate::library::LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: crate::library::messages::PrimitiveEdit::Symbol(msg),
            },
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

        // Library-context "Add New ▸ Symbol/Footprint" reuses the
        // project-root flow — the user wants both surfaces to behave
        // identically: Save-As scoped to the project dir, file written
        // empty, registered as a project library entry, opened as a
        // primitive editor tab. The library node we right-clicked from
        // is informational only; the file lands at project level.
        let tree_path = match self
            .interaction_state
            .project_tree_context_menu
            .as_ref()
            .and_then(|m| m.path.clone())
        {
            Some(p) if !p.is_empty() => p,
            _ => {
                tracing::warn!(
                    target: "signex::library",
                    ?kind,
                    "Add Library primitive: no project node in context menu state"
                );
                return Task::none();
            }
        };
        // Drop the menu state before delegating so the dispatcher's
        // post-action refresh doesn't re-show it.
        self.interaction_state.project_tree_context_menu = None;
        self.interaction_state.context_submenu = None;
        let project_path = vec![tree_path[0]];
        match kind {
            PrimitiveKind::Symbol => self.add_project_symbol_library(project_path),
            PrimitiveKind::Footprint => self.add_project_footprint_library(project_path),
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
}
