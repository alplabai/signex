//! Altium-style modal dialogs for Design menu commands.
//!
//! Three modals live here:
//! - Annotate Schematics — order pick-list + preview of proposed changes
//! - ERC — per-rule severity override grid + pin-connection matrix
//! - Reset-Annotations confirm — simple Yes/No

use iced::widget::{Column, Row, Space, button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::app::states::AnnotateOrder;
use crate::app::{Message, Signex};

const BACKDROP: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.55);

// ── Modal chrome — single source of truth ───────────────────────────
//
// Every modal in the app (Annotate, ERC, Reset Confirm, Rename, Remove,
// Close-Tab Confirm, Print Preview) reaches for these constants so the
// header height, title font, and close-X footprint stay locked in
// step with the main-window chrome strip (`view::view_main_window_chrome`).
// Tweak here, every modal updates.

/// Modal header total height — Altium-style compact (28 px). The
/// MENU_BAR_HEIGHT (36 px) read as too chunky on small confirm /
/// rename modals; 28 keeps the header tight relative to the body.
/// Close-X follows the same height so there's no empty strip below
/// the button.
pub(in crate::app::view) const MODAL_HEADER_HEIGHT: f32 = 28.0;
/// Asymmetric padding inside the modal header strip: zero on the right
/// so the close-X sits flush against the rounded corner (its own
/// top-right radius matches `MODAL_CORNER_RADIUS`); zero top/bottom so
/// the X fills the strip's full height; left inset matched to the
/// modal body padding (16 px) so the title left-aligns with the
/// body's first text column.
pub(in crate::app::view) const MODAL_HEADER_PADDING: iced::Padding = iced::Padding {
    top: 0.0,
    right: 0.0,
    bottom: 0.0,
    left: 16.0,
};
/// Title text size in the modal header.
pub(in crate::app::view) const MODAL_HEADER_TITLE_SIZE: f32 = 13.0;
/// Close-X hit-box width — same width the chrome close uses
/// (`view::view_main_window_chrome::chrome_btn`).
pub(in crate::app::view) const MODAL_CLOSE_X_HIT_W: f32 = 46.0;
/// Close-X hit-box height — also matches the chrome close (full
/// menu-bar height) so the modal X is pixel-identical to the OS-window X.
pub(in crate::app::view) const MODAL_CLOSE_X_HIT_H: f32 = MODAL_HEADER_HEIGHT;
/// SVG glyph size for the close-X. Same value the chrome close uses.
pub(in crate::app::view) const MODAL_CLOSE_X_ICON: f32 = 14.0;
/// Hover background for the close-X (Windows-native destructive red).
pub(in crate::app::view) const MODAL_CLOSE_X_HOVER: Color =
    Color::from_rgba(0.78, 0.22, 0.22, 1.0);

