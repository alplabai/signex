use iced::widget::{canvas, column, container, row};
use iced::{Element, Length};

use super::*;

impl Signex {
    #[allow(clippy::vec_init_then_push)]
    fn view_context_menu(&self) -> Element<'_, Message> {
        let mut items: Vec<Element<'_, Message>> = Vec::with_capacity(20);
        let canvas = &self.interaction_state.canvas;
        let panel_ctx = &self.document_state.panel_ctx;

        items.push(self.ctx_menu_item_disabled("Find Similar Objects...", None));
        items.push(self.ctx_menu_item_disabled("Find Text...", Some("Ctrl+F")));
        items.push(self.ctx_menu_item_disabled("Clear Filter", Some("Shift+C")));
        items.push(self.ctx_menu_sep());
        items.push(self.ctx_menu_item_disabled("Place", Some("\u{25B6}")));
        items.push(self.ctx_menu_item_disabled("Part Actions", Some("\u{25B6}")));
        items.push(self.ctx_menu_item_disabled("Sheet Actions", Some("\u{25B6}")));

        if !canvas.selected.is_empty() {
            items.push(self.ctx_menu_item_disabled("References", Some("\u{25B6}")));
            items.push(self.ctx_menu_item_disabled("Align", Some("\u{25B6}")));
            items.push(self.ctx_menu_item_disabled("Unions", Some("\u{25B6}")));
            items.push(self.ctx_menu_item_disabled("Snippets", Some("\u{25B6}")));
        }

        items.push(self.ctx_menu_item_disabled("Cross Probe", None));
        items.push(self.ctx_menu_sep());
        items.push(self.ctx_menu_item_kb("Cut", "Ctrl+X", ContextAction::Cut));
        items.push(self.ctx_menu_item_kb("Copy", "Ctrl+C", ContextAction::Copy));
        items.push(self.ctx_menu_item_kb("Paste", "Ctrl+V", ContextAction::Paste));
        items.push(self.ctx_menu_item_kb(
            "Smart Paste",
            "Shift+Ctrl+V",
            ContextAction::SmartPaste,
        ));
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
        items.push(self.ctx_menu_item_disabled("Preferences...", None));

