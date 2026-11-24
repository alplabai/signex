//! A document leaf's rows (mockup row 20): Open (the double-click row, strong),
//! Explore, Rename..., then Print..., Show Differences... and Show History,
//! then Remove from Project... last, in the danger ink.

use super::*;

use super::super::items::{command_text, danger, dd_planned_pill, strong};
use crate::app::ProjectTreeAction as A;
use crate::icons as ic;
use signex_widgets::tree_view::TreeKey;

fn act(action: A) -> Message {
    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(action))
}

impl Signex {
    /// The menu of the sheet, board or library file at `key` / `path`.
    pub(super) fn document_leaf_rows(
        &self,
        key: &TreeKey,
        path: &[usize],
        has_schematic: bool,
    ) -> Vec<DropdownEntry<Message>> {
        let i18n = &*self.ui_state.i18n;
        let tid = self.ui_state.effective_theme_id();
        let explore = text(i18n, menu_ids::CTX_TREE_EXPLORE);
        let mut rows = vec![
            strong(dd_msg(
                Some(ic::icon_menu_open(tid)),
                &text(i18n, menu_ids::CTX_TREE_OPEN),
                "",
                act(A::OpenNode(key.clone())),
            )),
            match self.clicked_project_dir(path) {
                Some(_) => dd_msg(
                    Some(ic::icon_menu_folder(tid)),
                    &explore,
                    "",
                    act(A::RevealInExplorer(key.clone())),
                ),
                None => dd_disabled(Some(ic::icon_menu_folder(tid)), &explore, None),
            },
            // No command renames a tree file, so the row has no shortcut.
            dd_msg(
                Some(ic::icon_menu_pen(tid)),
                &text(i18n, menu_ids::CTX_TREE_RENAME),
                "",
                act(A::OpenRenameDialog(key.clone())),
            ),
            DropdownEntry::Separator,
        ];
        rows.extend(self.leaf_history_rows(key, has_schematic));
        rows.extend([
            DropdownEntry::Separator,
            // Opens the Remove dialog (Delete / Exclude / Cancel).
            danger(dd_msg(
                Some(ic::icon_menu_trash(tid)),
                &text(i18n, menu_ids::CTX_TREE_REMOVE),
                "",
                act(A::OpenRemoveDialog(key.clone())),
            )),
        ]);
        rows
    }

    /// Print..., Show Differences... and Show History. Print acts on the
    /// active document, so it is live whenever a schematic tab is open
    /// (`has_schematic`), whichever leaf was clicked.
    fn leaf_history_rows(&self, key: &TreeKey, has_schematic: bool) -> [DropdownEntry<Message>; 3] {
        let i18n = &*self.ui_state.i18n;
        let tid = self.ui_state.effective_theme_id();
        let print = command_text(i18n, "print");
        [
            if has_schematic {
                dd_msg(
                    Some(ic::icon_menu_printer(tid)),
                    &print,
                    "",
                    act(A::PrintActive),
                )
            } else {
                dd_disabled(Some(ic::icon_menu_printer(tid)), &print, None)
            },
            dd_planned_pill(
                i18n,
                Some(ic::icon_menu_diff(tid)),
                &text(i18n, menu_ids::CTX_TREE_SHOW_DIFFERENCES),
            ),
            // Opens the History panel filtered to this file.
            dd_msg(
                Some(ic::icon_menu_history(tid)),
                &text(i18n, menu_ids::CTX_TREE_SHOW_HISTORY),
                "",
                act(A::ShowHistory(key.clone())),
            ),
        ]
    }
}