impl Signex {
    pub(super) fn view_annotate_dialog(&self) -> Element<'_, Message> {
        let modal_w = 1100.0_f32;
        let modal_h = 760.0_f32;
        let dialog = self.view_annotate_dialog_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::states::ModalId::AnnotateDialog)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(
            dialog,
            offset,
            self.ui_state.window_size,
            (modal_w, modal_h),
        )
    }

    /// Detached-window flavour — just the body, no backdrop, no drag
    /// handler on the header (the OS window chrome owns that).
    pub(super) fn view_annotate_dialog_body(&self) -> Element<'_, Message> {
        self.view_annotate_dialog_body_inner(false)
    }

    fn view_annotate_dialog_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        // Compute preview: walk *every* open sheet, share one counter so
        // the proposed designators line up with what the engine will do
        // across the project.
        let proposed = self.preview_project_annotations();
        let total_symbols: usize = proposed.len();
        let current_sheet_name = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|t| t.title.clone())
            .unwrap_or_else(|| "Current Sheet".to_string());

        // ── Header (draggable when in-window; OS-window-drag when
        // detached) ──
        let theme_id = self.ui_state.theme_id;
        let header_content: Element<'_, Message> = container(
            row![
                text("Annotate")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::CloseAnnotateDialog, theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let _ = border_c;
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::states::ModalId::AnnotateDialog,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            detached_header(header_content, super::super::states::ModalId::AnnotateDialog)
        };

        // ── Left column: Schematic Annotation Configuration ──
        let left_title = text("Schematic Annotation Configuration")
            .size(12)
            .color(text_c);

        let order_row = column![
            text("Order of Processing").size(10).color(text_muted),
            row![
                column![
                    row![
                        order_radio(
                            "Up Then Across",
                            AnnotateOrder::UpThenAcross,
                            self.ui_state.annotate.order,
                            text_c,
                            border_c,
                        ),
                        Space::new().width(4),
                        order_radio(
                            "Across Then Down",
                            AnnotateOrder::AcrossThenDown,
                            self.ui_state.annotate.order,
                            text_c,
                            border_c,
                        ),
                    ]
                    .spacing(0),
                    Space::new().height(4),
                    row![
                        order_radio(
                            "Down Then Across",
                            AnnotateOrder::DownThenAcross,
                            self.ui_state.annotate.order,
                            text_c,
                            border_c,
                        ),
                        Space::new().width(4),
                        order_radio(
                            "Across Then Up",
                            AnnotateOrder::AcrossThenUp,
                            self.ui_state.annotate.order,
                            text_c,
                            border_c,
                        ),
                    ]
                    .spacing(0),
                ]
                .spacing(0)
                .width(Length::FillPortion(3)),
                Space::new().width(8),
                order_preview(self.ui_state.annotate.order, text_c, text_muted, border_c),
            ]
            .align_y(iced::Alignment::Start)
            .spacing(0),
        ]
        .spacing(4);

        let process_location = column![
            text("Process Location of").size(10).color(text_muted),
            container(text("Designator").size(11).color(text_c),)
                .padding([5, 10])
                .width(Length::Fill)
                .style(bordered_style(border_c)),
        ]
        .spacing(4);

        let replace_sub = column![
            text("Replace Sub-Parts").size(10).color(text_muted),
            container(text("Off").size(11).color(text_muted))
                .padding([5, 10])
                .width(Length::Fill)
                .style(bordered_style(border_c)),
        ]
        .spacing(4);

        // Component-parameter matching list. Altium shows every parameter
        // name that appears on any symbol in the project, with a "Strictly"
        // toggle per row. We pull the set from the active snapshot's
        // Symbol.fields HashMap so what the user sees matches what their
        // schematic actually carries — no hard-coded IntLib catalogue.
        let param_names: Vec<String> = {
            let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            // Always include the three built-in fields so the list isn't
            // empty on a fresh schematic.
            set.insert("Reference".to_string());
            set.insert("Value".to_string());
            set.insert("Footprint".to_string());
            if let Some(snapshot) = self.active_render_snapshot() {
                for sym in &snapshot.symbols {
                    for key in sym.fields.keys() {
                        set.insert(key.clone());
                    }
                }
            }
            set.into_iter().collect()
        };
        let mut param_rows: iced::widget::Column<'_, Message> = column![
            row![
                text("Component Parameter")
                    .size(10)
                    .color(text_muted)
                    .width(Length::FillPortion(5)),
                text("Strictly")
                    .size(10)
                    .color(text_muted)
                    .width(Length::FillPortion(1)),
            ]
            .padding([4, 8]),
        ]
        .spacing(0);
        for name in &param_names {
            param_rows = param_rows.push(
                row![
                    row![
                        check_pip(false, border_c),
                        Space::new().width(6),
                        text(name.clone()).size(11).color(text_c),
                    ]
                    .align_y(iced::Alignment::Center)
                    .width(Length::FillPortion(5)),
                    container(check_pip(true, border_c))
                        .align_x(iced::alignment::Horizontal::Right)
                        .width(Length::FillPortion(1)),
                ]
                .padding([3, 8])
                .align_y(iced::Alignment::Center),
            );
        }
        if param_names.is_empty() {
            param_rows = param_rows.push(
                container(
                    text("No custom parameters on any symbol yet.")
                        .size(11)
                        .color(text_muted),
                )
                .padding([10, 8]),
            );
        }

        let matching = column![
            text("Matching Options").size(12).color(text_c),
            Space::new().height(4),
            row![
                text("Complete Existing Packages")
                    .size(10)
                    .color(text_muted)
                    .width(Length::FillPortion(2)),
                container(text("None").size(11).color(text_muted))
                    .padding([4, 8])
                    .width(Length::FillPortion(3))
                    .style(bordered_style(border_c)),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(6),
            Space::new().height(6),
            container(scrollable(param_rows).height(280))
                .padding(2)
                .style(bordered_style(border_c)),
            Space::new().height(4),
            text("Parameter matching is visual grouping only — the annotation engine ignores these checkboxes for now.")
                .size(10)
                .color(text_muted),
        ]
        .spacing(4);

        // Sheets list: include every open tab so the multi-sheet view is
        // visible. All but the active sheet render greyed because only the
        // active snapshot feeds the preview for v0.7.
        let mut sheet_rows: iced::widget::Column<'_, Message> = column![].spacing(0);
        let active_title = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|t| t.title.clone());
        for (idx, tab) in self.document_state.tabs.iter().enumerate() {
            let is_active = Some(&tab.title) == active_title.as_ref();
            let row_color = if is_active { text_c } else { text_muted };
            let suffix_placeholder = if is_active { "" } else { "—" };
            sheet_rows = sheet_rows.push(
                row![
                    row![
                        check_pip(is_active, border_c),
                        Space::new().width(6),
                        text(tab.title.clone()).size(11).color(row_color),
                    ]
                    .align_y(iced::Alignment::Center)
                    .width(Length::FillPortion(6)),
                    text("All")
                        .size(11)
                        .color(row_color)
                        .width(Length::FillPortion(1)),
                    text(format!("{idx}"))
                        .size(11)
                        .color(row_color)
                        .width(Length::FillPortion(1)),
                    text(format!("{}", idx + 1))
                        .size(11)
                        .color(row_color)
                        .width(Length::FillPortion(1)),
                    row![
                        check_pip(false, border_c),
                        Space::new().width(6),
                        text(suffix_placeholder.to_string())
                            .size(11)
                            .color(row_color),
                    ]
                    .align_y(iced::Alignment::Center)
                    .width(Length::FillPortion(2)),
                ]
                .padding([3, 8])
                .align_y(iced::Alignment::Center),
            );
        }
        if self.document_state.tabs.is_empty() {
            sheet_rows = sheet_rows.push(
                container(text("No schematic tab open.").size(11).color(text_muted))
                    .padding([6, 8]),
            );
        }

        let sheets_table = column![
            text("Schematic Sheets To Annotate").size(12).color(text_c),
            Space::new().height(4),
            row![
                text("Schematic Sheet")
                    .size(10)
                    .color(text_muted)
                    .width(Length::FillPortion(6)),
                text("Scope")
                    .size(10)
                    .color(text_muted)
                    .width(Length::FillPortion(1)),
                text("Ord...")
                    .size(10)
                    .color(text_muted)
                    .width(Length::FillPortion(1)),
                text("Start")
                    .size(10)
                    .color(text_muted)
                    .width(Length::FillPortion(1)),
                text("Add Suffix")
                    .size(10)
                    .color(text_muted)
                    .width(Length::FillPortion(2)),
            ]
            .padding([2, 8]),
            container(scrollable(sheet_rows).height(180))
                .style(bordered_style(border_c)),
            Space::new().height(4),
            text("Scope / Start / Suffix columns are visual for v0.7 — multi-sheet wiring lands in v1.1.")
                .size(10)
                .color(text_muted),
        ]
        .spacing(4);

        let left_col = column![
            left_title,
            Space::new().height(6),
            order_row,
            Space::new().height(8),
            process_location,
            Space::new().height(8),
            replace_sub,
            Space::new().height(10),
            matching,
            Space::new().height(10),
            sheets_table,
        ]
        .spacing(0)
        .width(Length::FillPortion(10));

        // ── Right column: Proposed Change List ──
        let right_title = text("Proposed Change List").size(12).color(text_c);

        // Column headers
        let list_headers = row![
            text("Lock")
                .size(10)
                .color(text_muted)
                .width(Length::Fixed(42.0)),
            text("Current")
                .size(10)
                .color(text_muted)
                .width(Length::FillPortion(2)),
            text("Proposed")
                .size(10)
                .color(text_muted)
                .width(Length::FillPortion(2)),
            text("Location")
                .size(10)
                .color(text_muted)
                .width(Length::FillPortion(3)),
        ]
        .padding([4, 8]);

        let locked_set = &self.ui_state.annotate.locked;
        let mut rows_col: iced::widget::Column<'_, Message> =
            column![].spacing(0).width(Length::Fill);
        if proposed.is_empty() {
            rows_col = rows_col.push(
                container(
                    text("No symbols need annotation — every reference has a number.")
                        .size(11)
                        .color(text_muted),
                )
                .padding([10, 12]),
            );
        } else {
            for entry in &proposed {
                let is_locked = locked_set.contains(&entry.uuid);
                // Rows that actually change highlight in the accent color;
                // unchanged rows fade into the muted palette. Locked rows
                // force the muted palette regardless of proposed change.
                let changing = entry.current != entry.proposed && !is_locked;
                let cur_color = if changing { text_c } else { text_muted };
                let new_color = if is_locked {
                    text_muted
                } else if changing {
                    Color::from_rgb(0.25, 0.75, 0.35)
                } else {
                    text_muted
                };
                let proposed_display = if is_locked {
                    entry.current.clone()
                } else {
                    entry.proposed.clone()
                };
                let lock_tick = if is_locked { "\u{2713}" } else { "" };
                let uuid = entry.uuid;
                let lock_btn: Element<'_, Message> = button(
                    container(text(lock_tick.to_string()).size(11).color(text_c))
                        .width(22)
                        .height(16)
                        .align_x(iced::alignment::Horizontal::Center)
                        .align_y(iced::alignment::Vertical::Center),
                )
                .on_press(Message::AnnotateToggleLock(uuid))
                .padding(0)
                .style(move |_: &Theme, _| button::Style {
                    background: Some(Background::Color(if is_locked {
                        Color::from_rgba(1.0, 1.0, 1.0, 0.12)
                    } else {
                        Color::from_rgba(1.0, 1.0, 1.0, 0.03)
                    })),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: border_c,
                    },
                    text_color: text_c,
                    ..button::Style::default()
                })
                .into();
                rows_col = rows_col.push(
                    row![
                        container(lock_btn)
                            .width(Length::Fixed(42.0))
                            .padding([0, 6]),
                        text(entry.current.clone())
                            .size(11)
                            .color(cur_color)
                            .width(Length::FillPortion(2)),
                        text(proposed_display)
                            .size(11)
                            .color(new_color)
                            .width(Length::FillPortion(2)),
                        text(entry.sheet.clone())
                            .size(11)
                            .color(text_muted)
                            .width(Length::FillPortion(3)),
                    ]
                    .align_y(iced::Alignment::Center)
                    .padding([2, 8]),
                );
            }
        }

        let right_list = container(scrollable(rows_col).height(520))
            .padding(4)
            .style(bordered_style(border_c));

        // Count only rows where proposed != current (actual changes).
        let changes: usize = proposed.iter().filter(|e| e.current != e.proposed).count();
        let summary_text = if changes == 0 {
            format!(
                "Annotation has nothing to do — all {total_symbols} symbols on '{current_sheet_name}' already carry a designator. Use Reset All or Reset & Renumber below to renumber from scratch."
            )
        } else {
            format!(
                "Annotation will assign new designators to {changes} of {total_symbols} symbols on '{current_sheet_name}'. Click Accept Changes to apply, Update Changes List to recompute, or Reset All to clear every number first.",
            )
        };
        let summary = container(text(summary_text).size(10).color(text_muted)).padding([4, 4]);

        let right_col = column![
            right_title,
            Space::new().height(6),
            list_headers,
            right_list,
            Space::new().height(6),
            text("Annotation Summary").size(10).color(text_c),
            summary,
        ]
        .spacing(0)
        .width(Length::FillPortion(11));

        // ── Footer ──
        let footer = row![
            secondary_button(
                "All On",
                Message::CloseAnnotateDialog, // placeholder — multi-sheet v1.1
                text_c,
                border_c,
            ),
            Space::new().width(4),
            secondary_button(
                "All Off",
                Message::CloseAnnotateDialog, // placeholder — multi-sheet v1.1
                text_c,
                border_c,
            ),
            Space::new().width(Length::Fill),
            secondary_button(
                "Update Changes List",
                Message::CloseAnnotateDialog, // preview is already live; close = cheap redraw
                text_c,
                border_c,
            ),
            Space::new().width(4),
            secondary_button(
                "Reset All",
                Message::Annotate(signex_engine::AnnotateMode::ResetOnly),
                text_c,
                border_c,
            ),
            Space::new().width(4),
            secondary_button(
                "Reset & Renumber",
                Message::Annotate(signex_engine::AnnotateMode::ResetAndRenumber),
                text_c,
                border_c,
            ),
            Space::new().width(4),
            primary_button(
                "Accept Changes",
                if changes == 0 {
                    None
                } else {
                    Some(Message::Annotate(signex_engine::AnnotateMode::Incremental))
                },
                border_c,
            ),
            Space::new().width(4),
            secondary_button("Close", Message::CloseAnnotateDialog, text_c, border_c),
        ]
        .align_y(iced::Alignment::Center);

        let body_row = row![
            container(left_col).padding([14, 14]),
            container(iced::widget::Space::new().width(1).height(Length::Fill),).style(
                move |_: &Theme| container::Style {
                    background: Some(Background::Color(border_c)),
                    ..container::Style::default()
                }
            ),
            container(right_col).padding([14, 14]),
        ]
        .spacing(0);

        let modal_w = 1100.0_f32;
        let modal_h = 760.0_f32;
        let dialog = container(
            column![header, body_row, container(footer).padding([10, 14]),]
                .width(modal_w)
                .height(modal_h),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);

        dialog.into()
    }

    pub(super) fn view_annotate_reset_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_annotate_reset_confirm_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::states::ModalId::AnnotateResetConfirm)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (420.0, 180.0))
    }

    pub(super) fn view_annotate_reset_confirm_body(&self) -> Element<'_, Message> {
        self.view_annotate_reset_confirm_body_inner(false)
    }

    fn view_annotate_reset_confirm_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let theme_id = self.ui_state.theme_id;
        let header_content: Element<'_, Message> = container(
            row![
                text("Reset All Annotations")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::CloseAnnotateResetConfirm, theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let _ = border_c;
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::states::ModalId::AnnotateResetConfirm,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            detached_header(
                header_content,
                super::super::states::ModalId::AnnotateResetConfirm,
            )
        };
        let dialog = container(
            column![
                header,
                container(
                    text(
                        "Every reference designator will be reset to '?', then the sheet will be renumbered from 1.\nThis cannot be undone through Ctrl+Z alone — consider saving first.",
                    )
                    .size(11)
                    .color(text_muted),
                )
                .padding([14, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button("Cancel", Message::CloseAnnotateResetConfirm, text_c, border_c),
                        Space::new().width(8),
                        primary_button(
                            "Reset & Renumber",
                            Some(Message::Annotate(
                                signex_engine::AnnotateMode::ResetAndRenumber,
                            )),
                            border_c,
                        ),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 14]),
            ]
            .width(420),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    pub(super) fn view_erc_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_erc_dialog_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::states::ModalId::ErcDialog)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (1000.0, 600.0))
    }

    pub(super) fn view_erc_dialog_body(&self) -> Element<'_, Message> {
        self.view_erc_dialog_body_inner(false)
    }

    fn view_erc_dialog_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let theme_id = self.ui_state.theme_id;
        let header_content: Element<'_, Message> = container(
            row![
                text("Electrical Rules Check")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::CloseErcDialog, theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let _ = border_c;
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::states::ModalId::ErcDialog,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            detached_header(header_content, super::super::states::ModalId::ErcDialog)
        };

        // Per-rule severity grid. 11 rules × 4 severities.
        let mut rule_rows = column![
            row![
                text("Rule")
                    .size(10)
                    .color(text_muted)
                    .width(Length::FillPortion(3)),
                text("Severity")
                    .size(10)
                    .color(text_muted)
                    .width(Length::FillPortion(2)),
            ]
            .padding([4, 8]),
        ]
        .spacing(0);
        for rule in ALL_RULES {
            let current = self
                .ui_state
                .erc
                .severity_override
                .get(rule)
                .copied()
                .unwrap_or_else(|| rule.default_severity());
            rule_rows = rule_rows.push(
                row![
                    text(rule.label())
                        .size(11)
                        .color(text_c)
                        .width(Length::FillPortion(3)),
                    severity_segmented(*rule, current, border_c, text_c, text_muted),
                ]
                .align_y(iced::Alignment::Center)
                .padding([2, 8]),
            );
        }

        let footer = row![
            Space::new().width(Length::Fill),
            primary_button("Run ERC", Some(Message::RunErc), border_c),
            Space::new().width(8),
            secondary_button("Close", Message::CloseErcDialog, text_c, border_c),
        ]
        .align_y(iced::Alignment::Center);

        // Two-column body: rules table on the left, pin-connection
        // matrix on the right. Matches Altium's ERC setup layout and
        // lets both the full 11-rule list and the full 6×6 matrix
        // breathe without internal scrolling.
        let rules_pane = column![
            text("Severity per rule").size(11).color(text_c),
            text("Click a cell to change.").size(10).color(text_muted),
            Space::new().height(8),
            container(rule_rows)
                .padding(4)
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border_c,
                    },
                    ..container::Style::default()
                }),
        ]
        .spacing(2)
        .width(Length::FillPortion(3));

        let matrix_pane = column![
            text("Pin Connection Matrix").size(11).color(text_c),
            text("Click a cell to cycle severity.")
                .size(10)
                .color(text_muted),
            Space::new().height(8),
            container(pin_matrix_view(tokens, &self.ui_state.erc.pin_matrix_overrides))
                .padding(8)
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border_c,
                    },
                    ..container::Style::default()
                }),
        ]
        .spacing(2)
        .width(Length::FillPortion(2));

        let body =
            row![rules_pane, Space::new().width(16), matrix_pane,].align_y(iced::Alignment::Start);

        let dialog = container(
            column![
                header,
                container(body)
                    .padding([14, 14])
                    .width(Length::Fill)
                    .height(Length::Fill),
                container(footer).padding([10, 14]),
            ]
            .width(1000)
            .height(600),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    // ────────────────────────────────────────────────────────────────
    // Small Altium-style modals (Rename / Remove-from-Project / Close-
    // Tab Confirm). All three render through the same chrome used by
    // Annotate / ERC: `draggable_header` + `close_x_button` on the
    // right + `wrap_modal` for absolute positioning with drag-offset
    // persistence in `ui_state.modal_offsets`.
    // ────────────────────────────────────────────────────────────────

    pub(super) fn view_rename_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_rename_dialog_body();
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::states::ModalId::RenameDialog)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (420.0, 200.0))
    }

    fn view_rename_dialog_body(&self) -> Element<'_, Message> {
        use iced::widget::text_input;

        let Some(ref st) = self.ui_state.rename_dialog else {
            return container(Space::new()).into();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let error_c = Color::from_rgb(0.90, 0.35, 0.30);

        let theme_id = self.ui_state.theme_id;
        let header_content: Element<'_, Message> = container(
            row![
                text("Rename File")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::CloseRenameDialog, theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let _ = border_c;
        let header = draggable_header(
            header_content,
            super::super::states::ModalId::RenameDialog,
            self.interaction_state.last_mouse_pos,
        );

        let current_name = st
            .target_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let mut body: iced::widget::Column<'_, Message> = column![
            text(format!("Rename \"{}\"", current_name))
                .size(11)
                .color(text_muted),
            text_input("new-name.snxsch", &st.buffer)
                .on_input(Message::RenameBufferChanged)
                .on_submit(Message::RenameSubmit)
                .size(12)
                .padding(6)
                .width(Length::Fill),
        ]
        .spacing(8);

        if let Some(ref err) = st.error {
            body = body.push(text(err.clone()).size(11).color(error_c));
        }

        let dialog = container(
            column![
                header,
                container(body).padding([14, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button("Cancel", Message::CloseRenameDialog, text_c, border_c),
                        Space::new().width(8),
                        primary_button("Rename", Some(Message::RenameSubmit), border_c),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 14]),
            ]
            .width(420),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    pub(super) fn view_remove_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_remove_dialog_body();
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::states::ModalId::RemoveDialog)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (560.0, 260.0))
    }

    fn view_remove_dialog_body(&self) -> Element<'_, Message> {
        let Some(ref st) = self.ui_state.remove_dialog else {
            return container(Space::new()).into();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let theme_id = self.ui_state.theme_id;
        let header_content: Element<'_, Message> = container(
            row![
                text("Remove from Project")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::CloseRemoveDialog, theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let _ = border_c;
        let header = draggable_header(
            header_content,
            super::super::states::ModalId::RemoveDialog,
            self.interaction_state.last_mouse_pos,
        );

        let option_card = |title: &'static str,
                           subtitle: &'static str,
                           msg: Message|
         -> Element<'_, Message> {
            let title_owned = title.to_string();
            let subtitle_owned = subtitle.to_string();
            button(
                column![
                    text(format!("\u{2192} {}", title_owned)).size(12).color(text_c),
                    text(subtitle_owned).size(10).color(text_muted),
                ]
                .spacing(4)
                .padding([2, 0]),
            )
            .on_press(msg)
            .padding([10, 14])
            .width(Length::Fill)
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                    }
                    _ => Color::from_rgba(1.0, 1.0, 1.0, 0.02),
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    border: Border {
                        width: 1.0,
                        radius: 4.0.into(),
                        color: border_c,
                    },
                    text_color: text_c,
                    ..button::Style::default()
                }
            })
            .into()
        };

        let dialog = container(
            column![
                header,
                container(
                    text(format!(
                        "Please choose how to remove file \"{}\"",
                        st.display_name
                    ))
                    .size(11)
                    .color(text_muted)
                )
                .padding([14, 14]),
                container(
                    column![
                        option_card(
                            "Delete file",
                            "File will be removed from project and permanently deleted.",
                            Message::RemoveConfirm(crate::app::RemoveChoice::DeleteFile),
                        ),
                        option_card(
                            "Exclude from project",
                            "File will be excluded from project but left in local folder. Not recommended for projects stored in version control.",
                            Message::RemoveConfirm(crate::app::RemoveChoice::ExcludeFromProject),
                        ),
                    ]
                    .spacing(8)
                )
                .padding([0, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button("Cancel", Message::CloseRemoveDialog, text_c, border_c),
                    ],
                )
                .padding([14, 14]),
            ]
            .width(560),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    pub(super) fn view_bom_preview(&self) -> Element<'_, Message> {
        let modal_w = 1000.0_f32;
        let modal_h = 700.0_f32;
        let dialog = self.view_bom_preview_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::states::ModalId::BomPreview)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (modal_w, modal_h))
    }

    pub(super) fn view_bom_preview_body(&self) -> Element<'_, Message> {
        self.view_bom_preview_body_inner(false)
    }

    fn view_bom_preview_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        use signex_output::{BomColumn, BomFormat, BomGrouping};
        let Some(ref preview) = self.document_state.bom_preview else {
            return container(Space::new()).into();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let theme_id = self.ui_state.theme_id;

        let header_content: Element<'_, Message> = container(
            row![
                text("Bill of Materials")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::BomPreviewClose, theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let _ = border_c;
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::states::ModalId::BomPreview,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            detached_header(header_content, super::super::states::ModalId::BomPreview)
        };

        // Pill style mirrors the main-app right-side Properties
        // panel: subtle theme-border at rest, theme-accent border
        // when active (orange on Altium Dark, cyan on Alp Lab,
        // etc.). Active fill is the same accent at 15 % alpha so
        // the pill reads as "selected" without being a hard solid
        // fill that fights the theme.
        let accent_c = crate::styles::ti(tokens.accent);
        let row_pill = |label: String,
                        on: bool,
                        msg: Message|
         -> Element<'_, Message> {
            button(text(label).size(11).color(text_c))
                .padding([4, 10])
                .on_press(msg)
                .style(move |_: &Theme, status: button::Status| {
                    let bg = match (on, status) {
                        (true, _) => Color { a: 0.15, ..accent_c },
                        (false, button::Status::Hovered | button::Status::Pressed) => {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.10)
                        }
                        _ => Color::from_rgba(1.0, 1.0, 1.0, 0.04),
                    };
                    let border_color = if on { accent_c } else { border_c };
                    button::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: border_color,
                        },
                        text_color: text_c,
                        ..button::Style::default()
                    }
                })
                .into()
        };

        let grouping_row: Element<'_, Message> = row![
            text("Grouping:").size(11).color(text_muted),
            Space::new().width(8),
            row_pill(
                "Grouped".to_string(),
                preview.options.grouping == BomGrouping::Grouped,
                Message::BomPreviewSetGrouping(BomGrouping::Grouped),
            ),
            Space::new().width(4),
            row_pill(
                "Ungrouped".to_string(),
                preview.options.grouping == BomGrouping::Ungrouped,
                Message::BomPreviewSetGrouping(BomGrouping::Ungrouped),
            ),
            Space::new().width(4),
            row_pill(
                "Flat".to_string(),
                preview.options.grouping == BomGrouping::Flat,
                Message::BomPreviewSetGrouping(BomGrouping::Flat),
            ),
        ]
        .align_y(iced::Alignment::Center)
        .into();

        let format_row: Element<'_, Message> = row![
            text("Format:").size(11).color(text_muted),
            Space::new().width(8),
            row_pill(
                "CSV".to_string(),
                preview.options.format == BomFormat::Csv,
                Message::BomPreviewSetFormat(BomFormat::Csv),
            ),
            Space::new().width(4),
            row_pill(
                "XLSX".to_string(),
                preview.options.format == BomFormat::Xlsx,
                Message::BomPreviewSetFormat(BomFormat::Xlsx),
            ),
            Space::new().width(4),
            row_pill(
                "HTML".to_string(),
                preview.options.format == BomFormat::Html,
                Message::BomPreviewSetFormat(BomFormat::Html),
            ),
        ]
        .align_y(iced::Alignment::Center)
        .into();

        let toggle_row: Element<'_, Message> = row![
            row_pill(
                "Include DNP".to_string(),
                preview.options.include_dnp,
                Message::BomPreviewSetIncludeDnp(!preview.options.include_dnp),
            ),
            Space::new().width(4),
            row_pill(
                "Include Not Fitted".to_string(),
                preview.options.include_not_fitted,
                Message::BomPreviewSetIncludeNotFitted(!preview.options.include_not_fitted),
            ),
        ]
        .align_y(iced::Alignment::Center)
        .into();

        // Variant picker — only shown when the active project
        // declares variants. "Base" is the no-override view.
        let variant_row: Element<'_, Message> = if preview.variants.is_empty() {
            Space::new().into()
        } else {
            let active_label = preview
                .options
                .active_variant
                .clone()
                .unwrap_or_else(|| "Base".to_string());
            let mut r = row![
                text("Variant:").size(11).color(text_muted),
                Space::new().width(8),
                row_pill(
                    "Base".to_string(),
                    preview.options.active_variant.is_none(),
                    Message::BomPreviewSetVariant(None),
                ),
            ]
            .align_y(iced::Alignment::Center);
            for variant in &preview.variants {
                let is_active = active_label.as_str() == variant.as_str();
                r = r.push(Space::new().width(4));
                r = r.push(row_pill(
                    variant.clone(),
                    is_active,
                    Message::BomPreviewSetVariant(Some(variant.clone())),
                ));
            }
            r.into()
        };

        // Column picker — toggles for the standard column set + any
        // custom-field column discovered in the rolled-up rows. The
        // pill state mirrors `preview.options.columns`; clicking
        // adds/removes from that Vec via `handle_bom_preview_toggle_column`.
        let mut custom_keys: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        for r in &preview.table.rows {
            for k in r.custom.keys() {
                custom_keys.insert(k.clone());
            }
        }
        let column_options: Vec<(BomColumn, &'static str)> = vec![
            (BomColumn::Name, "Name"),
            (BomColumn::Description, "Description"),
            (BomColumn::Designator, "Designator"),
            (BomColumn::Value, "Value"),
            (BomColumn::Footprint, "Footprint"),
            (BomColumn::LibRef, "LibRef"),
            (BomColumn::Qty, "Qty"),
        ];
        let mut column_row = row![text("Columns:").size(11).color(text_muted), Space::new().width(8)]
            .align_y(iced::Alignment::Center);
        for (col, label) in &column_options {
            let on = preview.options.columns.iter().any(|c| c == col);
            column_row = column_row.push(row_pill(
                label.to_string(),
                on,
                Message::BomPreviewToggleColumn(col.clone()),
            ));
            column_row = column_row.push(Space::new().width(4));
        }
        for key in &custom_keys {
            let col = BomColumn::Custom(key.clone());
            let on = preview.options.columns.iter().any(|c| c == &col);
            column_row = column_row.push(row_pill(
                key.clone(),
                on,
                Message::BomPreviewToggleColumn(col),
            ));
            column_row = column_row.push(Space::new().width(4));
        }
        let column_row: Element<'_, Message> = scrollable(column_row)
            .direction(iced::widget::scrollable::Direction::Horizontal(
                iced::widget::scrollable::Scrollbar::new()
                    .width(0)
                    .scroller_width(0),
            ))
            .into();

        // Build the table from `preview.options.columns`. Width
        // resolution: user override (set by the column-resize
        // handle drag) takes precedence; otherwise fall back to
        // the per-`BomColumn` default below.
        let default_column_width = |c: &BomColumn| -> f32 {
            match c {
                BomColumn::Name => 140.0,
                BomColumn::Description => 220.0,
                BomColumn::Designator | BomColumn::Reference => 220.0,
                BomColumn::Value => 110.0,
                BomColumn::Footprint => 140.0,
                BomColumn::LibRef => 160.0,
                BomColumn::Qty => 50.0,
                BomColumn::Custom(_) => 120.0,
            }
        };
        let column_width = |idx: usize, c: &BomColumn| -> f32 {
            preview
                .column_widths
                .get(&idx)
                .copied()
                .unwrap_or_else(|| default_column_width(c))
        };
        let column_value = |c: &BomColumn, r: &signex_output::BomRow| -> String {
            match c {
                BomColumn::Name => r.name.clone(),
                BomColumn::Description => r.description.clone(),
                BomColumn::Designator | BomColumn::Reference => r.references.join(", "),
                BomColumn::Value => r.value.clone(),
                BomColumn::Footprint => r.footprint.clone(),
                BomColumn::LibRef => r.lib_ref.clone(),
                BomColumn::Qty => r.qty.to_string(),
                BomColumn::Custom(key) => r.custom.get(key).cloned().unwrap_or_default(),
            }
        };
        // Compute the table's full content width — gutter + 1 px
        // gutter→data divider + sum(column_width) + (n-1) inter-
        // column dividers + (n-1) resize-handle slots. Used as
        // the Fixed width on the table inner so Direction::Both
        // scrollable knows when to engage horizontal scrolling.
        let data_columns_width: f32 = preview
            .options
            .columns
            .iter()
            .enumerate()
            .map(|(idx, c)| column_width(idx, c))
            .sum();
        let n_data = preview.options.columns.len();
        let dividers_width = n_data.saturating_sub(1) as f32;
        let resize_slots_width = n_data.saturating_sub(1) as f32 * 4.0;
        let table_width: f32 = 36.0 + 1.0 + data_columns_width
            + dividers_width + resize_slots_width;
        // Headers: clickable + draggable. Click cycles sort; press
        // arms a drag, release on another header drops the source
        // column at that index. Sort indicator (▲/▼) appears next to
        // the active sort column.
        let sort_arrow = |idx: usize| -> &'static str {
            match preview.sort {
                Some((c, true)) if c == idx => " ▲",
                Some((c, false)) if c == idx => " ▼",
                _ => "",
            }
        };
        // Row-number column (left) — Altium parity. No header label,
        // just shows the row position. Width 36 holds 1-9999 cleanly.
        // Header cells share `HEADER_ROW_H`; data rows share
        // `DATA_ROW_H`. Both centered vertically so text doesn't
        // float at the top of its stripe.
        const NUM_COL_WIDTH: f32 = 36.0;
        const HEADER_ROW_H: f32 = 24.0;
        const DATA_ROW_H: f32 = 22.0;
        let num_header: Element<'_, Message> = container(text(""))
            .width(Length::Fixed(NUM_COL_WIDTH))
            .height(HEADER_ROW_H)
            .into();

        let mut header_row: Row<'_, Message> =
            Row::new().spacing(0).align_y(iced::Alignment::Center).push(num_header);
        // Vertical divider between row-number column and the data
        // columns so the gutter is visibly its own zone.
        header_row = header_row.push(
            container(Space::new())
                .width(1)
                .height(HEADER_ROW_H)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(border_c)),
                    ..container::Style::default()
                }),
        );
        // Index of the last data column — the one that uses
        // Length::Fill so the table eats any leftover horizontal
        // space when the modal is wider than the sum of fixed
        // column widths.
        let last_data_col_idx = preview.options.columns.len().saturating_sub(1);
        // Only treat the press as a "real" drag once the cursor
        // has moved past 6 px from where it pressed — same
        // threshold the tab drag-ghost uses. A press-and-release
        // without motion counts as a click and never lights the
        // header up.
        const COL_DRAG_THRESHOLD_PX: f32 = 6.0;
        let cursor_x = self.interaction_state.last_mouse_pos.0;
        let active_drag = match (preview.column_drag, preview.column_drag_press_x) {
            (Some(idx), Some(ox))
                if (cursor_x - ox).abs() > COL_DRAG_THRESHOLD_PX =>
            {
                Some(idx)
            }
            _ => None,
        };
        let _ = preview.column_hover;
        for (idx, c) in preview.options.columns.iter().enumerate() {
            // Header bg is neutral by default. The only time a
            // column lights up is while the user is actively
            // dragging its header to reorder (cursor moved past
            // threshold) — sort doesn't get a highlight (the
            // ▲/▼ glyph already conveys that).
            let cell_bg = if active_drag == Some(idx) {
                Some(Background::Color(Color {
                    a: 0.22,
                    ..crate::styles::ti(tokens.accent)
                }))
            } else {
                None
            };
            // Header cell content: column name on the left, sort
            // arrow (when this column is the active sort) anchored
            // to the right via Space::Fill. Spreadsheet convention.
            let arrow = sort_arrow(idx).trim();
            let cell_content = row![
                text(c.header().to_string()).size(11).color(text_c),
                Space::new().width(Length::Fill),
                text(arrow.to_string()).size(11).color(text_c),
            ]
            .align_y(iced::Alignment::Center);
            // All headers use Fixed widths — needed for the
            // Direction::Both scrollable to know its content
            // size and engage horizontal scroll when columns
            // overflow the viewport. The previous Fill-last
            // experiment killed horizontal scrolling because
            // Both-direction collapses Fill children to zero.
            let header_cell_width = Length::Fixed(column_width(idx, c));
            let _ = last_data_col_idx;
            let cell = container(cell_content)
                .width(header_cell_width)
                .height(HEADER_ROW_H)
                .padding([0, 6])
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_: &Theme| container::Style {
                    background: cell_bg,
                    ..container::Style::default()
                });
            // mouse_area gives us press (drag-start) + release
            // + on_enter / on_exit. on_release fires on the
            // press-source widget; the drop logic uses
            // `column_hover` (set via on_enter on the target) to
            // resolve where the cursor was at release time.
            let header_cell: Element<'_, Message> = iced::widget::mouse_area(cell)
                .on_press(Message::BomPreviewColumnDragStart(idx))
                .on_release(if preview.column_drag == Some(idx) {
                    Message::BomPreviewSortColumn(idx)
                } else {
                    Message::BomPreviewColumnDragDrop(idx)
                })
                .on_enter(Message::BomPreviewColumnHoverEnter(idx))
                .on_exit(Message::BomPreviewColumnHoverExit(idx))
                .interaction(iced::mouse::Interaction::Pointer)
                .into();
            header_row = header_row.push(header_cell);
            if idx + 1 < preview.options.columns.len() {
                // 1 px theme-border vertical divider + 4 px
                // transparent resize handle. No accent bg in
                // either — they sit between columns and shouldn't
                // bleed sort highlights into neighbouring cells.
                header_row = header_row.push(
                    container(Space::new())
                        .width(1)
                        .height(24)
                        .style(move |_: &Theme| container::Style {
                            background: Some(Background::Color(border_c)),
                            ..container::Style::default()
                        }),
                );
                let resize_handle: Element<'_, Message> = iced::widget::mouse_area(
                    container(Space::new()).width(4).height(HEADER_ROW_H),
                )
                .on_press(Message::BomPreviewColumnResizeStart(idx))
                .on_release(Message::BomPreviewColumnResizeEnd)
                .interaction(iced::mouse::Interaction::ResizingHorizontally)
                .into();
                header_row = header_row.push(resize_handle);
            }
        }
        // Header strip locked to `table_width` so it scrolls in
        // lockstep with the body rows when the user pans
        // horizontally.
        let table_header_el: Element<'_, Message> = container(header_row)
            .style(crate::styles::toolbar_strip(tokens))
            .width(Length::Fixed(table_width.max(100.0)))
            .into();

        // Sort the row order if a sort spec is set. We sort indexes
        // into preview.table.rows so we can borrow rows by reference
        // without cloning the BomRow Vec. Numeric columns (Qty) sort
        // numerically; the rest sort case-insensitively.
        let mut row_order: Vec<usize> = (0..preview.table.rows.len()).collect();
        if let Some((sort_idx, asc)) = preview.sort
            && let Some(sort_col) = preview.options.columns.get(sort_idx)
        {
            let key = |r: &signex_output::BomRow| column_value(sort_col, r);
            row_order.sort_by(|&a, &b| {
                let ra = &preview.table.rows[a];
                let rb = &preview.table.rows[b];
                let cmp = match sort_col {
                    BomColumn::Qty => ra.qty.cmp(&rb.qty),
                    _ => key(ra)
                        .to_ascii_lowercase()
                        .cmp(&key(rb).to_ascii_lowercase()),
                };
                if asc { cmp } else { cmp.reverse() }
            });
        }

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(preview.table.rows.len());
        for (visible_idx, &row_idx) in row_order.iter().enumerate() {
            let r = &preview.table.rows[row_idx];
            let alt_bg = if visible_idx % 2 == 0 {
                Color::from_rgba(1.0, 1.0, 1.0, 0.0)
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.025)
            };
            // Single-line cell: rely on `wrapping(None)` +
            // `clip(true)` to cut overflow at the right edge.
            // Manual char-count truncation was over-trimming —
            // hard to estimate per-glyph width across themes /
            // fonts, and the "…" suffix never showed when our
            // estimate was too wide. iced 0.14 lacks a built-in
            // ellipsis primitive; clipping is the honest choice.
            const CELL_TEXT_SIZE: f32 = 11.0;
            let cell = |s: String, width: Length, _truncate_at: f32| -> Element<'_, Message> {
                container(
                    text(s)
                        .size(CELL_TEXT_SIZE)
                        .color(text_c)
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .width(width)
                .height(DATA_ROW_H)
                .padding([0, 6])
                .align_y(iced::alignment::Vertical::Center)
                .clip(true)
                .into()
            };
            // Leftmost row-number gutter cell — muted text, right-aligned.
            let num_cell: Element<'_, Message> = container(
                text((visible_idx + 1).to_string())
                    .size(CELL_TEXT_SIZE)
                    .color(text_muted),
            )
            .width(Length::Fixed(36.0))
            .height(DATA_ROW_H)
            .padding([0, 6])
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Center)
            .into();
            // Body rows: subtle 1 px vertical column dividers —
            // theme-border at 30 % alpha so they read as faint
            // groove lines without competing with the header
            // dividers above. Same ordering as the header so
            // columns line up pixel-for-pixel.
            let subtle_divider_color = Color {
                a: 0.3,
                ..border_c
            };
            let body_divider = move || -> Element<'_, Message> {
                container(Space::new())
                    .width(1)
                    .height(DATA_ROW_H)
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(subtle_divider_color)),
                        ..container::Style::default()
                    })
                    .into()
            };
            let mut row_inner: Row<'_, Message> = Row::new().spacing(0).push(num_cell);
            row_inner = row_inner.push(body_divider());
            for (col_idx, c) in preview.options.columns.iter().enumerate() {
                // Every column uses Fixed widths so the
                // Direction::Both scrollable can compute total
                // content width and turn on horizontal scroll
                // when columns overflow the viewport.
                let w = column_width(col_idx, c);
                let (cell_width, truncate_at) = (Length::Fixed(w), w);
                row_inner = row_inner.push(cell(column_value(c, r), cell_width, truncate_at));
                if col_idx + 1 < preview.options.columns.len() {
                    row_inner = row_inner.push(body_divider());
                    // 4 px transparent spacer to mirror the
                    // header's resize-handle width — keeps
                    // columns pixel-aligned with the header.
                    row_inner = row_inner.push(Space::new().width(4));
                }
            }
            let row_el = container(row_inner)
                .width(Length::Fixed(table_width.max(100.0)))
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(alt_bg)),
                    ..container::Style::default()
                });
            rows.push(row_el.into());
        }
        // Header + rows live inside the same vertical column so the
        // horizontal scroll moves them together. Without this, the
        // header was wrapped separately and stayed fixed while the
        // body rows scrolled past it.
        // Both-direction scrollable. Header + rows live inside
        // the same fixed-width column so horizontal scrolling
        // moves them together (the user pans columns left/right
        // without the header staying in place). The 16 px
        // bottom buffer keeps the last data row above the
        // horizontal scrollbar's reserved zone — without it,
        // row N would clip when the scrollable's H bar paints
        // inside the viewport at the bottom.
        let table_inner: Element<'_, Message> = column![
            table_header_el,
            column(rows).spacing(0),
            Space::new().height(16),
        ]
        .width(Length::Fixed(table_width.max(100.0)))
        .into();
        let body = scrollable(table_inner)
            .direction(iced::widget::scrollable::Direction::Both {
                vertical: iced::widget::scrollable::Scrollbar::default(),
                horizontal: iced::widget::scrollable::Scrollbar::default(),
            })
            .width(Length::Fill)
            .height(Length::Fill);

        let active_variant_label = preview
            .options
            .active_variant
            .clone()
            .unwrap_or_else(|| "Base".to_string());
        let row_count = preview.table.rows.len();
        let total_count = preview.table.rows.len();
        let status_label = format!(
            "{} of {} lines visible",
            row_count, total_count
        );
        let variant_label = format!("Current variant: {}", active_variant_label);

        // Variant dropdown for the top toolbar — placeholder in the
        // header strip area, sits to the left. (i) info badge sits
        // on the right.
        let variant_dropdown: Element<'_, Message> = if preview.variants.is_empty() {
            text(format!("Variant: {}", active_variant_label))
                .size(11)
                .color(text_muted)
                .into()
        } else {
            container(
                row![
                    Space::new().width(Length::Fill),
                    text(active_variant_label.clone()).size(11).color(text_c),
                    Space::new().width(Length::Fill),
                    text("\u{25BE}").size(10).color(text_c),
                ]
                .align_y(iced::Alignment::Center),
            )
            .width(160)
            .height(24)
            .padding([0, 10])
            .style(move |_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border_c,
                },
                ..iced::widget::container::Style::default()
            })
            .into()
        };
        let info_badge: Element<'_, Message> = container(
            text("i").size(11).color(iced::Color::WHITE),
        )
        .width(20)
        .height(20)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color {
                a: 0.7,
                ..crate::styles::ti(tokens.accent)
            })),
            border: iced::Border {
                width: 0.0,
                radius: 10.0.into(),
                color: iced::Color::TRANSPARENT,
            },
            ..iced::widget::container::Style::default()
        })
        .into();
        let toolbar_strip = container(
            row![
                variant_dropdown,
                Space::new().width(Length::Fill),
                info_badge,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([8, 14])
        .width(Length::Fill)
        .style(crate::styles::toolbar_strip(tokens));

        // Properties sidebar — General / Columns tabs, collapsible
        // sections inside.
        use crate::app::states::BomSidebarTab;
        let sidebar_tab = preview.sidebar_tab;
        let tab_pill = |label: &'static str, target: BomSidebarTab| -> Element<'_, Message> {
            let on = sidebar_tab == target;
            button(text(label.to_string()).size(11).color(text_c))
                .padding([4, 12])
                .on_press(Message::BomPreviewSetSidebarTab(target))
                .style(move |_: &iced::Theme, status: button::Status| {
                    let bg = match (on, status) {
                        (true, _) => iced::Color { a: 0.15, ..accent_c },
                        (false, button::Status::Hovered) => {
                            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                        }
                        _ => iced::Color::TRANSPARENT,
                    };
                    let border_color = if on { accent_c } else { border_c };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        border: iced::Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: border_color,
                        },
                        text_color: text_c,
                        ..button::Style::default()
                    }
                })
                .into()
        };

        let general_pane: Element<'_, Message> = column![
            section_header("BOM Items", text_muted),
            container(
                column![
                    grouping_row,
                    Space::new().height(6),
                    toggle_row,
                    Space::new().height(6),
                    variant_row,
                ]
                .spacing(0),
            )
            .padding([8, 12]),
            Space::new().height(8),
            section_header("Export Options", text_muted),
            container(
                column![format_row]
                .spacing(0),
            )
            .padding([8, 12]),
        ]
        .spacing(0)
        .into();

        // Columns tab — vertical list with a tick checkbox per row,
        // mirroring Altium's Columns table. Same standard column
        // set + any custom-field column discovered in the rolled
        // rows. Each row is clickable and toggles via
        // `Message::BomPreviewToggleColumn` — the existing handler
        // already does add/remove on the Vec.
        let _ = column_row; // pill row obsoleted by the list layout
        let mut col_list: Column<'_, Message> = Column::new().spacing(0);
        col_list = col_list.push(section_header("Columns", text_muted));
        let mut list_items: Column<'_, Message> = Column::new().spacing(0);
        let render_col_row = |col: BomColumn,
                              label: String,
                              on: bool|
         -> Element<'_, Message> {
            let pip_bg = if on {
                Color::from_rgb(0.00, 0.47, 0.84)
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.04)
            };
            let inner = if on {
                text("\u{2713}").size(9).color(Color::WHITE)
            } else {
                text(" ").size(9)
            };
            let pip = container(inner)
                .width(14)
                .height(14)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(pip_bg)),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: border_c,
                    },
                    ..container::Style::default()
                });
            button(
                row![
                    pip,
                    Space::new().width(8),
                    text(label).size(11).color(text_c),
                    Space::new().width(Length::Fill),
                ]
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::BomPreviewToggleColumn(col))
            .padding([3, 12])
            .width(Length::Fill)
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04)))
                    }
                    _ => None,
                };
                button::Style {
                    background: bg,
                    border: Border::default(),
                    text_color: text_c,
                    ..button::Style::default()
                }
            })
            .into()
        };
        for (col, label) in &column_options {
            let on = preview.options.columns.iter().any(|c| c == col);
            list_items = list_items.push(render_col_row(col.clone(), label.to_string(), on));
        }
        for key in &custom_keys {
            let col = BomColumn::Custom(key.clone());
            let on = preview.options.columns.iter().any(|c| c == &col);
            list_items = list_items.push(render_col_row(col, key.clone(), on));
        }
        col_list = col_list.push(container(list_items).padding([4, 0]));
        let columns_pane: Element<'_, Message> = col_list.into();

        let sidebar_body: Element<'_, Message> = match sidebar_tab {
            BomSidebarTab::General => general_pane,
            BomSidebarTab::Columns => columns_pane,
        };
        let sidebar = column![
            container(text("Properties").size(13).color(text_c))
                .padding([10, 12])
                .width(Length::Fill),
            container(
                row![
                    tab_pill("General", BomSidebarTab::General),
                    Space::new().width(4),
                    tab_pill("Columns", BomSidebarTab::Columns),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([0, 12]),
            Space::new().height(8),
            scrollable(sidebar_body)
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .spacing(0);

        // Main row: scrollable table + 1 px divider + sidebar.
        // Body now uses nested scrollables (vertical inside
        // horizontal) so the horizontal bar lives below the last
        // row instead of clipping it — no bottom-padding hack
        // needed.
        let main_row = row![
            container(body)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([6, 8]),
            container(Space::new())
                .width(1)
                .height(Length::Fill)
                .style(move |_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(border_c)),
                    ..iced::widget::container::Style::default()
                }),
            container(sidebar)
                .width(Length::Fixed(320.0))
                .height(Length::Fill),
        ]
        .height(Length::Fill);

        // Combined footer: status text on the left + Export button
        // on the right, in ONE strip with bottom-rounded radius.
        // Dropping the separate status / button rows removes the
        // empty band between them and tightens the modal's
        // bottom chrome.
        let footer = container(
            row![
                text(status_label).size(10).color(text_muted),
                Space::new().width(12),
                text("|").size(10).color(text_muted),
                Space::new().width(12),
                text(variant_label).size(10).color(text_muted),
                Space::new().width(Length::Fill),
                primary_button_themed(
                    "Export\u{2026}",
                    Some(Message::BomPreviewExport),
                    border_c,
                    Some(crate::styles::ti(tokens.accent)),
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 14])
        .width(Length::Fill)
        .style(crate::styles::modal_footer_strip(tokens));

        // 1 px theme-border line — same style the main app uses
        // between the menu bar and the document tab strip
        // (`crate::styles::chrome_separator`). The inner Space is
        // sized Fill on both axes so iced doesn't shrink the
        // container to zero height in some layout scenarios.
        let title_separator = container(
            Space::new().width(Length::Fill).height(Length::Fill),
        )
        .width(Length::Fill)
        .height(1)
        .style(crate::styles::chrome_separator(tokens));

        let dialog = container(
            column![header, title_separator, toolbar_strip, main_row, footer]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    pub(super) fn view_project_close_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_project_close_confirm_body();
        // Reuse the rename modal's offset slot — `RenameDialog` is
        // never up at the same time as project-close confirm, and
        // they're roughly the same footprint, so reusing the slot
        // keeps modal-offset state simpler.
        let offset = (0.0, 0.0);
        wrap_modal(dialog, offset, self.ui_state.window_size, (520.0, 360.0))
    }

    fn view_project_close_confirm_body(&self) -> Element<'_, Message> {
        use crate::app::ProjectCloseChoice;
        let Some(ref st) = self.ui_state.project_close_confirm else {
            return container(Space::new()).into();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let theme_id = self.ui_state.theme_id;
        let header_content: Element<'_, Message> = container(
            row![
                text("Close Project — Unsaved Edits")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::ProjectCloseConfirm(ProjectCloseChoice::Cancel),
                    theme_id,
                    text_muted,
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let _ = border_c;
        let header = draggable_header(
            header_content,
            super::super::states::ModalId::RenameDialog,
            self.interaction_state.last_mouse_pos,
        );

        // List of dirty filenames, scrollable in case the project
        // has dozens of unsaved sheets. Filenames only — paths would
        // overflow narrow modals and the project context already
        // implies the parent dir.
        let mut file_rows: Vec<Element<'_, Message>> = Vec::new();
        for path in &st.dirty_paths {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("(unnamed)")
                .to_string();
            file_rows.push(
                container(text(name).size(11).color(text_c))
                    .padding([4, 8])
                    .into(),
            );
        }
        let file_list = scrollable(column(file_rows).spacing(0))
            .height(Length::Fixed(160.0))
            .width(Length::Fill);

        let dialog = container(
            column![
                header,
                container(
                    text(format!(
                        "'{}' has {} unsaved file(s). What would you like to do?",
                        st.project_name,
                        st.dirty_paths.len()
                    ))
                    .size(11)
                    .color(text_muted)
                )
                .padding([14, 14]),
                container(file_list).padding([0, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button(
                            "Cancel",
                            Message::ProjectCloseConfirm(ProjectCloseChoice::Cancel),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        secondary_button(
                            "Discard All",
                            Message::ProjectCloseConfirm(ProjectCloseChoice::DiscardAll),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        primary_button(
                            "Save All",
                            Some(Message::ProjectCloseConfirm(ProjectCloseChoice::SaveAll)),
                            border_c,
                        ),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([14, 14]),
            ]
            .width(520),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// Tiny inline checkbox pip — read-only indicator used inside the Annotate
/// dialog's parameter list and sheet table.
fn check_pip(on: bool, border: Color) -> Element<'static, Message> {
    let inner = if on {
        text("✓").size(9).color(Color::WHITE)
    } else {
        text(" ").size(9).color(Color::WHITE)
    };
    let bg = if on {
        Color::from_rgb(0.00, 0.47, 0.84)
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    };
    container(inner)
        .width(12)
        .height(12)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border,
            },
            ..container::Style::default()
        })
        .into()
}

/// Compact visual *legend* of the annotate order — this is intentionally a
/// static R1..R4 diagram that illustrates how four parts arranged in a 2×2
/// grid would be numbered under the selected traversal. It does NOT reflect
/// the user's actual components; it's the same convention Altium uses.
fn order_preview(
    order: AnnotateOrder,
    text_c: Color,
    text_muted: Color,
    border: Color,
) -> Element<'static, Message> {
    // Pick labels for each of the four slots (top-left, top-right, bottom-left,
    // bottom-right) matching the selected order.
    // Slot layout:
    //   (0,0) tl   (0,1) tr
    //   (1,0) bl   (1,1) br
    let slots = match order {
        // Column-major, ascending within the column:
        //  1 3
        //  2 4
        AnnotateOrder::UpThenAcross => ("R1", "R3", "R2", "R4"),
        // Column-major, descending within the column:
        //  2 4
        //  1 3
        AnnotateOrder::DownThenAcross => ("R2", "R4", "R1", "R3"),
        // Row-major, descending rows:
        //  1 2
        //  3 4
        AnnotateOrder::AcrossThenDown => ("R1", "R2", "R3", "R4"),
        // Row-major, ascending rows:
        //  3 4
        //  1 2
        AnnotateOrder::AcrossThenUp => ("R3", "R4", "R1", "R2"),
    };
    let cell = |label: &'static str| -> Element<'static, Message> {
        container(text(label.to_string()).size(10).color(text_c))
            .width(34)
            .height(20)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: border,
                },
                ..container::Style::default()
            })
            .into()
    };
    let arrow = match order {
        AnnotateOrder::UpThenAcross => "↑→",
        AnnotateOrder::DownThenAcross => "↓→",
        AnnotateOrder::AcrossThenDown => "→↓",
        AnnotateOrder::AcrossThenUp => "→↑",
    };
    container(
        column![
            text("Preview").size(9).color(text_muted),
            Space::new().height(2),
            row![cell(slots.0), Space::new().width(4), cell(slots.1),].spacing(0),
            Space::new().height(4),
            row![cell(slots.2), Space::new().width(4), cell(slots.3),].spacing(0),
            Space::new().height(2),
            text(arrow).size(11).color(text_muted),
        ]
        .spacing(0)
        .align_x(iced::Alignment::Center),
    )
    .padding(6)
    .style(bordered_style(border))
    .into()
}

fn bordered_style(border: Color) -> impl Fn(&Theme) -> container::Style + 'static {
    move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border,
        },
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
        ..container::Style::default()
    }
}

