//! Add Existing / Add New flows for the project-navigation dock.
//!
//! Extracted verbatim from the project-navigation dock handlers
//! (`handlers/dock/project_navigation`); pure code motion, zero
//! behaviour change.

use super::*;

impl Signex {
    /// `Add Existing to Project…` — open a multi-select file picker
    /// scoped to schematic / PCB / library extensions. Picked paths
    /// land in [`ProjectMsg::AddExistingFilePicked`]; the handler copies
    /// any outside the project directory in turn and opens each.
    pub(crate) fn add_existing_to_project(&mut self, tree_path: Vec<usize>) -> iced::Task<Message> {
        let Some(&project_idx) = tree_path.first() else {
            return iced::Task::none();
        };
        if self.document_state.projects.get(project_idx).is_none() {
            return iced::Task::none();
        }
        iced::Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Add Existing to Project")
                    .add_filter(
                        "All Supported",
                        &["snxsch", "snxpcb", "snxlib", "snxsym", "snxfpt"],
                    )
                    .add_filter("Signex Schematic", &["snxsch"])
                    .add_filter("Signex PCB", &["snxpcb"])
                    .add_filter("Signex Library", &["snxlib"])
                    .add_filter("Signex Symbol", &["snxsym"])
                    .add_filter("Signex Footprint", &["snxfpt"])
                    .pick_files()
                    .await
                    .map(|files| {
                        files
                            .into_iter()
                            .map(|file| file.path().to_path_buf())
                            .collect::<Vec<_>>()
                    })
            },
            move |paths| Message::Project(ProjectMsg::AddExistingFilePicked { project_idx, paths }),
        )
    }

    /// `Add New ▸ Schematic` — Save-As dialog scoped to the project
    /// directory; result returns through [`ProjectMsg::AddNewSchematicPicked`].
    /// The handler writes a blank `.snxsch`, registers the entry on
    /// the project, and marks the .snxprj dirty.
    pub(crate) fn add_new_schematic(&mut self, tree_path: Vec<usize>) -> iced::Task<Message> {
        let Some(&project_idx) = tree_path.first() else {
            return iced::Task::none();
        };
        let project_dir = match self
            .document_state
            .projects
            .get(project_idx)
            .and_then(|p| p.path.parent().map(|d| d.to_path_buf()))
        {
            Some(d) => d,
            None => return iced::Task::none(),
        };
        let default_name = unique_name_in(&project_dir, "Sheet", "snxsch");
        iced::Task::perform(
            async move {
                rfd::AsyncFileDialog::new()
                    .set_title("Add New Schematic to Project")
                    .set_directory(&project_dir)
                    .set_file_name(&default_name)
                    .add_filter("Signex Schematic", &["snxsch"])
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            move |path| Message::Project(ProjectMsg::AddNewSchematicPicked { project_idx, path }),
        )
    }

    /// `Add New ▸ Symbol Library` — Save-As dialog scoped to the
    /// project dir. The picked path is forwarded to
    /// [`LibraryMessage::AddLibrarySymbolFilePicked`] which writes an
    /// empty `SymbolFile` and opens the file as a primitive editor tab.
    pub(crate) fn add_project_symbol_library(
        &mut self,
        tree_path: Vec<usize>,
    ) -> iced::Task<Message> {
        let Some(&project_idx) = tree_path.first() else {
            return iced::Task::none();
        };
        let project_dir = match self
            .document_state
            .projects
            .get(project_idx)
            .and_then(|p| p.path.parent().map(|d| d.to_path_buf()))
        {
            Some(d) => d,
            None => return iced::Task::none(),
        };
        let default_name = unique_name_in(&project_dir, "SymbolLibrary", "snxsym");
        iced::Task::perform(
            async move {
                rfd::AsyncFileDialog::new()
                    .set_title("New Symbol Library")
                    .set_directory(&project_dir)
                    .set_file_name(&default_name)
                    .add_filter("Signex Symbol Library", &["snxsym"])
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            move |picked| match picked {
                Some(path) => Message::Library(
                    crate::library::LibraryMessage::AddLibrarySymbolFilePicked(path),
                ),
                None => Message::Noop,
            },
        )
    }

    /// `Add New ▸ PCB Library` — counterpart to
    /// [`add_project_symbol_library`] for `.snxfpt` files.
    pub(crate) fn add_project_footprint_library(
        &mut self,
        tree_path: Vec<usize>,
    ) -> iced::Task<Message> {
        let Some(&project_idx) = tree_path.first() else {
            return iced::Task::none();
        };
        let project_dir = match self
            .document_state
            .projects
            .get(project_idx)
            .and_then(|p| p.path.parent().map(|d| d.to_path_buf()))
        {
            Some(d) => d,
            None => return iced::Task::none(),
        };
        let default_name = unique_name_in(&project_dir, "FootprintLibrary", "snxfpt");
        iced::Task::perform(
            async move {
                rfd::AsyncFileDialog::new()
                    .set_title("New PCB Library")
                    .set_directory(&project_dir)
                    .set_file_name(&default_name)
                    .add_filter("Signex Footprint Library", &["snxfpt"])
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            move |picked| match picked {
                Some(path) => Message::Library(
                    crate::library::LibraryMessage::AddLibraryFootprintFilePicked(path),
                ),
                None => Message::Noop,
            },
        )
    }

    pub(crate) fn handle_add_new_schematic_picked(
        &mut self,
        project_idx: usize,
        path: Option<std::path::PathBuf>,
    ) {
        let Some(path) = path else { return };
        // Force the .snxsch extension so users typing "Foo" don't end up
        // with a bare `Foo` file the directory probe / extension match
        // ignores.
        let path = if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("snxsch"))
            != Some(true)
        {
            let mut p = path.into_os_string();
            p.push(".snxsch");
            std::path::PathBuf::from(p)
        } else {
            path
        };
        let project_dir = match self
            .document_state
            .projects
            .get(project_idx)
            .and_then(|p| p.path.parent().map(|d| d.to_path_buf()))
        {
            Some(d) => d,
            None => return,
        };
        let project_path = match self
            .document_state
            .projects
            .get(project_idx)
            .map(|p| p.path.clone())
        {
            Some(p) => p,
            None => return,
        };
        if !path.starts_with(&project_dir) {
            crate::diagnostics::log_error(
                "Add New Schematic: destination outside project directory",
                &anyhow::anyhow!("{}", path.display()),
            );
            return;
        }
        if path.exists() {
            crate::diagnostics::log_error(
                "Add New Schematic: destination already exists",
                &anyhow::anyhow!("{}", path.display()),
            );
            return;
        }
        // Build a blank sheet through the same helper File ▸ New
        // Project uses so the on-disk format stays in lockstep.
        let sheet = blank_schematic_sheet_for_new_doc();
        let serialised = match signex_types::format::SnxSchematic::new(sheet).write_string() {
            Ok(s) => s,
            Err(e) => {
                crate::diagnostics::log_error(
                    "Add New Schematic: serialise blank sheet",
                    &anyhow::anyhow!("{}", e),
                );
                return;
            }
        };
        if let Err(e) = signex_types::atomic_io::atomic_write(&path, serialised.as_bytes()) {
            crate::diagnostics::log_error(
                "Add New Schematic: write blank sheet",
                &anyhow::anyhow!("{}", e),
            );
            return;
        }
        if self.register_project_file(project_idx, &path) {
            self.document_state.dirty_paths.insert(project_path);
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn handle_add_existing_file_picked(
        &mut self,
        project_idx: usize,
        paths: Option<Vec<std::path::PathBuf>>,
    ) {
        let Some(paths) = paths else { return };
        let (project_dir, project_path) = match self
            .document_state
            .projects
            .get(project_idx)
            .and_then(|p| p.path.parent().map(|d| (d.to_path_buf(), p.path.clone())))
        {
            Some(pair) => pair,
            None => return,
        };
        let mut any_added = false;
        for path in paths {
            let final_path = if path.starts_with(&project_dir) {
                path
            } else {
                let Some(file_name) = path.file_name() else {
                    continue;
                };
                let dest = project_dir.join(file_name);
                if dest.exists() {
                    crate::diagnostics::log_error(
                        "Add Existing: destination already exists",
                        &anyhow::anyhow!("{}", dest.display()),
                    );
                    continue;
                }
                if let Err(error) = std::fs::copy(&path, &dest) {
                    crate::diagnostics::log_error(
                        "Add Existing: copy failed",
                        &anyhow::anyhow!("{}", error),
                    );
                    continue;
                }
                dest
            };
            if self.register_project_file(project_idx, &final_path) {
                any_added = true;
            }
        }
        if any_added {
            // Mark the .snxprj dirty so the user knows to save. The
            // file copy already touched disk (irreversible) but the
            // project's *list* of children only persists once Save
            // writes the JSON .snxprj.
            self.document_state.dirty_paths.insert(project_path);
        }
        self.refresh_panel_ctx();
    }

    /// Push a freshly added file into the project's in-memory model so
    /// the tree picks it up. Returns `true` when something was actually
    /// inserted (the caller flips the project dirty bit on `true`).
    /// Files already referenced are skipped — re-adding the same file
    /// is a no-op rather than a duplicate row.
    fn register_project_file(&mut self, project_idx: usize, file_path: &std::path::Path) -> bool {
        let Some(loaded) = self.document_state.projects.get_mut(project_idx) else {
            return false;
        };
        let filename = match file_path.file_name().and_then(|s| s.to_str()) {
            Some(name) => name.to_string(),
            None => return false,
        };
        let stem = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "snxsch" => {
                if loaded.data.sheets.iter().any(|s| s.filename == filename) {
                    return false;
                }
                loaded.data.sheets.push(signex_types::project::SheetEntry {
                    name: stem,
                    filename,
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                });
                if loaded.data.schematic_root.is_none() {
                    loaded.data.schematic_root =
                        loaded.data.sheets.last().map(|s| s.filename.clone());
                }
                true
            }
            "snxpcb" => {
                if loaded.data.pcb_file.as_deref() == Some(filename.as_str()) {
                    return false;
                }
                if loaded.data.pcb_file.is_some() {
                    crate::diagnostics::log_error(
                        "Add Existing: project already has a PCB file",
                        &anyhow::anyhow!(
                            "kept existing {:?}, ignoring {}",
                            loaded.data.pcb_file,
                            filename,
                        ),
                    );
                    return false;
                }
                loaded.data.pcb_file = Some(filename);
                true
            }
            "snxlib" => {
                let entry_path = std::path::PathBuf::from(&filename);
                if loaded.data.libraries.iter().any(|e| e.path == entry_path) {
                    return false;
                }
                loaded
                    .data
                    .libraries
                    .push(signex_types::project::LibraryEntry {
                        path: entry_path,
                        kind: signex_types::project::LibraryEntryKind::ProjectLocal,
                        library_id: None,
                    });
                true
            }
            _ => {
                // .snxsym / .snxfpt are owned by a library; adding them
                // direct to a project doesn't fit the data model. Log
                // and skip — the user should add the parent .snxlib.
                crate::diagnostics::log_error(
                    "Add Existing: unsupported file type for project tree",
                    &anyhow::anyhow!(
                        ".{} files belong inside a .snxlib library, not the project root",
                        ext,
                    ),
                );
                false
            }
        }
    }
}

/// Pick a filename under `dir` that doesn't collide with anything on
/// disk. Tries `<base>.<ext>`, then `<base>2.<ext>`, `<base>3.<ext>`,
/// etc. Used to seed the Add-New-Schematic Save-As dialog so the user
/// doesn't have to dodge an existing file by hand.
fn unique_name_in(dir: &std::path::Path, base: &str, ext: &str) -> String {
    let primary = format!("{base}.{ext}");
    if !dir.join(&primary).exists() {
        return primary;
    }
    for n in 2..=999 {
        let name = format!("{base}{n}.{ext}");
        if !dir.join(&name).exists() {
            return name;
        }
    }
    primary
}

fn blank_schematic_sheet_for_new_doc() -> signex_types::schematic::SchematicSheet {
    super::super::super::document_files::blank_schematic_sheet_for_new_doc()
}
