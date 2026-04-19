use iced::widget::{canvas, column, container, row};
use iced::{Element, Length};

mod dialogs;
mod translate;

use super::*;

impl Signex {
    #[allow(clippy::vec_init_then_push)]
    fn view_context_menu(&self) -> Element<'_, Message> {
        let mut items: Vec<Element<'_, Message>> = Vec::with_capacity(20);
        let canvas = &self.interaction_state.canvas;
        let panel_ctx = &self.document_state.panel_ctx;

        // Right-pointing angle quote (not the BLACK RIGHT-POINTING TRIANGLE
        // U+25B6, which Windows renders via the color emoji font).
        const SUBMENU_ARROW: &str = "›";

        items.push(self.ctx_menu_item_disabled("Find Similar Objects...", None));
        items.push(self.ctx_menu_item_msg("Find Text...", "Ctrl+F", Message::OpenFind));
        items.push(self.ctx_menu_item_disabled("Clear Filter", Some("Shift+C")));
        items.push(self.ctx_menu_sep());
        items.push(self.ctx_menu_item_disabled("Place", Some(SUBMENU_ARROW)));
        items.push(self.ctx_menu_item_disabled("Part Actions", Some(SUBMENU_ARROW)));
        items.push(self.ctx_menu_item_disabled("Sheet Actions", Some(SUBMENU_ARROW)));

        if !canvas.selected.is_empty() {
            items.push(self.ctx_menu_item_disabled("References", Some(SUBMENU_ARROW)));
            items.push(self.ctx_menu_item_disabled("Align", Some(SUBMENU_ARROW)));
            items.push(self.ctx_menu_item_disabled("Unions", Some(SUBMENU_ARROW)));
            items.push(self.ctx_menu_item_disabled("Snippets", Some(SUBMENU_ARROW)));
        }

        items.push(self.ctx_menu_item_disabled("Cross Probe", None));
        items.push(self.ctx_menu_sep());
        items.push(self.ctx_menu_item_kb("Cut", "Ctrl+X", ContextAction::Cut));
        items.push(self.ctx_menu_item_kb("Copy", "Ctrl+C", ContextAction::Copy));
        items.push(self.ctx_menu_item_kb("Paste", "Ctrl+V", ContextAction::Paste));
        items.push(self.ctx_menu_item_kb("Smart Paste", "Shift+Ctrl+V", ContextAction::SmartPaste));
        items.push(self.ctx_menu_sep());

        if !canvas.selected.is_empty() {
            items.push(self.ctx_menu_item_kb("Rotate", "Space", ContextAction::RotateSelected));
            items.push(self.ctx_menu_item_kb("Mirror X", "X", ContextAction::MirrorX));
            items.push(self.ctx_menu_item_kb("Mirror Y", "Y", ContextAction::MirrorY));
            items.push(self.ctx_menu_item_kb("Delete", "Del", ContextAction::Delete));
            items.push(self.ctx_menu_sep());
        }

        items.push(self.ctx_menu_item_disabled("Comment...", None));
        items.push(self.ctx_menu_item_disabled("Pin Mapping...", None));
        items.push(self.ctx_menu_item_disabled("Project Options...", None));
        items.push(self.ctx_menu_item_msg("Preferences...", "", Message::OpenPreferences));

        if !canvas.selected.is_empty() {
            items.push(self.ctx_menu_item_disabled("Supplier Links...", None));
            // Properties → ensure the Properties panel is visible. The
            // panel already tracks the current selection, so it populates
            // with the right-clicked item's fields once shown.
            items.push(self.ctx_menu_item_msg(
                "Properties...",
                "F11",
                Message::Menu(menu_bar::MenuMessage::OpenPropertiesPanel),
            ));
        }