pub(in crate::app::view) fn wrap_modal<'a>(
    inner: Element<'a, Message>,
    offset: (f32, f32),
    window_size: (f32, f32),
    modal_size: (f32, f32),
) -> Element<'a, Message> {
    // Absolute top-left = centre + drag offset. The Translate widget (see
    // view/translate.rs) passes full parent limits to the child and then
    // translates the child's layout node, so the modal keeps its fixed
    // width/height even when positioned partially off-screen. No clamp is
    // applied here — Altium lets modals drag completely off the client
    // area and the OS window edge is the only hard boundary. If the user
    // drags the modal fully outside the window, they can dismiss it with
    // Escape (see bootstrap key handler).
    let (dx, dy) = offset;
    let (ww, wh) = window_size;
    let (mw, mh) = modal_size;
    let centre_x = (ww - mw) * 0.5;
    let centre_y = (wh - mh) * 0.5;
    let left = centre_x + dx;
    let top = centre_y + dy;

    let backdrop: Element<'a, Message> = container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BACKDROP)),
            ..container::Style::default()
        })
        .into();

    let positioned: Element<'a, Message> =
        super::translate::Translate::new(inner, (left, top)).into();

    iced::widget::stack![backdrop, positioned].into()
}

/// Wrap a header element in a mouse_area so pressing on it begins a modal
/// drag. Uses the last known mouse position as the drag anchor.
pub(in crate::app::view) fn draggable_header<'a>(
    header_content: Element<'a, Message>,
    modal: super::super::states::ModalId,
    last_mouse: (f32, f32),
) -> Element<'a, Message> {
    iced::widget::mouse_area(header_content)
        .on_press(Message::ModalDragStart {
            modal,
            x: last_mouse.0,
            y: last_mouse.1,
        })
        .into()
}

