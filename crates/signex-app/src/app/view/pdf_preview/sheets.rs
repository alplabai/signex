//! The Sheets and Pages sections of the Export PDF dialog: which sheets
//! print (Select All · Clear as links), and All / Current / Specific.

use iced::Widget as _;
use iced::widget::{column, row};
use signex_i18n::{MessageId, ids, t};
use signex_output::PageRange;
use signex_widgets::{Label, segmented, text, text_input};

use super::super::*;
use super::form::{Look, SP, TS, caption, check};
use crate::app::state::PreviewState;

/// The Pages switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PagesChoice {
    All,
    Current,
    Specific,
}

/// "N of M": the ticked sheets of all the sheets the export can cover.
pub(super) fn sheets_summary(preview: &PreviewState) -> (usize, usize) {
    let ticked = preview
        .sheet_files
        .iter()
        .filter(|(key, _)| preview.selected_files.contains(key))
        .count();
    (ticked, preview.sheet_files.len())
}

impl Signex {
    /// The Sheets section body: the links, then one checkbox per sheet.
    pub(super) fn view_pdf_sheets<'a>(&'a self, preview: &'a PreviewState) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let look = Look::of(&self.document_state.panel_ctx.tokens);
        if preview.sheet_files.is_empty() {
            return caption(t!(i18n, ids::PDF_NO_SHEETS), &look);
        }
        let link = |label: MessageId, msg: PrintPreviewMsg| -> Element<'a, Message> {
            let ink = look.accent_text;
            signex_widgets::button(text(t!(i18n, label)).size(TS.body).color(ink))
                .padding(0)
                .on_press(Message::PrintPreview(msg))
                .style(move |_: &iced::Theme, _| signex_widgets::button::Style {
                    text_color: ink,
                    ..signex_widgets::button::Style::default()
                })
                .boxed()
        };
        let mut list = column![
            row![
                link(ids::PDF_SELECT_ALL, PrintPreviewMsg::SelectAllFiles),
                caption(t!(i18n, ids::PDF_LINK_SEPARATOR), &look),
                link(ids::PDF_CLEAR, PrintPreviewMsg::ClearAllFiles),
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(SP.xs)
        .width(Length::Fill);
        for (page, _name) in &preview.sheet_files {
            let file = page.0.file_name().map_or_else(
                || page.0.display().to_string(),
                |f| f.to_string_lossy().into_owned(),
            );
            let key = page.clone();
            list = list.push(check(
                signex_i18n::UserText::new(file),
                preview.selected_files.contains(page),
                move |_| PrintPreviewMsg::ToggleFile(key.clone()),
            ));
        }
        list.boxed()
    }

    /// The Pages section body: All / Current / Specific and the page field,
    /// which takes input only for Specific.
    pub(super) fn view_pdf_pages<'a>(&'a self, preview: &'a PreviewState) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let look = Look::of(&self.document_state.panel_ctx.tokens);
        let current = match preview.pdf_options.page_range {
            PageRange::All => PagesChoice::All,
            PageRange::Current => PagesChoice::Current,
            // A range is a list of specific pages; the dialog never sets one.
            PageRange::Specific(_) | PageRange::Range(..) => PagesChoice::Specific,
        };
        let options = [
            (PagesChoice::All, ids::PDF_PAGES_ALL),
            (PagesChoice::Current, ids::PDF_PAGES_CURRENT),
            (PagesChoice::Specific, ids::PDF_PAGES_SPECIFIC),
        ]
        .map(|(choice, label)| (choice, Label::from(t!(i18n, label))));
        let switch = segmented(options, current, look.segmented(), |choice| {
            Message::PrintPreview(match choice {
                PagesChoice::All => PrintPreviewMsg::SetPageRangeAll,
                PagesChoice::Current => PrintPreviewMsg::SetPageRangeCurrent,
                PagesChoice::Specific => PrintPreviewMsg::SetPageRangeSpecific,
            })
        });
        let field = text_input(
            t!(i18n, ids::PLACEHOLDER_PDF_PAGE),
            &preview.specific_page_input,
        )
        .padding([SP.s, SP.m])
        .size(TS.body)
        .width(Length::Fill)
        .style(crate::panels::form_look::text_input_style);
        let field = if current == PagesChoice::Specific {
            field.on_input(|v| Message::PrintPreview(PrintPreviewMsg::SetSpecificPageInput(v)))
        } else {
            field
        };
        column![switch, field]
            .spacing(SP.s)
            .width(Length::Fill)
            .boxed()
    }
}
