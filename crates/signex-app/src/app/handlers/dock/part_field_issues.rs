//! Part fields whose `=` expression does not evaluate (an unknown
//! designator, a unit mismatch, a cycle across parts) are reported in
//! Messages when their sheet is saved (#1986). The canvas draws such a
//! field's source text, and its Properties row shows the error.

use super::super::super::*;
use signex_types::variables::{Resolver, TextTable};

impl Signex {
    /// Write the part-field issues of the open sheet at `path` to Messages.
    pub(crate) fn report_part_field_issues(&self, path: &std::path::Path) {
        let document_state = &self.document_state;
        let Some(engine) = document_state.engines.get(path) else {
            return;
        };
        let sheet = engine.document();
        let no_project = std::collections::BTreeMap::new();
        let project = document_state
            .document_project(path)
            .map_or(&no_project, |project| &project.data.parameters);
        let (sheet_table, project_table) = (TextTable(&sheet.variables), TextTable(project));
        let variables: [&dyn Resolver; 2] = [&sheet_table, &project_table];
        for issue in signex_types::part_fields::part_field_issues(&sheet.symbols, &variables) {
            crate::diagnostics::log_warning(format!("{}: {issue}", path.display()));
        }
    }
}
