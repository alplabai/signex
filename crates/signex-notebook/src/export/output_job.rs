//! The Design Notebook PDF output of an Output Job (mockup row 202's Add as
//! Output Job output…): the export dialog's settings kept as one output, so
//! a job run exports the notebook with the same code and the same result
//! as the dialog and `export_notebook_pdf`.

use serde::{Deserialize, Serialize};

use super::options::{ExportOptions, PageSelection};

/// The kind token of the output in an Output Job.
pub const NOTEBOOK_PDF_OUTPUT_KIND: &str = "design_notebook_pdf";

/// One Design Notebook PDF output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotebookPdfOutput {
    /// Always [`NOTEBOOK_PDF_OUTPUT_KIND`].
    pub kind: String,
    /// Whether a job run makes it.
    pub enabled: bool,
    /// The notebook file, as a path inside the project's `notebook/` folder.
    pub notebook: String,
    /// The export settings. A job has no current page, so the dialog's
    /// Current page is kept as a one-page range.
    pub options: ExportOptions,
}

impl NotebookPdfOutput {
    /// The output that exports `notebook` with `options`, enabled.
    pub fn new(notebook: impl Into<String>, options: &ExportOptions) -> Self {
        let mut options = options.clone();
        if let PageSelection::Current { page } = options.pages {
            options.pages = PageSelection::Range {
                first: page,
                last: page,
            };
        }
        Self {
            kind: NOTEBOOK_PDF_OUTPUT_KIND.to_string(),
            enabled: true,
            notebook: notebook.into(),
            options,
        }
    }

    /// The output as the TOML table an Output Job stores.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string(self).map_err(|e| e.to_string())
    }

    /// The output read back from [`Self::to_toml`]; refused when the kind
    /// is another output's.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let output: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if output.kind != NOTEBOOK_PDF_OUTPUT_KIND {
            return Err(format!(
                "the output kind is `{}`, not `{NOTEBOOK_PDF_OUTPUT_KIND}`",
                output.kind
            ));
        }
        Ok(output)
    }
}
