//! The one-screen Export PDF dialog (#2430, mockups m111 and m168): the
//! page list on the left, the page in the middle, the settings column on
//! the right. The modal shell (header, footer) is in `print_preview.rs`.
//! Rules: docs/MENU_BAR.md, "File ▸ Export ▸ PDF".

use iced::Widget as _;
use iced::widget::row;
use signex_i18n::{ids, t};

use super::*;
use crate::app::state::PreviewState;

mod bookmarks;
mod form;
mod layers;
mod page_setup;
mod rail;
mod settings;
mod sheets;
#[cfg(test)]
mod tests;
mod viewer;
mod watermark;
mod watermark_overlay;

impl Signex {
    /// The dialog body between header and footer.
    pub(in crate::app::view) fn view_pdf_dialog_body<'a>(
        &'a self,
        preview: &'a PreviewState,
    ) -> Element<'a, Message> {
        let look = form::Look::of(&self.document_state.panel_ctx.tokens);
        row![
            self.view_pdf_rail(preview),
            form::rule(look.line_soft, true),
            self.view_pdf_viewer(preview),
            form::rule(look.line_soft, true),
            self.view_pdf_settings_column(preview),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
    }

    /// The header's second line: the project (or document) file and how
    /// many sheets it has.
    pub(in crate::app::view) fn pdf_dialog_subtitle(&self, preview: &PreviewState) -> String {
        let file = self
            .document_state
            .document_project(&preview.source)
            .map_or(preview.source.as_path(), |project| project.path.as_path())
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.ui_state.i18n.message_plain(
            ids::PDF_DIALOG_SUBTITLE,
            &[
                ("file", file.into()),
                ("count", preview.sheet_files.len().into()),
            ],
        )
    }

    /// The footer caption: the last export's outcome while it is current,
    /// else the pages, the paper and the quality.
    pub(in crate::app::view) fn pdf_footer_caption<'a>(
        &self,
        preview: &'a PreviewState,
    ) -> signex_widgets::Label<'a> {
        let i18n = self.ui_state.i18n.as_ref();
        match (&preview.export_status, preview.pages.is_empty()) {
            // raw: the export status is worded by the export handler (#2061).
            (Some(status), _) => signex_widgets::raw(status.clone()),
            (None, true) => t!(i18n, ids::DIALOG_PRINT_PREVIEW_NO_FILES).into(),
            (None, false) => t!(
                i18n,
                ids::PDF_FOOTER_STATUS,
                "count" => preview.pages.len(),
                "paper" => viewer::paper_name(&preview.pdf_options, i18n),
                "quality" => signex_i18n::Translatable::new(preview.quality.label())
            )
            .into(),
        }
    }
}
