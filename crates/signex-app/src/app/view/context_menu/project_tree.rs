//! Projects-panel tree-view right-click menu.
//!
//! Data-to-view (#269): the menu is assembled as a `Vec<DropdownEntry>`
//! rendered by the shared `signex_widgets::active_bar_dropdown` widget. The
//! clicked node's [`TreeNodeRole`] (pure, unit-tested) selects which item
//! set the `&self` builder produces.

use super::*;

use super::items::{dd_disabled, dd_msg, save_entry, submenu_launcher};
use signex_widgets::active_bar_dropdown::DropdownEntry;
use signex_widgets::tree_view::TreeIcon;

/// Role a project-tree node plays, which selects its right-click menu.
/// Precedence matches the historic `if/else if` chain: a single-segment
/// path is always the project root, then a depth-3 `SnxLibrary` leaf, then
/// any file-backed openable leaf, then a container branch, else unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TreeNodeRole {
    Root,
    Library,
    OpenableLeaf,
    Container,
    Unknown,
}

/// Pure role detection for a project-tree node (see [`TreeNodeRole`]).
pub(super) fn tree_node_role(icon: &TreeIcon, path_len: usize, has_children: bool) -> TreeNodeRole {
    if path_len == 1 {
        return TreeNodeRole::Root;
    }
    // Library leaves under `Libraries ▸ <name>.snxlib` carry the
    // SnxLibrary icon and sit two levels below the project root.
    if matches!(icon, TreeIcon::SnxLibrary) && path_len == 3 {
        return TreeNodeRole::Library;
    }
    if matches!(
        icon,
        TreeIcon::Schematic
            | TreeIcon::Pcb
            | TreeIcon::SnxSchematic
            | TreeIcon::SnxPcb
            | TreeIcon::SnxProject
            | TreeIcon::SnxFootprint
            | TreeIcon::SnxSimulation
            | TreeIcon::SnxLibrary
            | TreeIcon::SnxSymbol
    ) {
        return TreeNodeRole::OpenableLeaf;
    }
    if has_children {
        return TreeNodeRole::Container;
    }
    TreeNodeRole::Unknown
}

impl Signex {
    /// Build the Projects-panel tree-view right-click menu. The item set
    /// is derived from the clicked node's [`TreeNodeRole`] — project root
    /// vs library leaf vs openable leaf vs container branch — so the menu
    /// matches what Altium shows in each context. Empty-area clicks are
    /// filtered upstream (no menu shown), so `path` is `Some` whenever
    /// this runs.
    #[allow(clippy::vec_init_then_push)]
    pub(in crate::app::view) fn view_project_tree_context_menu(
        &self,
        ctx: &crate::app::ProjectTreeContextMenuState,
    ) -> Element<'_, Message> {
        use crate::app::ProjectTreeAction as A;
        use signex_widgets::tree_view::get_node;

        let panel_ctx = &self.document_state.panel_ctx;
        let tokens = &panel_ctx.tokens;
        let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(18);

        let Some(path) = ctx.path.as_ref() else {
            // Background (empty-area) clicks currently produce no menu —
            // Altium shows a "New Project..." flow there which we do not
            // ship yet. Return an empty placeholder so the overlay layer
            // above has something to render into.
            return container(iced::widget::Space::new()).into();
        };
        let Some(node) = get_node(panel_ctx.project_tree.as_slice(), path) else {
            return container(iced::widget::Space::new()).into();
        };

        let role = tree_node_role(&node.icon, path.len(), !node.children.is_empty());
        let has_schematic = panel_ctx.has_schematic;
        let active_submenu = self.interaction_state.context_submenu;

