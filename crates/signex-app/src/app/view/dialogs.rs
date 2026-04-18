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
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        // Compute preview: walk current snapshot, propose new refs per order.
        let mut proposed: Vec<(String, String)> = Vec::new();
        let mut current_sheet_name = String::from("Current Sheet");
        let mut total_symbols = 0usize;
        if let Some(snapshot) = self.active_render_snapshot() {
            proposed = preview_annotations(snapshot, self.ui_state.annotate_order);
            total_symbols = snapshot.symbols.len();
        }
        if let Some(tab) = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
        {
            current_sheet_name = tab.title.clone();
        }

        // ── Header ──
        let header = container(
            row![
                text("Annotate").size(14).color(text_c),
                Space::new().width(Length::Fill),
                close_button("Close", Message::CloseAnnotateDialog, text_muted, border_c),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([10, 14])
        .style(crate::styles::toolbar_strip(tokens));

        // ── Left column: Schematic Annotation Configuration ──
        let left_title = text("Schematic Annotation Configuration")
            .size(12)
            .color(text_c);

        let order_row = column![
            text("Order of Processing").size(10).color(text_muted),
            row![
                order_radio(
                    "Across then Up",
                    AnnotateOrder::AcrossThenUp,
                    self.ui_state.annotate_order,
                    text_c,
                    border_c,
                ),
                Space::new().width(6),
                order_radio(
                    "Up then Across",
                    AnnotateOrder::UpThenAcross,
                    self.ui_state.annotate_order,
                    text_c,
                    border_c,
                ),
            ]
            .align_y(iced::Alignment::Center)
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
            text("Parameter-matching options (accuracy, body finish, tolerance…) ship in v0.7.1 with the full parameter manager.")
                .size(10)
                .color(text_muted),
        ]
        .spacing(4);

        let sheets_table = column![
            text("Schematic Sheets To Annotate").size(12).color(text_c),
            Space::new().height(4),
            row![
                text("Sheet").size(10).color(text_muted).width(Length::FillPortion(3)),
                text("Scope").size(10).color(text_muted).width(Length::FillPortion(1)),
                text("Start").size(10).color(text_muted).width(Length::FillPortion(1)),
            ]
            .padding([2, 8]),
            container(
                row![
                    text(current_sheet_name.clone())
                        .size(11)
                        .color(text_c)
                        .width(Length::FillPortion(3)),
                    text("All").size(11).color(text_muted).width(Length::FillPortion(1)),
                    text("1").size(11).color(text_muted).width(Length::FillPortion(1)),
                ]
                .padding([3, 8]),
            )
            .style(bordered_style(border_c)),
            Space::new().height(4),
            text("Multi-sheet annotation (hierarchical + scope) lands with v1.1 Advanced Schematic.")
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
            text("Current").size(10).color(text_muted).width(Length::FillPortion(2)),
            text("Proposed").size(10).color(text_muted).width(Length::FillPortion(2)),
            text("Location").size(10).color(text_muted).width(Length::FillPortion(3)),
        ]
        .padding([4, 8]);

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
            for (cur, new) in &proposed {
                rows_col = rows_col.push(
                    row![
                        text(cur.clone())
                            .size(11)
                            .color(text_muted)
                            .width(Length::FillPortion(2)),
                        text(new.clone())
                            .size(11)
                            .color(text_c)
                            .width(Length::FillPortion(2)),
                        text(current_sheet_name.clone())
                            .size(11)
                            .color(text_muted)
                            .width(Length::FillPortion(3)),
                    ]
                    .padding([2, 8]),
                );
            }
        }

        let right_list = container(scrollable(rows_col).height(360))
            .padding(4)
            .style(bordered_style(border_c));

        let summary_text = if proposed.is_empty() {
            format!(
                "Annotation has nothing to do — all {total_symbols} symbols on '{current_sheet_name}' already carry a designator. Use Reset All or Reset & Renumber below to renumber from scratch."
            )
        } else {
            format!(
                "Annotation will assign new designators to {} of {total_symbols} symbols on '{current_sheet_name}'. Click Accept Changes to apply, Update Changes List to recompute, or Reset All to clear every number first.",
                proposed.len(),
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
                if proposed.is_empty() {
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

        let dialog = container(
            column![
                header,
                body_row,
                container(footer).padding([10, 14]),
            ]
            .width(1080),
        )
        .style(crate::styles::context_menu(tokens));

        wrap_modal(dialog.into())
    }

    pub(super) fn view_annotate_reset_confirm(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let dialog = container(
            column![
                container(text("Reset All Annotations").size(14).color(text_c))
                    .padding([10, 14])
                    .style(crate::styles::toolbar_strip(tokens)),
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
        wrap_modal(dialog.into())
    }

    pub(super) fn view_erc_dialog(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header = container(
            row![
                text("Electrical Rules Check").size(14).color(text_c),
                Space::new().width(Length::Fill),
                close_button("Close", Message::CloseErcDialog, text_muted, border_c),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([10, 14])
        .style(crate::styles::toolbar_strip(tokens));

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

        let dialog = container(
            column![
                header,
                container(
                    column![
                        text("Severity per rule — click to change.")
                            .size(11)
                            .color(text_muted),
                        Space::new().height(6),
                        container(scrollable(rule_rows).height(300))
                            .padding(4)
                            .style(move |_: &Theme| container::Style {
                                border: Border {
                                    width: 1.0,
                                    radius: 3.0.into(),
                                    color: border_c,
                                },
                                ..container::Style::default()
                            }),
                        Space::new().height(10),
                        pin_matrix_view(tokens),
                    ]
                    .spacing(4),
                )
                .padding([14, 14]),
                container(footer).padding([10, 14]),
            ]
            .width(640),
        )
        .style(crate::styles::context_menu(tokens));
        wrap_modal(dialog.into())
    }
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

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

fn wrap_modal(inner: Element<'_, Message>) -> Element<'_, Message> {
    container(
        column![
            Space::new().height(Length::Fill),
            row![
                Space::new().width(Length::Fill),
                inner,
                Space::new().width(Length::Fill),
            ],
            Space::new().height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(BACKDROP)),
        ..container::Style::default()
    })
    .into()
}

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
fn preview_annotations(
    snapshot: &signex_render::schematic::SchematicRenderSnapshot,
    _order: AnnotateOrder,
) -> Vec<(String, String)> {
    // Collect existing per-prefix counters.
    let mut next: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for sym in &snapshot.symbols {
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
    let mut out = Vec::new();
    for i in idx {
        let sym = &snapshot.symbols[i];
        if !sym.reference.ends_with('?') {
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
        let n = next.entry(prefix.clone()).or_insert(0);
        *n += 1;
        out.push((sym.reference.clone(), format!("{prefix}{n}")));
    }
    out
}

/// Pin-connection matrix — compact read-only view for v0.7. Displays the
/// default Altium-style pin-to-pin compatibility grid. Editing ships in v0.7.1.
fn pin_matrix_view(tokens: &signex_types::theme::ThemeTokens) -> Element<'static, Message> {
    let text_c = crate::styles::ti(tokens.text);
    let text_muted = crate::styles::ti(tokens.text_secondary);
    let border = crate::styles::ti(tokens.border);
    let cell_ok = Color::from_rgba(0.25, 0.70, 0.30, 0.35);
    let cell_warn = Color::from_rgba(0.95, 0.70, 0.15, 0.45);
    let cell_err = Color::from_rgba(0.85, 0.25, 0.25, 0.45);

    // 6 primary pin types for v0.7 (full 12-type Altium matrix in v0.7.1).
    const TYPES: &[&str] = &[
        "Input", "Output", "Bidir", "PowerIn", "PowerOut", "NC",
    ];
    // 0 = ok, 1 = warn, 2 = error. Lower-triangular matrix, symmetric logic.
    #[rustfmt::skip]
    const MATRIX: [[u8; 6]; 6] = [
        //  In  Out Bi  Pin Pou NC
        [    0,  0,  0,  0,  0,  0 ], // Input
        [    0,  2,  0,  0,  2,  2 ], // Output (Out-Out short, Out-PowerOut short)
        [    0,  0,  0,  0,  0,  1 ], // Bidir
        [    0,  0,  0,  0,  0,  2 ], // Power In
        [    0,  2,  0,  0,  2,  2 ], // Power Out
        [    0,  2,  1,  2,  2,  0 ], // NC
    ];

    let cell = |v: u8| -> Element<'static, Message> {
        let bg = match v {
            2 => cell_err,
            1 => cell_warn,
            _ => cell_ok,
        };
        let ch = match v {
            2 => "E",
            1 => "W",
            _ => "·",
        };
        container(text(ch.to_string()).size(10).color(text_c))
            .width(30)
            .height(20)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 0.0.into(),
                    color: border,
                },
                ..container::Style::default()
            })
            .into()
    };

    let header_label = |label: &str| -> Element<'static, Message> {
        container(text(label.to_string()).size(9).color(text_muted))
            .width(60)
            .height(20)
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Center)
            .into()
    };
    let col_label = |label: &str| -> Element<'static, Message> {
        container(text(label.to_string()).size(9).color(text_muted))
            .width(30)
            .align_x(iced::alignment::Horizontal::Center)
            .into()
    };

    let mut header = row![container(Space::new().width(60)).width(60)].spacing(0);
    for t in TYPES {
        header = header.push(col_label(t));
    }

    let mut body = column![header].spacing(0);
    for (r, row_label) in TYPES.iter().enumerate() {
        let mut rr = row![header_label(row_label)].spacing(0);
        for c in 0..TYPES.len() {
            rr = rr.push(cell(MATRIX[r][c]));
        }
        body = body.push(rr);
    }

    container(
        column![
            text("Pin Connection Matrix (read-only, editable in v0.7.1)")
                .size(10)
                .color(text_muted),
            Space::new().height(4),
            body,
        ]
        .spacing(0),
    )
    .into()
}
