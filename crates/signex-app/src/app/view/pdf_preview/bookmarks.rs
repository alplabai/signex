//! The Bookmarks section (#1298, mockup m168) and the folded Bookmarks and
//! Nets section of the Export PDF dialog. "One bookmark per net" is the
//! nets-information setting, shown here only, so one setting has one
//! control.

use iced::Widget as _;
use iced::widget::{column, row, slider};
use signex_i18n::{ids, t};
use signex_output::PartBookmarkOrder;
use signex_widgets::{Label, segmented};

use super::super::*;
use super::form::{
    Look, SP, caption, check, check_with_help, checklist, form_row, gated, indented, net_bookmarks,
};
use crate::app::state::PreviewState;

/// Step of the bookmark zoom slider.
const ZOOM_STEP: f32 = 0.05;

impl Signex {
    /// The Bookmarks section body: per sheet, per part (grouped by sheet or
    /// A–Z, greyed while per part is off) and per net.
    pub(super) fn view_pdf_bookmarks<'a>(
        &'a self,
        preview: &'a PreviewState,
    ) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let look = Look::of(&self.document_state.panel_ctx.tokens);
        let options = &preview.pdf_options;
        let order = segmented(
            [
                (PartBookmarkOrder::BySheet, ids::PDF_BOOKMARK_GROUP_SHEET),
                (PartBookmarkOrder::Alphabetical, ids::PDF_BOOKMARK_GROUP_AZ),
            ]
            .map(|(order, label)| (order, Label::from(t!(i18n, label)))),
            options.part_bookmark_order,
            look.segmented(),
            |order| Message::PrintPreview(PrintPreviewMsg::PartBookmarkOrderPicked(order)),
        );
        column![
            check(
                t!(i18n, ids::PDF_BOOKMARK_SHEETS),
                options.bookmark_sheets,
                PrintPreviewMsg::BookmarkSheetsToggled,
            ),
            check_with_help(
                t!(i18n, ids::PDF_BOOKMARK_PARTS),
                t!(i18n, ids::PDF_BOOKMARK_PARTS_HELP),
                options.bookmark_parts,
                PrintPreviewMsg::BookmarkPartsToggled,
                &look,
            ),
            gated(
                indented(form_row(t!(i18n, ids::PDF_BOOKMARK_GROUP), &look, order)),
                preview.part_order_enabled(),
                &look,
            ),
            check(
                t!(i18n, ids::PDF_BOOKMARK_NETS),
                options.generate_nets_info,
                PrintPreviewMsg::SetGenerateNetsInfo,
            ),
        ]
        .spacing(SP.xs)
        .width(Length::Fill)
        .boxed()
    }

    /// The Bookmarks and Nets section body: the jump zoom, what a net
    /// bookmark lists, component parameters and global bookmarks.
    pub(super) fn view_pdf_bookmarks_and_nets<'a>(
        &'a self,
        preview: &'a PreviewState,
    ) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let look = Look::of(&self.document_state.panel_ctx.tokens);
        let options = &preview.pdf_options;
        let zoom = row![
            caption(t!(i18n, ids::PDF_ZOOM_FAR), &look),
            slider(0.0_f32..=1.0, options.bookmark_zoom, |v| {
                Message::PrintPreview(PrintPreviewMsg::SetBookmarkZoom(v))
            })
            .step(ZOOM_STEP)
            .width(Length::Fill),
            caption(t!(i18n, ids::PDF_ZOOM_CLOSE), &look),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
        let mut body: iced::widget::Column<Element<'a, Message>> = column![
            caption(t!(i18n, ids::PDF_ZOOM_HELP), &look),
            zoom,
            caption(t!(i18n, ids::PDF_NET_BOOKMARKS_HELP), &look),
        ]
        .spacing(SP.xs)
        .width(Length::Fill);
        for row in checklist(&net_bookmarks(), options, i18n) {
            body = body.push(row);
        }
        body.boxed()
    }
}
