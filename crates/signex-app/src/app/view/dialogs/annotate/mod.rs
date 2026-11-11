//! Annotate-Schematics modal — order pick-list + preview of proposed
//! changes, plus the reset-annotations confirm.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, column, container, row, scrollable};
use iced::{Background, Color, Element, Length, Theme, Widget};
use signex_i18n::{ids, t};
use signex_widgets::{button, raw, text};

use crate::app::state::AnnotateOrder;

use super::widgets::{draggable_header, fit_modal_size, modal_title_header, os_title_bar_slot};
use crate::styles::{
    DialogButtonKind, dialog_body, dialog_button, dialog_footer, dialog_footer_split,
};

mod controls;
mod reprefix_confirm;

use super::graphite as gr;
use super::graphite_annotate as ga;
use controls::{bordered_style, disabled_check_pip, order_radio, scroll_style};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    pub(in crate::app::view) fn view_annotate_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_annotate_dialog_body_inner(true);
        self.wrap_dialog(
            crate::app::state::ModalId::AnnotateDialog,
            dialog,
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
        let i18n = self.ui_state.i18n.as_ref();
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
        let header_content: Element<'_, Message> = gr::titled_header(
            "Annotate",
            self.detached_modal_subtitle(crate::app::state::ModalId::AnnotateDialog),
            Message::Annotate(AnnotateMsg::CloseDialog),
            tokens,
        );
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::super::state::ModalId::AnnotateDialog,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };

        let order_row = column![
            text(raw("Order of Processing"))
                .size(TS.caption)
                .color(text_muted),
            row![
                column![
                    row![
                        order_radio(
                            "Up Then Across",
                            AnnotateOrder::UpThenAcross,
                            self.ui_state.annotate_order,
                            tokens,
                        ),
                        Space::new().width(4),
                        order_radio(
                            "Across Then Down",
                            AnnotateOrder::AcrossThenDown,
                            self.ui_state.annotate_order,
                            tokens,
                        ),
                    ]
                    .spacing(0),
                    Space::new().height(4),
                    row![
                        order_radio(
                            "Down Then Across",
                            AnnotateOrder::DownThenAcross,
                            self.ui_state.annotate_order,
                            tokens,
                        ),
                        Space::new().width(4),
                        order_radio(
                            "Across Then Up",
                            AnnotateOrder::AcrossThenUp,
                            self.ui_state.annotate_order,
                            tokens,
                        ),
                    ]
                    .spacing(0),
                ]
                .spacing(0)
                .width(Length::FillPortion(3)),
                Space::new().width(8),
                ga::order_diagram(tokens, self.ui_state.annotate_order),
            ]
            .align_y(iced::Alignment::Start)
            .spacing(0),
        ]
        .spacing(SP.s);

        let process_location = column![
            text(raw("Process Location of"))
                .size(TS.caption)
                .color(text_muted),
            container(text(raw("Designator")).size(TS.body).color(text_c),)
                .padding([5.0, 10.0])
                .width(Length::Fill)
                .style(bordered_style(tokens)),
        ]
        .spacing(SP.s);

        let replace_sub = column![
            text(raw("Replace Sub-Parts"))
                .size(TS.caption)
                .color(text_muted),
            container(text(raw("Off")).size(TS.body).color(text_muted))
                .padding([5.0, 10.0])
                .width(Length::Fill)
                .style(bordered_style(tokens)),
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
                text(raw("Component Parameter"))
                    .size(TS.caption)
                    .color(text_muted)
                    .width(Length::FillPortion(5)),
                text(raw("Strictly"))
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
                        disabled_check_pip(false, tokens),
                        Space::new().width(6),
                        text(signex_i18n::UserText::new(name.clone()))
                            .size(TS.body)
                            .color(text_muted),
                    ]
                    .align_y(iced::Alignment::Center)
                    .width(Length::FillPortion(5)),
                    container(disabled_check_pip(true, tokens))
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
                    text(raw("No custom parameters on any symbol yet."))
                        .size(TS.body)
                        .color(text_muted),
                )
                .padding([10.0, SP.m]))
                .boxed(),
            );
        }

        let matching = column![
            text(raw("Matching Options")).size(TS.label).color(text_c),
            Space::new().height(4),
            row![
                text(raw("Complete Existing Packages"))
                    .size(TS.caption)
                    .color(text_muted)
                    .width(Length::FillPortion(2)),
                container(text(raw("None")).size(TS.body).color(text_muted))
                    .padding([SP.s, SP.m])
                    .width(Length::FillPortion(3))
                    .style(bordered_style(tokens)),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(6),
            Space::new().height(6),
            container(
                scrollable(param_rows)
                    .height(98)
                    .style(scroll_style(tokens))
            )
            .padding(SP.xs)
            .style(bordered_style(tokens)),
            Space::new().height(4),
            text(t!(i18n, ids::DIALOG_ANNOTATE_MATCHING_UNAVAILABLE))
                .size(TS.caption)
                .color(text_muted),
        ]
        .spacing(SP.s);

        let active_title = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|t| t.title.clone());

        // The mock's sheet card (checkbox, mono name, Start, Add Suffix) over
        // the open sheets.
        let sheets_view: Element<'_, Message> = {
            let rows = self
                .document_state
                .tabs
                .iter()
                .map(|tab| ga::SheetRow {
                    name: tab.title.clone(),
                    on: Some(&tab.title) == active_title.as_ref(),
                })
                .collect();
            ga::sheet_table(tokens, rows, "No schematic tab open.")
        };

        let sheets_table = column![
            text(raw("Schematic Sheets To Annotate"))
                .size(TS.label)
                .color(text_c),
            Space::new().height(4),
            sheets_view,
            Space::new().height(4),
            text(t!(i18n, ids::DIALOG_ANNOTATE_SHEET_OPTIONS_UNAVAILABLE))
                .size(TS.caption)
                .color(text_muted)
                .width(Length::Fill),
        ]
        .spacing(SP.s);

        let left_col = column![
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
        let right_title = text(raw("Proposed Change List"))
            .size(TS.label)
            .color(text_c);

        // Column headers
        let list_headers = row![
            text(raw("Lock"))
                .size(TS.caption)
                .color(text_muted)
                .width(Length::Fixed(42.0)),
            text(raw("Current"))
                .size(TS.caption)
                .color(text_muted)
                .width(Length::FillPortion(2)),
            text(raw("Proposed"))
                .size(TS.caption)
                .color(text_muted)
                .width(Length::FillPortion(2)),
            text(raw("Location"))
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
                    text(raw(if self.ui_state.annotate_preview.loading {
                        "Loading project sheets…"
                    } else if self.ui_state.annotate_preview.load_failed {
                        "Could not load the project's sheets - see the Messages panel."
                    } else {
                        "No symbols need annotation — every reference has a number."
                    }))
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
                let uuid = entry.uuid;
                // The lock is a checkbox.
                let lock_btn: Element<'_, Message> = button(ga::checkbox_pip(tokens, is_locked))
                    .on_press(Message::Annotate(AnnotateMsg::ToggleLock(uuid)))
                    .padding(0.0)
                    .style(ga::bare_button_style)
                    .boxed();
                // Designators in mono; a changing row is tinted and its proposed
                // designator takes the accent ink.
                let cur_text = text(raw(entry.current.clone()))
                    .size(TS.body)
                    .color(cur_color)
                    .width(Length::FillPortion(2))
                    .font(gr::mono());
                let new_text = text(raw(proposed_display))
                    .size(TS.body)
                    .color(if changing {
                        gr::changed_ink(tokens)
                    } else {
                        new_color
                    })
                    .width(Length::FillPortion(2))
                    .font(gr::mono());
                let line = row![
                    container(lock_btn)
                        .width(Length::Fixed(42.0))
                        .padding([0.0, 6.0]),
                    cur_text,
                    new_text,
                    text(signex_i18n::UserText::new(entry.sheet.clone()))
                        .size(TS.body)
                        .color(text_muted)
                        .width(Length::FillPortion(3)),
                ]
                .align_y(iced::Alignment::Center)
                .padding([SP.xs, SP.m]);
                let line: Element<'_, Message> = container(line)
                    .width(Length::Fill)
                    .style(gr::changed_row(tokens, changing))
                    .boxed();
                rows_col = rows_col.push(line);
            }
        }

        let right_list = container(
            scrollable(rows_col)
                .height(Length::Fill)
                .style(scroll_style(tokens)),
        )
        .height(Length::Fill)
        .padding(SP.s)
        .style(bordered_style(tokens));

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
        let summary = container(text(raw(summary_text)).size(TS.caption).color(text_muted))
            .padding([SP.s, SP.s]);
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
            text(raw("Annotation Summary"))
                .size(TS.caption)
                .color(text_c),
            summary,
            container(text(raw(reprefix_note)).size(TS.caption).color(text_muted))
                .padding([SP.s, SP.s]),
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
        let status: Element<'_, Message> = text(raw(if busy { "Running…" } else { "" }))
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
                dialog_button(
                    "Close",
                    DialogButtonKind::Cancel,
                    Some(Message::Annotate(AnnotateMsg::CloseDialog)),
                    tokens,
                ),
                dialog_button(
                    "Accept Changes",
                    gr::call_to_action(tokens),
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
            container(scrollable(left_col).style(scroll_style(tokens)))
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
        self.wrap_dialog(
            crate::app::state::ModalId::AnnotateResetConfirm,
            dialog,
            super::sizes::ANNOTATE_RESET_CONFIRM,
        )
    }

    pub(in crate::app::view) fn view_annotate_reset_confirm_body(&self) -> Element<'_, Message> {
        self.view_annotate_reset_confirm_body_inner(false)
    }

    fn view_annotate_reset_confirm_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;

        let header_content: Element<'_, Message> = modal_title_header(
            "Reset All Annotations",
            Message::Annotate(AnnotateMsg::CloseResetConfirm),
            tokens,
        );
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
                dialog_body(crate::styles::dialog_intro(
                    crate::styles::DialogIcon::Warn,
                    "Reset every designator?",
                    Some(
                        "Every reference designator will be reset to '?', then the sheet will be renumbered from 1.
This cannot be undone through Ctrl+Z alone — consider saving first."
                            .to_owned()
                    ),
                    tokens,
                )),
                dialog_footer(vec![
                    dialog_button(
                        "Cancel",
                        DialogButtonKind::Cancel,
                        Some(Message::Annotate(AnnotateMsg::CloseResetConfirm)),
                        tokens,
                    ),
                    dialog_button(
                        "Reset & Renumber",
                        DialogButtonKind::Danger,
                        (!self.ui_state.annotate_runs.is_running()).then_some(Message::Annotate(
                            AnnotateMsg::Run(signex_engine::AnnotateMode::ResetAndRenumber),
                        )),
                        tokens,
                    ),
                ]),
            ]
            .width(self.dialog_width(super::super::super::state::ModalId::AnnotateResetConfirm, super::sizes::ANNOTATE_RESET_CONFIRM)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }
}
