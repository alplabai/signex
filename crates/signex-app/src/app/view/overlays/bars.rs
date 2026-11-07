//! Editor-surface overlay builders — the blocking-modal gate, the
//! export/preview/net-colour top-of-stack overlays, the schematic /
//! footprint / symbol active bars, the in-canvas text-edit input, and
//! the status-bar panel list.
//!
//! Extracted from the former 1223-line `collect_overlays` god-function
//! in `view/mod.rs` (behaviour-preserving decomposition). Each builder
//! owns one overlay's guard + widget tree; `collect_overlays` is now a
//! thin assembler that calls them in push order. These are methods of
//! the same `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{column, container, row};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

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
    /// Pushes the dismiss backdrop then the error card.
    pub(in crate::app::view) fn error_notice_overlay(&self) -> Vec<Element<'_, Message>> {
        if self.document_state.error_notice.is_none()
            || self.modal_detached(crate::app::state::ModalId::ErrorNotice)
        {
            return Vec::new();
        }
        vec![
            Self::dismiss_layer(Message::Overlay(OverlayMsg::DismissErrorNotice)),
            self.view_error_notice(),
        ]
    }

    /// #431 — netlist-incomplete "Export anyway?" prompt. Same modal idiom as
    /// [`Self::error_notice_overlay`], but the card offers TWO actions —
    /// "Export anyway (incomplete)" and "Cancel". Clicking outside cancels
    /// (writes nothing). Pushes the dismiss backdrop then the prompt card.
    pub(in crate::app::view) fn netlist_incomplete_prompt_overlay(
        &self,
    ) -> Vec<Element<'_, Message>> {
        if self.document_state.netlist_incomplete_prompt.is_none()
            || self.modal_detached(crate::app::state::ModalId::NetlistIncompletePrompt)
        {
            return Vec::new();
        }
        vec![
            Self::dismiss_layer(Message::Export(ExportMsg::NetlistCancelIncomplete)),
            self.view_netlist_incomplete_prompt(),
        ]
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
    /// precise RGB inputs side-by-side. Pushes the dismiss backdrop
    /// then the picker card.
    pub(in crate::app::view) fn net_color_custom_overlay(&self) -> Vec<Element<'_, Message>> {
        if !self.ui_state.net_color_custom.show
            || self.modal_detached(crate::app::state::ModalId::NetColorCustom)
        {
            return Vec::new();
        }
        vec![
            Self::dismiss_layer(Message::NetColor(NetColorMsg::CustomShow(false))),
            self.view_net_color_custom_picker(),
        ]
    }

    /// Altium-style pause overlay: big centered "Placement Paused" card
    /// with a Resume button. Clicking Resume clears `pre_placement`,
    /// un-pauses the canvas, and drops back to the active placement tool
    /// so the user can keep dropping objects with the edited properties.
    /// v0.13 — Also fires when a footprint editor's placement is paused
    /// so TAB during pad/via/string placement surfaces the same overlay.
    pub(in crate::app::view) fn placement_paused_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let footprint_paused = self
            .footprint_editor_in_window(window_id)
            .is_some_and(|(_, ed)| ed.state.placement_paused);
        if !(interaction.canvas_for_window(window_id).placement_paused || footprint_paused) {
            return None;
        }
        let tokens = &document.panel_ctx.tokens;
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let text_c = crate::styles::ti(tokens.text);
        let accent_c = crate::styles::ti(tokens.accent);
        let border_c = crate::styles::ti(tokens.border);
        let card = container(
            column![
                iced::widget::text("⏸").size(TS.banner).color(accent_c),
                iced::widget::text("Placement Paused")
                    .size(TS.heading)
                    .color(text_c),
                iced::widget::text(
                    "Editing properties in the panel. Click Resume to keep placing."
                )
                .size(TS.body)
                .color(text_c),
                iced::widget::Space::new().height(6.0),
                iced::widget::button(
                    iced::widget::text("Resume Placement")
                        .size(TS.label)
                        .color(iced::Color::WHITE)
                )
                .padding([6, 18])
                .on_press(Message::Tool(ToolMessage::ResumePlacement))
                .style(iced::widget::button::primary),
            ]
            .spacing(SP.m)
            .align_x(iced::Alignment::Center),
        )
        .padding(24)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(iced::Color {
                a: 0.92,
                ..panel_bg
            })),
            border: iced::Border {
                color: border_c,
                width: 1.0,
                radius: 8.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });
        Some(
            container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .boxed(),
        )
    }

    /// The footprint editor `window_id` shows, with its path.
    fn footprint_editor_in_window(
        &self,
        window_id: iced::window::Id,
    ) -> Option<(&std::path::Path, &crate::app::FootprintDocument)> {
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
            interaction.draw_mode,
            &interaction.last_tool,
            &document.panel_ctx.tokens,
            self.ui_state.effective_theme_id(),
            bar_has_selection,
            bar_has_net_colors,
        )
        .map(Message::ActiveBar)
        .boxed();
        Some(
            column![
                iced::widget::Space::new().height(
                    y_offset + crate::active_bar::bar_top_margin(&document.panel_ctx.tokens)
                ),
                container(bar)
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
        let bar_margin = crate::active_bar::bar_top_margin(&document.panel_ctx.tokens);
        let theme_id = self.ui_state.effective_theme_id();
        let tokens = &document.panel_ctx.tokens;
        // Task 6 — footprint-native presets (`SelectionFilterKind`),
        // not the schematic `custom_filter_presets`.
        let footprint_presets = &interaction.footprint_filter_presets;
        let bar_items = crate::library::editor::footprint::unified_active_bar::bar_items(
            editor, theme_id, tokens,
        );
        let bar = signex_widgets::active_bar::view(bar_items, tokens)
            .map(Message::Library)
            .boxed();
        out.push(
            column![
                iced::widget::Space::new().height(y_offset + bar_margin),
                container(bar)
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center),
            ]
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
                self.ui_state.window_size.0,
            )
        {
            out.push(overlay.map(Message::Library).boxed());
        }
        out
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
        // Conservative footprint so the card stays on screen near the
        // right / bottom edges.
        const FOOTPRINT_MENU_ROWS: usize = 12;
        self.menu_overlay(
            window_id,
            close_msg,
            card_msg,
            (menu_state.x, menu_state.y),
            Self::CONTEXT_MENU_WIDTH,
            FOOTPRINT_MENU_ROWS,
        )
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
        let Some(card) =
            crate::library::editor::footprint::move_by_modal::view_move_by_modal(editor, tokens)
        else {
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
        vec![
            Self::dismiss_layer(close_msg),
            container(card.map(Message::Library).boxed())
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .boxed(),
        ]
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
        let Some(card) =
            crate::library::editor::footprint::align_modal::view_align_modal(editor, tokens)
        else {
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
        vec![
            Self::dismiss_layer(close_msg),
            container(card.map(Message::Library).boxed())
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .boxed(),
        ]
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
        let bar_margin = crate::active_bar::bar_top_margin(&document.panel_ctx.tokens);
        let theme_id = self.ui_state.effective_theme_id();
        let tokens = &document.panel_ctx.tokens;
        let bar_items = crate::library::editor::symbol::active_bar::bar_items(editor, theme_id);
        let bar = signex_widgets::active_bar::view(bar_items, tokens)
            .map(Message::Library)
            .boxed();
        out.push(
            column![
                iced::widget::Space::new().height(y_offset + bar_margin),
                container(bar)
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
        ) {
            out.push(overlay.map(Message::Library).boxed());
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
        let Some(card) =
            crate::library::editor::symbol::context_menu::view_context_menu(editor, tokens, path)
        else {
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
        let place_open = menu_state.open_submenu
            == Some(crate::library::editor::symbol::state::SymbolContextSubmenu::Place);
        // The collapsed menu is the stable 6-row top-level set (Place ▸,
        // Join, Delete, Select All, Deselect All, Fit; locked by
        // rows::tests::top_level_ids_are_stable); an expanded Place ▸ adds
        // its rows in place (accordion, not a flyout). The estimate must be
        // tight: the bottom-edge flip lifts the card by exactly its height.
        const TOP_LEVEL_ROWS: usize = 6;
        let expanded_rows = if place_open {
            crate::library::editor::symbol::context_menu::PLACE_TOOLS.len()
        } else {
            0
        };
        self.menu_overlay(
            window_id,
            close_msg,
            card_msg,
            (menu_state.x, menu_state.y),
            crate::library::editor::symbol::context_menu::MENU_WIDTH,
            TOP_LEVEL_ROWS + expanded_rows,
        )
    }

    /// In-canvas text-edit input — the floating `text_input` anchored on
    /// top of the label being edited. Converts the object's world
    /// position through the live camera into a window-absolute screen
    /// position each frame.
    pub(in crate::app::view) fn text_edit_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        document.engine_for_window(window_id, &self.ui_state)?;
        let edit_state = interaction.editing_text.as_ref()?;
        let text = edit_state.text.clone();
        let placeholder = crate::app::handlers::canvas::editor_placeholder(edit_state.kind);
        // Object world position -> window position through the live camera
        // and the canvas rectangle, so the editor opens on the label.
        let camera = interaction.canvas_for_window(window_id).live_camera();
        let (abs_x, abs_y) = canvas_region::world_to_window(
            (edit_state.world_x, edit_state.world_y),
            camera,
            self.canvas_bounds(),
        );
        // Font size in pixels matches the rendered label (10 pt ≈ 1.8 mm).
        let font_px = (camera.2 * 1.8).clamp(10.0, 64.0);
        // Estimate width from the text in the canvas face to keep the input
        // snug, with two characters of room to type into.
        let canvas_font_name = document.panel_ctx.canvas_font_name.as_str();
        let canvas_family = signex_types::font::nonblank_family(canvas_font_name);
        let approx_w =
            (signex_types::font::estimate_text_width_px(&edit_state.text, font_px, canvas_family)
                + 2.0 * font_px * signex_types::font::ESTIMATE_FALLBACK_ADVANCE_EM)
                .max(60.0);
        // Sit the input's baseline on the label text.
        let abs_x = abs_x - 2.0;
        let abs_y = abs_y - font_px - 2.0;
        let paper_c = crate::styles::ti(document.panel_ctx.tokens.paper);
        let text_c = crate::styles::ti(document.panel_ctx.tokens.text);
        let accent_c = crate::styles::ti(document.panel_ctx.tokens.accent);
        Some(
            column![
                iced::widget::Space::new().height(abs_y.max(0.0)).boxed(),
                row![
                    iced::widget::Space::new().width(abs_x.max(0.0)).boxed(),
                    container(
                        iced::widget::text_input(placeholder, text)
                            .id(crate::app::contracts::TEXT_EDIT_INPUT_ID.clone())
                            .on_input(|t| Message::TextEdit(TextEditMsg::Changed(t)))
                            .on_submit(Message::TextEdit(TextEditMsg::Submit))
                            .font(crate::fonts::canvas_iced_font(canvas_font_name))
                            .size(font_px)
                            .padding([1.0, SP.xs])
                            .width(approx_w)
                            .style(move |_: &iced::Theme, _status: iced::widget::text_input::Status| {
                                iced::widget::text_input::Style {
                                    background: iced::Background::Color(paper_c),
                                    border: iced::Border {
                                        color: accent_c,
                                        width: 1.0,
                                        radius: 0.0.into(),
                                        ..iced::Border::default()
                                    },
                                    placeholder: text_c,
                                    value: text_c,
                                    selection: accent_c,
                                }
                            }),
                    )
                    .boxed(),
                ]
                .boxed(),
            ]
            .boxed(),
        )
    }

    /// v0.18.10 status-bar panel list popup. Anchored above the "Panels"
    /// button in the bottom-right of the status bar; each row shows a ✓
    /// when the panel is open somewhere (docked, floating, or detached).
    /// Pushes the dismiss layer then the popup.
    pub(in crate::app::view) fn panel_list_overlay(&self) -> Vec<Element<'_, Message>> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        if !ui.panel_list_open {
            return Vec::new();
        }
        let tokens = &document.panel_ctx.tokens;
        let graphite = crate::styles::is_graphite(tokens);
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let has_sch = document.panel_ctx.has_schematic;
        let has_pcb = document.panel_ctx.has_pcb;
        // A panel counts as "open" if it lives in any dock region,
        // floats on top, or owns a detached OS window.
        let is_open = |k: crate::panels::PanelKind| self.is_panel_open(k);
        let panel_items: Vec<Element<'_, Message>> = crate::panels::ALL_PANELS
            .iter()
            .filter(|&&kind| (!kind.needs_schematic() || has_sch) && (!kind.needs_pcb() || has_pcb))
            .map(|&kind| {
                // Altium parity: a leading ✓ column marks open panels
                // so the user can see at a glance which ones are
                // already somewhere on screen. Clicking an open panel
                // still fires OpenPanel — the dock brings it forward.
                let check = if is_open(kind) { "\u{2713}" } else { "" };
                let check_c = if graphite {
                    crate::status_bar::check_ink(tokens)
                } else {
                    text_muted
                };
                let row_btn = iced::widget::button(
                    iced::widget::row![
                        iced::widget::container(
                            iced::widget::text(check.to_string())
                                .size(TS.body)
                                .color(check_c),
                        )
                        .width(Length::Fixed(16.0)),
                        iced::widget::text(kind.label().to_string())
                            .size(TS.body)
                            .color(text_c),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .width(Length::Fill)
                .on_press(Message::Overlay(OverlayMsg::OpenPanel(kind)));
                if graphite {
                    row_btn
                        .padding([0.0, SP.m])
                        .height(crate::status_bar::popup_row_height())
                        .style(crate::status_bar::popup_row(tokens))
                        .boxed()
                } else {
                    row_btn
                        .padding([SP.s, SP.l])
                        .style(crate::styles::menu_item(tokens))
                        .boxed()
                }
            })
            .collect();

        // Drop the scrollable wrapper — the list fits the window at
        // full height (15-ish panels × 21 px each = ~315 px) and a
        // menu-style popup reads cleaner without a scrollbar.
        let (ww, wh) = ui.window_size;
        let visible_rows = crate::panels::ALL_PANELS
            .iter()
            .filter(|&&k| (!k.needs_schematic() || has_sch) && (!k.needs_pcb() || has_pcb))
            .count();
        // Anchor the popup directly above the "Panels" button in the
        // bottom-right of the status bar, its bottom edge just above the bar.
        let (popup, left, top) = if graphite {
            let popup =
                container(column(panel_items).spacing(0).width(
                    crate::status_bar::POPUP_WIDTH - 2.0 * crate::status_bar::POPUP_PADDING,
                ))
                .padding(crate::status_bar::POPUP_PADDING)
                .style(crate::status_bar::popup(tokens));
            let popup_w = crate::status_bar::POPUP_WIDTH;
            let popup_h = crate::status_bar::popup_height(visible_rows);
            let status_h = crate::status_bar::status_bar_height(tokens);
            (
                popup,
                (ww - popup_w - crate::status_bar::POPUP_RIGHT_MARGIN).max(0.0),
                (wh - popup_h - status_h - crate::status_bar::POPUP_GAP).max(0.0),
            )
        } else {
            let popup = container(column(panel_items).spacing(0).width(210))
                .padding([6, 0])
                .style(crate::styles::context_menu(tokens));
            // 210 px wide; 22 px per row plus 12 px of padding; 26 px
            // clears the status bar.
            let popup_h = visible_rows as f32 * 22.0 + 12.0;
            (
                popup,
                (ww - 210.0 - 10.0).max(0.0),
                (wh - popup_h - 26.0).max(0.0),
            )
        };
        vec![
            Self::dismiss_layer(Message::Overlay(OverlayMsg::TogglePanelList)),
            super::super::translate::Translate::new(popup.boxed(), (left, top)).boxed(),
        ]
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
            active_tab: crate::app::state::PdfPreviewTab::Preview,
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
