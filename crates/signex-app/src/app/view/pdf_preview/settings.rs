//! PDF Settings tab and its section builders (files / structure /
//! additional) — methods of the shared `Signex` view impl, split from
//! `view/pdf_preview.rs` as pure code motion.

use super::super::*;
use crate::choice::Choice;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_widgets::dropdown_style::GraphiteDropdown as _;

/// A variant pick-list option: `None` prints the base design.
type VariantChoice = Choice<Option<String>>;

/// The variant options, Base first, and the one `current` names. A saved
/// variant the project no longer lists still shows as chosen.
fn variant_choices(
    variants: &[String],
    current: Option<&str>,
    i18n: &I18n,
) -> (Vec<VariantChoice>, VariantChoice) {
    let base = Choice {
        value: None,
        label: i18n.message_plain(ids::PDF_VARIANT_BASE, &[]),
    };
    let named = |name: &str| Choice {
        value: Some(name.to_owned()),
        label: name.to_owned(),
    };
    let chosen = current.map_or_else(|| base.clone(), named);
    let mut options = vec![base];
    options.extend(variants.iter().map(|name| named(name)));
    options.dedup();
    (options, chosen)
}

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

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
            self.view_pdf_page_setup_section(preview),
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
            .boxed()
    }

    /// Section header strip — accent panel-bg with a 1 px border.
    /// Reused by every Settings-tab section.
    pub(in crate::app::view) fn pdf_section_title(&self, label: MessageId) -> Element<'_, Message> {
        use iced::widget::container;
        use signex_widgets::text;
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let border_c = crate::styles::ti(tokens.border);
        container(
            text(t!(self.ui_state.i18n, label))
                .size(TS.label)
                .color(text_c),
        )
        .padding([6, 10])
        .width(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(panel_bg.into()),
            border: iced::Border {
                color: border_c,
                width: 1.0,
                radius: iced::border::Radius::default(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
    }

    /// Settings → Choose Project Files.
    fn view_pdf_files_section(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{Space, column, container, row, scrollable};
        use signex_widgets::{button, checkbox, text};
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let panel_bg = crate::styles::ti(tokens.panel_bg);

        // Snapshot of the export context's sheet set, captured when the
        // modal opened — always the exact page set the exporter will
        // emit, project-scoped or loose.
        let sheet_files = &preview.sheet_files;

        let mut file_list: iced::widget::Column<Element<'_, Message>> =
            column![].spacing(SP.xs).padding([SP.m, SP.l]);
        if sheet_files.is_empty() {
            file_list = file_list.push(
                text(t!(i18n, ids::PDF_NO_SHEETS))
                    .size(TS.body)
                    .color(text_muted)
                    .boxed(),
            );
        } else {
            for (page, name) in sheet_files {
                let is_selected = preview.selected_files.contains(page);
                let path_str = page.0.display().to_string();
                let row_el = row![
                    checkbox(is_selected).on_toggle({
                        let page = page.clone();
                        move |_| Message::PrintPreview(PrintPreviewMsg::ToggleFile(page.clone()))
                    }),
                    column![
                        text(signex_i18n::UserText::new(name.clone()))
                            .size(TS.body)
                            .color(text_c),
                        text(signex_i18n::UserText::new(path_str))
                            .size(TS.caption)
                            .color(text_muted),
                    ]
                    .spacing(1),
                ]
                .spacing(SP.m)
                .align_y(iced::Alignment::Center);
                file_list = file_list.push(row_el.boxed());
            }
        }
        let secondary_btn_style = move |_: &iced::Theme, _status| iced::widget::button::Style {
            background: Some(panel_bg.into()),
            text_color: text_c,
            border: iced::Border {
                color: border_c,
                width: 1.0,
                radius: iced::border::Radius::from(3.0),
                ..iced::Border::default()
            },
            ..iced::widget::button::Style::default()
        };
        let file_actions = row![
            button(
                text(t!(i18n, ids::PDF_SELECT_ALL))
                    .size(TS.body)
                    .color(text_c)
            )
            .padding([3.0, SP.m])
            .on_press(Message::PrintPreview(PrintPreviewMsg::SelectAllFiles))
            .style(secondary_btn_style),
            button(text(t!(i18n, ids::PDF_CLEAR)).size(TS.body).color(text_c))
                .padding([3.0, SP.m])
                .on_press(Message::PrintPreview(PrintPreviewMsg::ClearAllFiles))
                .style(secondary_btn_style),
        ]
        .spacing(6);

        column![
            self.pdf_section_title(ids::PDF_SECTION_FILES),
            container(
                column![
                    text(t!(i18n, ids::PDF_FILES_HELP))
                        .size(TS.body)
                        .color(text_muted),
                    Space::new().height(6),
                    container(scrollable(file_list).height(160))
                        .width(Length::Fill)
                        .style(move |_: &iced::Theme| container::Style {
                            border: iced::Border {
                                color: border_c,
                                width: 1.0,
                                radius: iced::border::Radius::default(),
                                ..iced::Border::default()
                            },
                            ..container::Style::default()
                        }),
                    Space::new().height(6),
                    file_actions,
                ]
                .padding([10.0, SP.l]),
            ),
        ]
        .spacing(0)
        .boxed()
    }

    /// Settings → Structure Settings.
    fn view_pdf_structure_section(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{Space, column, container, row};
        use signex_widgets::{checkbox, text};
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let opts = &preview.pdf_options;

        let (variant_options, variant_chosen) =
            variant_choices(&preview.variants, opts.variant.as_deref(), i18n);
        let variant_picker = iced::widget::pick_list(
            Some(variant_chosen),
            variant_options,
            |c: &VariantChoice| c.label.clone(),
        )
        .graphite()
        .on_select(|c: VariantChoice| Message::PrintPreview(PrintPreviewMsg::SetVariant(c.value)))
        .width(220);

        let labelled_check = |label: MessageId,
                              value: bool,
                              on: fn(bool) -> Message|
         -> iced::widget::Row<Element<'_, Message>> {
            row![
                text(t!(i18n, label))
                    .size(TS.body)
                    .color(text_muted)
                    .width(150),
                checkbox(value).on_toggle(on),
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center)
        };

        column![
            self.pdf_section_title(ids::PDF_SECTION_STRUCTURE),
            container(
                column![
                    text(t!(i18n, ids::PDF_STRUCTURE_HELP))
                        .size(TS.body)
                        .color(text_muted),
                    Space::new().height(8),
                    row![
                        checkbox(opts.use_physical_structure).on_toggle(|v| Message::PrintPreview(
                            PrintPreviewMsg::SetUsePhysicalStructure(v)
                        )),
                        text(t!(i18n, ids::PDF_USE_PHYSICAL))
                            .size(TS.body)
                            .color(text_c),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                    Space::new().height(6),
                    row![
                        text(t!(i18n, ids::PDF_VARIANT))
                            .size(TS.body)
                            .color(text_muted)
                            .width(150),
                        variant_picker,
                    ]
                    .spacing(SP.m)
                    .align_y(iced::Alignment::Center),
                    Space::new().height(8),
                    labelled_check(
                        ids::PDF_PHYSICAL_DESIGNATORS,
                        opts.physical_designators,
                        |v| Message::PrintPreview(PrintPreviewMsg::SetPhysicalDesignators(v))
                    ),
                    labelled_check(
                        ids::PDF_PHYSICAL_NET_LABELS,
                        opts.physical_net_labels,
                        |v| Message::PrintPreview(PrintPreviewMsg::SetPhysicalNetLabels(v))
                    ),
                    labelled_check(ids::PDF_PHYSICAL_PORTS, opts.physical_ports, |v| {
                        Message::PrintPreview(PrintPreviewMsg::SetPhysicalPorts(v))
                    }),
                    labelled_check(
                        ids::PDF_PHYSICAL_SHEET_NUMBER,
                        opts.physical_sheet_number,
                        |v| Message::PrintPreview(PrintPreviewMsg::SetPhysicalSheetNumber(v))
                    ),
                    labelled_check(
                        ids::PDF_PHYSICAL_DOCUMENT_NUMBER,
                        opts.physical_document_number,
                        |v| Message::PrintPreview(PrintPreviewMsg::SetPhysicalDocumentNumber(v))
                    ),
                ]
                .padding([10.0, SP.l])
                .spacing(SP.xs),
            ),
        ]
        .spacing(0)
        .boxed()
    }

    /// Settings → Additional PDF Settings.
    fn view_pdf_additional_section(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use crate::app::state::PdfQuality;
        use iced::widget::{Space, column, container, row};
        use signex_widgets::{checkbox, text};
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let opts = &preview.pdf_options;
        let qualities = Choice::named(
            &[
                PdfQuality::Draft72,
                PdfQuality::Medium300,
                PdfQuality::High600,
            ],
            PdfQuality::label,
            i18n,
        );
        let quality_chosen = crate::choice::chosen(&qualities, &preview.quality);

        let lbl_check = move |label: MessageId, value: bool, on: fn(bool) -> Message| {
            row![
                checkbox(value).on_toggle(on),
                text(t!(i18n, label)).size(TS.body).color(text_c),
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
            text(t!(i18n, ids::PDF_ZOOM)).size(TS.body).color(text_c),
            text(t!(i18n, ids::PDF_ZOOM_HELP))
                .size(TS.caption)
                .color(text_muted),
            Space::new().height(6),
            row![
                text(t!(i18n, ids::PDF_ZOOM_FAR))
                    .size(TS.caption)
                    .color(text_muted),
                zoom_slider,
                text(t!(i18n, ids::PDF_ZOOM_CLOSE))
                    .size(TS.caption)
                    .color(text_muted),
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(SP.xs);

        let info_col = column![
            text(t!(i18n, ids::PDF_ADDITIONAL_INFO))
                .size(TS.body)
                .color(text_c),
            Space::new().height(4),
            lbl_check(ids::PDF_GENERATE_NETS, opts.generate_nets_info, |v| {
                Message::PrintPreview(PrintPreviewMsg::SetGenerateNetsInfo(v))
            },),
            Space::new().height(4),
            text(t!(i18n, ids::PDF_NET_BOOKMARKS_HELP))
                .size(TS.caption)
                .color(text_muted),
            row![
                Space::new().width(14),
                column![
                    lbl_check(ids::PDF_BOOKMARK_PINS, opts.bookmark_pins, |v| {
                        Message::PrintPreview(PrintPreviewMsg::SetBookmarkPins(v))
                    }),
                    lbl_check(
                        ids::PDF_BOOKMARK_NET_LABELS,
                        opts.bookmark_net_labels,
                        |v| { Message::PrintPreview(PrintPreviewMsg::SetBookmarkNetLabels(v)) }
                    ),
                    lbl_check(ids::PDF_BOOKMARK_PORTS, opts.bookmark_ports, |v| {
                        Message::PrintPreview(PrintPreviewMsg::SetBookmarkPorts(v))
                    }),
                ]
                .spacing(SP.xs),
            ],
            Space::new().height(4),
            lbl_check(
                ids::PDF_INCLUDE_COMPONENT_PARAMETERS,
                opts.include_component_parameters,
                |v| Message::PrintPreview(PrintPreviewMsg::SetIncludeComponentParameters(v)),
            ),
            lbl_check(ids::PDF_GLOBAL_BOOKMARKS, opts.global_bookmarks, |v| {
                Message::PrintPreview(PrintPreviewMsg::SetGlobalBookmarks(v))
            },),
        ]
        .spacing(SP.xs);

        let schematics_include_col = column![
            text(t!(i18n, ids::PDF_SCHEMATICS_INCLUDE))
                .size(TS.body)
                .color(text_c),
            Space::new().height(4),
            lbl_check(ids::PDF_NO_ERC_MARKERS, opts.include_no_erc_markers, |v| {
                Message::PrintPreview(PrintPreviewMsg::SetIncludeNoErcMarkers(v))
            }),
            lbl_check(
                ids::PDF_NO_CONNECT_MARKERS,
                opts.include_no_connect_markers,
                |v| { Message::PrintPreview(PrintPreviewMsg::SetIncludeNoConnectMarkers(v)) }
            ),
            lbl_check(ids::PDF_PARAMETER_SETS, opts.include_parameter_sets, |v| {
                Message::PrintPreview(PrintPreviewMsg::SetIncludeParameterSets(v))
            }),
            lbl_check(ids::PDF_PROBES, opts.include_probes, |v| {
                Message::PrintPreview(PrintPreviewMsg::SetIncludeProbes(v))
            }),
            lbl_check(ids::PDF_BLANKETS, opts.include_blankets, |v| {
                Message::PrintPreview(PrintPreviewMsg::SetIncludeBlankets(v))
            }),
            lbl_check(ids::PDF_NOTES, opts.include_notes, |v| {
                Message::PrintPreview(PrintPreviewMsg::SetIncludeNotes(v))
            }),
            row![
                Space::new().width(14),
                lbl_check(
                    ids::PDF_COLLAPSED_NOTES,
                    opts.include_collapsed_notes,
                    |v| { Message::PrintPreview(PrintPreviewMsg::SetIncludeCollapsedNotes(v)) }
                ),
            ],
            Space::new().height(8),
            text(t!(i18n, ids::PDF_QUALITY)).size(TS.body).color(text_c),
            iced::widget::pick_list(quality_chosen, qualities, |q: &Choice<PdfQuality>| q
                .label
                .clone())
            .graphite()
            .on_select(|q: Choice<PdfQuality>| {
                Message::PrintPreview(PrintPreviewMsg::SetQuality(q.value))
            })
            .width(180),
        ]
        .spacing(SP.xs);

        let radio = move |label: MessageId,
                          this: signex_output::ColourMode,
                          current: signex_output::ColourMode,
                          on: fn(signex_output::ColourMode) -> Message| {
            signex_widgets::radio(t!(i18n, label), this, Some(current), on)
                .text_size(11)
                .size(TS.heading)
        };

        let sch_color_col = column![
            text(t!(i18n, ids::PDF_SCHEMATICS_COLOR_MODE))
                .size(TS.body)
                .color(text_c),
            Space::new().height(4),
            radio(
                ids::PDF_COLOR_MODE_COLOR,
                signex_output::ColourMode::Colour,
                opts.colour_mode,
                |v| Message::PrintPreview(PrintPreviewMsg::SetColourMode(v))
            ),
            radio(
                ids::PDF_COLOR_MODE_GREYSCALE,
                signex_output::ColourMode::Grayscale,
                opts.colour_mode,
                |v| Message::PrintPreview(PrintPreviewMsg::SetColourMode(v))
            ),
            radio(
                ids::PDF_COLOR_MODE_MONOCHROME,
                signex_output::ColourMode::BlackAndWhite,
                opts.colour_mode,
                |v| Message::PrintPreview(PrintPreviewMsg::SetColourMode(v))
            ),
            Space::new().height(8),
            text(t!(i18n, ids::PDF_PCB_COLOR_MODE))
                .size(TS.body)
                .color(text_c),
            Space::new().height(4),
            radio(
                ids::PDF_COLOR_MODE_COLOR,
                signex_output::ColourMode::Colour,
                opts.pcb_colour_mode,
                |v| Message::PrintPreview(PrintPreviewMsg::SetPcbColourMode(v))
            ),
            radio(
                ids::PDF_COLOR_MODE_GREYSCALE,
                signex_output::ColourMode::Grayscale,
                opts.pcb_colour_mode,
                |v| Message::PrintPreview(PrintPreviewMsg::SetPcbColourMode(v))
            ),
            radio(
                ids::PDF_COLOR_MODE_MONOCHROME,
                signex_output::ColourMode::BlackAndWhite,
                opts.pcb_colour_mode,
                |v| Message::PrintPreview(PrintPreviewMsg::SetPcbColourMode(v))
            ),
        ]
        .spacing(SP.xs);

        column![
            self.pdf_section_title(ids::PDF_SECTION_ADDITIONAL),
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
                .padding([10.0, SP.l]),
            ),
        ]
        .spacing(0)
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Base is an option of its own, not the text "Base" (#2059): a variant
    /// the user named "Base" stays a variant.
    #[test]
    fn the_variant_list_holds_base_as_no_variant() {
        let i18n = I18n::en_us();
        let variants = ["Base".to_owned(), "Lite".to_owned()];
        let (options, chosen) = variant_choices(&variants, None, &i18n);
        let values: Vec<_> = options.iter().map(|c| c.value.as_deref()).collect();
        assert_eq!(values, [None, Some("Base"), Some("Lite")]);
        assert_eq!(options[0].label, "Base");
        assert_eq!(chosen.value, None);
        let (_, chosen) = variant_choices(&variants, Some("Lite"), &i18n);
        assert_eq!(chosen.value.as_deref(), Some("Lite"));
    }

    /// The quality names read as they did when they came from `Display`.
    #[test]
    fn the_quality_names_come_from_the_catalog() {
        use crate::app::state::PdfQuality;
        let i18n = I18n::en_us();
        let all = [
            PdfQuality::Draft72,
            PdfQuality::Medium300,
            PdfQuality::High600,
        ];
        let labels: Vec<String> = Choice::named(&all, PdfQuality::label, &i18n)
            .into_iter()
            .map(|c| c.label)
            .collect();
        assert_eq!(
            labels,
            ["Draft (72 dpi)", "Medium (300 dpi)", "High (600 dpi)"]
        );
    }
}
