//! Projects-panel tree-view right-click menu.
//!
//! Data-to-view (#269): the menu is assembled as a `Vec<DropdownEntry>`
//! rendered by the shared `signex_widgets::active_bar_dropdown` widget. The
//! clicked node's [`TreeNodeRole`] (pure, unit-tested) selects which item
//! set the `&self` builder produces.

use super::*;

use super::items::{dd_disabled, dd_msg, dd_planned, save_entry, submenu_launcher};
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

/// Why Refresh All Pricing is greyed when no enabled distributor can price.
pub(super) const NO_PRICING_CLIENT: &str = "No enabled distributor returns prices yet";

/// "Refresh All Pricing" for a library node. The gate is static: it asks only
/// whether an *enabled* distributor (the Distributor APIs list) has a client
/// that can return prices. It does not look at credentials, because `view`
/// must not touch the OS keyring; a distributor without a key or token shows
/// up per row in the Messages panel as an authentication failure. The refresh
/// itself refuses a second concurrent run with a Messages-panel line, so the
/// row stays pressable while one runs.
pub(super) fn refresh_pricing_entry(
    library_ref: Option<crate::app::LibraryRowRef>,
    any_client_prices: bool,
) -> DropdownEntry<Message> {
    if !any_client_prices {
        return dd_disabled(None, "Refresh All Pricing", Some(NO_PRICING_CLIENT));
    }
    match library_ref {
        Some(row) => dd_msg(
            None,
            "Refresh All Pricing",
            "",
            Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                crate::app::ProjectTreeAction::RefreshLibraryPricing(row),
            )),
        ),
        None => dd_disabled(None, "Refresh All Pricing", Some("Library not found")),
    }
}

impl Signex {
    /// Whether "Enable Version Control..." is offered for `dir`: a pure read
    /// of the probed Model fact, never the disk (#1098).
    pub(in crate::app::view) fn offers_enable_version_control(
        &self,
        dir: &std::path::Path,
    ) -> bool {
        self.document_state.project_fs.facts.lacks_git(dir)
    }

    /// Build the Projects-panel tree-view right-click menu. The item set
    /// is derived from the clicked node's [`TreeNodeRole`] — project root
    /// vs library leaf vs openable leaf vs container branch — so the menu
    /// matches what Altium shows in each context. Empty-area clicks are
    /// filtered upstream (no menu shown), so `path` is `Some` whenever
    /// this runs.
    pub(in crate::app::view) fn view_project_tree_context_menu(
        &self,
        ctx: &crate::app::ProjectTreeContextMenuState,
    ) -> Element<'_, Message> {
        use crate::app::ProjectTreeAction as A;
        use signex_widgets::tree_view::{get_node, resolve_path};

        let panel_ctx = &self.document_state.panel_ctx;
        let tokens = &panel_ctx.tokens;
        let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(18);

        let Some(key) = ctx.key.as_ref() else {
            // Background (empty-area) clicks currently produce no menu —
            // Altium shows a "New Project..." flow there which we do not
            // ship yet. Return an empty placeholder so the overlay layer
            // above has something to render into.
            return container(iced::widget::Space::new()).into();
        };
        // Resolve the stored key against the tree as it is NOW, so the
        // actions below carry indices of the row the user right-clicked
        // even if the tree was rebuilt since. A row that no longer exists
        // yields no menu rather than a menu for whatever took its place.
        let Some(path_vec) = resolve_path(panel_ctx.project_tree.as_slice(), key) else {
            return container(iced::widget::Space::new()).into();
        };
        let path = &path_vec;
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

                v.push(dd_planned(
                    None,
                    "Make Project Available Online...",
                    Some("v3.4"),
                ));
                v.push(dd_msg(
                    None,
                    "Validate Project",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::ValidateProject(
                        key.clone(),
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
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                        A::AddExistingToProject(key.clone()),
                    )),
                ));
                v.push(save_entry(can_save));
                v.push(dd_msg(
                    None,
                    "Rename...",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                        A::OpenProjectRenameDialog(key.clone()),
                    )),
                ));
                v.push(DropdownEntry::Separator);
                v.push(if has_tabs {
                    dd_msg(
                        None,
                        "Close Project Documents",
                        "",
                        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                            A::CloseAllDocuments,
                        )),
                    )
                } else {
                    dd_disabled(None, "Close Project Documents", None)
                });
                v.push(dd_msg(
                    None,
                    "Close Project",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::CloseProject(
                        key.clone(),
                    ))),
                ));
                v.push(if has_project_dir {
                    dd_msg(
                        None,
                        "Explore",
                        "",
                        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                            A::RevealInExplorer(key.clone()),
                        )),
                    )
                } else {
                    dd_disabled(None, "Explore", None)
                });
                v.push(DropdownEntry::Separator);
                v.push(dd_planned(None, "Variants...", Some("v1.1")));
                v.push(DropdownEntry::Separator);
                v.push(dd_planned(None, "Project Packager...", Some("v4.2")));
                v.push(dd_planned(None, "Project Releaser...", Some("v5.2")));
                v.push(DropdownEntry::Separator);
                v.push(dd_planned(None, "Share...", Some("v3.4")));
                v.push(dd_msg(
                    None,
                    "Project Options...",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::OpenProjectOptions(
                        key.clone(),
                    ))),
                ));
                // Enable Version Control — only surfaced when the project
                // dir has no `.git/` yet.
                let no_git_yet = path
                    .first()
                    .and_then(|idx| self.document_state.projects.get(*idx))
                    .and_then(|p| p.path.parent())
                    .map(|dir| self.offers_enable_version_control(dir))
                    .unwrap_or(false);
                if no_git_yet {
                    v.push(dd_msg(
                        None,
                        "Enable Version Control...",
                        "",
                        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                            A::OpenEnableVersionControl(key.clone()),
                        )),
                    ));
                }
            }
            TreeNodeRole::Library => {
                // Identity of the right-clicked library, so the actions
                // below cannot be retargeted by a tree rebuild (#742).
                let library_ref = self.library_row_ref(path.as_slice());
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
                v.push(refresh_pricing_entry(
                    library_ref.clone(),
                    self.library
                        .settings
                        .preferred_order
                        .iter()
                        .any(|s| signex_library::source_supports_pricing(*s)),
                ));
                v.push(DropdownEntry::Separator);
                // Enable Version Control on the library directory itself —
                // hidden once the library has a `.git/`.
                if let Some(library_ref) = library_ref.clone()
                    && let Some(lib_dir) = library_ref.root.parent()
                    && self.offers_enable_version_control(lib_dir)
                {
                    v.push(dd_msg(
                        None,
                        "Enable Version Control...",
                        "",
                        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                            A::OpenLibraryEnableVersionControl(library_ref),
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
                        key.clone(),
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
                v.push(match library_ref {
                    Some(library_ref) => dd_msg(
                        None,
                        "Remove from Project...",
                        "",
                        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::RemoveLibrary(
                            library_ref,
                        ))),
                    ),
                    None => dd_disabled(None, "Remove from Project...", None),
                });
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
                        key.clone(),
                    ))),
                ));
                v.push(DropdownEntry::Separator);
                v.push(if has_project_dir {
                    dd_msg(
                        None,
                        "Explore",
                        "",
                        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                            A::RevealInExplorer(key.clone()),
                        )),
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
                        key.clone(),
                    ))),
                ));
                v.push(dd_msg(
                    None,
                    "Remove from Project",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::OpenRemoveDialog(
                        key.clone(),
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
                v.push(dd_planned(None, "Show Differences...", Some("v4.3")));
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
                        key.clone(),
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
