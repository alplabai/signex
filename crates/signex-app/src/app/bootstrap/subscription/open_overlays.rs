//! The Esc ladder's snapshot of what is open, read from live state.

use super::{OpenOverlays, RecoveryKind};
use crate::app::Signex;

impl Signex {
    /// The snapshot, with `modal_detached` saying which modals show in a
    /// window of their own (their in-window flag then reads closed).
    pub(super) fn open_overlays_where(
        &self,
        modal_detached: impl Fn(crate::app::state::ModalId) -> bool,
    ) -> OpenOverlays {
        OpenOverlays {
            find_replace_open: self.ui_state.find_replace.open
                && !modal_detached(crate::app::state::ModalId::FindReplace),
            find_similar_open: self.ui_state.find_similar.is_some(),
            paste_array_open: self.ui_state.paste_array.is_some(),
            place_image_open: self.ui_state.image_cache.dialog.is_some(),
            union_popover_open: self.ui_state.unions.popover.is_some(),
            smart_paste_open: self.ui_state.smart_paste.is_some(),
            variant_part_edit_open: self.ui_state.variant_part_edit.is_some(),
            kbd_shortcuts_open: self.ui_state.keyboard_shortcuts_open
                && !modal_detached(crate::app::state::ModalId::KeyboardShortcuts),
            about_open: self.ui_state.about_open
                && !modal_detached(crate::app::state::ModalId::About),
            first_run_tour_open: self.ui_state.first_run_tour_open
                && !modal_detached(crate::app::state::ModalId::FirstRunTour),
            autosave_recovery_open: self.document_state.autosave.current_offer().is_some(),
            prefs_open: self.ui_state.preferences_open
                && !modal_detached(crate::app::state::ModalId::Preferences),
            annotate_open: self.ui_state.annotate_dialog_open
                && !modal_detached(crate::app::state::ModalId::AnnotateDialog),
            erc_open: self.ui_state.erc_dialog_open
                && !modal_detached(crate::app::state::ModalId::ErcDialog),
            erc_save_profile_open: self.ui_state.save_profile_dialog.is_some(),
            rename_open: self.ui_state.rename_dialog.is_some(),
            remove_open: self.ui_state.remove_dialog.is_some(),
            enable_vc_open: self.ui_state.enable_version_control.is_some()
                && !modal_detached(crate::app::state::ModalId::EnableVersionControl),
            library_create_options_open: self.library.create_options.is_some()
                && !modal_detached(crate::app::state::ModalId::LibraryCreateOptions),
            passive_calculator_open: self.ui_state.passive_calculator_open
                && !modal_detached(crate::app::state::ModalId::PassiveCalculator),
            annotate_reset_confirm_open: self.ui_state.annotate_reset_confirm
                && !modal_detached(crate::app::state::ModalId::AnnotateResetConfirm),
            reprefix_confirm_open: self.ui_state.reprefix_confirm.is_some()
                && !modal_detached(crate::app::state::ModalId::ReprefixConfirm),
            app_quit_confirm_open: self.ui_state.app_quit_confirm.is_some(),
            project_close_confirm_open: self.ui_state.project_close_confirm.is_some(),
            create_child_sheet_confirm_open: self.ui_state.create_child_sheet_confirm.is_some(),
            grid_properties_open: self.ui_state.grid_properties.is_some()
                && !modal_detached(crate::app::state::ModalId::GridProperties),
            selection_filter_custom_open: self.ui_state.selection_filter_custom.is_some()
                && !modal_detached(crate::app::state::ModalId::SelectionFilterCustom),
            new_project_open: self.ui_state.new_project.is_some(),
            save_project_as_open: self.ui_state.save_project_as.is_some(),
            assign_footprints_open: self.ui_state.assign_footprints.is_some(),
            // A headless apply run has no dialog to close.
            update_pcb_open: self.ui_state.update_pcb.as_ref().is_some_and(|s| !s.apply),
            document_setup_open: self.ui_state.document_setup.is_some(),
            sheet_numbering_open: self.ui_state.sheet_numbering.is_some(),
            sheet_sync_open: self.ui_state.sheet_sync.is_some(),
            board_reannotate_open: self.ui_state.board_reannotate.is_some(),
            create_rooms_open: self.ui_state.rooms.create.is_some(),
            fab_outputs_open: self.ui_state.fab_outputs.is_some(),
            diff_pair_classes_open: self.ui_state.diff_pair_classes.is_some(),
            notebook_generate_open: self.ui_state.notebook_generate.dialog.is_some(),
            board_output_open: self.ui_state.board_output.dialog.is_some(),
            notebook_pdf_open: self.ui_state.notebook_pdf.dialog.is_some(),
            back_annotate_open: self.ui_state.back_annotate.is_some(),
            compare_board_open: self.ui_state.compare_board.is_some(),
            compare_board_confirm_open: self
                .ui_state
                .compare_board
                .as_ref()
                .is_some_and(|s| s.confirming),
            save_layout_open: self.ui_state.workspace_layouts.save.is_some(),
            manage_layouts_open: self.ui_state.workspace_layouts.manage.is_some(),
            manage_layouts_renaming: self
                .ui_state
                .workspace_layouts
                .manage
                .as_ref()
                .is_some_and(|dialog| dialog.rename.is_some()),
            library_picker_open: self.library.picker.is_some()
                && !modal_detached(crate::app::state::ModalId::LibraryPicker),
            library_picker_comparing: self
                .library
                .picker
                .as_ref()
                .is_some_and(|picker| picker.mode.is_comparing()),
            library_document_options_open: self.library.document_options.is_some()
                && !modal_detached(crate::app::state::ModalId::LibraryDocumentOptions),
            library_stock_copy_open: self.library.stock_copy.is_some()
                && !modal_detached(crate::app::state::ModalId::LibraryStockCopy),
            library_updates_open: self.library.library_updates.is_some()
                && !modal_detached(crate::app::state::ModalId::LibraryUpdates),
            library_import_open: self.library.import_dialog.is_some()
                && !modal_detached(crate::app::state::ModalId::LibraryImport),
            library_primitive_picker_open: self.library.primitive_picker.is_some()
                && !modal_detached(crate::app::state::ModalId::LibraryPrimitivePicker),
            close_library_confirm_open: self.library.close_library_confirm.is_some(),
            // The same read `symbol_delete_confirm_overlay` paints from.
            symbol_delete_open: self
                .document_view
                .panel_ctx
                .symbol_editor
                .as_ref()
                .is_some_and(|ctx| ctx.delete_warning.is_some()),
            recovery_kind: self
                .library
                .recovery
                .as_ref()
                .map(|recovery| match recovery {
                    crate::library::recovery::RecoveryDialog::LibraryMissing { .. } => {
                        RecoveryKind::LibraryMissing
                    }
                    crate::library::recovery::RecoveryDialog::GitMissing { .. } => {
                        RecoveryKind::GitMissing
                    }
                    crate::library::recovery::RecoveryDialog::BrokenPrimitiveBinding { .. } => {
                        RecoveryKind::BrokenBinding
                    }
                }),
            error_notice_open: self.document_view.error_notice.is_some(),
            error_notice_detached: modal_detached(crate::app::state::ModalId::ErrorNotice),
            netlist_incomplete_prompt_open: self.document_view.netlist_incomplete_prompt.is_some(),
            netlist_prompt_detached: modal_detached(
                crate::app::state::ModalId::NetlistIncompletePrompt,
            ),
            print_preview_open: self.document_view.preview.is_some()
                && !modal_detached(crate::app::state::ModalId::PrintPreview),
            bom_preview_open: self.document_view.bom_preview.is_some()
                && !modal_detached(crate::app::state::ModalId::BomPreview),
            net_color_custom_open: self.ui_state.net_color_custom.show
                && !modal_detached(crate::app::state::ModalId::NetColorCustom),
            // MUST mirror `delete_confirm_overlay`'s pick
            // (`view/overlays/modals.rs:291-316`): it iterates
            // `library_browsers` and returns on the FIRST entry with a
            // confirm open, so Esc has to cancel that same one or it
            // dismisses a card the user is not looking at.
            //
            // Two browser tabs can hold a confirm at once, and which of
            // them paints is then decided by `HashMap` iteration order.
            // That ambiguity predates this change and is not fixed here —
            // mirroring the builder keeps Esc and paint agreeing on
            // whichever one wins.
            delete_confirm: self
                .library
                .library_browsers
                .iter()
                .find(|(_, browser)| browser.delete_confirm.is_some())
                .map(|(path, _)| path.clone()),
            // MUST mirror `edit_row_modal_overlay`'s pick: first browser
            // with an open edit modal.
            edit_row_modal: self
                .library
                .library_browsers
                .iter()
                .find(|(_, browser)| browser.edit_modal.is_some())
                .map(|(path, _)| path.clone()),
            canvas_context_menu_open: self.interaction_state.context_menu.is_some(),
            // The tab strip's "+N" menu paints in the same slot (mockup row 5).
            tab_context_menu_open: self.interaction_state.tab_context_menu.is_some()
                || self.ui_state.tab_overflow_menu_open,
            project_tree_context_menu_open: self
                .interaction_state
                .project_tree_context_menu
                .is_some(),
            grid_picker_open: self.interaction_state.grid_picker.is_some(),
            zoom_presets_open: self.zoom_presets_shown(),
            // Open is not enough: it must paint, or Esc is swallowed for it.
            grid_snap_popover_open: self.grid_snap_popover_shown(),
            // Painted only while its window shows a board: no rung for a
            // popover left behind by a tab switch.
            board_status_popover_open: self
                .ui_state
                .board_status
                .popover
                .as_ref()
                .is_some_and(|p| self.status_board_target(p.window).is_some()),
            part_menu: self
                .part_menu_site()
                .map(|site| self.part_menu_dismissed(site.owner)),
        }
    }
}
