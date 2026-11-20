//! A library node's rows (mockup row 20) and its Add New › flyout: Add New ›, then
//! Enable Version Control... and Refresh All Pricing, then Collapse/Expand
//! and Refresh, then Remove from Project... last, in the danger ink.

use super::*;

use super::super::items::{danger, submenu_launcher, with_icon};
use crate::icons as ic;
use crate::menu_bar::MenuMessage;
use signex_types::theme::ThemeId;
use signex_widgets::tree_view::TreeKey;

/// The library's Add New flyout. Component opens the New Component modal;
/// Symbol Library and Footprint Library create files (a `.snxsym` holds many
/// symbols). Footprint Library is hidden while the footprint editor is gated
/// off, so the user never reaches a dead flow.
pub(in crate::app::view::context_menu) fn library_add_new_entries(
    i18n: &I18n,
    tid: ThemeId,
) -> Vec<DropdownEntry<Message>> {
    let row = |label, message: MenuMessage| {
        dd_msg(
            Some(ic::icon_menu_library(tid)),
            &text(i18n, label),
            "",
            Message::Menu(message),
        )
    };
    let mut v = vec![
        row(
            menu_ids::CTX_TREE_LIBRARY_ADD_COMPONENT,
            MenuMessage::AddLibraryComponent,
        ),
        row(
            menu_ids::CTX_TREE_LIBRARY_ADD_SYMBOL_LIBRARY,
            MenuMessage::AddLibrarySymbol,
        ),
    ];
    if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
        v.push(row(
            menu_ids::CTX_TREE_LIBRARY_ADD_FOOTPRINT_LIBRARY,
            MenuMessage::AddLibraryFootprint,
        ));
    }
    v
}

impl Signex {
    /// The menu of the library at `key` / `path`; `expanded` picks Collapse
    /// or Expand.
    pub(super) fn library_node_rows(
        &self,
        key: &TreeKey,
        path: &[usize],
        expanded: bool,
    ) -> Vec<DropdownEntry<Message>> {
        use crate::app::ProjectTreeAction as A;
        let i18n = &*self.ui_state.i18n;
        let tid = self.ui_state.effective_theme_id();
        let act = |action: A| Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(action));
        // Identity of the right-clicked library, so the actions below
        // cannot be retargeted by a tree rebuild (#742).
        let library_ref = self.library_row_ref(path);

        let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(10);
        v.push(submenu_launcher(
            Some(ic::icon_menu_plus(tid)),
            &text(i18n, menu_ids::CTX_TREE_LIBRARY_ADD_NEW),
            ContextSubmenu::AddNewToLibrary,
            self.interaction_state.context_submenu == Some(ContextSubmenu::AddNewToLibrary),
        ));
        v.push(DropdownEntry::Separator);
        // Enable Version Control on the library directory itself: hidden
        // once the library has a `.git/`.
        if let Some(library_ref) = library_ref.clone()
            && let Some(lib_dir) = library_ref.root.parent()
            && self.offers_enable_version_control(lib_dir)
        {
            v.push(dd_msg(
                Some(ic::icon_menu_git(tid)),
                &text(i18n, menu_ids::CTX_TREE_ENABLE_VERSION_CONTROL),
                "",
                act(A::OpenLibraryEnableVersionControl(library_ref)),
            ));
        }
        v.push(with_icon(
            refresh_pricing_entry(
                i18n,
                library_ref.clone(),
                self.library
                    .settings
                    .preferred_order
                    .iter()
                    .any(|s| signex_library::source_supports_pricing(*s)),
            ),
            ic::icon_menu_refresh(tid),
        ));
        v.push(DropdownEntry::Separator);
        v.push(toggle_row(i18n, key, expanded));
        v.push(refresh_row(i18n, Some(ic::icon_menu_refresh(tid))));
        v.push(DropdownEntry::Separator);
        // Opens the Remove dialog for the library; greyed (the widget then
        // draws it muted) when the library cannot be resolved.
        let remove = text(i18n, menu_ids::CTX_TREE_REMOVE_LIBRARY);
        v.push(danger(match library_ref {
            Some(library_ref) => dd_msg(
                Some(ic::icon_menu_trash(tid)),
                &remove,
                "",
                act(A::RemoveLibrary(library_ref)),
            ),
            None => dd_disabled(Some(ic::icon_menu_trash(tid)), &remove, None),
        }));
        v
    }
}