/// Borderless-window header — pressing anywhere on the header region
/// asks iced to start an OS-level window drag. Replaces the OS title
/// bar for detached modals opened with `decorations: false`.
pub(in crate::app::view) fn detached_header<'a>(
    header_content: Element<'a, Message>,
    modal: super::super::states::ModalId,
) -> Element<'a, Message> {
    iced::widget::mouse_area(header_content)
        .on_press(Message::StartDetachedWindowDrag(modal))
        .interaction(iced::mouse::Interaction::Grab)
        .into()
}

/// Compact X close button for borderless modal headers. Matches the
/// main-window chrome close (`view/mod.rs::view_main_window_chrome`):
/// no border, fully transparent at rest, Windows-native red bg + white
/// icon on hover. The `_border` argument is kept for API compatibility
/// with existing call sites — it is intentionally ignored.
pub(in crate::app::view) fn close_x_button(
    message: Message,
    theme_id: signex_types::theme::ThemeId,
    text_color: Color,
) -> Element<'static, Message> {
    // Use the same SVG and footprint as the main-window chrome close
    // (`view::view_main_window_chrome::chrome_btn`) so the modal X is
    // visually identical to the OS-window X, including stroke weight,
    // hit-box dimensions, and red-on-hover behaviour. Glyph is
    // ALWAYS white per design — `text_color` is ignored for the
    // icon. Some themes' text colour was washing the X out against
    // the toolbar bg.
    let _ = text_color;
    use iced::widget::svg;
    let handle = crate::icons::icon_chrome_window_close(theme_id);
    button(
        container(
            svg(handle)
                .width(MODAL_CLOSE_X_ICON)
                .height(MODAL_CLOSE_X_ICON)
                .style(move |_: &Theme, _| svg::Style {
                    color: Some(Color::WHITE),
                }),
        )
        .width(MODAL_CLOSE_X_HIT_W)
        .height(MODAL_CLOSE_X_HIT_H)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(0)
    .on_press(message)
    .style(move |_: &Theme, status: button::Status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        // Top-right radius matches the modal card's outer corner so
        // the red hover background fills the rounded corner cleanly
        // — same trick the OS chrome close uses on Windows 11.
        let radius = iced::border::Radius::default()
            .top_right(crate::styles::MODAL_CORNER_RADIUS);
        button::Style {
            background: if hovered {
                Some(Background::Color(MODAL_CLOSE_X_HOVER))
            } else {
                None
            },
            border: Border {
                radius,
                ..Border::default()
            },
            // Always white — keeps glyph readable on any theme +
            // the destructive red on hover.
            text_color: Color::WHITE,
            ..button::Style::default()
        }
    })
    .into()
}

