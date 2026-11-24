//! Editor-surface overlay builders — the blocking-modal gate, the
//! export/preview/net-colour top-of-stack overlays, the schematic /
//! footprint / symbol active bars. The in-canvas text-edit input lives in
//! `text_edit.rs`, the Placement Paused card in `placement_paused.rs` and
//! the status-bar panel list in `panel_list.rs`.
//!
//! Extracted from the former 1223-line `collect_overlays` god-function
//! in `view/mod.rs` (behaviour-preserving decomposition). Each builder
//! owns one overlay's guard + widget tree; `collect_overlays` is now a
//! thin assembler that calls them in push order. These are methods of
//! the same `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{column, container};

impl Signex {
    /// True when a modal that must own the entire overlay stack is up —
    /// the error notice, the netlist-incomplete prompt, print preview, or
    /// the custom net-colour picker. Mirrors the inline guard that
    /// early-returns from `collect_overlays` before any tool/menu overlay
    /// is pushed.
    ///
    /// The print-preview term is detachment-filtered (#547). It was not
    /// until then, and the omission cost the main window its entire
    /// overlay stack: once the preview moved into its own OS window
    /// `print_preview_overlay` stopped painting the in-window card, but
    /// this predicate still reported the stack blocked, so
    /// `collect_overlays` took its early return and every one of the five
    /// pre-blocking builders then returned nothing. Ctrl+G grid
    /// properties, the quit confirm, Preferences, the library modals —
    /// all opened their state, none appeared, and nothing on screen
    /// explained why. Detaching a modal is precisely the gesture that
    /// stops it covering this window.
    ///
    /// Only Print Preview needs the filter: `error_notice` and
    /// `netlist_incomplete_prompt` have no `ModalId` and so no detached
    /// form, and the custom net-colour picker
    /// (`ui_state.net_color_custom`) is a different overlay from the
    /// detachable F5 palette (`ModalId::NetColorPalette`,
    /// `ui_state.net_color_palette_open`), which is not one of these four.
    ///
    /// `OpenOverlays::has_blocking_modal` (`app/bootstrap/subscription.rs`)
    /// is the Esc ladder's copy of this predicate, built from a snapshot
    /// rather than live state. The two must agree term for term or Esc
    /// resolves against a stack that is not on screen.
    pub(crate) fn has_blocking_modal(&self) -> bool {
        (self.document_state.error_notice.is_some()
            && !self.modal_detached(crate::app::state::ModalId::ErrorNotice))
            || (self.document_state.netlist_incomplete_prompt.is_some()
                && !self.modal_detached(crate::app::state::ModalId::NetlistIncompletePrompt))
            || (self.document_state.preview.is_some()
                && !self.modal_detached(crate::app::state::ModalId::PrintPreview))
            || (self.ui_state.net_color_custom.show
                && !self.modal_detached(crate::app::state::ModalId::NetColorCustom))
    }

