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
        let prefs_detached = ui.windows.values().any(|kind| {
            matches!(
                kind,
                crate::app::state::WindowKind::DetachedModal(
                    crate::app::state::ModalId::Preferences
                )
            )
        });
        if !(ui.preferences_open && !prefs_detached) {
            return None;
        }
        let pref_view = crate::preferences::view(
            ui.preferences_nav,
            ui.preferences_draft_theme,
            ui.theme_id,
            &ui.preferences_draft_font,
            ui.preferences_draft_power_port_style,
            ui.preferences_draft_label_style,
            ui.preferences_draft_multisheet_style,
            ui.preferences_draft_grid_style,
            ui.preferences_draft_symbol_grid_size_mm,
            ui.preferences_draft_symbol_grid_style,
            ui.custom_theme.as_ref().map(|c| c.name.as_str()),
            ui.preferences_dirty,
            &ui.erc_severity_override,
            &self.library.settings,
            &document.panel_ctx.tokens,
            &ui.preferences_draft_component_classes,
            &ui.preferences_keymap_editor,
            &ui.preferences_keymap_status,
            &ui.preferences_keymap_search,
            ui.preferences_keymap_recorder.as_ref(),
            ui.theme_id,
        )
        .map(|m| Message::Preferences(PreferencesMsg::Inner(m)));
        Some(pref_view)
    }

    /// Find & Replace dialog.
    pub(in crate::app::view) fn find_replace_overlay(&self) -> Option<Element<'_, Message>> {
        let ui = &self.ui_state;
        if !ui.find_replace.open {
            return None;
        }
        Some(
            crate::find_replace::view(&ui.find_replace, &self.document_state.panel_ctx.tokens)
                .map(Message::FindReplaceMsg),
        )
    }

    /// Keyboard-shortcuts reference modal.
    pub(in crate::app::view) fn keyboard_shortcuts_overlay(&self) -> Option<Element<'_, Message>> {
        let ui = &self.ui_state;
        if !ui.keyboard_shortcuts_open {
            return None;
        }
        Some(crate::keyboard_shortcuts_modal::view(
            &self.document_state.panel_ctx.tokens,
            ui.theme_id,
            &ui.keymap_profiles,
        ))
    }

    /// First-run onboarding tour overlay.
    pub(in crate::app::view) fn first_run_tour_overlay(&self) -> Option<Element<'_, Message>> {
        if !self.ui_state.first_run_tour_open {
            return None;
        }
        Some(crate::first_run_tour::view(
            &self.document_state.panel_ctx.tokens,
        ))
    }

    /// The simple, non-detachable confirm/property dialogs. Each is an
    /// independent single-condition push; order matches the inline
    /// sequence exactly (rename → remove → project-close → app-quit →
    /// project-options → enable-VC → grid-properties → selection-filter).
    pub(in crate::app::view) fn simple_dialogs_overlay(&self) -> Vec<Element<'_, Message>> {
        let ui = &self.ui_state;
        let mut out: Vec<Element<'_, Message>> = Vec::new();
        if ui.rename_dialog.is_some() {
            out.push(self.view_rename_dialog());
        }
        if ui.remove_dialog.is_some() {
            out.push(self.view_remove_dialog());
        }
        if ui.project_close_confirm.is_some() {
            out.push(self.view_project_close_confirm());
        }
        if ui.app_quit_confirm.is_some() {
            out.push(self.view_app_quit_confirm());
        }
        if ui.project_options.is_some() {
            out.push(self.view_project_options_dialog());
        }
        if ui.enable_version_control.is_some() {
            out.push(self.view_enable_version_control_dialog());
        }
        if ui.grid_properties.is_some() {
            out.push(self.view_grid_properties_dialog());
        }
        if ui.selection_filter_custom.is_some() {
            out.push(self.view_selection_filter_custom_dialog());
        }
        out
    }

    /// Detachable dialogs (annotate, annotate-reset confirm, ERC). Each
    /// is skipped when its detached OS window owns the view so the modal
    /// never renders in both windows at once.
    pub(in crate::app::view) fn detachable_dialogs_overlay(&self) -> Vec<Element<'_, Message>> {
        let ui = &self.ui_state;
        let modal_detached = |m: crate::app::state::ModalId| -> bool {
            ui.windows.values().any(|kind| {
                matches!(kind, crate::app::state::WindowKind::DetachedModal(x) if *x == m)
            })
        };
        let mut out: Vec<Element<'_, Message>> = Vec::new();
        if ui.annotate_dialog_open && !modal_detached(crate::app::state::ModalId::AnnotateDialog) {
            out.push(self.view_annotate_dialog());
        }
        if ui.annotate_reset_confirm
            && !modal_detached(crate::app::state::ModalId::AnnotateResetConfirm)
        {
            out.push(self.view_annotate_reset_confirm());
        }
        if ui.erc_dialog_open && !modal_detached(crate::app::state::ModalId::ErcDialog) {
            out.push(self.view_erc_dialog());
        }
        out
    }

    /// v0.9 Library — picker modal overlay. Centered card on a dim
    /// backdrop; dismiss-on-ESC handled via the close X.
    pub(in crate::app::view) fn library_picker_overlay(&self) -> Option<Element<'_, Message>> {
        let document = &self.document_state;
        let picker = self.library.picker.as_ref()?;
        let card = crate::library::picker::view(&self.library, picker, &document.panel_ctx.tokens)
            .map(Message::Library);
        let backdrop = container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.0, 0.0, 0.0, 0.45,
                ))),
                ..Default::default()
            });
        Some(backdrop.into())
    }

    /// New Component modal was removed (v0.13); "Add Component" now
    /// appends a draft row directly to the Library Browser table. This
    /// branch keeps the per-library class computation compiled/exercised
    /// until the append-row-direct dispatcher migration lands and the
    /// `library.new_component` state + messages can be pruned. It
    /// deliberately contributes no overlay.
    pub(in crate::app::view) fn new_component_overlay(&self) -> Vec<Element<'_, Message>> {
        if let Some(nc) = self.library.new_component.as_ref() {
            // Class registry is per-library: use the picked library's
            // manifest classes. Falls back to the user's prefs default
            // when no library is selected yet.
            let library_classes: Vec<crate::fonts::ComponentClassEntry> = nc
                .library_idx
                .and_then(|i| self.library.open_libraries.get(i))
                .and_then(|lib| self.library.set.get(lib.library_id))
                .map(|adapter| {
                    adapter
                        .library_classes()
                        .into_iter()
                        .map(|c| crate::fonts::ComponentClassEntry {
                            key: c.key,
                            label: c.label,
                        })
                        .collect()
                })
                .unwrap_or_default();
            let classes_to_show = if library_classes.is_empty() {
                self.ui_state.component_classes.clone()
            } else {
                library_classes
            };
            let _ = (nc, classes_to_show);
        }
        Vec::new()
    }

    /// F25 (2026-05-03) — Edit Component Details modal removed. Row click
    /// selects → Properties panel surfaces detail. Render branch retained
    /// behind `EDIT_MODAL_ENABLED` for one release; prune the supporting
    /// state + dispatchers in a follow-up cleanup pass.
    pub(in crate::app::view) fn edit_row_modal_overlay(&self) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        const EDIT_MODAL_ENABLED: bool = false;
        let mut out: Vec<Element<'_, Message>> = Vec::new();
        #[allow(clippy::overly_complex_bool_expr)]
        for (lib_path, browser_state) in &self.library.library_browsers {
            if EDIT_MODAL_ENABLED && let Some(edit) = browser_state.edit_modal.as_ref() {
                // Class registry is per-library — read from the
                // editing library's manifest. Falls back to the
                // user's prefs default when the library has no
                // classes registered yet.
                let row_classes: Vec<crate::fonts::ComponentClassEntry> = self
                    .library
                    .library_at(lib_path)
                    .and_then(|lib| self.library.set.get(lib.library_id))
                    .map(|adapter| {
                        adapter
                            .library_classes()
                            .into_iter()
                            .map(|c| crate::fonts::ComponentClassEntry {
                                key: c.key,
                                label: c.label,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let row_classes = if row_classes.is_empty() {
                    self.ui_state.component_classes.clone()
                } else {
                    row_classes
                };
                let card = crate::library::edit_row_modal::view(
                    lib_path.as_path(),
                    edit,
                    &document.panel_ctx.tokens,
                    row_classes,
                )
                .map(Message::Library);
                let backdrop = container(card)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .style(|_: &iced::Theme| iced::widget::container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba(
                            0.0, 0.0, 0.0, 0.45,
                        ))),
                        ..Default::default()
                    });
                out.push(backdrop.into());
                break; // only one edit modal at a time
            }
        }
        out
    }

    /// Delete Selected confirm modal (Deliverable D). First browser with
    /// a pending confirm wins.
    pub(in crate::app::view) fn delete_confirm_overlay(&self) -> Option<Element<'_, Message>> {
        let document = &self.document_state;
        for (lib_path, browser_state) in &self.library.library_browsers {
            if let Some(confirm) = browser_state.delete_confirm.as_ref() {
                let card = crate::library::edit_row_modal::view_delete_confirm(
                    lib_path.as_path(),
                    confirm,
                    &document.panel_ctx.tokens,
                )
                .map(Message::Library);
                let backdrop = container(card)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .style(|_: &iced::Theme| iced::widget::container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba(
                            0.0, 0.0, 0.0, 0.45,
                        ))),
                        ..Default::default()
                    });
                return Some(backdrop.into());
            }
        }
        None
    }

    /// Primitive picker (Pick Symbol / Pick Footprint).
    pub(in crate::app::view) fn primitive_picker_overlay(&self) -> Option<Element<'_, Message>> {
        let document = &self.document_state;
        let picker = self.library.primitive_picker.as_ref()?;
        let card = crate::library::primitive_picker::view(
            &self.library,
            picker,
            &document.panel_ctx.tokens,
        )
        .map(Message::Library);
        let backdrop = container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.0, 0.0, 0.0, 0.45,
                ))),
                ..Default::default()
            });
        Some(backdrop.into())
    }

    /// Tools ▸ Document Options modal — Altium SchLib parity.
    pub(in crate::app::view) fn document_options_overlay(&self) -> Option<Element<'_, Message>> {
        let document = &self.document_state;
        let state = self.library.document_options.as_ref()?;
        let card = crate::library::document_options::view(state, &document.panel_ctx.tokens)
            .map(Message::Library);
        let backdrop = container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.0, 0.0, 0.0, 0.45,
                ))),
                ..Default::default()
            });
        Some(backdrop.into())
    }

    /// "Library Options" modal — pops between the New Library Save-As
    /// dialog and the actual `LocalGitAdapter::init` so the user can opt
    /// into Git LFS for binary 3D models before anything hits disk.
    pub(in crate::app::view) fn create_options_overlay(&self) -> Option<Element<'_, Message>> {
        let document = &self.document_state;
        let state = self.library.create_options.as_ref()?;
        let card = crate::library::create_options::view(state, &document.panel_ctx.tokens)
            .map(Message::Library);
        let backdrop = container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.0, 0.0, 0.0, 0.45,
                ))),
                ..Default::default()
            });
        Some(backdrop.into())
    }

    /// Close-Library — Unsaved Drafts confirm modal.
    pub(in crate::app::view) fn close_library_confirm_overlay(&self) -> Option<Element<'_, Message>> {
        let document = &self.document_state;
        let confirm = self.library.close_library_confirm.as_ref()?;
        let card =
            crate::library::close_prompt::view(&self.library, confirm, &document.panel_ctx.tokens)
                .map(Message::Library);
        let backdrop = container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.0, 0.0, 0.0, 0.45,
                ))),
                ..Default::default()
            });
        Some(backdrop.into())
    }

    /// Library recovery dialog (Stage 10). Surfaces missing-snxlib,
    /// missing-.git, and broken primitive bindings as user-facing modals
    /// instead of silent log lines.
    pub(in crate::app::view) fn library_recovery_overlay(&self) -> Option<Element<'_, Message>> {
        let document = &self.document_state;
        let dialog = self.library.recovery.as_ref()?;
        let card = crate::library::recovery::view(dialog, &document.panel_ctx.tokens)
            .map(Message::Library);
        let backdrop = container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.0, 0.0, 0.0, 0.45,
                ))),
                ..Default::default()
            });
        Some(backdrop.into())
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

    /// "Library Updates Available" modal (Stage 16 §3.5). Opened on
    /// schematic open under Team workflow mode when a placed Symbol's
    /// `library_version` drifts from the source row's current version.
    pub(in crate::app::view) fn library_updates_overlay(&self) -> Option<Element<'_, Message>> {
        let document = &self.document_state;
        let state = self.library.library_updates.as_ref()?;
        let card = crate::library::updates_dialog::view(state, &document.panel_ctx.tokens)
            .map(Message::Library);
        let backdrop = container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.0, 0.0, 0.0, 0.45,
                ))),
                ..Default::default()
            });
        Some(backdrop.into())
    }
}
