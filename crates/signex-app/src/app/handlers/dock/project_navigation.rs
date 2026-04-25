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
                if let Some(node) = selected_node
                    && matches!(
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
                    && let Err(error) =
                        self.open_project_tree_document(path, node.label.clone())
                {
                    crate::diagnostics::log_error("Failed to open project tree document", &error);
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
        }
        Task::none()
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
        // Collect dirty paths inside this project's directory.
        // dirty_paths is project-scoped via on-disk parent dir;
        // a file outside `project_dir` is somebody else's problem.
        let dirty: Vec<std::path::PathBuf> = if let Some(dir) = project_dir.as_deref() {
            let mut v: Vec<std::path::PathBuf> = self
                .document_state
                .dirty_paths
                .iter()
                .filter(|p| p.parent() == Some(dir))
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
                // Save every dirty file. For files that match an
                // engine entry (the typical case — close_tab_now's
                // park-on-dirty rule keeps them alive), call
                // `engine.save()` and clear the dirty flag on disk.
                // Files without a live engine can't be saved here;
                // they shouldn't exist if `dirty_paths` is consistent
                // with `engines`, but log + skip just in case.
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
                            }
                        }
                    } else {
                        crate::diagnostics::log_info(format!(
                            "Project close: no live engine for dirty {} — skipping save",
                            path.display()
                        ));
                        self.document_state.dirty_paths.remove(path);
                    }
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
        });
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
        if let Some(filename) = state
            .target_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            && let Some(idx) = owner_idx
            && let Some(loaded) = self.document_state.projects.get_mut(idx)
        {
            loaded
                .data
                .sheets
                .retain(|entry| entry.filename != filename);
            if loaded.data.schematic_root.as_deref() == Some(filename.as_str()) {
                loaded.data.schematic_root = None;
            }
            if loaded.data.pcb_file.as_deref() == Some(filename.as_str()) {
                loaded.data.pcb_file = None;
            }
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

        if filename.ends_with(".standard_sch") || filename.ends_with(".snxsch") {
            let schematic = standard_parser::parse_schematic_file(&file_path)
                .with_context(|| format!("parse schematic {}", file_path.display()))?;
            self.open_schematic_tab(file_path, filename.replace(".standard_sch", ""), schematic);
            return Ok(());
        }

        if filename.ends_with(".standard_pcb") || filename.ends_with(".snxpcb") {
            let board = standard_parser::parse_pcb_file(&file_path)
                .with_context(|| format!("parse pcb {}", file_path.display()))?;
            let title = filename
                .trim_end_matches(".standard_pcb")
                .trim_end_matches(".snxpcb")
                .to_string();
            self.open_pcb_tab(file_path, title, board);
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