        match role {
            TreeNodeRole::Root => {
                // Project root — Altium's "right-click project" menu. Items
                // not yet wired carry a "vX.Y" right-column badge so the
                // user knows which release lands the feature rather than
                // staring at a silently-greyed row.
                let has_tabs = !self.document_state.tabs.is_empty();
                // Multi-project: gate on the *clicked* project's directory,
                // not the active project's (#54).
                let has_project_dir = path
                    .first()
                    .and_then(|idx| self.document_state.projects.get(*idx))
                    .and_then(|p| p.path.parent())
                    .is_some();
                // Save is enabled when *either* a schematic is active or
                // the right-clicked project's .snxprj is dirty.
                let project_path = path
                    .first()
                    .and_then(|idx| self.document_state.projects.get(*idx))
                    .map(|p| p.path.clone());
                let project_is_dirty = project_path
                    .as_ref()
                    .map(|p| self.document_state.dirty_paths.contains(p))
                    .unwrap_or(false);
                let can_save = has_schematic || project_is_dirty;

                v.push(dd_disabled(
                    None,
                    "Make Project Available Online...",
                    Some("v3.4"),
                ));
                v.push(dd_msg(
                    None,
                    "Validate Project",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::ValidateProject(
                        path.clone(),
                    ))),
                ));
                v.push(submenu_launcher(
                    tokens,
                    None,
                    "Add New to Project",
                    ContextSubmenu::AddNewToProject,
                    active_submenu == Some(ContextSubmenu::AddNewToProject),
                ));
                v.push(dd_msg(
                    None,
                    "Add Existing to Project...",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::AddExistingToProject(
                        path.clone(),
                    ))),
                ));
                v.push(save_entry(can_save));
                v.push(dd_msg(
                    None,
                    "Rename...",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                        A::OpenProjectRenameDialog(path.clone()),
                    )),
                ));
                v.push(DropdownEntry::Separator);
                v.push(if has_tabs {
                    dd_msg(
                        None,
                        "Close Project Documents",
                        "",
                        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::CloseAllDocuments)),
                    )
                } else {
                    dd_disabled(None, "Close Project Documents", None)
                });
                v.push(dd_msg(
                    None,
                    "Close Project",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::CloseProject(
                        path.clone(),
                    ))),
                ));
                v.push(if has_project_dir {
                    dd_msg(
                        None,
                        "Explore",
                        "",
                        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::RevealInExplorer(
                            path.clone(),
                        ))),
                    )
                } else {
                    dd_disabled(None, "Explore", None)
                });
                v.push(DropdownEntry::Separator);
                v.push(dd_disabled(None, "Variants...", Some("v1.1")));
                v.push(DropdownEntry::Separator);
                v.push(dd_disabled(None, "Project Packager...", Some("v4.2")));
                v.push(dd_disabled(None, "Project Releaser...", Some("v5.2")));
                v.push(DropdownEntry::Separator);
                v.push(dd_disabled(None, "Share...", Some("v3.4")));
                v.push(dd_msg(
                    None,
                    "Project Options...",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::OpenProjectOptions(
                        path.clone(),
                    ))),
                ));
                // Enable Version Control — only surfaced when the project
                // dir has no `.git/` yet.
                let no_git_yet = path
                    .first()
                    .and_then(|idx| self.document_state.projects.get(*idx))
                    .and_then(|p| p.path.parent())
                    .map(|dir| !dir.join(".git").exists())
                    .unwrap_or(false);
                if no_git_yet {
                    v.push(dd_msg(
                        None,
                        "Enable Version Control...",
                        "",
                        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                            A::OpenEnableVersionControl(path.clone()),
                        )),
                    ));
                }
            }
            TreeNodeRole::Library => {
                // Library node menu — mirrors Altium's "Add New ▸" submenu:
                // Component opens the New Component modal; Symbol / Footprint
                // create FILES via the adapter. The basic expand / refresh
                // actions stay so empty libraries are still navigable.
                v.push(dd_msg(
                    None,
                    "Add New ▸ Component",
                    "",
                    Message::Menu(crate::menu_bar::MenuMessage::AddLibraryComponent),
                ));
                // F31 — these create FILES, not individual primitives. A
                // `.snxsym` file holds many symbols (Altium parity).
                v.push(dd_msg(
                    None,
                    "Add New ▸ Symbol Library",
                    "",
                    Message::Menu(crate::menu_bar::MenuMessage::AddLibrarySymbol),
                ));
                // v0.13.0 — footprint editor gated off; hide the create
                // entry so the user never reaches a dead flow.
                if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
                    v.push(dd_msg(
                        None,
                        "Add New ▸ Footprint Library",
                        "",
                        Message::Menu(crate::menu_bar::MenuMessage::AddLibraryFootprint),
                    ));
                }
                v.push(DropdownEntry::Separator);
                // Stage 18 distributor refresh stub — resolves the clicked
                // library's `.snxlib` file via the project tree.
                if let Some(lib_path) = self.library_node_path_from_tree(path.as_slice()) {
                    v.push(dd_msg(
                        None,
                        "Refresh All Pricing",
                        "",
                        Message::Library(crate::library::LibraryMessage::LibraryRefreshAllPricing(
                            lib_path,
                        )),
                    ));
                } else {
                    v.push(dd_disabled(None, "Refresh All Pricing", None));
                }
                v.push(DropdownEntry::Separator);
                // Enable Version Control on the library directory itself —
                // hidden once the library has a `.git/`.
                if let Some(lib_file) = self.library_node_path_from_tree(path.as_slice())
                    && let Some(lib_dir) = lib_file.parent()
                    && !lib_dir.join(".git").exists()
                {
                    v.push(dd_msg(
                        None,
                        "Enable Version Control...",
                        "",
                        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                            A::OpenLibraryEnableVersionControl(path.clone()),
                        )),
                    ));
                    v.push(DropdownEntry::Separator);
                }
                let toggle_label = if node.expanded { "Collapse" } else { "Expand" };
                v.push(dd_msg(
                    None,
                    toggle_label,
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::ToggleNode(
                        path.clone(),
                    ))),
                ));
                v.push(dd_msg(
                    None,
                    "Refresh",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::Refresh)),
                ));
                // F23 — "Remove from Project" reuses the RemoveDialog flow.
                v.push(DropdownEntry::Separator);
                v.push(dd_msg(
                    None,
                    "Remove from Project...",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::OpenRemoveDialog(
                        path.clone(),
                    ))),
                ));
            }
            TreeNodeRole::OpenableLeaf => {
                // Sheet / PCB / library leaf — Altium's per-document menu.
                // Open + Explore are wired; Print fires whenever a schematic
                // tab is open; other items are version-badged stubs.
                let has_project_dir = path
                    .first()
                    .and_then(|idx| self.document_state.projects.get(*idx))
                    .and_then(|p| p.path.parent())
                    .is_some();

                v.push(dd_msg(
                    None,
                    "Open",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::OpenNode(
                        path.clone(),
                    ))),
                ));
                v.push(DropdownEntry::Separator);
                v.push(if has_project_dir {
                    dd_msg(
                        None,
                        "Explore",
                        "",
                        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::RevealInExplorer(
                            path.clone(),
                        ))),
                    )
                } else {
                    dd_disabled(None, "Explore", None)
                });
                v.push(DropdownEntry::Separator);
                v.push(dd_msg(
                    None,
                    "Rename...",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::OpenRenameDialog(
                        path.clone(),
                    ))),
                ));
                v.push(dd_msg(
                    None,
                    "Remove from Project",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::OpenRemoveDialog(
                        path.clone(),
                    ))),
                ));
                v.push(DropdownEntry::Separator);
                // Print exports / prints the active document — enabled
                // whenever a schematic tab is open, regardless of which
                // tree row was clicked.
                v.push(if has_schematic {
                    dd_msg(
                        None,
                        "Print...",
                        "",
                        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::PrintActive)),
                    )
                } else {
                    dd_disabled(None, "Print...", None)
                });
                v.push(dd_disabled(None, "Show Differences...", Some("v4.3")));
            }
            TreeNodeRole::Container => {
                // Source Documents / Libraries / Settings folders. Minimum
                // useful action is the tree-widget's own expand/collapse.
                let label = if node.expanded { "Collapse" } else { "Expand" };
                v.push(dd_msg(
                    None,
                    label,
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::ToggleNode(
                        path.clone(),
                    ))),
                ));
                v.push(dd_msg(
                    None,
                    "Expand All",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::ExpandAll)),
                ));
                v.push(dd_msg(
                    None,
                    "Collapse All",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::CollapseAll)),
                ));
                v.push(DropdownEntry::Separator);
                v.push(dd_msg(
                    None,
                    "Refresh",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::Refresh)),
                ));
            }
            TreeNodeRole::Unknown => {
                // Unknown role — give the user at least Refresh so a stale
                // tree can be rebuilt without restarting.
                v.push(dd_msg(
                    None,
                    "Refresh",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::Refresh)),
                ));
            }
        }

        signex_widgets::active_bar_dropdown::view(v, tokens, Some(Self::CONTEXT_MENU_WIDTH))
    }
}