// `detach_button` was removed once the three big modals started
// opening as separate OS windows by default (see
// `handle_open_annotate_dialog` et al.). Drag-off is no longer needed
// because there is no in-window overlay to drag.

#[allow(dead_code)]
fn close_button(
    label: &str,
    message: Message,
    text_color: Color,
    border: Color,
) -> Element<'_, Message> {
    button(container(text(label.to_string()).size(11).color(text_color)).padding([3, 10]))
        .on_press(message)
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.03))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            text_color,
            ..button::Style::default()
        })
        .into()
}

/// Subtle section divider used inside the BOM modal's properties
/// sidebar. Rendered as a 1-line label on a slightly tinted strip
/// so the panel reads as a stack of named sections.
fn section_header(title: &str, muted: Color) -> Element<'_, Message> {
    container(
        row![
            text("\u{25BE}").size(10).color(muted),
            Space::new().width(6),
            text(title.to_string()).size(11).color(muted),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([6, 12])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.025))),
        ..container::Style::default()
    })
    .into()
}

fn secondary_button(
    label: &str,
    message: Message,
    text_color: Color,
    border: Color,
) -> Element<'_, Message> {
    button(container(text(label.to_string()).size(11).color(text_color)).padding([5, 14]))
        .on_press(message)
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: border,
            },
            text_color,
            ..button::Style::default()
        })
        .into()
}