        if !canvas.selected.is_empty() {
            items.push(self.ctx_menu_item_disabled("Supplier Links...", None));
            items.push(self.ctx_menu_item_disabled("Properties...", None));
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
        .style(move |_: &iced::Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => {
                    Some(iced::Background::Color(hover_c))
                }
                _ => None,
            };
            iced::widget::button::Style {
                background: bg,
                border: iced::Border::default(),
                text_color: text_c,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    }

    fn ctx_menu_item_disabled<'a>(&self, label: &str, right: Option<&str>) -> Element<'a, Message> {
        let text_secondary =
            crate::styles::ti(self.document_state.panel_ctx.tokens.text_secondary);
        let mut row = iced::widget::row![
            iced::widget::text(label.to_string())
                .size(11)
                .color(text_secondary),
            iced::widget::Space::new().width(Length::Fill),
        ]
        .spacing(12)
        .width(Length::Fill);

        if let Some(right_text) = right {
            row = row.push(
                iced::widget::text(right_text.to_string())
                    .size(10)
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

    pub fn view(&self) -> Element<'_, Message> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let menu = menu_bar::view(&document.panel_ctx.tokens).map(Message::Menu);

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
        if !document.tabs.is_empty() {
            main = main.push(
                tab_bar::view(&document.tabs, document.active_tab, &document.panel_ctx.tokens)
                    .map(Message::Tab),
            );
        }
        let main = main.push(center_row).push(bottom_handle).push(bottom).push(status);

        let has_active_bar = self.has_active_schematic();
        let needs_overlay = has_active_bar
            || interaction.editing_text.is_some()
            || interaction.context_menu.is_some()
            || interaction.active_bar_menu.is_some()
            || ui.panel_list_open
            || ui.find_replace.open
            || ui.preferences_open
            || !document.dock.floating.is_empty();

        if needs_overlay {
            let overlays = self.collect_overlays();
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
        let width = if !has_panels { 0.0 } else if collapsed { 28.0 } else { size };
        container(panel)
            .width(width)
            .height(Length::Fill)
            .style(crate::styles::panel_region(&self.document_state.panel_ctx.tokens))
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
        let height = if !has_panels { 0.0 } else if collapsed { 28.0 } else { size };
        container(panel)
            .width(Length::Fill)
            .height(height)
            .style(crate::styles::panel_region(&self.document_state.panel_ctx.tokens))
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
                .style(crate::styles::resize_handle(&self.document_state.panel_ctx.tokens))
        } else {
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(size)
                .style(crate::styles::resize_handle(&self.document_state.panel_ctx.tokens))
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
                        .color(crate::styles::ti(self.document_state.panel_ctx.tokens.text_secondary)),
                    iced::widget::text("Open a project with File > Open or Ctrl+O")
                        .size(11)
                        .color(crate::styles::ti(self.document_state.panel_ctx.tokens.text_secondary)),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(&self.document_state.panel_ctx.tokens))
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

        if self.has_active_schematic() && let Some(ref edit_state) = interaction.editing_text {
            let text = edit_state.text.clone();
            layers.push(
                column![
                    iced::widget::Space::new().height(edit_state.screen_y - 12.0),
                    row![
                        iced::widget::Space::new().width(edit_state.screen_x - 4.0),
                        container(
                            iced::widget::text_input("", &text)
                                .on_input(Message::TextEditChanged)
                                .on_submit(Message::TextEditSubmit)
                                .size(13)
                                .padding([4, 6])
                                .width(180),
                        )
                        .style(crate::styles::context_menu(&document.panel_ctx.tokens)),
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
            // Bar: MENU_BAR_HEIGHT + tabs + 4 top-margin + ~28 bar-height ≈ bottom of bar.
            // Add a small gap so the dropdown visually touches the bar.
            let ab_y: f32 = crate::menu_bar::MENU_BAR_HEIGHT
                + if document.tabs.is_empty() { 0.0 } else { 28.0 }
                + 34.0;
            let bar_w: f32 = crate::active_bar::BAR_WIDTH_PX;
            let (ww, _) = ui.window_size;
            let adjusted_x = x_off + (ww - bar_w) / 2.0;

            layers.push(Self::dismiss_layer(Message::ActiveBar(
                crate::active_bar::ActiveBarMsg::CloseMenus,
            )));
            layers.push(
                container(column![
                    iced::widget::Space::new().height(ab_y),
                    container(row![iced::widget::Space::new().width(adjusted_x), dropdown])
                        .width(ww),
                ])
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .into(),
            );
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
            let has_sch = document.panel_ctx.has_schematic;
            let has_pcb = document.panel_ctx.has_pcb;
            let panel_items: Vec<Element<'_, Message>> = crate::panels::ALL_PANELS
                .iter()
                .filter(|&&kind| (!kind.needs_schematic() || has_sch) && (!kind.needs_pcb() || has_pcb))
                .map(|&kind| {
                    iced::widget::button(
                        iced::widget::text(kind.label().to_string())
                            .size(11)
                            .color(text_c),
                    )
                    .padding([4, 12])
                    .width(Length::Fill)
                    .on_press(Message::OpenPanel(kind))
                    .style(crate::styles::menu_item(&document.panel_ctx.tokens))
                    .into()
                })
                .collect();

            let popup = container(
                iced::widget::scrollable(column(panel_items).spacing(0).width(180)).height(300),
            )
            .padding([6, 0])
            .style(crate::styles::context_menu(&document.panel_ctx.tokens));

            layers.push(Self::dismiss_layer(Message::TogglePanelList));
            layers.push(
                container(
                    container(popup)
                        .align_x(iced::alignment::Horizontal::Right)
                        .align_y(iced::alignment::Vertical::Bottom)
                        .padding([15, 10]),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            );
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

        let (ww, wh) = ui.window_size;
        for i in 0..document.dock.floating.len() {
            if let Some(panel_widget) = document.dock.view_floating_panel(i, &document.panel_ctx) {
                let fp = &document.dock.floating[i];
                let max_x = (ww - fp.width).max(0.0);
                let px = fp.x.clamp(0.0, max_x);
                let py = fp.y.clamp(0.0, wh - 40.0).max(0.0);
                layers.push(
                    column![
                        iced::widget::Space::new().height(py),
                        row![
                            iced::widget::Space::new().width(px),
                            panel_widget.map(Message::Dock),
                        ],
                    ]
                    .into(),
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

        layers
    }
}