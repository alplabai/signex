//! Projects-panel tree-view right-click menu.
//!
//! Data-to-view (#269): the menu is assembled as a `Vec<DropdownEntry>`
//! rendered by the shared `signex_widgets::active_bar_dropdown` widget. The
//! clicked node's [`TreeNodeRole`] (pure, unit-tested) selects which item
//! set the `&self` builder produces: the project root (`root`), a library
//! (`library`), a document leaf (`leaf`), or a folder. The two flyouts
//! these menus open live in `planned` and `library`.

use super::*;

use super::items::{dd_disabled, dd_msg, text};
use iced::Widget as _;
use signex_i18n::{I18n, menu_ids};
use signex_widgets::active_bar_dropdown::DropdownEntry;
use signex_widgets::tree_view::TreeIcon;

pub(super) mod kind_inks;
mod leaf;
pub(super) mod library;
pub(super) mod planned;
mod root;

/// Role a project-tree node plays, which selects its right-click menu.
/// Precedence: a single-segment path is always the project root, then a
/// depth-3 `SnxLibrary` leaf, then any file-backed openable leaf, then a
/// container branch, else unknown.
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

/// "Refresh All Pricing" for a library node. The gate is static: it asks only
/// whether an *enabled* distributor (the Distributor APIs list) has a client
/// that can return prices. It does not look at credentials, because `view`
/// must not touch the OS keyring; a distributor without a key or token shows
/// up per row in the Messages panel as an authentication failure. The refresh
/// itself refuses a second concurrent run with a Messages-panel line, so the
/// row stays pressable while one runs.
pub(super) fn refresh_pricing_entry(
    i18n: &I18n,
    library_ref: Option<crate::app::LibraryRowRef>,
    any_client_prices: bool,
) -> DropdownEntry<Message> {
    if !any_client_prices {
        return dd_disabled(
            None,
            &text(i18n, menu_ids::CTX_TREE_REFRESH_PRICING),
            Some(&text(i18n, menu_ids::CTX_TREE_NO_PRICING_CLIENT)),
        );
    }
    match library_ref {
        Some(row) => dd_msg(
            None,
            &text(i18n, menu_ids::CTX_TREE_REFRESH_PRICING),
            "",
            Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                crate::app::ProjectTreeAction::RefreshLibraryPricing(row),
            )),
        ),
        None => dd_disabled(
            None,
            &text(i18n, menu_ids::CTX_TREE_REFRESH_PRICING),
            Some(&text(i18n, menu_ids::CTX_TREE_LIBRARY_NOT_FOUND)),
        ),
    }
}

/// The Refresh row (rebuild the tree), with an optional `icon`.
fn refresh_row(i18n: &I18n, icon: Option<iced::widget::svg::Handle>) -> DropdownEntry<Message> {
    dd_msg(
        icon,
        &text(i18n, menu_ids::CTX_TREE_REFRESH),
        "",
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            crate::app::ProjectTreeAction::Refresh,
        )),
    )
}

