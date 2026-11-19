//! Page-setup controls of the PDF export modal: size / orientation / scale
//! in a toolbar row on the Preview tab, margins in a Settings-tab section.
//! Pure views — every edit is a `PrintPreviewMsg` the handlers validate.

use signex_i18n::MessageId;
use signex_output::{MarginEdge, PageSize, PageSizeKind, PdfScale, PdfScaleMode};
use signex_types::schematic::SheetOrientation;
use signex_widgets::dropdown_style::GraphiteDropdown as _;

use iced::Widget;

use super::super::*;
use crate::choice::{Choice, chosen};

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Body text size of the modal's compact controls.
const FIELD_TEXT: f32 = crate::styles::TypeScale::DEFAULT.body;
/// Numeric field width, in multiples of the text size (fits `5080.00`).
const FIELD_WIDTH_EM: f32 = 7.0;

impl Signex {
    fn pdf_numeric_field(
        &self,
        value: &str,
        on_input: impl Fn(String) -> PrintPreviewMsg + 'static,
    ) -> Element<'static, Message> {
        signex_widgets::text_input(signex_i18n::UserText::new(String::new()), value.to_owned())
            .on_input(move |v| Message::PrintPreview(on_input(v)))
            .padding([SP.s, SP.m])
            .size(FIELD_TEXT + 1.0)
            .width(FIELD_TEXT * FIELD_WIDTH_EM)
            .boxed()
    }

    /// Preview-tab row: page size, orientation, Custom dimensions, scale.
    pub(in crate::app::view) fn view_pdf_page_controls(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'static, Message> {
        use iced::widget::{pick_list, row};
        use signex_i18n::{ids, t};
        use signex_widgets::{raw, text};
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let options = &preview.pdf_options;
        let inputs = &preview.page_setup;

        let current_size = options.page_size;
        let orientation = options.orientation;
        let sizes = Choice::named(&PageSizeKind::ALL, page_size_label, i18n);
        let size_picker = pick_list(
            chosen(&sizes, &current_size.kind()),
            sizes,
            |c: &Choice<PageSizeKind>| c.label.clone(),
        )
        .graphite()
        .on_select(move |c: Choice<PageSizeKind>| {
            Message::PrintPreview(PrintPreviewMsg::SetPageSize(
                c.value.select(current_size, orientation),
            ))
        });
        // A Custom page is exactly the width and height entered, so its
        // orientation is derived from them and shown, not picked.
        let orientation_control: Element<'static, Message> =
            if matches!(current_size, PageSize::Custom { .. }) {
                text(t!(i18n, orientation_label(orientation)))
                    .size(FIELD_TEXT)
                    .color(text_muted)
                    .boxed()
            } else {
                let orientations = Choice::named(
                    &[SheetOrientation::Portrait, SheetOrientation::Landscape],
                    orientation_label,
                    i18n,
                );
                pick_list(
                    chosen(&orientations, &orientation),
                    orientations,
                    |c: &Choice<SheetOrientation>| c.label.clone(),
                )
                .graphite()
                .on_select(|c: Choice<SheetOrientation>| {
                    Message::PrintPreview(PrintPreviewMsg::SetOrientation(c.value))
                })
                .boxed()
            };

        let mut controls = row![
            text(t!(i18n, ids::PDF_PAGE))
                .size(FIELD_TEXT)
                .color(text_muted)
                .boxed(),
            size_picker.boxed(),
            orientation_control,
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center);

        if matches!(current_size, PageSize::Custom { .. }) {
            controls = controls.push(
                text(t!(i18n, ids::PDF_WIDTH_SHORT))
                    .size(FIELD_TEXT)
                    .color(text_muted)
                    .boxed(),
            );
            controls = controls.push(
                self.pdf_numeric_field(&inputs.custom_width, PrintPreviewMsg::SetCustomWidth),
            );
            controls = controls.push(
                text(t!(i18n, ids::PDF_HEIGHT_SHORT))
                    .size(FIELD_TEXT)
                    .color(text_muted)
                    .boxed(),
            );
            controls = controls.push(
                self.pdf_numeric_field(&inputs.custom_height, PrintPreviewMsg::SetCustomHeight),
            );
            // raw: a unit symbol, the same in every language.
            controls = controls.push(text(raw("mm")).size(FIELD_TEXT).color(text_muted).boxed());
        }

        let modes = Choice::named(&PdfScaleMode::ALL, scale_mode_label, i18n);
        let scale_picker = pick_list(
            chosen(&modes, &options.scale.mode()),
            modes,
            |c: &Choice<PdfScaleMode>| c.label.clone(),
        )
        .graphite()
        .on_select(|c: Choice<PdfScaleMode>| {
            Message::PrintPreview(PrintPreviewMsg::SetScaleMode(c.value))
        });
        controls = controls.push(
            text(t!(i18n, ids::PDF_SCALE))
                .size(FIELD_TEXT)
                .color(text_muted)
                .boxed(),
        );
        controls = controls.push(scale_picker.boxed());
        if matches!(options.scale, PdfScale::Percent(_)) {
            controls = controls
                .push(self.pdf_numeric_field(&inputs.percent, PrintPreviewMsg::SetScalePercent));
            // raw: the percent sign, a symbol.
            controls = controls.push(text(raw("%")).size(FIELD_TEXT).color(text_muted).boxed());
        }
        controls.boxed()
    }

    /// Why the last page-setup edit was rejected (theme error colour) or
    /// automatically adjusted (theme warning colour); an empty spacer when the
    /// last edit applied cleanly.
    pub(in crate::app::view) fn view_pdf_page_setup_notice(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'static, Message> {
        match preview.page_setup.notice() {
            Some(message) => {
                let tokens = &self.document_state.panel_ctx.tokens;
                let colour = if preview.page_setup.notice_is_informational() {
                    tokens.warning
                } else {
                    tokens.error
                };
                // raw: the page-setup notice is worded by its validator (#2061).
                signex_widgets::text(signex_widgets::raw(message.to_string()))
                    .size(FIELD_TEXT)
                    .color(crate::styles::ti(colour))
                    .boxed()
            }
            None => iced::widget::Space::new().height(0).boxed(),
        }
    }

    /// Settings-tab "Page Setup" section: the four margins in mm.
    pub(in crate::app::view) fn view_pdf_page_setup_section(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{Space, column, container, row};
        use signex_i18n::{ids, t};
        use signex_widgets::{raw, text};
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_muted = crate::styles::ti(tokens.text_secondary);

        let mut margins = row![].spacing(SP.m).align_y(iced::Alignment::Center);
        for edge in MarginEdge::ALL {
            margins = margins.push(
                text(t!(i18n, margin_edge_label(edge)))
                    .size(FIELD_TEXT)
                    .color(text_muted)
                    .boxed(),
            );
            margins = margins.push(
                self.pdf_numeric_field(preview.page_setup.margin_text(edge), move |v| {
                    PrintPreviewMsg::SetMargin(edge, v)
                }),
            );
        }
        // raw: a unit symbol, the same in every language.
        margins = margins.push(text(raw("mm")).size(FIELD_TEXT).color(text_muted).boxed());

        column![
            self.pdf_section_title(ids::PDF_SECTION_PAGE_SETUP),
            container(
                column![
                    text(t!(i18n, ids::PDF_MARGINS_HELP))
                        .size(FIELD_TEXT)
                        .color(text_muted)
                        .boxed(),
                    Space::new().height(6).boxed(),
                    margins.boxed(),
                    Space::new().height(6).boxed(),
                    self.view_pdf_page_setup_notice(preview),
                ]
                .spacing(SP.xs)
                .boxed(),
            )
            .padding([SP.m, SP.l])
            .boxed(),
        ]
        .boxed()
    }
}

