//! Annotate-Schematics modal — order pick-list + preview of proposed
//! changes, plus the reset-annotations confirm.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};

use crate::app::state::AnnotateOrder;

use super::widgets::{
    close_x_button, draggable_header, fit_modal_size, os_title_bar_slot, wrap_modal,
};
use super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};
use crate::styles::{
    DialogButtonKind, dialog_body, dialog_button, dialog_footer, dialog_footer_split,
    dialog_message,
};

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
        .boxed();
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
                .padding([5.0, 10.0])
                .width(Length::Fill)
                .style(bordered_style(border_c)),
        ]
        .spacing(SP.s);

        let replace_sub = column![
            text("Replace Sub-Parts").size(TS.caption).color(text_muted),
            container(text("Off").size(TS.body).color(text_muted))
                .padding([5.0, 10.0])
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
        let mut param_rows: iced::widget::Column<Element<'_, Message>> = column![
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
                (row![
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
                .align_y(iced::Alignment::Center))
                .boxed(),
            );
        }
        if param_names.is_empty() {
            param_rows = param_rows.push(
                (container(
                    text("No custom parameters on any symbol yet.")
                        .size(TS.body)
                        .color(text_muted),
                )
                .padding([10.0, SP.m]))
                .boxed(),
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
        let mut sheet_rows: iced::widget::Column<Element<'_, Message>> = column![].spacing(0);
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
                (row![
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
                .align_y(iced::Alignment::Center))
                .boxed(),
            );
        }
        if self.document_state.tabs.is_empty() {
            sheet_rows = sheet_rows.push(
                (container(
                    text("No schematic tab open.")
                        .size(TS.body)
                        .color(text_muted),
                )
                .padding([6.0, SP.m]))
                .boxed(),
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
            text("Scope / Start / Suffix columns are visual only — multi-sheet wiring lands with Advanced Schematic.")
                .size(TS.caption)
                .color(text_muted)
                .width(Length::Fill),
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
        .width(Length::Fill);

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
        let mut rows_col: iced::widget::Column<Element<'_, Message>> =
            column![].spacing(0).width(Length::Fill);
        if proposed.is_empty() {
            rows_col = rows_col.push(
                (container(
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
                .padding([10.0, SP.l]))
                .boxed(),
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
                .padding(0.0)
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
                        ..Border::default()
                    },
                    text_color: text_c,
                    ..button::Style::default()
                })
                .boxed();
                rows_col = rows_col.push(
                    (row![
                        container(lock_btn)
                            .width(Length::Fixed(42.0))
                            .padding([0.0, 6.0]),
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
                    .padding([SP.xs, SP.m]))
                    .boxed(),
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
        .width(Length::Fill)
        .height(Length::Fill);

        // ── Footer ──
        // A run reads, numbers and writes off the UI thread; the buttons that
        // start another stay off until it ends (#1753).
        let busy = self.ui_state.annotate_runs.is_running();
        let idle = |message: Message| (!busy).then_some(message);
        let secondary = |label: &str, message: Option<Message>| {
            dialog_button(label, DialogButtonKind::Secondary, message, tokens)
        };
        let status: Element<'_, Message> = text(if busy { "Running…" } else { "" })
            .size(TS.caption)
            .color(text_muted)
            .boxed();
        let footer = dialog_footer_split(
            vec![
                secondary(
                    "All On",
                    Some(Message::Annotate(AnnotateMsg::SetAllLocked(false))),
                ),
                secondary(
                    "All Off",
                    Some(Message::Annotate(AnnotateMsg::SetAllLocked(true))),
                ),
            ],
            vec![
                status,
                secondary(
                    "Re-prefix to standard",
                    (standard.is_some() && !self.ui_state.annotate_preview.loading && !busy)
                        .then_some(Message::Annotate(AnnotateMsg::ReprefixPressed)),
                ),
                secondary(
                    "Reset All",
                    idle(Message::Annotate(AnnotateMsg::Run(
                        signex_engine::AnnotateMode::ResetOnly,
                    ))),
                ),
                secondary(
                    "Reset & Renumber",
                    idle(Message::Annotate(AnnotateMsg::Run(
                        signex_engine::AnnotateMode::ResetAndRenumber,
                    ))),
                ),
                secondary("Close", Some(Message::Annotate(AnnotateMsg::CloseDialog))),
                dialog_button(
                    "Accept Changes",
                    DialogButtonKind::Primary,
                    if changes == 0 || self.ui_state.annotate_preview.loading || busy {
                        None
                    } else {
                        Some(Message::Annotate(AnnotateMsg::Run(
                            signex_engine::AnnotateMode::Incremental,
                        )))
                    },
                    tokens,
                ),
            ],
        );

        let body_row = row![
            // Fill portions bound each column, so long lines wrap instead
            // of running past the dialog edge (#1770).
            container(scrollable(left_col))
                .padding([14.0, 14.0])
                .width(Length::FillPortion(10)),
            container(iced::widget::Space::new().width(1).height(Length::Fill),).style(
                move |_: &Theme| container::Style {
                    background: Some(Background::Color(border_c)),
                    ..container::Style::default()
                }
            ),
            container(right_col)
                .padding([14.0, 14.0])
                .width(Length::FillPortion(11)),
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
            column![header, body_row, footer,]
                .width(modal_w)
                .height(modal_h),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);

        dialog.boxed()
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
        .boxed();
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
                dialog_body(dialog_message(
                    "Every reference designator will be reset to '?', then the sheet will be renumbered from 1.
This cannot be undone through Ctrl+Z alone — consider saving first.",
                    tokens,
                )),
                dialog_footer(vec![
                    dialog_button(
                        "Cancel",
                        DialogButtonKind::Secondary,
                        Some(Message::Annotate(AnnotateMsg::CloseResetConfirm)),
                        tokens,
                    ),
                    dialog_button(
                        "Reset & Renumber",
                        DialogButtonKind::Primary,
                        (!self.ui_state.annotate_runs.is_running()).then_some(Message::Annotate(
                            AnnotateMsg::Run(signex_engine::AnnotateMode::ResetAndRenumber),
                        )),
                        tokens,
                    ),
                ]),
            ]
            .width(self.dialog_width(super::super::super::state::ModalId::AnnotateResetConfirm, 420.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }
}
