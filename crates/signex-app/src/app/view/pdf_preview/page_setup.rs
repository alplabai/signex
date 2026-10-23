//! Page-setup controls of the PDF export modal: size / orientation / scale
//! in a toolbar row on the Preview tab, margins in a Settings-tab section.
//! Pure views — every edit is a `PrintPreviewMsg` the handlers validate.

use signex_output::{MarginEdge, PageSize, PageSizeKind, PdfScale, PdfScaleMode};

use super::super::*;

/// Body text size of the modal's compact controls.
const FIELD_TEXT: f32 = 11.0;
/// Numeric field width, in multiples of the text size (fits `5080.00`).
const FIELD_WIDTH_EM: f32 = 7.0;

impl Signex {
    fn pdf_numeric_field(
        &self,
        value: &str,
        on_input: impl Fn(String) -> PrintPreviewMsg + 'static,
    ) -> Element<'static, Message> {
        iced::widget::text_input("", value)
            .on_input(move |v| Message::PrintPreview(on_input(v)))
            .padding([4, 8])
            .size(FIELD_TEXT + 1.0)
            .width(FIELD_TEXT * FIELD_WIDTH_EM)
            .into()
    }

    /// Preview-tab row: page size, orientation, Custom dimensions, scale.
    pub(in crate::app::view) fn view_pdf_page_controls(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'static, Message> {
        use iced::widget::{pick_list, row, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let options = &preview.pdf_options;
        let inputs = &preview.page_setup;

        let current_size = options.page_size;
        let orientation = options.orientation;
        let size_picker = pick_list(
            PageSizeKind::ALL.to_vec(),
            Some(current_size.kind()),
            move |kind| {
                Message::PrintPreview(PrintPreviewMsg::SetPageSize(
                    kind.select(current_size, orientation),
                ))
            },
        )
        .text_size(FIELD_TEXT);
        // A Custom page is exactly the width and height entered, so its
        // orientation is derived from them and shown, not picked.
        let orientation_control: Element<'static, Message> =
            if matches!(current_size, PageSize::Custom { .. }) {
                text(orientation.to_string())
                    .size(FIELD_TEXT)
                    .color(text_muted)
                    .into()
            } else {
                pick_list(
                    vec![
                        signex_output::Orientation::Portrait,
                        signex_output::Orientation::Landscape,
                    ],
                    Some(orientation),
                    |o| Message::PrintPreview(PrintPreviewMsg::SetOrientation(o)),
                )
                .text_size(FIELD_TEXT)
                .into()
            };

        let mut controls = row![
            text("Page").size(FIELD_TEXT).color(text_muted),
            size_picker,
            orientation_control,
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        if matches!(current_size, PageSize::Custom { .. }) {
            controls = controls.push(text("W").size(FIELD_TEXT).color(text_muted));
            controls = controls.push(
                self.pdf_numeric_field(&inputs.custom_width, PrintPreviewMsg::SetCustomWidth),
            );
            controls = controls.push(text("H").size(FIELD_TEXT).color(text_muted));
            controls = controls.push(
                self.pdf_numeric_field(&inputs.custom_height, PrintPreviewMsg::SetCustomHeight),
            );
            controls = controls.push(text("mm").size(FIELD_TEXT).color(text_muted));
        }

        let scale_picker = pick_list(
            PdfScaleMode::ALL.to_vec(),
            Some(options.scale.mode()),
            |m| Message::PrintPreview(PrintPreviewMsg::SetScaleMode(m)),
        )
        .text_size(FIELD_TEXT);
        controls = controls.push(text("Scale").size(FIELD_TEXT).color(text_muted));
        controls = controls.push(scale_picker);
        if matches!(options.scale, PdfScale::Percent(_)) {
            controls = controls
                .push(self.pdf_numeric_field(&inputs.percent, PrintPreviewMsg::SetScalePercent));
            controls = controls.push(text("%").size(FIELD_TEXT).color(text_muted));
        }
        controls.into()
    }

    /// Why the last page-setup edit was rejected or adjusted, in the theme's
    /// error colour; an empty spacer when the last edit applied cleanly.
    pub(in crate::app::view) fn view_pdf_page_setup_notice(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'static, Message> {
        match preview.page_setup.notice() {
            Some(message) => {
                let error_c = crate::styles::ti(self.document_state.panel_ctx.tokens.error);
                iced::widget::text(message.to_string())
                    .size(FIELD_TEXT)
                    .color(error_c)
                    .into()
            }
            None => iced::widget::Space::new().height(0).into(),
        }
    }

    /// Settings-tab "Page Setup" section: the four margins in mm.
    pub(in crate::app::view) fn view_pdf_page_setup_section(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{Space, column, container, row, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_muted = crate::styles::ti(tokens.text_secondary);

        let mut margins = row![].spacing(8).align_y(iced::Alignment::Center);
        for edge in MarginEdge::ALL {
            margins = margins.push(text(edge.to_string()).size(FIELD_TEXT).color(text_muted));
            margins = margins.push(
                self.pdf_numeric_field(preview.page_setup.margin_text(edge), move |v| {
                    PrintPreviewMsg::SetMargin(edge, v)
                }),
            );
        }
        margins = margins.push(text("mm").size(FIELD_TEXT).color(text_muted));

        column![
            self.pdf_section_title("Page Setup"),
            container(
                column![
                    text("Margins are measured from the page edge.")
                        .size(FIELD_TEXT)
                        .color(text_muted),
                    Space::new().height(6),
                    margins,
                    Space::new().height(6),
                    self.view_pdf_page_setup_notice(preview),
                ]
                .spacing(2),
            )
            .padding([8, 12]),
        ]
        .into()
    }
}
