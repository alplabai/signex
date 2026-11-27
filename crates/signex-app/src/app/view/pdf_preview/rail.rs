//! The page list on the left of the Export PDF dialog: one thumbnail per
//! sheet, an unticked sheet's dimmed rather than removed (mockup row 111).

use iced::widget::{Space, column, container, image, scrollable};
use iced::{Border, Widget as _};
use signex_i18n::{ids, t};
use signex_output::PageRange;
use signex_widgets::text;

use super::super::*;
use super::form::{Look, SP, TS};
use crate::app::state::{PageKey, PreviewState};

/// Width of the page list.
pub(super) const RAIL_W: f32 = 152.0;
/// Size of a thumbnail page (the A4-landscape box of the mockup).
const THUMB_W: f32 = 128.0;
const THUMB_H: f32 = 91.0;
/// Padding of the list and the gap between its thumbnails.
const RAIL_PAD: f32 = SP.m + SP.xs;

/// What a thumbnail stands for.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum RailCaption<'a> {
    /// A sheet of the export, `number` its place in the list.
    Sheet {
        number: usize,
        name: &'a str,
        key: &'a PageKey,
    },
    /// A printed page when the pages cannot be matched to the sheets (a
    /// Current or Specific range): its sheet number.
    Page { page_number: usize },
}

/// One thumbnail of the page list.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RailEntry<'a> {
    pub caption: RailCaption<'a>,
    /// The preview page it shows, `None` when the sheet is not printed.
    pub page: Option<usize>,
    /// Ticked under Sheets; an unticked sheet's thumbnail is dimmed.
    pub ticked: bool,
}

/// The page list: one entry per sheet in page order. Under All the ticked
/// sheets print one page each in that order, so each carries its page.
/// Under another range the printed pages cannot be told apart by sheet,
/// so the list holds the printed pages themselves.
pub(super) fn rail_entries(preview: &PreviewState) -> Vec<RailEntry<'_>> {
    let ticked: Vec<bool> = preview
        .sheet_files
        .iter()
        .map(|(key, _)| preview.selected_files.contains(key))
        .collect();
    let ticked_count = ticked.iter().filter(|t| **t).count();
    let maps_to_sheets = matches!(preview.pdf_options.page_range, PageRange::All)
        && ticked_count == preview.pages.len();
    if !maps_to_sheets && !preview.pages.is_empty() {
        return preview
            .pages
            .iter()
            .enumerate()
            .map(|(position, page)| RailEntry {
                caption: RailCaption::Page {
                    page_number: page.page_number,
                },
                page: Some(position),
                ticked: true,
            })
            .collect();
    }
    let mut next_page = 0;
    preview
        .sheet_files
        .iter()
        .zip(ticked)
        .enumerate()
        .map(|(index, ((key, name), ticked))| {
            let page = (ticked && maps_to_sheets).then(|| {
                next_page += 1;
                next_page - 1
            });
            RailEntry {
                caption: RailCaption::Sheet {
                    number: index + 1,
                    name,
                    key,
                },
                page,
                ticked,
            }
        })
        .collect()
}

impl Signex {
    /// The page list column.
    pub(super) fn view_pdf_rail<'a>(&'a self, preview: &'a PreviewState) -> Element<'a, Message> {
        let tokens = &self.document_view.panel_ctx.tokens;
        let look = Look::of(tokens);
        let mut list = column![]
            .spacing(RAIL_PAD)
            .padding(RAIL_PAD)
            .width(Length::Fill);
        for entry in rail_entries(preview) {
            list = list.push(self.view_pdf_thumb(preview, entry, &look));
        }
        container(
            scrollable(list)
                .direction(crate::styles::thin_vertical())
                .style(crate::styles::scrollbar_style(tokens))
                .height(Length::Fill),
        )
        .width(RAIL_W)
        .height(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(look.sunken.into()),
            ..container::Style::default()
        })
        .boxed()
    }

    fn view_pdf_thumb<'a>(
        &'a self,
        preview: &'a PreviewState,
        entry: RailEntry<'a>,
        look: &Look,
    ) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let look = *look;
        let is_selected = entry.page == Some(preview.selected);
        // A page not drawn yet is a blank sheet: it is rasterised when shown.
        let handle = entry
            .page
            .and_then(|position| preview.pages.get(position))
            .and_then(|page| page.handle());
        let face: Element<'a, Message> = match handle {
            Some(handle) => image(handle.clone())
                .content_fit(iced::ContentFit::Contain)
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
            None => Space::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
        };
        let page = container(face)
            .width(THUMB_W)
            .height(THUMB_H)
            .padding(1)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Color::WHITE.into()),
                border: Border {
                    color: if is_selected {
                        look.accent
                    } else {
                        look.line_strong
                    },
                    width: if is_selected { 2.0 } else { 1.0 },
                    radius: 3.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });
        let label: signex_widgets::Label<'a> = match entry.caption {
            RailCaption::Sheet { number, name, .. } => t!(
                i18n,
                ids::PDF_THUMB_CAPTION,
                "number" => number,
                "name" => name
            )
            .into(),
            RailCaption::Page { page_number } => {
                t!(i18n, ids::PDF_PAGE_N, "page" => page_number.to_string()).into()
            }
        };
        let card = column![
            page,
            text(label)
                .size(TS.caption)
                .color(look.ink3)
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End),
        ]
        .spacing(SP.xs)
        .align_x(iced::Alignment::Center)
        .width(Length::Fill);
        match entry.page {
            _ if !entry.ticked => super::form::gated(
                card.boxed(),
                false,
                &Look {
                    panel: look.sunken,
                    ..look
                },
            ),
            Some(position) => iced::widget::mouse_area(card)
                .on_press(Message::PrintPreview(PrintPreviewMsg::SelectPage(position)))
                .interaction(iced::mouse::Interaction::Pointer)
                .boxed(),
            None => card.boxed(),
        }
    }
}
