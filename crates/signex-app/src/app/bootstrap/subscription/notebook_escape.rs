//! The Esc rungs of the Design Notebook dialogs: Generate from Project
//! (mockup row 201) and Export Design Notebook PDF (mockup row 202).

use super::OpenOverlays;
use crate::app::view::overlay_id::OverlayId;
use crate::app::{Message, NotebookGenerateMsg, NotebookPdfMsg};

impl OpenOverlays {
    /// Esc on an open notebook dialog closes it.
    pub(super) fn notebook_escape(&self, id: OverlayId) -> Option<Message> {
        match id {
            OverlayId::NotebookGenerate => self
                .notebook_generate_open
                .then(|| Message::from(NotebookGenerateMsg::DialogCancelled)),
            OverlayId::NotebookPdf => self
                .notebook_pdf_open
                .then(|| Message::from(NotebookPdfMsg::Closed)),
            _ => None,
        }
    }
}
