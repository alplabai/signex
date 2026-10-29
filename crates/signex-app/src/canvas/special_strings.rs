//! The values `${TOKEN}` special strings resolve to on one sheet (#1080),
//! kept by the canvas so a text item shows what the PDF and SVG print for it.
//! Resolution is `signex_output::resolve`, the resolver the title block uses.

use signex_output::{PdfOptions, ProjectMetadata, SubstitutionContext};

/// An owned [`SubstitutionContext`]: what the canvas holds between frames.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecialStrings {
    pub metadata: ProjectMetadata,
    pub filename: String,
    pub sheet_name: String,
    /// `None` when the sheet has no number in its project (a hierarchical
    /// child, say): it prints none rather than another sheet's.
    pub sheet_number: Option<usize>,
    pub sheet_count: usize,
}

impl SpecialStrings {
    pub fn resolve(&self, text: &str) -> String {
        let defaults = PdfOptions::default();
        signex_output::resolve(
            text,
            &SubstitutionContext {
                metadata: &self.metadata,
                filename: self.filename.clone(),
                sheet_name: self.sheet_name.clone(),
                sheet_number: self.sheet_number.unwrap_or(0),
                sheet_count: self.sheet_count,
                signex_version: env!("CARGO_PKG_VERSION"),
                variant: defaults.variant,
                physical_structure: defaults.use_physical_structure,
                physical_sheet_number: defaults.physical_sheet_number
                    && self.sheet_number.is_some(),
                physical_document_number: defaults.physical_document_number,
            },
        )
    }
}
