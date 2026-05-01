use iced::{Element, Length};

use super::*;

impl Signex {
    /// Export-error modal — plain "something went wrong, here's the
    /// message" dialog with an OK button. Sits on top of the print-preview
    /// overlay when both would otherwise render; dismiss_layer handles
    /// click-outside-to-close.
    pub(super) fn view_export_error(&self) -> Element<'_, Message> {
        use iced::widget::{button, column, container, row, text};
        let msg = match &self.document_state.export_error {
            Some(m) => m.clone(),
            None => return iced::widget::Space::new().into(),
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let text_c = crate::styles::ti(tokens.text);
        let border_c = crate::styles::ti(tokens.border);
        let err_red = iced::Color::from_rgb(0.85, 0.25, 0.25);

        let ok_btn = button(text("OK").size(12).color(iced::Color::WHITE))
            .padding([6, 20])
            .on_press(Message::DismissExportError)
            .style(
                move |_: &iced::Theme, _status| iced::widget::button::Style {
                    background: Some(err_red.into()),
                    text_color: iced::Color::WHITE,
                    border: iced::Border {
                        radius: iced::border::Radius::from(4.0),
                        ..iced::Border::default()
                    },
                    ..iced::widget::button::Style::default()
                },
            );

        let body = column![
            row![
                text("\u{26A0}").size(24).color(err_red),
                iced::widget::Space::new().width(10),
                text("Export Failed").size(14).color(text_c),
            ]
            .align_y(iced::Alignment::Center),
            iced::widget::Space::new().height(8),
            text(msg).size(12).color(text_c),
            iced::widget::Space::new().height(12),
            row![iced::widget::Space::new().width(Length::Fill), ok_btn,],
        ]
        .padding(20);

        let card = container(body)
            .max_width(480)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(panel_bg.into()),
                border: iced::Border {
                    color: border_c,
                    width: 1.0,
                    radius: iced::border::Radius::from(8.0),
                },
                shadow: iced::Shadow {
                    color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                ..container::Style::default()
            });

        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into()
    }

    /// Print Preview overlay. Shows thumbnails of every rendered page on
    /// the left, the selected page full-size on the right, with Export PDF
    /// and Close buttons at the bottom. Triggered by File → Print Preview
    /// (Ctrl+P) and File → Export PDF; disappears on Close or when the
    /// export completes. In-window flavour wraps the body in `wrap_modal`
    /// for backdrop + drag-to-position.
    pub(super) fn view_print_preview(&self) -> Element<'_, Message> {
        use crate::app::state::ModalId;
        use crate::app::view::dialogs::wrap_modal;
        let body = self.view_print_preview_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&ModalId::PrintPreview)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(
            body,
            offset,
            self.ui_state.window_size,
            (Self::PDF_MODAL_W, Self::PDF_MODAL_H),
        )
    }

    /// Detached-window flavour — bare body, no backdrop, no in-window
    /// drag handler (the OS window-drag covers the header).
    pub(super) fn view_print_preview_body(&self) -> Element<'_, Message> {
        self.view_print_preview_inner(false)
    }

    fn view_print_preview_inner(&self, draggable: bool) -> Element<'_, Message> {
        use crate::app::state::{ModalId, PdfPreviewTab};
        use crate::app::view::dialogs::{
            close_x_button, detached_header, draggable_header, MODAL_HEADER_HEIGHT,
            MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE,
        };
        use iced::widget::{button, column, container, row, text, Space};
        let theme_id = self.ui_state.theme_id;

        let preview = match &self.document_state.preview {
            Some(p) => p,
            None => return iced::widget::Space::new().into(),
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let accent_c = crate::styles::ti(tokens.accent);

        // Header — same chrome as every other modal.
        let header_content: Element<'_, Message> = container(
            row![
                text("Export PDF").size(MODAL_HEADER_TITLE_SIZE).color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::PrintPreviewClose, theme_id, text_muted),
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
                ModalId::PrintPreview,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            detached_header(header_content, ModalId::PrintPreview)
        };

        // Tab strip — Preview | Settings.
        let tab_strip = self.view_pdf_tab_strip(preview.active_tab);

        // Body switches by tab.
        let body: Element<'_, Message> = match preview.active_tab {
            PdfPreviewTab::Preview => self.view_pdf_preview_tab(preview),
            PdfPreviewTab::Settings => self.view_pdf_settings_tab(preview),
        };

        // Footer — page count + Export PDF.
        let export_btn = button(text("Export PDF").size(12).color(iced::Color::WHITE))
            .padding([6, 14])
            .on_press(Message::PrintPreviewExport)
            .style(
                move |_: &iced::Theme, _status| iced::widget::button::Style {
                    background: Some(accent_c.into()),
                    text_color: iced::Color::WHITE,
                    border: iced::Border {
                        radius: iced::border::Radius::from(4.0),
                        ..iced::Border::default()
                    },
                    ..iced::widget::button::Style::default()
                },
            );
        let footer_caption = if preview.pages.is_empty() {
            "No files selected for export".to_string()
        } else {
            format!("{} page(s) — preview at 96 DPI", preview.pages.len())
        };
        let footer = container(
            row![
                text(footer_caption).size(11).color(text_muted),
                Space::new().width(Length::Fill),
                export_btn,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding([10, 14]);

        let dialog = container(
            column![header, tab_strip, body, footer]
                .width(Self::PDF_MODAL_W)
                .height(Self::PDF_MODAL_H),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    /// Two-tab strip — Preview | Settings — sitting just under the
    /// modal header. Uses the same `TabPill` widget the document tab
    /// bar paints with: 3-sided border (top + L/R), accent stripe on
    /// the active tab, fill that fades for inactive. `is_last=true`
    /// on the rightmost so the trailing border doesn't double up
    /// against an adjacent tab's left edge.
    fn view_pdf_tab_strip(
        &self,
        active: crate::app::state::PdfPreviewTab,
    ) -> Element<'_, Message> {
        use crate::app::state::PdfPreviewTab;
        use iced::widget::{container, mouse_area, row, text, Space};
        use signex_widgets::tab_pill::{AccentPosition, TabPill, TabPillStyle};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let accent_c = crate::styles::ti(tokens.accent);
        let hover_c = crate::styles::ti(tokens.hover);

        let pill_fill = |is_active: bool| -> iced::Color {
            if is_active {
                hover_c
            } else {
                iced::Color {
                    a: hover_c.a * 0.35,
                    ..hover_c
                }
            }
        };

        let tab = |label: &'static str, this: PdfPreviewTab, is_last: bool| {
            let is_active = this == active;
            let label_color = if is_active { text_c } else { text_muted };
            let style = TabPillStyle {
                fill: pill_fill(is_active),
                border: border_c,
                accent: accent_c,
                is_active,
                is_last,
                accent_position: AccentPosition::Bottom,
            };
            let inner = container(text(label).size(12).color(label_color))
                .padding([6, 18]);
            mouse_area(TabPill::new(inner, style))
                .on_press(Message::PrintPreviewSetTab(this))
                .interaction(iced::mouse::Interaction::Pointer)
        };

        container(
            row![
                tab("Preview", PdfPreviewTab::Preview, false),
                tab("Settings", PdfPreviewTab::Settings, true),
                Space::new().width(Length::Fill),
            ]
            .spacing(0)
            .align_y(iced::Alignment::Center),
        )
        .padding([0, 14])
        .into()
    }

    /// Preview tab — top toolbar (Sheet/Colour/Pages/Output), thumb
    /// rail on the left, pan/zoom viewport on the right.
    fn view_pdf_preview_tab(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{
            button, checkbox, column, container, image, mouse_area, row, scrollable, text,
            text_input, Space,
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let accent_c = crate::styles::ti(tokens.accent);
        let hover_c = crate::styles::ti(tokens.hover);

        let mode_button = |label: &'static str, selected: bool, msg: Message| {
            let selected_bg = accent_c;
            let selected_text = iced::Color::WHITE;
            let default_bg = panel_bg;
            let default_text = text_c;
            button(text(label).size(11).color(if selected {
                selected_text
            } else {
                default_text
            }))
            .padding([4, 10])
            .on_press(msg)
            .style(
                move |_: &iced::Theme, _status| iced::widget::button::Style {
                    background: Some(if selected {
                        selected_bg.into()
                    } else {
                        default_bg.into()
                    }),
                    text_color: if selected {
                        selected_text
                    } else {
                        default_text
                    },
                    border: iced::Border {
                        color: border_c,
                        width: 1.0,
                        radius: iced::border::Radius::from(4.0),
                    },
                    ..iced::widget::button::Style::default()
                },
            )
        };

        let colour_controls = row![
            text("Colour").size(11).color(text_muted),
            mode_button(
                "Color",
                matches!(preview.pdf_options.colour_mode, signex_output::ColourMode::Colour),
                Message::PrintPreviewSetColourMode(signex_output::ColourMode::Colour),
            ),
            mode_button(
                "Gray",
                matches!(preview.pdf_options.colour_mode, signex_output::ColourMode::Grayscale),
                Message::PrintPreviewSetColourMode(signex_output::ColourMode::Grayscale),
            ),
            mode_button(
                "B/W",
                matches!(preview.pdf_options.colour_mode, signex_output::ColourMode::BlackAndWhite),
                Message::PrintPreviewSetColourMode(signex_output::ColourMode::BlackAndWhite),
            ),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let range_controls = row![
            text("Pages").size(11).color(text_muted),
            mode_button(
                "All",
                matches!(preview.pdf_options.page_range, signex_output::PageRange::All),
                Message::PrintPreviewSetPageRangeAll,
            ),
            mode_button(
                "Current",
                matches!(preview.pdf_options.page_range, signex_output::PageRange::Current),
                Message::PrintPreviewSetPageRangeCurrent,
            ),
            mode_button(
                "Specific",
                matches!(preview.pdf_options.page_range, signex_output::PageRange::Specific(_)),
                Message::PrintPreviewSetPageRangeSpecific,
            ),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let specific_page_input: Element<'_, Message> = if matches!(
            preview.pdf_options.page_range,
            signex_output::PageRange::Specific(_)
        ) {
            row![
                text("Page").size(11).color(text_muted),
                text_input("1", &preview.specific_page_input)
                    .on_input(Message::PrintPreviewSetSpecificPageInput)
                .padding([4, 8])
                .size(12)
                .width(80),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            iced::widget::Space::new().height(0).into()
        };

        let fit_to_page = matches!(
            preview.pdf_options.scale,
            signex_output::PdfScale::FitToPage
        );
        let toggles_row = row![
            text("Output").size(11).color(text_muted),
            row![
                checkbox(fit_to_page).on_toggle(Message::PrintPreviewSetFitToPage),
                text("Fit to Page").size(11).color(text_c),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
            row![
                checkbox(preview.pdf_options.include_title_block)
                    .on_toggle(Message::PrintPreviewSetIncludeTitleBlock),
                text("Title Block").size(11).color(text_c),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center);

        let page_size_label = match &preview.pdf_options.page_size {
            signex_output::PageSize::IsoA0 => "ISO A0",
            signex_output::PageSize::IsoA1 => "ISO A1",
            signex_output::PageSize::IsoA2 => "ISO A2",
            signex_output::PageSize::IsoA3 => "ISO A3",
            signex_output::PageSize::IsoA4 => "ISO A4",
            signex_output::PageSize::IsoA5 => "ISO A5",
            signex_output::PageSize::AnsiA => "ANSI A",
            signex_output::PageSize::AnsiB => "ANSI B",
            signex_output::PageSize::AnsiC => "ANSI C",
            signex_output::PageSize::AnsiD => "ANSI D",
            signex_output::PageSize::AnsiE => "ANSI E",
            signex_output::PageSize::UsLetter => "US Letter",
            signex_output::PageSize::UsLegal => "US Legal",
            signex_output::PageSize::Custom { .. } => "Custom",
        };
        let orientation_label = match preview.pdf_options.orientation {
            signex_output::Orientation::Portrait => "Portrait",
            signex_output::Orientation::Landscape => "Landscape",
        };
        let summary_row = row![
            text("Sheet").size(11).color(text_muted),
            text(format!("{} • {}", page_size_label, orientation_label))
                .size(11)
                .color(text_c),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let toolbar = container(
            row![
                summary_row,
                Space::new().width(16),
                colour_controls,
                Space::new().width(12),
                range_controls,
                specific_page_input,
                Space::new().width(Length::Fill),
                toggles_row,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([8, 14]);

        // Thumb rail.
        let mut thumbs: iced::widget::Column<'_, Message> = column![].spacing(4).padding(8);
        for (i, page) in preview.pages.iter().enumerate() {
            let selected = i == preview.selected;
            let thumb = image(preview.page_handles[i].clone())
                .content_fit(iced::ContentFit::Contain)
                .width(120)
                .height(85);
            let card_bg = if selected { hover_c } else { panel_bg };
            let card_border = if selected { accent_c } else { border_c };
            let card = container(
                column![
                    thumb,
                    text(format!("Page {}", page.page_number))
                        .size(10)
                        .color(text_c)
                ]
                .spacing(2)
                .align_x(iced::Alignment::Center),
            )
            .padding(4)
            .width(132)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(card_bg.into()),
                border: iced::Border {
                    color: card_border,
                    width: if selected { 2.0 } else { 1.0 },
                    radius: iced::border::Radius::from(4.0),
                },
                ..container::Style::default()
            });
            thumbs = thumbs.push(mouse_area(card).on_press(Message::PrintPreviewSelectPage(i)));
        }
        let thumb_rail = scrollable(thumbs).width(148).height(Length::Fill);

        // Pan/zoom viewport. The image is positioned via Translate so
        // pan delta moves it inside a clipped container — no
        // scrollbars, just drag-to-pan + wheel-zoom.
        let viewport: Element<'_, Message> = if preview.pages.is_empty() {
            container(
                text("No files selected — toggle files in Settings → Files.")
                    .size(12)
                    .color(text_muted),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Color::WHITE.into()),
                border: iced::Border {
                    color: border_c,
                    width: 1.0,
                    radius: iced::border::Radius::from(2.0),
                },
                ..container::Style::default()
            })
            .into()
        } else {
            let selected_page = &preview.pages[preview.selected];
            let zoom = preview.zoom;
            let scaled_w = (selected_page.width_px as f32 * zoom).max(64.0);
            let scaled_h = (selected_page.height_px as f32 * zoom).max(64.0);
            // At zoom ≤ 1 we want the page to fill the viewport
            // preserving aspect; above 1× we render at exact scaled
            // pixels and let the user pan around.
            let img_el: Element<'_, Message> = if zoom <= 1.0 {
                image(preview.page_handles[preview.selected].clone())
                    .content_fit(iced::ContentFit::Contain)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            } else {
                image(preview.page_handles[preview.selected].clone())
                    .content_fit(iced::ContentFit::Fill)
                    .width(Length::Fixed(scaled_w))
                    .height(Length::Fixed(scaled_h))
                    .into()
            };
            // Position the image at the pan offset. Below 1× the pan
            // is forced to (0, 0) (see the zoom handler) so the
            // translate is a no-op.
            let positioned: Element<'_, Message> = if zoom <= 1.0 {
                img_el
            } else {
                super::view::translate::Translate::new(img_el, preview.pan).into()
            };
            let surface = container(positioned)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(iced::Color::WHITE.into()),
                    border: iced::Border {
                        color: border_c,
                        width: 1.0,
                        radius: iced::border::Radius::from(2.0),
                    },
                    ..container::Style::default()
                })
                .clip(true);
            let interaction = if zoom > 1.0 {
                if preview.panning.is_some() {
                    iced::mouse::Interaction::Grabbing
                } else {
                    iced::mouse::Interaction::Grab
                }
            } else {
                iced::mouse::Interaction::default()
            };
            iced::widget::mouse_area(surface)
                .on_press(Message::PrintPreviewPanStart)
                .on_release(Message::PrintPreviewPanFinished)
                .on_scroll(|delta| {
                    use iced::mouse::ScrollDelta;
                    let dy = match delta {
                        ScrollDelta::Lines { y, .. } => y,
                        ScrollDelta::Pixels { y, .. } => y,
                    };
                    Message::PrintPreviewZoom(dy)
                })
                .interaction(interaction)
                .into()
        };

        let zoom = preview.zoom;
        let page_caption = if preview.pages.is_empty() {
            text("—".to_string()).size(11).color(text_muted)
        } else {
            let selected_page = &preview.pages[preview.selected];
            text(format!(
                "Page {} of {} — {}×{} px · {:.0}%",
                selected_page.page_number,
                preview.pages.len(),
                selected_page.width_px,
                selected_page.height_px,
                zoom * 100.0,
            ))
            .size(11)
            .color(text_muted)
        };

        let centre = column![viewport, page_caption]
            .spacing(6)
            .width(Length::Fill)
            .height(Length::Fill);

        let body_row = container(
            row![thumb_rail, Space::new().width(8), centre]
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .padding([0, 14])
        .width(Length::Fill)
        .height(Length::Fill);

        column![toolbar, body_row]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Settings tab — stitches the three section helpers below into a
    /// single scrollable column. Each helper owns its own widgets and
    /// reads/writes through `preview.pdf_options.*` directly so the
    /// rasterizer and exporter stay in lockstep with the UI.
    fn view_pdf_settings_tab(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{column, scrollable, Space};
        let body = column![
            self.view_pdf_files_section(preview),
            Space::new().height(10),
            self.view_pdf_structure_section(preview),
            Space::new().height(10),
            self.view_pdf_additional_section(preview),
        ]
        .spacing(0)
        .padding([10, 14]);
        scrollable(body)
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    }

    /// Section header strip — accent panel-bg with a 1 px border.
    /// Reused by every Settings-tab section.
    fn pdf_section_title(&self, label: &'static str) -> Element<'_, Message> {
        use iced::widget::{container, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let border_c = crate::styles::ti(tokens.border);
        container(text(label).size(12).color(text_c))
            .padding([6, 10])
            .width(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(panel_bg.into()),
                border: iced::Border {
                    color: border_c,
                    width: 1.0,
                    radius: iced::border::Radius::default(),
                },
                ..container::Style::default()
            })
            .into()
    }

    /// Settings → Choose Project Files.
    fn view_pdf_files_section(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{button, checkbox, column, container, row, scrollable, text, Space};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let panel_bg = crate::styles::ti(tokens.panel_bg);

        let project_sheets: Vec<(std::path::PathBuf, String)> = self
            .document_state
            .active_loaded_project()
            .map(|p| {
                let dir = std::path::PathBuf::from(&p.data.dir);
                p.data
                    .sheets
                    .iter()
                    .map(|s| (dir.join(&s.filename), s.name.clone()))
                    .collect()
            })
            .unwrap_or_default();

        let mut file_list: iced::widget::Column<'_, Message> =
            column![].spacing(2).padding([8, 12]);
        if project_sheets.is_empty() {
            file_list = file_list.push(
                text("No project loaded — load a .snxprj to pick files.")
                    .size(11)
                    .color(text_muted),
            );
        } else {
            for (path, name) in &project_sheets {
                let is_selected = preview.selected_files.contains(path);
                let path_str = path.display().to_string();
                let row_el = row![
                    checkbox(is_selected).on_toggle({
                        let path = path.clone();
                        move |_| Message::PrintPreviewToggleFile(path.clone())
                    }),
                    column![
                        text(name.clone()).size(11).color(text_c),
                        text(path_str).size(10).color(text_muted),
                    ]
                    .spacing(1),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center);
                file_list = file_list.push(row_el);
            }
        }
        let secondary_btn_style = move |_: &iced::Theme, _status| iced::widget::button::Style {
            background: Some(panel_bg.into()),
            text_color: text_c,
            border: iced::Border {
                color: border_c,
                width: 1.0,
                radius: iced::border::Radius::from(3.0),
            },
            ..iced::widget::button::Style::default()
        };
        let file_actions = row![
            button(text("Select All").size(11).color(text_c))
                .padding([3, 8])
                .on_press(Message::PrintPreviewSelectAllFiles)
                .style(secondary_btn_style),
            button(text("Clear").size(11).color(text_c))
                .padding([3, 8])
                .on_press(Message::PrintPreviewClearAllFiles)
                .style(secondary_btn_style),
        ]
        .spacing(6);

        column![
            self.pdf_section_title("Choose Project Files"),
            container(
                column![
                    text("Select the files in the project to export from the list. Multiple files can be selected.")
                        .size(11)
                        .color(text_muted),
                    Space::new().height(6),
                    container(scrollable(file_list).height(160))
                        .width(Length::Fill)
                        .style(move |_: &iced::Theme| container::Style {
                            border: iced::Border {
                                color: border_c,
                                width: 1.0,
                                radius: iced::border::Radius::default(),
                            },
                            ..container::Style::default()
                        }),
                    Space::new().height(6),
                    file_actions,
                ]
                .padding([10, 12]),
            ),
        ]
        .spacing(0)
        .into()
    }

    /// Settings → Structure Settings.
    fn view_pdf_structure_section(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{checkbox, column, container, row, text, Space};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let opts = &preview.pdf_options;

        let variant_label = opts
            .variant
            .clone()
            .unwrap_or_else(|| "Base".to_string());
        let mut variant_options: Vec<String> = vec!["Base".to_string()];
        variant_options.extend(preview.variants.clone());
        variant_options.dedup();
        let variant_picker = iced::widget::pick_list(
            variant_options,
            Some(variant_label),
            |s| {
                if s.eq_ignore_ascii_case("Base") {
                    Message::PrintPreviewSetVariant(None)
                } else {
                    Message::PrintPreviewSetVariant(Some(s))
                }
            },
        )
        .text_size(11)
        .width(220);

        let labelled_check = |label: &'static str,
                              value: bool,
                              on: fn(bool) -> Message|
         -> iced::widget::Row<'_, Message> {
            row![
                text(label).size(11).color(text_muted).width(150),
                checkbox(value).on_toggle(on),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
        };

        column![
            self.pdf_section_title("Structure Settings"),
            container(
                column![
                    text("If checked, exported sheets are expanded from logical to physical sheets. Choose a variant and which expanded names appear.")
                        .size(11)
                        .color(text_muted),
                    Space::new().height(8),
                    row![
                        checkbox(opts.use_physical_structure)
                            .on_toggle(Message::PrintPreviewSetUsePhysicalStructure),
                        text("Use Physical Structure").size(11).color(text_c),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                    Space::new().height(6),
                    row![
                        text("Variant").size(11).color(text_muted).width(150),
                        variant_picker,
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                    Space::new().height(8),
                    labelled_check("Designators", opts.physical_designators, Message::PrintPreviewSetPhysicalDesignators),
                    labelled_check("Net Labels", opts.physical_net_labels, Message::PrintPreviewSetPhysicalNetLabels),
                    labelled_check("Ports and Sheet Entries", opts.physical_ports, Message::PrintPreviewSetPhysicalPorts),
                    labelled_check("Sheet Number Parameter", opts.physical_sheet_number, Message::PrintPreviewSetPhysicalSheetNumber),
                    labelled_check("Document Number Parameter", opts.physical_document_number, Message::PrintPreviewSetPhysicalDocumentNumber),
                ]
                .padding([10, 12])
                .spacing(2),
            ),
        ]
        .spacing(0)
        .into()
    }

    /// Settings → Additional PDF Settings.
    fn view_pdf_additional_section(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use crate::app::state::PdfQuality;
        use iced::widget::{checkbox, column, container, row, text, Space};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let opts = &preview.pdf_options;

        let lbl_check = move |label: &'static str, value: bool, on: fn(bool) -> Message| {
            row![
                checkbox(value).on_toggle(on),
                text(label).size(11).color(text_c),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center)
        };

        let zoom_slider = iced::widget::slider(
            0.0_f32..=1.0,
            opts.bookmark_zoom,
            Message::PrintPreviewSetBookmarkZoom,
        )
        .step(0.05_f32)
        .width(180);
        let zoom_col = column![
            text("Zoom").size(11).color(text_c),
            text("Slider controls the zoom level in the PDF reader when jumping to components or nets.")
                .size(10)
                .color(text_muted),
            Space::new().height(6),
            row![
                text("Far").size(10).color(text_muted),
                zoom_slider,
                text("Close").size(10).color(text_muted),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(2);

        let info_col = column![
            text("Additional Information").size(11).color(text_c),
            Space::new().height(4),
            lbl_check(
                "Generate nets information",
                opts.generate_nets_info,
                Message::PrintPreviewSetGenerateNetsInfo,
            ),
            Space::new().height(4),
            text("The following bookmarks can be created in the PDF for nets:")
                .size(10)
                .color(text_muted),
            row![
                Space::new().width(14),
                column![
                    lbl_check("Pins", opts.bookmark_pins, Message::PrintPreviewSetBookmarkPins),
                    lbl_check("Net Labels", opts.bookmark_net_labels, Message::PrintPreviewSetBookmarkNetLabels),
                    lbl_check("Ports", opts.bookmark_ports, Message::PrintPreviewSetBookmarkPorts),
                ]
                .spacing(2),
            ],
            Space::new().height(4),
            lbl_check(
                "Include Component Parameters",
                opts.include_component_parameters,
                Message::PrintPreviewSetIncludeComponentParameters,
            ),
            lbl_check(
                "Global Bookmarks for Components and Nets",
                opts.global_bookmarks,
                Message::PrintPreviewSetGlobalBookmarks,
            ),
        ]
        .spacing(2);

        let schematics_include_col = column![
            text("Schematics include").size(11).color(text_c),
            Space::new().height(4),
            lbl_check("No-ERC Markers", opts.include_no_erc_markers, Message::PrintPreviewSetIncludeNoErcMarkers),
            lbl_check("Parameter Sets", opts.include_parameter_sets, Message::PrintPreviewSetIncludeParameterSets),
            lbl_check("Probes", opts.include_probes, Message::PrintPreviewSetIncludeProbes),
            lbl_check("Blankets", opts.include_blankets, Message::PrintPreviewSetIncludeBlankets),
            lbl_check("Notes", opts.include_notes, Message::PrintPreviewSetIncludeNotes),
            row![
                Space::new().width(14),
                lbl_check("Collapsed notes", opts.include_collapsed_notes, Message::PrintPreviewSetIncludeCollapsedNotes),
            ],
            Space::new().height(8),
            text("Quality").size(11).color(text_c),
            iced::widget::pick_list(
                vec![PdfQuality::Draft72, PdfQuality::Medium300, PdfQuality::High600],
                Some(preview.quality),
                Message::PrintPreviewSetQuality,
            )
            .text_size(11)
            .width(180),
        ]
        .spacing(2);

        let radio = move |label: &'static str,
                          this: signex_output::ColourMode,
                          current: signex_output::ColourMode,
                          on: fn(signex_output::ColourMode) -> Message| {
            iced::widget::radio(label, this, Some(current), on)
                .text_size(11)
                .size(14)
        };

        let sch_color_col = column![
            text("Schematics Color Mode").size(11).color(text_c),
            Space::new().height(4),
            radio("Color", signex_output::ColourMode::Colour, opts.colour_mode, Message::PrintPreviewSetColourMode),
            radio("Greyscale", signex_output::ColourMode::Grayscale, opts.colour_mode, Message::PrintPreviewSetColourMode),
            radio("Monochrome", signex_output::ColourMode::BlackAndWhite, opts.colour_mode, Message::PrintPreviewSetColourMode),
            Space::new().height(8),
            text("PCB Color Mode").size(11).color(text_c),
            Space::new().height(4),
            radio("Color", signex_output::ColourMode::Colour, opts.pcb_colour_mode, Message::PrintPreviewSetPcbColourMode),
            radio("Greyscale", signex_output::ColourMode::Grayscale, opts.pcb_colour_mode, Message::PrintPreviewSetPcbColourMode),
            radio("Monochrome", signex_output::ColourMode::BlackAndWhite, opts.pcb_colour_mode, Message::PrintPreviewSetPcbColourMode),
        ]
        .spacing(2);

        column![
            self.pdf_section_title("Additional PDF Settings"),
            container(
                row![
                    column![zoom_col, Space::new().height(12), info_col]
                        .spacing(0)
                        .width(Length::FillPortion(2)),
                    Space::new().width(16),
                    schematics_include_col.width(Length::FillPortion(2)),
                    Space::new().width(16),
                    sch_color_col.width(Length::FillPortion(2)),
                ]
                .padding([10, 12]),
            ),
        ]
        .spacing(0)
        .into()
    }

}