fn primary_button(label: &str, message: Option<Message>, border: Color) -> Element<'_, Message> {
    primary_button_themed(label, message, border, None)
}

/// Theme-aware primary button. Pass `Some(accent)` to use the
/// theme's accent colour as the button bg (Altium-amber on Signex,
/// cyan on Alp Lab, etc.). Pass `None` to fall back to the legacy
/// hardcoded blue (existing call sites that haven't been migrated).
fn primary_button_themed(
    label: &str,
    message: Option<Message>,
    border: Color,
    accent: Option<Color>,
) -> Element<'_, Message> {
    let enabled = message.is_some();
    let active_bg = accent.unwrap_or(Color::from_rgb(0.00, 0.47, 0.84));
    let bg = if enabled {
        active_bg
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    };
    let fg = if enabled {
        Color::WHITE
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.4)
    };
    let mut b = button(container(text(label.to_string()).size(11).color(fg)).padding([5, 14]))
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: border,
            },
            text_color: fg,
            ..button::Style::default()
        });
    if let Some(msg) = message {
        b = b.on_press(msg);
    }
    b.into()
}

fn order_radio(
    label: &str,
    value: AnnotateOrder,
    current: AnnotateOrder,
    text_c: Color,
    border: Color,
) -> Element<'_, Message> {
    let selected = value == current;
    let bg = if selected {
        Color::from_rgb(0.00, 0.47, 0.84)
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    };
    let fg = if selected { Color::WHITE } else { text_c };
    button(container(text(label.to_string()).size(11).color(fg)).padding([4, 10]))
        .on_press(Message::AnnotateOrderChanged(value))
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            text_color: fg,
            ..button::Style::default()
        })
        .into()
}

