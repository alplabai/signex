//! The Page Setup section of the Export PDF dialog: Size, Orientation,
//! Scale and the four margins as two rows of two, in mm from the page edge.
//! Pure views — every edit is a `PrintPreviewMsg` the handlers validate.

use iced::Widget as _;
use iced::widget::{Space, column, pick_list, row};
use signex_i18n::{ids, t};
use signex_output::{MarginEdge, PageSize, PageSizeKind, PdfScale, PdfScaleMode};
use signex_types::schematic::SheetOrientation;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{segmented, text};

use super::super::*;
use super::form::{KEY_GAP, KEY_W, Look, NUM_W, SP, TS, caption, form_row, num_field, unit};
use crate::app::state::PreviewState;

/// Width of a margin's edge name in the two-by-two grid.
const EDGE_W: f32 = 40.0;

impl Signex {
    /// The Page Setup section body.
    pub(super) fn view_pdf_page_setup<'a>(
        &'a self,
        preview: &'a PreviewState,
    ) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let look = Look::of(&self.document_state.panel_ctx.tokens);
        let options = &preview.pdf_options;
        let inputs = &preview.page_setup;
        let current_size = options.page_size;
        let orientation = options.orientation;
        let is_custom = matches!(current_size, PageSize::Custom { .. });

        let size_picker = pick_list(
            Some(current_size.kind()),
            PageSizeKind::ALL.to_vec(),
            |kind: &PageSizeKind| kind.to_string(),
        )
        .graphite()
        .text_size(TS.body)
        .width(Length::Fill)
        .on_select(move |kind: PageSizeKind| {
            Message::PrintPreview(PrintPreviewMsg::SetPageSize(
                kind.select(current_size, orientation),
            ))
        });

        // A Custom page is exactly the width and height entered, so its
        // orientation follows from them and is shown, not picked.
        let orientation_label = |o: SheetOrientation| match o {
            SheetOrientation::Portrait => t!(i18n, ids::PDF_ORIENTATION_PORTRAIT),
            SheetOrientation::Landscape => t!(i18n, ids::PDF_ORIENTATION_LANDSCAPE),
        };
        let orientation_control: Element<'a, Message> = if is_custom {
            text(orientation_label(orientation))
                .size(TS.body)
                .color(look.ink3)
                .boxed()
        } else {
            segmented(
                [SheetOrientation::Portrait, SheetOrientation::Landscape]
                    .map(|o| (o, signex_widgets::Label::from(orientation_label(o)))),
                orientation,
                look.segmented(),
                |o| Message::PrintPreview(PrintPreviewMsg::SetOrientation(o)),
            )
        };

        let scale_picker = pick_list(
            Some(options.scale.mode()),
            PdfScaleMode::ALL.to_vec(),
            |m: &PdfScaleMode| m.to_string(),
        )
        .graphite()
        .text_size(TS.body)
        .width(Length::Fill)
        .on_select(|m| Message::PrintPreview(PrintPreviewMsg::SetScaleMode(m)));
        // The percent field takes input only for a Percent scale.
        let percent_input: Option<fn(String) -> PrintPreviewMsg> =
            matches!(options.scale, PdfScale::Percent(_))
                .then_some(PrintPreviewMsg::SetScalePercent);
        let scale = row![
            scale_picker,
            num_field(&inputs.percent, Length::Fixed(NUM_W), percent_input),
            unit(t!(i18n, ids::PDF_UNIT_PERCENT), &look),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);

        let mut rows = column![form_row(
            t!(i18n, ids::PDF_SIZE),
            &look,
            size_picker.boxed()
        )]
        .spacing(SP.s + SP.xs)
        .width(Length::Fill);
        if is_custom {
            let custom = row![
                text(t!(i18n, ids::PDF_WIDTH_SHORT))
                    .size(TS.body)
                    .color(look.ink3),
                num_field(
                    &inputs.custom_width,
                    Length::Fill,
                    Some(PrintPreviewMsg::SetCustomWidth)
                ),
                text(t!(i18n, ids::PDF_HEIGHT_SHORT))
                    .size(TS.body)
                    .color(look.ink3),
                num_field(
                    &inputs.custom_height,
                    Length::Fill,
                    Some(PrintPreviewMsg::SetCustomHeight)
                ),
                unit(t!(i18n, ids::PDF_UNIT_MM), &look),
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center);
            rows = rows.push(form_row(
                t!(i18n, ids::PDF_CUSTOM_SIZE),
                &look,
                custom.boxed(),
            ));
        }
        // The margins caption sits under the fields, past the label column.
        let margins_help = row![
            Space::new().width(KEY_W),
            caption(t!(i18n, ids::PDF_MARGINS_HELP), &look),
        ]
        .spacing(KEY_GAP);
        rows = rows
            .push(form_row(
                t!(i18n, ids::PDF_ORIENTATION),
                &look,
                orientation_control,
            ))
            .push(form_row(t!(i18n, ids::PDF_SCALE), &look, scale.boxed()))
            .push(form_row(
                t!(i18n, ids::PDF_MARGINS),
                &look,
                self.view_pdf_margins(preview, &look),
            ))
            .push(margins_help.boxed())
            .push(self.view_pdf_page_setup_notice(preview));
        rows.boxed()
    }

    /// The margins as two rows of two: Top / Bottom, Left / Right.
    fn view_pdf_margins<'a>(
        &'a self,
        preview: &'a PreviewState,
        look: &Look,
    ) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let ink4 = look.ink4;
        let edge = |edge: MarginEdge| -> Element<'a, Message> {
            let name = match edge {
                MarginEdge::Top => ids::PDF_MARGIN_TOP,
                MarginEdge::Bottom => ids::PDF_MARGIN_BOTTOM,
                MarginEdge::Left => ids::PDF_MARGIN_LEFT,
                MarginEdge::Right => ids::PDF_MARGIN_RIGHT,
            };
            let field = signex_widgets::text_input(
                signex_i18n::UserText::new(String::new()),
                preview.page_setup.margin_text(edge).to_owned(),
            )
            .padding([SP.s, SP.m])
            .size(TS.body)
            .width(Length::Fill)
            .style(crate::panels::form_look::text_input_style)
            .on_input(move |v| Message::PrintPreview(PrintPreviewMsg::SetMargin(edge, v)));
            row![
                text(t!(i18n, name))
                    .size(TS.caption)
                    .color(ink4)
                    .width(EDGE_W)
                    .wrapping(iced::widget::text::Wrapping::None)
                    .ellipsis(iced::widget::text::Ellipsis::End),
                field,
            ]
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center)
            .width(Length::FillPortion(1))
            .boxed()
        };
        column![
            row![edge(MarginEdge::Top), edge(MarginEdge::Bottom)].spacing(SP.m),
            row![edge(MarginEdge::Left), edge(MarginEdge::Right)].spacing(SP.m),
        ]
        .spacing(SP.xs)
        .boxed()
    }

    /// Why the last page-setup edit was rejected (theme error colour) or
    /// automatically adjusted (theme warning colour); an empty spacer when the
    /// last edit applied cleanly.
    pub(in crate::app::view) fn view_pdf_page_setup_notice(
        &self,
        preview: &PreviewState,
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
                text(signex_widgets::raw(message.to_string()))
                    .size(TS.body)
                    .color(crate::styles::ti(colour))
                    .boxed()
            }
            None => Space::new().height(0).boxed(),
        }
    }
}
