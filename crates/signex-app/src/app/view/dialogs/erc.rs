//! Electrical Rule Check (ERC) modal — per-rule severity override grid
//! and the pin-connection matrix.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use super::widgets::{
    close_x_button, draggable_header, fit_modal_size, os_title_bar_slot, primary_button,
    secondary_button, secondary_button_maybe, wrap_modal,
};
use super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};

impl Signex {
    pub(in crate::app::view) fn view_erc_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_erc_dialog_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::super::state::ModalId::ErcDialog)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(
            dialog,
            offset,
            self.ui_state.window_size,
            self.erc_modal_size(),
        )
    }

    /// The in-window ERC modal: 1000 x 600 where the window allows, capped
    /// to the window otherwise (#1227).
    pub(in crate::app::view) fn erc_modal_size(&self) -> (f32, f32) {
        fit_modal_size(self.ui_state.window_size, (1000.0, 600.0))
    }

    pub(in crate::app::view) fn view_erc_dialog_body(&self) -> Element<'_, Message> {
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
                close_x_button(Message::Erc(ErcMsg::CloseDialog), theme_id, text_muted),
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
                super::super::super::state::ModalId::ErcDialog,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };

        // Per-rule severity grid: one row per built-in rule × 4 severities.
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
        // A project that stores its own rules shows (and edits) those; the
        // machine preferences only back a project without a rule set.
        let stored = self
            .document_state
            .active_document_project()
            .and_then(|p| p.data.rules.as_ref())
            .map(|rules| signex_erc::project_rules::resolve(&rules.erc));
        let (severity_map, pin_map) = match &stored {
            Some(resolved) => (&resolved.severity, &resolved.pin_matrix),
            None => (
                &self.ui_state.erc_severity_override,
                &self.ui_state.pin_matrix_overrides,
            ),
        };
        for rule in &signex_erc::RuleKind::ALL {
            let current = severity_map
                .get(rule)
                .copied()
                .unwrap_or_else(|| rule.default_severity());
            let label = if rule.is_on_off_only() {
                format!("{} (on/off; severity per matrix cell)", rule.label())
            } else {
                rule.label().to_string()
            };
            rule_rows = rule_rows.push(
                row![
                    text(label)
                        .size(11)
                        .color(text_c)
                        .width(Length::FillPortion(3)),
                    severity_segmented(*rule, current, border_c, text_muted),
                ]
                .align_y(iced::Alignment::Center)
                .padding([2, 8]),
            );
        }

        let history = self
            .document_state
            .active_document_project()
            .and_then(|p| self.document_state.rules_history.get(&p.id));
        let can_undo = history.is_some_and(|h| h.can_undo());
        let can_redo = history.is_some_and(|h| h.can_redo());
        let has_project = self.document_state.active_document_project().is_some();
        let unreadable = self.active_project_has_unreadable_rules();
        let scope_note = match (has_project, stored.is_some(), unreadable) {
            (_, true, _) => "Editing the rules stored in this project.",
            (true, false, true) => {
                "This project's rule set is from a newer Signex and is kept unchanged; \
                 editing this machine's preferences."
            }
            (true, false, false) => {
                "Editing this machine's preferences; this project stores no rules."
            }
            (false, false, _) => "Editing this machine's preferences.",
        };
        let mut footer = row![text(scope_note).size(10).color(text_muted)]
            .spacing(8)
            .align_y(iced::Alignment::Center);
        // A rule set from a newer Signex sits unread in the file; storing
        // would replace it on save, so the action is not offered.
        if has_project && stored.is_none() && !unreadable {
            footer = footer.push(secondary_button(
                "Store ERC rules in this project",
                Message::Erc(ErcMsg::StoreRulesInProject),
                text_c,
                border_c,
            ));
        }
        if stored.is_some() {
            footer = footer
                .push(secondary_button_maybe(
                    "Undo",
                    can_undo.then_some(Message::Erc(ErcMsg::RulesUndone)),
                    if can_undo { text_c } else { text_muted },
                    border_c,
                ))
                .push(secondary_button_maybe(
                    "Redo",
                    can_redo.then_some(Message::Erc(ErcMsg::RulesRedone)),
                    if can_redo { text_c } else { text_muted },
                    border_c,
                ));
        }
        let footer = footer
            .push(Space::new().width(Length::Fill))
            .push(primary_button(
                "Run ERC",
                Some(Message::Erc(ErcMsg::Run)),
                border_c,
            ))
            .push(Space::new().width(8))
            .push(secondary_button(
                "Close",
                Message::Erc(ErcMsg::CloseDialog),
                text_c,
                border_c,
            ));

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
            container(pin_matrix_view(tokens, pin_map))
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

        // In-window: sized to the window-capped modal. Detached: the OS
        // window owns the size, so fill it. The body scrolls; the footer
        // holds the buttons and stays outside the scrollable (#1227).
        let (modal_w, modal_h): (Length, Length) = if draggable {
            let (w, h) = self.erc_modal_size();
            (Length::Fixed(w), Length::Fixed(h))
        } else {
            (Length::Fill, Length::Fill)
        };
        let dialog = container(
            column![
                header,
                container(iced::widget::scrollable(
                    container(body).padding([14, 14]).width(Length::Fill)
                ))
                .width(Length::Fill)
                .height(Length::Fill),
                container(footer).padding([10, 14]),
            ]
            .width(modal_w)
            .height(modal_h),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }
}

