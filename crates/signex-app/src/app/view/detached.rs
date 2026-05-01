use iced::{Element, Length};

use super::*;

impl Signex {
    /// Altium-style Move Selection dialog. Two numeric inputs plus
    /// OK / Cancel. No header drag region on the body itself — the
    /// modal opens borderless so the OS-window-drag handler owns that.
    fn view_move_selection_body(&self) -> Element<'_, Message> {
        use iced::widget::{Space, button, column, container, row, text, text_input};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let ms = &self.ui_state.move_selection;
        let selection_count = self.interaction_state.active_canvas().selected.len();

        let header = iced::widget::mouse_area(
            container(
                row![
                    text("Move Selection").size(14).color(text_c),
                    Space::new().width(iced::Length::Fill),
                    self.view_close_x(Message::CloseMoveSelectionDialog),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([10, 14])
            .style(crate::styles::modal_header_strip(tokens)),
        )
        .on_press(Message::StartDetachedWindowDrag(
            super::state::ModalId::MoveSelection,
        ))
        .interaction(iced::mouse::Interaction::Grab);

        let field = |label: &'static str, value: &str, msg: fn(String) -> Message| {
            column![
                text(label).size(10).color(text_muted),
                text_input("0.00", value)
                    .on_input(msg)
                    .padding([4, 8])
                    .size(12),
            ]
            .spacing(4)
        };

        let body = container(
            column![
                text(format!("{} item(s) selected", selection_count))
                    .size(11)
                    .color(text_muted),
                Space::new().height(12),
                row![
                    field("ΔX (mm)", &ms.dx, Message::MoveSelectionDxChanged),
                    Space::new().width(14),
                    field("ΔY (mm)", &ms.dy, Message::MoveSelectionDyChanged),
                ]
                .align_y(iced::Alignment::Start),
            ]
            .spacing(0),
        )
        .padding([14, 14]);

        let ok_enabled = selection_count > 0;
        let ok_bg = if ok_enabled {
            iced::Color::from_rgb(0.00, 0.47, 0.84)
        } else {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
        };
        let ok_fg = if ok_enabled {
            iced::Color::WHITE
        } else {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.4)
        };
        let mut ok_btn = button(container(text("Apply").size(11).color(ok_fg)).padding([4, 14]))
            .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(ok_bg)),
                border: iced::Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    ..iced::Border::default()
                },
                text_color: ok_fg,
                ..iced::widget::button::Style::default()
            });
        if ok_enabled {
            ok_btn = ok_btn.on_press(Message::MoveSelectionApply);
        }

        let footer = container(
            row![
                Space::new().width(iced::Length::Fill),
                button(container(text("Cancel").size(11).color(text_c)).padding([4, 14]))
                    .on_press(Message::CloseMoveSelectionDialog)
                    .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.04
                        ),)),
                        border: iced::Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: border_c,
                        },
                        text_color: text_c,
                        ..iced::widget::button::Style::default()
                    }),
                Space::new().width(8),
                ok_btn,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([10, 14]);

        container(
            column![header, body, footer]
                .width(iced::Length::Fixed(420.0))
                .height(iced::Length::Fixed(240.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .into()
    }

    /// Compact X close button shared by the detached-modal bodies.
    fn view_close_x(&self, message: Message) -> Element<'_, Message> {
        use iced::widget::{button, container, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text_secondary);
        let border = crate::styles::ti(tokens.border);
        button(container(text("\u{00D7}".to_string()).size(14).color(text_c)).padding([0, 6]))
            .on_press(message)
            .style(
                move |_: &iced::Theme, status: iced::widget::button::Status| {
                    let bg = match status {
                        iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.1),
                        )),
                        _ => Some(iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.03,
                        ))),
                    };
                    iced::widget::button::Style {
                        background: bg,
                        border: iced::Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: border,
                        },
                        text_color: text_c,
                        ..iced::widget::button::Style::default()
                    }
                },
            )
            .into()
    }

    /// Altium F5 Net Color palette — list of net labels with a per-net
    /// color picker. Ships with a 10-swatch palette; a full ColorPicker
    /// widget can replace it later without changing the message contract.
    fn view_net_color_palette_body(&self) -> Element<'_, Message> {
        use iced::widget::{Space, button, column, container, row, scrollable, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header = iced::widget::mouse_area(
            container(
                row![
                    text("Net Colors").size(14).color(text_c),
                    Space::new().width(iced::Length::Fill),
                    self.view_close_x(Message::CloseNetColorPalette),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([10, 14])
            .style(crate::styles::modal_header_strip(tokens)),
        )
        .on_press(Message::StartDetachedWindowDrag(
            super::state::ModalId::NetColorPalette,
        ))
        .interaction(iced::mouse::Interaction::Grab);

        // Gather unique net labels from the active snapshot.
        let mut nets: Vec<String> = self
            .interaction_state
            .canvas
            .active_snapshot()
            .map(|s| {
                s.labels
                    .iter()
                    .filter(|l| {
                        matches!(
                            l.label_type,
                            signex_types::schematic::LabelType::Net
                                | signex_types::schematic::LabelType::Global
                                | signex_types::schematic::LabelType::Hierarchical
                        )
                    })
                    .map(|l| l.text.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect()
            })
            .unwrap_or_default();
        nets.sort();

        const PALETTE: &[(u8, u8, u8)] = &[
            (0xE0, 0x54, 0x54),
            (0xE0, 0xB0, 0x4A),
            (0x78, 0xC2, 0x6A),
            (0x42, 0xB8, 0xE0),
            (0x6F, 0x77, 0xE0),
            (0xB0, 0x6F, 0xE0),
            (0xE0, 0x6F, 0xB0),
            (0xC2, 0xA0, 0x78),
            (0x78, 0xC2, 0xA0),
            (0xA0, 0xA0, 0xA0),
        ];

        let mut rows_col = column![].spacing(4);
        if nets.is_empty() {
            rows_col = rows_col.push(
                text("No net labels on the active sheet.")
                    .size(11)
                    .color(text_muted),
            );
        } else {
            for net in nets {
                let current = self.ui_state.net_colors.get(&net).copied();
                let mut swatches = row![].spacing(4).align_y(iced::Alignment::Center);
                for (r, g, b) in PALETTE {
                    let is_current = current.is_some_and(|c| c.r == *r && c.g == *g && c.b == *b);
                    let swatch_color = iced::Color::from_rgb8(*r, *g, *b);
                    let border_w = if is_current { 2.0_f32 } else { 1.0_f32 };
                    let net_copy = net.clone();
                    let r_c = *r;
                    let g_c = *g;
                    let b_c = *b;
                    swatches =
                        swatches.push(
                            button(container(Space::new().width(14).height(14)).style(
                                move |_: &iced::Theme| container::Style {
                                    background: Some(iced::Background::Color(swatch_color)),
                                    border: iced::Border {
                                        width: border_w,
                                        radius: 2.0.into(),
                                        color: text_c,
                                    },
                                    ..container::Style::default()
                                },
                            ))
                            .on_press(Message::NetColorSet {
                                net: net_copy.clone(),
                                color: Some(signex_types::theme::Color {
                                    r: r_c,
                                    g: g_c,
                                    b: b_c,
                                    a: 255,
                                }),
                            })
                            .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                                background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                                border: iced::Border::default(),
                                ..iced::widget::button::Style::default()
                            }),
                        );
                }
                // Clear-override button
                let net_clear = net.clone();
                swatches = swatches.push(
                    button(container(text("×").size(10).color(text_c)).padding([0, 6]))
                        .on_press(Message::NetColorSet {
                            net: net_clear,
                            color: None,
                        })
                        .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                            background: Some(iced::Background::Color(iced::Color::from_rgba(
                                1.0, 1.0, 1.0, 0.04,
                            ))),
                            border: iced::Border {
                                width: 1.0,
                                radius: 2.0.into(),
                                color: border_c,
                            },
                            text_color: text_c,
                            ..iced::widget::button::Style::default()
                        }),
                );

                rows_col = rows_col.push(
                    row![
                        text(net)
                            .size(11)
                            .color(text_c)
                            .width(iced::Length::FillPortion(2)),
                        swatches,
                    ]
                    .align_y(iced::Alignment::Center)
                    .padding([2, 8]),
                );
            }
        }

        container(
            column![
                header,
                container(scrollable(rows_col).height(iced::Length::Fill))
                    .padding([14, 14])
                    .height(iced::Length::Fill),
            ]
            .width(iced::Length::Fixed(520.0))
            .height(iced::Length::Fixed(480.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .into()
    }

    /// Altium-style Parameter Manager — a scrolling table listing every
    /// placed symbol with columns for reference / value / footprint and
    /// a "Parameter" column that reveals the union of custom fields
    /// across the design. Each cell is a text_input so the user can edit
    /// values inline. Changes route through Command::SetSymbolField so
    /// undo/redo / dirty-flagging behaves.
    fn view_parameter_manager_body(&self) -> Element<'_, Message> {
        use iced::widget::{Space, column, container, row, scrollable, text, text_input};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header = iced::widget::mouse_area(
            container(
                row![
                    text("Parameter Manager").size(14).color(text_c),
                    Space::new().width(iced::Length::Fill),
                    self.view_close_x(Message::CloseParameterManager),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([10, 14])
            .style(crate::styles::modal_header_strip(tokens)),
        )
        .on_press(Message::StartDetachedWindowDrag(
            super::state::ModalId::ParameterManager,
        ))
        .interaction(iced::mouse::Interaction::Grab);

        // Collect all parameter keys across symbols (besides the built-
        // in reference / value / footprint). Keeps the table compact —
        // only columns that someone actually uses show up.
        let Some(engine) = self.document_state.active_engine() else {
            return container(
                column![
                    header,
                    container(text("No active schematic.").size(11).color(text_muted))
                        .padding([14, 14]),
                ]
                .width(iced::Length::Fixed(900.0))
                .height(iced::Length::Fixed(560.0)),
            )
            .style(crate::styles::modal_card(tokens))
        .clip(true)
            .into();
        };
        let doc = engine.document();
        let mut keys: Vec<String> = doc
            .symbols
            .iter()
            .filter(|s| !s.is_power)
            .flat_map(|s| s.fields.keys().cloned())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        keys.sort();

        let header_row = {
            let mut r = row![
                text("Reference")
                    .size(10)
                    .color(text_muted)
                    .width(iced::Length::Fixed(100.0)),
                text("Value")
                    .size(10)
                    .color(text_muted)
                    .width(iced::Length::Fixed(160.0)),
                text("Footprint")
                    .size(10)
                    .color(text_muted)
                    .width(iced::Length::Fixed(200.0)),
            ];
            for k in &keys {
                r = r.push(
                    text(k.clone())
                        .size(10)
                        .color(text_muted)
                        .width(iced::Length::Fixed(140.0)),
                );
            }
            r.padding([4, 8])
        };

        let mut rows_col = column![].spacing(2);
        rows_col = rows_col.push(header_row);
        for sym in &doc.symbols {
            if sym.is_power {
                continue;
            }
            let mut r = row![
                text(sym.reference.clone())
                    .size(11)
                    .color(text_c)
                    .width(iced::Length::Fixed(100.0)),
                text(sym.value.clone())
                    .size(11)
                    .color(text_c)
                    .width(iced::Length::Fixed(160.0)),
                text(sym.footprint.clone())
                    .size(11)
                    .color(text_muted)
                    .width(iced::Length::Fixed(200.0)),
            ];
            for k in &keys {
                let v = sym.fields.get(k).cloned().unwrap_or_default();
                let sym_uuid = sym.uuid;
                let k_str = k.clone();
                r = r.push(
                    text_input("", &v)
                        .on_input(move |new_val| Message::ParameterManagerEdit {
                            symbol_uuid: sym_uuid,
                            key: k_str.clone(),
                            value: new_val,
                        })
                        .padding([2, 6])
                        .size(11)
                        .width(iced::Length::Fixed(140.0)),
                );
            }
            rows_col = rows_col.push(r.padding([2, 8]));
        }

        container(
            column![
                header,
                container(
                    scrollable(rows_col)
                        .direction(scrollable::Direction::Both {
                            vertical: scrollable::Scrollbar::default(),
                            horizontal: scrollable::Scrollbar::default(),
                        })
                        .height(iced::Length::Fill),
                )
                .padding([14, 14])
                .height(iced::Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    border: iced::Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border_c,
                    },
                    ..container::Style::default()
                }),
            ]
            .width(iced::Length::Fixed(900.0))
            .height(iced::Length::Fixed(560.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .into()
    }

    /// Custom net-colour picker modal. Grid of quick-pick swatches on
    /// the left, precise R / G / B / hex on the right, live preview
    /// and OK / Cancel at the bottom. Ships with a 24-color palette
    /// matching the common Altium net-colour presets plus a handful of
    /// EDA-specific diagnostic colours.
    pub(super) fn view_net_color_custom_picker(&self) -> Element<'_, Message> {
        use super::contracts::Channel;
        use iced::widget::{Space, button, column, container, row, text, text_input};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let draft = self.ui_state.net_color_custom.draft;

        // Expanded 48-swatch palette arranged as 6 cols × 8 rows so
        // the Quick Pick grid fills the modal's left column. First
        // three rows are "standard" hues, next three rows are shade
        // variants, and the last two rows hold greys / light pastels /
        // schematic-specific high-contrast hues.
        const PALETTE: &[(u8, u8, u8)] = &[
            // Row 1 — primaries (bright)
            (0xEF, 0x44, 0x44), // Red
            (0xF9, 0x73, 0x16), // Orange
            (0xEA, 0xB3, 0x08), // Yellow
            (0x22, 0xC5, 0x5E), // Green
            (0x06, 0xB6, 0xD4), // Cyan
            (0x3B, 0x82, 0xF6), // Blue
            // Row 2 — pinks + magentas + purples
            (0xF4, 0x72, 0xB6), // Pink 400
            (0xE1, 0x14, 0x8C), // Hot Pink
            (0xD9, 0x46, 0xEF), // Fuchsia
            (0xA8, 0x55, 0xF7), // Purple
            (0x8B, 0x5C, 0xF6), // Violet
            (0x6D, 0x28, 0xD9), // Indigo
            // Row 3 — greens + teals + lime
            (0x84, 0xCC, 0x16), // Lime
            (0x10, 0xB9, 0x81), // Emerald
            (0x14, 0xB8, 0xA6), // Teal
            (0x0E, 0xA5, 0xE9), // Sky
            (0x60, 0xA5, 0xFA), // Light Blue
            (0x2D, 0xD4, 0xBF), // Turquoise
            // Row 4 — dark variants
            (0x9F, 0x12, 0x39), // Wine
            (0xB4, 0x53, 0x09), // Rust
            (0xA1, 0x6A, 0x3C), // Brown
            (0x16, 0xA3, 0x4A), // Dark Green
            (0x15, 0x5E, 0x75), // Deep Cyan
            (0x1E, 0x40, 0xAF), // Deep Blue
            // Row 5 — extra dark / night hues
            (0x7F, 0x1D, 0x1D), // Deep Red
            (0x78, 0x35, 0x0F), // Auburn
            (0x5B, 0x21, 0xB6), // Royal Purple
            (0x3B, 0x0A, 0x45), // Eggplant
            (0x1E, 0x3A, 0x8A), // Navy
            (0x0F, 0x17, 0x2A), // Midnight
            // Row 6 — pastels
            (0xFE, 0xCA, 0xCA), // Pastel Red
            (0xFE, 0xD7, 0xAA), // Pastel Orange
            (0xFE, 0xF0, 0x8A), // Pastel Yellow
            (0xBB, 0xF7, 0xD0), // Pastel Green
            (0xA5, 0xF3, 0xFC), // Pastel Cyan
            (0xBF, 0xDB, 0xFE), // Pastel Blue
            // Row 7 — muted + desaturated
            (0x64, 0x74, 0x8B), // Slate
            (0x78, 0x71, 0x6C), // Stone
            (0x4B, 0x55, 0x63), // Dark Slate
            (0x9C, 0xA3, 0xAF), // Gray
            (0xD1, 0xD5, 0xDB), // Light Gray
            (0xFF, 0xFF, 0xFF), // White
            // Row 8 — schematic diagnostic colors
            (0xFF, 0x00, 0xFF), // Bright Magenta
            (0x00, 0xFF, 0xFF), // Bright Cyan
            (0xFF, 0xFF, 0x00), // Bright Yellow
            (0x00, 0xFF, 0x00), // Bright Green
            (0xFF, 0xA5, 0x00), // Bright Orange
            (0x1F, 0x23, 0x2A), // Ink
        ];

        let swatch_btn =
            |r: u8, g: u8, b: u8| -> Element<'_, Message> {
                let col = iced::Color::from_rgb8(r, g, b);
                let is_current = (draft.r - col.r).abs() < 0.01
                    && (draft.g - col.g).abs() < 0.01
                    && (draft.b - col.b).abs() < 0.01;
                let sw = iced::Color::from_rgb8(r, g, b);
                let border_w = if is_current { 2.0 } else { 1.0 };
                let border_col = if is_current {
                    iced::Color::WHITE
                } else {
                    iced::Color::from_rgba(0.2, 0.2, 0.22, 0.9)
                };
                button(container(Space::new().width(24).height(20)).style(
                    move |_: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(sw)),
                        border: iced::Border {
                            width: border_w,
                            radius: 3.0.into(),
                            color: border_col,
                        },
                        ..container::Style::default()
                    },
                ))
                .padding(0)
                .on_press(Message::NetColorCustomDraft(col))
                .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                    border: iced::Border::default(),
                    ..iced::widget::button::Style::default()
                })
                .into()
            };

        // Build the 6 × 4 palette grid row by row.
        let mut palette_col = column![].spacing(6);
        for chunk in PALETTE.chunks(6) {
            let mut r_el = row![].spacing(6);
            for (r, g, b) in chunk {
                r_el = r_el.push(swatch_btn(*r, *g, *b));
            }
            palette_col = palette_col.push(r_el);
        }

        // RGB inputs — parse as u8, clamp on submit. Uses the
        // `draft` colour as the current value so swatch clicks and
        // text edits stay in sync.
        let channel_row =
            |label: &'static str, value: f32, chan: Channel| -> Element<'_, Message> {
                let v255 = (value * 255.0).round() as i32;
                row![
                    text(label)
                        .size(11)
                        .color(text_muted)
                        .width(iced::Length::Fixed(16.0)),
                    text_input("0", &v255.to_string())
                        .size(11)
                        .padding([3, 8])
                        .width(iced::Length::Fixed(70.0))
                        .on_input(move |s| Message::NetColorCustomChannel(chan, s)),
                ]
                .align_y(iced::Alignment::Center)
                .spacing(6)
                .into()
            };

        let preview_hex = format!(
            "#{:02X}{:02X}{:02X}",
            (draft.r * 255.0).round() as u8,
            (draft.g * 255.0).round() as u8,
            (draft.b * 255.0).round() as u8,
        );
        let preview_box = container(Space::new().width(iced::Length::Fill).height(32)).style(
            move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(draft)),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border_c,
                },
                ..container::Style::default()
            },
        );

        let rgb_col = column![
            text("Custom RGB").size(11).color(text_c),
            Space::new().height(6),
            channel_row("R", draft.r, Channel::R),
            channel_row("G", draft.g, Channel::G),
            channel_row("B", draft.b, Channel::B),
            Space::new().height(10),
            preview_box,
            Space::new().height(4),
            text(preview_hex).size(10).color(text_muted),
        ]
        .spacing(4)
        .width(iced::Length::Fixed(150.0));

        let body = row![
            column![
                text("Quick Pick").size(11).color(text_c),
                Space::new().height(6),
                palette_col,
            ]
            .spacing(0)
            .width(iced::Length::Fill),
            Space::new().width(16),
            rgb_col,
        ];

        let footer = row![
            Space::new().width(iced::Length::Fill),
            button(container(text("Cancel").size(11).color(text_c)).padding([4, 14]),)
                .on_press(Message::NetColorCustomShow(false))
                .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04,
                    ))),
                    border: iced::Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border_c,
                    },
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }),
            Space::new().width(8),
            button(
                container(text("Use Color").size(11).color(iced::Color::WHITE)).padding([4, 14]),
            )
            .on_press(Message::NetColorCustomSubmit(draft))
            .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.00, 0.47, 0.84,
                ))),
                border: iced::Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    ..iced::Border::default()
                },
                text_color: iced::Color::WHITE,
                ..iced::widget::button::Style::default()
            }),
        ]
        .align_y(iced::Alignment::Center);

        let card = container(
            column![
                container(
                    row![
                        text("Pick Net Color").size(13).color(text_c),
                        Space::new().width(iced::Length::Fill),
                        self.view_close_x(Message::NetColorCustomShow(false)),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 14])
                .style(crate::styles::toolbar_strip(
                    &self.document_state.panel_ctx.tokens
                )),
                container(body).padding([14, 14]),
                container(footer).padding([10, 14]),
            ]
            .width(iced::Length::Fixed(430.0)),
        )
        .style(crate::styles::context_menu(
            &self.document_state.panel_ctx.tokens,
        ));

        // Anchor below the Active Bar Net Color button (rightmost icon).
        let (ww, _wh) = self.ui_state.window_size;
        let card_w = 430.0;
        let x = ((ww - card_w) * 0.5).max(0.0);
        let y = crate::menu_bar::MENU_BAR_HEIGHT
            + if self.document_state.tabs.is_empty() {
                0.0
            } else {
                28.0
            }
            + 80.0;
        // Wrap in a mouse_area with on_press(Noop) so clicks inside the
        // card are captured and DON'T fall through to the dismiss
        // layer sitting beneath. Without this, clicking on the card's
        // background / between swatches closes the picker.
        let card_capturing = iced::widget::mouse_area(card).on_press(Message::Noop);
        super::view::translate::Translate::new(card_capturing, (x, y)).into()
    }

    pub(super) fn view_detached_modal(&self, modal: super::state::ModalId) -> Element<'_, Message> {
        use super::state::ModalId;
        match modal {
            ModalId::AnnotateDialog => self.view_annotate_dialog_body(),
            ModalId::ErcDialog => self.view_erc_dialog_body(),
            ModalId::AnnotateResetConfirm => self.view_annotate_reset_confirm_body(),
            // Stubs — these modals don't yet have extractable bodies; fall
            // back to a placeholder so the window is non-empty until their
            // body helpers land.
            ModalId::MoveSelection => self.view_move_selection_body(),
            ModalId::NetColorPalette => self.view_net_color_palette_body(),
            ModalId::ParameterManager => self.view_parameter_manager_body(),
            ModalId::PrintPreview => self.view_print_preview_body(),
            ModalId::BomPreview => {
                // Stack the body underneath a 6 px edge-resize
                // overlay so the borderless OS window can be
                // resized by dragging its edges. Without this,
                // `decorations: false` strips the OS frame and
                // there's nothing to grab.
                let body = self.view_bom_preview_body();
                let resize_active = self
                    .document_state
                    .bom_preview
                    .as_ref()
                    .map(|p| p.column_resize.is_some())
                    .unwrap_or(false);
                let mut stack = iced::widget::Stack::new()
                    .push(body)
                    .push(Self::detached_modal_resize_overlay(modal));
                // While a column-resize drag is in flight, lay an
                // invisible mouse_area over the whole modal that
                // pins the cursor to ResizingHorizontally. Without
                // this, the cursor reverts to default the moment
                // it leaves the 4 px handle's hit zone — which
                // happens immediately on horizontal drag.
                if resize_active {
                    let overlay: Element<'_, Message> = iced::widget::mouse_area(
                        iced::widget::Space::new()
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .on_release(Message::BomPreviewColumnResizeEnd)
                    .interaction(iced::mouse::Interaction::ResizingHorizontally)
                    .into();
                    stack = stack.push(overlay);
                }
                stack.into()
            }
            ModalId::Preferences
            | ModalId::FindReplace
            | ModalId::RenameDialog
            | ModalId::RemoveDialog => {
                iced::widget::container(iced::widget::text("Detached modal"))
                    .padding(20)
                    .into()
            }
        }
    }

    /// Same 6 px edge-resize overlay as the main window's, but
    /// emitting `StartDetachedModalResize { modal, direction }`
    /// so it dispatches to the right OS window. Used as a stack
    /// layer above the modal's body in `view_detached_modal`.
    fn detached_modal_resize_overlay<'a>(
        modal: super::state::ModalId,
    ) -> Element<'a, Message> {
        use iced::mouse::Interaction;
        use iced::widget::{Space, column, mouse_area, row};
        use iced::window::Direction;

        const EDGE: f32 = 6.0;

        let straight =
            move |direction: Direction, cursor: Interaction, horizontal: bool|
                -> Element<'a, Message> {
                let (w, h) = if horizontal {
                    (Length::Fill, Length::Fixed(EDGE))
                } else {
                    (Length::Fixed(EDGE), Length::Fill)
                };
                mouse_area(Space::new().width(w).height(h))
                    .on_press(Message::StartDetachedModalResize { modal, direction })
                    .interaction(cursor)
                    .into()
            };
        let corner = move |direction: Direction, cursor: Interaction| -> Element<'a, Message> {
            mouse_area(
                Space::new()
                    .width(Length::Fixed(EDGE))
                    .height(Length::Fixed(EDGE)),
            )
            .on_press(Message::StartDetachedModalResize { modal, direction })
            .interaction(cursor)
            .into()
        };

        let top = straight(Direction::North, Interaction::ResizingVertically, true);
        let bottom = straight(Direction::South, Interaction::ResizingVertically, true);
        let left = straight(Direction::West, Interaction::ResizingHorizontally, false);
        let right = straight(Direction::East, Interaction::ResizingHorizontally, false);
        let nw = corner(Direction::NorthWest, Interaction::ResizingDiagonallyDown);
        let ne = corner(Direction::NorthEast, Interaction::ResizingDiagonallyUp);
        let sw = corner(Direction::SouthWest, Interaction::ResizingDiagonallyUp);
        let se = corner(Direction::SouthEast, Interaction::ResizingDiagonallyDown);

        let middle = row![
            left,
            Space::new().width(Length::Fill).height(Length::Fill),
            right
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        column![
            row![nw, top, ne]
                .width(Length::Fill)
                .height(Length::Fixed(EDGE)),
            middle,
            row![sw, bottom, se]
                .width(Length::Fill)
                .height(Length::Fixed(EDGE)),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

}