    /// Export-error modal — appears when PDF / netlist / BOM export
    /// hits a user-actionable failure (write permission, invalid path,
    /// empty schematic). Dismiss via OK button or clicking outside.
    /// The card goes to the dismiss helper unpositioned, so a press on its
    /// padding keeps it open (#2377).
    pub(in crate::app::view) fn error_notice_overlay(&self) -> Vec<Element<'_, Message>> {
        if self.modal_detached(crate::app::state::ModalId::ErrorNotice) {
            return Vec::new();
        }
        let Some(card) = self.error_notice_card() else {
            return Vec::new();
        };
        Self::dismiss_and_centre(Message::Overlay(OverlayMsg::DismissErrorNotice), card)
    }

    /// #431 — netlist-incomplete "Export anyway?" prompt. Same modal idiom as
    /// [`Self::error_notice_overlay`], but the card offers TWO actions —
    /// "Export anyway (incomplete)" and "Cancel". Clicking outside cancels
    /// (writes nothing); a press on the card or its padding does not (#2377).
    pub(in crate::app::view) fn netlist_incomplete_prompt_overlay(
        &self,
    ) -> Vec<Element<'_, Message>> {
        if self.modal_detached(crate::app::state::ModalId::NetlistIncompletePrompt) {
            return Vec::new();
        }
        let Some(card) = self.netlist_incomplete_prompt_card() else {
            return Vec::new();
        };
        Self::dismiss_and_centre(Message::Export(ExportMsg::NetlistCancelIncomplete), card)
    }

    /// Print preview overlay — Altium parity: opens as a separate OS
    /// window (see `handle_print_preview_requested → handle_detach_modal`)
    /// so it can be dragged outside the app's client area. Only fall
    /// back to the in-window overlay if the OS window failed to open.
    pub(in crate::app::view) fn print_preview_overlay(&self) -> Option<Element<'_, Message>> {
        let preview_detached = self.modal_detached(crate::app::state::ModalId::PrintPreview);
        if self.document_state.preview.is_some() && !preview_detached {
            Some(self.view_print_preview())
        } else {
            None
        }
    }

    /// BOM preview overlay — same detach-first pattern as Print Preview.
    pub(in crate::app::view) fn bom_preview_overlay(&self) -> Option<Element<'_, Message>> {
        let bom_detached = self.modal_detached(crate::app::state::ModalId::BomPreview);
        if self.document_state.bom_preview.is_some() && !bom_detached {
            Some(self.view_bom_preview())
        } else {
            None
        }
    }

    /// Custom net-colour picker. Bespoke modal (not the iced_aw
    /// ColorPicker) because the user needs a quick-pick palette +
    /// precise RGB inputs side-by-side. The card is placed by the dismiss
    /// helper, so a press on its padding keeps it open (#2377).
    pub(in crate::app::view) fn net_color_custom_overlay(&self) -> Vec<Element<'_, Message>> {
        if !self.ui_state.net_color_custom.show
            || self.modal_detached(crate::app::state::ModalId::NetColorCustom)
        {
            return Vec::new();
        }
        Self::dismiss_and_place(
            Message::NetColor(NetColorMsg::CustomShow(false)),
            self.view_net_color_custom_picker(),
            self.net_color_custom_anchor(),
        )
    }

    /// The footprint editor `window_id` shows, with its path.
    pub(super) fn footprint_editor_in_window(
        &self,
        window_id: iced::window::Id,
    ) -> Option<(&std::path::Path, &crate::app::FootprintDocument)> {
        // A board in sketch mode shows its sketch editor (#2234).
        if self.ui_state.main_window_id == Some(window_id)
            && let Some(editor) = self.board_sketch_editor()
        {
            return Some((editor.path.as_path(), editor));
        }
        let docs = &self.document_state;
        let path = docs
            .tab_shown_in_window(window_id, &self.ui_state)?
            .kind
            .as_footprint_editor()?;
        Some((path, docs.footprint_editors.get(path)?))
    }

    /// The symbol editor `window_id` shows, with its path.
    fn symbol_editor_in_window(
        &self,
        window_id: iced::window::Id,
    ) -> Option<(&std::path::Path, &crate::app::SymbolDocument)> {
        let docs = &self.document_state;
        let path = docs
            .tab_shown_in_window(window_id, &self.ui_state)?
            .kind
            .as_symbol_editor()?;
        Some((path, docs.symbol_editors.get(path)?))
    }

    /// Schematic Active Bar overlay for the schematic `window_id` shows;
    /// its selection set is that window's canvas.
    pub(in crate::app::view) fn schematic_active_bar_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        self.document_state
            .engine_for_window(window_id, &self.ui_state)?;
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let y_offset: f32 = canvas_region::chrome_top(&document.panel_ctx.tokens);
        let bar_has_selection = !interaction.canvas_for_window(window_id).selected.is_empty();
        let bar_has_net_colors = !ui.net_colors.is_empty();
        let bar = crate::active_bar::view_bar(
            interaction.current_tool,
            interaction.corner_rule.mode,
            &interaction.last_tool,
            &document.panel_ctx.tokens,
            self.ui_state.effective_theme_id(),
            bar_has_selection,
            bar_has_net_colors,
            &ui.i18n,
        )
        .map(Message::ActiveBar)
        .boxed();
        Some(
            column![
                iced::widget::Space::new().height(y_offset + crate::active_bar::bar_top_margin()),
                container(super::scroll_guard(bar))
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center),
            ]
            .boxed(),
        )
    }

    /// v0.13 — footprint editor active bar (+ its dropdown overlay)
    /// mounted at the SAME app-view layer as the schematic's, so both
    /// share identical `Space::height(y_offset + 4.0)` math and land on
    /// a pixel-identical screen y.
    pub(in crate::app::view) fn footprint_active_bar_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let Some((_, editor)) = self.footprint_editor_in_window(window_id) else {
            return Vec::new();
        };
        let mut out: Vec<Element<'_, Message>> = Vec::new();
        let y_offset: f32 = canvas_region::chrome_top(&document.panel_ctx.tokens);
        let bar_margin = crate::active_bar::bar_top_margin();
        let theme_id = self.ui_state.effective_theme_id();
        let tokens = &document.panel_ctx.tokens;
        // Task 6 — footprint-native presets (`SelectionFilterKind`),
        // not the schematic `custom_filter_presets`.
        let footprint_presets = &interaction.footprint_filter_presets;
        let bar_items = crate::library::editor::footprint::unified_active_bar::bar_items(
            editor,
            theme_id,
            tokens,
            &self.ui_state.i18n,
        );
        use crate::library::editor::footprint::unified_active_bar as footprint_bar;
        let span = self.footprint_bar_span(window_id);
        let run = footprint_bar::scrolling_slots(editor);
        let left = footprint_bar::bar_left_in(&bar_items, run.clone(), span);
        let bar = signex_widgets::active_bar::view_within(
            bar_items,
            run,
            footprint_bar::bar_max_width(span.width),
            tokens,
        )
        .map(Message::Library)
        .boxed();
        // Translated, not padded: padding would limit a bar wider than the
        // canvas and squeeze its last slots to nothing.
        out.push(
            signex_widgets::Translate::new(super::scroll_guard(bar), (left, y_offset + bar_margin))
                .boxed(),
        );
        // Position the dropdown panel directly below the bar's bottom
        // edge (see the schematic mount for the 42 px derivation).
        let dropdown_top: u16 = ((y_offset + bar_margin) as u16).saturating_add(38);
        if let Some(overlay) =
            crate::library::editor::footprint::unified_active_bar::dropdown_overlay(
                editor,
                theme_id,
                tokens,
                footprint_presets,
                dropdown_top,
                span,
                &self.ui_state.i18n,
            )
        {
            out.push(super::scroll_guard(overlay.map(Message::Library).boxed()));
        }
        out
    }

    /// The canvas between the docks of `window_id`, where the footprint
    /// Active Bar fits and centres: the window's own layout when undocked,
    /// the focused pane when the main window is tiled.
    pub(in crate::app::view) fn footprint_bar_span(
        &self,
        window_id: iced::window::Id,
    ) -> crate::library::editor::footprint::unified_active_bar::BarSpan {
        let undocked = (self.ui_state.main_window_id != Some(window_id)).then_some(window_id);
        let canvas = self.canvas_bounds_in(undocked);
        crate::library::editor::footprint::unified_active_bar::BarSpan {
            x: canvas.x,
            width: canvas.width,
            window_width: self.ui_state.size_of_window(window_id).0,
        }
    }

    /// v0.26 — right-click context menu overlay for the footprint
    /// canvas. Sits above the active-bar dropdown so a long-press menu
    /// is occluded by — never under — its own dismiss layer. Pushes the
    /// dismiss layer then the clamped menu card.
    pub(in crate::app::view) fn footprint_context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        let Some((path, editor)) = self.footprint_editor_in_window(window_id) else {
            return Vec::new();
        };
        let tokens = &document.panel_ctx.tokens;
        let Some(menu_state) = editor.state.context_menu.as_ref() else {
            return Vec::new();
        };
        let Some(card) = crate::library::editor::footprint::context_menu::view_context_menu(
            editor,
            tokens,
            path,
            document.pad_clipboard.is_some(),
            &self.ui_state.active_keymap,
            &self.ui_state.i18n,
        ) else {
            return Vec::new();
        };
        // Dismiss layer — left-click anywhere outside closes the menu.
        // Right-press passes through to the canvas (so a right-drag-to-
        // pan gesture starts pan motion + closes the menu via the
        // CursorMoved threshold).
        let close_msg = Message::Library(
            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                path: path.to_path_buf(),
                msg: crate::library::messages::PrimitiveEdit::Footprint(
                    crate::library::messages::FootprintEditorMsg::CloseContextMenu,
                ),
            },
        );
        let card_msg = card.map(Message::Library).boxed();
        Self::menu_overlay(close_msg, card_msg, (menu_state.x, menu_state.y))
    }

    /// v0.14 — typed-delta "Move Selection By X, Y…" modal for the
    /// footprint editor. A blocking dialog once open; pushes its dismiss
    /// backdrop then the centered card.
    pub(in crate::app::view) fn footprint_move_by_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        let Some((path, editor)) = self.footprint_editor_in_window(window_id) else {
            return Vec::new();
        };
        let tokens = &document.panel_ctx.tokens;
        let Some(card) = crate::library::editor::footprint::move_by_modal::view_move_by_modal(
            editor,
            tokens,
            &self.ui_state.i18n,
        ) else {
            return Vec::new();
        };
        let close_msg = Message::Library(
            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                path: path.to_path_buf(),
                msg: crate::library::messages::PrimitiveEdit::Footprint(
                    crate::library::messages::FootprintEditorMsg::MoveByCancel,
                ),
            },
        );
        Self::dismiss_and_centre(close_msg, card.map(Message::Library).boxed())
    }

    /// #370 — "Align…" dialog for the footprint editor. Mounted at the
    /// same overlay layer as the Move-By modal (a blocking dialog once
    /// open): a dismiss backdrop that routes to `AlignCancel`, then the
    /// centered card.
    pub(in crate::app::view) fn footprint_align_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        let Some((path, editor)) = self.footprint_editor_in_window(window_id) else {
            return Vec::new();
        };
        let tokens = &document.panel_ctx.tokens;
        let Some(card) = crate::library::editor::footprint::align_modal::view_align_modal(
            editor,
            tokens,
            &self.ui_state.i18n,
        ) else {
            return Vec::new();
        };
        let close_msg = Message::Library(
            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                path: path.to_path_buf(),
                msg: crate::library::messages::PrimitiveEdit::Footprint(
                    crate::library::messages::FootprintEditorMsg::AlignCancel,
                ),
            },
        );
        Self::dismiss_and_centre(close_msg, card.map(Message::Library).boxed())
    }

    /// v0.13 — symbol library editor active bar (+ its dropdown overlay)
    /// mounted at the SAME app-view layer as the schematic / footprint
    /// bars.
    pub(in crate::app::view) fn symbol_editor_active_bar_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        let Some((_, editor)) = self.symbol_editor_in_window(window_id) else {
            return Vec::new();
        };
        let mut out: Vec<Element<'_, Message>> = Vec::new();
        let y_offset: f32 = canvas_region::chrome_top(&document.panel_ctx.tokens);
        // Below the editor's own toolbar strip so the bar never covers it.
        let bar_margin = crate::active_bar::bar_top_margin()
            + crate::active_bar::symbol_editor_bar_drop(&document.panel_ctx.tokens);
        let theme_id = self.ui_state.effective_theme_id();
        let tokens = &document.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let bar_items =
            crate::library::editor::symbol::active_bar::bar_items(editor, theme_id, i18n);
        let bar = signex_widgets::active_bar::view(bar_items, tokens)
            .map(Message::Library)
            .boxed();
        out.push(
            column![
                iced::widget::Space::new().height(y_offset + bar_margin),
                container(super::scroll_guard(bar))
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center),
            ]
            .boxed(),
        );
        let dropdown_top: u16 = ((y_offset + bar_margin) as u16).saturating_add(38);
        if let Some(overlay) = crate::library::editor::symbol::active_bar::dropdown_overlay(
            editor,
            theme_id,
            tokens,
            dropdown_top,
            i18n,
        ) {
            out.push(super::scroll_guard(overlay.map(Message::Library).boxed()));
        }
        out
    }

    /// Right-click context menu overlay for the symbol canvas. Mirrors
    /// [`Self::footprint_context_menu_overlay`] 1:1 in structure — see
    /// that method for the coordinate / clamping rationale.
    pub(in crate::app::view) fn symbol_context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        let Some((path, editor)) = self.symbol_editor_in_window(window_id) else {
            return Vec::new();
        };
        let tokens = &document.panel_ctx.tokens;
        let Some(menu_state) = editor.context_menu.as_ref() else {
            return Vec::new();
        };
        let Some(card) = crate::library::editor::symbol::context_menu::view_context_menu(
            editor,
            tokens,
            path,
            &self.ui_state.active_keymap,
            &self.ui_state.i18n,
        ) else {
            return Vec::new();
        };
        let close_msg = Message::Library(
            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                path: path.to_path_buf(),
                msg: crate::library::messages::PrimitiveEdit::Symbol(
                    crate::library::messages::SymbolEditorMsg::CloseContextMenu,
                ),
            },
        );
        let card_msg = card.map(Message::Library).boxed();
        // An expanded Place ▸ grows the card in place (accordion, not a
        // flyout); the placement measures the card it really lays out.
        Self::menu_overlay(close_msg, card_msg, (menu_state.x, menu_state.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::{ModalId, PreviewState, WindowKind};

    /// A print preview with no pages — enough to set the state flag the
    /// blocking predicate reads, which is all these tests care about.
    fn blank_preview() -> PreviewState {
        PreviewState {
            source: std::path::PathBuf::new(),
            pages: Vec::new(),
            raster_source: None,
            selected: 0,
            pdf_options: signex_output::PdfOptions::default(),
            page_setup: crate::app::state::PageSetupInputs::from_options(
                &signex_output::PdfOptions::default(),
            ),
            specific_page_input: "1".to_string(),
            range_note: None,
            zoom: 1.0,
            dialog: Default::default(),
            pan: (0.0, 0.0),
            panning: None,
            sheet_files: Vec::new(),
            selected_files: std::collections::HashSet::new(),
            variants: Vec::new(),
            quality: crate::app::state::PdfQuality::Draft72,
            raster_generation: 0,
            source_path: None,
            export_status: None,
        }
    }

    /// #547 — the painter kept suppressing the whole overlay stack for a
    /// print preview that had moved into its own OS window, while
    /// `print_preview_overlay` had already stopped painting the in-window
    /// card. The main window then painted NOTHING: `collect_overlays`
    /// early-returns on this predicate, and all five pre-blocking
    /// builders return empty. Ctrl+G, the quit confirm, Preferences and
    /// the library modals all opened their state with nothing on screen.
    #[test]
    fn a_detached_print_preview_stops_blocking_the_main_windows_overlay_stack() {
        let (mut app, _boot) = Signex::new();
        app.document_state.preview = Some(blank_preview());
        assert!(
            app.has_blocking_modal(),
            "an in-window print preview does own the whole stack"
        );

        app.ui_state.windows.insert(
            iced::window::Id::unique(),
            WindowKind::DetachedModal(ModalId::PrintPreview),
        );
        assert!(
            !app.has_blocking_modal(),
            "a preview in its own OS window covers nothing in this one"
        );
    }

    /// The filter is one term, not four. `net_color_custom` is the
    /// bespoke picker, a different overlay from the detachable F5 palette
    /// (`ModalId::NetColorPalette`) — detaching that palette must not
    /// unblock the picker.
    #[test]
    fn detaching_the_net_colour_palette_does_not_unblock_the_custom_picker() {
        let (mut app, _boot) = Signex::new();
        app.ui_state.net_color_custom.show = true;
        app.ui_state.windows.insert(
            iced::window::Id::unique(),
            WindowKind::DetachedModal(ModalId::NetColorPalette),
        );
        assert!(
            app.has_blocking_modal(),
            "the custom picker has no detached form and still owns the stack"
        );
    }

    /// The Esc ladder builds its own copy of this predicate from a
    /// snapshot (`OpenOverlays::has_blocking_modal`,
    /// `app/bootstrap/subscription.rs`). If the two disagree, Esc
    /// resolves against a stack that is not on screen — which is exactly
    /// what #547 was. Pin the agreement on the term that diverged.
    #[test]
    fn the_painter_and_the_esc_ladder_agree_about_a_detached_preview() {
        let (mut app, _boot) = Signex::new();
        // `Signex::new()` opens the first-run tour, which is a rung of its
        // own and would answer the Esc before the question below is even
        // asked.
        app.ui_state.first_run_tour_open = false;
        app.document_state.preview = Some(blank_preview());
        app.ui_state.windows.insert(
            iced::window::Id::unique(),
            WindowKind::DetachedModal(ModalId::PrintPreview),
        );
        // The ladder's answer, reached through its public entry point: a
        // detached preview claims no Esc, so it is not blocking there
        // either.
        assert!(
            app.escape_overlay_message().is_none(),
            "the ladder already treats a detached preview as not blocking"
        );
        assert!(
            !app.has_blocking_modal(),
            "so the painter must too, or Esc and paint disagree"
        );
    }
}
