//! Export Design Notebook PDF (mockup row 202): the settings form, with the
//! stale-reference banner when the check found any, and the done card.
//! State and rules: `app/notebook_pdf/`.

mod done;
mod form;

#[cfg(test)]
mod tests;

use iced::widget::{Space, column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;

use super::sizes;
use crate::app::notebook_pdf::{NotebookPdfDialog, Stage};
use crate::app::{Message, NotebookPdfMsg, Signex};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

type Parts<'a> = (
    Element<'a, Message>,
    Element<'a, Message>,
    Element<'a, Message>,
);

fn out(msg: NotebookPdfMsg) -> Message {
    Message::from(msg)
}

impl Signex {
    pub(in crate::app::view) fn view_notebook_pdf_dialog(&self) -> Element<'_, Message> {
        let Some(dialog) = self.ui_state.notebook_pdf.dialog.as_ref() else {
            return container(Space::new()).boxed();
        };
        let size = match dialog.stage {
            Stage::Done(_) => sizes::NOTEBOOK_PDF_DONE,
            _ => sizes::NOTEBOOK_PDF,
        };
        let tokens = &self.document_view.panel_ctx.tokens;
        let (header, body, footer) = parts(dialog, self.ui_state.i18n.as_ref(), tokens);
        let card = container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, size.0),
        )))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed();
        self.wrap_centred_dialog(card, size)
    }
}

/// The header, body and footer of `dialog` as it stands.
pub(super) fn parts<'a>(
    dialog: &'a NotebookPdfDialog,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Parts<'a> {
    match &dialog.stage {
        Stage::Done(done) => done::parts(done, i18n, tokens),
        _ => form::parts(dialog, i18n, tokens),
    }
}

/// The header's second line: the notebook file, then the document number
/// and revision when the title block has them.
fn subtitle(dialog: &NotebookPdfDialog, i18n: &I18n) -> String {
    let title = dialog.inspection().map(|inspection| &inspection.title);
    let doc = title.and_then(|title| title.doc.as_deref());
    let rev = title.and_then(|title| title.rev.as_deref());
    match (doc, rev) {
        (Some(doc), Some(rev)) => t!(
            i18n,
            ids::NOTEBOOK_PDF_SUBTITLE_REV,
            "file" => dialog.notebook.as_str(),
            "doc" => doc,
            "rev" => rev
        )
        .into_string(),
        (Some(doc), None) => t!(
            i18n,
            ids::NOTEBOOK_PDF_SUBTITLE,
            "file" => dialog.notebook.as_str(),
            "doc" => doc
        )
        .into_string(),
        _ => dialog.notebook.clone(),
    }
}

/// `bytes` for the done card: `820 KB`, `1.4 MB`.
pub(super) fn file_size(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    let n = bytes as f64;
    if n < KB * KB {
        format!("{} KB", (n / KB).ceil().max(1.0) as u64)
    } else {
        format!("{:.1} MB", n / (KB * KB))
    }
}
