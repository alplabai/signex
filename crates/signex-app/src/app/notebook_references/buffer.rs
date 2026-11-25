//! Where a reference fix and Open in notebook meet the notebook tab. A
//! notebook file is written only on Save, so a fix edits the text the
//! notebook tab holds, never the file on disk. Until the tab holds a
//! notebook, both say so in Messages.

use signex_i18n::{ids, t};
use signex_notebook::{Fix, Reference, ReferenceReport, apply_fix, apply_plan, plan_update_all};

use crate::app::Signex;

impl Signex {
    /// The text of the notebook file `file` (inside `notebook/`) as the
    /// notebook tab holds it, unsaved edits included; `None` while no tab
    /// holds it. The notebook tab answers this.
    pub(crate) fn notebook_buffer(&self, _file: &str) -> Option<String> {
        None
    }

    /// Put `text` in the notebook tab's buffer of `file`, as one undo step
    /// of the tab, written on Save. The notebook tab answers this.
    pub(crate) fn replace_notebook_buffer(
        &mut self,
        file: &str,
        _text: String,
    ) -> Result<(), String> {
        let line = t!(self.ui_state.i18n, ids::NOTEBOOK_REF_OPEN_FIRST, "file" => file);
        Err(line.into_string())
    }

    /// Open the notebook tab at the block of `file` starting on `line`.
    /// The notebook tab answers this.
    pub(crate) fn open_notebook_at(&mut self, file: &str, line: u32) {
        let text = t!(
            self.ui_state.i18n, ids::NOTEBOOK_REF_OPEN_AT,
            "file" => file, "line" => line
        );
        crate::diagnostics::log_info(text.into_string());
    }

    /// Update all: every renamed reference of `report` updated in the
    /// notebook tab's text of `file`; the others are left for a choice.
    pub(crate) fn apply_update_all(
        &mut self,
        file: &str,
        report: &ReferenceReport,
    ) -> Result<usize, String> {
        let Some(text) = self.notebook_buffer(file) else {
            let line = t!(self.ui_state.i18n, ids::NOTEBOOK_REF_OPEN_FIRST, "file" => file);
            return Err(line.into_string());
        };
        let fixed = apply_plan(&text, &plan_update_all(report)).map_err(|why| why.to_string())?;
        self.replace_notebook_buffer(file, fixed.text)?;
        Ok(fixed.changed)
    }

    /// Apply `fix` to `reference` in the notebook tab's text of `file`;
    /// how many uses changed, or why nothing did.
    pub(crate) fn fix_reference_in_buffer(
        &mut self,
        file: &str,
        reference: &Reference,
        fix: &Fix,
    ) -> Result<usize, String> {
        let Some(text) = self.notebook_buffer(file) else {
            let line = t!(self.ui_state.i18n, ids::NOTEBOOK_REF_OPEN_FIRST, "file" => file);
            return Err(line.into_string());
        };
        let fixed = apply_fix(&text, reference, fix).map_err(|why| why.to_string())?;
        self.replace_notebook_buffer(file, fixed.text)?;
        Ok(fixed.changed)
    }
}
