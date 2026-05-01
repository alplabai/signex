use iced::widget::{column, container, row};
use iced::{Element, Length};

use super::*;

impl Signex {
    fn dismiss_layer(on_press: Message) -> Element<'static, Message> {
        // Opaque semi-transparent backdrop that blocks interaction with
        // underlying content. Left-click anywhere on it triggers the
        // dismiss message.
        //
        // We intentionally do *not* wire `on_right_press` — iced's
        // `mouse_area` would `capture_event()` the right-press and
        // prevent the underlying canvas from starting a pan. Instead
        // the canvas itself owns the right-press (its pan gesture) and
        // closes the context menu once the pan actually starts moving
        // (see `canvas/mod.rs`'s `CursorMoved` handler, which fires
        // `CloseContextMenu` the moment `pan_moved` flips on).
        const BACKDROP_OPACITY: f32 = 0.55;
        iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        0.0,
                        0.0,
                        0.0,
                        BACKDROP_OPACITY,
                    ))),
                    ..container::Style::default()
                }),
        )
        .on_press(on_press)
        .into()
    }

    pub(super) fn collect_overlays(&self) -> Vec<Element<'_, Message>> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let mut layers = Vec::new();

        // Export-error modal — appears when PDF / netlist / BOM export
        // hits a user-actionable failure (write permission, invalid path,
        // empty schematic). Dismiss via OK button or clicking outside.
        if document.export_error.is_some() {
            layers.push(Self::dismiss_layer(Message::DismissExportError));
            layers.push(self.view_export_error());
        }

        // Print preview overlay — Altium parity: opens as a separate OS
        // window (see `handle_print_preview_requested → handle_detach_modal`)
        // so it can be dragged outside the app's client area. Only fall
        // back to the in-window overlay if the OS window failed to open.
        let preview_detached = ui
            .windows
            .values()
            .any(|kind| matches!(kind, super::state::WindowKind::DetachedModal(super::state::ModalId::PrintPreview)));
        if document.preview.is_some() && !preview_detached {
            layers.push(self.view_print_preview());
        }

        // BOM preview overlay — same detach-first pattern as Print Preview.
        let bom_detached = ui
            .windows
            .values()
            .any(|kind| matches!(kind, super::state::WindowKind::DetachedModal(super::state::ModalId::BomPreview)));
        if document.bom_preview.is_some() && !bom_detached {
            layers.push(self.view_bom_preview());
        }

        // Custom net-colour picker. Bespoke modal (not the iced_aw
        // ColorPicker) because the user needs a quick-pick palette +
        // precise RGB inputs side-by-side.
        if ui.net_color_custom.show {
            layers.push(Self::dismiss_layer(Message::NetColorCustomShow(false)));
            layers.push(self.view_net_color_custom_picker());
        }

        // Blocking modals must own the overlay stack. If we keep adding
        // tool/menu overlays after these, they can end up visually above
        // the modal and make the dialog look broken.
        let has_blocking_modal = document.export_error.is_some()
            || document.preview.is_some()
            || ui.net_color_custom.show;
        if has_blocking_modal {
            return layers;
        }

        // Altium-style pause overlay: big centered "Placement Paused" card
        // with a Resume button. Clicking Resume clears `pre_placement`,
        // un-pauses the canvas, and drops back to the active placement tool
        // so the user can keep dropping objects with the edited properties.
        if interaction.canvas.placement_paused {
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
                    .on_press(Message::ResumePlacement)
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
            layers.push(
                container(card)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into(),
            );
        }

        if self.has_active_schematic() {
            let y_offset: f32 = crate::menu_bar::MENU_BAR_HEIGHT
                + if document.tabs.is_empty() { 0.0 } else { 28.0 };
            // Active Bar overlay is only painted on the main window, so
            // the main canvas's selection set is the right gate.
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
            layers.push(
                column![
                    iced::widget::Space::new().height(y_offset + 4.0),
                    container(bar)
                        .width(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Center),
                ]
                .into(),
            );
        }

        if self.has_active_schematic()
            && let Some(ref edit_state) = interaction.editing_text
        {
            let text = edit_state.text.clone();
            // Convert object world position → window-absolute screen position.
            // The canvas Program publishes its latest camera into this Cell each
            // frame — that's the only way to read it from outside the Program.
            let (cam_off_x, cam_off_y, cam_scale) = interaction.canvas.live_camera.get();
            // Anchor the editor at the AABB top-left of the rendered glyphs
            // (computed when edit started). This works for any
            // justify_h / justify_v combination, including Center / Bottom
            // notes — the old anchor-based offset landed up-and-to-the-side
            // of the visible text for those.
            // (Foreign-format imports are handled by a separate companion
            // crate; this code path never sees those structures.)
            let canvas_local_x = edit_state.world_min_x as f32 * cam_scale + cam_off_x;
            let canvas_local_y = edit_state.world_min_y as f32 * cam_scale + cam_off_y;
            // Canvas top-left within the window: menu bar + tab bar above,
            // left dock + left resize handle (5px when shown) to the side.
            let tabs_h: f32 = if document.tabs.is_empty() { 0.0 } else { 28.0 };
            let y_canvas_origin: f32 = crate::menu_bar::MENU_BAR_HEIGHT + tabs_h;
            let has_left = document.dock.has_panels(PanelPosition::Left);
            let left_col = document.dock.is_collapsed(PanelPosition::Left);
            let left_dock_w: f32 = if !has_left {
                0.0
            } else if left_col {
                28.0
            } else {
                ui.left_width
            };
            let left_handle_w: f32 = if has_left && !left_col { 5.0 } else { 0.0 };
            let x_canvas_origin: f32 = left_dock_w + left_handle_w;
            // Canvas right / bottom edges so the inline editor can be
            // clipped against them — without this clamp the floating
            // text_input bleeds over the side / bottom dock panels and
            // status bar (overlays sit above docked widgets in z-order).
            let has_right = document.dock.has_panels(PanelPosition::Right);
            let right_col = document.dock.is_collapsed(PanelPosition::Right);
            let right_dock_w: f32 = if !has_right {
                0.0
            } else if right_col {
                28.0
            } else {
                ui.right_width
            };
            let right_handle_w: f32 = if has_right && !right_col { 5.0 } else { 0.0 };
            let has_bottom = document.dock.has_panels(PanelPosition::Bottom);
            let bottom_col = document.dock.is_collapsed(PanelPosition::Bottom);
            let bottom_dock_h: f32 = if !has_bottom {
                0.0
            } else if bottom_col {
                28.0
            } else {
                ui.bottom_height
            };
            let bottom_handle_h: f32 = if has_bottom && !bottom_col { 5.0 } else { 0.0 };
            let (win_w, win_h) = ui.window_size;
            let status_bar_h: f32 = 22.0;
            let x_canvas_right: f32 = (win_w - right_dock_w - right_handle_w).max(x_canvas_origin);
            let y_canvas_bottom: f32 =
                (win_h - bottom_dock_h - bottom_handle_h - status_bar_h).max(y_canvas_origin);
            // Match the renderer exactly: text notes draw at
            // SCHEMATIC_TEXT_EM_MM em height, scaled by the camera. No
            // clamp — the inline editor must agree with the canvas
            // glyphs at every zoom level so the cursor sits on the
            // baseline of the visible text.
            let font_px = (cam_scale * edit_state.world_height as f32).max(1.0);
            // Width = AABB width in screen pixels, with a tiny pad so the
            // caret has room when the field is empty.
            let aabb_w_px =
                (edit_state.world_height as f32).max(1.0) * 0.0; // placeholder, real value below
            let _ = aabb_w_px;
            let approx_w = ((edit_state.text.chars().count() as f32 + 2.0)
                * font_px
                * 0.5)
                .max(40.0);
            // No vertical fudge: AABB top-left already accounts for em-box
            // leading, so the input top sits exactly on the canvas glyph
            // top.
            let abs_x = x_canvas_origin + canvas_local_x;
            // With `line_height(Relative(1.0))` the text_input's line-box
            // is exactly `font_px` tall and its glyphs sit at the top of
            // that box, matching `canvas::Text { align_y: Top }` used by
            // the renderer. No vertical fudge needed.
            let abs_y = y_canvas_origin + canvas_local_y;
            // Clamp the editor to the canvas region. If the text-note
            // anchor sits to the left of (or above) the canvas, push the
            // overlay back inside and trim the available width / height
            // by the same amount so it doesn't overpaint the side docks
            // or status bar. Same on the right / bottom: never extend
            // past the canvas edge.
            let abs_x_clamped = abs_x.max(x_canvas_origin);
            let abs_y_clamped = abs_y.max(y_canvas_origin);
            let left_trim = (abs_x_clamped - abs_x).max(0.0);
            let top_trim = (abs_y_clamped - abs_y).max(0.0);
            let max_w = (x_canvas_right - abs_x_clamped).max(0.0);
            let max_h = (y_canvas_bottom - abs_y_clamped).max(0.0);
            let editor_visible = max_w > 1.0 && max_h > 1.0;
            let approx_w = (approx_w - left_trim).clamp(1.0, max_w.max(1.0));
            let _ = top_trim;
            // Match the rendered text-note color so the editor reads as
            // the same glyph, not as a panel-coloured input on top of it.
            let body_c = signex_render::colors::to_iced(
                &interaction.canvas.canvas_colors.body,
            );
            let accent_c = crate::styles::ti(document.panel_ctx.tokens.accent);
            let transparent = iced::Color::TRANSPARENT;
            if editor_visible {
            layers.push(
                column![
                    iced::widget::Space::new().height(abs_y_clamped.max(0.0)),
                    row![
                        iced::widget::Space::new().width(abs_x_clamped.max(0.0)),
                        container(
                            iced::widget::text_input("", &text)
                                .on_input(Message::TextEditChanged)
                                .on_submit(Message::TextEditSubmit)
                                .size(font_px)
                                .line_height(iced::widget::text::LineHeight::Relative(1.0))
                                .padding(0)
                                .width(approx_w)
                                .font(signex_render::canvas_font())
                                .style(move |_: &iced::Theme, _status: iced::widget::text_input::Status| {
                                    iced::widget::text_input::Style {
                                        background: iced::Background::Color(transparent),
                                        border: iced::Border {
                                            color: transparent,
                                            width: 0.0,
                                            radius: 0.0.into(),
                                        },
                                        icon: body_c,
                                        placeholder: body_c,
                                        value: body_c,
                                        selection: accent_c,
                                    }
                                }),
                        )
                        .max_width(max_w)
                        .max_height(max_h),
                    ],
                ]
                .into(),
            );
            }
        }

        if let Some(ab_menu) = interaction.active_bar_menu {
            let has_selection = !interaction.canvas.selected.is_empty();
            let has_net_colors = !ui.net_colors.is_empty();
            let dropdown = crate::active_bar::view_dropdown(
                ab_menu,
                &document.panel_ctx.tokens,
                &interaction.selection_filters,
                &interaction.custom_filter_presets,
                self.ui_state.theme_id,
                has_selection,
                has_net_colors,
            )
            .map(Message::ActiveBar);
            let x_off = crate::active_bar::dropdown_x_offset(ab_menu);
            // Bar: MENU_BAR_HEIGHT + tabs + 4 top-margin + bar-height ≈ bottom of bar.
            // Bar-height = 28 button + 6 vertical padding + 2 border = 36, plus 4
            // top margin = 40. Add a small gap so the dropdown visually touches.
            let ab_y: f32 = crate::menu_bar::MENU_BAR_HEIGHT
                + if document.tabs.is_empty() { 0.0 } else { 28.0 }
                + 40.0;
            let bar_w: f32 = crate::active_bar::BAR_WIDTH_PX;
            let (ww, _) = ui.window_size;
            let adjusted_x = x_off + (ww - bar_w) / 2.0;

            layers.push(Self::dismiss_layer(Message::ActiveBar(
                crate::active_bar::ActiveBarMsg::CloseMenus,
            )));
            // Absolute-position the dropdown with Translate so the
            // column can auto-size to its widest label. The old
            // column+row+Space wrapping forced a fixed-width column
            // which clipped labels like "Elliptical Arc".
            layers
                .push(super::view::translate::Translate::new(dropdown, (adjusted_x, ab_y)).into());
        }

        if let Some(ref ctx_menu) = interaction.context_menu {
            let menu = self.view_context_menu();
            // Clamp the menu inside the window so a click near the
            // right/bottom edge doesn't push it off-screen. Estimate
            // the menu's footprint conservatively from the maximum
            // possible row count (≈ 22 rows × 22 px + padding) and
            // CONTEXT_MENU_WIDTH; flip-up / flip-left when the click
            // lands too close to an edge.
            let (win_w, win_h) = self.ui_state.window_size;
            let menu_w = Self::CONTEXT_MENU_WIDTH as f32;
            let est_menu_h: f32 = 22.0 * 22.0 + 8.0;
            let edge_margin: f32 = 4.0;
            let x = if ctx_menu.x + menu_w + edge_margin > win_w {
                (win_w - menu_w - edge_margin).max(0.0)
            } else {
                ctx_menu.x
            };
            let y = if ctx_menu.y + est_menu_h + edge_margin > win_h {
                (ctx_menu.y - est_menu_h).max(0.0)
            } else {
                ctx_menu.y
            };
            layers.push(Self::dismiss_layer(Message::CloseContextMenu));
            layers.push(
                column![
                    iced::widget::Space::new().height(y),
                    row![
                        iced::widget::Space::new().width(x),
                        menu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .into(),
            );
            // Submenu (Place / Align) — pop to the right of the parent
            // menu (or left if the right edge would overflow), and
            // align its top to the launcher row's y-position so the
            // first submenu item sits next to the row that opened it.
            if let Some(submenu_kind) = interaction.context_submenu {
                let submenu = self.view_context_submenu(submenu_kind);
                // Wrap in mouse_area so on_enter/on_exit on the panel
                // can extend the close timer when the cursor crosses
                // from the launcher into the submenu and back.
                let submenu = iced::widget::mouse_area(submenu)
                    .on_enter(Message::EnterContextSubmenuPanel)
                    .on_exit(Message::LeaveContextSubmenuPanel);
                let submenu_w = menu_w;
                let sub_x = if x + menu_w + submenu_w + edge_margin > win_w {
                    (x - submenu_w).max(0.0)
                } else {
                    x + menu_w
                };
                // Approximate launcher-row y inside the parent menu.
                // Each ctx_menu_item_* row is ≈ 22 px tall (text + 4 px
                // top + 4 px bottom + a tiny line-height fudge); the
                // separator is rendered as a 1 px line. The numbers
                // below come from counting rows above each launcher in
                // `view_context_menu`.
                const ROW_H: f32 = 22.0;
                const SEP_H: f32 = 1.0;
                const TOP_PAD: f32 = 4.0;
                let launcher_y = match submenu_kind {
                    // Above Place: 3 always-visible rows + 1 separator.
                    ContextSubmenu::Place => TOP_PAD + 3.0 * ROW_H + SEP_H,
                    // Align is only shown when something is selected;
                    // above Align: the same 3 rows + 1 sep, then
                    // Place / Part Actions / Sheet Actions / References.
                    ContextSubmenu::Align => TOP_PAD + 7.0 * ROW_H + SEP_H,
                    // AddNewToProject only fires from the project-tree
                    // menu, never from the canvas menu — fall through
                    // to a safe placeholder if the state somehow leaks
                    // (no submenu rendered, just a 0-offset).
                    ContextSubmenu::AddNewToProject => 0.0,
                };
                let sub_y = (y + launcher_y - 4.0).max(0.0);
                layers.push(
                    column![
                        iced::widget::Space::new().height(sub_y),
                        row![
                            iced::widget::Space::new().width(sub_x),
                            submenu,
                            iced::widget::Space::new().width(Length::Fill),
                        ]
                        .width(Length::Fill),
                    ]
                    .into(),
                );
            }
        }

        // Document-tab right-click menu. Rendered before the project-
        // tree menu since the two are mutually exclusive — only one of
        // them can be open at a time, and opening one closes the
        // others (see Message::ShowTabContextMenu).
        if let Some(ref tab_ctx) = interaction.tab_context_menu {
            let menu = self.view_tab_context_menu(tab_ctx);
            // Conservative footprint matches the project-tree menu so
            // the two visually align.
            let menu_w = Self::CONTEXT_MENU_WIDTH as f32;
            let est_menu_h: f32 = 5.0 * 22.0 + 8.0;
            let (win_w, win_h) = ui.window_size;
            let edge_margin: f32 = 4.0;
            let x = if tab_ctx.x + menu_w + edge_margin > win_w {
                (win_w - menu_w - edge_margin).max(0.0)
            } else {
                tab_ctx.x
            };
            let y = if tab_ctx.y + est_menu_h + edge_margin > win_h {
                (tab_ctx.y - est_menu_h).max(0.0)
            } else {
                tab_ctx.y
            };
            layers.push(Self::dismiss_layer(Message::CloseTabContextMenu));
            layers.push(
                column![
                    iced::widget::Space::new().height(y),
                    row![
                        iced::widget::Space::new().width(x),
                        menu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .into(),
            );
        }

        // Projects-panel tree right-click menu. Rendered here (after the
        // canvas context menu) so the canvas menu's dismiss layer does
        // not cover this one — the two are mutually exclusive in
        // practice since `ShowProjectTreeContextMenu` nulls out
        // `context_menu` before opening.
        if let Some(ref tree_ctx) = interaction.project_tree_context_menu {
            let menu = self.view_project_tree_context_menu(tree_ctx);
            // Conservative footprint: at most 6 rows × 22 px + 8 px
            // padding. Width matches the canvas menu so the two look
            // consistent.
            let menu_w = Self::CONTEXT_MENU_WIDTH as f32;
            let est_menu_h: f32 = 6.0 * 22.0 + 8.0;
            let (win_w, win_h) = ui.window_size;
            let edge_margin: f32 = 4.0;
            let x = if tree_ctx.x + menu_w + edge_margin > win_w {
                (win_w - menu_w - edge_margin).max(0.0)
            } else {
                tree_ctx.x
            };
            let y = if tree_ctx.y + est_menu_h + edge_margin > win_h {
                (tree_ctx.y - est_menu_h).max(0.0)
            } else {
                tree_ctx.y
            };
            layers.push(Self::dismiss_layer(Message::CloseProjectTreeContextMenu));
            layers.push(
                column![
                    iced::widget::Space::new().height(y),
                    row![
                        iced::widget::Space::new().width(x),
                        menu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .into(),
            );
            // Adjacent submenu (currently only AddNewToProject opens
            // from this menu). Mirrors the canvas-menu submenu logic
            // above — pop to the right of the parent (or left if the
            // right edge would overflow), align top to the launcher
            // row's y inside the parent menu.
            if let Some(ContextSubmenu::AddNewToProject) = interaction.context_submenu {
                let submenu = self.view_context_submenu(ContextSubmenu::AddNewToProject);
                let submenu = iced::widget::mouse_area(submenu)
                    .on_enter(Message::EnterContextSubmenuPanel)
                    .on_exit(Message::LeaveContextSubmenuPanel);
                let submenu_w = menu_w;
                let sub_x = if x + menu_w + submenu_w + edge_margin > win_w {
                    (x - submenu_w).max(0.0)
                } else {
                    x + menu_w
                };
                // Launcher position inside the project-tree menu:
                // `Make Project Available Online...` (row 0)
                // `Validate Project`                 (row 1)
                // `Add New to Project ›`             (row 2) ← target
                // → top + 2 rows, no separator above the launcher.
                const ROW_H: f32 = 22.0;
                const TOP_PAD: f32 = 4.0;
                let launcher_y = TOP_PAD + 2.0 * ROW_H;
                let sub_y = (y + launcher_y - 4.0).max(0.0);
                layers.push(
                    column![
                        iced::widget::Space::new().height(sub_y),
                        row![
                            iced::widget::Space::new().width(sub_x),
                            submenu,
                            iced::widget::Space::new().width(Length::Fill),
                        ]
                        .width(Length::Fill),
                    ]
                    .into(),
                );
            }
        }

        if ui.panel_list_open {
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
                    super::state::WindowKind::DetachedPanel(k) => Some(*k),
                    _ => None,
                })
                .collect();
            let is_open = |k: crate::panels::PanelKind| {
                docked.contains(&k) || floating.contains(&k) || detached.contains(&k)
            };
            let panel_items: Vec<Element<'_, Message>> = crate::panels::ALL_PANELS
                .iter()
                .filter(|&&kind| {
                    (!kind.needs_schematic() || has_sch) && (!kind.needs_pcb() || has_pcb)
                })
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
                    .on_press(Message::OpenPanel(kind))
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

            layers.push(Self::dismiss_layer(Message::TogglePanelList));
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
            layers.push(translate::Translate::new(Element::from(popup), (left, top)).into());
        }

        if let Some(fp) = document.dock.floating.iter().find(|fp| fp.dragging) {
            let (ww, wh) = ui.window_size;
            let zone = 120.0;
            let cx = fp.x + fp.width / 2.0;
            let cy = fp.y + fp.height / 4.0;
            let zone_style = crate::styles::dock_zone_highlight(&document.panel_ctx.tokens);
            if cx < zone {
                layers.push(
                    container(iced::widget::Space::new())
                        .width(ui.left_width)
                        .height(Length::Fill)
                        .style(zone_style)
                        .into(),
                );
            } else if cx > ww - zone {
                layers.push(
                    row![
                        iced::widget::Space::new().width(Length::Fill),
                        container(iced::widget::Space::new())
                            .width(ui.right_width)
                            .height(Length::Fill)
                            .style(zone_style),
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                );
            } else if cy > wh - zone {
                layers.push(
                    column![
                        iced::widget::Space::new().height(Length::Fill),
                        container(iced::widget::Space::new())
                            .width(Length::Fill)
                            .height(ui.bottom_height)
                            .style(zone_style),
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                );
            }
        }

        for i in 0..document.dock.floating.len() {
            if let Some(panel_widget) = document.dock.view_floating_panel(i, &document.panel_ctx) {
                let fp = &document.dock.floating[i];
                // No clamp — panels follow Altium behaviour and may be
                // dragged anywhere, even past the window edge. The OS clips
                // at the window boundary; within that, Translate renders
                // the panel at fp.(x, y) without resizing it.
                layers.push(
                    translate::Translate::new(panel_widget.map(Message::Dock), (fp.x, fp.y)).into(),
                );
            }
        }

        if ui.preferences_open {
            let pref_view = crate::preferences::view(
                ui.preferences_nav,
                ui.preferences_draft_theme,
                ui.theme_id,
                &ui.preferences_draft_font,
                ui.preferences_draft_power_port_style,
                ui.preferences_draft_label_style,
                ui.preferences_draft_multisheet_style,
                ui.preferences_draft_grid_style,
                ui.custom_theme.as_ref().map(|c| c.name.as_str()),
                ui.preferences_dirty,
                &ui.erc_severity_override,
            )
            .map(Message::PreferencesMsg);
            layers.push(pref_view);
        }

        if ui.find_replace.open {
            let dialog = crate::find_replace::view(&ui.find_replace, &document.panel_ctx.tokens)
                .map(Message::FindReplaceMsg);
            layers.push(dialog);
        }

        if ui.rename_dialog.is_some() {
            layers.push(self.view_rename_dialog());
        }
        if ui.remove_dialog.is_some() {
            layers.push(self.view_remove_dialog());
        }
        if ui.project_close_confirm.is_some() {
            layers.push(self.view_project_close_confirm());
        }

        // Skip overlay rendering for any modal whose detached OS window
        // owns the view. Without this guard the user sees the modal in
        // both the main window and the popped-out window at the same
        // time.
        let modal_detached = |m: super::state::ModalId| -> bool {
            ui.windows
                .values()
                .any(|kind| matches!(kind, super::state::WindowKind::DetachedModal(x) if *x == m))
        };

        if ui.annotate_dialog_open && !modal_detached(super::state::ModalId::AnnotateDialog) {
            layers.push(self.view_annotate_dialog());
        }
        if ui.annotate_reset_confirm && !modal_detached(super::state::ModalId::AnnotateResetConfirm)
        {
            layers.push(self.view_annotate_reset_confirm());
        }
        if ui.erc_dialog_open && !modal_detached(super::state::ModalId::ErcDialog) {
            layers.push(self.view_erc_dialog());
        }

        layers
    }
}