/// Collapse or Expand for the node at `key`, by whether it is `expanded`.
fn toggle_row(
    i18n: &I18n,
    key: &signex_widgets::tree_view::TreeKey,
    expanded: bool,
) -> DropdownEntry<Message> {
    let label = if expanded {
        text(i18n, menu_ids::CTX_TREE_COLLAPSE)
    } else {
        text(i18n, menu_ids::CTX_TREE_EXPAND)
    };
    dd_msg(
        None,
        &label,
        "",
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            crate::app::ProjectTreeAction::ToggleNode(key.clone()),
        )),
    )
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

    /// The directory of the project the tree row at `path` belongs to:
    /// multi-project gates read the *clicked* project, not the active one
    /// (#54).
    fn clicked_project_dir(&self, path: &[usize]) -> Option<&std::path::Path> {
        path.first()
            .and_then(|idx| self.document_state.projects.get(*idx))
            .and_then(|p| p.path.parent())
    }

    /// Build the Projects-panel tree-view right-click menu. The item set
    /// is derived from the clicked node's [`TreeNodeRole`]: project root,
    /// library leaf, openable leaf or container branch. An empty-area click
    /// gets the one New Project row.
    ///
    /// Returns the card fitted to its labels; an empty placeholder when the
    /// click names no live row. The overlay places it at the click.
    pub(in crate::app::view) fn view_project_tree_context_menu(
        &self,
        ctx: &crate::app::ProjectTreeContextMenuState,
    ) -> super::placement::MenuCard<'_> {
        let panel_ctx = &self.document_state.panel_ctx;
        // No entries (an empty-area click, or a row that no longer exists)
        // renders an empty placeholder so the overlay layer above has
        // something to render into.
        let Some(v) = self.project_tree_entries(ctx) else {
            return super::placement::MenuCard {
                element: container(iced::widget::Space::new()).boxed(),
                flyout_offset: None,
            };
        };
        super::placement::menu_card(v, &panel_ctx.tokens, &panel_ctx.ui_font_name)
    }

    /// The pure row data behind [`Self::view_project_tree_context_menu`]:
    /// `None` when the click names a tree row that no longer exists.
    pub(super) fn project_tree_entries(
        &self,
        ctx: &crate::app::ProjectTreeContextMenuState,
    ) -> Option<Vec<DropdownEntry<Message>>> {
        use crate::app::ProjectTreeAction as A;
        use signex_widgets::tree_view::{get_node, resolve_path};

        // The tree the menu was opened on: the window's own in an undocked
        // window, else the main window's.
        let panel_ctx = ctx
            .window
            .map_or(&self.document_state.panel_ctx, |w| self.panel_ctx_for(w));
        let i18n = &*self.ui_state.i18n;

        // An empty-area click offers New Project (#2232), the dialog every
        // other New Project entry opens.
        let Some(key) = ctx.key.as_ref() else {
            return Some(vec![dd_msg(
                None,
                &signex_i18n::t!(
                    self.ui_state.i18n,
                    signex_i18n::ids::DIALOG_NEW_PROJECT_ENTRY_ROW
                )
                .into_string(),
                &self.keymap_shortcut_label("new_document"),
                Message::Menu(crate::menu_bar::MenuMessage::NewProject),
            )]);
        };
        // Resolve the stored key against the tree as it is NOW, so the
        // actions below carry indices of the row the user right-clicked
        // even if the tree was rebuilt since. A row that no longer exists
        // yields no menu rather than a menu for whatever took its place.
        let path = resolve_path(panel_ctx.project_tree.as_slice(), key)?;
        let node = get_node(panel_ctx.project_tree.as_slice(), &path)?;

        let rows = match tree_node_role(&node.icon, path.len(), !node.children.is_empty()) {
            TreeNodeRole::Root => self.project_root_rows(ctx, key, &path),
            TreeNodeRole::Library => self.library_node_rows(key, &path, node.expanded),
            TreeNodeRole::OpenableLeaf => {
                self.document_leaf_rows(key, &path, panel_ctx.has_schematic)
            }
            // Source Documents / Libraries / Settings folders: the tree's
            // own expand and collapse, then Refresh.
            TreeNodeRole::Container => vec![
                toggle_row(i18n, key, node.expanded),
                dd_msg(
                    None,
                    &text(i18n, menu_ids::CTX_TREE_EXPAND_ALL),
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::ExpandAll)),
                ),
                dd_msg(
                    None,
                    &text(i18n, menu_ids::CTX_TREE_COLLAPSE_ALL),
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::CollapseAll)),
                ),
                DropdownEntry::Separator,
                refresh_row(i18n, None),
            ],
            // Unknown role: at least Refresh, so a stale tree can be rebuilt
            // without restarting.
            TreeNodeRole::Unknown => vec![refresh_row(i18n, None)],
        };
        Some(rows)
    }
}
