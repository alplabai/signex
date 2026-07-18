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

impl Signex {
    /// True when a modal that must own the entire overlay stack is up —
    /// export-error, print preview, or the custom net-colour picker.
    /// Mirrors the inline guard that early-returns from
    /// `collect_overlays` before any tool/menu overlay is pushed.
    pub(in crate::app::view) fn has_blocking_modal(&self) -> bool {
        self.document_state.export_error.is_some()
            || self.document_state.preview.is_some()
            || self.ui_state.net_color_custom.show
    }

    /// Export-error modal — appears when PDF / netlist / BOM export
    /// hits a user-actionable failure (write permission, invalid path,
    /// empty schematic). Dismiss via OK button or clicking outside.
    /// Pushes the dismiss backdrop then the error card.
    pub(in crate::app::view) fn export_error_overlay(&self) -> Vec<Element<'_, Message>> {
        if self.document_state.export_error.is_none() {
            return Vec::new();
        }
        vec![
            Self::dismiss_layer(Message::Export(ExportMsg::DismissError)),
            self.view_export_error(),
        ]
    }

    /// Print preview overlay — Altium parity: opens as a separate OS
    /// window (see `handle_print_preview_requested → handle_detach_modal`)
    /// so it can be dragged outside the app's client area. Only fall
    /// back to the in-window overlay if the OS window failed to open.
    pub(in crate::app::view) fn print_preview_overlay(&self) -> Option<Element<'_, Message>> {
        let preview_detached = self.ui_state.windows.values().any(|kind| {
            matches!(
                kind,
                crate::app::state::WindowKind::DetachedModal(
                    crate::app::state::ModalId::PrintPreview
                )
            )
        });
        if self.document_state.preview.is_some() && !preview_detached {
            Some(self.view_print_preview())
        } else {
            None
        }
    }

    /// BOM preview overlay — same detach-first pattern as Print Preview.
    pub(in crate::app::view) fn bom_preview_overlay(&self) -> Option<Element<'_, Message>> {
        let bom_detached = self.ui_state.windows.values().any(|kind| {
            matches!(
                kind,
                crate::app::state::WindowKind::DetachedModal(
                    crate::app::state::ModalId::BomPreview
                )
            )
        });
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
        if !self.ui_state.net_color_custom.show {
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
    pub(in crate::app::view) fn placement_paused_overlay(&self) -> Option<Element<'_, Message>> {
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let footprint_paused = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|tab| tab.kind.as_footprint_editor())
            .and_then(|path| self.document_state.footprint_editors.get(path))
            .map(|ed| ed.state.placement_paused)
            .unwrap_or(false);
        if !(interaction.canvas.placement_paused || footprint_paused) {
            return None;
        }
        let tokens = &document.panel_ctx.tokens;
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let text_c = crate::styles::ti(tokens.text);
        let accent_c = crate::styles::ti(tokens.accent);
        let border_c = crate::styles::ti(tokens.border);
        let card = container(
            column![
                iced::widget::text("⏸").size(64).color(accent_c),
                iced::widget::text("Placement Paused")
                    .size(16)
                    .color(text_c),
                iced::widget::text(
                    "Editing properties in the panel. Click Resume to keep placing."
                )
                .size(11)
                .color(text_c),
                iced::widget::Space::new().height(6.0),
                iced::widget::button(
                    iced::widget::text("Resume Placement")
                        .size(12)
                        .color(iced::Color::WHITE)
                )
                .padding([6, 18])
                .on_press(Message::Tool(ToolMessage::ResumePlacement))
                .style(iced::widget::button::primary),
            ]
            .spacing(8)
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
            },
            ..container::Style::default()
        });
        Some(
            container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
        )
    }

    /// Schematic Active Bar overlay — only painted on the main window,
    /// so the main canvas's selection set is the right gate.
    pub(in crate::app::view) fn schematic_active_bar_overlay(
        &self,
    ) -> Option<Element<'_, Message>> {
        if !self.has_active_schematic() {
            return None;
        }
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let y_offset: f32 =
            crate::menu_bar::MENU_BAR_HEIGHT + if document.tabs.is_empty() { 0.0 } else { 28.0 };
        let bar_has_selection = !interaction.canvas.selected.is_empty();
        let bar_has_net_colors = !ui.net_colors.is_empty();
        let bar = crate::active_bar::view_bar(
            interaction.current_tool,
            interaction.draw_mode,
            &interaction.last_tool,
            &document.panel_ctx.tokens,
            self.ui_state.theme_id,
            bar_has_selection,
            bar_has_net_colors,
        )
        .map(Message::ActiveBar);
        Some(
            column![
                iced::widget::Space::new().height(y_offset + 4.0),
                container(bar)
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center),
            ]
            .into(),
        )
    }

    /// v0.13 — footprint editor active bar (+ its dropdown overlay)
    /// mounted at the SAME app-view layer as the schematic's, so both
    /// share identical `Space::height(y_offset + 4.0)` math and land on
    /// a pixel-identical screen y.
    pub(in crate::app::view) fn footprint_active_bar_overlay(&self) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) else {
            return Vec::new();
        };
        let Some(path) = active_tab.kind.as_footprint_editor() else {
            return Vec::new();
        };
        let Some(editor) = self.document_state.footprint_editors.get(path) else {
            return Vec::new();
        };
        let mut out: Vec<Element<'_, Message>> = Vec::new();
        let y_offset: f32 =
            crate::menu_bar::MENU_BAR_HEIGHT + if document.tabs.is_empty() { 0.0 } else { 28.0 };
        let theme_id = self.ui_state.theme_id;
        let tokens = &document.panel_ctx.tokens;
        // Task 6 — footprint-native presets (`SelectionFilterKind`),
        // not the schematic `custom_filter_presets`.
        let footprint_presets = &interaction.footprint_filter_presets;
        let bar_items = crate::library::editor::footprint::unified_active_bar::bar_items(
            editor, theme_id, tokens,
        );
        let bar = signex_widgets::active_bar::view(bar_items, tokens).map(Message::Library);
        out.push(
            column![
                iced::widget::Space::new().height(y_offset + 4.0),
                container(bar)
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center),
            ]
            .into(),
        );
        // Position the dropdown panel directly below the bar's bottom
        // edge (see the schematic mount for the 42 px derivation).
        let dropdown_top: u16 = (y_offset as u16).saturating_add(42);
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
            out.push(overlay.map(Message::Library));
        }
        out
    }

    /// v0.26 — right-click context menu overlay for the footprint
    /// canvas. Sits above the active-bar dropdown so a long-press menu
    /// is occluded by — never under — its own dismiss layer. Pushes the
    /// dismiss layer then the clamped menu card.
    pub(in crate::app::view) fn footprint_context_menu_overlay(&self) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) else {
            return Vec::new();
        };
        let Some(path) = active_tab.kind.as_footprint_editor() else {
            return Vec::new();
        };
        let Some(editor) = self.document_state.footprint_editors.get(path) else {
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
        let card_msg = card.map(Message::Library);
        let (ww, wh) = self.ui_state.window_size;
        // Conservative footprint estimate so the card stays on screen
        // near right / bottom edges.
        let est_menu_w: f32 = 220.0;
        let est_menu_h: f32 = 320.0;
        let edge_margin: f32 = 4.0;
        let x = if menu_state.x + est_menu_w + edge_margin > ww {
            (ww - est_menu_w - edge_margin).max(0.0)
        } else {
            menu_state.x
        };
        let y = if menu_state.y + est_menu_h + edge_margin > wh {
            (menu_state.y - est_menu_h).max(0.0)
        } else {
            menu_state.y
        };
        vec![
            Self::dismiss_layer(close_msg),
            super::super::translate::Translate::new(card_msg, (x, y)).into(),
        ]
    }

    /// v0.14 — typed-delta "Move Selection By X, Y…" modal for the
    /// footprint editor. A blocking dialog once open; pushes its dismiss
    /// backdrop then the centered card.
    pub(in crate::app::view) fn footprint_move_by_overlay(&self) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) else {
            return Vec::new();
        };
        let Some(path) = active_tab.kind.as_footprint_editor() else {
            return Vec::new();
        };
        let Some(editor) = self.document_state.footprint_editors.get(path) else {
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
            container(card.map(Message::Library))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
        ]
    }

    /// v0.13 — symbol library editor active bar (+ its dropdown overlay)
    /// mounted at the SAME app-view layer as the schematic / footprint
    /// bars.
    pub(in crate::app::view) fn symbol_editor_active_bar_overlay(
        &self,
    ) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) else {
            return Vec::new();
        };
        let Some(path) = active_tab.kind.as_symbol_editor() else {
            return Vec::new();
        };
        let Some(editor) = self.document_state.symbol_editors.get(path) else {
            return Vec::new();
        };
        let mut out: Vec<Element<'_, Message>> = Vec::new();
        let y_offset: f32 =
            crate::menu_bar::MENU_BAR_HEIGHT + if document.tabs.is_empty() { 0.0 } else { 28.0 };
        let theme_id = self.ui_state.theme_id;
        let tokens = &document.panel_ctx.tokens;
        let bar_items = crate::library::editor::symbol::active_bar::bar_items(editor, theme_id);
        let bar = signex_widgets::active_bar::view(bar_items, tokens).map(Message::Library);
        out.push(
            column![
                iced::widget::Space::new().height(y_offset + 4.0),
                container(bar)
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center),
            ]
            .into(),
        );
        let dropdown_top: u16 = (y_offset as u16).saturating_add(42);
        if let Some(overlay) = crate::library::editor::symbol::active_bar::dropdown_overlay(
            editor,
            theme_id,
            tokens,
            dropdown_top,
        ) {
            out.push(overlay.map(Message::Library));
        }
        out
    }

    /// Right-click context menu overlay for the symbol canvas. Mirrors
    /// [`Self::footprint_context_menu_overlay`] 1:1 in structure — see
    /// that method for the coordinate / clamping rationale.
    pub(in crate::app::view) fn symbol_context_menu_overlay(&self) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) else {
            return Vec::new();
        };
        let Some(path) = active_tab.kind.as_symbol_editor() else {
            return Vec::new();
        };
        let Some(editor) = self.document_state.symbol_editors.get(path) else {
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
        let card_msg = card.map(Message::Library);
        let place_open = menu_state.open_submenu
            == Some(crate::library::editor::symbol::state::SymbolContextSubmenu::Place);
        let (x, y) = Self::clamp_symbol_menu_position(
            (menu_state.x, menu_state.y),
            self.ui_state.window_size,
            place_open,
        );
        vec![
            Self::dismiss_layer(close_msg),
            super::super::translate::Translate::new(card_msg, (x, y)).into(),
        ]
    }

    /// Clamp the symbol context menu's requested `(x, y)` so a
    /// conservative estimate of its footprint stays on screen near
    /// the right / bottom window edges (matches the footprint
    /// overlay's clamping, sized down for the shorter symbol menu).
    /// `place_submenu_open` accounts for the extra rows the Place ▸
    /// submenu adds in place (accordion, not a flyout) when expanded —
    /// otherwise the estimate under-shoots the expanded card's real
    /// footprint and it can run off the bottom edge.
    fn clamp_symbol_menu_position(
        requested: (f32, f32),
        window: (f32, f32),
        place_submenu_open: bool,
    ) -> (f32, f32) {
        let (mx, my) = requested;
        let (ww, wh) = window;
        let est_menu_w: f32 = crate::library::editor::symbol::context_menu::MENU_WIDTH;
        // Real card height = row count × one row's box + the panel's own
        // 4px top/bottom padding (`container(col).padding(4)`). One row is
        // a 13pt label in [5, 12] button padding, no icon at this level ≈
        // 28 px — see signex_widgets::active_bar::dropdown::view. The
        // collapsed menu is the stable 6-row top-level set (Place ▸, Join,
        // Delete, Select All, Deselect All, Fit — locked by
        // rows::tests::top_level_ids_are_stable); an expanded Place ▸ adds
        // its rows in place (accordion, not a flyout). A tight estimate
        // matters: the bottom-edge flip below lifts the card by exactly
        // est_menu_h, so an over-estimate leaves a visible gap between the
        // card's bottom and the cursor.
        const ROW_HEIGHT_PX: f32 = 28.0;
        const CARD_V_PADDING_PX: f32 = 8.0;
        const TOP_LEVEL_ROWS: f32 = 6.0;
        let expanded_rows = if place_submenu_open {
            crate::library::editor::symbol::context_menu::PLACE_TOOLS.len() as f32
        } else {
            0.0
        };
        let est_menu_h: f32 = (TOP_LEVEL_ROWS + expanded_rows) * ROW_HEIGHT_PX + CARD_V_PADDING_PX;
        let edge_margin: f32 = 4.0;
        let x = if mx + est_menu_w + edge_margin > ww {
            (ww - est_menu_w - edge_margin).max(0.0)
        } else {
            mx
        };
        let y = if my + est_menu_h + edge_margin > wh {
            (my - est_menu_h).max(0.0)
        } else {
            my
        };
        (x, y)
    }

    /// In-canvas text-edit input — the floating `text_input` anchored on
    /// top of the label being edited. Converts the object's world
    /// position through the live camera into a window-absolute screen
    /// position each frame.
    pub(in crate::app::view) fn text_edit_overlay(&self) -> Option<Element<'_, Message>> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        if !self.has_active_schematic() {
            return None;
        }
        let edit_state = interaction.editing_text.as_ref()?;
        let text = edit_state.text.clone();
        // Convert object world position → window-absolute screen position.
        // The canvas Program publishes its latest camera into this Cell each
        // frame — that's the only way to read it from outside the Program.
        let (cam_off_x, cam_off_y, cam_scale) = interaction.canvas.live_camera.get();
        let canvas_local_x = edit_state.world_x as f32 * cam_scale + cam_off_x;
        let canvas_local_y = edit_state.world_y as f32 * cam_scale + cam_off_y;
        // Canvas top-left within the window: menu bar + tab bar above,
        // left dock + left resize handle (5px when shown) to the side.
        let tabs_h: f32 = if document.tabs.is_empty() { 0.0 } else { 28.0 };
        let y_canvas_origin: f32 = crate::menu_bar::MENU_BAR_HEIGHT + tabs_h;
        let has_left = document.dock.has_panels(crate::dock::PanelPosition::Left);
        let left_col = document.dock.is_collapsed(crate::dock::PanelPosition::Left);
        let left_dock_w: f32 = if !has_left {
            0.0
        } else if left_col {
            28.0
        } else {
            ui.left_width
        };
        let left_handle_w: f32 = if has_left && !left_col { 5.0 } else { 0.0 };
        let x_canvas_origin: f32 = left_dock_w + left_handle_w;
        // Font size in pixels matches the rendered label (10 pt ≈ 1.8 mm).
        let font_px = (cam_scale * 1.8).clamp(10.0, 64.0);
        // Estimate width from text length to keep the input snug.
        let approx_w = ((edit_state.text.chars().count() as f32 + 2.0) * font_px * 0.62).max(60.0);
        // Offset the input so the baseline sits on top of the label text.
        let abs_x = x_canvas_origin + canvas_local_x - 2.0;
        let abs_y = y_canvas_origin + canvas_local_y - font_px - 2.0;
        let paper_c = crate::styles::ti(document.panel_ctx.tokens.paper);
        let text_c = crate::styles::ti(document.panel_ctx.tokens.text);
        let accent_c = crate::styles::ti(document.panel_ctx.tokens.accent);
        Some(
            column![
                iced::widget::Space::new().height(abs_y.max(0.0)),
                row![
                    iced::widget::Space::new().width(abs_x.max(0.0)),
                    container(
                        iced::widget::text_input("", &text)
                            .on_input(|t| Message::TextEdit(TextEditMsg::Changed(t)))
                            .on_submit(Message::TextEdit(TextEditMsg::Submit))
                            .size(font_px)
                            .padding([1, 2])
                            .width(approx_w)
                            .style(move |_: &iced::Theme, _status: iced::widget::text_input::Status| {
                                iced::widget::text_input::Style {
                                    background: iced::Background::Color(paper_c),
                                    border: iced::Border {
                                        color: accent_c,
                                        width: 1.0,
                                        radius: 0.0.into(),
                                    },
                                    icon: text_c,
                                    placeholder: text_c,
                                    value: text_c,
                                    selection: accent_c,
                                }
                            }),
                    ),
                ],
            ]
            .into(),
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
        let text_c = crate::styles::ti(document.panel_ctx.tokens.text);
        let text_muted = crate::styles::ti(document.panel_ctx.tokens.text_secondary);
        let has_sch = document.panel_ctx.has_schematic;
        let has_pcb = document.panel_ctx.has_pcb;
        // Build a lookup of currently-open panel kinds so each row
        // can show a ✓ mark. A panel counts as "open" if it lives in
        // any dock region, floats on top, or owns a detached OS
        // window.
        let docked: std::collections::HashSet<crate::panels::PanelKind> = [
            crate::dock::PanelPosition::Left,
            crate::dock::PanelPosition::Right,
            crate::dock::PanelPosition::Bottom,
        ]
        .iter()
        .flat_map(|pos| document.dock.panel_kinds(*pos).to_vec())
        .collect();
        let floating: std::collections::HashSet<crate::panels::PanelKind> =
            document.dock.floating.iter().map(|fp| fp.kind).collect();
        let detached: std::collections::HashSet<crate::panels::PanelKind> = ui
            .windows
            .values()
            .filter_map(|w| match w {
                crate::app::state::WindowKind::DetachedPanel(k) => Some(*k),
                _ => None,
            })
            .collect();
        let is_open = |k: crate::panels::PanelKind| {
            docked.contains(&k) || floating.contains(&k) || detached.contains(&k)
        };
        let panel_items: Vec<Element<'_, Message>> = crate::panels::ALL_PANELS
            .iter()
            .filter(|&&kind| (!kind.needs_schematic() || has_sch) && (!kind.needs_pcb() || has_pcb))
            .map(|&kind| {
                // Altium parity: a leading ✓ column marks open panels
                // so the user can see at a glance which ones are
                // already somewhere on screen. Clicking an open panel
                // still fires OpenPanel — the dock brings it forward.
                let check = if is_open(kind) { "\u{2713}" } else { "" };
                iced::widget::button(
                    iced::widget::row![
                        iced::widget::container(
                            iced::widget::text(check.to_string())
                                .size(11)
                                .color(text_muted),
                        )
                        .width(Length::Fixed(16.0)),
                        iced::widget::text(kind.label().to_string())
                            .size(11)
                            .color(text_c),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([4, 12])
                .width(Length::Fill)
                .on_press(Message::Overlay(OverlayMsg::OpenPanel(kind)))
                .style(crate::styles::menu_item(&document.panel_ctx.tokens))
                .into()
            })
            .collect();

        // Drop the scrollable wrapper — the list fits the window at
        // full height (15-ish panels × 21 px each = ~315 px) and a
        // menu-style popup reads cleaner without a scrollbar.
        let popup = container(column(panel_items).spacing(0).width(210))
            .padding([6, 0])
            .style(crate::styles::context_menu(&document.panel_ctx.tokens));

        // Anchor the popup directly above the "Panels" button in the
        // bottom-right of the status bar. Approx: popup 210 px wide,
        // 22 px per row × visible rows + 12 px vertical padding.
        // Status bar sits at y = wh - 22, so we place the popup so
        // its bottom edge lands just above it.
        let (ww, wh) = ui.window_size;
        let visible_rows = crate::panels::ALL_PANELS
            .iter()
            .filter(|&&k| (!k.needs_schematic() || has_sch) && (!k.needs_pcb() || has_pcb))
            .count() as f32;
        let popup_w = 210.0_f32;
        let popup_h = visible_rows * 22.0 + 12.0;
        let left = (ww - popup_w - 10.0).max(0.0);
        let top = (wh - popup_h - 26.0).max(0.0);
        vec![
            Self::dismiss_layer(Message::Overlay(OverlayMsg::TogglePanelList)),
            super::super::translate::Translate::new(Element::from(popup), (left, top)).into(),
        ]
    }
}
