//! Altium-style modal dialogs for Design menu commands.
//!
//! Three modals live here:
//! - Annotate Schematics — order pick-list + preview of proposed changes
//! - ERC — per-rule severity override grid + pin-connection matrix
//! - Reset-Annotations confirm — simple Yes/No

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::app::state::AnnotateOrder;
use crate::app::{Message, Signex};

const BACKDROP: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.55);

impl Signex {
    pub(super) fn view_annotate_dialog(&self) -> Element<'_, Message> {
        let modal_w = 1100.0_f32;
        let modal_h = 760.0_f32;
        let dialog = self.view_annotate_dialog_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::state::ModalId::AnnotateDialog)
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

    fn view_annotate_dialog_body_inner(
        &self,
        draggable: bool,
    ) -> Element<'_, Message> {
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
        let header_content: Element<'_, Message> = container(
            row![
                text("Annotate").size(14).color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::CloseAnnotateDialog, text_muted, border_c),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([10, 14])
        .style(crate::styles::toolbar_strip(tokens))
        .into();
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::state::ModalId::AnnotateDialog,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            detached_header(header_content, super::super::state::ModalId::AnnotateDialog)
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
                            self.ui_state.annotate_order,
                            text_c,
                            border_c,
                        ),
                        Space::new().width(4),
                        order_radio(
                            "Across Then Down",
                            AnnotateOrder::AcrossThenDown,
                            self.ui_state.annotate_order,
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
                            self.ui_state.annotate_order,
                            text_c,
                            border_c,
                        ),
                        Space::new().width(4),
                        order_radio(
                            "Across Then Up",
                            AnnotateOrder::AcrossThenUp,
                            self.ui_state.annotate_order,
                            text_c,
                            border_c,
                        ),
                    ]
                    .spacing(0),
                ]
                .spacing(0)
                .width(Length::FillPortion(3)),
                Space::new().width(8),
                order_preview(self.ui_state.annotate_order, text_c, text_muted, border_c),
            ]
            .align_y(iced::Alignment::Start)
            .spacing(0),
        ]
        .spacing(4);

        let process_location = column![
            text("Process Location of").size(10).color(text_muted),
            container(
                text("Designator")
                    .size(11)
                    .color(text_c),
            )
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
            let mut set: std::collections::BTreeSet<String> =
                std::collections::BTreeSet::new();
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
                    text("All").size(11).color(row_color).width(Length::FillPortion(1)),
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
                        text(suffix_placeholder.to_string()).size(11).color(row_color),
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
                container(
                    text("No schematic tab open.").size(11).color(text_muted),
                )
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
            text("Lock").size(10).color(text_muted).width(Length::Fixed(42.0)),
            text("Current").size(10).color(text_muted).width(Length::FillPortion(2)),
            text("Proposed").size(10).color(text_muted).width(Length::FillPortion(2)),
            text("Location").size(10).color(text_muted).width(Length::FillPortion(3)),
        ]
        .padding([4, 8]);

        let locked_set = &self.ui_state.annotate_locked;
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
        let changes: usize = proposed
            .iter()
            .filter(|e| e.current != e.proposed)
            .count();
        let summary_text = if changes == 0 {
            format!(
                "Annotation has nothing to do — all {total_symbols} symbols on '{current_sheet_name}' already carry a designator. Use Reset All or Reset & Renumber below to renumber from scratch."
            )
        } else {
            format!(
                "Annotation will assign new designators to {changes} of {total_symbols} symbols on '{current_sheet_name}'. Click Accept Changes to apply, Update Changes List to recompute, or Reset All to clear every number first.",
            )
        };
        let summary = container(
            text(summary_text).size(10).color(text_muted),
        )
        .padding([4, 4]);

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
            container(
                iced::widget::Space::new().width(1).height(Length::Fill),
            )
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(border_c)),
                ..container::Style::default()
            }),
            container(right_col).padding([14, 14]),
        ]
        .spacing(0);

        let modal_w = 1100.0_f32;
        let modal_h = 760.0_f32;
        let dialog = container(
            column![
                header,
                body_row,
                container(footer).padding([10, 14]),
            ]
            .width(modal_w)
            .height(modal_h),
        )
        .style(crate::styles::context_menu(tokens));

        dialog.into()
    }

    pub(super) fn view_annotate_reset_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_annotate_reset_confirm_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::state::ModalId::AnnotateResetConfirm)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (420.0, 180.0))
    }

    pub(super) fn view_annotate_reset_confirm_body(&self) -> Element<'_, Message> {
        self.view_annotate_reset_confirm_body_inner(false)
    }

    fn view_annotate_reset_confirm_body_inner(
        &self,
        draggable: bool,
    ) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header_content: Element<'_, Message> = container(
            row![
                text("Reset All Annotations").size(14).color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::CloseAnnotateResetConfirm,
                    text_muted,
                    border_c,
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([10, 14])
        .style(crate::styles::toolbar_strip(tokens))
        .into();
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::state::ModalId::AnnotateResetConfirm,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            detached_header(
                header_content,
                super::super::state::ModalId::AnnotateResetConfirm,
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
        .style(crate::styles::context_menu(tokens));
        dialog.into()
    }

    pub(super) fn view_erc_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_erc_dialog_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::state::ModalId::ErcDialog)
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

        let header_content: Element<'_, Message> = container(
            row![
                text("Electrical Rules Check").size(14).color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::CloseErcDialog, text_muted, border_c),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([10, 14])
        .style(crate::styles::toolbar_strip(tokens))
        .into();
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::state::ModalId::ErcDialog,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            detached_header(
                header_content,
                super::super::state::ModalId::ErcDialog,
            )
        };

        // Per-rule severity grid. 11 rules × 4 severities.
        let mut rule_rows = column![
            row![
                text("Rule").size(10).color(text_muted).width(Length::FillPortion(3)),
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
                .erc_severity_override
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
            text("Severity per rule")
                .size(11)
                .color(text_c),
            text("Click a cell to change.")
                .size(10)
                .color(text_muted),
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
            text("Pin Connection Matrix")
                .size(11)
                .color(text_c),
            text("Click a cell to cycle severity.")
                .size(10)
                .color(text_muted),
            Space::new().height(8),
            container(pin_matrix_view(tokens, &self.ui_state.pin_matrix_overrides))
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

        let body = row![
            rules_pane,
            Space::new().width(16),
            matrix_pane,
        ]
        .align_y(iced::Alignment::Start);

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
        .style(crate::styles::context_menu(tokens));
        dialog.into()
    }
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// Tiny inline checkbox pip — read-only indicator used inside the Annotate
/// dialog's parameter list and sheet table. The boxes are visual only for
/// v0.7; v0.7.1 makes them interactive.
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

fn wrap_modal<'a>(
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
fn draggable_header<'a>(
    header_content: Element<'a, Message>,
    modal: super::super::state::ModalId,
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
fn detached_header<'a>(
    header_content: Element<'a, Message>,
    modal: super::super::state::ModalId,
) -> Element<'a, Message> {
    iced::widget::mouse_area(header_content)
        .on_press(Message::StartDetachedWindowDrag(modal))
        .interaction(iced::mouse::Interaction::Grab)
        .into()
}

