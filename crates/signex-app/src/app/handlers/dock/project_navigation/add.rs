//! Add Existing / Add New flows for the project-navigation dock.
//!
//! File work runs off the update thread (see `add_io`); every completion
//! carries the project by id and looks it up again when it lands.

use super::*;
use crate::app::state::ProjectId;

impl Signex {
    /// `Add Existing to Project…` — open a multi-select file picker
    /// scoped to schematic / PCB / library extensions. Picked paths
    /// land in [`ProjectMsg::AddExistingFilePicked`]; the handler copies
    /// any outside the project directory in turn and opens each.
    pub(crate) fn add_existing_to_project(&mut self, tree_path: Vec<usize>) -> iced::Task<Message> {
        let Some(project_id) = self.project_id_for_tree_path(&tree_path) else {
            return iced::Task::none();
        };
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
            move |paths| Message::Project(ProjectMsg::AddExistingFilePicked { project_id, paths }),
        )
    }

    /// `Add New ▸ Schematic` — Save-As dialog scoped to the project
    /// directory; result returns through [`ProjectMsg::AddNewSchematicPicked`].
    /// The handler writes a blank `.snxsch`, registers the entry on
    /// the project, and marks the .snxprj dirty.
    pub(crate) fn add_new_schematic(&mut self, tree_path: Vec<usize>) -> iced::Task<Message> {
        let Some(project_id) = self.project_id_for_tree_path(&tree_path) else {
            return iced::Task::none();
        };
        let project_dir = match self
            .document_state
            .project_by_id(project_id)
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
            move |path| Message::Project(ProjectMsg::AddNewSchematicPicked { project_id, path }),
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
        let Some(project_id) = self.project_id_for_tree_path(&tree_path) else {
            return iced::Task::none();
        };
        let project_dir = match self
            .document_state
            .project_by_id(project_id)
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
                Some(path) => {
                    Message::Library(crate::library::LibraryMessage::AddLibrarySymbolFilePicked {
                        project_id,
                        path,
                    })
                }
                None => Message::noop(),
            },
        )
    }

    /// `Add New ▸ PCB Library` — counterpart to
    /// [`add_project_symbol_library`] for `.snxfpt` files.
    pub(crate) fn add_project_footprint_library(
        &mut self,
        tree_path: Vec<usize>,
    ) -> iced::Task<Message> {
        let Some(project_id) = self.project_id_for_tree_path(&tree_path) else {
            return iced::Task::none();
        };
        let project_dir = match self
            .document_state
            .project_by_id(project_id)
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
                    crate::library::LibraryMessage::AddLibraryFootprintFilePicked {
                        project_id,
                        path,
                    },
                ),
                None => Message::noop(),
            },
        )
    }

    pub(crate) fn handle_add_new_schematic_picked(
        &mut self,
        project_id: ProjectId,
        path: Option<std::path::PathBuf>,
    ) -> iced::Task<Message> {
        let Some(path) = path else {
            return iced::Task::none();
        };
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
        let Some((project_dir, _)) = self.project_dir_and_path(project_id, "Add New", &[]) else {
            return iced::Task::none();
        };
        let fold = self
            .document_state
            .project_by_id(project_id)
            .map_or_else(crate::app::CaseFold::host_default, |p| p.case_fold);
        if !crate::app::state::library_entry::is_inside_project(&project_dir, &path, fold) {
            crate::diagnostics::log_error(
                "Add New Schematic: destination outside project directory",
                &anyhow::anyhow!("{}", path.display()),
            );
            return iced::Task::none();
        }
        if self.refuse_with_notice_while_project_moves(
            &path,
            "create",
            crate::app::state::ErrorNotice::create,
        ) {
            return iced::Task::none();
        }
        self.begin_project_write(&path);
        // The blank sheet is written off the update thread; the project is
        // looked up again by id when the write lands.
        iced::Task::perform(
            {
                let path = path.clone();
                async move {
                    add_io::on_blocking_pool(move || add_io::write_blank_schematic(&path)).await
                }
            },
            move |result| {
                Message::Project(ProjectMsg::AddNewSchematicWritten {
                    project_id,
                    path,
                    result,
                })
            },
        )
    }

    /// Completion of the blank-schematic write: register it on the project
    /// the dialog was opened for, or report the failure.
    pub(crate) fn handle_add_new_schematic_written(
        &mut self,
        project_id: ProjectId,
        path: std::path::PathBuf,
        result: Result<(), String>,
    ) {
        self.end_project_write(&path);
        if let Err(e) = result {
            crate::diagnostics::log_error("Add New Schematic", &anyhow::anyhow!("{e}"));
            return;
        }
        let Some((_, project_path)) =
            self.project_dir_and_path(project_id, "Add New", std::slice::from_ref(&path))
        else {
            return;
        };
        if self.register_project_file(project_id, &path) {
            self.document_state.mark_document_dirty(&project_path);
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn handle_add_existing_file_picked(
        &mut self,
        project_id: ProjectId,
        paths: Option<Vec<std::path::PathBuf>>,
    ) -> iced::Task<Message> {
        let Some(paths) = paths else {
            return iced::Task::none();
        };
        let Some((project_dir, _)) = self.project_dir_and_path(project_id, "Add Existing", &[])
        else {
            return iced::Task::none();
        };
        if self.refuse_with_notice_while_project_moves(
            &project_dir,
            "add files to",
            crate::app::state::ErrorNotice::create,
        ) {
            return iced::Task::none();
        }
        let fold =
            crate::app::state::project_case::fold_of(self.document_state.project_by_id(project_id));
        let plans: Vec<_> = paths
            .into_iter()
            .map(|p| add_io::plan_existing(&project_dir, p, fold))
            .collect();
        self.begin_project_write(&project_dir);
        iced::Task::perform(
            async move {
                add_io::on_blocking_pool(move || {
                    Ok(plans
                        .into_iter()
                        .map(add_io::run_existing)
                        .collect::<Vec<_>>())
                })
                .await
            },
            move |results| {
                Message::Project(ProjectMsg::AddExistingFilesCopied {
                    project_id,
                    project_dir: project_dir.clone(),
                    results,
                })
            },
        )
    }

    /// Completion of the Add Existing copies: report each failure, register
    /// the files that made it onto the project the dialog was opened for.
    pub(crate) fn handle_add_existing_files_copied(
        &mut self,
        project_id: ProjectId,
        project_dir: &std::path::Path,
        results: Result<Vec<Result<std::path::PathBuf, String>>, String>,
    ) -> iced::Task<Message> {
        self.end_project_write(project_dir);
        let results = match results {
            Ok(r) => r,
            Err(e) => vec![Err(e)],
        };
        let mut landed = Vec::new();
        for result in results {
            match result {
                Ok(path) => landed.push(path),
                Err(e) => crate::diagnostics::log_error("Add Existing", &anyhow::anyhow!("{e}")),
            }
        }
        let Some((_, project_path)) =
            self.project_dir_and_path(project_id, "Add Existing", &landed)
        else {
            return iced::Task::none();
        };
        let mut any_added = false;
        let mut new_libraries = Vec::new();
        for path in landed {
            if self.register_project_file(project_id, &path) {
                any_added = true;
                if add_io::is_snxlib(&path) {
                    new_libraries.push(path);
                }
            }
        }
        if any_added {
            // Mark the .snxprj dirty so the user knows to save. The
            // file copy already touched disk (irreversible) but the
            // project's *list* of children only persists once Save
            // writes the JSON .snxprj.
            self.document_state.mark_document_dirty(&project_path);
        }
        self.refresh_panel_ctx();
        self.mount_added_libraries(new_libraries)
    }

    /// Mount each freshly registered `.snxlib` off the update thread, as the
    /// project-open fan-out does, so it reaches the Components panel without
    /// a reopen (#1213). A failure lands in the Messages panel.
    fn mount_added_libraries(&mut self, paths: Vec<std::path::PathBuf>) -> iced::Task<Message> {
        use crate::library::mount::{MountIntent, MountRequest, spawn_pending_mounts};
        let mut cold = Vec::new();
        let mut tasks = Vec::new();
        for path in paths {
            match self.library.request_mount(&path, MountIntent::Silent) {
                MountRequest::Spawn => cold.push(path),
                // Already mounted: rescan it the way project open does, so
                // primitive files written by another process show up.
                MountRequest::AlreadyMounted => tasks.push(self.refresh_library_task(
                    &path,
                    crate::library::refresh::RefreshFollowUp::Nothing,
                )),
                MountRequest::InFlight => {}
            }
        }
        tasks.push(spawn_pending_mounts(cold));
        iced::Task::batch(tasks)
    }

    /// The project a project-tree path points into, as a stable id. The tree
    /// path's first segment is an index into the live list; that index is only
    /// good until the next await, so it is resolved to an id before any dialog
    /// opens and the completion looks the project up again (#833).
    fn project_id_for_tree_path(&self, tree_path: &[usize]) -> Option<ProjectId> {
        let &project_idx = tree_path.first()?;
        self.document_state.projects.get(project_idx).map(|p| p.id)
    }

    /// Resolve a completion's project at completion time: its directory and
    /// `.snxprj` path, or `None` — logged to the Messages panel — when the
    /// project was closed while the dialog was open (#833).
    fn project_dir_and_path(
        &self,
        project_id: ProjectId,
        action: &str,
        on_disk: &[std::path::PathBuf],
    ) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
        let resolved = self.document_state.project_by_id(project_id).and_then(|p| {
            p.path
                .parent()
                .map(|dir| (dir.to_path_buf(), p.path.clone()))
        });
        if resolved.is_none() {
            if on_disk.is_empty() {
                crate::diagnostics::log_warning(format!(
                    "{action}: the target project ({project_id}) was closed before the dialog \
                     finished; nothing was added"
                ));
            } else {
                let files = on_disk
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                crate::diagnostics::log_warning(format!(
                    "{action}: the target project ({project_id}) was closed before the write \
                     finished. The file(s) are already on disk but are not in any project: \
                     {files}. Use Add Existing from inside the project to add them."
                ));
            }
        }
        resolved
    }

    fn log_already_listed(file_path: &std::path::Path, entry: &str) {
        crate::diagnostics::log_warning(format!(
            "Add Existing: {} is already in the project as {entry}; not added again",
            file_path.display()
        ));
    }

    fn entry_or_refuse(
        filename: Result<String, fn(&std::path::Path)>,
        file_path: &std::path::Path,
    ) -> Option<String> {
        filename.map_err(|refuse| refuse(file_path)).ok()
    }

    fn log_outside_project_refusal(file_path: &std::path::Path) {
        crate::diagnostics::log_warning(format!(
            "Add Existing: not added, {} is not inside the project directory",
            file_path.display()
        ));
    }

    fn log_non_utf8_refusal(file_path: &std::path::Path) {
        crate::diagnostics::log_warning(format!(
            "Add Existing: not added, the path is not valid UTF-8: {}",
            file_path.display()
        ));
    }

    /// Push a freshly added file into the project's in-memory model so
    /// the tree picks it up. Returns `true` when something was actually
    /// inserted (the caller flips the project dirty bit on `true`).
    /// Files already referenced are skipped — re-adding the same file
    /// is a no-op rather than a duplicate row.
    fn register_project_file(
        &mut self,
        project_id: ProjectId,
        file_path: &std::path::Path,
    ) -> bool {
        let Some(loaded) = self.document_state.project_by_id_mut(project_id) else {
            return false;
        };
        if file_path.file_name().and_then(|s| s.to_str()).is_none() {
            Self::log_non_utf8_refusal(file_path);
            return false;
        }
        // A file picked from a project subdirectory is registered by its path
        // below the project (forward slashes on every platform), because
        // `SheetEntry.filename` / `pcb_file` are joined onto the project
        // directory. A file at the project root, or one copied in (it lands
        // at the root), stays a bare name. A sheet or PCB outside the project
        // has no such entry and is refused; `Err` names the refusal to log.
        let filename: Result<String, fn(&std::path::Path)> = {
            use crate::app::state::library_entry::library_entry_location;
            let (rel, kind) = library_entry_location(loaded.dir(), file_path, loaded.case_fold);
            match kind {
                signex_types::project::LibraryEntryKind::ProjectLocal => rel
                    .components()
                    .map(|c| c.as_os_str().to_str())
                    .collect::<Option<Vec<&str>>>()
                    .map(|parts| parts.join("/"))
                    .ok_or(Self::log_non_utf8_refusal),
                _ => Err(Self::log_outside_project_refusal),
            }
        };
        let fold = loaded.case_fold;
        let project_dir = loaded.dir().to_path_buf();
        let is_listed = |entry: &str| {
            crate::app::state::library_entry::entry_resolves_to(
                &project_dir,
                entry,
                file_path,
                fold,
            )
        };
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "snxsch" => {
                let Some(filename) = Self::entry_or_refuse(filename, file_path) else {
                    return false;
                };
                if let Some(listed) = loaded.data.sheets.iter().find(|s| is_listed(&s.filename)) {
                    Self::log_already_listed(file_path, &listed.filename);
                    return false;
                }
                register_sheet_entry(loaded, &filename)
            }
            "snxpcb" => {
                let Some(filename) = Self::entry_or_refuse(filename, file_path) else {
                    return false;
                };
                if let Some(listed) = loaded.data.pcb_file.as_deref().filter(|e| is_listed(e)) {
                    Self::log_already_listed(file_path, listed);
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
                // Inside the project: stored relative to it. Outside: absolute
                // and Shared. Either way it resolves to the picked path
                // (#1215); duplicates are judged on the resolved path.
                use crate::app::state::library_entry::{is_library_listed, library_entry_location};
                let project_dir = std::path::PathBuf::from(&loaded.data.dir);
                let fold = loaded.case_fold;
                if is_library_listed(&loaded.data, file_path, fold) {
                    return false;
                }
                let (entry_path, kind) = library_entry_location(&project_dir, file_path, fold);
                loaded
                    .data
                    .libraries
                    .push(signex_types::project::LibraryEntry {
                        path: entry_path,
                        kind,
                        library_id: None,
                        unknown: Default::default(),
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

/// List the schematic `filename` (relative to the project folder) among the
/// project's sheets: the one place a sheet is registered. Compared under the
/// project's case rules, so `Sheet1.snxsch` and `sheet1.snxsch` are one sheet
/// on a case-insensitive volume. The first sheet listed becomes the root when
/// the project has none. `true` when something was added.
pub(crate) fn register_sheet_entry(loaded: &mut crate::app::LoadedProject, filename: &str) -> bool {
    let fold = crate::app::state::project_case::fold_of(Some(&*loaded));
    let key =
        |name: &str| crate::app::state::scope::path_key_with(std::path::Path::new(name), fold);
    let wanted = key(filename);
    if loaded
        .data
        .sheets
        .iter()
        .any(|s| key(&s.filename) == wanted)
    {
        return false;
    }
    let name = std::path::Path::new(filename)
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    loaded.data.sheets.push(signex_types::project::SheetEntry {
        name,
        filename: filename.to_owned(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    });
    if loaded.data.schematic_root.is_none() {
        loaded.data.schematic_root = Some(filename.to_owned());
    }
    true
}
