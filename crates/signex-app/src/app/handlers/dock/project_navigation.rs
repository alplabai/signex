use anyhow::{Context, Result};
use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_dock_project_navigation_panel_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> bool {
        use signex_widgets::tree_view::{TreeIcon, TreeMsg, get_node};

        match panel_msg {
            crate::panels::PanelMsg::Tree(TreeMsg::Toggle(path)) => {
                signex_widgets::tree_view::toggle(
                    &mut self.document_state.panel_ctx.project_tree,
                    path,
                );
                true
            }
            crate::panels::PanelMsg::Tree(TreeMsg::Select(path)) => {
                let selected_node =
                    get_node(self.document_state.panel_ctx.project_tree.as_slice(), path);
                let is_openable = selected_node.is_some_and(|node| {
                    matches!(
                        node.icon,
                        TreeIcon::Schematic
                            | TreeIcon::Pcb
                            | TreeIcon::SnxSchematic
                            | TreeIcon::SnxPcb
                            | TreeIcon::SnxProject
                            | TreeIcon::SnxFootprint
                            | TreeIcon::SnxSimulation
                            | TreeIcon::SnxLibrary
                            | TreeIcon::SnxSymbol
                    )
                });
                if !is_openable {
                    self.interaction_state.last_tree_click = None;
                    return true;
                }
                // Double-click gate — first click memos the path; the
                // second click on the *same* path within the window
                // actually opens the file. Otherwise the memo just
                // updates so the next tick is treated as the first
                // click of a fresh sequence.
                const TREE_DOUBLE_CLICK_WINDOW: std::time::Duration =
                    std::time::Duration::from_millis(500);
                let now = std::time::Instant::now();
                let is_double = matches!(
                    &self.interaction_state.last_tree_click,
                    Some((prev, t)) if prev == path && now.duration_since(*t) <= TREE_DOUBLE_CLICK_WINDOW
                );
                if is_double {
                    self.interaction_state.last_tree_click = None;
                    if let Some(node) = selected_node
                        && let Err(error) =
                            self.open_project_tree_document(path, node.label.clone())
                    {
                        crate::diagnostics::log_error(
                            "Failed to open project tree document",
                            &error,
                        );
                    }
                } else {
                    self.interaction_state.last_tree_click = Some((path.clone(), now));
                }
                true
            }
            crate::panels::PanelMsg::Tree(TreeMsg::ContextMenu(path)) => {
                // Routed up to the app level as a Message so the overlay
                // dispatcher can anchor the menu at `last_mouse_pos`.
                // `handle_*_panel_message` returns `bool` and has no
                // way to emit a follow-up Task, so we poke the state
                // directly — same shape as the canvas menu wiring.
                self.dispatch_show_project_tree_context_menu(Some(path.clone()));
                true
            }
            crate::panels::PanelMsg::Tree(TreeMsg::BackgroundContextMenu) => {
                self.dispatch_show_project_tree_context_menu(None);
                true
            }
            _ => false,
        }
    }

    fn dispatch_show_project_tree_context_menu(&mut self, path: Option<Vec<usize>>) {
        let (x, y) = self.interaction_state.last_mouse_pos;
        self.interaction_state.context_menu = None;
        self.interaction_state.project_tree_context_menu =
            Some(crate::app::ProjectTreeContextMenuState { x, y, path });
    }

    pub(crate) fn handle_project_tree_action(
        &mut self,
        action: crate::app::ProjectTreeAction,
    ) -> Task<Message> {
        use crate::app::ProjectTreeAction;
        use signex_widgets::tree_view::TreeMsg;

        // Dismiss the menu first — every action either takes effect
        // instantly or triggers a follow-up error, and a lingering
        // menu is always wrong after a pick.
        self.interaction_state.project_tree_context_menu = None;

        match action {
            ProjectTreeAction::OpenNode(path) => {
                // Reuse the existing leaf-open path. Wrapping the inner
                // call in the same PanelMsg handler keeps the icon-gate
                // logic (only schematic / pcb / snx* leaves open) in
                // one place.
                let msg = crate::panels::PanelMsg::Tree(TreeMsg::Select(path));
                self.handle_dock_project_navigation_panel_message(&msg);
            }
            ProjectTreeAction::ToggleNode(path) => {
                signex_widgets::tree_view::toggle(
                    &mut self.document_state.panel_ctx.project_tree,
                    &path,
                );
            }
            ProjectTreeAction::ExpandAll => {
                set_expanded_recursive(
                    &mut self.document_state.panel_ctx.project_tree,
                    true,
                );
            }
            ProjectTreeAction::CollapseAll => {
                set_expanded_recursive(
                    &mut self.document_state.panel_ctx.project_tree,
                    false,
                );
            }
            ProjectTreeAction::Refresh => {
                self.document_state.panel_ctx.project_tree =
                    crate::panels::build_project_tree(&self.document_state.panel_ctx);
            }
            ProjectTreeAction::CloseAllDocuments => {
                // Reverse order so index-based close doesn't shift the
                // items we haven't touched yet. Collects close tasks
                // into a single batch so orphan-window close commands
                // from every tab fire concurrently.
                let mut tasks = Vec::new();
                for idx in (0..self.document_state.tabs.len()).rev() {
                    tasks.push(self.close_tab_now(idx));
                }
                return Task::batch(tasks);
            }
            ProjectTreeAction::RevealInExplorer(tree_path) => {
                // The tree_path's first index picks the owning project,
                // so right-clicking project B's leaf reveals in B's dir
                // even when project A is active. Single-element path =
                // project root → reveal the project directory itself.
                let target = if tree_path.len() <= 1 {
                    tree_path
                        .first()
                        .and_then(|idx| self.document_state.projects.get(*idx))
                        .and_then(|p| p.path.parent().map(|d| d.to_path_buf()))
                } else {
                    self.tree_path_to_file_path(&tree_path)
                };
                if let Some(path) = target
                    && let Err(err) = reveal_in_file_manager(&path)
                {
                    crate::diagnostics::log_error("Failed to reveal in file manager", &err);
                }
            }
            ProjectTreeAction::PrintActive => {
                return Task::perform(async {}, |_| Message::PrintPreviewRequested);
            }
            ProjectTreeAction::OpenRenameDialog(tree_path) => {
                self.open_rename_dialog(tree_path);
            }
            ProjectTreeAction::OpenRemoveDialog(tree_path) => {
                self.open_remove_dialog(tree_path);
            }
            ProjectTreeAction::CloseProject(tree_path) => {
                return self.close_project_at_tree_path(&tree_path);
            }
            ProjectTreeAction::ValidateProject(tree_path) => {
                return self.run_validate_project(tree_path);
            }
            ProjectTreeAction::OpenProjectRenameDialog(tree_path) => {
                self.open_project_rename_dialog(tree_path);
            }
            ProjectTreeAction::OpenProjectOptions(tree_path) => {
                self.open_project_options_dialog(tree_path);
            }
            ProjectTreeAction::AddExistingToProject(tree_path) => {
                return self.add_existing_to_project(tree_path);
            }
            ProjectTreeAction::AddNewSchematic(tree_path) => {
                return self.add_new_schematic(tree_path);
            }
            ProjectTreeAction::OpenEnableVersionControl(tree_path) => {
                self.open_enable_version_control_dialog(tree_path);
            }
            ProjectTreeAction::OpenLibraryEnableVersionControl(tree_path) => {
                self.open_library_enable_version_control_dialog(tree_path);
            }
        }
        Task::none()
    }

    pub(crate) fn open_enable_version_control_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(&project_idx) = tree_path.first() else {
            return;
        };
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return;
        };
        let Some(project_dir) = project.path.parent() else {
            return;
        };
        if project_dir.join(".git").exists() {
            // Already version-controlled — nothing to enable.
            return;
        }
        let items = collect_track_items(project, project_dir);
        let intro_text = format!(
            "Initialise a Git repository at {} and stage every \
             ticked entry in the project as the first commit. \
             From then on, every save commits through libgit2 — \
             including library mutations inside the project's \
             `.snxlib` directories.",
            project_dir.display()
        );
        self.ui_state.enable_version_control =
            Some(crate::app::EnableVersionControlState {
                scope: crate::app::VersionControlScope::Project,
                project_path: project.path.clone(),
                project_dir: project_dir.to_path_buf(),
                project_name: project.data.name.clone(),
                items,
                use_lfs: false,
                intro_text,
                error: None,
            });
    }

    /// v0.11 library-node: open the same Enable Version Control modal
    /// scoped to a single `.snxlib` directory rather than the whole
    /// project tree. The library context-menu only surfaces this when
    /// the library's `root_dir` has no `.git/` already.
    pub(crate) fn open_library_enable_version_control_dialog(
        &mut self,
        tree_path: Vec<usize>,
    ) {
        // Tree path under a project's `Libraries` group is
        // `[project_idx, libraries_branch_idx, library_idx]` — see
        // `library_node_path_from_tree` for the canonical lookup.
        let Some(&project_idx) = tree_path.first() else {
            return;
        };
        let Some(&library_idx) = tree_path.get(2) else {
            return;
        };
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return;
        };
        let Some(entry) = project.data.libraries.get(library_idx) else {
            return;
        };
        let library_file_path = project.data.resolve_library_path(entry);
        let Some(root_dir) = library_file_path.parent().map(|p| p.to_path_buf()) else {
            return;
        };
        if root_dir.join(".git").exists() {
            // Already version-controlled — nothing to enable.
            return;
        }
        let library_name = library_file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("library")
            .to_string();
        let items = collect_track_items_for_library(&root_dir);
        let intro_text = format!(
            "Initialise a Git repository at {} and stage the \
             ticked entries as the first commit. The library \
             adapter will pick up `.git/` on the next save and \
             route subsequent edits through libgit2.",
            root_dir.display()
        );
        // `project_path` is purely informational; point it at a
        // `library.toml` (whether it exists yet or not) inside the
        // library so the modal can mirror the project-scope pattern.
        self.ui_state.enable_version_control =
            Some(crate::app::EnableVersionControlState {
                scope: crate::app::VersionControlScope::Library,
                project_path: root_dir.join("library.toml"),
                project_dir: root_dir,
                project_name: library_name,
                items,
                use_lfs: false,
                intro_text,
                error: None,
            });
    }

    pub(crate) fn handle_enable_version_control_confirm(&mut self) {
        let Some(state) = self.ui_state.enable_version_control.clone() else {
            return;
        };
        // Build the `.gitignore` body from unticked rows. An empty
        // body (every row ticked) is passed through as `None` so the
        // library skips writing the file entirely — keeps the
        // working tree clean for the all-tracked case. The library
        // handles the actual write + rollback atomically alongside
        // `git init`, so disk state never goes half-applied even on
        // failure.
        let gitignore = build_gitignore_body(&state.items);
        let gitignore_arg = if gitignore.is_empty() {
            None
        } else {
            Some(gitignore.as_str())
        };
        match try_init_project_repo(&state.project_dir, state.use_lfs, gitignore_arg) {
            Ok(()) => {
                self.ui_state.enable_version_control = None;
                self.refresh_panel_ctx();
                let scope_label = match state.scope {
                    crate::app::VersionControlScope::Project => "repository",
                    crate::app::VersionControlScope::Library => "library repository",
                };
                crate::diagnostics::log_info(format!(
                    "[git] initialised {scope_label} at {}",
                    state.project_dir.display()
                ));
            }
            Err(error) => {
                if let Some(s) = self.ui_state.enable_version_control.as_mut() {
                    s.error = Some(error.to_string());
                }
            }
        }
    }

    /// Close every tab backed by the project whose root is at
    /// `tree_path[0]`, then drop the project from the workspace and
    /// promote a sibling (or `None`) to active. Mirrors Altium's
    /// Projects-panel right-click → Close Project.
    ///
    /// If any file in the project's directory has unsaved edits
    /// (`dirty_paths` intersects the project dir), opens the
    /// `ProjectCloseConfirm` modal first instead of closing
    /// immediately. The modal's choice handler
    /// (`handle_project_close_confirm`) calls back into this method
    /// once the user picks Save All or Discard All.
    fn close_project_at_tree_path(&mut self, tree_path: &[usize]) -> Task<Message> {
        let Some(&project_idx) = tree_path.first() else {
            return Task::none();
        };
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return Task::none();
        };
        let project_dir = project.path.parent().map(|p| p.to_path_buf());
        // Collect dirty paths inside this project's directory tree.
        // `dirty_paths` is project-scoped by ancestor check —
        // primitive editors live under `<project>/<lib>.snxlib/
        // symbols|footprints/<file>` so the immediate parent dir is
        // never the project dir itself; a strict `parent() == dir`
        // check would miss every nested primitive draft (which is
        // why Add New ▸ Symbol drafts weren't surfacing in the
        // close-project prompt). Walk ancestors instead. The
        // project's own `.snxprj` file (when added to dirty_paths
        // by the auto-attach-library flow) sits exactly at the
        // project root and is also picked up here.
        let dirty: Vec<std::path::PathBuf> = if let Some(dir) = project_dir.as_deref() {
            let mut v: Vec<std::path::PathBuf> = self
                .document_state
                .dirty_paths
                .iter()
                .filter(|p| p.starts_with(dir))
                .cloned()
                .collect();
            v.sort();
            v
        } else {
            Vec::new()
        };

        if !dirty.is_empty() {
            self.ui_state.project_close_confirm =
                Some(crate::app::ProjectCloseConfirmState {
                    tree_path: tree_path.to_vec(),
                    project_name: project.data.name.clone(),
                    dirty_paths: dirty,
                });
            return Task::none();
        }

        self.execute_close_project_at_tree_path(tree_path)
    }

    /// Inner close-project flow that actually drops tabs + the
    /// project entry. Called either directly (clean project) or via
    /// the project-close confirm modal once the user picks Save All
    /// or Discard All. Trusts that `dirty_paths` for this project is
    /// already empty (callers ensure this).
    fn execute_close_project_at_tree_path(&mut self, tree_path: &[usize]) -> Task<Message> {
        let Some(&project_idx) = tree_path.first() else {
            return Task::none();
        };
        let Some(target_id) = self
            .document_state
            .projects
            .get(project_idx)
            .map(|p| p.id)
        else {
            return Task::none();
        };

        // Collect tab indices owned by this project, highest first —
        // `close_tab_now` removes by index, so descending order keeps
        // untouched indices valid as we iterate.
        let mut indices: Vec<usize> = self
            .document_state
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(i, t)| (t.project_id == Some(target_id)).then_some(i))
            .collect();
        indices.sort_unstable_by(|a, b| b.cmp(a));
        let mut tasks: Vec<Task<Message>> = Vec::with_capacity(indices.len());
        for idx in indices {
            tasks.push(self.close_tab_now(idx));
        }

        // Drop the project's parked engines too — without this, an
        // engine that was kept alive by `dirty_paths` would leak
        // through project close. Save All / Discard All have already
        // pruned `dirty_paths` for the project, so this loop just
        // sweeps the engine map for any path under this project's
        // directory.
        if let Some(project) = self.document_state.projects.get(project_idx)
            && let Some(dir) = project.path.parent()
        {
            let dir = dir.to_path_buf();
            self.document_state
                .engines
                .retain(|p, _| p.parent() != Some(&dir));
        }

        // Drop the project from the workspace, then pick a new active
        // project — the one that now sits in the same slot (was
        // immediately after the closed one), else the previous slot,
        // else None when the workspace is empty.
        self.document_state.projects.retain(|p| p.id != target_id);
        self.document_state.active_project = self
            .document_state
            .projects
            .get(project_idx)
            .or_else(|| {
                project_idx
                    .checked_sub(1)
                    .and_then(|i| self.document_state.projects.get(i))
            })
            .map(|p| p.id);

        self.refresh_panel_ctx();

        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks)
        }
    }

    /// Resolve the user's Save All / Discard All / Cancel choice on
    /// the project-close confirmation modal. Owned by this module
    /// because it's a follow-up of `close_project_at_tree_path`.
    pub(crate) fn handle_project_close_confirm(
        &mut self,
        choice: crate::app::ProjectCloseChoice,
    ) -> Task<Message> {
        use crate::app::ProjectCloseChoice;
        let Some(state) = self.ui_state.project_close_confirm.take() else {
            return Task::none();
        };
        match choice {
            ProjectCloseChoice::Cancel => Task::none(),
            ProjectCloseChoice::SaveAll => {
                // Save every dirty file. Files without a live engine
                // shouldn't exist if `dirty_paths` is consistent with
                // `engines` — count those as failures so the user
                // sees something is wrong instead of silently losing
                // the dirty state. If ANY save fails, the project
                // stays open + the user gets an error modal listing
                // the unsaved files.
                let mut failed: Vec<std::path::PathBuf> = Vec::new();
                for path in &state.dirty_paths {
                    if let Some(engine) = self.document_state.engines.get_mut(path) {
                        match engine.save() {
                            Ok(_) => {
                                self.document_state.dirty_paths.remove(path);
                                if let Some(tab) = self
                                    .document_state
                                    .tabs
                                    .iter_mut()
                                    .find(|t| &t.path == path)
                                {
                                    tab.dirty = false;
                                }
                            }
                            Err(err) => {
                                crate::diagnostics::log_error(
                                    "Failed to save during project close",
                                    &anyhow::anyhow!("{err}"),
                                );
                                failed.push(path.clone());
                            }
                        }
                    } else {
                        crate::diagnostics::log_info(format!(
                            "Project close: no live engine for dirty {} — cannot save",
                            path.display()
                        ));
                        failed.push(path.clone());
                    }
                }
                if !failed.is_empty() {
                    let listing: Vec<String> = failed
                        .iter()
                        .filter_map(|p| {
                            p.file_name().and_then(|s| s.to_str()).map(|s| s.to_string())
                        })
                        .collect();
                    self.document_state.export_error = Some(format!(
                        "Could not save {} file(s) — project not closed:\n  {}",
                        failed.len(),
                        listing.join("\n  ")
                    ));
                    return Task::none();
                }
                self.execute_close_project_at_tree_path(&state.tree_path)
            }
            ProjectCloseChoice::DiscardAll => {
                // Drop dirty engines + clear the dirty flags. The
                // engines map is also swept by
                // `execute_close_project_at_tree_path` afterwards,
                // but doing it here too keeps the flow symmetric
                // with SaveAll (after this match the project's
                // dirty_paths entries are gone either way).
                for path in &state.dirty_paths {
                    self.document_state.engines.remove(path);
                    self.document_state.dirty_paths.remove(path);
                    if let Some(tab) = self
                        .document_state
                        .tabs
                        .iter_mut()
                        .find(|t| &t.path == path)
                    {
                        tab.dirty = false;
                    }
                }
                self.execute_close_project_at_tree_path(&state.tree_path)
            }
        }
    }

    fn open_rename_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(target_path) = self.tree_path_to_file_path(&tree_path) else {
            return;
        };
        let filename = target_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        self.ui_state.rename_dialog = Some(crate::app::RenameDialogState {
            target_path,
            tree_path,
            buffer: filename,
            error: None,
            is_project_rename: false,
        });
    }

    /// Open the rename modal seeded with the project name (the `.snxprj`
    /// file stem). On submit, [`handle_rename_submit`] sees
    /// `is_project_rename = true` and renames the trio
    /// `<old>.snxprj` / `<old>.snxsch` / `<old>.snxpcb` together.
    pub(crate) fn open_project_rename_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(&project_idx) = tree_path.first() else {
            return;
        };
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return;
        };
        let target_path = project.path.clone();
        let stem = target_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        self.ui_state.rename_dialog = Some(crate::app::RenameDialogState {
            target_path,
            tree_path,
            buffer: stem,
            error: None,
            is_project_rename: true,
        });
    }

    pub(crate) fn open_project_options_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(&project_idx) = tree_path.first() else {
            return;
        };
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return;
        };
        self.ui_state.project_options = Some(crate::app::ProjectOptionsState {
            project_idx,
            name: project.data.name.clone(),
            directory: project.data.dir.clone(),
            schematic_root: project.data.schematic_root.clone(),
            pcb_file: project.data.pcb_file.clone(),
            library_count: project.data.libraries.len(),
        });
    }

    /// Validate Project — promote the project to active, ensure its
    /// root schematic is open, then dispatch the existing ERC dialog.
    /// `tree_path[0]` is the owning project; we open the schematic
    /// root if no tab from this project is currently active so the
    /// ERC engine targets the right sheet.
    pub(crate) fn run_validate_project(&mut self, tree_path: Vec<usize>) -> iced::Task<Message> {
        let Some(&project_idx) = tree_path.first() else {
            return iced::Task::none();
        };
        let project = match self.document_state.projects.get(project_idx) {
            Some(p) => p,
            None => return iced::Task::none(),
        };
        let project_id = project.id;
        let project_dir = match project.path.parent() {
            Some(d) => d.to_path_buf(),
            None => return iced::Task::none(),
        };
        let schematic_root = project.data.schematic_root.clone();
        self.document_state.active_project = Some(project_id);

        // If the active tab already belongs to this project we're good;
        // otherwise open the schematic_root so ERC has a sheet to walk.
        let active_belongs = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|t| t.project_id == Some(project_id))
            .unwrap_or(false);
        if !active_belongs
            && let Some(root) = schematic_root
        {
            let path = project_dir.join(&root);
            if path.exists() {
                self.handle_document_file_opened(Some(path));
            }
        }
        self.refresh_panel_ctx();
        self.update(Message::OpenErcDialog)
    }

    /// `Add Existing to Project…` — open a multi-select file picker
    /// scoped to schematic / PCB / library extensions. Picked paths
    /// land in [`Message::AddExistingFilePicked`]; the handler copies
    /// any outside the project directory in turn and opens each.
    pub(crate) fn add_existing_to_project(
        &mut self,
        tree_path: Vec<usize>,
    ) -> iced::Task<Message> {
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
            move |paths| Message::AddExistingFilePicked { project_idx, paths },
        )
    }

    /// `Add New ▸ Schematic` — Save-As dialog scoped to the project
    /// directory; result returns through [`Message::AddNewSchematicPicked`].
    /// The handler writes a blank `.snxsch`, registers the entry on
    /// the project, and marks the .snxprj dirty.
    pub(crate) fn add_new_schematic(
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
            move |path| Message::AddNewSchematicPicked { project_idx, path },
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
        if let Err(e) = std::fs::write(&path, serialised.as_bytes()) {
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
    fn register_project_file(
        &mut self,
        project_idx: usize,
        file_path: &std::path::Path,
    ) -> bool {
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
                if loaded
                    .data
                    .libraries
                    .iter()
                    .any(|e| e.path == entry_path)
                {
                    return false;
                }
                loaded.data.libraries.push(signex_types::project::LibraryEntry {
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

    fn open_remove_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(target_path) = self.tree_path_to_file_path(&tree_path) else {
            return;
        };
        let display_name = target_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        self.ui_state.remove_dialog = Some(crate::app::RemoveDialogState {
            target_path,
            tree_path,
            display_name,
        });
    }

    /// Resolve a project-tree path (indices) to the file path on disk
    /// for the leaf node at that position. Multi-root aware: the first
    /// index picks which project's directory to resolve against, so a
    /// leaf under project B isn't accidentally resolved against project
    /// A's parent directory.
    fn tree_path_to_file_path(
        &self,
        tree_path: &[usize],
    ) -> Option<std::path::PathBuf> {
        let node = signex_widgets::tree_view::get_node(
            self.document_state.panel_ctx.project_tree.as_slice(),
            tree_path,
        )?;
        let project_idx = *tree_path.first()?;
        let project = self.document_state.projects.get(project_idx)?;
        let dir = project.path.parent()?;
        Some(dir.join(&node.label))
    }

    pub(crate) fn handle_rename_submit(&mut self) -> iced::Task<Message> {
        let Some(state) = self.ui_state.rename_dialog.clone() else {
            return iced::Task::none();
        };

        if state.is_project_rename {
            return self.handle_project_rename_submit(&state);
        }

        let new_name = state.buffer.trim();
        if new_name.is_empty() {
            self.set_rename_error("Name cannot be empty.");
            return iced::Task::none();
        }
        // Reject path separators so users can't escape the project dir
        // and so a fs::rename never crosses filesystem boundaries.
        if new_name.contains(['/', '\\']) {
            self.set_rename_error("Name cannot contain path separators.");
            return iced::Task::none();
        }
        let parent = match state.target_path.parent() {
            Some(p) => p,
            None => {
                self.set_rename_error("Target file has no parent directory.");
                return iced::Task::none();
            }
        };
        let new_path = parent.join(new_name);
        if new_path == state.target_path {
            // No-op rename — close the dialog silently.
            self.ui_state.rename_dialog = None;
            return iced::Task::none();
        }
        if new_path.exists() {
            self.set_rename_error("A file with that name already exists.");
            return iced::Task::none();
        }

        if let Err(e) = std::fs::rename(&state.target_path, &new_path) {
            self.set_rename_error(&format!("Rename failed: {e}"));
            return iced::Task::none();
        }

        // Update in-memory project data. NOTE: this does not rewrite
        // parent sheets that reference this child sheet — if the
        // renamed file is referenced from a parent `sheet` S-expr,
        // the parent still points to the old name. Recorded as a
        // known limitation; proper rewrite lands with project-write
        // support (v0.9). For now, sheets at the root level (no
        // parent reference) rename cleanly.
        let old_filename = state
            .target_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        // Mutate the *owning* project's data via the dialog's
        // tree_path[0], not the active project — renaming a sheet of
        // project B while project A is active should still touch B.
        // (#54)
        let owner_idx = state.tree_path.first().copied();
        if let (Some(old), Some(idx)) = (old_filename.as_ref(), owner_idx)
            && let Some(loaded) = self.document_state.projects.get_mut(idx)
        {
            for entry in loaded.data.sheets.iter_mut() {
                if &entry.filename == old {
                    entry.filename = new_name.to_string();
                }
            }
            if loaded.data.schematic_root.as_deref() == Some(old.as_str()) {
                loaded.data.schematic_root = Some(new_name.to_string());
            }
        }

        // Update any open tabs + engine-map keys that pointed at the
        // old path. Do this before `refresh_panel_ctx` so the tree
        // rebuild sees the new filenames.
        let old_path = state.target_path.clone();
        for tab in self.document_state.tabs.iter_mut() {
            if tab.path == old_path {
                tab.path = new_path.clone();
                if let Some(stem) = new_path.file_stem().and_then(|s| s.to_str()) {
                    tab.title = stem.to_string();
                }
            }
        }
        if let Some(engine) = self.document_state.engines.remove(&old_path) {
            self.document_state.engines.insert(new_path.clone(), engine);
        }
        if self.document_state.active_path.as_ref() == Some(&old_path) {
            self.document_state.active_path = Some(new_path.clone());
        }

        self.ui_state.rename_dialog = None;
        self.refresh_panel_ctx();
        iced::Task::none()
    }

    fn set_rename_error(&mut self, msg: &str) {
        if let Some(d) = self.ui_state.rename_dialog.as_mut() {
            d.error = Some(msg.to_string());
        }
    }

    /// Project-root rename — `state.target_path` is the project's
    /// `.snxprj`; the buffer is the new *stem*. We rename the trio
    /// `<old>.snxprj` / `<old>.snxsch` / `<old>.snxpcb` together so
    /// `parse_project`'s directory probe still resolves the schematic
    /// + pcb after the rename.
    fn handle_project_rename_submit(
        &mut self,
        state: &crate::app::RenameDialogState,
    ) -> iced::Task<Message> {
        let new_stem = state.buffer.trim();
        if new_stem.is_empty() {
            self.set_rename_error("Name cannot be empty.");
            return iced::Task::none();
        }
        if new_stem.contains(['/', '\\', '.']) {
            self.set_rename_error("Name cannot contain '/', '\\', or '.'.");
            return iced::Task::none();
        }
        let dir = match state.target_path.parent() {
            Some(d) => d.to_path_buf(),
            None => {
                self.set_rename_error("Project file has no parent directory.");
                return iced::Task::none();
            }
        };
        let old_stem = match state.target_path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => {
                self.set_rename_error("Project file has no stem.");
                return iced::Task::none();
            }
        };
        if new_stem == old_stem {
            self.ui_state.rename_dialog = None;
            return iced::Task::none();
        }
        let new_prj = dir.join(format!("{new_stem}.snxprj"));
        if new_prj.exists() {
            self.set_rename_error("A project with that name already exists.");
            return iced::Task::none();
        }
        // Companion schematic / pcb files — rename whichever exist.
        let companions: [(&str, &str); 2] = [("snxsch", "snxsch"), ("snxpcb", "snxpcb")];
        let mut renamed: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
        // Rename the .snxprj first; if any subsequent rename fails we
        // try to roll back so the user isn't left with a half-renamed
        // project.
        if let Err(e) = std::fs::rename(&state.target_path, &new_prj) {
            self.set_rename_error(&format!("Rename failed: {e}"));
            return iced::Task::none();
        }
        renamed.push((state.target_path.clone(), new_prj.clone()));
        for (ext, _) in companions {
            let old_companion = dir.join(format!("{old_stem}.{ext}"));
            if !old_companion.exists() {
                continue;
            }
            let new_companion = dir.join(format!("{new_stem}.{ext}"));
            if new_companion.exists() {
                // Roll back partial renames.
                for (from, to) in renamed.iter().rev() {
                    let _ = std::fs::rename(to, from);
                }
                self.set_rename_error(
                    "A companion file with the new name already exists; aborting.",
                );
                return iced::Task::none();
            }
            if let Err(e) = std::fs::rename(&old_companion, &new_companion) {
                for (from, to) in renamed.iter().rev() {
                    let _ = std::fs::rename(to, from);
                }
                self.set_rename_error(&format!("Rename failed for .{ext}: {e}"));
                return iced::Task::none();
            }
            renamed.push((old_companion, new_companion));
        }

        // Update in-memory project + open tabs / engines for every
        // path that just moved.
        let owner_idx = state.tree_path.first().copied();
        if let Some(idx) = owner_idx
            && let Some(loaded) = self.document_state.projects.get_mut(idx)
        {
            loaded.path = new_prj.clone();
            loaded.data.name = new_stem.to_string();
            // schematic_root / pcb_file are basename strings.
            if loaded.data.schematic_root.as_deref()
                == Some(&format!("{old_stem}.snxsch"))
            {
                loaded.data.schematic_root = Some(format!("{new_stem}.snxsch"));
            }
            if loaded.data.pcb_file.as_deref() == Some(&format!("{old_stem}.snxpcb")) {
                loaded.data.pcb_file = Some(format!("{new_stem}.snxpcb"));
            }
            for entry in loaded.data.sheets.iter_mut() {
                if entry.filename == format!("{old_stem}.snxsch") {
                    entry.filename = format!("{new_stem}.snxsch");
                }
                if entry.name == old_stem {
                    entry.name = new_stem.to_string();
                }
            }
        }

        for (from, to) in &renamed {
            for tab in self.document_state.tabs.iter_mut() {
                if tab.path == *from {
                    tab.path = to.clone();
                    if let Some(stem) = to.file_stem().and_then(|s| s.to_str()) {
                        tab.title = stem.to_string();
                    }
                }
            }
            if let Some(engine) = self.document_state.engines.remove(from) {
                self.document_state.engines.insert(to.clone(), engine);
            }
            if self.document_state.active_path.as_ref() == Some(from) {
                self.document_state.active_path = Some(to.clone());
            }
            if self.document_state.dirty_paths.remove(from) {
                self.document_state.dirty_paths.insert(to.clone());
            }
        }

        self.ui_state.rename_dialog = None;
        self.refresh_panel_ctx();
        iced::Task::none()
    }

    pub(crate) fn handle_remove_confirm(
        &mut self,
        choice: crate::app::RemoveChoice,
    ) -> iced::Task<Message> {
        use crate::app::RemoveChoice;

        let Some(state) = self.ui_state.remove_dialog.take() else {
            return iced::Task::none();
        };

        // Close any tab backed by this file before we touch disk or
        // drop it from the project's in-memory list — otherwise the
        // tab would keep referring to a file that no longer exists.
        let mut close_tasks: Vec<iced::Task<Message>> = Vec::new();
        while let Some(idx) = self
            .document_state
            .tabs
            .iter()
            .position(|t| t.path == state.target_path)
        {
            close_tasks.push(self.close_tab_now(idx));
        }

        if matches!(choice, RemoveChoice::DeleteFile) {
            if let Err(e) = std::fs::remove_file(&state.target_path) {
                crate::diagnostics::log_error(
                    "Failed to delete project file",
                    &anyhow::anyhow!("{e}"),
                );
            }
        }

        // Drop the sheet from the *owning* project's in-memory data so
        // the tree rebuild reflects the removal. Resolves project via
        // the dialog's `tree_path[0]` so removing a sheet of project B
        // doesn't accidentally mutate active project A. Session-scoped:
        // reopening the project rescans disk and (for Exclude)
        // re-discovers the file, until proper project-write support
        // lands. (#54)
        let owner_idx = state.tree_path.first().copied();
        let mut mutated = false;
        let mut project_path: Option<std::path::PathBuf> = None;
        if let Some(filename) = state
            .target_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            && let Some(idx) = owner_idx
            && let Some(loaded) = self.document_state.projects.get_mut(idx)
        {
            project_path = Some(loaded.path.clone());
            let before = loaded.data.sheets.len();
            loaded
                .data
                .sheets
                .retain(|entry| entry.filename != filename);
            if loaded.data.sheets.len() != before {
                mutated = true;
            }
            if loaded.data.schematic_root.as_deref() == Some(filename.as_str()) {
                loaded.data.schematic_root = None;
                mutated = true;
            }
            if loaded.data.pcb_file.as_deref() == Some(filename.as_str()) {
                loaded.data.pcb_file = None;
                mutated = true;
            }
        }
        if mutated
            && let Some(p) = project_path
        {
            // Match Add Existing — Remove from Project also dirties the
            // .snxprj so the project root row gets the red dot until
            // the user saves the metadata change. The on-disk file
            // delete already happened above when DeleteFile was picked;
            // the dirty bit only tracks the project's *list* of
            // children, which lives in the .snxprj.
            self.document_state.dirty_paths.insert(p);
        }

        self.refresh_panel_ctx();
        if close_tasks.is_empty() {
            iced::Task::none()
        } else {
            iced::Task::batch(close_tasks)
        }
    }

    fn open_project_tree_document(
        &mut self,
        tree_path: &[usize],
        filename: String,
    ) -> Result<()> {
        // Multi-project: walk to the owning project via tree_path[0]
        // instead of the active project, so clicking a leaf inside
        // project B opens B's file even when A is the active project.
        // (#54)
        let project_idx = *tree_path
            .first()
            .with_context(|| format!("project tree path was empty for {}", filename))?;
        let project_dir = self
            .document_state
            .projects
            .get(project_idx)
            .and_then(|p| p.path.parent())
            .with_context(|| format!("resolve project directory for {}", filename))?;
        let file_path = project_dir.join(&filename);
        if !file_path.exists() {
            anyhow::bail!("project tree file does not exist: {}", file_path.display());
        }

        if let Some(index) = self
            .document_state
            .tabs
            .iter()
            .position(|tab| tab.path == file_path)
        {
            if index != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = index;
                self.sync_active_tab();
            }
            return Ok(());
        }

        if filename.ends_with(".snxsch") {
            let title = filename.trim_end_matches(".snxsch").to_string();
            // If we parked an engine for this file (closed while dirty),
            // restore it instead of reparsing — Altium parity. Re-parsing
            // would silently discard the user's in-memory edits, which is
            // exactly what `dirty_paths` was built to prevent.
            if self.document_state.engines.contains_key(&file_path)
                && self.document_state.dirty_paths.contains(&file_path)
            {
                self.attach_parked_schematic_tab(file_path, title);
                return Ok(());
            }
            let text = std::fs::read_to_string(&file_path)
                .with_context(|| format!("read schematic {}", file_path.display()))?;
            let schematic = signex_types::format::SnxSchematic::parse(&text)
                .with_context(|| format!("parse schematic {}", file_path.display()))?
                .sheet;
            self.open_schematic_tab(file_path, title, schematic);
            return Ok(());
        }

        if filename.ends_with(".snxpcb") {
            let text = std::fs::read_to_string(&file_path)
                .with_context(|| format!("read pcb {}", file_path.display()))?;
            let board = signex_types::format::SnxPcb::parse(&text)
                .with_context(|| format!("parse pcb {}", file_path.display()))?
                .board;
            let title = filename.trim_end_matches(".snxpcb").to_string();
            self.open_pcb_tab(file_path, title, board);
            return Ok(());
        }

        // Standalone primitive editor tabs — `.snxsym` / `.snxfpt`
        // route through the library subsystem so the same
        // `OpenPrimitiveEditor` path used by the Library panel
        // right-click handles project-tree double-clicks too.
        if filename.ends_with(".snxsym") || filename.ends_with(".snxfpt") {
            let _ = self.handle_open_primitive(file_path);
            return Ok(());
        }

        // `.snxlib/` is a directory package, not a document. Open it
        // as a Library Browser tab in the main canvas area — the
        // browser is the primary surface for working with library rows
        // (table grid + symbol/footprint preview).
        if filename.ends_with(".snxlib") {
            // The browser handler returns a Task; in this synchronous
            // path we can drop it because mount + open are all
            // immediate-side-effecting (no async file dialogs etc.).
            let _ = self.handle_open_library_browser(file_path);
            return Ok(());
        }

        anyhow::bail!("unsupported project tree document: {filename}")
    }
}

/// Recursively set every node's `expanded` state — used by
/// Expand all / Collapse all menu items.
fn set_expanded_recursive(
    nodes: &mut [signex_widgets::tree_view::TreeNode],
    expanded: bool,
) {
    for node in nodes {
        node.expanded = expanded;
        set_expanded_recursive(&mut node.children, expanded);
    }
}

/// Open the OS file manager at `path`, selecting the file when the
/// platform supports it (Windows `explorer /select,` and macOS `open
/// -R`). Linux/Unix fallback uses `xdg-open` on the parent directory
/// since most file managers don't accept a select-file argument.
fn reveal_in_file_manager(path: &std::path::Path) -> anyhow::Result<()> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    {
        // `explorer /select,<path>` opens the containing folder with the
        // file highlighted. If the path is a directory, explorer.exe
        // just opens it. Use a trailing comma so paths with spaces are
        // interpreted as a single argument.
        let arg = if path.is_dir() {
            path.as_os_str().to_owned()
        } else {
            let mut s = std::ffi::OsString::from("/select,");
            s.push(path.as_os_str());
            s
        };
        Command::new("explorer")
            .arg(arg)
            .spawn()
            .map_err(|e| anyhow::anyhow!("explorer.exe failed to spawn: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        // `open -R <path>` reveals the file in Finder; plain `open
        // <dir>` opens a directory.
        let mut cmd = Command::new("open");
        if path.is_file() {
            cmd.arg("-R");
        }
        cmd.arg(path)
            .spawn()
            .map_err(|e| anyhow::anyhow!("open failed to spawn: {e}"))?;
        return Ok(());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // Linux / BSD: fall back to xdg-open on the parent directory.
        let target = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|e| anyhow::anyhow!("xdg-open failed to spawn: {e}"))?;
        Ok(())
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
    super::super::document_files::blank_schematic_sheet_for_new_doc()
}

/// Thin wrapper around `signex_library::enable_project_version_control`
/// — kept here so the dispatch handler can stay synchronous and
/// surface the `LibraryError` as a user-facing string. `gitignore`
/// is the body of the `.gitignore` to write before init (one line
/// per pattern, trailing newline); `None` skips the write entirely
/// so a fully-tracked initial commit stays bit-identical.
fn try_init_project_repo(
    project_dir: &std::path::Path,
    use_lfs: bool,
    gitignore: Option<&str>,
) -> Result<(), signex_library::LibraryError> {
    signex_library::enable_project_version_control(project_dir, use_lfs, gitignore)
}

/// Build the per-row pick-list for the project-scope Enable Version
/// Control modal. Surfaces the `.snxprj`, every sheet, the pcb file,
/// and each `.snxlib` directory as separately tickable rows so the
/// user can opt expensive folders out of the initial commit.
pub(crate) fn collect_track_items(
    project: &crate::app::state::LoadedProject,
    project_dir: &std::path::Path,
) -> Vec<crate::app::TrackItem> {
    let mut items: Vec<crate::app::TrackItem> = Vec::new();
    // The .snxprj itself — always ticked, can't really be excluded
    // sensibly but we surface it so users see the full picture.
    if let Some(name) = project.path.file_name().and_then(|n| n.to_str()) {
        items.push(crate::app::TrackItem {
            absolute: project.path.clone(),
            relative: name.to_string(),
            label: "Project".to_string(),
            is_directory: false,
            tracked: true,
        });
    }
    // Schematic sheets registered on the project.
    for sheet in &project.data.sheets {
        let abs = project_dir.join(&sheet.filename);
        items.push(crate::app::TrackItem {
            absolute: abs,
            relative: sheet.filename.clone(),
            label: "Schematic".to_string(),
            is_directory: false,
            tracked: true,
        });
    }
    // Optional PCB file.
    if let Some(pcb) = project.data.pcb_file.as_ref() {
        items.push(crate::app::TrackItem {
            absolute: project_dir.join(pcb),
            relative: pcb.clone(),
            label: "PCB".to_string(),
            is_directory: false,
            tracked: true,
        });
    }
    // Each library directory the project pulls in. Project-local
    // entries materialise as `.snxlib` directories under
    // `project_dir`; the parent of `resolve_library_path()` is the
    // library's working tree.
    for entry in &project.data.libraries {
        let resolved = project.data.resolve_library_path(entry);
        let Some(lib_root) = resolved.parent() else {
            continue;
        };
        // Only surface project-local libraries — shared/global ones
        // live outside the project dir and don't belong in the
        // project-scope `.gitignore`.
        if !lib_root.starts_with(project_dir) {
            continue;
        }
        let relative = lib_root
            .strip_prefix(project_dir)
            .ok()
            .and_then(|p| p.to_str())
            .unwrap_or_default()
            .to_string();
        if relative.is_empty() {
            continue;
        }
        items.push(crate::app::TrackItem {
            absolute: lib_root.to_path_buf(),
            relative,
            label: "Library".to_string(),
            is_directory: true,
            tracked: true,
        });
    }
    items
}

/// Build the per-row pick-list for the library-scope Enable Version
/// Control modal. Each row is a top-level entry inside the library's
/// `root_dir` — the `library.toml` / `components.tsv` manifest pair
/// plus any of the canonical subdirectories (`classes/` / `symbols/`
/// / `footprints/` / `sims/` / `3dmodels/`) that already exist on
/// disk. Entries that don't exist are skipped so the picker only
/// shows real artefacts.
pub(crate) fn collect_track_items_for_library(
    root_dir: &std::path::Path,
) -> Vec<crate::app::TrackItem> {
    let mut items: Vec<crate::app::TrackItem> = Vec::new();
    // Manifest-shaped files at the library root. Only surface them
    // when present — bare-bones libraries may carry only the
    // `.snxlib` file without a sidecar config.
    let files: &[(&str, &str)] = &[
        ("library.toml", "Config"),
        ("components.tsv", "Components"),
    ];
    for (name, label) in files {
        let abs = root_dir.join(name);
        if !abs.exists() {
            continue;
        }
        items.push(crate::app::TrackItem {
            absolute: abs,
            relative: (*name).to_string(),
            label: (*label).to_string(),
            is_directory: false,
            tracked: true,
        });
    }
    // Canonical subdirectories — surfaced as `Folder` rows. Skip the
    // ones that aren't on disk yet so the picker stays accurate.
    let dirs: &[&str] = &["classes", "symbols", "footprints", "sims", "3dmodels"];
    for name in dirs {
        let abs = root_dir.join(name);
        if !abs.is_dir() {
            continue;
        }
        items.push(crate::app::TrackItem {
            absolute: abs,
            relative: (*name).to_string(),
            label: "Folder".to_string(),
            is_directory: true,
            tracked: true,
        });
    }
    items
}

/// Render the user's tick-list into a `.gitignore` body. Returns an
/// empty string when every row is ticked (no exclusions needed) so
/// the caller can skip writing a no-op file. Directory rows get a
/// trailing slash so git matches the directory and its contents.
pub(crate) fn build_gitignore_body(items: &[crate::app::TrackItem]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for item in items {
        if item.tracked {
            continue;
        }
        if item.relative.is_empty() {
            continue;
        }
        // Use forward slashes — git always wants forward slashes
        // even on Windows. `relative` is already forward-slashed for
        // the items we generate, but be defensive.
        let mut pat = item.relative.replace('\\', "/");
        if item.is_directory && !pat.ends_with('/') {
            pat.push('/');
        }
        lines.push(pat);
    }
    if lines.is_empty() {
        String::new()
    } else {
        let mut out = String::from("# Generated by Signex Enable Version Control\n");
        for line in lines {
            out.push_str(&line);
            out.push('\n');
        }
        out
    }
}