/// Compact X close button for borderless modal headers.
fn close_x_button(
    message: Message,
    text_color: Color,
    border: Color,
) -> Element<'static, Message> {
    button(
        container(text("\u{00D7}".to_string()).size(14).color(text_color))
            .padding([0, 6]),
    )
    .on_press(message)
    .style(move |_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => {
                Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.1)))
            }
            _ => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.03))),
        };
        button::Style {
            background: bg,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            text_color,
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

fn primary_button(
    label: &str,
    message: Option<Message>,
    border: Color,
) -> Element<'_, Message> {
    let enabled = message.is_some();
    let bg = if enabled {
        Color::from_rgb(0.00, 0.47, 0.84)
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
    button(
        container(text(label.to_string()).size(11).color(fg)).padding([4, 10]),
    )
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
    let seg = |label: &'static str, value: signex_erc::Severity, color: Color| -> Element<
        'static,
        Message,
    > {
        let selected = value == current;
        let bg = if selected { color } else { Color::from_rgba(1.0, 1.0, 1.0, 0.03) };
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
        seg("E", signex_erc::Severity::Error, Color::from_rgb(0.85, 0.25, 0.25)),
        seg("W", signex_erc::Severity::Warning, Color::from_rgb(0.95, 0.70, 0.15)),
        seg("I", signex_erc::Severity::Info, Color::from_rgb(0.30, 0.60, 0.95)),
        seg("Off", signex_erc::Severity::Off, Color::from_rgb(0.35, 0.36, 0.42)),
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
/// result from running Annotate Incremental on the active snapshot. Preserves
/// the engine's ordering logic (by y,x,uuid) — but the dialog currently only
/// offers one order because the engine hard-codes that; adding UpThenAcross
/// wiring ships in v0.7.1 when the engine learns about the order flag.
/// One row of the project-wide proposed change list.
#[derive(Debug, Clone)]
pub(super) struct AnnotatePreviewEntry {
    pub sheet: String,
    pub current: String,
    pub proposed: String,
    /// Symbol uuid — lets the row's lock checkbox toggle the global
    /// `ui_state.annotate_locked` set without re-looking-up the symbol.
    pub uuid: uuid::Uuid,
}

impl super::super::Signex {
    /// Walk every schematic in the project — open tabs (live engine or
    /// cached session) plus every sheet listed in project_data.sheets that
    /// hasn't been opened yet. Unopened sheets are parsed on-the-fly so
    /// the change list reflects the whole project, not just what the user
    /// has active.
    pub(super) fn preview_project_annotations(&self) -> Vec<AnnotatePreviewEntry> {
        use crate::app::documents::TabDocument;
        let is_target = |sym: &signex_types::schematic::Symbol| -> bool {
            !sym.is_power && !sym.reference.starts_with('#')
        };

        // Owned sheets (parsed from disk) are boxed so we can hold them in
        // the same vector as the borrowed ones and still use slice APIs.
        let mut owned_sheets: Vec<(String, signex_types::schematic::SchematicSheet)> =
            Vec::new();
        let mut open_paths: std::collections::HashSet<std::path::PathBuf> =
            std::collections::HashSet::new();

        // Pass 1: collect open tabs.
        let mut borrowed: Vec<(String, &signex_types::schematic::SchematicSheet)> =
            Vec::new();
        for (idx, tab) in self.document_state.tabs.iter().enumerate() {
            open_paths.insert(tab.path.clone());
            if idx == self.document_state.active_tab {
                if let Some(eng) = self.document_state.engine.as_ref() {
                    borrowed.push((tab.title.clone(), eng.document()));
                }
            } else if let Some(TabDocument::Schematic(session)) = tab.cached_document.as_ref() {
                borrowed.push((tab.title.clone(), session.document()));
            }
        }
        // Fallback when no tabs are open but an engine still holds a doc.
        if borrowed.is_empty() && open_paths.is_empty() {
            if let Some(eng) = self.document_state.engine.as_ref() {
                borrowed.push(("(untitled)".to_string(), eng.document()));
            }
        }

        // Pass 2: parse every remaining project sheet from disk so the
        // change list spans sheets the user hasn't opened yet.
        if let (Some(project), Some(project_path)) = (
            self.document_state.project_data.as_ref(),
            self.document_state.project_path.as_ref(),
        ) {
            let project_dir = project_path
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_default();
            for sheet_entry in &project.sheets {
                let file_path = project_dir.join(&sheet_entry.filename);
                if open_paths.contains(&file_path) {
                    continue;
                }
                if let Ok(parsed) = kicad_parser::parse_schematic_file(&file_path) {
                    let title = sheet_entry
                        .name
                        .trim_end_matches(".kicad_sch")
                        .to_string();
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
        let mut next: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
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
    let mut next: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
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
    const TYPES: &[&str] = &[
        "Input", "Output", "Bidir", "PowerIn", "PowerOut", "NC",
    ];
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
        for c in 0..TYPES.len() {
            let sev = overrides
                .get(&(r as u8, c as u8))
                .copied()
                .unwrap_or(BASELINE[r][c]);
            rr = rr.push(cell(r as u8, c as u8, sev));
        }
        body = body.push(rr);
    }

    container(body).into()
}