        container(column(items).spacing(0).width(Self::CONTEXT_MENU_WIDTH))
            .padding([4, 0])
            .style(crate::styles::context_menu(&panel_ctx.tokens))
            .into()
    }

    fn ctx_menu_item_kb<'a>(
        &self,
        label: &str,
        shortcut: &str,
        action: ContextAction,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let hover_c = crate::styles::ti(tokens.hover);
        iced::widget::button(
            iced::widget::row![
                iced::widget::text(label.to_string()).size(11).color(text_c),
                iced::widget::Space::new().width(Length::Fill),
                iced::widget::text(shortcut.to_string())
                    .size(10)
                    .color(crate::styles::ti(tokens.text_secondary)),
            ]
            .spacing(12)
            .width(Length::Fill),
        )
        .width(Self::CONTEXT_MENU_WIDTH)
        .padding([4, 12])
        .on_press(Message::ContextAction(action))
        .style(
            move |_: &iced::Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(hover_c)),
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    border: iced::Border::default(),
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }
            },
        )
        .into()
    }

    fn ctx_menu_item_msg<'a>(
        &self,
        label: &str,
        shortcut: &str,
        message: Message,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let hover_c = crate::styles::ti(tokens.hover);
        iced::widget::button(
            iced::widget::row![
                iced::widget::text(label.to_string()).size(11).color(text_c),
                iced::widget::Space::new().width(Length::Fill),
                iced::widget::text(shortcut.to_string())
                    .size(10)
                    .color(crate::styles::ti(tokens.text_secondary)),
            ]
            .spacing(12)
            .width(Length::Fill),
        )
        .width(Self::CONTEXT_MENU_WIDTH)
        .padding([4, 12])
        .on_press(message)
        .style(
            move |_: &iced::Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(hover_c)),
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    border: iced::Border::default(),
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }
            },
        )
        .into()
    }

    fn ctx_menu_item_disabled<'a>(&self, label: &str, right: Option<&str>) -> Element<'a, Message> {
        let text_secondary = crate::styles::ti(self.document_state.panel_ctx.tokens.text_secondary);
        let mut row = iced::widget::row![
            iced::widget::text(label.to_string())
                .size(11)
                .color(text_secondary),
            iced::widget::Space::new().width(Length::Fill),
        ]
        .spacing(12)
        .width(Length::Fill);

        if let Some(right_text) = right {
            // Submenu/shortcut column. Bigger than the label so the arrow
            // (›) is readable at a glance. Non-emoji glyph so Windows does
            // not render it through the color emoji font.
            row = row.push(
                iced::widget::text(right_text.to_string())
                    .size(14)
                    .color(text_secondary),
            );
        }

        container(row)
            .padding([4, 12])
            .width(Self::CONTEXT_MENU_WIDTH)
            .into()
    }

    fn ctx_menu_sep<'a>(&self) -> Element<'a, Message> {
        let border_c = crate::styles::ti(self.document_state.panel_ctx.tokens.border);
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(1)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(border_c.into()),
                ..container::Style::default()
            })
            .into()
    }

    pub fn view(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        // Secondary windows (detached modals, future undocked tabs) render
        // just their own content — no menu / dock / canvas. The main
        // window's view_main drops any overlay whose modal is currently
        // detached so we don't double-render.
        if let Some(kind) = self.ui_state.windows.get(&window_id) {
            return match kind {
                super::state::WindowKind::DetachedModal(modal) => self.view_detached_modal(*modal),
                // Undocked tab = full duplicate of the main app view.
                // Shared Signex state means edits sync automatically; the
                // only difference between main and undocked is the OS
                // window id they render into.
                super::state::WindowKind::UndockedTab { .. } => self.view_main_for(Some(window_id)),
                super::state::WindowKind::DetachedPanel(kind) => {
                    let panel = crate::panels::view_panel(*kind, &self.document_state.panel_ctx)
                        .map(crate::dock::DockMessage::Panel)
                        .map(Message::Dock);
                    iced::widget::container(iced::widget::scrollable(panel))
                        .padding(8)
                        .into()
                }
            };
        }
        self.view_main_for(None)
    }

    /// Cursor-following translucent preview of a tab being dragged.
    /// Shape matches the real tab bar entry — rounded container with
    /// the title text, the ↗ undock indicator, and the × close icon —
    /// so it reads as "the tab itself is moving". The ghost is
    /// non-interactive; it just shows what the user is carrying.
    fn view_tab_drag_ghost(&self, title: &str) -> Element<'_, Message> {
        use iced::widget::{Space, container, row, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border = crate::styles::ti(tokens.border);
        let active_bg = crate::styles::ti(tokens.hover);
        // Slight translucency so the user still sees the tab bar
        // underneath — reads as "ghost" rather than a solid widget.
        let bg = iced::Color {
            a: 0.88,
            ..active_bg
        };
        let tab_like = container(
            row![
                text(title.to_string()).size(11).color(text_c),
                Space::new().width(6),
                text("\u{2197}".to_string()).size(12).color(text_muted),
                Space::new().width(4),
                text("\u{00D7}".to_string()).size(14).color(text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 10])
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border {
                width: 1.0,
                radius: 0.0.into(),
                color: border,
            },
            text_color: Some(text_c),
            ..container::Style::default()
        });
        // Anchor near the cursor (right + below) so the pointer
        // remains visible while the ghost trails it.
        let (cx, cy) = self.interaction_state.last_mouse_pos;
        super::view::translate::Translate::new(tab_like, (cx + 10.0, cy + 6.0)).into()
    }

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
        let selection_count = self.interaction_state.canvas.selected.len();

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
            .style(crate::styles::toolbar_strip(tokens)),
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
        .style(crate::styles::context_menu(tokens))
        .into()
    }

    /// Compact X close button shared by the new v0.7.1 detached bodies.
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
    /// color picker. v0.7.1 ships this with a small palette of preset
    /// colors (10 swatches); a full ColorPicker widget can replace it
    /// later without changing the message contract.
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
            .style(crate::styles::toolbar_strip(tokens)),
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
        .style(crate::styles::context_menu(tokens))
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
            .style(crate::styles::toolbar_strip(tokens)),
        )
        .on_press(Message::StartDetachedWindowDrag(
            super::state::ModalId::ParameterManager,
        ))
        .interaction(iced::mouse::Interaction::Grab);

        // Collect all parameter keys across symbols (besides the built-
        // in reference / value / footprint). Keeps the table compact —
        // only columns that someone actually uses show up.
        let Some(engine) = self.document_state.engine.as_ref() else {
            return container(
                column![
                    header,
                    container(text("No active schematic.").size(11).color(text_muted))
                        .padding([14, 14]),
                ]
                .width(iced::Length::Fixed(900.0))
                .height(iced::Length::Fixed(560.0)),
            )
            .style(crate::styles::context_menu(tokens))
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
        .style(crate::styles::context_menu(tokens))
        .into()
    }

    /// Custom net-colour picker modal. Grid of quick-pick swatches on
    /// the left, precise R / G / B / hex on the right, live preview
    /// and OK / Cancel at the bottom. Ships with a 24-color palette
    /// matching the common Altium net-colour presets plus a handful of
    /// EDA-specific diagnostic colours.
    fn view_net_color_custom_picker(&self) -> Element<'_, Message> {
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

    fn view_detached_modal(&self, modal: super::state::ModalId) -> Element<'_, Message> {
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
            ModalId::Preferences | ModalId::FindReplace | ModalId::CloseTabConfirm => {
                iced::widget::container(iced::widget::text("Detached modal"))
                    .padding(20)
                    .into()
            }
        }
    }

    fn view_main_for(&self, undocked_window: Option<iced::window::Id>) -> Element<'_, Message> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        // Context-aware menu: each leaf gates on whether its action
        // makes sense in the current app state. `has_schematic` /
        // `has_selection` drive most entries; undo / redo consult
        // the engine's history so they grey out when empty.
        let menu_ctx = crate::menu_bar::MenuContext {
            has_schematic: self.has_active_schematic(),
            has_pcb: self.has_active_pcb(),
            has_project: document.project_path.is_some(),
            has_selection: !interaction.canvas.selected.is_empty(),
            can_undo: document
                .engine
                .as_ref()
                .map(|e| e.can_undo())
                .unwrap_or(false),
            can_redo: document
                .engine
                .as_ref()
                .map(|e| e.can_redo())
                .unwrap_or(false),
        };
        let menu = menu_bar::view(&document.panel_ctx.tokens, menu_ctx).map(Message::Menu);

        let left_has_panels = document.dock.has_panels(PanelPosition::Left);
        let right_has_panels = document.dock.has_panels(PanelPosition::Right);
        let bottom_has_panels = document.dock.has_panels(PanelPosition::Bottom);
        let left_collapsed = document.dock.is_collapsed(PanelPosition::Left);
        let right_collapsed = document.dock.is_collapsed(PanelPosition::Right);
        let bottom_collapsed = document.dock.is_collapsed(PanelPosition::Bottom);

        let left = self.view_dock_panel(
            PanelPosition::Left,
            left_has_panels,
            left_collapsed,
            ui.left_width,
        );
        let left_handle = self.view_resize_handle(
            DragTarget::LeftPanel,
            left_has_panels && !left_collapsed,
            true,
        );
        let center = self.view_center();
        let right_handle = self.view_resize_handle(
            DragTarget::RightPanel,
            right_has_panels && !right_collapsed,
            true,
        );
        let right = self.view_dock_panel(
            PanelPosition::Right,
            right_has_panels,
            right_collapsed,
            ui.right_width,
        );

        let center_row = row![left, left_handle, center, right_handle, right];
        let bottom_handle = self.view_resize_handle(
            DragTarget::BottomPanel,
            bottom_has_panels && !bottom_collapsed,
            false,
        );
        let bottom = self.view_dock_panel_h(
            PanelPosition::Bottom,
            bottom_has_panels,
            bottom_collapsed,
            ui.bottom_height,
        );

        let status = status_bar::view(
            ui.cursor_x,
            ui.cursor_y,
            ui.grid_visible,
            ui.snap_enabled,
            ui.zoom,
            ui.unit,
            &interaction.current_tool,
            ui.grid_size_mm,
            &document.panel_ctx.tokens,
        )
        .map(Message::StatusBar);

        let mut main = column![menu];
        // Partition tabs across windows: main owns every tab that isn't
        // currently rendered by an undocked-tab window; each undocked
        // window owns exactly its one tab. Closing a tab in one window
        // can no longer reach tabs that belong to the other.
        let all_undocked_paths: std::collections::HashSet<std::path::PathBuf> = ui
            .windows
            .values()
            .filter_map(|kind| match kind {
                super::state::WindowKind::UndockedTab { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        let visible_paths: std::collections::HashSet<std::path::PathBuf> = match undocked_window {
            Some(id) => match ui.windows.get(&id) {
                Some(super::state::WindowKind::UndockedTab { path, .. }) => {
                    std::iter::once(path.clone()).collect()
                }
                _ => std::collections::HashSet::new(),
            },
            None => document
                .tabs
                .iter()
                .map(|t| t.path.clone())
                .filter(|p| !all_undocked_paths.contains(p))
                .collect(),
        };
        if !document.tabs.is_empty() && !visible_paths.is_empty() {
            let dragging = ui.tab_dragging.map(|(idx, _, _)| idx);
            main = main.push(
                tab_bar::view(
                    &document.tabs,
                    document.active_tab,
                    dragging,
                    &visible_paths,
                    &document.panel_ctx.tokens,
                )
                .map(Message::Tab),
            );
        }
        let main = main
            .push(center_row)
            .push(bottom_handle)
            .push(bottom)
            .push(status);

        let has_active_bar = self.has_active_schematic();
        let dragging_tab = ui.tab_dragging.is_some();
        let needs_overlay = has_active_bar
            || interaction.editing_text.is_some()
            || interaction.context_menu.is_some()
            || interaction.active_bar_menu.is_some()
            || interaction.canvas.placement_paused
            || ui.panel_list_open
            || ui.find_replace.open
            || ui.preferences_open
            || ui.close_tab_confirm.is_some()
            || ui.annotate_dialog_open
            || ui.annotate_reset_confirm
            || ui.erc_dialog_open
            || !document.dock.floating.is_empty()
            || dragging_tab
            || ui.net_color_custom.show;

        if needs_overlay {
            let mut overlays = self.collect_overlays();
            // Tab drag ghost: while a tab is being dragged, follow the
            // cursor with a translucent copy of the tab label (Altium
            // parity — see the user's screenshot). Gives direct visual
            // feedback that the tab is being moved and will drop
            // wherever the user releases, including into a new window
            // past the edge.
            if let Some((tab_idx, _, _)) = ui.tab_dragging
                && let Some(tab) = document.tabs.get(tab_idx)
            {
                overlays.push(self.view_tab_drag_ghost(&tab.title));
            }
            let mut stack = iced::widget::Stack::new().push(main);
            for overlay in overlays {
                stack = stack.push(overlay);
            }
            stack.into()
        } else {
            main.into()
        }
    }

    fn view_dock_panel(
        &self,
        pos: PanelPosition,
        has_panels: bool,
        collapsed: bool,
        size: f32,
    ) -> Element<'_, Message> {
        let panel = self
            .document_state
            .dock
            .view_region(pos, &self.document_state.panel_ctx)
            .map(Message::Dock);
        let width = if !has_panels {
            0.0
        } else if collapsed {
            28.0
        } else {
            size
        };
        container(panel)
            .width(width)
            .height(Length::Fill)
            .style(crate::styles::panel_region(
                &self.document_state.panel_ctx.tokens,
            ))
            .into()
    }

    fn view_dock_panel_h(
        &self,
        pos: PanelPosition,
        has_panels: bool,
        collapsed: bool,
        size: f32,
    ) -> Element<'_, Message> {
        let panel = self
            .document_state
            .dock
            .view_region(pos, &self.document_state.panel_ctx)
            .map(Message::Dock);
        let height = if !has_panels {
            0.0
        } else if collapsed {
            28.0
        } else {
            size
        };
        container(panel)
            .width(Length::Fill)
            .height(height)
            .style(crate::styles::panel_region(
                &self.document_state.panel_ctx.tokens,
            ))
            .into()
    }

    fn view_resize_handle(
        &self,
        target: DragTarget,
        visible: bool,
        horizontal: bool,
    ) -> Element<'_, Message> {
        let size = if visible { 5 } else { 0 };
        let handle_container = if horizontal {
            container(iced::widget::Space::new())
                .width(size)
                .height(Length::Fill)
                .style(crate::styles::resize_handle(
                    &self.document_state.panel_ctx.tokens,
                ))
        } else {
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(size)
                .style(crate::styles::resize_handle(
                    &self.document_state.panel_ctx.tokens,
                ))
        };
        let interaction = if horizontal {
            iced::mouse::Interaction::ResizingHorizontally
        } else {
            iced::mouse::Interaction::ResizingVertically
        };
        iced::widget::mouse_area(handle_container)
            .interaction(interaction)
            .on_press(Message::DragStart(target))
            .into()
    }

    fn view_center(&self) -> Element<'_, Message> {
        if self.has_active_schematic() {
            canvas(&self.interaction_state.canvas)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else if self.has_active_pcb() {
            canvas(&self.interaction_state.pcb_canvas)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            container(
                column![
                    iced::widget::text("No document open")
                        .size(14)
                        .color(crate::styles::ti(
                            self.document_state.panel_ctx.tokens.text_secondary
                        )),
                    iced::widget::text("Open a project with File > Open or Ctrl+O")
                        .size(11)
                        .color(crate::styles::ti(
                            self.document_state.panel_ctx.tokens.text_secondary
                        )),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(
                &self.document_state.panel_ctx.tokens,
            ))
            .into()
        }
    }

    fn dismiss_layer(on_press: Message) -> Element<'static, Message> {
        iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(on_press)
        .into()
    }

    fn collect_overlays(&self) -> Vec<Element<'_, Message>> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let mut layers = Vec::new();

        // Custom net-colour picker. Bespoke modal (not the iced_aw
        // ColorPicker) because the user needs a quick-pick palette +
        // precise RGB inputs side-by-side.
        if ui.net_color_custom.show {
            layers.push(Self::dismiss_layer(Message::NetColorCustomShow(false)));
            layers.push(self.view_net_color_custom_picker());
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
            let bar = crate::active_bar::view_bar(
                interaction.current_tool,
                interaction.draw_mode,
                &interaction.last_tool,
                &document.panel_ctx.tokens,
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
            let canvas_local_x = edit_state.world_x as f32 * cam_scale + cam_off_x;
            let canvas_local_y = edit_state.world_y as f32 * cam_scale + cam_off_y;
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
            // Font size in pixels matches the rendered label (10 pt ≈ 1.8 mm).
            let font_px = (cam_scale * 1.8).clamp(10.0, 64.0);
            // Estimate width from text length to keep the input snug.
            let approx_w =
                ((edit_state.text.chars().count() as f32 + 2.0) * font_px * 0.62).max(60.0);
            // Offset the input so the baseline sits on top of the label text.
            let abs_x = x_canvas_origin + canvas_local_x - 2.0;
            let abs_y = y_canvas_origin + canvas_local_y - font_px - 2.0;
            let paper_c = crate::styles::ti(document.panel_ctx.tokens.paper);
            let text_c = crate::styles::ti(document.panel_ctx.tokens.text);
            let accent_c = crate::styles::ti(document.panel_ctx.tokens.accent);
            layers.push(
                column![
                    iced::widget::Space::new().height(abs_y.max(0.0)),
                    row![
                        iced::widget::Space::new().width(abs_x.max(0.0)),
                        container(
                            iced::widget::text_input("", &text)
                                .on_input(Message::TextEditChanged)
                                .on_submit(Message::TextEditSubmit)
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
            );
        }

        if let Some(ab_menu) = interaction.active_bar_menu {
            let dropdown = crate::active_bar::view_dropdown(
                ab_menu,
                &document.panel_ctx.tokens,
                &interaction.selection_filters,
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
            layers.push(Self::dismiss_layer(Message::CloseContextMenu));
            layers.push(
                column![
                    iced::widget::Space::new().height(ctx_menu.y),
                    row![
                        iced::widget::Space::new().width(ctx_menu.x),
                        menu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .into(),
            );
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
                ui.custom_theme.as_ref().map(|c| c.name.as_str()),
                ui.preferences_dirty,
            )
            .map(Message::PreferencesMsg);
            layers.push(pref_view);
        }

        if ui.find_replace.open {
            let dialog = crate::find_replace::view(&ui.find_replace, &document.panel_ctx.tokens)
                .map(Message::FindReplaceMsg);
            layers.push(dialog);
        }

        if let Some(idx) = ui.close_tab_confirm
            && let Some(tab) = document.tabs.get(idx)
        {
            layers.push(self.view_close_tab_confirm(&tab.title));
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

    fn view_close_tab_confirm(&self, tab_title: &str) -> Element<'_, Message> {
        use iced::widget::{Space, button, text};
        use iced::{Background, Border, Color, Theme};

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let message = format!(
            "'{}' has unsaved changes. Do you want to save before closing?",
            tab_title,
        );

        let btn = |label: &'static str, msg: Message, primary: bool| -> Element<'_, Message> {
            let label_color = if primary { Color::WHITE } else { text_c };
            let bg = if primary {
                Color::from_rgb(0.00, 0.47, 0.84)
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.04)
            };
            button(container(text(label.to_string()).size(12).color(label_color)).padding([5, 14]))
                .on_press(msg)
                .style(move |_: &Theme, _| iced::widget::button::Style {
                    background: Some(Background::Color(bg)),
                    border: Border {
                        width: 1.0,
                        radius: 4.0.into(),
                        color: border_c,
                    },
                    text_color: label_color,
                    ..iced::widget::button::Style::default()
                })
                .into()
        };

        let dialog = container(
            column![
                container(text("Unsaved Changes").size(14).color(text_c))
                    .padding([10, 14])
                    .style(crate::styles::toolbar_strip(tokens)),
                container(text(message).size(11).color(text_muted)).padding([14, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        btn(
                            "Cancel",
                            Message::CloseTabConfirm(CloseTabChoice::Cancel),
                            false,
                        ),
                        btn(
                            "Don't Save",
                            Message::CloseTabConfirm(CloseTabChoice::DiscardAndClose),
                            false,
                        ),
                        btn(
                            "Save",
                            Message::CloseTabConfirm(CloseTabChoice::SaveAndClose),
                            true,
                        ),
                    ]
                    .spacing(8),
                )
                .padding([10, 14]),
            ]
            .width(420),
        )
        .style(crate::styles::context_menu(tokens));

        container(
            column![
                Space::new().height(Length::Fill),
                row![
                    Space::new().width(Length::Fill),
                    dialog,
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
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.55))),
            ..container::Style::default()
        })
        .into()
    }
}