fn severity_segmented(
    rule: signex_erc::RuleKind,
    current: signex_erc::Severity,
    border: Color,
    text_muted: Color,
) -> Element<'static, Message> {
    let seg = |label: &'static str,
               value: signex_erc::Severity,
               color: Color|
     -> Element<'static, Message> {
        let selected = if rule.is_on_off_only() && value != signex_erc::Severity::Off {
            current != signex_erc::Severity::Off
        } else {
            value == current
        };
        let bg = if selected {
            color
        } else {
            Color::from_rgba(1.0, 1.0, 1.0, 0.03)
        };
        let fg = if selected { Color::WHITE } else { text_muted };
        button(container(text(label.to_string()).size(10).color(fg)).padding([3, 8]))
            .on_press(Message::Erc(ErcMsg::SeverityChanged(rule, value)))
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
    let off = seg(
        "Off",
        signex_erc::Severity::Off,
        Color::from_rgb(0.35, 0.36, 0.42),
    );
    if rule.is_on_off_only() {
        // Severity comes from the pin matrix per cell; the row only gates it.
        return row![
            seg(
                "On",
                rule.default_severity(),
                Color::from_rgb(0.30, 0.60, 0.95)
            ),
            off
        ]
        .spacing(2)
        .into();
    }
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
        off,
    ]
    .spacing(2)
    .into()
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

    // Six axes; `PinAxis::of` documents how the 14 pin electrical types fold
    // onto them. The baseline grid lives in `signex_erc::pin_matrix`.
    use signex_erc::{PinAxis, Severity};

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
        .on_press(Message::Erc(ErcMsg::PinMatrixCellCycled { row: r, col: c }))
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
    for axis in PinAxis::ALL {
        header = header.push(col_label(axis.label()));
    }

    let mut body = column![header].spacing(0);
    for (r, row_axis) in PinAxis::ALL.iter().enumerate() {
        let mut rr = row![header_label(row_axis.label())].spacing(0);
        for c in 0..PinAxis::ALL.len() {
            let sev = overrides
                .get(&(r as u8, c as u8))
                .copied()
                .unwrap_or_else(|| signex_erc::pin_matrix::baseline_cell(r as u8, c as u8));
            rr = rr.push(cell(r as u8, c as u8, sev));
        }
        body = body.push(rr);
    }

    container(body).into()
}