fn severity_segmented(
    rule: signex_erc::RuleKind,
    current: signex_erc::Severity,
    border: Color,
    text_c: Color,
    text_muted: Color,
) -> Element<'static, Message> {
    let seg = |label: &'static str,
               value: signex_erc::Severity,
               color: Color|
     -> Element<'static, Message> {
        let selected = value == current;
        let bg = if selected {
            color
        } else {
            Color::from_rgba(1.0, 1.0, 1.0, 0.03)
        };
        let fg = if selected { Color::WHITE } else { text_muted };
        button(container(text(label.to_string()).size(10).color(fg)).padding([3, 8]))
            .on_press(Message::ErcSeverityChanged(rule, value))
            .style(move |_: &Theme, _| button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                text_color: fg,
                ..button::Style::default()
            })
            .into()
    };
    let _ = text_c;
    row![
        seg(
            "E",
            signex_erc::Severity::Error,
            Color::from_rgb(0.85, 0.25, 0.25)
        ),
        seg(
            "W",
            signex_erc::Severity::Warning,
            Color::from_rgb(0.95, 0.70, 0.15)
        ),
        seg(
            "I",
            signex_erc::Severity::Info,
            Color::from_rgb(0.30, 0.60, 0.95)
        ),
        seg(
            "Off",
            signex_erc::Severity::Off,
            Color::from_rgb(0.35, 0.36, 0.42)
        ),
    ]
    .spacing(2)
    .into()
}

const ALL_RULES: &[signex_erc::RuleKind] = &[
    signex_erc::RuleKind::UnusedPin,
    signex_erc::RuleKind::DuplicateRefDesignator,
    signex_erc::RuleKind::HierPortDisconnected,
    signex_erc::RuleKind::DanglingWire,
    signex_erc::RuleKind::NetLabelConflict,
    signex_erc::RuleKind::OrphanLabel,
    signex_erc::RuleKind::BusBitWidthMismatch,
    signex_erc::RuleKind::BadHierSheetPin,
    signex_erc::RuleKind::MissingPowerFlag,
    signex_erc::RuleKind::PowerPortShort,
    signex_erc::RuleKind::SymbolOutsideSheet,
];

/// Compute the proposed (current, new) reference designator pairs that would
/// result from running Annotate Incremental on the active snapshot.
/// Preserves the engine's ordering logic (by y,x,uuid). The dialog
/// currently only offers one order because the engine hard-codes that;
/// order-flag wiring is a v0.7.1 follow-up.
/// One row of the project-wide proposed change list.
#[derive(Debug, Clone)]
pub(super) struct AnnotatePreviewEntry {
    pub sheet: String,
    pub current: String,
    pub proposed: String,
    /// Symbol uuid — lets the row's lock checkbox toggle the global
    /// `ui_state.annotate.locked` set without re-looking-up the symbol.
    pub uuid: uuid::Uuid,
}

