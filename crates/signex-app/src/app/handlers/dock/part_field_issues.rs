//! Part fields whose `=` expression does not evaluate (an unknown
//! designator, a unit mismatch, a cycle across parts, a typed `-` sign) are
//! reported in Messages when their sheet is saved, by Ctrl+S, Save All or a
//! save before a close, and when an export reads it (#1986). The canvas, the
//! outputs and the BOM show such a field's source text, and its Properties
//! row shows the error.

use super::super::super::*;
use signex_types::designator::DesignatorStandard;
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
        let signed = document_state
            .designator_standard_for(path)
            .is_some_and(DesignatorStandard::shows_product_aspect_sign);
        let (sheet_table, project_table) = (TextTable(&sheet.variables), TextTable(project));
        let variables: [&dyn Resolver; 2] = [&sheet_table, &project_table];
        let issues =
            signex_types::part_fields::part_field_issues(&sheet.symbols, &variables, signed);
        for issue in issues {
            crate::diagnostics::log_warning(format!("{}: {issue}", path.display()));
        }
    }

    /// [`Self::report_part_field_issues`] for `anchor` and the open sheets
    /// of its project: what an export (PDF, BOM, netlist) reads.
    pub(crate) fn report_part_field_issues_in_scope_of(&self, anchor: &std::path::Path) {
        let document_state = &self.document_state;
        let project = document_state.document_project(anchor).map(|p| p.id);
        let in_scope = |path: &std::path::Path| {
            path == anchor
                || project.is_some_and(|id| {
                    document_state
                        .document_project(path)
                        .is_some_and(|p| p.id == id)
                })
        };
        let mut paths: Vec<&std::path::PathBuf> = document_state
            .engines
            .keys()
            .filter(|path| in_scope(path.as_path()))
            .collect();
        paths.sort();
        for path in paths {
            self.report_part_field_issues(path);
        }
    }
}
