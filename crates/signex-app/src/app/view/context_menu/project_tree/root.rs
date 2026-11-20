//! The project root's rows (mockup row 20), grouped by intent: add (Add New ›, Add
//! Existing...), check and sync (Validate, Update PCB Document), file (Save,
//! Rename..., Explore), settings (Project Options..., Enable Version
//! Control...), close, then the Planned › launcher.

use super::*;

use super::super::items::{dd_command, planned_launcher, save_entry, submenu_launcher};
use crate::icons as ic;
use signex_widgets::tree_view::TreeKey;

impl Signex {
    /// The project root's menu for the project at `key` / `path`.
    pub(super) fn project_root_rows(
        &self,
        ctx: &crate::app::ProjectTreeContextMenuState,
        key: &TreeKey,
        path: &[usize],
    ) -> Vec<DropdownEntry<Message>> {
        use crate::app::ProjectTreeAction as A;
        let i18n = &*self.ui_state.i18n;
        let tid = self.ui_state.effective_theme_id();
        let open = self.interaction_state.context_submenu;
        let act = |action: A| Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(action));
        let project_dir = self.clicked_project_dir(path);
        // Save writes the active document and the active project, so the
        // row follows the same gate as Ctrl+S and File ▸ Save, not the
        // clicked project's own state.
        let can_save = crate::keymap::AppCommandId::new("save_document").is_ok_and(|id| {
            crate::app::command::is_enabled(&id, &self.command_context(ctx.window))
        });

        let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(20);
        // Add.
        v.push(submenu_launcher(
            Some(ic::icon_menu_plus(tid)),
            &text(i18n, menu_ids::CTX_TREE_ADD_NEW),
            ContextSubmenu::AddNewToProject,
            open == Some(ContextSubmenu::AddNewToProject),
        ));
        v.push(dd_msg(
            Some(ic::icon_menu_file(tid)),
            &text(i18n, menu_ids::CTX_TREE_ADD_EXISTING),
            "",
            act(A::AddExistingToProject(key.clone())),
        ));
        v.push(DropdownEntry::Separator);
        // Check and sync.
        v.push(dd_msg(
            Some(ic::icon_menu_shield(tid)),
            &text(i18n, menu_ids::CTX_TREE_VALIDATE_PROJECT),
            "",
            act(A::ValidateProject(key.clone())),
        ));
        // #1312: the Update PCB dialog for the clicked project.
        v.push(dd_msg(
            Some(ic::icon_menu_sync(tid)),
            &signex_i18n::t!(self.ui_state.i18n, signex_i18n::ids::UPDATE_PCB_TREE_ROW)
                .into_string(),
            &self.keymap_shortcut_label("update_pcb_from_schematic"),
            act(A::UpdatePcb(key.clone())),
        ));
        v.push(DropdownEntry::Separator);
        // File.
        v.push(save_entry(
            i18n,
            Some(ic::icon_menu_save(tid)),
            can_save,
            &self.keymap_shortcut_label("save_document"),
        ));
        v.push(dd_msg(
            Some(ic::icon_menu_pen(tid)),
            &text(i18n, menu_ids::CTX_TREE_RENAME),
            "",
            act(A::OpenProjectRenameDialog(key.clone())),
        ));
        let explore = text(i18n, menu_ids::CTX_TREE_EXPLORE);
        v.push(match project_dir {
            Some(_) => dd_msg(
                Some(ic::icon_menu_folder(tid)),
                &explore,
                "",
                act(A::RevealInExplorer(key.clone())),
            ),
            None => dd_disabled(Some(ic::icon_menu_folder(tid)), &explore, None),
        });
        v.push(DropdownEntry::Separator);
        // Settings.
        v.push(dd_command(
            i18n,
            Some(ic::icon_menu_gear(tid)),
            "open_project_options",
            &self.keymap_shortcut_label("open_project_options"),
            key.first().cloned(),
        ));
        // Enable Version Control: only while the project dir has no `.git/`.
        if project_dir.is_some_and(|dir| self.offers_enable_version_control(dir)) {
            v.push(dd_msg(
                Some(ic::icon_menu_git(tid)),
                &text(i18n, menu_ids::CTX_TREE_ENABLE_VERSION_CONTROL),
                "",
                act(A::OpenEnableVersionControl(key.clone())),
            ));
        }
        v.push(DropdownEntry::Separator);
        // Close.
        let close_documents = text(i18n, menu_ids::CTX_TREE_CLOSE_DOCUMENTS);
        v.push(if self.document_state.tabs.is_empty() {
            dd_disabled(None, &close_documents, None)
        } else {
            dd_msg(None, &close_documents, "", act(A::CloseAllDocuments))
        });
        v.push(dd_msg(
            Some(ic::icon_menu_close(tid)),
            &text(i18n, menu_ids::MENU_PROJECT_CLOSE),
            "",
            act(A::CloseProject(key.clone())),
        ));
        v.push(DropdownEntry::Separator);
        // The features that are not built yet, in one muted flyout.
        v.push(planned_launcher(
            i18n,
            Some(ic::icon_menu_more(tid)),
            &text(i18n, menu_ids::CTX_TREE_PLANNED),
            ContextSubmenu::ProjectPlanned,
            open == Some(ContextSubmenu::ProjectPlanned),
        ));
        v
    }
}
