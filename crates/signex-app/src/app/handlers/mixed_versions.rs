//! Messages-panel warning for a part whose units draw with different versions
//! of their library symbol (#2209). Nothing is changed: the user decides when
//! to run Update from library.

use crate::app::Signex;

impl Signex {
    /// Warn about the parts that contain one of `touched` (per sheet) and
    /// whose units use different versions. Called with what a placement put
    /// down and what Annotate renumbered, the two ways a unit can join a part
    /// of another version, so an old mixed part is not reported again by an
    /// unrelated edit. Only sheets with an open engine are read.
    pub(crate) fn warn_mixed_version_parts(
        &self,
        touched: &std::collections::HashMap<
            std::path::PathBuf,
            std::collections::HashSet<uuid::Uuid>,
        >,
    ) {
        for (path, ids) in touched {
            let Some(engine) = self.document_state.engines.get(path) else {
                continue;
            };
            for reference in engine.document().mixed_version_parts_among(ids) {
                crate::diagnostics::log_warning(mixed_version_text(&reference, path));
            }
        }
    }

    /// After a numbering pass: warn about the parts that contain a symbol
    /// whose designator the pass changed (`before` maps sheet -> uuid ->
    /// designator), which is how Annotate joins `U?` units into one part.
    pub(crate) fn warn_mixed_after_renumbering(
        &self,
        before: &std::collections::HashMap<
            std::path::PathBuf,
            std::collections::HashMap<uuid::Uuid, String>,
        >,
    ) {
        let touched = before
            .iter()
            .filter_map(|(path, refs)| {
                let engine = self.document_state.engines.get(path)?;
                let ids: std::collections::HashSet<uuid::Uuid> = (engine.document().symbols.iter())
                    .filter(|s| refs.get(&s.uuid).is_some_and(|old| *old != s.reference))
                    .map(|s| s.uuid)
                    .collect();
                Some((path.clone(), ids))
            })
            .collect();
        self.warn_mixed_version_parts(&touched);
    }
}

/// The Messages line for a part whose units draw with different versions.
fn mixed_version_text(reference: &str, path: &std::path::Path) -> String {
    format!(
        "{reference} on {}: its units use different versions of the library symbol; \
         use Update from library to bring them together",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_warning_is_one_sentence_with_no_run_of_spaces() {
        let text = super::mixed_version_text("U1", std::path::Path::new("a.snxsch"));
        assert!(!text.contains("  "), "{text:?}");
        assert!(text.contains("symbol; use Update from library"));
    }
}
