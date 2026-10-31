//! Dialog and library-modal overlay builders — Preferences, Find &
//! Replace, keyboard shortcuts, the first-run tour, the simple confirm
//! dialogs, the detachable annotate/ERC dialogs, and the full stack of
//! centered-card-on-dim-backdrop library modals, plus the command
//! palette dropdown and the library-updates modal painted last.
//!
//! Extracted from the former 1223-line `collect_overlays` god-function
//! in `view/mod.rs` (behaviour-preserving decomposition). Each builder
//! owns one overlay's guard + widget tree; ordering is enforced by the
//! `collect_overlays` assembler.

use super::*;
use iced::widget::container;

impl Signex {
    /// Preferences renders inline only if it hasn't been detached into
    /// its own OS window. Open-flow auto-detaches via
    /// `handle_preferences_open_requested`, so this in-window path is
    /// the fallback when the detach failed.
    pub(in crate::app::view) fn preferences_overlay(&self) -> Option<Element<'_, Message>> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let prefs_detached = self.modal_detached(crate::app::state::ModalId::Preferences);
        if !(ui.preferences_open && !prefs_detached) {
            return None;
        }
        let pref_view = crate::preferences::view(crate::preferences::PrefsView {
            nav: ui.preferences_nav,
            draft_theme: ui.preferences_draft_theme,
            saved_theme: ui.theme_id,
            draft_font: &ui.preferences_draft_font,
            draft_grid_style: ui.preferences_draft_grid_style,
            draft_pcb_gpu_render: ui.preferences_draft_pcb_gpu_render,
            draft_annotate_undo: ui.preferences_draft_annotate_undo,
            draft_move_keeps_wires: ui.preferences_draft_move_keeps_wires,
            draft_net_hover_enabled: ui.preferences_draft_net_hover_enabled,
            draft_drag_direction_select: ui.preferences_draft_drag_direction_select,
            draft_increment_step: ui.preferences_draft_increment_step,
            draft_paste_designators: ui.preferences_draft_paste_designators,
            draft_window_controls_placement: ui.preferences_draft_window_controls_placement,
            window_controls_restart_note: self.window_controls_needs_restart(),
            draft_autosave_enabled: ui.preferences_draft_autosave_enabled,
            draft_symbol_grid_size_mm: ui.preferences_draft_symbol_grid_size_mm,
            draft_symbol_grid_style: ui.preferences_draft_symbol_grid_style,
            draft_symbol_pin_selection: ui.preferences_draft_symbol_pin_selection,
            draft_zoom_step_percent: ui.preferences_draft_zoom_step_percent,
            draft_ui_scale_percent: ui.preferences_draft_ui_scale_percent,
            custom_name: ui.custom_theme.as_ref().map(|c| c.name.as_str()),
            theme_status: &ui.preferences_theme_status,
            dirty: ui.preferences_dirty,
            profiles: &ui.profile_manager,
            default_profile: ui.default_rule_profile.as_deref(),
            distributor_settings: &self.library.settings,
            panel_tokens: &document.panel_ctx.tokens,
            draft_component_classes: &ui.preferences_draft_component_classes,
            keymap_editor: &ui.preferences_keymap_editor,
            keymap_status: &ui.preferences_keymap_status,
            keymap_load_error: ui.keymap_load_error.as_deref(),
            keymap_backup: ui.keymap_backup.as_deref(),
            prefs_load_error: ui.prefs_load_error.as_deref(),
            prefs_status: &ui.preferences_prefs_status,
            keymap_search: &ui.preferences_keymap_search,
            keymap_recorder: ui.preferences_keymap_recorder.as_ref(),
            theme_id: ui.effective_theme_id(),
        })
        .map(|m| Message::Preferences(PreferencesMsg::Inner(m)));
        Some(pref_view)
    }

    /// Find & Replace dialog.
    pub(in crate::app::view) fn find_replace_overlay(&self) -> Option<Element<'_, Message>> {
        let ui = &self.ui_state;
        if !ui.find_replace.open || self.modal_detached(crate::app::state::ModalId::FindReplace) {
            return None;
        }
        Some(
            crate::find_replace::view(&ui.find_replace, &self.document_state.panel_ctx.tokens)
                .map(Message::FindReplaceMsg),
        )
    }

    /// Find Similar Objects dialog (#1257). In-window only: it has no
    /// detached form.
    pub(in crate::app::view) fn find_similar_overlay(&self) -> Option<Element<'_, Message>> {
        let state = self.ui_state.find_similar.as_ref()?;
        Some(
            crate::find_similar::view(state, &self.document_state.panel_ctx.tokens).map(|msg| {
                Message::Selection(
                    crate::app::selection_request::SelectionRequest::FindSimilar(msg),
                )
            }),
        )
    }

    /// Paste Array dialog (#1264). In-window only: it has no detached form.
    pub(in crate::app::view) fn paste_array_overlay(&self) -> Option<Element<'_, Message>> {
        let state = self.ui_state.paste_array.as_ref()?;
        Some(
            crate::paste_array::view(state, &self.document_state.panel_ctx.tokens)
                .map(|msg| Message::Edit(crate::app::EditMsg::PasteArray(msg))),
        )
    }

    /// Keyboard-shortcuts reference modal.
    pub(in crate::app::view) fn keyboard_shortcuts_overlay(&self) -> Option<Element<'_, Message>> {
        let ui = &self.ui_state;
        if !ui.keyboard_shortcuts_open
            || self.modal_detached(crate::app::state::ModalId::KeyboardShortcuts)
        {
            return None;
        }
        // `opaque` captures mouse presses inside the layer so they cannot
        // pass through to the layers below it in the `Stack` (#618).
        let built;
        let sections = match &ui.shortcut_sections {
            Some(cached) => cached.as_slice(),
            None => {
                built = crate::keyboard_shortcuts_modal::build_sections(&ui.keymap_profiles);
                built.as_slice()
            }
        };
        Some(iced::widget::opaque(crate::keyboard_shortcuts_modal::view(
            &self.document_state.panel_ctx.tokens,
            ui.effective_theme_id(),
            sections,
        )))
    }

    /// The shortcuts sheet as the content of its own OS window.
    pub(in crate::app::view) fn view_keyboard_shortcuts_window(&self) -> Element<'_, Message> {
        let ui = &self.ui_state;
        let built;
        let sections = match &ui.shortcut_sections {
            Some(cached) => cached.as_slice(),
            None => {
                built = crate::keyboard_shortcuts_modal::build_sections(&ui.keymap_profiles);
                built.as_slice()
            }
        };
        crate::keyboard_shortcuts_modal::window_view(
            &self.document_state.panel_ctx.tokens,
            ui.effective_theme_id(),
            sections,
        )
    }

    /// Help ▸ About Signex dialog.
    pub(in crate::app::view) fn about_overlay(&self) -> Option<Element<'_, Message>> {
        let ui = &self.ui_state;
        if !ui.about_open || self.modal_detached(crate::app::state::ModalId::About) {
            return None;
        }
        Some(iced::widget::opaque(crate::about_modal::view(
            &self.document_state.panel_ctx.tokens,
            ui.effective_theme_id(),
        )))
    }

    /// Tools > Passive Network Calculator modal.
    pub(in crate::app::view) fn passive_calculator_overlay(&self) -> Option<Element<'_, Message>> {
        let ui = &self.ui_state;
        if !ui.passive_calculator_open
            || self.modal_detached(crate::app::state::ModalId::PassiveCalculator)
        {
            return None;
        }
        Some(crate::passive_calculator_modal::view(
            &self.document_state.panel_ctx.tokens,
            ui.effective_theme_id(),
            &ui.passive_calculator,
        ))
    }

    /// First-run onboarding tour overlay.
    pub(in crate::app::view) fn first_run_tour_overlay(&self) -> Option<Element<'_, Message>> {
        if !self.ui_state.first_run_tour_open
            || self.modal_detached(crate::app::state::ModalId::FirstRunTour)
        {
            return None;
        }
        Some(crate::first_run_tour::view(
            &self.document_state.panel_ctx.tokens,
        ))
    }

    /// The recovery-copy prompt (#1164); shows the first queued offer.
    pub(in crate::app::view) fn autosave_recovery_overlay(&self) -> Option<Element<'_, Message>> {
        if self.modal_detached(crate::app::state::ModalId::AutosaveRecovery) {
            return None;
        }
        self.document_state
            .autosave
            .current_offer()
            .map(|offer| self.view_autosave_recovery(offer))
    }

    // ── Simple, non-detachable confirm / property dialogs ────────────
    //
    // These eight used to be pushed into one `Vec` by a single
    // `simple_dialogs_overlay`, and the three below them by
    // `detachable_dialogs_overlay` (#535 part 2a). Splitting them is not
    // cosmetic: both round-2 Esc regressions lived in the *intra-vec*
    // push order inside those composites — grid-properties vs enable-VC,
    // and annotate vs its reset confirm — where paint order was real but
    // invisible from `collect_overlays`. The order is now stated once, in
    // the one place that owns paint order, which is also what lets the
    // upcoming `PAINT_ORDER` express it at all.
    //
    // Pure code motion: same conditions, same `view_*` calls, same
    // sequence.

    /// True while `modal` is showing in its own detached OS window rather
    /// than as an in-window card. The detachable builders below skip
    /// painting in that case, so the modal never renders in both windows
    /// at once.
    ///
    /// Visible to the whole `view` module since #547: `bars.rs` had two
    /// hand-rolled copies of this predicate and a third place —
    /// `has_blocking_modal` — that was missing it entirely.
    ///
    /// `pub(crate)` since #555: the input router asks the same question
    /// to decide which window a detachable modal's keys belong to.
    pub(crate) fn modal_detached(&self, modal: crate::app::state::ModalId) -> bool {
        self.ui_state.windows.values().any(
            |kind| matches!(kind, crate::app::state::WindowKind::DetachedModal(x) if *x == modal),
        )
    }

    pub(in crate::app::view) fn rename_dialog_overlay(&self) -> Option<Element<'_, Message>> {
        (self.ui_state.rename_dialog.is_some()
            && !self.modal_detached(crate::app::state::ModalId::RenameDialog))
        .then(|| self.view_rename_dialog())
    }

    pub(in crate::app::view) fn remove_dialog_overlay(&self) -> Option<Element<'_, Message>> {
        (self.ui_state.remove_dialog.is_some()
            && !self.modal_detached(crate::app::state::ModalId::RemoveDialog))
        .then(|| self.view_remove_dialog())
    }

    pub(in crate::app::view) fn project_close_confirm_overlay(
        &self,
    ) -> Option<Element<'_, Message>> {
        (self.ui_state.project_close_confirm.is_some()
            && !self.modal_detached(crate::app::state::ModalId::ProjectCloseConfirm))
        .then(|| self.view_project_close_confirm())
    }

    pub(in crate::app::view) fn create_child_sheet_confirm_overlay(
        &self,
    ) -> Option<Element<'_, Message>> {
        (self.ui_state.create_child_sheet_confirm.is_some()
            && !self.modal_detached(crate::app::state::ModalId::CreateChildSheet))
        .then(|| self.view_create_child_sheet_confirm())
    }

    pub(in crate::app::view) fn app_quit_confirm_overlay(&self) -> Option<Element<'_, Message>> {
        (self.ui_state.app_quit_confirm.is_some()
            && !self.modal_detached(crate::app::state::ModalId::AppQuitConfirm))
        .then(|| self.view_app_quit_confirm())
    }

    pub(in crate::app::view) fn enable_version_control_overlay(
        &self,
    ) -> Option<Element<'_, Message>> {
        (self.ui_state.enable_version_control.is_some()
            && !self.modal_detached(crate::app::state::ModalId::EnableVersionControl))
        .then(|| self.view_enable_version_control_dialog())
    }

    pub(in crate::app::view) fn grid_properties_overlay(&self) -> Option<Element<'_, Message>> {
        (self.ui_state.grid_properties.is_some()
            && !self.modal_detached(crate::app::state::ModalId::GridProperties))
        .then(|| self.view_grid_properties_dialog())
    }

    pub(in crate::app::view) fn selection_filter_custom_overlay(
        &self,
    ) -> Option<Element<'_, Message>> {
        (self.ui_state.selection_filter_custom.is_some()
            && !self.modal_detached(crate::app::state::ModalId::SelectionFilterCustom))
        .then(|| self.view_selection_filter_custom_dialog())
    }

    // ── Detachable dialogs ───────────────────────────────────────────

    pub(in crate::app::view) fn annotate_dialog_overlay(&self) -> Option<Element<'_, Message>> {
        (self.ui_state.annotate_dialog_open
            && !self.modal_detached(crate::app::state::ModalId::AnnotateDialog))
        .then(|| self.view_annotate_dialog())
    }

    pub(in crate::app::view) fn annotate_reset_confirm_overlay(
        &self,
    ) -> Option<Element<'_, Message>> {
        (self.ui_state.annotate_reset_confirm
            && !self.modal_detached(crate::app::state::ModalId::AnnotateResetConfirm))
        .then(|| self.view_annotate_reset_confirm())
    }

    pub(in crate::app::view) fn reprefix_confirm_overlay(&self) -> Option<Element<'_, Message>> {
        (self.ui_state.reprefix_confirm.is_some()
            && !self.modal_detached(crate::app::state::ModalId::ReprefixConfirm))
        .then(|| self.view_reprefix_confirm())
    }

    pub(in crate::app::view) fn erc_dialog_overlay(&self) -> Option<Element<'_, Message>> {
        (self.ui_state.erc_dialog_open
            && !self.modal_detached(crate::app::state::ModalId::ErcDialog))
        .then(|| self.view_erc_dialog())
    }

    /// The dim backdrop the in-window library cards sit on.
    fn library_scrim<'a>(card: Element<'a, Message>) -> Element<'a, Message> {
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.0, 0.0, 0.0, 0.45,
                ))),
                ..Default::default()
            })
            .into()
    }

    /// The card of a library dialog, shared by the in-window overlay (on a
    /// scrim) and the dialog's own window (centred, no scrim). `owner` names the
    /// browser for the two per-browser dialogs; `None` there means the first
    /// browser with the dialog open.
    pub(in crate::app::view) fn library_dialog_card(
        &self,
        modal: crate::app::state::ModalId,
        owner: Option<&std::path::Path>,
    ) -> Option<Element<'_, Message>> {
        use crate::app::state::ModalId;
        let tokens = &self.document_state.panel_ctx.tokens;
        let lib = &self.library;
        // In a dialog's own window the card fills the window.
        let windowed = owner.is_some() || self.modal_detached(modal);
        let card: Element<'_, Message> = match modal {
            ModalId::LibraryPicker => {
                crate::library::picker::view(lib.picker.as_ref()?, tokens, windowed)
                    .map(Message::Library)
            }
            ModalId::LibraryPrimitivePicker => crate::library::primitive_picker::view(
                lib,
                lib.primitive_picker.as_ref()?,
                tokens,
                windowed,
            )
            .map(Message::Library),
            ModalId::LibraryDocumentOptions => crate::library::document_options::view(
                lib.document_options.as_ref()?,
                tokens,
                windowed,
            )
            .map(Message::Library),
            ModalId::LibraryCreateOptions => {
                crate::library::create_options::view(lib.create_options.as_ref()?, tokens, windowed)
                    .map(Message::Library)
            }
            ModalId::LibraryStockCopy => {
                crate::library::stock_copy::view(lib, lib.stock_copy.as_ref()?, tokens, windowed)
                    .map(Message::Library)
            }
            ModalId::LibraryCloseConfirm => crate::library::close_prompt::view(
                lib,
                lib.close_library_confirm.as_ref()?,
                tokens,
                windowed,
            )
            .map(Message::Library),
            ModalId::LibraryRecovery => {
                crate::library::recovery::view(lib.recovery.as_ref()?, tokens, windowed)
                    .map(Message::Library)
            }
            ModalId::LibraryUpdates => crate::library::updates_dialog::view(
                lib.library_updates.as_ref()?,
                tokens,
                windowed,
            )
            .map(Message::Library),
            ModalId::LibraryEditRow => {
                let (path, edit) = self.browser_modal(owner, |b| b.edit_modal.as_ref())?;
                let classes = self.row_classes_for(path);
                let template = lib.row_template(path, edit.draft.class.as_str());
                crate::library::edit_row_modal::view(
                    path, edit, tokens, windowed, classes, template,
                )
                .map(Message::Library)
            }
            ModalId::LibraryDeleteConfirm => {
                let (path, confirm) = self.browser_modal(owner, |b| b.delete_confirm.as_ref())?;
                crate::library::edit_row_modal::view_delete_confirm(path, confirm, tokens, windowed)
                    .map(Message::Library)
            }
            ModalId::NetColorCustom => {
                if !self.ui_state.net_color_custom.show {
                    return None;
                }
                self.view_net_color_custom_picker()
            }
            _ => return None,
        };
        Some(card)
    }

    /// The browser (and its dialog state) a per-browser dialog is about:
    /// `owner` when given, else the first browser that has one open and no
    /// window of its own.
    fn browser_modal<'a, T>(
        &'a self,
        owner: Option<&std::path::Path>,
        pick: impl Fn(&'a crate::library::state::LibraryBrowserState) -> Option<&'a T>,
    ) -> Option<(&'a std::path::Path, &'a T)> {
        match owner {
            Some(path) => {
                let (key, browser) = self.library.library_browsers.get_key_value(path)?;
                pick(browser).map(|state| (key.as_path(), state))
            }
            None => self
                .library
                .library_browsers
                .iter()
                .find_map(|(key, browser)| {
                    (!self.browser_modal_has_window(key))
                        .then(|| pick(browser).map(|state| (key.as_path(), state)))
                        .flatten()
                }),
        }
    }

    /// Class registry of the editing library, else the user's default.
    fn row_classes_for(
        &self,
        lib_path: &std::path::Path,
    ) -> Vec<crate::fonts::ComponentClassEntry> {
        let from_lib: Vec<crate::fonts::ComponentClassEntry> = self
            .library
            .library_at(lib_path)
            .map(|lib| {
                lib.cached_classes
                    .iter()
                    .map(|c| crate::fonts::ComponentClassEntry {
                        key: c.key.clone(),
                        label: c.label.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        if from_lib.is_empty() {
            self.ui_state.component_classes.clone()
        } else {
            from_lib
        }
    }

    /// Whether `path`'s browser has a dialog window of its own (either kind).
    pub(crate) fn browser_modal_has_window(&self, path: &std::path::Path) -> bool {
        self.ui_state
            .library_modal_owners
            .values()
            .any(|owner| owner == path)
    }

    fn library_overlay(&self, modal: crate::app::state::ModalId) -> Option<Element<'_, Message>> {
        if self.modal_detached(modal) {
            return None;
        }
        self.library_dialog_card(modal, None)
            .map(Self::library_scrim)
    }

    pub(in crate::app::view) fn library_picker_overlay(&self) -> Option<Element<'_, Message>> {
        self.library_overlay(crate::app::state::ModalId::LibraryPicker)
    }

    /// Edit Component Details modal; the first browser with one open (and no
    /// window of its own) is painted.
    pub(in crate::app::view) fn edit_row_modal_overlay(&self) -> Vec<Element<'_, Message>> {
        self.library_dialog_card(crate::app::state::ModalId::LibraryEditRow, None)
            .map(Self::library_scrim)
            .into_iter()
            .collect()
    }

    /// Delete Selected confirm modal; same rule as the edit modal.
    pub(in crate::app::view) fn delete_confirm_overlay(&self) -> Option<Element<'_, Message>> {
        self.library_dialog_card(crate::app::state::ModalId::LibraryDeleteConfirm, None)
            .map(Self::library_scrim)
    }

    pub(in crate::app::view) fn primitive_picker_overlay(&self) -> Option<Element<'_, Message>> {
        self.library_overlay(crate::app::state::ModalId::LibraryPrimitivePicker)
    }

    pub(in crate::app::view) fn document_options_overlay(&self) -> Option<Element<'_, Message>> {
        self.library_overlay(crate::app::state::ModalId::LibraryDocumentOptions)
    }

    pub(in crate::app::view) fn create_options_overlay(&self) -> Option<Element<'_, Message>> {
        self.library_overlay(crate::app::state::ModalId::LibraryCreateOptions)
    }

    pub(in crate::app::view) fn stock_copy_overlay(&self) -> Option<Element<'_, Message>> {
        self.library_overlay(crate::app::state::ModalId::LibraryStockCopy)
    }

    pub(in crate::app::view) fn close_library_confirm_overlay(
        &self,
    ) -> Option<Element<'_, Message>> {
        self.library_overlay(crate::app::state::ModalId::LibraryCloseConfirm)
    }

    pub(in crate::app::view) fn library_recovery_overlay(&self) -> Option<Element<'_, Message>> {
        self.library_overlay(crate::app::state::ModalId::LibraryRecovery)
    }

    /// Command palette dropdown (Ctrl+Shift+P). Painted last so it sits
    /// above every other modal layer; click-outside dismisses via the
    /// standard dismiss_layer pattern. Pushes the dismiss layer then the
    /// dropdown.
    pub(in crate::app::view) fn command_palette_overlay(&self) -> Vec<Element<'_, Message>> {
        if !self.ui_state.command_palette.open {
            return Vec::new();
        }
        vec![
            Self::dismiss_layer(Message::CommandPalette(CommandPaletteMsg::Close)),
            self.view_command_palette_dropdown(),
        ]
    }

    pub(in crate::app::view) fn library_updates_overlay(&self) -> Option<Element<'_, Message>> {
        self.library_overlay(crate::app::state::ModalId::LibraryUpdates)
    }
}
