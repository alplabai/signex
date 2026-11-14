use iced::Task;
use signex_i18n::ids;

use super::super::super::*;
use crate::app::document::DocumentKey;

impl Signex {
    /// File ▸ Open: its title and filter names in the language in force.
    pub(crate) fn open_project_dialog(&self) -> crate::io_ports::FileDialog {
        let i18n = &self.ui_state.i18n;
        self.ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::FILE_DIALOG_OPEN_PROJECT)
            .filter_in(i18n, ids::FILE_FILTER_PROJECT, &["snxprj"])
            .filter_in(i18n, ids::FILE_FILTER_SCHEMATIC, &["snxsch"])
            .filter_in(i18n, ids::FILE_FILTER_ALL_SUPPORTED, &["snxprj", "snxsch"])
            .filter_in(i18n, ids::FILE_FILTER_ALL_FILES, &["*"])
    }

    pub(super) fn handle_menu_file_command(&mut self, msg: &MenuMessage) -> Option<Task<Message>> {
        match msg {
            MenuMessage::OpenProject => {
                Some(Task::perform(self.open_project_dialog().pick_file(), |p| {
                    Message::File(FileMsg::Opened(p))
                }))
            }
            MenuMessage::OpenRecentProject(path) => {
                Some(self.handle_open_recent_project(path.clone()))
            }
            MenuMessage::ClearRecentProjects => Some(self.handle_clear_recent_projects()),
            MenuMessage::ImportAltium => Some(Task::perform(
                self.ui_state
                    .io
                    .picker
                    .dialog()
                    .title_in(&self.ui_state.i18n, ids::FILE_DIALOG_IMPORT_ALTIUM)
                    .filter_in(
                        &self.ui_state.i18n,
                        ids::FILE_FILTER_ALTIUM_DESIGN,
                        &["PrjPcb", "SchDoc", "PcbDoc"],
                    )
                    .filter_in(
                        &self.ui_state.i18n,
                        ids::FILE_FILTER_ALTIUM_LIBRARY,
                        &["SchLib", "PcbLib"],
                    )
                    .filter_in(&self.ui_state.i18n, ids::FILE_FILTER_ALL_FILES, &["*"])
                    .pick_file(),
                |p| Message::File(FileMsg::AltiumImportPicked(p)),
            )),
            MenuMessage::SaveAs => Some(self.save_as_dialog_for_active_tab()),
            MenuMessage::SaveAll => Some(self.save_all_dirty()),
            MenuMessage::CloseDocument => {
                let active = self.document_state.active_tab;
                Some(self.handle_tab_context_action(crate::app::TabContextAction::Close(active)))
            }
            MenuMessage::CloseProject => Some(self.close_active_project()),
            // The dialog; a headless run keeps the picker so its answers drive it.
            MenuMessage::NewProject if self.headless_answers().is_none() => {
                Some(self.open_new_project_dialog())
            }
            MenuMessage::NewProject => {
                let target = self.save_target(None);
                let picker = self.ui_state.io.picker.clone();
                let i18n = self.ui_state.i18n.clone();
                Some(Task::perform(
                    async move {
                        target
                            .new_project_dialog(
                                picker
                                    .dialog()
                                    .title_in(&i18n, ids::FILE_DIALOG_NEW_PROJECT),
                            )
                            .await
                            .set_file_name("Untitled.snxprj")
                            .filter_in(&i18n, ids::FILE_FILTER_PROJECT, &["snxprj"])
                            .save_file()
                            .await
                    },
                    |p| Message::File(FileMsg::NewProject(p)),
                ))
            }
            MenuMessage::OpenSampleProject => Some(self.handle_open_sample_project()),
            MenuMessage::ExportPdf => Some(self.update(Message::Export(ExportMsg::PdfOpenDialog))),
            MenuMessage::ExportNetlist => self.handle_export_netlist_requested(),
            MenuMessage::ExportBom => Some(self.handle_bom_preview_open()),
            MenuMessage::ExportErcReport => Some(self.handle_erc_report_requested()),
            MenuMessage::Exit => Some(self.update(Message::Window(WindowMsg::CloseMainWindow))),
            MenuMessage::LibraryOpenLibrary => Some(self.update(Message::Library(
                crate::library::LibraryMessage::OpenLibraryDialog,
            ))),
            MenuMessage::LibraryPlaceComponent => {
                Some(self.update(Message::Library(crate::library::LibraryMessage::OpenPicker)))
            }
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
                // The row is greyed unless the active tab is an editor inside
                // an open library (`CommandContext::has_library_editor`).
                self.active_editor_library_root().map(|library_path| {
                    self.update(Message::Library(
                        crate::library::LibraryMessage::OpenDocumentOptions { library_path },
                    ))
                })
            }
            _ => None,
        }
    }

    /// The root of the open library containing the active symbol or footprint
    /// editor tab. The one answer behind Tools ▸ Document Options and its
    /// enablement.
    pub(crate) fn active_editor_library_root(&self) -> Option<std::path::PathBuf> {
        let tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?;
        let path = match &tab.kind {
            crate::app::TabKind::SymbolEditor(p) | crate::app::TabKind::FootprintEditor(p) => p,
            _ => return None,
        };
        self.library
            .containing_library_with(path, self.identity_fold_for(path))
            .map(|lib| lib.root.clone())
    }

    /// Resolve the active tab; if it's a `.snxsym` standalone editor
    /// fire `msg` against its `path`. Returns `None` when no Symbol
    /// editor is active so the menu item silently no-ops on other
    /// tab kinds (the same guard Save has).
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
            .and_then(|m| m.key.as_deref())
            .and_then(|key| {
                signex_widgets::tree_view::resolve_path(
                    self.document_state.panel_ctx.project_tree.as_slice(),
                    key,
                )
            }) {
            Some(p) if !p.is_empty() => p,
            _ => {
                crate::diagnostics::log_warning(format!(
                    "Add {kind:?} library: the project node the menu was opened on no longer exists"
                ));
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

impl Signex {
    /// File ▸ Save All: every dirty document and row through the shared
    /// Save All flow, with nothing to do afterwards.
    fn save_all_dirty(&mut self) -> Task<Message> {
        self.commit_pending_title_block_drafts();
        // The board sketch goes to its board first; a refused bake holds it.
        if !self.settle_board_sketch_of(None) {
            return Task::none();
        }
        let rows = self.library.dirty_editors();
        let mut paths: Vec<std::path::PathBuf> = self
            .document_state
            .dirty_document_paths()
            .into_iter()
            .collect();
        paths.sort();
        if paths.is_empty() && rows.is_empty() {
            return Task::none();
        }
        self.begin_save_all(
            "Save All",
            &paths,
            &rows,
            crate::app::contracts::SaveAllNext::Nothing,
        )
    }

    /// Pick the Save As dialog by the active tab's document kind (#671).
    /// Kinds without Save As log a message and open nothing.
    pub(crate) fn save_as_dialog_for_active_tab(&mut self) -> Task<Message> {
        let key = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(DocumentKey::of);
        match key {
            Some(DocumentKey::Schematic(_)) => {
                let from_path = self.active_tab_path().unwrap_or_default();
                let target = self.save_target(Some(&from_path));
                let save_as_name = target.save_as_suggested_name();
                let picker = self.ui_state.io.picker.clone();
                let i18n = self.ui_state.i18n.clone();
                Task::perform(
                    async move {
                        let base = picker
                            .dialog()
                            .title_in(&i18n, ids::FILE_DIALOG_SAVE_SCHEMATIC_AS);
                        let dialog = target.save_as_dialog(base).await.filter_in(
                            &i18n,
                            ids::FILE_FILTER_SCHEMATIC,
                            &["snxsch"],
                        );
                        let dialog = match save_as_name {
                            Some(name) => dialog.set_file_name(name),
                            None => dialog,
                        };
                        dialog.save_file().await
                    },
                    move |path| {
                        path.map(|to_path| {
                            Message::File(FileMsg::SaveAs {
                                from_path: from_path.clone(),
                                to_path,
                            })
                        })
                        .unwrap_or(Message::noop())
                    },
                )
            }
            Some(DocumentKey::Symbol(path) | DocumentKey::Footprint(path)) => {
                super::super::document_files::spawn_save_as_for_primitive(
                    self.ui_state.io.picker.clone(),
                    &self.ui_state.i18n,
                    path,
                )
            }
            _ => {
                crate::diagnostics::log_warning(
                    "[save-as] Save As is not supported for the active tab",
                );
                Task::none()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use signex_i18n::LanguagePref;

    use super::*;

    type Filters = Vec<(String, Vec<String>)>;

    fn switch(app: &mut Signex, tag: &str) {
        let pref = LanguagePref::Locale(tag.to_owned());
        let _ = app.update(Message::Ui(UiMsg::LanguageChanged(pref)));
    }

    fn shown(app: &Signex) -> (String, Filters) {
        let dialog = app.open_project_dialog();
        let (title, filters) = dialog.shown_for_test();
        (title.to_owned(), filters.to_vec())
    }

    fn extensions(filters: &Filters) -> Vec<Vec<String>> {
        filters.iter().map(|(_, e)| e.clone()).collect()
    }

    /// #2058: the native Open dialog's title and filter names follow the
    /// language; the extensions do not.
    #[test]
    fn the_open_dialog_title_and_filter_names_follow_the_language() {
        let (mut app, _task) = Signex::new();
        switch(&mut app, "en-US");
        let (title, filters) = shown(&app);
        assert_eq!(title, "Open Project or Schematic");
        assert_eq!(filters[0].0, "Signex Project");
        assert_eq!(filters[3].0, "All files");

        switch(&mut app, "en-XA");
        let (pseudo_title, pseudo_filters) = shown(&app);
        assert_ne!(pseudo_title, title);
        assert_ne!(pseudo_filters[0].0, filters[0].0);
        assert_eq!(extensions(&pseudo_filters), extensions(&filters));

        switch(&mut app, "en-US");
        assert_eq!(shown(&app), (title, filters));
    }
}
