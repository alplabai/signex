use iced::Task;

use super::super::super::*;

mod add;
mod close_project;
mod open_document;
mod project_actions;
mod remove;
mod rename;
mod version_control;

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
                // Single-click highlight: every leaf click sets the
                // tree's "selected" path so the row gets the active
                // background tint immediately, even before the second
                // click opens it. Persists across panel refreshes via
                // `runtime.rs`.
                self.document_state.panel_ctx.project_tree_selected = Some(path.clone());
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
                set_expanded_recursive(&mut self.document_state.panel_ctx.project_tree, true);
            }
            ProjectTreeAction::CollapseAll => {
                set_expanded_recursive(&mut self.document_state.panel_ctx.project_tree, false);
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
                return Task::perform(async {}, |_| {
                    Message::PrintPreview(PrintPreviewMsg::Requested)
                });
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
            ProjectTreeAction::AddProjectSymbolLibrary(tree_path) => {
                return self.add_project_symbol_library(tree_path);
            }
            ProjectTreeAction::AddProjectFootprintLibrary(tree_path) => {
                return self.add_project_footprint_library(tree_path);
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
}

/// Recursively set every node's `expanded` state — used by
/// Expand all / Collapse all menu items.
fn set_expanded_recursive(nodes: &mut [signex_widgets::tree_view::TreeNode], expanded: bool) {
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