impl super::super::Signex {
    /// Walk every schematic in the project — open tabs (live engine or
    /// cached session) plus every sheet listed in project_data.sheets that
    /// hasn't been opened yet. Unopened sheets are parsed on-the-fly so
    /// the change list reflects the whole project, not just what the user
    /// has active.
    pub(super) fn preview_project_annotations(&self) -> Vec<AnnotatePreviewEntry> {
        let is_target = |sym: &signex_types::schematic::Symbol| -> bool {
            !sym.is_power && !sym.reference.starts_with('#')
        };

        // Owned sheets (parsed from disk) are boxed so we can hold them in
        // the same vector as the borrowed ones and still use slice APIs.
        let mut owned_sheets: Vec<(String, signex_types::schematic::SchematicSheet)> = Vec::new();
        let mut open_paths: std::collections::HashSet<std::path::PathBuf> =
            std::collections::HashSet::new();

        // Pass 1: collect open tabs.
        let mut borrowed: Vec<(String, &signex_types::schematic::SchematicSheet)> = Vec::new();
        for tab in self.document_state.tabs.iter() {
            open_paths.insert(tab.path.clone());
            if let Some(engine) = self.document_state.engines.get(&tab.path) {
                borrowed.push((tab.title.clone(), engine.document()));
            }
        }
        // Fallback when no tabs are open but an engine still holds a doc.
        if borrowed.is_empty()
            && open_paths.is_empty()
            && let Some(eng) = self.document_state.active_engine()
        {
            borrowed.push(("(untitled)".to_string(), eng.document()));
        }

        // Pass 2: parse every remaining project sheet from disk so the
        // change list spans sheets the user hasn't opened yet.
        if let Some(loaded) = self.document_state.active_loaded_project() {
            let project_dir = loaded
                .path
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_default();
            for sheet_entry in &loaded.data.sheets {
                let file_path = project_dir.join(&sheet_entry.filename);
                if open_paths.contains(&file_path) {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&file_path)
                    && let Ok(parsed) =
                        signex_types::format::SnxSchematic::parse(&text).map(|snx| snx.sheet)
                {
                    let title = sheet_entry.name.trim_end_matches(".snxsch").to_string();
                    owned_sheets.push((title, parsed));
                }
            }
        }

        // Merge into a single Vec of borrowed references.
        let mut sheets: Vec<(String, &signex_types::schematic::SchematicSheet)> = borrowed;
        for (title, sheet) in &owned_sheets {
            sheets.push((title.clone(), sheet));
        }

        // Pass 1: global max per prefix.
        let mut next: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        for (_, sheet) in &sheets {
            for sym in &sheet.symbols {
                if !is_target(sym) {
                    continue;
                }
                let prefix: String = sym
                    .reference
                    .chars()
                    .take_while(|c| c.is_ascii_alphabetic())
                    .collect();
                if prefix.is_empty() {
                    continue;
                }
                if let Ok(n) = sym.reference[prefix.len()..].parse::<u32>() {
                    let e = next.entry(prefix).or_insert(0);
                    if n > *e {
                        *e = n;
                    }
                }
            }
        }

        // Pass 2: iterate sheets, assigning proposed designators to '?' tails.
        let mut out = Vec::new();
        for (title, sheet) in &sheets {
            let mut idx: Vec<usize> = (0..sheet.symbols.len()).collect();
            idx.sort_by(|a, b| {
                let sa = &sheet.symbols[*a];
                let sb = &sheet.symbols[*b];
                sa.position
                    .y
                    .partial_cmp(&sb.position.y)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(
                        sa.position
                            .x
                            .partial_cmp(&sb.position.x)
                            .unwrap_or(std::cmp::Ordering::Equal),
                    )
                    .then(sa.uuid.cmp(&sb.uuid))
            });
            for i in idx {
                let sym = &sheet.symbols[i];
                if sym.reference.is_empty() || !is_target(sym) {
                    continue;
                }
                let prefix: String = sym
                    .reference
                    .chars()
                    .take_while(|c| c.is_ascii_alphabetic())
                    .collect();
                let proposed = if sym.reference.ends_with('?') && !prefix.is_empty() {
                    let n = next.entry(prefix.clone()).or_insert(0);
                    *n += 1;
                    format!("{prefix}{n}")
                } else {
                    sym.reference.clone()
                };
                out.push(AnnotatePreviewEntry {
                    sheet: title.clone(),
                    current: sym.reference.clone(),
                    proposed,
                    uuid: sym.uuid,
                });
            }
        }
        out
    }
}

#[allow(dead_code)]
fn preview_annotations(
    snapshot: &signex_render::schematic::SchematicRenderSnapshot,
    _order: AnnotateOrder,
) -> Vec<(String, String)> {
    // Power ports (#PWR, #FLG, `is_power`) aren't designators — they're
    // net anchors whose "reference" is the net name. Skip them from the
    // annotate preview and the change list entirely.
    let is_designator_target = |sym: &signex_types::schematic::Symbol| -> bool {
        !sym.is_power && !sym.reference.starts_with('#')
    };
    // Collect existing per-prefix counters.
    let mut next: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for sym in &snapshot.symbols {
        if !is_designator_target(sym) {
            continue;
        }
        let prefix: String = sym
            .reference
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if prefix.is_empty() {
            continue;
        }
        if let Ok(n) = sym.reference[prefix.len()..].parse::<u32>() {
            let e = next.entry(prefix).or_insert(0);
            if n > *e {
                *e = n;
            }
        }
    }
    // Order: y ascending, then x ascending (matches the engine).
    let mut idx: Vec<usize> = (0..snapshot.symbols.len()).collect();
    idx.sort_by(|a, b| {
        let sa = &snapshot.symbols[*a];
        let sb = &snapshot.symbols[*b];
        sa.position
            .y
            .partial_cmp(&sb.position.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                sa.position
                    .x
                    .partial_cmp(&sb.position.x)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(sa.uuid.cmp(&sb.uuid))
    });
    // Emit a row for every symbol so the user sees the full project — rows
    // where current == proposed indicate "no change". Only symbols whose
    // reference ends in '?' will actually be renumbered.
    let mut out = Vec::new();
    for i in idx {
        let sym = &snapshot.symbols[i];
        if sym.reference.is_empty() || !is_designator_target(sym) {
            continue;
        }
        let prefix: String = sym
            .reference
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if sym.reference.ends_with('?') && !prefix.is_empty() {
            let n = next.entry(prefix.clone()).or_insert(0);
            *n += 1;
            out.push((sym.reference.clone(), format!("{prefix}{n}")));
        } else {
            // Already annotated — propose keeping the same designator.
            out.push((sym.reference.clone(), sym.reference.clone()));
        }
    }
    out
}

/// Pin-connection matrix. Click a cell to cycle Error → Warning →
/// Info → Off → baseline. Overrides persist via
/// `write_pin_matrix_overrides`. Currently 6×6 primary pin types;
/// the full Altium 12-type grid lands when the rule engine learns
/// the remaining variants (Open Collector, Open Emitter, HiZ, …).
fn pin_matrix_view(
    tokens: &signex_types::theme::ThemeTokens,
    overrides: &std::collections::HashMap<(u8, u8), signex_erc::Severity>,
) -> Element<'static, Message> {
    let text_c = crate::styles::ti(tokens.text);
    let text_muted = crate::styles::ti(tokens.text_secondary);
    let border = crate::styles::ti(tokens.border);

    // 6 primary pin types. Full 12-type Altium matrix lands when
    // pin-type taxonomy is extended in rules.rs (Open Collector, Open
    // Emitter, HiZ, Unspecified, etc.).
    const TYPES: &[&str] = &["Input", "Output", "Bidir", "PowerIn", "PowerOut", "NC"];
    // Same baseline as the PinMatrixCellCycled handler — keep in sync.
    use signex_erc::Severity;
    #[rustfmt::skip]
    const BASELINE: [[Severity; 6]; 6] = [
        [Severity::Off, Severity::Off,     Severity::Off,     Severity::Off,   Severity::Off,     Severity::Off],
        [Severity::Off, Severity::Error,   Severity::Off,     Severity::Off,   Severity::Error,   Severity::Error],
        [Severity::Off, Severity::Off,     Severity::Off,     Severity::Off,   Severity::Off,     Severity::Warning],
        [Severity::Off, Severity::Off,     Severity::Off,     Severity::Off,   Severity::Off,     Severity::Error],
        [Severity::Off, Severity::Error,   Severity::Off,     Severity::Off,   Severity::Error,   Severity::Error],
        [Severity::Off, Severity::Error,   Severity::Warning, Severity::Error, Severity::Error,   Severity::Off],
    ];

    let cell = |r: u8, c: u8, sev: Severity| -> Element<'static, Message> {
        let (bg, ch) = match sev {
            Severity::Error => (Color::from_rgba(0.85, 0.25, 0.25, 0.55), "E"),
            Severity::Warning => (Color::from_rgba(0.95, 0.70, 0.15, 0.55), "W"),
            Severity::Info => (Color::from_rgba(0.30, 0.55, 0.85, 0.55), "I"),
            Severity::Off => (Color::from_rgba(0.25, 0.70, 0.30, 0.35), "\u{00B7}"),
        };
        button(
            container(text(ch.to_string()).size(12).color(text_c))
                .width(40)
                .height(30)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center),
        )
        .on_press(Message::PinMatrixCellCycled { row: r, col: c })
        .padding(0)
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 0.0.into(),
                color: border,
            },
            text_color: text_c,
            ..button::Style::default()
        })
        .into()
    };

    let header_label = |label: &str| -> Element<'static, Message> {
        container(text(label.to_string()).size(10).color(text_muted))
            .width(80)
            .height(30)
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Center)
            .padding([0, 6])
            .into()
    };
    let col_label = |label: &str| -> Element<'static, Message> {
        container(text(label.to_string()).size(10).color(text_muted))
            .width(40)
            .height(24)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into()
    };

    let mut header = row![container(Space::new().width(80)).width(80)].spacing(0);
    for t in TYPES {
        header = header.push(col_label(t));
    }

    let mut body = column![header].spacing(0);
    for (r, row_label) in TYPES.iter().enumerate() {
        let mut rr = row![header_label(row_label)].spacing(0);
        for (c, &baseline_sev) in BASELINE[r].iter().enumerate().take(TYPES.len()) {
            let sev = overrides
                .get(&(r as u8, c as u8))
                .copied()
                .unwrap_or(baseline_sev);
            rr = rr.push(cell(r as u8, c as u8, sev));
        }
        body = body.push(rr);
    }

    container(body).into()
}