/// The catalog name of a page-size picker entry.
pub(super) fn page_size_label(kind: PageSizeKind) -> MessageId {
    use signex_i18n::ids;
    match kind {
        PageSizeKind::IsoA0 => ids::PDF_PAGE_SIZE_ISO_A0,
        PageSizeKind::IsoA1 => ids::PDF_PAGE_SIZE_ISO_A1,
        PageSizeKind::IsoA2 => ids::PDF_PAGE_SIZE_ISO_A2,
        PageSizeKind::IsoA3 => ids::PDF_PAGE_SIZE_ISO_A3,
        PageSizeKind::IsoA4 => ids::PDF_PAGE_SIZE_ISO_A4,
        PageSizeKind::IsoA5 => ids::PDF_PAGE_SIZE_ISO_A5,
        PageSizeKind::IsoB5 => ids::PDF_PAGE_SIZE_ISO_B5,
        PageSizeKind::AnsiA => ids::PDF_PAGE_SIZE_ANSI_A,
        PageSizeKind::AnsiB => ids::PDF_PAGE_SIZE_ANSI_B,
        PageSizeKind::AnsiC => ids::PDF_PAGE_SIZE_ANSI_C,
        PageSizeKind::AnsiD => ids::PDF_PAGE_SIZE_ANSI_D,
        PageSizeKind::AnsiE => ids::PDF_PAGE_SIZE_ANSI_E,
        PageSizeKind::UsLetter => ids::PDF_PAGE_SIZE_US_LETTER,
        PageSizeKind::UsLegal => ids::PDF_PAGE_SIZE_US_LEGAL,
        PageSizeKind::Tabloid => ids::PDF_PAGE_SIZE_TABLOID,
        PageSizeKind::Custom => ids::PROPERTIES_FORMAT_CUSTOM,
    }
}

/// The catalog name of a page orientation.
pub(super) fn orientation_label(orientation: SheetOrientation) -> MessageId {
    use signex_i18n::ids;
    match orientation {
        SheetOrientation::Landscape => ids::PROPERTIES_ORIENTATION_LANDSCAPE,
        SheetOrientation::Portrait => ids::PROPERTIES_ORIENTATION_PORTRAIT,
    }
}

/// The catalog name of a scale picker entry.
pub(super) fn scale_mode_label(mode: PdfScaleMode) -> MessageId {
    use signex_i18n::ids;
    match mode {
        PdfScaleMode::FitToPage => ids::BOARD_PRINT_SCALE_FIT,
        PdfScaleMode::OneToOne => ids::BOARD_PRINT_SCALE_ONE,
        PdfScaleMode::Percent => ids::PDF_SCALE_PERCENT,
    }
}

/// The catalog name of a page margin's edge.
pub(super) fn margin_edge_label(edge: MarginEdge) -> MessageId {
    use signex_i18n::ids;
    match edge {
        MarginEdge::Top => ids::PDF_MARGIN_TOP,
        MarginEdge::Right => ids::PDF_MARGIN_RIGHT,
        MarginEdge::Bottom => ids::PDF_MARGIN_BOTTOM,
        MarginEdge::Left => ids::PDF_MARGIN_LEFT,
    }
}
