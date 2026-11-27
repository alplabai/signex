//! The PDF Layers section of the Export PDF dialog (#1298, mockup row 168):
//! the "Write PDF layers" switch heads it; each group is a layer a reader
//! can switch, its box says whether it is visible when the PDF opens.

use iced::Widget as _;
use iced::widget::{column, container, row};
use signex_i18n::{MessageId, ids, t};
use signex_output::PdfLayer;

use super::super::*;
use super::form::{Look, SP, caption, check, gated, switch_section};
use crate::app::state::PreviewState;

/// A layer's row label. The file's group names (`PdfLayer::group_name`)
/// are file text; the dialog names them from the catalog.
pub(super) fn layer_label(layer: PdfLayer) -> MessageId {
    match layer {
        PdfLayer::WiresAndBuses => ids::PDF_LAYER_WIRES,
        PdfLayer::SignalHarnesses => ids::PDF_LAYER_HARNESSES,
        PdfLayer::Parts => ids::PDF_LAYER_PARTS,
        PdfLayer::NetLabelsAndPorts => ids::PDF_LAYER_NET_LABELS,
        PdfLayer::PowerPorts => ids::PDF_LAYER_POWER_PORTS,
        PdfLayer::TitleBlockAndBorder => ids::PDF_LAYER_TITLE_BLOCK,
        PdfLayer::Watermark => ids::PDF_LAYER_WATERMARK,
    }
}

impl Signex {
    /// The PDF Layers section body.
    pub(super) fn view_pdf_layers<'a>(&'a self, preview: &'a PreviewState) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_view.panel_ctx.tokens;
        let look = Look::of(tokens);
        let layers = &preview.dialog.layers;
        let watermark_on = preview.dialog.watermark.is_on;

        let mut rows: iced::widget::Column<Element<'a, Message>> =
            column![caption(t!(i18n, ids::PDF_LAYERS_HELP), &look)]
                .spacing(SP.xs)
                .width(Length::Fill);
        for layer in PdfLayer::ALL.iter().copied() {
            let note = (layer == PdfLayer::Watermark).then(|| {
                caption(
                    t!(
                        i18n,
                        if watermark_on {
                            ids::PDF_LAYER_WATERMARK_LOCKED
                        } else {
                            ids::PDF_LAYER_WATERMARK_OFF
                        }
                    ),
                    &look,
                )
            });
            let mut line: iced::widget::Row<Element<'a, Message>> = row![
                container(check(
                    t!(i18n, layer_label(layer)),
                    layers.is_visible(layer),
                    move |visible| PdfOptionMsg::PdfLayerVisibilityToggled(layer, visible).into(),
                ))
                .width(Length::Fill)
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center);
            if let Some(note) = note {
                line = line.push(note);
            }
            rows = rows.push(gated(line.boxed(), preview.layer_row_enabled(layer), &look));
        }

        let mut body: iced::widget::Column<Element<'a, Message>> = column![switch_section(
            t!(i18n, ids::PDF_LAYERS_SWITCH),
            layers.is_on,
            Message::from(PdfOptionMsg::PdfLayersToggled(!layers.is_on)),
            rows.boxed(),
            &look,
        )]
        .spacing(SP.xs)
        .width(Length::Fill);
        if !layers.is_on {
            body = body.push(caption(t!(i18n, ids::PDF_LAYERS_OFF), &look));
        }
        body.boxed()
    }
}
