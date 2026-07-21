//! PDF Settings tab and its section builders (files / structure /
//! additional) — methods of the shared `Signex` view impl, split from
//! `view/pdf_preview.rs` as pure code motion.

use super::super::*;

impl Signex {
    /// Settings tab — stitches the three section helpers below into a
    /// single scrollable column. Each helper owns its own widgets and
    /// reads/writes through `preview.pdf_options.*` directly so the
    /// rasterizer and exporter stay in lockstep with the UI.
    pub(in crate::app::view) fn view_pdf_settings_tab(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{Space, column, scrollable};
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
        use iced::widget::{Space, button, checkbox, column, container, row, scrollable, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let panel_bg = crate::styles::ti(tokens.panel_bg);

        // Snapshot of the export context's sheet set, captured when the
        // modal opened — always the exact page set the exporter will
        // emit, project-scoped or loose.
        let sheet_files = &preview.sheet_files;

        let mut file_list: iced::widget::Column<'_, Message> =
            column![].spacing(2).padding([8, 12]);
        if sheet_files.is_empty() {
            file_list = file_list.push(
                text("No sheets to export — open a schematic first.")
                    .size(11)
                    .color(text_muted),
            );
        } else {
            for (path, name) in sheet_files {
                let is_selected = preview.selected_files.contains(path);
                let path_str = path.display().to_string();
                let row_el = row![
                    checkbox(is_selected).on_toggle({
                        let path = path.clone();
                        move |_| Message::PrintPreview(PrintPreviewMsg::ToggleFile(path.clone()))
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
                .on_press(Message::PrintPreview(PrintPreviewMsg::SelectAllFiles))
                .style(secondary_btn_style),
            button(text("Clear").size(11).color(text_c))
                .padding([3, 8])
                .on_press(Message::PrintPreview(PrintPreviewMsg::ClearAllFiles))
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
        use iced::widget::{Space, checkbox, column, container, row, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let opts = &preview.pdf_options;

        let variant_label = opts.variant.clone().unwrap_or_else(|| "Base".to_string());
        let mut variant_options: Vec<String> = vec!["Base".to_string()];
        variant_options.extend(preview.variants.clone());
        variant_options.dedup();
        let variant_picker = iced::widget::pick_list(variant_options, Some(variant_label), |s| {
            if s.eq_ignore_ascii_case("Base") {
                Message::PrintPreview(PrintPreviewMsg::SetVariant(None))
            } else {
                Message::PrintPreview(PrintPreviewMsg::SetVariant(Some(s)))
            }
        })
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
                            .on_toggle(|v| Message::PrintPreview(PrintPreviewMsg::SetUsePhysicalStructure(v))),
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
                    labelled_check("Designators", opts.physical_designators, |v| Message::PrintPreview(PrintPreviewMsg::SetPhysicalDesignators(v))),
                    labelled_check("Net Labels", opts.physical_net_labels, |v| Message::PrintPreview(PrintPreviewMsg::SetPhysicalNetLabels(v))),
                    labelled_check("Ports and Sheet Entries", opts.physical_ports, |v| Message::PrintPreview(PrintPreviewMsg::SetPhysicalPorts(v))),
                    labelled_check("Sheet Number Parameter", opts.physical_sheet_number, |v| Message::PrintPreview(PrintPreviewMsg::SetPhysicalSheetNumber(v))),
                    labelled_check("Document Number Parameter", opts.physical_document_number, |v| Message::PrintPreview(PrintPreviewMsg::SetPhysicalDocumentNumber(v))),
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
        use iced::widget::{Space, checkbox, column, container, row, text};
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

        let zoom_slider = iced::widget::slider(0.0_f32..=1.0, opts.bookmark_zoom, |v| {
            Message::PrintPreview(PrintPreviewMsg::SetBookmarkZoom(v))
        })
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
            lbl_check("Generate nets information", opts.generate_nets_info, |v| {
                Message::PrintPreview(PrintPreviewMsg::SetGenerateNetsInfo(v))
            },),
            Space::new().height(4),
            text("The following bookmarks can be created in the PDF for nets:")
                .size(10)
                .color(text_muted),
            row![
                Space::new().width(14),
                column![
                    lbl_check("Pins", opts.bookmark_pins, |v| Message::PrintPreview(
                        PrintPreviewMsg::SetBookmarkPins(v)
                    )),
                    lbl_check("Net Labels", opts.bookmark_net_labels, |v| {
                        Message::PrintPreview(PrintPreviewMsg::SetBookmarkNetLabels(v))
                    }),
                    lbl_check("Ports", opts.bookmark_ports, |v| Message::PrintPreview(
                        PrintPreviewMsg::SetBookmarkPorts(v)
                    )),
                ]
                .spacing(2),
            ],
            Space::new().height(4),
            lbl_check(
                "Include Component Parameters",
                opts.include_component_parameters,
                |v| Message::PrintPreview(PrintPreviewMsg::SetIncludeComponentParameters(v)),
            ),
            lbl_check(
                "Global Bookmarks for Components and Nets",
                opts.global_bookmarks,
                |v| Message::PrintPreview(PrintPreviewMsg::SetGlobalBookmarks(v)),
            ),
        ]
        .spacing(2);

        let schematics_include_col = column![
            text("Schematics include").size(11).color(text_c),
            Space::new().height(4),
            lbl_check("No-ERC Markers", opts.include_no_erc_markers, |v| {
                Message::PrintPreview(PrintPreviewMsg::SetIncludeNoErcMarkers(v))
            }),
            lbl_check("Parameter Sets", opts.include_parameter_sets, |v| {
                Message::PrintPreview(PrintPreviewMsg::SetIncludeParameterSets(v))
            }),
            lbl_check("Probes", opts.include_probes, |v| Message::PrintPreview(
                PrintPreviewMsg::SetIncludeProbes(v)
            )),
            lbl_check(
                "Blankets",
                opts.include_blankets,
                |v| Message::PrintPreview(PrintPreviewMsg::SetIncludeBlankets(v))
            ),
            lbl_check("Notes", opts.include_notes, |v| Message::PrintPreview(
                PrintPreviewMsg::SetIncludeNotes(v)
            )),
            row![
                Space::new().width(14),
                lbl_check("Collapsed notes", opts.include_collapsed_notes, |v| {
                    Message::PrintPreview(PrintPreviewMsg::SetIncludeCollapsedNotes(v))
                }),
            ],
            Space::new().height(8),
            text("Quality").size(11).color(text_c),
            iced::widget::pick_list(
                vec![
                    PdfQuality::Draft72,
                    PdfQuality::Medium300,
                    PdfQuality::High600
                ],
                Some(preview.quality),
                |v| Message::PrintPreview(PrintPreviewMsg::SetQuality(v)),
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
            radio(
                "Color",
                signex_output::ColourMode::Colour,
                opts.colour_mode,
                |v| Message::PrintPreview(PrintPreviewMsg::SetColourMode(v))
            ),
            radio(
                "Greyscale",
                signex_output::ColourMode::Grayscale,
                opts.colour_mode,
                |v| Message::PrintPreview(PrintPreviewMsg::SetColourMode(v))
            ),
            radio(
                "Monochrome",
                signex_output::ColourMode::BlackAndWhite,
                opts.colour_mode,
                |v| Message::PrintPreview(PrintPreviewMsg::SetColourMode(v))
            ),
            Space::new().height(8),
            text("PCB Color Mode").size(11).color(text_c),
            Space::new().height(4),
            radio(
                "Color",
                signex_output::ColourMode::Colour,
                opts.pcb_colour_mode,
                |v| Message::PrintPreview(PrintPreviewMsg::SetPcbColourMode(v))
            ),
            radio(
                "Greyscale",
                signex_output::ColourMode::Grayscale,
                opts.pcb_colour_mode,
                |v| Message::PrintPreview(PrintPreviewMsg::SetPcbColourMode(v))
            ),
            radio(
                "Monochrome",
                signex_output::ColourMode::BlackAndWhite,
                opts.pcb_colour_mode,
                |v| Message::PrintPreview(PrintPreviewMsg::SetPcbColourMode(v))
            ),
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
