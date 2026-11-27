//! The sheet PDF of `export_schematic_pdf` (#2663): the scope, the class
//! styles and the variant the Print Preview's Export uses, with every page
//! and the options the preview first opens with. A headless run has no
//! preview, no current tab and no file picks.

use std::path::Path;

use signex_i18n::{Translatable, ids};
use signex_output::{ExportContext, PdfOptions};

use crate::app::Signex;

impl Signex {
    /// What a PDF of `source`'s project prints: its pages and the options.
    /// The stitch and numbering problems go to Messages, once, as the
    /// preview's Export logs them. Refused with the reason when `source` is
    /// not open, the project's channel naming cannot be read or there is no
    /// page.
    pub(crate) fn schematic_pdf_job(
        &mut self,
        source: &Path,
    ) -> Result<(ExportContext, PdfOptions), Translatable> {
        self.commit_drafts_in_scope_of(Some(source));
        let Some((mut ctx, issues)) =
            super::build_export_scope_for_source(&self.document_state, source)
        else {
            return Err(Translatable::new(ids::NOTICE_PDF_SOURCE_CLOSED)
                .arg("path", source.display().to_string()));
        };
        if issues.channel_naming_unreadable {
            return Err(Translatable::new(ids::DIALOG_SCHEMATIC_DESIGN_UNREADABLE));
        }
        let mut options = PdfOptions {
            net_class_styles: super::class_styles::export_class_styles(
                &self.document_state,
                source,
                &ctx,
            ),
            schematic_defaults: self.document_state.schematic_defaults_for(source),
            draw_not_fitted: self.ui_state.cross_out_not_fitted,
            ..PdfOptions::default()
        };
        super::log_stitch_issues(source, &ctx, &issues);
        super::sheet_numbers::log_numbering_issues(issues.sheet_numbering.as_ref());
        if ctx.sheets.is_empty() {
            return Err(Translatable::new(ids::PDF_EXPORT_NO_PAGES)
                .arg("document", source.display().to_string()));
        }
        super::apply_printed_variant(&mut ctx.metadata, &mut options.variant);
        Ok((ctx, options))
    }
}
