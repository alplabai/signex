//! Annotate-Schematics modal — order pick-list + preview of proposed
//! changes, plus the reset-annotations confirm.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::app::state::AnnotateOrder;

use super::widgets::{
    close_x_button, draggable_header, fit_modal_size, os_title_bar_slot, primary_button,
    secondary_button, secondary_button_maybe, wrap_modal,
};
use super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};

mod controls;
mod reprefix_confirm;

use controls::{bordered_style, check_pip, order_preview, order_radio};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    pub(in crate::app::view) fn view_annotate_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_annotate_dialog_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::super::state::ModalId::AnnotateDialog)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(
            dialog,
            offset,
            self.ui_state.window_size,
            self.annotate_modal_size(),
        )
    }

    /// The in-window Annotate modal: 1100 x 760 where the window allows,
    /// capped to the window otherwise (#1227).
    pub(in crate::app::view) fn annotate_modal_size(&self) -> (f32, f32) {
        fit_modal_size(self.ui_state.window_size, (1100.0, 760.0))
    }

    /// Detached-window flavour — just the body, no backdrop, no drag
    /// handler on the header (the OS window chrome owns that).
    pub(in crate::app::view) fn view_annotate_dialog_body(&self) -> Element<'_, Message> {
        self.view_annotate_dialog_body_inner(false)
    }

    fn view_annotate_dialog_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        // The project's sheets, from the one assembler — the same set
        // `handle_annotate` acts on, so the dialog and the operation cannot
        // describe different sheets (#406).
        //
        // The rows come from `ui_state.annotate_preview`, loaded off-thread
        // and re-fetched whenever the open documents' content fingerprint
        // changes (see `handlers/erc/annotate_preview.rs`), so a cached
        // preview cannot disagree with the action it previews (#111).
        let proposed = &self.ui_state.annotate_preview.entries;
        let total_symbols: usize = proposed.len();
        let current_sheet_name = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|t| t.title.clone())
            .unwrap_or_else(|| "Current Sheet".to_string());

        // ── Header (draggable when in-window; OS-window-drag when
        // detached) ──
        let theme_id = self.ui_state.effective_theme_id();
        let header_content: Element<'_, Message> = container(
            row![
                text("Annotate").size(MODAL_HEADER_TITLE_SIZE).color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::Annotate(AnnotateMsg::CloseDialog),
                    theme_id,
                    text_muted
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::super::state::ModalId::AnnotateDialog,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };

        // ── Left column: Schematic Annotation Configuration ──
        let left_title = text("Schematic Annotation Configuration")
            .size(TS.label)
            .color(text_c);

        let order_row = column![
            text("Order of Processing")
                .size(TS.caption)
                .color(text_muted),
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
        .spacing(SP.s);

        let process_location = column![
            text("Process Location of")
                .size(TS.caption)
                .color(text_muted),
            container(text("Designator").size(TS.body).color(text_c),)
                .padding([5, 10])
                .width(Length::Fill)
                .style(bordered_style(border_c)),
        ]
        .spacing(SP.s);

        let replace_sub = column![
            text("Replace Sub-Parts").size(TS.caption).color(text_muted),
            container(text("Off").size(TS.body).color(text_muted))
                .padding([5, 10])
                .width(Length::Fill)
                .style(bordered_style(border_c)),
        ]
        .spacing(SP.s);

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
                    .size(TS.caption)
                    .color(text_muted)
                    .width(Length::FillPortion(5)),
                text("Strictly")
                    .size(TS.caption)
                    .color(text_muted)
                    .width(Length::FillPortion(1)),
            ]
            .padding([SP.s, SP.m]),
        ]
        .spacing(0);
        for name in &param_names {
            param_rows = param_rows.push(
                row![
                    row![
                        check_pip(false, border_c),
                        Space::new().width(6),
                        text(name.clone()).size(TS.body).color(text_c),
                    ]
                    .align_y(iced::Alignment::Center)
                    .width(Length::FillPortion(5)),
                    container(check_pip(true, border_c))
                        .align_x(iced::alignment::Horizontal::Right)
                        .width(Length::FillPortion(1)),
                ]
                .padding([3.0, SP.m])
                .align_y(iced::Alignment::Center),
            );
        }
        if param_names.is_empty() {
            param_rows = param_rows.push(
                container(
                    text("No custom parameters on any symbol yet.")
                        .size(TS.body)
                        .color(text_muted),
                )
                .padding([10.0, SP.m]),
            );
        }

        let matching = column![
            text("Matching Options").size(TS.label).color(text_c),
            Space::new().height(4),
            row![
                text("Complete Existing Packages")
                    .size(TS.caption)
                    .color(text_muted)
                    .width(Length::FillPortion(2)),
                container(text("None").size(TS.body).color(text_muted))
                    .padding([SP.s, SP.m])
                    .width(Length::FillPortion(3))
                    .style(bordered_style(border_c)),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(6),
            Space::new().height(6),
            container(scrollable(param_rows).height(280))
                .padding(SP.xs)
                .style(bordered_style(border_c)),
            Space::new().height(4),
            text("Parameter matching is visual grouping only — the annotation engine ignores these checkboxes for now.")
                .size(TS.caption)
                .color(text_muted),
        ]
        .spacing(SP.s);

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
                        text(tab.title.clone()).size(TS.body).color(row_color),
                    ]
                    .align_y(iced::Alignment::Center)
                    .width(Length::FillPortion(6)),
                    text("All")
                        .size(TS.body)
                        .color(row_color)
                        .width(Length::FillPortion(1)),
                    text(format!("{idx}"))
                        .size(TS.body)
                        .color(row_color)
                        .width(Length::FillPortion(1)),
                    text(format!("{}", idx + 1))
                        .size(TS.body)
                        .color(row_color)
                        .width(Length::FillPortion(1)),
                    row![
                        check_pip(false, border_c),
                        Space::new().width(6),
                        text(suffix_placeholder.to_string())
                            .size(TS.body)
                            .color(row_color),
                    ]
                    .align_y(iced::Alignment::Center)
                    .width(Length::FillPortion(2)),
                ]
                .padding([3.0, SP.m])
                .align_y(iced::Alignment::Center),
            );
        }
        if self.document_state.tabs.is_empty() {
            sheet_rows = sheet_rows.push(
                container(
                    text("No schematic tab open.")
                        .size(TS.body)
                        .color(text_muted),
                )
                .padding([6.0, SP.m]),
            );
        }

        let sheets_table = column![
            text("Schematic Sheets To Annotate").size(TS.label).color(text_c),
            Space::new().height(4),
            row![
                text("Schematic Sheet")
                    .size(TS.caption)
                    .color(text_muted)
                    .width(Length::FillPortion(6)),
                text("Scope")
                    .size(TS.caption)
                    .color(text_muted)
                    .width(Length::FillPortion(1)),
                text("Ord...")
                    .size(TS.caption)
                    .color(text_muted)
                    .width(Length::FillPortion(1)),
                text("Start")
                    .size(TS.caption)
                    .color(text_muted)
                    .width(Length::FillPortion(1)),
                text("Add Suffix")
                    .size(TS.caption)
                    .color(text_muted)
                    .width(Length::FillPortion(2)),
            ]
            .padding([SP.xs, SP.m]),
            container(scrollable(sheet_rows).height(180))
                .style(bordered_style(border_c)),
            Space::new().height(4),
            text("Scope / Start / Suffix columns are visual for v0.7 — multi-sheet wiring lands in v1.1.")
                .size(TS.caption)
                .color(text_muted),
        ]
        .spacing(SP.s);

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
        let right_title = text("Proposed Change List").size(TS.label).color(text_c);

        // Column headers
        let list_headers = row![
            text("Lock")
                .size(TS.caption)
                .color(text_muted)
                .width(Length::Fixed(42.0)),
            text("Current")
                .size(TS.caption)
                .color(text_muted)
                .width(Length::FillPortion(2)),
            text("Proposed")
                .size(TS.caption)
                .color(text_muted)
                .width(Length::FillPortion(2)),
            text("Location")
                .size(TS.caption)
                .color(text_muted)
                .width(Length::FillPortion(3)),
        ]
        .padding([SP.s, SP.m]);

        let locked_set = &self.ui_state.annotate_locked;
        let mut rows_col: iced::widget::Column<'_, Message> =
            column![].spacing(0).width(Length::Fill);
        if proposed.is_empty() {
            rows_col = rows_col.push(
                container(
                    text(if self.ui_state.annotate_preview.loading {
                        "Loading project sheets…"
                    } else if self.ui_state.annotate_preview.load_failed {
                        "Could not load the project's sheets - see the Messages panel."
                    } else {
                        "No symbols need annotation — every reference has a number."
                    })
                    .size(TS.body)
                    .color(text_muted),
                )
                .padding([10.0, SP.l]),
            );
        } else {
            for entry in proposed {
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
                    container(text(lock_tick.to_string()).size(TS.body).color(text_c))
                        .width(22)
                        .height(16)
                        .align_x(iced::alignment::Horizontal::Center)
                        .align_y(iced::alignment::Vertical::Center),
                )
                .on_press(Message::Annotate(AnnotateMsg::ToggleLock(uuid)))
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
                            .size(TS.body)
                            .color(cur_color)
                            .width(Length::FillPortion(2)),
                        text(proposed_display)
                            .size(TS.body)
                            .color(new_color)
                            .width(Length::FillPortion(2)),
                        text(entry.sheet.clone())
                            .size(TS.body)
                            .color(text_muted)
                            .width(Length::FillPortion(3)),
                    ]
                    .align_y(iced::Alignment::Center)
                    .padding([SP.xs, SP.m]),
                );
            }
        }

        let right_list = container(scrollable(rows_col).height(Length::Fill))
            .height(Length::Fill)
            .padding(SP.s)
            .style(bordered_style(border_c));

        // Count only rows where proposed != current (actual changes).
        let changes: usize = proposed.iter().filter(|e| e.current != e.proposed).count();
        let summary_text = if changes == 0 {
            format!(
                "Annotation has nothing to do — all {total_symbols} symbols on '{current_sheet_name}' already carry a designator. Use Reset All or Reset & Renumber below to renumber from scratch."
            )
        } else {
            format!(
                "Annotation will assign new designators to {changes} of {total_symbols} symbols on '{current_sheet_name}'. The list updates live. Click Accept Changes to apply, or Reset All to clear every number first.",
            )
        };
        let summary =
            container(text(summary_text).size(TS.caption).color(text_muted)).padding([SP.s, SP.s]);
        let standard = self
            .document_state
            .active_document_project()
            .and_then(|p| p.data.designator_standard());
        let reprefix_note = controls::reprefix_note(standard);

        let right_col = column![
            right_title,
            Space::new().height(6),
            list_headers,
            right_list,
            Space::new().height(6),
            text("Annotation Summary").size(TS.caption).color(text_c),
            summary,
            container(text(reprefix_note).size(TS.caption).color(text_muted)).padding([SP.s, SP.s]),
        ]
        .spacing(0)
        .width(Length::FillPortion(11))
        .height(Length::Fill);

        // ── Footer ──
        let footer = row![
            secondary_button(
                "All On",
                Message::Annotate(AnnotateMsg::SetAllLocked(false)),
                text_c,
                border_c,
            ),
            Space::new().width(4),
            secondary_button(
                "All Off",
                Message::Annotate(AnnotateMsg::SetAllLocked(true)),
                text_c,
                border_c,
            ),
            Space::new().width(Length::Fill),
            secondary_button_maybe(
                "Re-prefix to standard",
                (standard.is_some() && !self.ui_state.annotate_preview.loading)
                    .then_some(Message::Annotate(AnnotateMsg::ReprefixPressed)),
                text_c,
                border_c,
            ),
            Space::new().width(4),
            secondary_button(
                "Reset All",
                Message::Annotate(AnnotateMsg::Run(signex_engine::AnnotateMode::ResetOnly)),
                text_c,
                border_c,
            ),
            Space::new().width(4),
            secondary_button(
                "Reset & Renumber",
                Message::Annotate(AnnotateMsg::Run(
                    signex_engine::AnnotateMode::ResetAndRenumber,
                )),
                text_c,
                border_c,
            ),
            Space::new().width(4),
            primary_button(
                "Accept Changes",
                if changes == 0 || self.ui_state.annotate_preview.loading {
                    None
                } else {
                    Some(Message::Annotate(AnnotateMsg::Run(
                        signex_engine::AnnotateMode::Incremental,
                    )))
                },
                border_c,
            ),
            Space::new().width(4),
            secondary_button(
                "Close",
                Message::Annotate(AnnotateMsg::CloseDialog),
                text_c,
                border_c
            ),
        ]
        .align_y(iced::Alignment::Center);

        let body_row = row![
            container(scrollable(left_col)).padding([14, 14]),
            container(iced::widget::Space::new().width(1).height(Length::Fill),).style(
                move |_: &Theme| container::Style {
                    background: Some(Background::Color(border_c)),
                    ..container::Style::default()
                }
            ),
            container(right_col).padding([14, 14]),
        ]
        .spacing(0)
        .height(Length::Fill);

        // In-window: the window-capped modal. Detached: fill the OS window.
        // The body fills what the header and footer leave, with the left
        // column and the proposed list scrolling, so the footer buttons
        // stay reachable at any scale (#1227).
        let (modal_w, modal_h): (Length, Length) = if draggable {
            let (w, h) = self.annotate_modal_size();
            (Length::Fixed(w), Length::Fixed(h))
        } else {
            (Length::Fill, Length::Fill)
        };
        let dialog = container(
            column![header, body_row, container(footer).padding([10, 14]),]
                .width(modal_w)
                .height(modal_h),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);

        dialog.into()
    }

    pub(in crate::app::view) fn view_annotate_reset_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_annotate_reset_confirm_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::super::state::ModalId::AnnotateResetConfirm)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (420.0, 180.0))
    }

    pub(in crate::app::view) fn view_annotate_reset_confirm_body(&self) -> Element<'_, Message> {
        self.view_annotate_reset_confirm_body_inner(false)
    }

    fn view_annotate_reset_confirm_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let theme_id = self.ui_state.effective_theme_id();
        let header_content: Element<'_, Message> = container(
            row![
                text("Reset All Annotations")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::Annotate(AnnotateMsg::CloseResetConfirm),
                    theme_id,
                    text_muted
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::super::state::ModalId::AnnotateResetConfirm,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };
        let dialog = container(
            column![
                header,
                container(
                    text(
                        "Every reference designator will be reset to '?', then the sheet will be renumbered from 1.\nThis cannot be undone through Ctrl+Z alone — consider saving first.",
                    )
                    .size(TS.body)
                    .color(text_muted),
                )
                .padding([14, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button("Cancel", Message::Annotate(AnnotateMsg::CloseResetConfirm), text_c, border_c),
                        Space::new().width(8),
                        primary_button(
                            "Reset & Renumber",
                            Some(Message::Annotate(AnnotateMsg::Run(
                                signex_engine::AnnotateMode::ResetAndRenumber,
                            ))),
                            border_c,
                        ),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 14]),
            ]
            .width(self.dialog_width(super::super::super::state::ModalId::AnnotateResetConfirm, 420.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }
}
