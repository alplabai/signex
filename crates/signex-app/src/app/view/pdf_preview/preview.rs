//! PDF preview tab (toolbar, thumbnail rail, pan/zoom viewport) — a
//! method of the shared `Signex` view impl, split from
//! `view/pdf_preview.rs` as pure code motion.

use super::super::*;

impl Signex {
    /// Preview tab — top toolbar (Sheet/Colour/Pages/Output), thumb
    /// rail on the left, pan/zoom viewport on the right.
    pub(in crate::app::view) fn view_pdf_preview_tab(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{
            Space, button, checkbox, column, container, image, mouse_area, row, scrollable, text,
            text_input,
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
                matches!(
                    preview.pdf_options.colour_mode,
                    signex_output::ColourMode::Colour
                ),
                Message::PrintPreview(PrintPreviewMsg::SetColourMode(
                    signex_output::ColourMode::Colour
                )),
            ),
            mode_button(
                "Gray",
                matches!(
                    preview.pdf_options.colour_mode,
                    signex_output::ColourMode::Grayscale
                ),
                Message::PrintPreview(PrintPreviewMsg::SetColourMode(
                    signex_output::ColourMode::Grayscale
                )),
            ),
            mode_button(
                "B/W",
                matches!(
                    preview.pdf_options.colour_mode,
                    signex_output::ColourMode::BlackAndWhite
                ),
                Message::PrintPreview(PrintPreviewMsg::SetColourMode(
                    signex_output::ColourMode::BlackAndWhite
                )),
            ),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let range_controls = row![
            text("Pages").size(11).color(text_muted),
            mode_button(
                "All",
                matches!(
                    preview.pdf_options.page_range,
                    signex_output::PageRange::All
                ),
                Message::PrintPreview(PrintPreviewMsg::SetPageRangeAll),
            ),
            mode_button(
                "Current",
                matches!(
                    preview.pdf_options.page_range,
                    signex_output::PageRange::Current
                ),
                Message::PrintPreview(PrintPreviewMsg::SetPageRangeCurrent),
            ),
            mode_button(
                "Specific",
                matches!(
                    preview.pdf_options.page_range,
                    signex_output::PageRange::Specific(_)
                ),
                Message::PrintPreview(PrintPreviewMsg::SetPageRangeSpecific),
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
                    .on_input(|v| Message::PrintPreview(PrintPreviewMsg::SetSpecificPageInput(v)))
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
                checkbox(fit_to_page)
                    .on_toggle(|v| Message::PrintPreview(PrintPreviewMsg::SetFitToPage(v))),
                text("Fit to Page").size(11).color(text_c),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
            row![
                checkbox(preview.pdf_options.include_title_block)
                    .on_toggle(|v| Message::PrintPreview(PrintPreviewMsg::SetIncludeTitleBlock(v))),
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
            thumbs = thumbs.push(
                mouse_area(card).on_press(Message::PrintPreview(PrintPreviewMsg::SelectPage(i))),
            );
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
                .on_press(Message::PrintPreview(PrintPreviewMsg::PanStart))
                .on_release(Message::PrintPreview(PrintPreviewMsg::PanFinished))
                .on_scroll(|delta| {
                    use iced::mouse::ScrollDelta;
                    let dy = match delta {
                        ScrollDelta::Lines { y, .. } => y,
                        ScrollDelta::Pixels { y, .. } => y,
                    };
                    Message::PrintPreview(PrintPreviewMsg::Zoom(dy))
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
}
